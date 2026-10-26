use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use axum::extract::{ConnectInfo, State};
use axum::response::{IntoResponse, Response};

use crate::error::AppError;

/// 简单滑动窗口限流器（单实例内存版）
#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<HashMap<IpAddr, Vec<std::time::Instant>>>>,
    limit: usize,
    window: std::time::Duration,
}

#[derive(Clone)]
pub struct RateLimitState {
    pub limiter: RateLimiter,
    pub trusted_proxy_ips: Vec<IpAddr>,
}

impl RateLimiter {
    pub fn new(limit: usize, window: std::time::Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            limit,
            window,
        }
    }

    pub fn check(&self, ip: IpAddr) -> bool {
        let now = std::time::Instant::now();
        let mut map = self.inner.lock().unwrap();
        if map.len() > 100_000 {
            map.retain(|_, timestamps| {
                timestamps
                    .iter()
                    .any(|t| now.duration_since(*t) < self.window)
            });
        }
        let entry = map.entry(ip).or_default();
        entry.retain(|t| now.duration_since(*t) < self.window);
        if entry.len() >= self.limit {
            false
        } else {
            entry.push(now);
            true
        }
    }
}

/// 仅在直连对端属于受信任反向代理时采信 X-Forwarded-For。
pub fn client_ip(req: &axum::extract::Request, peer: IpAddr, trusted: &[IpAddr]) -> IpAddr {
    if trusted.contains(&peer) {
        if let Some(value) = req.headers().get("x-forwarded-for") {
            if let Ok(raw) = value.to_str() {
                if let Some(first) = raw.split(',').next().map(str::trim) {
                    if let Ok(forwarded) = first.parse::<IpAddr>() {
                        return forwarded;
                    }
                }
            }
        }
    }
    peer
}

pub async fn public_rate_limit(
    State(rate_state): State<RateLimitState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let peer = req
        .extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|addr| addr.0.ip())
        .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
    let ip = client_ip(&req, peer, &rate_state.trusted_proxy_ips);
    if !rate_state.limiter.check(ip) {
        return AppError::RateLimited.into_response();
    }
    next.run(req).await
}
