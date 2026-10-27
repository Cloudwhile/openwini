use axum::extract::{Path, State};
use axum::http::{header, HeaderValue};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};

use crate::error::ApiResult;
use crate::state::AppState;
use crate::validation::{self, CatalogEntry, MAX_RESPONSE_BYTES, MAX_SOURCES};

/// 源条目（字段名与顺序即宿主契约）
#[derive(Serialize, Deserialize)]
pub(crate) struct SourceEntry {
    #[serde(rename = "developerId")]
    developer_id: String,
    #[serde(rename = "developerName")]
    developer_name: String,
    #[serde(rename = "repositoryUrl")]
    repository_url: String,
    #[serde(rename = "pluginsUrl")]
    plugins_url: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sources", get(list_sources))
        .route("/sources/{developer_id}/plugins", get(source_plugins))
}

fn json_response(body: String) -> Response {
    let mut resp = Response::new(axum::body::Body::from(body));
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    // 宿主索引允许短时缓存：下架/暂停/停止后最多 60 秒内同步到客户端
    resp.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=60"),
    );
    resp
}

/// 公开目录开关：管理员关闭后，网页目录与宿主索引一并停摆（返回空集，保持宿主契约 200）
async fn catalog_enabled(state: &AppState) -> bool {
    state
        .site_config("public_catalog_enabled")
        .await
        .unwrap_or_else(|| "true".into())
        == "true"
}

/// GET /api/sources —— 全部拥有 published 插件的开发者源（顶层数组；空数组合法）
async fn list_sources(State(state): State<AppState>) -> ApiResult<Response> {
    if !catalog_enabled(&state).await {
        return Ok(json_response("[]".to_string()));
    }
    let rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT d.developer_id,
                COALESCE(NULLIF(d.display_name, ''), d.developer_id) AS display_name,
                (ARRAY_AGG(p.repository_url ORDER BY p.created_at))[1] AS repository_url
         FROM developers d
         JOIN plugins p ON p.developer_id = d.id AND p.status = 'published'
         WHERE d.is_banned = FALSE
         GROUP BY d.developer_id, d.display_name
         ORDER BY d.developer_id",
    )
    .fetch_all(state.pool())
    .await
    .map_err(anyhow::Error::from)?;

    let base = state.catalog_base_url().await;
    let mut sources: Vec<SourceEntry> = rows
        .into_iter()
        .filter(|(id, name, repo)| {
            validation::valid_developer_id(id)
                && !name.trim().is_empty()
                && name.encode_utf16().count() <= 200
                && validation::parse_repo_url(repo).is_ok()
        })
        .map(
            |(developer_id, developer_name, repository_url)| SourceEntry {
                developer_id: developer_id.clone(),
                developer_name,
                repository_url,
                plugins_url: format!("{base}/api/sources/{developer_id}/plugins"),
            },
        )
        .collect();
    if sources.len() as i64 > MAX_SOURCES {
        // 契约上限防御：超出时截断并告警（正常情况下发布时的配额校验会先行拦截）
        tracing::error!("sources 超过 {} 条，已截断", MAX_SOURCES);
        sources.truncate(MAX_SOURCES as usize);
    }
    for s in &sources {
        if s.developer_name.is_empty() || s.developer_name.chars().count() > 200 {
            tracing::error!("source developerName 非法：{}", s.developer_id);
        }
    }
    Ok(json_response(serde_json::to_string(&sources).unwrap()))
}

/// GET /api/sources/{developerId}/plugins —— 该开发者全部 published 插件
/// 未审核 / 已停止内容绝不出现；updates_halted 只出现冻结版本（current_submission_id 不变）
async fn source_plugins(
    State(state): State<AppState>,
    Path(developer_id): Path<String>,
) -> ApiResult<Response> {
    let empty = || json_response("[]".to_string());
    if !catalog_enabled(&state).await || !validation::valid_developer_id(&developer_id) {
        return Ok(empty());
    }
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            i64,
            Vec<String>,
            String,
            serde_json::Value,
        ),
    >(
        "SELECT p.plugin_id, s.manifest_name, s.version, d.developer_id, p.repository_url,
                s.download_url, s.sha256, s.min_host_version, s.contract_version::bigint,
                s.permissions, s.description, s.manifest_json
         FROM plugins p
         JOIN developers d ON d.id = p.developer_id
         JOIN submissions s ON s.id = p.current_submission_id
         WHERE d.developer_id = $1 AND p.status = 'published' AND d.is_banned = FALSE
         ORDER BY p.plugin_id",
    )
    .bind(&developer_id)
    .fetch_all(state.pool())
    .await
    .map_err(anyhow::Error::from)?;

    let known = crate::submission::known_permission_set(&state)
        .await
        .unwrap_or_default();

    let mut entries: Vec<CatalogEntry> = Vec::with_capacity(rows.len());
    for (id, name, version, dev_id, repo, dl, sha, mhv, cv, perms, desc, manifest) in rows {
        let network = match crate::plugin_security::network(&manifest) {
            Ok(network) => network,
            Err(error) => {
                tracing::error!("插件 {} 网络披露无效，已剔除：{}", id, error);
                continue;
            }
        };
        let e = CatalogEntry {
            id,
            name,
            version,
            developer_id: dev_id,
            repository_url: repo,
            download_url: dl,
            sha256: sha,
            min_host_version: mhv,
            contract_version: cv,
            permissions: perms,
            description: desc,
            network,
        };
        // 出索引双重校验：任一条不符不得进入索引
        match e.validate_package(&manifest, &known) {
            Ok(()) => entries.push(e),
            Err(err) => tracing::error!("插件 {} 出索引校验失败，已剔除：{}", e.id, err),
        }
    }

    // 单源 ≤2048 条防御
    if entries.len() as i64 > validation::MAX_PLUGINS_PER_SOURCE {
        tracing::error!(
            "源 {} 插件超过 {} 条，已截断",
            developer_id,
            validation::MAX_PLUGINS_PER_SOURCE
        );
        entries.truncate(validation::MAX_PLUGINS_PER_SOURCE as usize);
    }

    // 单响应 ≤4 MiB：逐条累计，超出即截断并告警
    let mut body = String::from("[");
    let mut count = 0usize;
    for e in &entries {
        let piece = if count == 0 {
            serde_json::to_string(e).unwrap()
        } else {
            format!(",{}", serde_json::to_string(e).unwrap())
        };
        if body.len() + piece.len() + 1 > MAX_RESPONSE_BYTES {
            tracing::error!("源 {} 响应超过 4 MiB，已截断", developer_id);
            break;
        }
        body.push_str(&piece);
        count += 1;
    }
    body.push(']');
    Ok(json_response(body))
}
