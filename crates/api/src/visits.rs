//! 访问统计：公开页面访客的 PV/UV 采集与聚合。
//!
//! 计数点：SPA 各公开页面加载时必然请求的公开 API（site / catalog / plugins/:id /
//! developers/:id）。指纹 = sha256(APP_ENCRYPTION_KEY || 日切 || ip || user-agent)，
//! 按日轮转：同一天同一访客同一指纹，跨日不可关联；**不落原始 IP**。
//! 采集为异步旁路（tokio::spawn），失败只记日志，不影响请求本身。

use axum::extract::{ConnectInfo, Request, State};
use axum::middleware::Next;
use axum::response::Response;
use sha2::{Digest, Sha256};

use crate::error::ApiResult;
use crate::ratelimit::client_ip as trusted_client_ip;
use crate::state::AppState;

/// 公开 API 路径 → 计数类别（不在列表中的路径不计，如 permissions/stats）。
/// 注意：axum nest 会先剥离 /api 前缀再进入本中间件，因此两种形态都要兼容。
fn classify(path: &str) -> Option<(&'static str, Option<String>)> {
    let path = path.strip_prefix("/api").unwrap_or(path);
    if path == "/public/site" || path == "/public/catalog" {
        return Some(("site", None));
    }
    if let Some(id) = path.strip_prefix("/public/plugins/") {
        if crate::validation::valid_plugin_id(id) {
            return Some(("plugin", Some(id.to_string())));
        }
        return None;
    }
    if let Some(id) = path.strip_prefix("/public/developers/") {
        if crate::validation::valid_developer_id(id) {
            return Some(("developer", Some(id.to_string())));
        }
        return None;
    }
    None
}

/// 采集中间件：挂在公开查询路由上（限流层内侧，仅统计真实服务的请求）
pub async fn track(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if let Some((kind, target)) = classify(req.uri().path()) {
        let peer = req
            .extensions()
            .get::<ConnectInfo<std::net::SocketAddr>>()
            .map(|addr| addr.0.ip())
            .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
        let ip = trusted_client_ip(&req, peer, &state.config().trusted_proxy_ips).to_string();
        let ua = req
            .headers()
            .get(axum::http::header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .chars()
            .take(200)
            .collect::<String>();
        let day = chrono::Utc::now().date_naive();
        let key = state.config().app_encryption_key.clone();
        let pool = state.pool().clone();
        tokio::spawn(async move {
            let mut hasher = Sha256::new();
            hasher.update(&key);
            hasher.update(day.to_string().as_bytes());
            hasher.update(ip.as_bytes());
            hasher.update(ua.as_bytes());
            let hash = format!("{:x}", hasher.finalize());
            let kind_col = match target {
                Some(id) => format!("{kind}:{id}"),
                None => kind.to_string(),
            };
            let result = sqlx::query(
                "INSERT INTO visit_stats (day, visitor_hash, kind, hits) VALUES ($1,$2,$3,1)
                 ON CONFLICT (day, visitor_hash, kind) DO UPDATE SET hits = visit_stats.hits + 1, updated_at = now()",
            )
            .bind(day)
            .bind(&hash)
            .bind(&kind_col)
            .execute(&pool)
            .await;
            if let Err(e) = result {
                tracing::warn!("访问统计写入失败: {e}");
            }
        });
    }
    next.run(req).await
}

/// 近 N 天 PV/UV 日序列；kinds=None 表示全站，Some(列表) 限定 kind 集合
async fn series(
    state: &AppState,
    days: i64,
    kinds: Option<Vec<String>>,
) -> ApiResult<Vec<(chrono::NaiveDate, i64, i64)>> {
    let rows: Vec<(chrono::NaiveDate, i64, i64)> = sqlx::query_as(
        "SELECT day, COALESCE(SUM(hits),0)::BIGINT, COUNT(DISTINCT visitor_hash)::BIGINT
         FROM visit_stats
         WHERE day > CURRENT_DATE - ($1::int) AND ($2::text[] IS NULL OR kind = ANY($2))
         GROUP BY day ORDER BY day",
    )
    .bind(days.clamp(1, 90) as i32)
    .bind(kinds)
    .fetch_all(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    Ok(rows)
}

async fn developer_plugin_kinds(state: &AppState, developer_row_id: i64) -> ApiResult<Vec<String>> {
    let kinds: Vec<String> =
        sqlx::query_scalar("SELECT 'plugin:' || plugin_id FROM plugins WHERE developer_id = $1")
            .bind(developer_row_id)
            .fetch_all(state.pool())
            .await
            .map_err(anyhow::Error::from)?;
    Ok(kinds)
}

/// 站点级序列（管理员概览）
pub async fn site_series(
    state: &AppState,
    days: i64,
) -> ApiResult<Vec<(chrono::NaiveDate, i64, i64)>> {
    series(state, days, None).await
}

/// 某开发者名下全部插件详情页的序列（开发者概览）
pub async fn developer_series(
    state: &AppState,
    days: i64,
    developer_row_id: i64,
) -> ApiResult<Vec<(chrono::NaiveDate, i64, i64)>> {
    let kinds = developer_plugin_kinds(state, developer_row_id).await?;
    if kinds.is_empty() {
        return Ok(Vec::new());
    }
    series(state, days, Some(kinds)).await
}

/// 窗口内独立访客总量（跨日去重；日序列的 UV 直接相加会重复计数）
async fn window_uv(state: &AppState, days: i64, kinds: Option<Vec<String>>) -> ApiResult<i64> {
    let (uv,): (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT visitor_hash) FROM visit_stats
         WHERE day > CURRENT_DATE - ($1::int) AND ($2::text[] IS NULL OR kind = ANY($2))",
    )
    .bind(days.clamp(1, 90) as i32)
    .bind(kinds)
    .fetch_one(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    Ok(uv)
}

pub async fn site_window_uv(state: &AppState, days: i64) -> ApiResult<i64> {
    window_uv(state, days, None).await
}

pub async fn developer_window_uv(
    state: &AppState,
    days: i64,
    developer_row_id: i64,
) -> ApiResult<i64> {
    let kinds = developer_plugin_kinds(state, developer_row_id).await?;
    if kinds.is_empty() {
        return Ok(0);
    }
    window_uv(state, days, Some(kinds)).await
}
