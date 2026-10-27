mod admin_api;
mod ai;
mod audit;
mod auth;
mod catalog;
mod config;
mod crypto;
mod developer_api;
mod error;
mod github;
mod mailer;
mod manifest;
#[cfg(test)]
mod platform_contract;
mod plugin_security;
mod public_api;
mod ratelimit;
mod routes;
mod state;
mod status;
mod submission;
mod validation;
mod visits;
mod web;
mod zip;

use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "wini_api=info,tower_http=info".into()),
        )
        .init();

    // 自动加载工作目录的 .env（不覆盖已存在的环境变量，显式设置优先）
    match dotenvy::dotenv() {
        Ok(path) => tracing::info!("已加载环境配置 {}", path.display()),
        Err(_) => tracing::info!("未找到 .env，使用进程环境变量"),
    }

    let cfg = config::Config::from_env()?;
    tracing::info!(environment = %cfg.app_env, "应用环境配置已校验");
    crypto::init_app_key(cfg.app_encryption_key.clone());

    // 数据库可能比本服务晚就绪：重试连接
    let mut pool = None;
    for i in 0..30 {
        match PgPoolOptions::new()
            .max_connections(16)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect_with(cfg.pg_connect.clone())
            .await
        {
            Ok(p) => {
                pool = Some(p);
                break;
            }
            Err(e) => {
                tracing::warn!("数据库尚未就绪（{}/30）：{e}", i + 1);
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }
    }
    let pool = pool.ok_or_else(|| anyhow::anyhow!("数据库连接失败"))?;

    sqlx::migrate!("../../migrations").run(&pool).await?;
    tracing::info!("数据库迁移完成");

    let (mail_tx, mail_rx) = tokio::sync::mpsc::unbounded_channel();
    let state = state::AppState::new(pool, cfg.clone(), mail_tx);
    tokio::spawn(mailer::run_worker(state.clone(), mail_rx));
    // 崩溃/重启恢复：把上一实例遗留的 queued 邮件重新驱动（跳过 5 分钟内的在途任务）
    mailer::recover_queued(&state).await;

    let app = routes::build_router(state.clone())?;
    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr)
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                anyhow::anyhow!(
                    "端口 {} 已被占用（可能是上一个 wini-api 实例仍在运行）。\
                 Windows 下可用 taskkill /IM wini-api.exe /F 结束旧实例后重试",
                    cfg.bind_addr
                )
            } else {
                anyhow::anyhow!("监听 {} 失败：{e}", cfg.bind_addr)
            }
        })?;
    tracing::info!("wini-api 监听 {}", cfg.bind_addr);
    if state.config().admin_github_logins.is_empty() {
        tracing::warn!("未配置 ADMIN_GITHUB_LOGINS：将没有任何管理员可审核申请");
    }
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("收到退出信号");
    })
    .await?;
    Ok(())
}
