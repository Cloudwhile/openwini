use std::path::{Path, PathBuf};

use anyhow::Context;
use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Router;
use base64::Engine;
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Clone)]
struct WebState {
    directory: PathBuf,
    content_security_policy: HeaderValue,
}

pub fn router(directory: &Path) -> anyhow::Result<Router> {
    let index = std::fs::read_to_string(directory.join("index.html"))
        .with_context(|| format!("无法读取前端入口 {}/index.html", directory.display()))?;

    // 浏览器按 LF 计算内联脚本哈希，允许入口中恢复主题和语言的初始化脚本。
    let index = index.replace("\r\n", "\n").replace('\r', "\n");
    let mut script_sources = "'self'".to_string();
    for script in index.split("<script").skip(1) {
        if let Some((_, content)) = script.split_once('>') {
            if let Some((body, _)) = content.split_once("</script>") {
                if !body.trim().is_empty() {
                    let hash = base64::engine::general_purpose::STANDARD
                        .encode(Sha256::digest(body.as_bytes()));
                    script_sources.push_str(&format!(" 'sha256-{hash}'"));
                }
            }
        }
    }
    let policy = format!(
        "default-src 'self'; script-src {script_sources}; style-src 'self' 'unsafe-inline'; \
         img-src 'self' https: data:; font-src 'self' data:; connect-src 'self'; \
         object-src 'none'; frame-ancestors 'none'; base-uri 'self'; \
         form-action 'self' https://github.com"
    );

    Ok(Router::new().fallback(serve).with_state(WebState {
        directory: directory.to_path_buf(),
        content_security_policy: HeaderValue::from_str(&policy)?,
    }))
}

async fn serve(State(state): State<WebState>, request: Request) -> Response {
    let raw_path = request.uri().path().to_string();
    let path = match urlencoding::decode(&raw_path) {
        Ok(path) => path,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if path.split('/').any(|segment| segment.starts_with('.')) {
        return StatusCode::NOT_FOUND.into_response();
    }

    let is_asset = path == "/assets" || path.starts_with("/assets/");
    let is_page = !is_asset && Path::new(path.as_ref()).extension().is_none();
    let files = ServeDir::new(&state.directory).append_index_html_on_directories(false);
    let result = if is_page {
        files
            .fallback(ServeFile::new(state.directory.join("index.html")))
            .oneshot(request)
            .await
    } else {
        files.oneshot(request).await
    };
    let mut response = match result {
        Ok(response) => response.into_response(),
        Err(error) => match error {},
    };

    let cache = if is_asset && response.status().is_success() {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        state.content_security_policy,
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Method;
    use sqlx::postgres::PgPoolOptions;

    const INDEX: &str =
        "<!doctype html><html><script>console.log('theme')</script><div id=\"root\"></div></html>";
    const SCRIPT: &str = "console.log('app')";

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let directory =
                std::env::temp_dir().join(format!("wini-web-{}", rand::random::<u64>()));
            std::fs::create_dir_all(directory.join("assets")).unwrap();
            std::fs::write(directory.join("index.html"), INDEX).unwrap();
            std::fs::write(directory.join("assets/app-v1.js"), SCRIPT).unwrap();
            Self(directory)
        }

        fn app(&self, serve_web: bool) -> Router {
            let config = crate::config::Config {
                pg_connect: sqlx::postgres::PgConnectOptions::new(),
                bind_addr: "127.0.0.1:0".into(),
                app_env: "development".into(),
                app_encryption_key: vec![0; 32],
                github_client_id: String::new(),
                github_client_secret: String::new(),
                app_base_url: "http://localhost".into(),
                admin_github_logins: Vec::new(),
                official_github_login: String::new(),
                cookie_secure: false,
                trusted_proxy_ips: Vec::new(),
                web_dist_dir: serve_web.then(|| self.0.clone()),
            };
            let pool = PgPoolOptions::new()
                .connect_lazy("postgres://localhost/wini_web_test")
                .unwrap();
            let (mail_tx, _) = tokio::sync::mpsc::unbounded_channel();
            let state = crate::state::AppState::new(pool, config, mail_tx);
            crate::routes::build_router(state).unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    async fn request(app: Router, method: Method, uri: &str) -> Response {
        app.oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    }

    async fn text(response: Response) -> String {
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn serves_entry_and_browser_routes() {
        let fixture = Fixture::new();
        let app = fixture.app(true);
        for uri in ["/", "/index.html", "/dev/plugins/example"] {
            let response = request(app.clone(), Method::GET, uri).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-cache");
            assert!(response.headers()[header::CONTENT_TYPE]
                .to_str()
                .unwrap()
                .starts_with("text/html"));
            assert!(response.headers()[header::CONTENT_SECURITY_POLICY]
                .to_str()
                .unwrap()
                .contains("'sha256-"));
            assert_eq!(text(response).await, INDEX);
        }
    }

    #[tokio::test]
    async fn serves_versioned_assets_and_keeps_missing_files_as_404() {
        let fixture = Fixture::new();
        let app = fixture.app(true);
        let response = request(app.clone(), Method::GET, "/assets/app-v1.js").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );
        assert_eq!(text(response).await, SCRIPT);

        for uri in ["/assets/missing.js", "/assets/missing", "/missing.png"] {
            let response = request(app.clone(), Method::GET, uri).await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-cache");
            assert_ne!(text(response).await, INDEX);
        }
    }

    #[tokio::test]
    async fn keeps_api_errors_and_health_routes_separate_from_pages() {
        let fixture = Fixture::new();
        let app = fixture.app(true);
        for uri in ["/api", "/api/missing"] {
            let response = request(app.clone(), Method::GET, uri).await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
            assert!(!response.headers().contains_key(header::CACHE_CONTROL));
        }
        let response = request(app, Method::GET, "/healthz").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(text(response).await, "");
    }

    #[tokio::test]
    async fn supports_head_and_rejects_page_writes() {
        let fixture = Fixture::new();
        let app = fixture.app(true);
        let response = request(app.clone(), Method::HEAD, "/dev/plugins/example").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_LENGTH],
            INDEX.len().to_string()
        );
        assert_eq!(text(response).await, "");
        let response = request(app, Method::POST, "/dev/plugins/example").await;
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn api_only_mode_keeps_existing_fallback() {
        let fixture = Fixture::new();
        let response = request(fixture.app(false), Method::GET, "/").await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    }
}
