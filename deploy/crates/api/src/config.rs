use anyhow::{Context, Result};
use sqlx::postgres::PgConnectOptions;
use std::net::IpAddr;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    /// 数据库连接配置（由分键变量或 DATABASE_URL 构建，统一为 PgConnectOptions）
    pub pg_connect: PgConnectOptions,
    pub bind_addr: String,
    pub app_env: String,
    /// 32 字节十六进制（64 字符），用于加密 GitHub Token / SMTP 密码 / AI Key
    pub app_encryption_key: Vec<u8>,
    pub github_client_id: String,
    pub github_client_secret: String,
    /// 前端基地址（用于邮件里的详情链接、OAuth 回跳）
    pub app_base_url: String,
    /// 引导管理员用的 GitHub 登录名列表（逗号分隔，仅授予、不收回）
    pub admin_github_logins: Vec<String>,
    /// 唯一拥有官方（system / winiofficial）身份的 GitHub 登录名；登录后自动绑定官方身份
    pub official_github_login: String,
    pub cookie_secure: bool,
    /// 仅这些反向代理地址可以声明 X-Forwarded-For。
    pub trusted_proxy_ips: Vec<IpAddr>,
    /// 可选的前端生产产物目录；留空时只提供 API。
    pub web_dist_dir: Option<PathBuf>,
}

/// 数据库配置：优先使用分键变量 DB_HOST/DB_PORT/DB_USER/DB_PASSWORD/DB_NAME，
/// 否则回退到 DATABASE_URL（postgres://…）。分键方式下密码无需 URL 编码。
fn db_options_from_env() -> Result<PgConnectOptions> {
    let host = std::env::var("DB_HOST").unwrap_or_default();
    if !host.trim().is_empty() {
        let port: u16 = std::env::var("DB_PORT")
            .unwrap_or_else(|_| "5432".into())
            .trim()
            .parse()
            .context("DB_PORT 不是合法端口")?;
        let user = std::env::var("DB_USER").context("已设置 DB_HOST，则 DB_USER 必填")?;
        let password = std::env::var("DB_PASSWORD").unwrap_or_default();
        let database = std::env::var("DB_NAME").unwrap_or_default();
        let mut opts = PgConnectOptions::new()
            .host(host.trim())
            .port(port)
            .username(user.trim());
        if !password.is_empty() {
            opts = opts.password(&password);
        }
        if !database.trim().is_empty() {
            opts = opts.database(database.trim());
        }
        return Ok(opts);
    }
    let url = std::env::var("DATABASE_URL").context(
        "缺少数据库配置：请设置 DB_HOST/DB_PORT/DB_USER/DB_PASSWORD/DB_NAME 或 DATABASE_URL",
    )?;
    url.parse::<PgConnectOptions>()
        .map_err(|e| anyhow::anyhow!("DATABASE_URL 解析失败：{e}"))
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let pg_connect = db_options_from_env()?;
        let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
        let app_env = std::env::var("APP_ENV")
            .unwrap_or_else(|_| "development".into())
            .trim()
            .to_ascii_lowercase();
        if app_env != "development" && app_env != "production" {
            anyhow::bail!("APP_ENV 必须是 development 或 production");
        }
        let key_hex = std::env::var("APP_ENCRYPTION_KEY")
            .context("缺少环境变量 APP_ENCRYPTION_KEY（可用 openssl rand -hex 32 生成）")?;
        let app_encryption_key = hex::decode(key_hex.trim())
            .map_err(|_| anyhow::anyhow!("APP_ENCRYPTION_KEY 不是合法十六进制"))?;
        if app_encryption_key.len() != 32 {
            anyhow::bail!("APP_ENCRYPTION_KEY 必须是 32 字节（64 个十六进制字符）");
        }
        let admin_github_logins: Vec<String> = std::env::var("ADMIN_GITHUB_LOGINS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        let github_client_id = std::env::var("GITHUB_CLIENT_ID").unwrap_or_default();
        let github_client_secret = std::env::var("GITHUB_CLIENT_SECRET").unwrap_or_default();
        let app_base_url = std::env::var("APP_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:5173".into())
            .trim_end_matches('/')
            .to_string();
        let cookie_secure = std::env::var("COOKIE_SECURE")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true);
        let trusted_proxy_ips = std::env::var("TRUSTED_PROXY_IPS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.parse::<IpAddr>()
                    .with_context(|| format!("TRUSTED_PROXY_IPS 包含非法 IP：{s}"))
            })
            .collect::<Result<Vec<_>>>()?;
        if app_env == "production" {
            let parsed = url::Url::parse(&app_base_url).context("APP_BASE_URL 不是合法 URL")?;
            if parsed.scheme() != "https" || parsed.host_str().is_none() {
                anyhow::bail!("生产环境 APP_BASE_URL 必须是带主机名的 https URL");
            }
            if !cookie_secure {
                anyhow::bail!("生产环境 COOKIE_SECURE 必须为 true");
            }
            if admin_github_logins.is_empty() {
                anyhow::bail!("生产环境必须配置至少一个 ADMIN_GITHUB_LOGINS");
            }
            if !github_oauth_configured_values(&github_client_id, &github_client_secret) {
                anyhow::bail!("生产环境必须配置 GITHUB_CLIENT_ID 和 GITHUB_CLIENT_SECRET");
            }
        }
        Ok(Self {
            pg_connect,
            bind_addr,
            app_env,
            app_encryption_key,
            github_client_id,
            github_client_secret,
            app_base_url,
            admin_github_logins,
            official_github_login: std::env::var("OFFICIAL_GITHUB_LOGIN")
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase(),
            cookie_secure,
            trusted_proxy_ips,
            web_dist_dir: std::env::var("WEB_DIST_DIR")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(PathBuf::from),
        })
    }

    pub fn github_oauth_configured(&self) -> bool {
        !self.github_client_id.is_empty() && !self.github_client_secret.is_empty()
    }
}

fn github_oauth_configured_values(client_id: &str, client_secret: &str) -> bool {
    !client_id.trim().is_empty() && !client_secret.trim().is_empty()
}
