use anyhow::{Context as _, Result};
use lettre::message::{Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::state::AppState;

// —— 模板变量：插件名、版本、开发者、原因、详情链接（绝不包含任何密钥或 token）——

#[derive(Clone)]
pub struct MailVars {
    pub plugin_name: String,
    pub plugin_id: String,
    pub version: String,
    pub developer: String,
    pub reason: Option<String>,
    pub link: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Locale {
    Zh,
    En,
}

impl Locale {
    fn from_preference(value: &str) -> Self {
        if value.trim().eq_ignore_ascii_case("zh")
            || value.trim().to_ascii_lowercase().starts_with("zh-")
        {
            Self::Zh
        } else {
            Self::En
        }
    }
}

fn new_event_id() -> String {
    format!("evt_{}", crate::crypto::random_hex(16))
}

impl MailVars {
    pub fn link_for(state: &AppState, plugin_id: &str) -> String {
        format!("{}/plugins/{}", state.config().app_base_url, plugin_id)
    }
    pub fn admin_queue_link(state: &AppState) -> String {
        format!("{}/admin/review", state.config().app_base_url)
    }
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn html_unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&amp;", "&")
}

fn render(
    template: &str,
    v: &MailVars,
    locale: Locale,
    event_id: &str,
) -> Result<(String, String, String)> {
    let name = html_escape(&v.plugin_name);
    let ver = html_escape(&v.version);
    let pid = html_escape(&v.plugin_id);
    let dev = html_escape(&v.developer);
    let reason = html_escape(v.reason.as_deref().unwrap_or(if locale == Locale::Zh {
        "未提供"
    } else {
        "Not provided"
    }));
    let link = html_escape(&v.link);
    let event_id = html_escape(event_id);

    let (subject, title, intro, detail, action) = match (template, locale) {
        ("submission_receipt", Locale::Zh) => (format!("已收到插件申请：{} {}", name, ver), "已收到插件申请", format!("你好，{}：", dev), format!("我们已收到你提交的插件申请「{}」（{}，版本 {}）。申请已进入人工审核队列。人工审核仅用于减少接口滥用，不代表插件安全或可靠。", name, pid, ver), "查看申请详情"),
        ("submission_receipt", Locale::En) => (format!("Plugin submission received: {} {}", name, ver), "Plugin submission received", format!("Hello, {}:", dev), format!("We received your submission for “{}” ({}; version {}). It is now in the review queue. Human review only helps reduce interface abuse and does not guarantee plugin safety or reliability.", name, pid, ver), "View submission"),
        ("review_approved", Locale::Zh) => (format!("申请已通过：{} {}", name, ver), "申请已通过", format!("你好，{}：", dev), format!("你的插件「{}」（{}，版本 {}）已通过审核并进入公开目录。审核不代表插件安全或可靠，请在下载和使用前自行核对来源、权限与代码。", name, pid, ver), "查看插件"),
        ("review_approved", Locale::En) => (format!("Submission approved: {} {}", name, ver), "Submission approved", format!("Hello, {}:", dev), format!("Your plugin “{}” ({}; version {}) was published to the public catalog. Review does not guarantee safety or reliability; check the source, permissions, and code before use.", name, pid, ver), "View plugin"),
        ("review_rejected", Locale::Zh) => (format!("申请已退回：{} {}", name, ver), "申请已退回", format!("你好，{}：", dev), format!("你的插件「{}」（{}，版本 {}）已被退回。退回原因：{}。你可以修改后重新提交。", name, pid, ver, reason), "查看申请"),
        ("review_rejected", Locale::En) => (format!("Submission returned: {} {}", name, ver), "Submission returned", format!("Hello, {}:", dev), format!("Your plugin “{}” ({}; version {}) was returned. Reason: {}. You can update it and resubmit.", name, pid, ver, reason), "View submission"),
        ("version_published", Locale::Zh) => (format!("新版本已发布：{} {}", name, ver), "新版本已发布", format!("你好，{}：", dev), format!("你的插件「{}」（{}）的新版本 {} 已通过审核并发布。审核不代表插件安全或可靠，请自行核对插件内容。", name, pid, ver), "查看插件"),
        ("version_published", Locale::En) => (format!("New version published: {} {}", name, ver), "New version published", format!("Hello, {}:", dev), format!("Version {} of your plugin “{}” ({}) was published. Review does not guarantee safety or reliability; verify the plugin yourself.", ver, name, pid), "View plugin"),
        ("plugin_stopped", Locale::Zh) => (format!("插件已停止：{}", name), "插件已停止", format!("你好，{}：", dev), format!("你的插件「{}」（{}）已被停止并从公开目录移除。原因：{}。", name, pid, reason), "查看插件"),
        ("plugin_stopped", Locale::En) => (format!("Plugin stopped: {}", name), "Plugin stopped", format!("Hello, {}:", dev), format!("Your plugin “{}” ({}) was stopped and removed from the public catalog. Reason: {}.", name, pid, reason), "View plugin"),
        ("updates_halted", Locale::Zh) => (format!("插件已停止更新：{}", name), "插件已停止更新", format!("你好，{}：", dev), format!("你的插件「{}」（{}）已停止更新。当前版本 {} 仍保留在目录中，新版本暂不进入索引。原因：{}。", name, pid, ver, reason), "查看插件"),
        ("updates_halted", Locale::En) => (format!("Updates halted: {}", name), "Updates halted", format!("Hello, {}:", dev), format!("Updates for your plugin “{}” ({}) were halted. Current version {} remains in the catalog; new versions will not be indexed for now. Reason: {}.", name, pid, ver, reason), "View plugin"),
        ("plugin_resumed", Locale::Zh) => (format!("插件已恢复：{}", name), "插件已恢复", format!("你好，{}：", dev), format!("你的插件「{}」（{}）已恢复，重新进入公开目录并恢复更新。", name, pid), "查看插件"),
        ("plugin_resumed", Locale::En) => (format!("Plugin resumed: {}", name), "Plugin resumed", format!("Hello, {}:", dev), format!("Your plugin “{}” ({}) was resumed and is visible in the public catalog again.", name, pid), "View plugin"),
        ("admin_new_submission", Locale::Zh) => (format!("新申请待处理：{} {}", name, ver), "新申请待处理", "管理员你好：".into(), format!("开发者 {} 提交了插件「{}」（{}，版本 {}），请在后台处理。", dev, name, pid, ver), "打开审核队列"),
        ("admin_new_submission", Locale::En) => (format!("New submission to review: {} {}", name, ver), "New submission to review", "Hello admin:".into(), format!("Developer {} submitted “{}” ({}; version {}). Please review it in the console.", dev, name, pid, ver), "Open review queue"),
        ("welcome", Locale::Zh) => ("欢迎加入 WindowsIsland 插件目录".into(), "欢迎加入 WindowsIsland 插件目录", format!("你好，{}：", dev), "欢迎注册 WindowsIsland 插件目录。你可以连接 GitHub、提交和管理自己的插件。目录审核主要用于减少接口滥用，不代表插件安全或可靠。".into(), "打开开发者后台"),
        ("welcome", Locale::En) => ("Welcome to the WindowsIsland Plugin Catalog".into(), "Welcome to the WindowsIsland Plugin Catalog", format!("Hello, {}:", dev), "Welcome to the WindowsIsland Plugin Catalog. You can connect GitHub and submit or manage your plugins. Catalog review mainly helps reduce interface abuse and does not guarantee safety or reliability.".into(), "Open developer console"),
        ("login_alert", Locale::Zh) => ("检测到新的登录".into(), "检测到新的登录", format!("你好，{}：", dev), format!("你的 WindowsIsland 账号刚刚完成一次 GitHub 登录。如果这不是你本人操作，请立即检查 GitHub 账号并断开可疑授权。登录信息：{}。", reason), "打开开发者后台"),
        ("login_alert", Locale::En) => ("New sign-in detected".into(), "New sign-in detected", format!("Hello, {}:", dev), format!("Your WindowsIsland account was just signed in through GitHub. If this was not you, review your GitHub account and revoke any suspicious authorization immediately. Sign-in details: {}.", reason), "Open developer console"),
        ("test", Locale::Zh) => ("WindowsIsland 邮件配置测试".into(), "邮件配置测试", "你好：".into(), "这是一封测试邮件。如果你收到它，说明 SMTP 配置可以发送邮件。".into(), "打开开发者后台"),
        ("test", Locale::En) => ("WindowsIsland email configuration test".into(), "Email configuration test", "Hello:".into(), "This is a test email. If you received it, SMTP delivery is working.".into(), "Open developer console"),
        (other, _) => anyhow::bail!("未知邮件模板：{other:?}"),
    };
    let (event_label, catalog_label) = match locale {
        Locale::Zh => ("事件 ID", "WindowsIsland 插件目录"),
        Locale::En => ("Event ID", "WindowsIsland Plugin Catalog"),
    };
    let plain = format!(
        "{}\n\n{}\n\n{}\n\n{}\n{}\n\n{}：{}\n{}",
        html_unescape(title),
        html_unescape(&intro),
        html_unescape(&detail),
        html_unescape(action),
        html_unescape(&link),
        event_label,
        html_unescape(&event_id),
        catalog_label
    );
    let html_event_label = html_escape(event_label);
    let html_catalog_label = html_escape(catalog_label);
    let html = format!(
        r#"<!doctype html><html><body style="margin:0;background:#f6f9fc;color:#414552;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif"><div style="padding:64px 16px"><table role="presentation" align="center" width="600" style="max-width:600px;width:100%;background:#fff;border-radius:16px"><tr><td style="padding:36px 48px 28px"><div style="font-size:20px;font-weight:700;color:#625afa">WindowsIsland</div></td></tr><tr><td style="padding:0 48px 12px;font-size:24px;line-height:32px;font-weight:700">{title}</td></tr><tr><td style="padding:0 48px 32px;font-size:16px;line-height:24px">{intro}<br><br>{detail}<br><br><a href="{link}" style="display:inline-block;background:#625afa;color:#fff;padding:12px 18px;border-radius:8px;font-weight:600;text-decoration:none">{action}</a></td></tr><tr><td style="padding:20px 48px 36px;color:#687385;font-size:12px;line-height:20px;border-top:1px solid #e6ebf1">{html_event_label}: {event_id}<br>{html_catalog_label}</td></tr></table></div></body></html>"#
    );
    Ok((html_unescape(&subject), plain, html))
}

/// 渲染模板并落库入队；返回 email_logs 行 id
pub async fn queue_email(
    state: &AppState,
    to: &str,
    template: &str,
    plugin_id: Option<&str>,
    vars: &MailVars,
) -> Result<i64> {
    let event_id = new_event_id();
    queue_email_with_event_id(state, to, template, plugin_id, vars, &event_id, "zh").await
}

/// Render and queue an email with a caller-supplied event ID and language.
/// The same ID is retained across SMTP retries and appears in both email formats.
pub async fn queue_email_with_event_id(
    state: &AppState,
    to: &str,
    template: &str,
    plugin_id: Option<&str>,
    vars: &MailVars,
    event_id: &str,
    preferred_language: &str,
) -> Result<i64> {
    let event_id = if event_id.trim().is_empty() {
        new_event_id()
    } else {
        event_id.trim().to_string()
    };
    let (subject, body, html) = render(
        template,
        vars,
        Locale::from_preference(preferred_language),
        &event_id,
    )?;
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO email_logs (event_id, to_address, template, subject, body, body_html, plugin_id)
         VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id",
    )
    .bind(&event_id)
    .bind(to)
    .bind(template)
    .bind(&subject)
    .bind(&body)
    .bind(&html)
    .bind(plugin_id)
    .fetch_one(state.pool())
    .await?;
    state.enqueue_mail(row.0);
    Ok(row.0)
}

pub async fn run_worker(state: AppState, mut rx: tokio::sync::mpsc::UnboundedReceiver<i64>) {
    while let Some(id) = rx.recv().await {
        let st = state.clone();
        tokio::spawn(async move {
            if let Err(e) = process_job(&st, id).await {
                tracing::error!("email job {id} 处理异常: {e:#}");
            }
        });
    }
}

async fn process_job(state: &AppState, id: i64) -> Result<()> {
    #[derive(sqlx::FromRow)]
    struct Row {
        to_address: String,
        subject: String,
        body: String,
        body_html: String,
    }
    let Some(row) = sqlx::query_as::<_, Row>(
        "SELECT to_address, subject, body, COALESCE(body_html, '') AS body_html FROM email_logs WHERE id = $1 AND status = 'queued'",
    )
    .bind(id)
    .fetch_optional(state.pool())
    .await?
    else {
        return Ok(());
    };

    // 先做配置级校验：配置错误不重试（重试也无法成功），直接失败并给出可读原因
    let settings = match load_smtp_settings(state).await {
        Ok(s) => s,
        Err(reason) => {
            let error = format!("配置错误（未尝试发送）：{reason}");
            tracing::error!("email {id} 发送放弃：{error}");
            sqlx::query("UPDATE email_logs SET status='failed', attempts=0, error=$1 WHERE id=$2")
                .bind(&error)
                .bind(id)
                .execute(state.pool())
                .await?;
            return Ok(());
        }
    };

    let mut attempts = 0u32;
    let mut last_err;
    // 配置已通过校验，剩下的视为临时发送失败：初次尝试 + 最多 3 次重试，指数退避（10s / 30s / 90s）
    loop {
        attempts += 1;
        match send_once(
            &settings,
            &row.to_address,
            &row.subject,
            &row.body,
            &row.body_html,
        )
        .await
        {
            Ok(()) => {
                sqlx::query(
                    "UPDATE email_logs SET status='sent', attempts=$1, sent_at=now(), error=NULL WHERE id=$2",
                )
                .bind(attempts as i32)
                .bind(id)
                .execute(state.pool())
                .await?;
                return Ok(());
            }
            Err(e) => {
                last_err = format!("{e:#}");
                tracing::warn!("email {id} 第 {attempts} 次发送失败: {last_err}");
                if attempts >= 4 {
                    break;
                }
                let delay_secs = 10u64 * 3u64.pow(attempts - 1);
                tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
            }
        }
    }
    sqlx::query("UPDATE email_logs SET status='failed', attempts=$1, error=$2 WHERE id=$3")
        .bind(attempts as i32)
        .bind(last_err)
        .bind(id)
        .execute(state.pool())
        .await?;
    Ok(())
}

/// SMTP 配置（已解密）。构造即完成全部配置校验，构造失败 = 配置错误。
struct SmtpSettings {
    mailer: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

async fn load_smtp_settings(state: &AppState) -> std::result::Result<SmtpSettings, String> {
    let host = state
        .site_config_plain("smtp_host")
        .await
        .unwrap_or_default();
    if host.is_empty() {
        return Err("SMTP 服务器地址未配置".into());
    }
    let port: u16 = state
        .site_config_plain("smtp_port")
        .await
        .unwrap_or_else(|| "587".into())
        .parse()
        .map_err(|_| "SMTP 端口不是合法数字")?;
    let username = state
        .site_config_plain("smtp_username")
        .await
        .unwrap_or_default();
    let password = state
        .site_config_plain("smtp_password")
        .await
        .unwrap_or_default();
    let encryption = state
        .site_config_plain("smtp_encryption")
        .await
        .unwrap_or_else(|| "starttls".into());
    let from_addr = state
        .site_config_plain("smtp_from_address")
        .await
        .unwrap_or_default();
    let from_name = state
        .site_config_plain("smtp_from_name")
        .await
        .unwrap_or_default();
    if from_addr.is_empty() {
        return Err("发件人地址未配置".into());
    }

    let mut builder = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&host).port(port);
    match encryption.as_str() {
        "starttls" => {
            builder = builder.tls(lettre::transport::smtp::client::Tls::Required(
                lettre::transport::smtp::client::TlsParameters::new(host.clone())
                    .map_err(|e| format!("STARTTLS 参数初始化失败（{host}）：{e}"))?,
            ));
        }
        "ssl" => {
            builder = builder.tls(lettre::transport::smtp::client::Tls::Wrapper(
                lettre::transport::smtp::client::TlsParameters::new(host.clone())
                    .map_err(|e| format!("SSL 参数初始化失败（{host}）：{e}"))?,
            ));
        }
        "none" => {}
        other => {
            return Err(format!(
                "未知 SMTP 加密方式：{other}（应为 starttls / ssl / none）"
            ))
        }
    }
    if !username.is_empty() {
        builder = builder.credentials(Credentials::new(username, password));
    }
    let from: Mailbox = if from_name.is_empty() {
        from_addr
            .parse()
            .map_err(|_| format!("发件人地址不合法：{from_addr}"))?
    } else {
        Mailbox::new(
            Some(from_name),
            from_addr
                .parse()
                .map_err(|_| format!("发件人地址不合法：{from_addr}"))?,
        )
    };
    Ok(SmtpSettings {
        mailer: builder.build(),
        from,
    })
}

async fn send_once(
    settings: &SmtpSettings,
    to: &str,
    subject: &str,
    body: &str,
    body_html: &str,
) -> Result<()> {
    let to_mb: Mailbox = to.parse().context("收件人地址不合法")?;
    let email = Message::builder()
        .from(settings.from.clone())
        .to(to_mb)
        .subject(subject)
        .multipart(
            MultiPart::alternative()
                .singlepart(SinglePart::plain(body.to_string()))
                .singlepart(SinglePart::html(body_html.to_string())),
        )
        .context("构建邮件失败")?;
    settings.mailer.send(email).await.context("SMTP 发送失败")?;
    Ok(())
}

/// 服务启动时恢复上一实例遗留的 queued 邮件。
/// 只恢复入队超过 5 分钟的行，避免与刚崩溃实例的在途任务重复发送。
pub async fn recover_queued(state: &AppState) {
    let ids: Vec<(i64,)> = sqlx::query_as(
        "SELECT id FROM email_logs WHERE status = 'queued' AND created_at < now() - interval '5 minutes'",
    )
    .fetch_all(state.pool())
    .await
    .unwrap_or_default();
    if ids.is_empty() {
        return;
    }
    tracing::info!("恢复 {} 封上次实例遗留的待发邮件", ids.len());
    for (id,) in ids {
        state.enqueue_mail(id);
    }
}

/// 给全部管理员发通知（新申请待审核）
pub async fn notify_admins(
    state: &AppState,
    template: &str,
    vars: &MailVars,
    plugin_id: Option<&str>,
) {
    let admins: Vec<(Option<String>, String)> = sqlx::query_as(
        "SELECT email, COALESCE(preferred_language, 'zh') FROM developers WHERE is_admin AND NOT is_banned AND email IS NOT NULL AND email <> ''",
    )
    .fetch_all(state.pool())
    .await
    .unwrap_or_default();
    for (email, language) in admins
        .into_iter()
        .filter_map(|(e, language)| e.map(|email| (email, language)))
    {
        // Each recipient gets its own immutable email event ID; sharing one ID
        // would violate the unique index and make retries ambiguous.
        let event_id = new_event_id();
        if let Err(e) = queue_email_with_event_id(
            state, &email, template, plugin_id, vars, &event_id, &language,
        )
        .await
        {
            tracing::error!("入队管理员通知失败: {e:#}");
        }
    }
}

pub async fn notify_developer_with_event(
    state: &AppState,
    dev_row_id: i64,
    template: &str,
    vars: &MailVars,
    plugin_id: Option<&str>,
    event_id: Option<&str>,
) {
    let email: Option<(Option<String>, String)> = sqlx::query_as(
        "SELECT email, COALESCE(preferred_language, 'zh') FROM developers WHERE id = $1 AND NOT is_system AND email IS NOT NULL AND email <> ''",
    )
    .bind(dev_row_id)
    .fetch_optional(state.pool())
    .await
    .unwrap_or(None);
    if let Some((Some(email), language)) = email {
        let event_id = event_id.map(str::to_owned).unwrap_or_else(new_event_id);
        if let Err(e) = queue_email_with_event_id(
            state, &email, template, plugin_id, vars, &event_id, &language,
        )
        .await
        {
            tracing::error!("入队开发者通知失败: {e:#}");
        }
    }
}

pub async fn notify_welcome(state: &AppState, dev_row_id: i64, event_id: &str) {
    let row: Option<(Option<String>, String, String)> = sqlx::query_as(
        "SELECT email, COALESCE(display_name, ''), COALESCE(preferred_language, 'zh')
         FROM developers WHERE id=$1 AND NOT is_system AND email IS NOT NULL AND email <> ''",
    )
    .bind(dev_row_id)
    .fetch_optional(state.pool())
    .await
    .unwrap_or(None);
    if let Some((Some(email), display_name, language)) = row {
        let vars = MailVars {
            plugin_name: String::new(),
            plugin_id: String::new(),
            version: String::new(),
            developer: display_name,
            reason: None,
            link: format!("{}/developer", state.config().app_base_url),
        };
        if let Err(error) =
            queue_email_with_event_id(state, &email, "welcome", None, &vars, event_id, &language)
                .await
        {
            tracing::error!("欢迎邮件入队失败: {error:#}");
        }
    }
}

pub async fn notify_unusual_login(
    state: &AppState,
    dev_row_id: i64,
    event_id: &str,
    ip: &str,
    user_agent: &str,
    login_at: chrono::DateTime<chrono::Utc>,
) {
    let row: Option<(Option<String>, String, String)> = sqlx::query_as(
        "SELECT email, COALESCE(display_name, ''), COALESCE(preferred_language, 'zh')
         FROM developers WHERE id=$1 AND NOT is_system AND email IS NOT NULL AND email <> ''",
    )
    .bind(dev_row_id)
    .fetch_optional(state.pool())
    .await
    .unwrap_or(None);
    if let Some((Some(email), display_name, language)) = row {
        let detail = if Locale::from_preference(&language) == Locale::Zh {
            format!(
                "IP：{}；用户代理：{}；时间：{} UTC",
                ip,
                user_agent.chars().take(180).collect::<String>(),
                login_at.to_rfc3339()
            )
        } else {
            format!(
                "IP: {}; User-Agent: {}; Time: {} UTC",
                ip,
                user_agent.chars().take(180).collect::<String>(),
                login_at.to_rfc3339()
            )
        };
        let vars = MailVars {
            plugin_name: String::new(),
            plugin_id: String::new(),
            version: String::new(),
            developer: display_name,
            reason: Some(detail),
            link: format!("{}/developer", state.config().app_base_url),
        };
        if let Err(error) = queue_email_with_event_id(
            state,
            &email,
            "login_alert",
            None,
            &vars,
            event_id,
            &language,
        )
        .await
        {
            tracing::error!("异常登录提醒入队失败: {error:#}");
        }
    }
}
