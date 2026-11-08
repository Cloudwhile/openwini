use axum::extract::{Path, State};
use axum::http::{header, HeaderValue};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};

use crate::error::ApiResult;
use crate::state::AppState;
use crate::validation::{self, CatalogEntry, MAX_RESPONSE_BYTES, MAX_SOURCES};
use anyhow::{ensure, Context};
use std::collections::HashSet;

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

// A successful response is authoritative: never return a partial catalog.
fn json_response<T: Serialize>(entries: &[T], maximum: i64) -> anyhow::Result<Response> {
    ensure!(
        i64::try_from(entries.len())? <= maximum,
        "catalog count exceeds contract limit"
    );
    let body = serde_json::to_vec(entries).context("serialize catalog")?;
    ensure!(
        body.len() <= MAX_RESPONSE_BYTES,
        "catalog response exceeds 4 MiB"
    );
    let mut resp = Response::new(axum::body::Body::from(body));
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    resp.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=60"),
    );
    Ok(resp)
}

async fn catalog_settings(state: &AppState) -> anyhow::Result<(bool, String)> {
    // AppState::site_config discards database errors. Catalog reads must preserve them.
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT key, value_encrypted FROM site_configs
         WHERE key IN ('public_catalog_enabled', 'catalog_base_url')",
    )
    .fetch_all(state.pool())
    .await?;
    let mut enabled = true;
    let mut base = state
        .config()
        .app_base_url
        .trim_end_matches('/')
        .to_string();
    for (key, value) in rows {
        match (key.as_str(), value.as_deref()) {
            ("public_catalog_enabled", Some("false")) => enabled = false,
            ("public_catalog_enabled", Some("true") | None) => {}
            ("public_catalog_enabled", Some(_)) => anyhow::bail!("invalid catalog enabled setting"),
            ("catalog_base_url", Some(value)) => {
                let value = value.trim().trim_end_matches('/');
                if !value.is_empty() && value != "https://plugins.windowsisland.example" {
                    base = value.to_string();
                }
            }
            _ => {}
        }
    }
    Ok((enabled, base))
}

fn sources_response(
    rows: Vec<(String, String, Vec<String>)>,
    base: &str,
) -> anyhow::Result<Response> {
    let mut sources = Vec::with_capacity(rows.len());
    for (developer_id, developer_name, repositories) in rows {
        ensure!(
            validation::valid_developer_id(&developer_id),
            "invalid source developerId"
        );
        ensure!(
            !developer_name.trim().is_empty() && developer_name.encode_utf16().count() <= 200,
            "invalid source developerName"
        );
        ensure!(repositories.len() == 1, "source repositories disagree");
        let repository_url = repositories
            .into_iter()
            .next()
            .context("missing source repository")?;
        validation::parse_repo_url(&repository_url).map_err(anyhow::Error::msg)?;
        let plugins_url = format!("{base}/api/sources/{developer_id}/plugins");
        let parsed = url::Url::parse(&plugins_url).context("invalid source pluginsUrl")?;
        ensure!(
            matches!(parsed.scheme(), "http" | "https")
                && parsed.host_str().is_some()
                && parsed.username().is_empty()
                && parsed.password().is_none()
                && parsed.query().is_none()
                && parsed.fragment().is_none(),
            "invalid source pluginsUrl"
        );
        sources.push(SourceEntry {
            developer_id,
            developer_name,
            repository_url,
            plugins_url,
        });
    }
    json_response(&sources, MAX_SOURCES)
}

/// GET /api/sources —— 全部拥有 published 插件的开发者源（顶层数组；空数组合法）
async fn list_sources(State(state): State<AppState>) -> ApiResult<Response> {
    let (enabled, base) = catalog_settings(&state).await?;
    if !enabled {
        return Ok(json_response::<SourceEntry>(&[], MAX_SOURCES)?);
    }
    let rows = sqlx::query_as::<_, (String, String, Vec<String>)>(
        "SELECT d.developer_id,
                COALESCE(NULLIF(d.display_name, ''), d.developer_id) AS display_name,
                ARRAY_AGG(DISTINCT p.repository_url) AS repositories
         FROM developers d
         JOIN plugins p ON p.developer_id = d.id AND p.status = 'published'
         WHERE d.is_banned = FALSE
         GROUP BY d.developer_id, d.display_name
         ORDER BY d.developer_id",
    )
    .fetch_all(state.pool())
    .await
    .map_err(anyhow::Error::from)?;

    Ok(sources_response(rows, &base)?)
}

/// GET /api/sources/{developerId}/plugins —— 该开发者全部 published 插件
/// 未审核 / 已停止内容绝不出现；updates_halted 只出现冻结版本（current_submission_id 不变）
async fn source_plugins(
    State(state): State<AppState>,
    Path(developer_id): Path<String>,
) -> ApiResult<Response> {
    let (enabled, _) = catalog_settings(&state).await?;
    if !enabled || !validation::valid_developer_id(&developer_id) {
        return Ok(json_response::<CatalogEntry>(
            &[],
            validation::MAX_PLUGINS_PER_SOURCE,
        )?);
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
        "SELECT p.plugin_id, COALESCE(s.manifest_name, ''), COALESCE(s.version, ''), d.developer_id, p.repository_url,
                COALESCE(s.download_url, ''), COALESCE(s.sha256, ''), COALESCE(s.min_host_version, ''), COALESCE(s.contract_version, 0)::bigint,
                COALESCE(s.permissions, ARRAY[]::text[]), COALESCE(s.description, ''), COALESCE(s.manifest_json, 'null'::jsonb)
         FROM plugins p
         JOIN developers d ON d.id = p.developer_id
         LEFT JOIN submissions s ON s.id = p.current_submission_id
         WHERE d.developer_id = $1 AND p.status = 'published' AND d.is_banned = FALSE
         ORDER BY p.plugin_id",
    )
    .bind(&developer_id)
    .fetch_all(state.pool())
    .await
    .map_err(anyhow::Error::from)?;

    let known = crate::submission::known_permission_set(&state).await?;
    let mut entries = Vec::with_capacity(rows.len());
    for (id, name, version, dev_id, repo, dl, sha, mhv, cv, perms, desc, manifest) in rows {
        let network = crate::plugin_security::network(&manifest).map_err(anyhow::Error::msg)?;
        entries.push((
            CatalogEntry {
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
            },
            manifest,
        ));
    }
    Ok(plugins_response(entries, &known)?)
}

fn plugins_response(
    entries: Vec<(CatalogEntry, serde_json::Value)>,
    known: &HashSet<String>,
) -> anyhow::Result<Response> {
    let repository = entries
        .first()
        .map(|(entry, _)| entry.repository_url.as_str());
    for (entry, manifest) in &entries {
        ensure!(
            Some(entry.repository_url.as_str()) == repository,
            "source repositories disagree"
        );
        entry
            .validate_package(manifest, known)
            .map_err(anyhow::Error::msg)?;
    }
    let entries: Vec<_> = entries.into_iter().map(|(entry, _)| entry).collect();
    json_response(&entries, validation::MAX_PLUGINS_PER_SOURCE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};
    use serde_json::{json, Value};

    fn source() -> (String, String, Vec<String>) {
        (
            "demo".into(),
            "Demo".into(),
            vec!["https://github.com/example/plugins".into()],
        )
    }
    fn package() -> (CatalogEntry, Value) {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/plugin-contract.json"))
                .unwrap();
        (
            serde_json::from_value(fixture["entry"].clone()).unwrap(),
            fixture["manifest"].clone(),
        )
    }
    fn known() -> HashSet<String> {
        validation::HOST_PERMISSIONS
            .iter()
            .map(|p| (*p).to_string())
            .collect()
    }
    fn response(result: anyhow::Result<Response>) -> Response {
        result
            .map_err(crate::error::AppError::Internal)
            .into_response()
    }
    async fn body(response: Response) -> Value {
        serde_json::from_slice(
            &to_bytes(response.into_body(), MAX_RESPONSE_BYTES)
                .await
                .unwrap(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn complete_and_empty_catalogs_are_successful_json() {
        let sources = sources_response(vec![source()], "https://catalog.example").unwrap();
        assert_eq!(sources.status(), StatusCode::OK);
        assert_eq!(
            sources.headers()[header::CACHE_CONTROL],
            "public, max-age=60"
        );
        assert_eq!(
            body(sources).await,
            json!([{
                "developerId":"demo", "developerName":"Demo",
                "repositoryUrl":"https://github.com/example/plugins",
                "pluginsUrl":"https://catalog.example/api/sources/demo/plugins"
            }])
        );
        assert_eq!(
            body(sources_response(vec![], "https://catalog.example").unwrap()).await,
            json!([])
        );
        assert_eq!(
            body(plugins_response(vec![], &known()).unwrap()).await,
            json!([])
        );
        let (entry, manifest) = package();
        let expected = serde_json::to_value(&entry).unwrap();
        assert_eq!(
            body(plugins_response(vec![(entry, manifest)], &known()).unwrap()).await,
            json!([expected])
        );
    }

    #[test]
    fn invalid_source_data_fails_the_whole_response() {
        let mut cases = vec![];
        let mut invalid = source();
        invalid.0 = "INVALID".into();
        cases.push(invalid);
        let mut invalid = source();
        invalid.1 = " ".into();
        cases.push(invalid);
        let mut invalid = source();
        invalid.1 = "😀".repeat(101);
        cases.push(invalid);
        let mut invalid = source();
        invalid.2 = vec!["https://example.com/repo".into()];
        cases.push(invalid);
        let mut invalid = source();
        invalid.2.push("https://github.com/example/other".into());
        cases.push(invalid);
        for invalid in cases {
            let result = response(sources_response(
                vec![source(), invalid],
                "https://catalog.example",
            ));
            assert_eq!(result.status(), StatusCode::INTERNAL_SERVER_ERROR);
            assert!(!result.headers().contains_key(header::CACHE_CONTROL));
        }
        let mut boundary = source();
        boundary.1 = "😀".repeat(100);
        assert!(sources_response(vec![boundary], "https://catalog.example").is_ok());
        assert!(sources_response(vec![source()], "https://catalog.example/?query=bad").is_err());
    }

    #[test]
    fn invalid_package_or_mixed_repository_fails_the_whole_response() {
        let mut invalid = package();
        invalid.0.sha256 = "bad".into();
        assert_eq!(
            response(plugins_response(vec![package(), invalid], &known())).status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        let mut other = package();
        other.0.repository_url = "https://github.com/example/other".into();
        assert!(plugins_response(vec![package(), other], &known()).is_err());
        let mut invalid = package();
        invalid.1["minimumHostVersion"] = json!("99.0.0");
        assert!(plugins_response(vec![package(), invalid], &known()).is_err());
        assert!(plugins_response(vec![package()], &HashSet::new()).is_err());
        let mut invalid = package();
        invalid.1["permissions"] = json!(["network"]);
        invalid.1["network"] = json!({});
        assert!(plugins_response(vec![package(), invalid], &known()).is_err());
    }

    #[test]
    fn both_endpoints_reject_excess_counts_without_truncation() {
        assert!(sources_response(
            (0..MAX_SOURCES).map(|_| source()).collect(),
            "https://catalog.example"
        )
        .is_ok());
        assert_eq!(
            response(sources_response(
                (0..=MAX_SOURCES).map(|_| source()).collect(),
                "https://catalog.example"
            ))
            .status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert!(plugins_response(
            (0..validation::MAX_PLUGINS_PER_SOURCE)
                .map(|_| package())
                .collect(),
            &known()
        )
        .is_ok());
        assert_eq!(
            response(plugins_response(
                (0..=validation::MAX_PLUGINS_PER_SOURCE)
                    .map(|_| package())
                    .collect(),
                &known()
            ))
            .status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn source_and_plugin_responses_reject_excess_utf8_bytes() {
        let base = format!("https://catalog.example/{}", "x".repeat(9000));
        assert_eq!(
            response(sources_response(
                (0..MAX_SOURCES).map(|_| source()).collect(),
                &base
            ))
            .status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        let packages = (0..600)
            .map(|_| {
                let (mut entry, mut manifest) = package();
                entry.description = "x".repeat(8000);
                manifest["description"] = json!(entry.description);
                (entry, manifest)
            })
            .collect();
        assert_eq!(
            response(plugins_response(packages, &known())).status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        // JSON array/string punctuation consumes four bytes; count bytes, not Unicode characters.
        let exact = "é".repeat((MAX_RESPONSE_BYTES - 4) / 2);
        assert!(json_response(&[&exact], 1).is_ok());
        let oversized = format!("{exact}x");
        assert!(json_response(&[oversized], 1).is_err());
    }
    #[tokio::test]
    async fn database_failure_returns_uncached_error_on_both_routes() {
        use crate::config::Config;
        use axum::{body::Body, http::Request};
        use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
        use tower::ServiceExt;

        // Closing a lazy pool makes failure deterministic without any database or environment access.
        let options = PgConnectOptions::new();
        let pool = PgPoolOptions::new().connect_lazy_with(options.clone());
        pool.close().await;
        let config = Config {
            pg_connect: options,
            bind_addr: "127.0.0.1:0".into(),
            app_env: "development".into(),
            app_encryption_key: vec![0; 32],
            github_client_id: String::new(),
            github_client_secret: String::new(),
            app_base_url: "http://localhost".into(),
            admin_github_logins: vec![],
            official_github_login: String::new(),
            cookie_secure: false,
            trusted_proxy_ips: vec![],
            web_dist_dir: None,
        };
        let (mail_tx, _mail_rx) = tokio::sync::mpsc::unbounded_channel();
        let app = router().with_state(AppState::new(pool, config, mail_tx));
        for path in ["/sources", "/sources/demo/plugins"] {
            let result = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(result.status(), StatusCode::INTERNAL_SERVER_ERROR);
            assert!(!result.headers().contains_key(header::CACHE_CONTROL));
        }
    }
}
