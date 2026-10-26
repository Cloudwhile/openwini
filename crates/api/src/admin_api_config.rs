
async fn get_site_config(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!({
        "siteName": state.site_config("site_name").await.unwrap_or_default(),
        "siteNameEn": state.site_config("site_name_en").await.unwrap_or_default(),
        "announcement": state.site_config("announcement").await.unwrap_or_default(),
        "pageSize": state.site_config("page_size").await.unwrap_or_else(|| "20".into()),
        "publicCatalogEnabled": state.site_config("public_catalog_enabled").await.unwrap_or_else(|| "true".into()) == "true",
        "catalogBaseUrl": state.catalog_base_url().await,
    })))
}

async fn put_site_config(
    State(state): State<AppState>,
    admin: AdminUser,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    let actor = admin.0.dev.developer_id.clone();
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    if let Some(v) = body.get("siteName").and_then(|v| v.as_str()) {
        if v.trim().is_empty() || v.chars().count() > 100 {
            return Err(AppError::BadRequest(
                "站点名称必须非空且不超过 100 字符".into(),
            ));
        }
        state
            .set_site_config_tx(&mut *tx, "site_name", v.trim(), false, &actor)
            .await
            .map_err(anyhow::Error::from)?;
    }
    if let Some(v) = body.get("siteNameEn").and_then(|v| v.as_str()) {
        if v.chars().count() > 100 {
            return Err(AppError::BadRequest("英文站点名称不能超过 100 字符".into()));
        }
        state
            .set_site_config_tx(&mut *tx, "site_name_en", v.trim(), false, &actor)
            .await
            .map_err(anyhow::Error::from)?;
    }
    if let Some(v) = body.get("announcement").and_then(|v| v.as_str()) {
        state
            .set_site_config_tx(&mut *tx, "announcement", v, false, &actor)
            .await
            .map_err(anyhow::Error::from)?;
    }
    if let Some(v) = body.get("pageSize").and_then(|v| v.as_i64()) {
        if !(1..=100).contains(&v) {
            return Err(AppError::BadRequest("每页条数必须在 1-100".into()));
        }
        state
            .set_site_config_tx(&mut *tx, "page_size", &v.to_string(), false, &actor)
            .await
            .map_err(anyhow::Error::from)?;
    }
    if let Some(v) = body.get("publicCatalogEnabled").and_then(|v| v.as_bool()) {
        state
            .set_site_config_tx(
                &mut *tx,
                "public_catalog_enabled",
                &v.to_string(),
                false,
                &actor,
            )
            .await
            .map_err(anyhow::Error::from)?;
    }
    if let Some(v) = body.get("catalogBaseUrl").and_then(|v| v.as_str()) {
        let u = v.trim().trim_end_matches('/');
        let parsed = url::Url::parse(u)
            .map_err(|_| AppError::BadRequest("目录基地址不是合法 URL".into()))?;
        if parsed.scheme() != "https"
            && parsed.host_str() != Some("localhost")
            && !parsed.host_str().unwrap_or("").starts_with("127.")
        {
            // 允许 http 仅限本机调试
            return Err(AppError::BadRequest(
                "目录基地址必须是 https；http 仅限 localhost/127.0.0.1".into(),
            ));
        }
        state
            .set_site_config_tx(&mut *tx, "catalog_base_url", u, false, &actor)
            .await
            .map_err(anyhow::Error::from)?;
    }
    crate::audit::write(
        &mut tx,
        &admin.0.dev.developer_id,
        "admin",
        "site_config_updated",
        None,
        None,
        None,
        None,
        Some(&serde_json::to_string(&body).unwrap_or_default()),
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    state.invalidate_site_cache().await;
    Ok(Json(json!({ "ok": true })))
}

async fn get_smtp_config(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> ApiResult<Json<Value>> {
    let has_password = !state
        .site_config_plain("smtp_password")
        .await
        .unwrap_or_default()
        .is_empty();
    Ok(Json(json!({
        "host": state.site_config("smtp_host").await.unwrap_or_default(),
        "port": state.site_config("smtp_port").await.unwrap_or_else(|| "587".into()),
        "username": state.site_config("smtp_username").await.unwrap_or_default(),
        "hasPassword": has_password,
        "encryption": state.site_config("smtp_encryption").await.unwrap_or_else(|| "starttls".into()),
        "fromAddress": state.site_config("smtp_from_address").await.unwrap_or_default(),
        "fromName": state.site_config("smtp_from_name").await.unwrap_or_default(),
    })))
}

async fn put_smtp_config(
    State(state): State<AppState>,
    admin: AdminUser,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    let actor = admin.0.dev.developer_id.clone();
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    let str_field = |v: Option<&Value>| v.and_then(|x| x.as_str()).map(|s| s.to_string());
    if let Some(host) = str_field(body.get("host")) {
        state
            .set_site_config_tx(&mut *tx, "smtp_host", &host, false, &actor)
            .await
            .map_err(anyhow::Error::from)?;
    }
    if let Some(port) = str_field(body.get("port")) {
        port.parse::<u16>()
            .map_err(|_| AppError::BadRequest("端口不合法".into()))?;
        state
            .set_site_config_tx(&mut *tx, "smtp_port", &port, false, &actor)
            .await
            .map_err(anyhow::Error::from)?;
    }
    if let Some(u) = str_field(body.get("username")) {
        state
            .set_site_config_tx(&mut *tx, "smtp_username", &u, false, &actor)
            .await
            .map_err(anyhow::Error::from)?;
    }
    if let Some(p) = str_field(body.get("password")) {
        if !p.is_empty() {
            state
                .set_site_config_tx(&mut *tx, "smtp_password", &p, true, &actor)
                .await
                .map_err(anyhow::Error::from)?;
        }
    }
    if let Some(enc) = str_field(body.get("encryption")) {
        if !["starttls", "ssl", "none"].contains(&enc.as_str()) {
            return Err(AppError::BadRequest(
                "加密方式必须是 starttls / ssl / none".into(),
            ));
        }
        state
            .set_site_config_tx(&mut *tx, "smtp_encryption", &enc, false, &actor)
            .await
            .map_err(anyhow::Error::from)?;
    }
    if let Some(f) = str_field(body.get("fromAddress")) {
        state
            .set_site_config_tx(&mut *tx, "smtp_from_address", &f, false, &actor)
            .await
            .map_err(anyhow::Error::from)?;
    }
    if let Some(f) = str_field(body.get("fromName")) {
        state
            .set_site_config_tx(&mut *tx, "smtp_from_name", &f, false, &actor)
            .await
            .map_err(anyhow::Error::from)?;
    }
    crate::audit::write(
        &mut tx,
        &actor,
        "admin",
        "smtp_config_updated",
        None,
        None,
        None,
        None,
        None,
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    state.invalidate_site_cache().await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct SmtpTestBody {
    to: String,
}

async fn smtp_test(
    State(state): State<AppState>,
    admin: AdminUser,
    Json(body): Json<SmtpTestBody>,
) -> ApiResult<Json<Value>> {
    let vars = MailVars {
        plugin_name: String::new(),
        plugin_id: String::new(),
        version: String::new(),
        developer: String::new(),
        reason: None,
        link: state.config().app_base_url.clone(),
    };
    let id = mailer::queue_email(&state, body.to.trim(), "test", None, &vars)
        .await
        .map_err(|e| AppError::BadRequest(format!("入队失败：{e}")))?;
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    crate::audit::write(
        &mut tx,
        &admin.0.dev.developer_id,
        "admin",
        "smtp_test_sent",
        None,
        None,
        None,
        None,
        None,
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "queued": true, "emailLogId": id })))
}

#[derive(Deserialize)]
struct SmtpLogsQuery {
    page: Option<i64>,
}

async fn smtp_logs(
    State(state): State<AppState>,
    _admin: AdminUser,
    Query(q): Query<SmtpLogsQuery>,
) -> ApiResult<Json<Value>> {
    let page_size: i64 = 20;
    let page = q.page.unwrap_or(1).max(1);
    let rows: Vec<(String, String, String, String, String, Option<String>, i32, Option<String>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT event_id, to_address, template, status, COALESCE(subject,''), plugin_id, attempts, error, created_at
         FROM email_logs ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(page_size)
    .bind((page - 1) * page_size)
    .fetch_all(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM email_logs")
        .fetch_one(state.pool())
        .await
        .map_err(anyhow::Error::from)?;
    let items: Vec<Value> = rows
        .into_iter()
        .map(|(event_id, to, template, status, subject, plugin_id, attempts, error, created)| {
            json!({
                "eventId": event_id,
                "to": to, "template": template, "status": status, "subject": subject,
                "pluginId": plugin_id, "attempts": attempts, "error": error, "createdAt": created,
            })
        })
        .collect();
    Ok(Json(
        json!({ "items": items, "total": total.0, "page": page, "pageSize": page_size }),
    ))
}

async fn get_ai_config(State(state): State<AppState>, _admin: AdminUser) -> ApiResult<Json<Value>> {
    Ok(Json(json!({
        "enabled": state.site_config("ai_enabled").await.unwrap_or_else(|| "false".into()) == "true",
        "baseUrl": state.site_config("ai_base_url").await.unwrap_or_default(),
        "model": state.site_config("ai_model").await.unwrap_or_default(),
        "hasApiKey": !state.site_config_plain("ai_api_key").await.unwrap_or_default().is_empty(),
        "timeoutSec": state.site_config("ai_timeout_sec").await.unwrap_or_else(|| "60".into()),
    })))
}

async fn put_ai_config(
    State(state): State<AppState>,
    admin: AdminUser,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    let actor = admin.0.dev.developer_id.clone();
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    if let Some(e) = body.get("enabled").and_then(|v| v.as_bool()) {
        state
            .set_site_config_tx(
                &mut *tx,
                "ai_enabled",
                if e { "true" } else { "false" },
                false,
                &actor,
            )
            .await
            .map_err(anyhow::Error::from)?;
    }
    if let Some(v) = body.get("baseUrl").and_then(|v| v.as_str()) {
        state
            .set_site_config_tx(&mut *tx, "ai_base_url", v.trim(), false, &actor)
            .await
            .map_err(anyhow::Error::from)?;
    }
    if let Some(v) = body.get("model").and_then(|v| v.as_str()) {
        state
            .set_site_config_tx(&mut *tx, "ai_model", v.trim(), false, &actor)
            .await
            .map_err(anyhow::Error::from)?;
    }
    if let Some(v) = body.get("apiKey").and_then(|v| v.as_str()) {
        if !v.is_empty() {
            state
                .set_site_config_tx(&mut *tx, "ai_api_key", v, true, &actor)
                .await
                .map_err(anyhow::Error::from)?;
        }
    }
    if let Some(v) = body.get("timeoutSec").and_then(|v| v.as_i64()) {
        if !(5..=600).contains(&v) {
            return Err(AppError::BadRequest("超时时间必须在 5-600 秒".into()));
        }
        state
            .set_site_config_tx(&mut *tx, "ai_timeout_sec", &v.to_string(), false, &actor)
            .await
            .map_err(anyhow::Error::from)?;
    }
    crate::audit::write(
        &mut tx,
        &actor,
        "admin",
        "ai_config_updated",
        None,
        None,
        None,
        None,
        None,
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    state.invalidate_site_cache().await;
    Ok(Json(json!({ "ok": true })))
}
