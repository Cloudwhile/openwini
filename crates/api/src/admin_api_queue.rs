// —— 审核队列 ——

#[derive(Deserialize)]
struct QueueQuery {
    developer: Option<String>,
    permission: Option<String>,
    page: Option<i64>,
}

async fn queue(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(q): Query<QueueQuery>,
) -> ApiResult<Json<Value>> {
    let page_size: i64 = 20;
    let page = q.page.unwrap_or(1).max(1);
    let dev_filter = q.developer.clone().unwrap_or_default();
    let perm_filter = q.permission.clone().unwrap_or_default();
    // 按提交时间升序列出全部 pending 申请
    let rows: Vec<(i64, String, String, String, String, String, Vec<String>, String, String, bool, Option<Value>, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT s.id, p.plugin_id, d.developer_id, COALESCE(NULLIF(d.display_name,''), d.developer_id),
                s.version, s.manifest_name, s.permissions, p.repository_url, s.download_url,
                s.is_new_version, s.ai_report_json, s.ai_state, s.created_at
         FROM submissions s
         JOIN plugins p ON p.id = s.plugin_id
         JOIN developers d ON d.id = s.submitted_by
         WHERE s.status = 'pending'
           AND ($1 = '' OR d.developer_id ILIKE '%' || $1 || '%')
           AND ($2 = '' OR $2 = ANY(s.permissions))
         ORDER BY s.created_at ASC
         LIMIT $3 OFFSET $4",
    )
    .bind(&dev_filter)
    .bind(&perm_filter)
    .bind(page_size)
    .bind((page - 1) * page_size)
    .fetch_all(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM submissions s
         JOIN plugins p ON p.id = s.plugin_id
         JOIN developers d ON d.id = s.submitted_by
         WHERE s.status = 'pending'
           AND ($1 = '' OR d.developer_id ILIKE '%' || $1 || '%')
           AND ($2 = '' OR $2 = ANY(s.permissions))",
    )
    .bind(&dev_filter)
    .bind(&perm_filter)
    .fetch_one(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    let items: Vec<Value> = rows
        .into_iter()
        .map(
            |(
                id,
                plugin_id,
                dev_id,
                dev_name,
                version,
                name,
                perms,
                repo,
                dl,
                is_new,
                ai_report,
                ai_state,
                created,
            )| {
                json!({
                    "submissionId": id,
                    "pluginId": plugin_id,
                    "developerId": dev_id,
                    "developerName": dev_name,
                    "version": version,
                    "name": name,
                    "permissions": perms,
                    "repositoryUrl": repo,
                    "downloadUrl": dl,
                    "isNewVersion": is_new,
                    "aiReport": ai_report,
                    "aiState": ai_state,
                    "createdAt": created,
                    // isSelf 仅供前端标注「本人申请」（允许自审，审计照常记录）
                    "isSelf": dev_id == admin.0.dev.developer_id,
                })
            },
        )
        .collect();
    Ok(Json(
        json!({ "items": items, "total": total.0, "page": page, "pageSize": page_size }),
    ))
}

async fn queue_detail(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(submission_id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let row = sqlx::query(
        "SELECT s.id, s.version, s.manifest_json, s.manifest_name, s.description, s.permissions,
                s.download_url, s.sha256, s.min_host_version, s.contract_version, s.status,
                s.is_new_version, s.review_reason, s.ai_report_json, s.ai_state, s.created_at,
                p.plugin_id, p.status AS plugin_status, p.updates_halted, p.repository_url AS plugin_repo,
                d.developer_id, COALESCE(NULLIF(d.display_name,''), d.developer_id), d.is_banned,
                s.submitted_by
         FROM submissions s
         JOIN plugins p ON p.id = s.plugin_id
         JOIN developers d ON d.id = s.submitted_by
         WHERE s.id = $1",
    )
    .bind(submission_id)
    .fetch_optional(state.pool())
    .await
    .map_err(anyhow::Error::from)?
    .ok_or_else(|| AppError::NotFound("申请不存在".into()))?;
    let r = &row;
    let dev_id: String = r.get("developer_id");
    let submitted_by: i64 = r.get("submitted_by");
    Ok(Json(json!({
        "submissionId": r.get::<i64, _>("id"),
        "pluginId": r.get::<String, _>("plugin_id"),
        "pluginStatus": r.get::<String, _>("plugin_status"),
        "updatesHalted": r.get::<bool, _>("updates_halted"),
        "developerId": dev_id,
        "developerName": r.get::<String, _>("developer_name"),
        "developerBanned": r.get::<bool, _>("is_banned"),
        "version": r.get::<String, _>("version"),
        "name": r.get::<String, _>("manifest_name"),
        "description": r.get::<String, _>("description"),
        "manifest": r.get::<Value, _>("manifest_json"),
        "permissions": r.get::<Vec<String>, _>("permissions"),
        "repositoryUrl": r.get::<String, _>("plugin_repo"),
        "downloadUrl": r.get::<String, _>("download_url"),
        "sha256": r.get::<String, _>("sha256"),
        "minHostVersion": r.get::<String, _>("min_host_version"),
        "contractVersion": r.get::<i32, _>("contract_version"),
        "status": r.get::<String, _>("status"),
        "isNewVersion": r.get::<bool, _>("is_new_version"),
        "reviewReason": r.get::<Option<String>, _>("review_reason"),
        "aiReport": r.get::<Option<Value>, _>("ai_report_json"),
        "aiState": r.get::<String, _>("ai_state"),
        "createdAt": r.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
        "isSelf": dev_id == admin.0.dev.developer_id,
        "submittedByRow": submitted_by,
    })))
}

#[derive(Deserialize)]
struct ReviewBody {
    note: Option<String>,
    reason: Option<String>,
}

/// 通过：一次点击发布到公开索引
async fn approve(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(submission_id): Path<i64>,
    body: Option<Json<ReviewBody>>,
) -> ApiResult<Json<Value>> {
    let note = body
        .and_then(|Json(b)| b.note)
        .filter(|s| !s.trim().is_empty());
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;

    #[derive(sqlx::FromRow)]
    struct SRow {
        id: i64,
        plugin_row_id: i64,
        plugin_id: String,
        developer_row_id: i64,
        developer_id: String,
        status: String,
        plugin_status: String,
        updates_halted: bool,
        is_new_version: bool,
        version: String,
        manifest_name: String,
    }
    let s = sqlx::query_as::<_, SRow>(
        "SELECT s.id, s.plugin_id AS plugin_row_id, p.plugin_id, d.id AS developer_row_id,
                d.developer_id, s.status, p.status AS plugin_status, p.updates_halted,
                s.is_new_version, s.version, s.manifest_name
         FROM submissions s
         JOIN plugins p ON p.id = s.plugin_id
         JOIN developers d ON d.id = s.submitted_by
         WHERE s.id = $1 FOR UPDATE OF s",
    )
    .bind(submission_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?
    .ok_or_else(|| AppError::NotFound("申请不存在".into()))?;

    if s.status != "pending" {
        return Err(AppError::Conflict("该申请不在待审核状态".into()));
    }
    // 开发者被封禁时不能发布
    let banned: (bool,) = sqlx::query_as("SELECT is_banned FROM developers WHERE id=$1")
        .bind(s.developer_row_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
    if banned.0 {
        return Err(AppError::Conflict(
            "该开发者已被封禁，不能发布其申请".into(),
        ));
    }
    if s.updates_halted {
        return Err(AppError::Conflict(
            "该插件处于「停止更新」状态：请先恢复更新，再审核排队中的新版本".into(),
        ));
    }

    // 出索引前防御：契约全字段校验
    let entry = load_entry_for_submission(&state, submission_id).await?;
    let known = sub::known_permission_set(&state)
        .await
        .map_err(anyhow::Error::from)?;
    entry
        .validate(&known)
        .map_err(|e| AppError::Conflict(format!("发布校验未通过：{e}")))?;

    // 配额：源 ≤512、单源插件 ≤2048、全目录 ≤8192
    check_catalog_quotas(
        &mut tx,
        s.developer_row_id,
        s.is_new_version || s.plugin_status == "published",
    )
    .await?;

    // 同一开发者已发布插件必须同仓库（宿主源契约）
    let other_repo: Option<(String,)> = sqlx::query_as(
        "SELECT DISTINCT p2.repository_url FROM plugins p2
         WHERE p2.developer_id = $1 AND p2.status='published' AND p2.id <> $2 LIMIT 1",
    )
    .bind(s.developer_row_id)
    .bind(s.plugin_row_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?;
    let this_repo: (String,) = sqlx::query_as("SELECT repository_url FROM plugins WHERE id=$1")
        .bind(s.plugin_row_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
    if let Some((other,)) = other_repo {
        if other != this_repo.0 {
            return Err(AppError::Conflict(format!(
                "该开发者已有使用仓库 {other} 的已发布插件，宿主目录源要求同一开发者全部已发布插件同仓库，无法发布"
            )));
        }
    }

    let action = if s.plugin_status == "published" || s.is_new_version {
        "version_published"
    } else {
        "submission_approved"
    };
    sqlx::query(
        "UPDATE submissions SET status='approved', reviewed_by=$1, reviewed_at=now(), review_reason=$2 WHERE id=$3",
    )
    .bind(admin.0.dev.id)
    .bind(note.as_deref())
    .bind(s.id)
    .execute(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?;
    sqlx::query(
        "UPDATE plugins SET status='published', current_version=$1, current_submission_id=$2,
            stop_reason=NULL, updated_at=now()
         WHERE id=$3",
    )
    .bind(&s.version)
    .bind(s.id)
    .bind(s.plugin_row_id)
    .execute(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?;
    let event_id = crate::audit::write(
        &mut tx,
        &admin.0.dev.developer_id,
        "admin",
        action,
        Some(&s.plugin_id),
        Some(s.id),
        Some("pending"),
        Some("published"),
        note.as_deref(),
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;

    // 通知：首次发布 / 新版本发布
    let vars = MailVars {
        plugin_name: s.manifest_name.clone(),
        plugin_id: s.plugin_id.clone(),
        version: s.version.clone(),
        developer: s.developer_id.clone(),
        reason: None,
        link: MailVars::link_for(&state, &s.plugin_id),
    };
    let template = if action == "version_published" {
        "version_published"
    } else {
        "review_approved"
    };
    mailer::notify_developer_with_event(
        &state,
        s.developer_row_id,
        template,
        &vars,
        Some(&s.plugin_id),
        Some(&event_id),
    )
    .await;
    Ok(Json(json!({ "ok": true, "status": "published", "eventId": event_id })))
}

async fn load_entry_for_submission(
    state: &AppState,
    submission_id: i64,
) -> ApiResult<v::CatalogEntry> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, i64, Vec<String>, String)>(
        "SELECT p.plugin_id, s.manifest_name, s.version, d.developer_id, p.repository_url,
                s.download_url, s.sha256, s.min_host_version, s.contract_version, s.permissions, s.description
         FROM submissions s
         JOIN plugins p ON p.id = s.plugin_id
         JOIN developers d ON d.id = p.developer_id
         WHERE s.id = $1",
    )
    .bind(submission_id)
    .fetch_optional(state.pool())
    .await
    .map_err(anyhow::Error::from)?
    .ok_or_else(|| AppError::NotFound("申请不存在".into()))?;
    let (
        id,
        name,
        version,
        developer_id,
        repository_url,
        download_url,
        sha256,
        min_host_version,
        contract_version,
        permissions,
        description,
    ) = row;
    Ok(v::CatalogEntry {
        id,
        name,
        version,
        developer_id,
        repository_url,
        download_url,
        sha256,
        min_host_version,
        contract_version,
        permissions,
        description,
    })
}

async fn check_catalog_quotas(
    tx: &mut PgConnection,
    developer_row_id: i64,
    is_version: bool,
) -> ApiResult<()> {
    if !is_version {
        let dev_published: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM plugins WHERE developer_id=$1 AND status='published'",
        )
        .bind(developer_row_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
        if dev_published.0 == 0 {
            let sources: (i64,) = sqlx::query_as(
                "SELECT COUNT(DISTINCT p.developer_id) FROM plugins p
                 JOIN developers d ON d.id = p.developer_id
                 WHERE p.status='published' AND d.is_banned=FALSE",
            )
            .fetch_one(&mut *tx)
            .await
            .map_err(anyhow::Error::from)?;
            if sources.0 >= v::MAX_SOURCES {
                return Err(AppError::Conflict(format!(
                    "目录源数量已达上限（{}）",
                    v::MAX_SOURCES
                )));
            }
        }
        let dev_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM plugins WHERE developer_id=$1 AND status='published'",
        )
        .bind(developer_row_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
        if dev_count.0 >= v::MAX_PLUGINS_PER_SOURCE {
            return Err(AppError::Conflict(format!(
                "该开发者已发布插件数已达单源上限（{}）",
                v::MAX_PLUGINS_PER_SOURCE
            )));
        }
    }
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM plugins p JOIN developers d ON d.id=p.developer_id
         WHERE p.status='published' AND d.is_banned=FALSE",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?;
    if total.0 >= v::MAX_TOTAL_PLUGINS {
        return Err(AppError::Conflict(format!(
            "全目录插件总数已达上限（{}）",
            v::MAX_TOTAL_PLUGINS
        )));
    }
    Ok(())
}

/// 退回：必须填写原因，原因通知开发者
async fn reject(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(submission_id): Path<i64>,
    Json(body): Json<ReviewBody>,
) -> ApiResult<Json<Value>> {
    let reason = body
        .reason
        .or(body.note)
        .and_then(|s| s.trim().is_empty().then_some(None).unwrap_or(Some(s)))
        .ok_or_else(|| AppError::BadRequest("退回必须填写原因".into()))?;
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;

    #[derive(sqlx::FromRow)]
    struct SRow {
        id: i64,
        plugin_row_id: i64,
        plugin_id: String,
        status: String,
        plugin_status: String,
        is_new_version: bool,
        developer_row_id: i64,
        version: String,
        manifest_name: String,
    }
    let s = sqlx::query_as::<_, SRow>(
        "SELECT s.id, s.plugin_id AS plugin_row_id, p.plugin_id, s.status, p.status AS plugin_status,
                s.is_new_version, p.developer_id AS developer_row_id, s.version, s.manifest_name
         FROM submissions s JOIN plugins p ON p.id = s.plugin_id
         WHERE s.id = $1 FOR UPDATE OF s",
    )
    .bind(submission_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?
    .ok_or_else(|| AppError::NotFound("申请不存在".into()))?;
    if s.status != "pending" {
        return Err(AppError::Conflict("该申请不在待审核状态".into()));
    }
    sqlx::query("UPDATE submissions SET status='rejected', reviewed_by=$1, reviewed_at=now(), review_reason=$2 WHERE id=$3")
        .bind(admin.0.dev.id)
        .bind(&reason)
        .bind(s.id)
        .execute(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
    // 新版本被退回：旧版本继续留在索引里（插件状态不变）
    // 首次发布被退回：插件 → rejected
    if !s.is_new_version && s.plugin_status == "pending" {
        sqlx::query("UPDATE plugins SET status='rejected', updated_at=now() WHERE id=$1")
            .bind(s.plugin_row_id)
            .execute(&mut *tx)
            .await
            .map_err(anyhow::Error::from)?;
    }
    let event_id = crate::audit::write(
        &mut tx,
        &admin.0.dev.developer_id,
        "admin",
        "submission_rejected",
        Some(&s.plugin_id),
        Some(s.id),
        Some("pending"),
        Some("rejected"),
        Some(&reason),
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;

    let vars = MailVars {
        plugin_name: s.manifest_name.clone(),
        plugin_id: s.plugin_id.clone(),
        version: s.version.clone(),
        developer: admin.0.dev.display_name.clone(),
        reason: Some(reason.clone()),
        link: MailVars::link_for(&state, &s.plugin_id),
    };
    mailer::notify_developer_with_event(
        &state,
        s.developer_row_id,
        "review_rejected",
        &vars,
        Some(&s.plugin_id),
        Some(&event_id),
    )
    .await;
    Ok(Json(json!({ "ok": true, "status": "rejected", "eventId": event_id })))
}

// —— 插件管理 ——
