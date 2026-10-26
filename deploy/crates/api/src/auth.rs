use axum::extract::{ConnectInfo, FromRequestParts, Query, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use sqlx::FromRow;

use crate::crypto;
use crate::error::{ApiResult, AppError};
use crate::mailer;
use crate::state::AppState;

const SESSION_COOKIE: &str = "wini_session";
const OAUTH_STATE_COOKIE: &str = "wini_oauth_state";
const SESSION_TTL_DAYS: i64 = 30;

#[derive(Debug, FromRow)]
pub struct DeveloperRow {
    pub id: i64,
    pub github_user_id: Option<i64>,
    pub github_login: String,
    pub developer_id: String,
    pub display_name: String,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub is_admin: bool,
    pub is_banned: bool,
    pub is_system: bool,
    pub github_token_encrypted: Option<String>,
    pub preferred_language: String,
}

pub struct CurrentUser {
    pub csrf: String,
    pub dev: DeveloperRow,
}

impl CurrentUser {
    pub fn github_token(&self) -> ApiResult<String> {
        match &self.dev.github_token_encrypted {
            Some(enc) => crypto::decrypt(&crypto::current_key(), enc)
                .map_err(|_| AppError::Conflict("GitHub 授权数据损坏，请重新授权".into())),
            None => Err(AppError::Forbidden(
                "尚未连接 GitHub 授权，无法执行该操作。请到「我的插件」页面重新连接 GitHub。"
                    .into(),
            )),
        }
    }
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    for v in headers.get_all(header::COOKIE) {
        let s = v.to_str().ok()?;
        for pair in s.split(';') {
            let pair = pair.trim();
            if let Some((k, v)) = pair.split_once('=') {
                if k == name {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

fn set_cookie(name: &str, value: &str, max_age: i64, secure: bool) -> String {
    let mut c = format!("{name}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}");
    if secure {
        c.push_str("; Secure");
    }
    c
}

// —— 鉴权提取器：会话 + CSRF（非 GET 请求校验 X-CSRF-Token）——
async fn try_current_user(
    parts: &mut axum::http::request::Parts,
    state: &AppState,
) -> Result<Option<CurrentUser>, AppError> {
    let Some(token) = cookie_value(&parts.headers, SESSION_COOKIE) else {
        return Ok(None);
    };
    let hash = crypto::token_hash(&token);
    let Some((dev_id, csrf)) = sqlx::query_as::<_, (i64, String)>(
        "SELECT s.developer_id, s.csrf_token FROM sessions s
         JOIN developers d ON d.id = s.developer_id
         WHERE s.token_hash = $1 AND s.expires_at > now()",
    )
    .bind(&hash)
    .fetch_optional(state.pool())
    .await
    .map_err(anyhow::Error::from)?
    else {
        return Ok(None);
    };
    if parts.method != Method::GET && parts.method != Method::HEAD {
        let provided = parts
            .headers
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if provided != csrf {
            return Err(AppError::Forbidden("CSRF 校验失败".into()));
        }
    }
    let dev = sqlx::query_as::<_, DeveloperRow>("SELECT * FROM developers WHERE id = $1")
        .bind(dev_id)
        .fetch_one(state.pool())
        .await
        .map_err(anyhow::Error::from)?;
    Ok(Some(CurrentUser { csrf, dev }))
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        try_current_user(parts, state)
            .await?
            .ok_or_else(|| AppError::Unauthorized("未登录或会话已过期，请重新登录".into()))
    }
}

/// 使 Option<CurrentUser> 可作为提取器（未登录时得到 None）
impl axum::extract::OptionalFromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Option<Self>, Self::Rejection> {
        try_current_user(parts, state).await
    }
}

pub struct AdminUser(pub CurrentUser);

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = CurrentUser::from_request_parts(parts, state).await?;
        if !user.dev.is_admin {
            return Err(AppError::Forbidden("需要管理员权限".into()));
        }
        // 被封禁的管理员立即失去全部管理端操作能力（会话可以保留，仅可访问公开内容）
        if user.dev.is_banned {
            return Err(AppError::Forbidden(
                "该管理员账号已被封禁，无法执行管理操作".into(),
            ));
        }
        Ok(AdminUser(user))
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/github/start", get(oauth_start))
        .route("/auth/github/callback", get(oauth_callback))
        .route("/auth/logout", post(logout_with_headers))
        .route("/auth/me", get(me))
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    #[allow(dead_code)]
    error_description: Option<String>,
}

async fn oauth_start(State(state): State<AppState>) -> ApiResult<Response> {
    let cfg = state.config();
    if !cfg.github_oauth_configured() {
        return Err(AppError::BadRequest(
            "服务端未配置 GITHUB_CLIENT_ID / GITHUB_CLIENT_SECRET".into(),
        ));
    }
    let oauth_state = crypto::random_hex(16);
    let redirect_uri = format!("{}/api/auth/github/callback", cfg.app_base_url);
    let url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope={}&state={}",
        cfg.github_client_id,
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(crate::github::OAUTH_SCOPES),
        oauth_state
    );
    let mut resp = Redirect::to(&url).into_response();
    resp.headers_mut().append(
        header::SET_COOKIE,
        set_cookie(OAUTH_STATE_COOKIE, &oauth_state, 600, cfg.cookie_secure)
            .parse()
            .unwrap(),
    );
    Ok(resp)
}

async fn oauth_callback(
    State(state): State<AppState>,
    Query(q): Query<CallbackQuery>,
    headers: HeaderMap,
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
) -> ApiResult<Response> {
    let cfg = state.config();
    let back_to_login = |msg: &str| -> Response {
        Redirect::to(&format!(
            "{}/login?error={}",
            cfg.app_base_url,
            urlencoding::encode(msg)
        ))
        .into_response()
    };
    if let Some(err) = q.error {
        return Ok(back_to_login(&format!("GitHub 授权失败：{err}")));
    }
    let code = q
        .code
        .ok_or_else(|| AppError::BadRequest("缺少 code".into()))?;
    let expected = cookie_value(&headers, OAUTH_STATE_COOKIE);
    let provided = q.state.unwrap_or_default();
    if expected.is_none() || provided.is_empty() || provided != expected.unwrap_or_default() {
        return Ok(back_to_login(
            "登录状态校验失败（state 不匹配），请重新登录",
        ));
    }
    let token = crate::github::exchange_code(&state, &code)
        .await
        .map_err(|e| AppError::BadRequest(format!("GitHub OAuth 令牌交换失败：{e}")))?;
    let user = crate::github::get_user(&state, &token)
        .await
        .map_err(|e| AppError::BadRequest(format!("读取 GitHub 用户信息失败：{e}")))?;
    let email = crate::github::get_primary_email(&state, &token).await;
    let token_encrypted =
        crypto::encrypt(&cfg.app_encryption_key, &token).map_err(anyhow::Error::from)?;
    let login_ip = request_login_ip(&headers, peer.ip(), &cfg.trusted_proxy_ips);
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .chars()
        .take(512)
        .collect::<String>();

    let is_bootstrap_admin = cfg
        .admin_github_logins
        .contains(&user.login.to_ascii_lowercase());

    // 唯一官方身份账号：登录即绑定 winiofficial（system）身份并以其建立会话
    let is_official = !cfg.official_github_login.is_empty()
        && user.login.to_ascii_lowercase() == cfg.official_github_login;
    if is_official {
        let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
        // 若该 GitHub 账号此前以普通开发者身份登录过，先解绑旧行（官方身份唯一）
        sqlx::query(
            "UPDATE developers SET github_user_id=NULL, github_token_encrypted=NULL, updated_at=now()
             WHERE github_user_id=$1 AND NOT is_system",
        )
        .bind(user.id)
        .execute(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
        // 绑定到 winiofficial 系统行：写入 GitHub 标识与 Token（官方直发需要），授予管理员
        let updated = sqlx::query(
            "UPDATE developers SET github_user_id=$1, github_login=$2, email=$3, avatar_url=$4,
                github_token_encrypted=$5, is_admin=TRUE, is_banned=FALSE, updated_at=now()
             WHERE developer_id=$6 AND is_system",
        )
        .bind(user.id)
        .bind(&user.login)
        .bind(&email)
        .bind(&user.avatar_url)
        .bind(&token_encrypted)
        .bind(crate::validation::OFFICIAL_DEVELOPER_ID)
        .execute(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?
        .rows_affected();
        if updated == 1 {
            crate::audit::write(
                &mut tx,
                crate::validation::OFFICIAL_DEVELOPER_ID,
                "system",
                "official_identity_bound",
                None,
                None,
                None,
                None,
                Some(&format!(
                    "GitHub 账号 {} 绑定直发账号（OFFICIAL_GITHUB_LOGIN）",
                    user.login
                )),
            )
            .await?;
        }
        tx.commit().await.map_err(anyhow::Error::from)?;
        let row: (i64,) = sqlx::query_as("SELECT id FROM developers WHERE developer_id=$1")
            .bind(crate::validation::OFFICIAL_DEVELOPER_ID)
            .fetch_one(state.pool())
            .await
            .map_err(anyhow::Error::from)?;
        let preferred_language: String =
            sqlx::query_scalar("SELECT preferred_language FROM developers WHERE id=$1")
                .bind(row.0)
                .fetch_one(state.pool())
                .await
                .map_err(anyhow::Error::from)?;
        let unusual = is_unusual_login(&state, row.0, login_ip.as_deref(), &user_agent).await?;
        let event_id = record_login_event(
            &state,
            row.0,
            login_ip.as_deref(),
            &user_agent,
            &preferred_language,
            false,
            unusual,
        )
        .await?;
        let session = create_session(state.pool(), row.0, login_ip.as_deref(), &user_agent).await?;
        if unusual {
            mailer::notify_unusual_login(
                &state,
                row.0,
                &event_id,
                login_ip.as_deref().unwrap_or("unknown"),
                &user_agent,
                Utc::now(),
            )
            .await;
        }
        return Ok(finish_login_redirect(&state, &session));
    }

    // 已有账号：刷新资料与 Token，引导管理员标记，建立会话
    let existing: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM developers WHERE github_user_id = $1")
            .bind(user.id)
            .fetch_optional(state.pool())
            .await
            .map_err(anyhow::Error::from)?;

    let mut resp = if let Some((dev_row_id,)) = existing {
        sqlx::query(
            "UPDATE developers SET github_login=$1, avatar_url=$2, email=$3,
                github_token_encrypted=$4,
                is_admin = CASE WHEN $5 THEN TRUE ELSE is_admin END,
                updated_at=now()
             WHERE id=$6 AND NOT is_system",
        )
        .bind(&user.login)
        .bind(&user.avatar_url)
        .bind(&email)
        .bind(&token_encrypted)
        .bind(is_bootstrap_admin)
        .bind(dev_row_id)
        .execute(state.pool())
        .await
        .map_err(anyhow::Error::from)?;
        let preferred_language: String =
            sqlx::query_scalar("SELECT preferred_language FROM developers WHERE id=$1")
                .bind(dev_row_id)
                .fetch_one(state.pool())
                .await
                .map_err(anyhow::Error::from)?;
        let unusual =
            is_unusual_login(&state, dev_row_id, login_ip.as_deref(), &user_agent).await?;
        let event_id = record_login_event(
            &state,
            dev_row_id,
            login_ip.as_deref(),
            &user_agent,
            &preferred_language,
            false,
            unusual,
        )
        .await?;
        let session =
            create_session(state.pool(), dev_row_id, login_ip.as_deref(), &user_agent).await?;
        if unusual {
            mailer::notify_unusual_login(
                &state,
                dev_row_id,
                &event_id,
                login_ip.as_deref().unwrap_or("unknown"),
                &user_agent,
                Utc::now(),
            )
            .await;
        }
        finish_login_redirect(&state, &session)
    } else {
        // 新用户：直接以 GitHub 用户名派生 developerId 自动注册（无需用户填写）
        let display_name = user.name.clone().unwrap_or_else(|| user.login.clone());
        let (dev_row_id, developer_id) = auto_register(
            &state,
            user.id,
            &user.login,
            &display_name,
            email.as_deref(),
            user.avatar_url.as_deref(),
            &token_encrypted,
            is_bootstrap_admin,
        )
        .await?;
        let preferred_language = infer_language(&headers);
        sqlx::query("UPDATE developers SET preferred_language=$1 WHERE id=$2")
            .bind(&preferred_language)
            .bind(dev_row_id)
            .execute(state.pool())
            .await
            .map_err(anyhow::Error::from)?;
        let event_id = record_login_event(
            &state,
            dev_row_id,
            login_ip.as_deref(),
            &user_agent,
            &preferred_language,
            true,
            false,
        )
        .await?;
        let session =
            create_session(state.pool(), dev_row_id, login_ip.as_deref(), &user_agent).await?;
        mailer::notify_welcome(&state, dev_row_id, &event_id).await;
        let _ = developer_id;
        finish_login_redirect(&state, &session)
    };
    resp.headers_mut().append(
        header::SET_COOKIE,
        set_cookie(OAUTH_STATE_COOKIE, "", 0, cfg.cookie_secure)
            .parse()
            .unwrap(),
    );
    Ok(resp)
}

/// GitHub 登录名 → 合法 developerId 候选：小写化、非 [a-z0-9_] 转下划线、
/// 数字/下划线开头补 u 前缀、超长截断到 64
pub fn sanitize_developer_candidate(login: &str) -> String {
    let mut s: String = login
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() {
        s = "user".to_string();
    }
    if !s.starts_with(|c: char| c.is_ascii_lowercase()) {
        s = format!("u{s}");
    }
    if s.len() > 64 {
        s.truncate(64);
    }
    s
}

/// 自动注册：生成候选 id，重名时依次尝试 _1/_2/…（ON CONFLICT 保证并发安全）
async fn auto_register(
    state: &AppState,
    github_user_id: i64,
    github_login: &str,
    display_name: &str,
    email: Option<&str>,
    avatar_url: Option<&str>,
    token_encrypted: &str,
    is_admin: bool,
) -> ApiResult<(i64, String)> {
    let base = sanitize_developer_candidate(github_login);
    let mut last_err = AppError::Conflict("无法生成唯一的 developerId，请稍后重试".into());
    for suffix in std::iter::once(None).chain((1..=99).map(Some)) {
        let candidate = match &suffix {
            None => base.clone(),
            Some(n) => {
                let tail = format!("_{n}");
                let cut = 64usize.saturating_sub(tail.len());
                let head: String = base.chars().take(cut.min(base.chars().count())).collect();
                format!("{head}{tail}")
            }
        };
        if candidate == crate::validation::OFFICIAL_DEVELOPER_ID {
            continue;
        }
        let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
        let row: Option<(i64,)> = sqlx::query_as(
            "INSERT INTO developers
                (github_user_id, github_login, developer_id, display_name, email, avatar_url,
                 is_admin, github_token_encrypted)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
             ON CONFLICT (developer_id) DO NOTHING
             RETURNING id",
        )
        .bind(github_user_id)
        .bind(github_login)
        .bind(&candidate)
        .bind(display_name)
        .bind(email)
        .bind(avatar_url)
        .bind(is_admin)
        .bind(token_encrypted)
        .fetch_optional(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
        if let Some((row_id,)) = row {
            crate::audit::write(
                &mut tx,
                &candidate,
                "developer",
                "register",
                None,
                None,
                None,
                None,
                Some(&format!(
                    "GitHub 首次登录自动创建（源自用户名 {github_login}）"
                )),
            )
            .await?;
            tx.commit().await.map_err(anyhow::Error::from)?;
            return Ok((row_id, candidate));
        }
        tx.rollback().await.map_err(anyhow::Error::from)?;
        last_err = AppError::Conflict(format!("developerId 候选 {candidate} 被占用"));
    }
    Err(last_err)
}

fn finish_login_redirect(state: &AppState, session: &str) -> Response {
    let mut r = Redirect::to(&format!("{}/me", state.config().app_base_url)).into_response();
    r.headers_mut().append(
        header::SET_COOKIE,
        set_cookie(
            SESSION_COOKIE,
            session,
            SESSION_TTL_DAYS * 86400,
            state.config().cookie_secure,
        )
        .parse()
        .unwrap(),
    );
    r
}

async fn create_session(
    pool: &sqlx::PgPool,
    dev_row_id: i64,
    login_ip: Option<&str>,
    user_agent: &str,
) -> ApiResult<String> {
    let token = crypto::random_hex(32);
    let csrf = crypto::random_hex(32);
    sqlx::query(
        "INSERT INTO sessions
            (token_hash, developer_id, csrf_token, expires_at, login_ip, login_user_agent, login_at)
         VALUES ($1,$2,$3, now() + make_interval(days => $4), $5::inet, $6, now())",
    )
    .bind(crypto::token_hash(&token))
    .bind(dev_row_id)
    .bind(&csrf)
    // make_interval 的 days 参数是 integer，按 i64 绑定会报函数不存在
    .bind(SESSION_TTL_DAYS as i32)
    .bind(login_ip)
    .bind(user_agent)
    .execute(pool)
    .await
    .map_err(anyhow::Error::from)?;
    Ok(token)
}

fn infer_language(headers: &HeaderMap) -> String {
    let raw = headers
        .get("accept-language")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if raw.starts_with("zh") {
        "zh".into()
    } else {
        "en".into()
    }
}

fn request_login_ip(
    headers: &HeaderMap,
    peer: std::net::IpAddr,
    trusted_proxy_ips: &[std::net::IpAddr],
) -> Option<String> {
    if trusted_proxy_ips.contains(&peer) {
        for name in ["x-real-ip", "x-forwarded-for"] {
            if let Some(value) = headers.get(name).and_then(|v| v.to_str().ok()) {
                if let Some(first) = value.split(',').next().map(str::trim) {
                    if first.parse::<std::net::IpAddr>().is_ok() {
                        return Some(first.to_string());
                    }
                }
            }
        }
    }
    Some(peer.to_string())
}

async fn is_unusual_login(
    state: &AppState,
    developer_id: i64,
    login_ip: Option<&str>,
    user_agent: &str,
) -> ApiResult<bool> {
    let previous: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT login_ip::text, login_user_agent
         FROM sessions
         WHERE developer_id=$1 AND login_ip IS NOT NULL AND login_at IS NOT NULL
         ORDER BY login_at DESC
         LIMIT 1",
    )
    .bind(developer_id)
    .fetch_optional(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    let Some((previous_ip, previous_ua)) = previous else {
        return Ok(false);
    };
    Ok(previous_ip.as_deref() != login_ip || previous_ua.as_deref().unwrap_or("") != user_agent)
}

async fn record_login_event(
    state: &AppState,
    developer_id: i64,
    login_ip: Option<&str>,
    user_agent: &str,
    language: &str,
    is_welcome: bool,
    is_unusual: bool,
) -> ApiResult<String> {
    let event_id = format!("evt_{}", crypto::random_hex(12));
    sqlx::query(
        "INSERT INTO login_events
            (event_id, developer_id, login_ip, user_agent, language, is_welcome, is_unusual)
         VALUES ($1, $2, $3::inet, $4, $5, $6, $7)",
    )
    .bind(&event_id)
    .bind(developer_id)
    .bind(login_ip)
    .bind(user_agent)
    .bind(language)
    .bind(is_welcome)
    .bind(is_unusual)
    .execute(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    Ok(event_id)
}

pub async fn logout_with_headers(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let _ = &user;
    if let Some(token) = cookie_value(&headers, SESSION_COOKIE) {
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
            .bind(crypto::token_hash(&token))
            .execute(state.pool())
            .await
            .map_err(anyhow::Error::from)?;
    }
    let mut r = StatusCode::NO_CONTENT.into_response();
    r.headers_mut().append(
        header::SET_COOKIE,
        set_cookie(SESSION_COOKIE, "", 0, state.config().cookie_secure)
            .parse()
            .unwrap(),
    );
    Ok(r)
}

async fn me(user: Option<CurrentUser>) -> ApiResult<Json<serde_json::Value>> {
    match user {
        None => Ok(Json(json!({ "authenticated": false }))),
        Some(u) => {
            let github_connected = u.dev.github_token_encrypted.is_some();
            Ok(Json(json!({
                "authenticated": true,
                "csrfToken": u.csrf,
                "developer": {
                    "developerId": u.dev.developer_id,
                    "displayName": u.dev.display_name,
                    "githubLogin": u.dev.github_login,
                    "avatarUrl": u.dev.avatar_url,
                    "email": u.dev.email,
                    "isAdmin": u.dev.is_admin,
                    "isBanned": u.dev.is_banned,
                    "githubConnected": github_connected,
                    "preferredLanguage": u.dev.preferred_language,
                }
            })))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_developer_candidate;

    #[test]
    fn plain_login_unchanged() {
        assert_eq!(sanitize_developer_candidate("cloudwhile"), "cloudwhile");
    }

    #[test]
    fn uppercase_and_hyphen_normalized() {
        assert_eq!(sanitize_developer_candidate("Some-User"), "some_user");
        assert_eq!(sanitize_developer_candidate("Alice"), "alice");
    }

    #[test]
    fn digit_or_underscore_start_gets_u_prefix() {
        assert_eq!(sanitize_developer_candidate("1abc"), "u1abc");
        assert_eq!(sanitize_developer_candidate("-abc"), "u_abc");
    }

    #[test]
    fn truncated_to_64_and_still_letter_first() {
        let long = format!("a{}", "b".repeat(100));
        let out = sanitize_developer_candidate(&long);
        assert_eq!(out.len(), 64);
        assert!(out.starts_with('a'));
        assert!(crate::validation::valid_developer_id(&out));
    }

    #[test]
    fn every_output_is_valid() {
        for input in [
            "cloudwhile",
            "1",
            "-",
            "A-B_c",
            "Zz9",
            "x".repeat(80).as_str(),
        ] {
            assert!(
                crate::validation::valid_developer_id(&sanitize_developer_candidate(input)),
                "failed for {input}"
            );
        }
    }
}
