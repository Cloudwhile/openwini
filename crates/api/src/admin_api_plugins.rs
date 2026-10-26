
#[derive(Deserialize)]
struct AdminPluginQuery {
    q: Option<String>,
    status: Option<String>,
    page: Option<i64>,
}

async fn admin_plugins(
    State(state): State<AppState>,
    _admin: AdminUser,
    Query(q): Query<AdminPluginQuery>,
) -> ApiResult<Json<Value>> {
    let page_size: i64 = 20;
    let page = q.page.unwrap_or(1).max(1);
    let pattern = format!(
        "%{}%",
        q.q.clone()
            .unwrap_or_default()
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let status = q.status.clone().unwrap_or_default();
    let rows: Vec<(
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        bool,
        Option<String>,
        Option<String>,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT p.plugin_id, d.developer_id, COALESCE(NULLIF(d.display_name,''), d.developer_id),
                p.status, p.current_version, p.repository_url, p.updates_halted,
                p.stop_reason, p.updates_halted_reason, p.updated_at
         FROM plugins p JOIN developers d ON d.id = p.developer_id
         WHERE ($1 = '' OR p.plugin_id ILIKE $1 OR d.developer_id ILIKE $1)
           AND ($2 = '' OR p.status = $2)
         ORDER BY p.updated_at DESC LIMIT $3 OFFSET $4",
    )
    .bind(&pattern)
    .bind(&status)
    .bind(page_size)
    .bind((page - 1) * page_size)
    .fetch_all(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM plugins p JOIN developers d ON d.id = p.developer_id
         WHERE ($1 = '' OR p.plugin_id ILIKE $1 OR d.developer_id ILIKE $1)
           AND ($2 = '' OR p.status = $2)",
    )
    .bind(&pattern)
    .bind(&status)
    .fetch_one(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    let items: Vec<Value> = rows
        .into_iter()
        .map(|(plugin_id, dev, dev_name, status, version, repo, halted, stop_reason, halt_reason, updated)| {
            json!({
                "pluginId": plugin_id, "developerId": dev, "developerName": dev_name,
                "status": status,
                "displayStatus": crate::status::display_status(&status, halted, stop_reason.is_some()),
                "version": version.unwrap_or_default(), "repositoryUrl": repo,
                "updatesHalted": halted, "stopReason": stop_reason, "updatesHaltedReason": halt_reason,
                "official": dev == v::OFFICIAL_DEVELOPER_ID,
                "updatedAt": updated,
            })
        })
        .collect();
    Ok(Json(
        json!({ "items": items, "total": total.0, "page": page, "pageSize": page_size }),
    ))
}

async fn load_plugin_mut(
    tx: &mut PgConnection,
    plugin_id: &str,
) -> ApiResult<(i64, String, String, i64, bool)> {
    let row = sqlx::query_as::<_, (i64, String, String, i64, bool)>(
        "SELECT p.id, p.plugin_id, p.status, p.developer_id, p.updates_halted
         FROM plugins p WHERE p.plugin_id = $1 FOR UPDATE",
    )
    .bind(plugin_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?
    .ok_or_else(|| AppError::NotFound("插件不存在".into()))?;
    Ok(row)
}

#[derive(Deserialize)]
struct ReasonBody {
    reason: Option<String>,
    note: Option<String>,
}

fn require_reason(body: &ReasonBody, label: &str) -> ApiResult<String> {
    body.reason
        .clone()
        .or_else(|| body.note.clone())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::BadRequest(format!("{label}必须填写原因")))
}

/// 停止：立即从公开索引移除
async fn stop_plugin(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(plugin_id): Path<String>,
    Json(body): Json<ReasonBody>,
) -> ApiResult<Json<Value>> {
    let reason = require_reason(&body, "停止操作")?;
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    let (row_id, plugin_id, status, dev_row, _halted) =
        load_plugin_mut(&mut tx, &plugin_id).await?;
    if !crate::status::admin_can_stop(&status) {
        return Err(AppError::Conflict("只有已发布的插件才能停止".into()));
    }
    // 同事务：插件 → stopped；其 pending 申请自动退回；全部记审计
    sqlx::query(
        "UPDATE plugins SET status='stopped', stop_reason=$1, updates_halted=FALSE, updates_halted_reason=NULL, updated_at=now() WHERE id=$2",
    )
    .bind(&reason)
    .bind(row_id)
    .execute(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?;
    let pending: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, version, manifest_name FROM submissions WHERE plugin_id=$1 AND status='pending'",
    )
    .bind(row_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?;
    for (sid, _version, _name) in &pending {
        let why = format!("插件已被停止：{reason}");
        sqlx::query("UPDATE submissions SET status='rejected', reviewed_by=$1, reviewed_at=now(), review_reason=$2 WHERE id=$3")
            .bind(admin.0.dev.id)
            .bind(&why)
            .bind(sid)
            .execute(&mut *tx)
            .await
            .map_err(anyhow::Error::from)?;
        crate::audit::write(
            &mut tx,
            &admin.0.dev.developer_id,
            "admin",
            "submission_rejected",
            Some(&plugin_id),
            Some(*sid),
            Some("pending"),
            Some("rejected"),
            Some(&why),
        )
        .await?;
    }
    let event_id = crate::audit::write(
        &mut tx,
        &admin.0.dev.developer_id,
        "admin",
        "plugin_stopped",
        Some(&plugin_id),
        None,
        Some("published"),
        Some("stopped"),
        Some(&reason),
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;

    let manifest_name: Option<String> = sqlx::query_scalar(
        "SELECT s.manifest_name FROM submissions s WHERE s.id = (SELECT current_submission_id FROM plugins WHERE id=$1)",
    )
    .bind(row_id)
    .fetch_optional(state.pool())
    .await
    .map_err(anyhow::Error::from)?
    .flatten();
    let version: Option<String> =
        sqlx::query_scalar("SELECT current_version FROM plugins WHERE id=$1")
            .bind(row_id)
            .fetch_optional(state.pool())
            .await
            .map_err(anyhow::Error::from)?
            .flatten();
    let vars = MailVars {
        plugin_name: manifest_name.unwrap_or_else(|| plugin_id.clone()),
        plugin_id: plugin_id.clone(),
        version: version.unwrap_or_default(),
        developer: admin.0.dev.display_name.clone(),
        reason: Some(reason),
        link: MailVars::link_for(&state, &plugin_id),
    };
    mailer::notify_developer_with_event(
        &state,
        dev_row,
        "plugin_stopped",
        &vars,
        Some(&plugin_id),
        Some(&event_id),
    )
    .await;
    Ok(Json(json!({ "ok": true, "status": "stopped", "eventId": event_id })))
}

/// 退回已发布插件：published → rejected（移出索引，开发者修改后可重新提交）
async fn unpublish_plugin(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(plugin_id): Path<String>,
    Json(body): Json<ReasonBody>,
) -> ApiResult<Json<Value>> {
    let reason = require_reason(&body, "退回操作")?;
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    let (row_id, plugin_id, status, dev_row, _halted) =
        load_plugin_mut(&mut tx, &plugin_id).await?;
    if !crate::status::admin_can_unpublish(&status) {
        return Err(AppError::Conflict("只有已发布的插件才能退回审核".into()));
    }
    // 同事务：插件 → rejected；该插件的 pending 新版本申请一并退回；全部记审计
    sqlx::query(
        "UPDATE plugins SET status='rejected', stop_reason=$1, updates_halted=FALSE, updates_halted_reason=NULL, updated_at=now() WHERE id=$2",
    )
    .bind(&reason)
    .bind(row_id)
    .execute(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?;
    let pending: Vec<(i64,)> =
        sqlx::query_as("SELECT id FROM submissions WHERE plugin_id=$1 AND status='pending'")
            .bind(row_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(anyhow::Error::from)?;
    for (sid,) in &pending {
        let why = format!("插件被管理员退回审核：{reason}");
        sqlx::query(
            "UPDATE submissions SET status='rejected', reviewed_by=$1, reviewed_at=now(), review_reason=$2 WHERE id=$3",
        )
        .bind(admin.0.dev.id)
        .bind(&why)
        .bind(sid)
        .execute(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
        crate::audit::write(
            &mut tx,
            &admin.0.dev.developer_id,
            "admin",
            "submission_rejected",
            Some(&plugin_id),
            Some(*sid),
            Some("pending"),
            Some("rejected"),
            Some(&why),
        )
        .await?;
    }
    let event_id = crate::audit::write(
        &mut tx,
        &admin.0.dev.developer_id,
        "admin",
        "plugin_unpublished",
        Some(&plugin_id),
        None,
        Some("published"),
        Some("rejected"),
        Some(&reason),
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;

    let (manifest_name, version) = current_plugin_info(&state, row_id).await;
    let vars = MailVars {
        plugin_name: if manifest_name.is_empty() {
            plugin_id.clone()
        } else {
            manifest_name
        },
        plugin_id: plugin_id.clone(),
        version,
        developer: admin.0.dev.display_name.clone(),
        reason: Some(reason),
        link: MailVars::link_for(&state, &plugin_id),
    };
    mailer::notify_developer_with_event(
        &state,
        dev_row,
        "review_rejected",
        &vars,
        Some(&plugin_id),
        Some(&event_id),
    )
    .await;
    Ok(Json(json!({ "ok": true, "status": "rejected", "eventId": event_id })))
}

/// 停止更新（冻结更新链）：索引保留当前版本，新版本不再进入索引
async fn halt_updates(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(plugin_id): Path<String>,
    Json(body): Json<ReasonBody>,
) -> ApiResult<Json<Value>> {
    let reason = require_reason(&body, "停止更新操作")?;
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    let (row_id, plugin_id, status, dev_row, halted) = load_plugin_mut(&mut tx, &plugin_id).await?;
    if !crate::status::admin_can_halt(&status, halted) {
        return Err(AppError::Conflict(
            "只有已发布且未冻结更新的插件才能停止更新".into(),
        ));
    }
    sqlx::query("UPDATE plugins SET updates_halted=TRUE, updates_halted_reason=$1, updated_at=now() WHERE id=$2")
        .bind(&reason)
        .bind(row_id)
        .execute(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
    let event_id = crate::audit::write(
        &mut tx,
        &admin.0.dev.developer_id,
        "admin",
        "updates_halted",
        Some(&plugin_id),
        None,
        Some("published"),
        Some("updates_halted"),
        Some(&reason),
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;

    let info = current_plugin_info(&state, row_id).await;
    let vars = MailVars {
        plugin_name: info.0,
        plugin_id: plugin_id.clone(),
        version: info.1,
        developer: admin.0.dev.display_name.clone(),
        reason: Some(reason),
        link: MailVars::link_for(&state, &plugin_id),
    };
    mailer::notify_developer_with_event(
        &state,
        dev_row,
        "updates_halted",
        &vars,
        Some(&plugin_id),
        Some(&event_id),
    )
    .await;
    Ok(Json(json!({ "ok": true, "updatesHalted": true, "eventId": event_id })))
}

/// 恢复：stopped → published（重新进入索引），或解除停止更新
async fn resume_plugin(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(plugin_id): Path<String>,
    Json(body): Json<ReasonBody>,
) -> ApiResult<Json<Value>> {
    let note = body
        .note
        .clone()
        .or(body.reason.clone())
        .unwrap_or_default();
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    let (row_id, plugin_id, status, dev_row, was_halted) =
        load_plugin_mut(&mut tx, &plugin_id).await?;
    if !crate::status::admin_can_resume(&status, was_halted) {
        return Err(AppError::Conflict(format!(
            "状态 {status} 的插件不适用恢复操作（停止更新状态显示为「已发布 + 停止更新」）"
        )));
    }
    let (from, to, action) = if status == "stopped" {
        ("stopped", "published", "plugin_resumed")
    } else {
        ("updates_halted", "published", "updates_resumed")
    };
    sqlx::query(
        "UPDATE plugins SET status='published', stop_reason=NULL, updates_halted=FALSE, updates_halted_reason=NULL, updated_at=now() WHERE id=$1",
    )
    .bind(row_id)
    .execute(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?;
    let event_id = crate::audit::write(
        &mut tx,
        &admin.0.dev.developer_id,
        "admin",
        action,
        Some(&plugin_id),
        None,
        Some(from),
        Some(to),
        if note.trim().is_empty() {
            None
        } else {
            Some(note.trim())
        },
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;

    let info = current_plugin_info(&state, row_id).await;
    let vars = MailVars {
        plugin_name: info.0,
        plugin_id: plugin_id.clone(),
        version: info.1,
        developer: admin.0.dev.display_name.clone(),
        reason: None,
        link: MailVars::link_for(&state, &plugin_id),
    };
    mailer::notify_developer_with_event(
        &state,
        dev_row,
        "plugin_resumed",
        &vars,
        Some(&plugin_id),
        Some(&event_id),
    )
    .await;
    Ok(Json(json!({ "ok": true, "status": "published", "eventId": event_id })))
}

async fn current_plugin_info(state: &AppState, row_id: i64) -> (String, String) {
    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT s.manifest_name, p.current_version
         FROM plugins p LEFT JOIN submissions s ON s.id = p.current_submission_id
         WHERE p.id = $1",
    )
    .bind(row_id)
    .fetch_optional(state.pool())
    .await
    .unwrap_or(None);
    match row {
        Some((name, ver)) => (name.unwrap_or_default(), ver.unwrap_or_default()),
        None => (String::new(), String::new()),
    }
}

#[derive(Deserialize)]
struct LinksBody {
    #[serde(rename = "homepageUrl")]
    homepage_url: Option<String>,
    #[serde(rename = "docsUrl")]
    docs_url: Option<String>,
}

async fn update_links(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(plugin_id): Path<String>,
    Json(body): Json<LinksBody>,
) -> ApiResult<Json<Value>> {
    let hp = body
        .homepage_url
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let dc = body
        .docs_url
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    for (u, label) in [(hp.as_deref(), "主页链接"), (dc.as_deref(), "文档链接")] {
        if let Some(u) = u {
            url::Url::parse(u)
                .map_err(|_| AppError::BadRequest(format!("{label} 不是合法 URL")))?;
        }
    }
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    let (row_id, plugin_id, _status, _dev_row, _halted) =
        load_plugin_mut(&mut tx, &plugin_id).await?;
    sqlx::query("UPDATE plugins SET homepage_url=$1, docs_url=$2, updated_at=now() WHERE id=$3")
        .bind(&hp)
        .bind(&dc)
        .bind(row_id)
        .execute(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
    let audit_reason = format!(
        "homepage={:?} docs={:?}",
        hp.clone().unwrap_or_default(),
        dc.clone().unwrap_or_default()
    );
    crate::audit::write(
        &mut tx,
        &admin.0.dev.developer_id,
        "admin",
        "plugin_links_updated",
        Some(&plugin_id),
        None,
        None,
        None,
        Some(&audit_reason),
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "ok": true })))
}

// —— 官方身份（winiofficial）管理 ——
