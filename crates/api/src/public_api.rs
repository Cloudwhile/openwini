use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::CurrentUser;
use crate::error::{ApiResult, AppError};
use crate::state::AppState;

/// 动态公开内容禁止浏览器启发式缓存：
/// 状态变更（下架/暂停/停止）后，目录与详情必须立即反映最新事实。
fn no_store<T: IntoResponse>(r: T) -> Response {
    let mut resp = r.into_response();
    resp.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    resp
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/public/site", get(site_info))
        .route("/public/catalog", get(catalog))
        .route("/public/plugins/{plugin_id}", get(plugin_detail))
        .route("/public/developers/{developer_id}", get(developer_page))
        .route("/public/permissions", get(permissions_table))
        .route("/public/stats", get(stats))
}

async fn site_info(State(state): State<AppState>) -> ApiResult<Response> {
    Ok(no_store(Json(json!({
        "siteName": state.site_config("site_name").await.unwrap_or_else(|| "WindowsIsland 插件中心".into()),
        "siteNameEn": state.site_config("site_name_en").await.unwrap_or_else(|| "WindowsIsland Plugin Hub".into()),
        "announcement": state.site_config("announcement").await.unwrap_or_default(),
        "publicCatalogEnabled": state.site_config("public_catalog_enabled").await.unwrap_or_else(|| "true".into()) == "true",
        "officialDeveloperId": crate::validation::OFFICIAL_DEVELOPER_ID,
    }))))
}

#[derive(Deserialize)]
struct CatalogQuery {
    q: Option<String>,
    developer: Option<String>,
    page: Option<i64>,
}

/// 公开目录：仅 published；未审核、已停止、已取消内容绝不出现
async fn catalog(
    State(state): State<AppState>,
    Query(q): Query<CatalogQuery>,
) -> ApiResult<Response> {
    let enabled = state
        .site_config("public_catalog_enabled")
        .await
        .unwrap_or_else(|| "true".into())
        == "true";
    if !enabled {
        return Err(AppError::Forbidden("公众目录浏览已被管理员关闭".into()));
    }
    let page_size: i64 = state
        .site_config("page_size")
        .await
        .and_then(|v| v.parse().ok())
        .unwrap_or(20)
        .clamp(1, 100);
    let page = q.page.unwrap_or(1).max(1);
    // 参数布局固定：$1 搜索串、$2 开发者过滤、$3 官方 developerId（仅列表排序用）、$4 limit、$5 offset
    let pattern = format!(
        "%{}%",
        q.q.clone()
            .unwrap_or_default()
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let dev_filter = q.developer.clone().unwrap_or_default();
    let official_id = crate::validation::OFFICIAL_DEVELOPER_ID.to_string();

    let where_sql = "FROM plugins p
         JOIN developers d ON d.id = p.developer_id
         JOIN submissions s ON s.id = p.current_submission_id
         WHERE p.status = 'published' AND d.is_banned = FALSE
           AND (p.plugin_id ILIKE $1 OR s.manifest_name ILIKE $1 OR s.description ILIKE $1)
           AND ($2 = '' OR d.developer_id = $2)";

    let total: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) {where_sql}"))
        .bind(&pattern)
        .bind(&dev_filter)
        .fetch_one(state.pool())
        .await
        .map_err(anyhow::Error::from)?;

    type ListRow = (
        String,
        String,
        String,
        String,
        String,
        Vec<String>,
        String,
        String,
        bool,
        bool,
    );
    let rows: Vec<ListRow> = sqlx::query_as(&format!(
        "SELECT p.plugin_id, s.manifest_name, s.version, d.developer_id,
                COALESCE(NULLIF(d.display_name,''), d.developer_id), s.permissions,
                left(s.description, 200), left(s.description_en, 200), p.updates_halted, (d.developer_id = $3)
         {where_sql}
         ORDER BY (d.developer_id = $3) DESC, s.manifest_name ASC
         LIMIT $4 OFFSET $5"
    ))
    .bind(&pattern)
    .bind(&dev_filter)
    .bind(&official_id)
    .bind(page_size)
    .bind((page - 1) * page_size)
    .fetch_all(state.pool())
    .await
    .map_err(anyhow::Error::from)?;

    let items: Vec<Value> = rows
        .into_iter()
        .map(
            |(id, name, version, dev, dev_name, perms, desc, desc_en, halted, official)| {
                json!({
                    "id": id, "name": name, "version": version,
                    "developerId": dev, "developerName": dev_name,
                    "permissions": perms, "description": desc, "descriptionEn": desc_en,
                    "updatesHalted": halted, "official": official,
                })
            },
        )
        .collect();
    Ok(no_store(Json(
        json!({ "items": items, "total": total, "page": page, "pageSize": page_size }),
    )))
}

/// 插件公开详情：draft/pending/rejected/cancelled 一律 404
async fn plugin_detail(
    State(state): State<AppState>,
    Path(plugin_id): Path<String>,
) -> ApiResult<Response> {
    #[derive(sqlx::FromRow)]
    struct Row {
        plugin_id: String,
        manifest_name: String,
        version: String,
        developer_id: String,
        developer_name: String,
        repository_url: String,
        homepage_url: Option<String>,
        docs_url: Option<String>,
        status: String,
        updates_halted: bool,
        updates_halted_reason: Option<String>,
        stop_reason: Option<String>,
        permissions: Vec<String>,
        description: String,
        description_en: String,
        updated_at: chrono::DateTime<chrono::Utc>,
    }
    let row = sqlx::query_as::<_, Row>(
        "SELECT p.plugin_id, s.manifest_name, s.version, d.developer_id,
                COALESCE(NULLIF(d.display_name,''), d.developer_id) AS developer_name,
                p.repository_url, p.homepage_url, p.docs_url, p.status, p.updates_halted,
                p.updates_halted_reason, p.stop_reason, s.permissions, s.description,
                s.description_en, p.updated_at
         FROM plugins p
         JOIN developers d ON d.id = p.developer_id
         JOIN submissions s ON s.id = p.current_submission_id
         WHERE p.plugin_id = $1 AND d.is_banned = FALSE AND p.current_submission_id IS NOT NULL",
    )
    .bind(&plugin_id)
    .fetch_optional(state.pool())
    .await
    .map_err(anyhow::Error::from)?
    .ok_or_else(|| AppError::NotFound("插件不存在或未公开".into()))?;

    // stopped 仍可见（展示中性下架说明）；paused 可见（暂停说明）；
    // unlisted 是开发者明确取消公开展示，与未发布状态一样一律 404
    if !crate::status::publicly_visible_detail(&row.status) {
        return Err(AppError::NotFound("插件不存在或未公开".into()));
    }

    let risk = load_permission_risk(&state, &row.permissions).await;
    let official = row.developer_id == crate::validation::OFFICIAL_DEVELOPER_ID;
    let public_status = if row.status == "published" && row.updates_halted {
        "updates_halted"
    } else {
        row.status.as_str()
    };

    Ok(no_store(Json(json!({
        "id": row.plugin_id,
        "name": row.manifest_name,
        "version": row.version,
        "developerId": row.developer_id,
        "developerName": row.developer_name,
        "official": official,
        "status": public_status,
        "description": row.description,
        "descriptionEn": row.description_en,
        "permissions": row.permissions.iter().map(|p| json!({
            "name": p, "risk": risk.get(p).cloned().unwrap_or_else(|| "unknown".into()),
        })).collect::<Vec<_>>(),
        "repositoryUrl": row.repository_url,
        "homepageUrl": row.homepage_url,
        "docsUrl": row.docs_url,
        "updatesHaltedNotice": if row.updates_halted {
            json!(row.updates_halted_reason.unwrap_or_else(|| "该插件当前处于停止更新状态，现有版本仍可安装。".into()))
        } else { Value::Null },
        "stoppedNotice": if row.status == "stopped" {
            json!(row.stop_reason.unwrap_or_else(|| "该插件已被停止提供。".into()))
        } else { Value::Null },
        "pausedNotice": if row.status == "paused" {
            json!("开发者已暂停提供该插件，恢复后将重新进入目录。")
        } else { Value::Null },
        "updatedAt": row.updated_at,
    }))))
}

async fn developer_page(
    State(state): State<AppState>,
    Path(developer_id): Path<String>,
) -> ApiResult<Response> {
    let dev: (String, String) = sqlx::query_as(
        "SELECT developer_id, COALESCE(NULLIF(display_name,''), developer_id)
         FROM developers WHERE developer_id = $1 AND is_banned = FALSE",
    )
    .bind(&developer_id)
    .fetch_optional(state.pool())
    .await
    .map_err(anyhow::Error::from)?
    .ok_or_else(|| AppError::NotFound("开发者不存在".into()))?;

    let rows: Vec<(String, String, String, Vec<String>, bool, String, String)> = sqlx::query_as(
        "SELECT p.plugin_id, s.manifest_name, s.version, s.permissions, p.updates_halted,
                left(s.description, 200), left(s.description_en, 200)
         FROM plugins p
         JOIN submissions s ON s.id = p.current_submission_id
         WHERE p.developer_id = (SELECT id FROM developers WHERE developer_id = $1)
           AND p.status = 'published'
         ORDER BY s.manifest_name",
    )
    .bind(&developer_id)
    .fetch_all(state.pool())
    .await
    .map_err(anyhow::Error::from)?;

    Ok(no_store(Json(json!({
        "developerId": dev.0,
        "developerName": dev.1,
        "official": dev.0 == crate::validation::OFFICIAL_DEVELOPER_ID,
        "plugins": rows.into_iter().map(|(id, name, version, perms, halted, desc, desc_en)| json!({
            "id": id, "name": name, "version": version, "permissions": perms, "updatesHalted": halted,
            "description": desc, "descriptionEn": desc_en,
        })).collect::<Vec<_>>(),
    }))))
}

async fn permissions_table(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT name, risk, description FROM known_permissions ORDER BY risk DESC, name",
    )
    .fetch_all(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    Ok(Json(json!({
        "permissions": rows.into_iter().map(|(name, risk, desc)| json!({
            "name": name, "risk": risk, "description": desc,
        })).collect::<Vec<_>>()
    })))
}

async fn stats(State(state): State<AppState>) -> ApiResult<Response> {
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM plugins p JOIN developers d ON d.id = p.developer_id
         WHERE p.status='published' AND d.is_banned=FALSE",
    )
    .fetch_one(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    Ok(no_store(Json(json!({ "publishedPlugins": count.0 }))))
}

pub async fn load_permission_risk(
    state: &AppState,
    perms: &[String],
) -> std::collections::HashMap<String, String> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT name, risk FROM known_permissions")
        .fetch_all(state.pool())
        .await
        .unwrap_or_default();
    let map: std::collections::HashMap<String, String> = rows.into_iter().collect();
    let _ = perms;
    map
}

// 占位：避免未使用导入告警（CurrentUser 在路由守卫版本中使用）
#[allow(dead_code)]
fn _current_user_marker(_u: &CurrentUser) {}
