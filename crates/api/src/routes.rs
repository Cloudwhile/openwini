use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;

use crate::catalog;
use crate::error::AppError;
use crate::ratelimit::{public_rate_limit, RateLimitState, RateLimiter};
use crate::state::AppState;

/// 直接在浏览器访问 API（:8080）时浏览器会请求 /favicon.ico；
/// 内嵌多尺寸 ICO（17 KiB），让接口页也显示站点图标。
async fn favicon() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "image/x-icon")],
        include_bytes!("favicon.ico.bytes").as_slice(),
    )
}

pub fn build_router(state: AppState) -> anyhow::Result<Router> {
    let web_dist_dir = state.config().web_dist_dir.clone();
    let public_limiter = RateLimiter::new(240, std::time::Duration::from_secs(60));
    let private_limiter = RateLimiter::new(120, std::time::Duration::from_secs(60));
    let trusted_proxy_ips = state.config().trusted_proxy_ips.clone();

    // 宿主目录 API（公开、只读、限流、缓存头）
    let host_api = catalog::router().layer(middleware::from_fn_with_state(
        RateLimitState {
            limiter: public_limiter.clone(),
            trusted_proxy_ips: trusted_proxy_ips.clone(),
        },
        public_rate_limit,
    ));

    // 网站公开查询 API（访问统计在限流层内侧，仅统计真实服务的请求）
    let public_api = crate::public_api::router()
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::visits::track,
        ))
        .layer(middleware::from_fn_with_state(
            RateLimitState {
                limiter: public_limiter,
                trusted_proxy_ips: trusted_proxy_ips.clone(),
            },
            public_rate_limit,
        ));

    // 登录态 API（会话 + CSRF 在提取器内校验；额外限流）
    let private_api = crate::auth::router()
        .merge(crate::developer_api::router())
        .merge(crate::admin_api::router())
        .layer(middleware::from_fn_with_state(
            RateLimitState {
                limiter: private_limiter,
                trusted_proxy_ips,
            },
            public_rate_limit,
        ));

    let app = Router::new()
        .route("/healthz", get(|| async { StatusCode::OK }))
        .route("/readyz", get(readyz))
        .route("/favicon.ico", get(favicon))
        .nest(
            "/api",
            host_api
                .merge(public_api)
                .merge(private_api)
                .fallback(fallback),
        )
        .with_state(state);

    match web_dist_dir {
        Some(directory) => Ok(app.fallback_service(crate::web::router(&directory)?)),
        None => Ok(app.fallback(fallback)),
    }
}

async fn readyz(State(state): axum::extract::State<AppState>) -> impl IntoResponse {
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(state.pool())
        .await
    {
        Ok(_) => (StatusCode::OK, "ok").into_response(),
        Err(err) => {
            tracing::warn!("readyz 数据库检查失败: {err}");
            (StatusCode::SERVICE_UNAVAILABLE, "unready").into_response()
        }
    }
}

async fn fallback() -> impl IntoResponse {
    AppError::NotFound("接口不存在".into())
}
