use sqlx::{PgConnection, PgPool};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

pub struct Inner {
    pub pool: PgPool,
    pub config: Config,
    pub http: reqwest::Client,
    /// site_configs 内存缓存（写入时同步刷新）
    pub site_cache: RwLock<HashMap<String, String>>,
    pub mail_tx: tokio::sync::mpsc::UnboundedSender<i64>,
}

impl AppState {
    pub fn new(
        pool: PgPool,
        config: Config,
        mail_tx: tokio::sync::mpsc::UnboundedSender<i64>,
    ) -> Self {
        Self(Arc::new(Inner {
            pool,
            http: reqwest::Client::builder()
                .user_agent("wini-plugins-platform")
                .build()
                .expect("reqwest client"),
            site_cache: RwLock::new(HashMap::new()),
            mail_tx,
            config,
        }))
    }

    pub fn pool(&self) -> &PgPool {
        &self.0.pool
    }

    pub fn config(&self) -> &Config {
        &self.0.config
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.0.http
    }

    pub async fn site_config(&self, key: &str) -> Option<String> {
        if let Some(v) = self.0.site_cache.read().await.get(key) {
            return Some(v.clone());
        }
        let v: Option<(Option<String>,)> =
            sqlx::query_as("SELECT value_encrypted FROM site_configs WHERE key = $1")
                .bind(key)
                .fetch_optional(self.pool())
                .await
                .ok()
                .flatten();
        let v = v.and_then(|(raw,)| raw);
        if let Some(v) = &v {
            self.0
                .site_cache
                .write()
                .await
                .insert(key.to_string(), v.clone());
        }
        v
    }

    /// 读取配置并解密敏感项
    pub async fn site_config_plain(&self, key: &str) -> Option<String> {
        let raw = self.site_config(key).await?;
        let is_secret =
            sqlx::query_scalar::<_, bool>("SELECT is_secret FROM site_configs WHERE key = $1")
                .bind(key)
                .fetch_optional(self.pool())
                .await
                .ok()
                .flatten()
                .unwrap_or(false);
        if !is_secret {
            return Some(raw);
        }
        if raw.is_empty() {
            return Some(raw);
        }
        crate::crypto::decrypt(&self.0.config.app_encryption_key, &raw).ok()
    }

    pub async fn set_site_config_tx(
        &self,
        tx: &mut PgConnection,
        key: &str,
        value: &str,
        is_secret: bool,
        actor: &str,
    ) -> anyhow::Result<()> {
        let stored = if is_secret && !value.is_empty() {
            crate::crypto::encrypt(&self.0.config.app_encryption_key, value)?
        } else {
            value.to_string()
        };
        sqlx::query(
            "INSERT INTO site_configs (key, value_encrypted, is_secret, updated_by, updated_at)
             VALUES ($1, $2, $3, $4, now())
             ON CONFLICT (key) DO UPDATE
               SET value_encrypted = EXCLUDED.value_encrypted,
                   is_secret = EXCLUDED.is_secret,
                   updated_by = EXCLUDED.updated_by,
                   updated_at = now()",
        )
        .bind(key)
        .bind(&stored)
        .bind(is_secret)
        .bind(actor)
        .execute(&mut *tx)
        .await?;
        Ok(())
    }

    pub async fn invalidate_site_cache(&self) {
        self.0.site_cache.write().await.clear();
    }

    pub async fn catalog_base_url(&self) -> String {
        let configured = self.site_config("catalog_base_url").await;
        resolve_catalog_base_url(configured.as_deref(), &self.config().app_base_url)
    }

    pub fn enqueue_mail(&self, email_log_id: i64) {
        let _ = self.0.mail_tx.send(email_log_id);
    }
}

fn resolve_catalog_base_url(configured: Option<&str>, app_base_url: &str) -> String {
    configured
        .map(str::trim)
        .map(|value| value.trim_end_matches('/'))
        .filter(|value| !value.is_empty() && *value != "https://plugins.windowsisland.example")
        .unwrap_or(app_base_url)
        .trim_end_matches('/')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::resolve_catalog_base_url;

    #[test]
    fn missing_or_empty_catalog_base_uses_public_app_address() {
        for configured in [None, Some(""), Some("  "), Some("/")] {
            assert_eq!(
                resolve_catalog_base_url(configured, "https://wini-plugins.abszero.cc/"),
                "https://wini-plugins.abszero.cc"
            );
        }
    }

    #[test]
    fn legacy_catalog_placeholder_uses_public_app_address() {
        for configured in [
            "https://plugins.windowsisland.example",
            " https://plugins.windowsisland.example/ ",
        ] {
            assert_eq!(
                resolve_catalog_base_url(Some(configured), "https://wini-plugins.abszero.cc"),
                "https://wini-plugins.abszero.cc"
            );
        }
    }

    #[test]
    fn explicit_catalog_base_is_preserved() {
        for configured in ["https://catalog.example/custom/", "http://localhost:8080/"] {
            assert_eq!(
                resolve_catalog_base_url(Some(configured), "https://wini-plugins.abszero.cc"),
                configured.trim_end_matches('/')
            );
        }
    }
}
