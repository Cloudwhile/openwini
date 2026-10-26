
#[derive(Deserialize)]
struct DevQuery {
    q: Option<String>,
    page: Option<i64>,
}

async fn list_developers(
    State(state): State<AppState>,
    _admin: AdminUser,
    Query(q): Query<DevQuery>,
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
    let rows: Vec<(
        String,
        String,
        String,
        bool,
        bool,
        bool,
        i64,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT developer_id, COALESCE(NULLIF(display_name,''), developer_id), github_login,
                is_admin, is_banned, is_system,
                (SELECT COUNT(*) FROM plugins p WHERE p.developer_id = developers.id),
                created_at
         FROM developers
         WHERE ($1 = '' OR developer_id ILIKE $1 OR github_login ILIKE $1 OR display_name ILIKE $1)
         ORDER BY created_at ASC LIMIT $2 OFFSET $3",
    )
    .bind(&pattern)
    .bind(page_size)
    .bind((page - 1) * page_size)
    .fetch_all(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM developers
         WHERE ($1 = '' OR developer_id ILIKE $1 OR github_login ILIKE $1 OR display_name ILIKE $1)",
    )
    .bind(&pattern)
    .fetch_one(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    let items: Vec<Value> = rows
        .into_iter()
        .map(
            |(id, name, login, is_admin, banned, is_system, plugins, created)| {
                json!({
                    "developerId": id, "displayName": name, "githubLogin": login,
                    "isAdmin": is_admin, "isBanned": banned, "isSystem": is_system,
                    "pluginCount": plugins, "createdAt": created,
                })
            },
        )
        .collect();
    Ok(Json(
        json!({ "items": items, "total": total.0, "page": page, "pageSize": page_size }),
    ))
}

/// 封禁：名下全部插件同步从索引移除（→ stopped），解封不自动恢复
async fn ban_developer(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(developer_id): Path<String>,
    Json(body): Json<ReasonBody>,
) -> ApiResult<Json<Value>> {
    let reason = require_reason(&body, "封禁操作")?;
    if developer_id == v::OFFICIAL_DEVELOPER_ID {
        return Err(AppError::Forbidden("系统账号不可封禁".into()));
    }
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    let row: (i64, bool) =
        sqlx::query_as("SELECT id, is_admin FROM developers WHERE developer_id=$1 FOR UPDATE")
            .bind(&developer_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(anyhow::Error::from)?
            .ok_or_else(|| AppError::NotFound("开发者不存在".into()))?;
    if row.0 == admin.0.dev.id {
        return Err(AppError::Forbidden("不能封禁自己".into()));
    }
    if row.1 {
        // 管理员必须先被撤销管理员身份才能封禁：避免封禁链误伤导致管理员集体不可用
        return Err(AppError::Forbidden(
            "该用户是管理员，请先撤销其管理员权限，再执行封禁".into(),
        ));
    }
    sqlx::query("UPDATE developers SET is_banned=TRUE, updated_at=now() WHERE id=$1")
        .bind(row.0)
        .execute(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
    let stop_reason = format!("因开发者账号被封禁而下架（{reason}）");
    let plugins: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, plugin_id FROM plugins WHERE developer_id=$1 AND status='published'",
    )
    .bind(row.0)
    .fetch_all(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?;
    for (pid_row, pid) in &plugins {
        sqlx::query("UPDATE plugins SET status='stopped', stop_reason=$1, updates_halted=FALSE, updates_halted_reason=NULL, updated_at=now() WHERE id=$2")
            .bind(&stop_reason)
            .bind(pid_row)
            .execute(&mut *tx)
            .await
            .map_err(anyhow::Error::from)?;
        crate::audit::write(
            &mut tx,
            &admin.0.dev.developer_id,
            "admin",
            "plugin_stopped",
            Some(pid),
            None,
            Some("published"),
            Some("stopped"),
            Some(&stop_reason),
        )
        .await?;
    }
    // 待审申请一并退回
    let pending: Vec<(i64, String)> = sqlx::query_as(
        "SELECT s.id, p.plugin_id FROM submissions s JOIN plugins p ON p.id=s.plugin_id
         WHERE s.status='pending' AND p.developer_id=$1",
    )
    .bind(row.0)
    .fetch_all(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?;
    for (sid, pid) in &pending {
        let why = format!("开发者被封禁：{reason}");
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
            Some(pid),
            Some(*sid),
            Some("pending"),
            Some("rejected"),
            Some(&why),
        )
        .await?;
    }
    crate::audit::write(
        &mut tx,
        &admin.0.dev.developer_id,
        "admin",
        "developer_banned",
        None,
        None,
        None,
        None,
        Some(&reason),
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "ok": true, "stoppedPlugins": plugins.len() })))
}

async fn unban_developer(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(developer_id): Path<String>,
    Json(body): Json<ReasonBody>,
) -> ApiResult<Json<Value>> {
    let note = body
        .note
        .clone()
        .or(body.reason.clone())
        .unwrap_or_default();
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    let row: (i64,) = sqlx::query_as("SELECT id FROM developers WHERE developer_id=$1 FOR UPDATE")
        .bind(&developer_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?
        .ok_or_else(|| AppError::NotFound("开发者不存在".into()))?;
    sqlx::query("UPDATE developers SET is_banned=FALSE, updated_at=now() WHERE id=$1")
        .bind(row.0)
        .execute(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
    crate::audit::write(
        &mut tx,
        &admin.0.dev.developer_id,
        "admin",
        "developer_unbanned",
        None,
        None,
        None,
        None,
        if note.trim().is_empty() {
            None
        } else {
            Some(note.trim())
        },
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct SetAdminBody {
    #[serde(rename = "isAdmin")]
    is_admin: bool,
}

async fn set_admin(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(developer_id): Path<String>,
    Json(body): Json<SetAdminBody>,
) -> ApiResult<Json<Value>> {
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    let row: (i64,) =
        sqlx::query_as("SELECT id FROM developers WHERE developer_id=$1 AND NOT is_system")
            .bind(&developer_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(anyhow::Error::from)?
            .ok_or_else(|| AppError::NotFound("开发者不存在".into()))?;
    if row.0 == admin.0.dev.id && !body.is_admin {
        return Err(AppError::Forbidden("不能收回自己的管理员权限".into()));
    }
    sqlx::query("UPDATE developers SET is_admin=$1, updated_at=now() WHERE id=$2")
        .bind(body.is_admin)
        .bind(row.0)
        .execute(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
    crate::audit::write(
        &mut tx,
        &admin.0.dev.developer_id,
        "admin",
        if body.is_admin {
            "admin_granted"
        } else {
            "admin_revoked"
        },
        None,
        None,
        None,
        None,
        Some(&developer_id),
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "ok": true })))
}

// —— 审计日志 ——
