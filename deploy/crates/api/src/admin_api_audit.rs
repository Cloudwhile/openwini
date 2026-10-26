
/// 站点近 N 天访问序列（PV/UV）
#[derive(Deserialize)]
pub struct VisitsQuery {
    pub days: Option<i64>,
}

pub fn visits_payload(days: i64, series: Vec<(chrono::NaiveDate, i64, i64)>, uv: i64) -> Value {
    let total_pv: i64 = series.iter().map(|(_, pv, _)| pv).sum();
    json!({
        "days": days,
        "series": series
            .into_iter()
            .map(|(day, pv, day_uv)| json!({ "day": day.to_string(), "pv": pv, "uv": day_uv }))
            .collect::<Vec<_>>(),
        "totalPv": total_pv,
        "totalUv": uv,
    })
}

async fn admin_visits(
    State(state): State<AppState>,
    _admin: AdminUser,
    Query(q): Query<VisitsQuery>,
) -> ApiResult<Json<Value>> {
    let days = q.days.unwrap_or(14).clamp(1, 90);
    let series = crate::visits::site_series(&state, days).await?;
    let uv = crate::visits::site_window_uv(&state, days).await?;
    Ok(Json(visits_payload(days, series, uv)))
}

/// 管理员概览统计：真实聚合（单次请求）
async fn admin_stats(State(state): State<AppState>, _admin: AdminUser) -> ApiResult<Json<Value>> {
    let (pending_queue,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM submissions WHERE status='pending'")
            .fetch_one(state.pool())
            .await
            .map_err(anyhow::Error::from)?;
    let status_rows: Vec<(String, i64)> =
        sqlx::query_as("SELECT status, COUNT(*) FROM plugins GROUP BY status")
            .fetch_all(state.pool())
            .await
            .map_err(anyhow::Error::from)?;
    let mut by_status = std::collections::BTreeMap::new();
    let mut total_plugins = 0i64;
    for (s, c) in status_rows {
        total_plugins += c;
        by_status.insert(s, c);
    }
    let (dev_total,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM developers WHERE NOT is_system")
            .fetch_one(state.pool())
            .await
            .map_err(anyhow::Error::from)?;
    let (dev_banned,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM developers WHERE is_banned")
        .fetch_one(state.pool())
        .await
        .map_err(anyhow::Error::from)?;
    let (audit_total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_logs")
        .fetch_one(state.pool())
        .await
        .map_err(anyhow::Error::from)?;
    let (mail_failed,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM email_logs WHERE status='failed'")
            .fetch_one(state.pool())
            .await
            .map_err(anyhow::Error::from)?;
    // 宿主 App 接口状态：目录开关 + 目录源/可分发插件聚合
    let catalog_enabled = state
        .site_config("public_catalog_enabled")
        .await
        .unwrap_or_else(|| "true".into())
        == "true";
    let (sources,): (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT p.developer_id) FROM plugins p
         JOIN developers d ON d.id = p.developer_id
         WHERE p.status='published' AND d.is_banned=FALSE",
    )
    .fetch_one(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    let count = |k: &str| by_status.get(k).copied().unwrap_or(0);
    Ok(Json(json!({
        "pendingQueue": pending_queue,
        "plugins": {
            "total": total_plugins,
            "published": count("published"),
            "pending": count("pending"),
            "draft": count("draft"),
            "rejected": count("rejected"),
            "stopped": count("stopped"),
            "paused": count("paused"),
            "unlisted": count("unlisted"),
            "cancelled": count("cancelled"),
        },
        "developers": { "total": dev_total, "banned": dev_banned },
        "auditTotal": audit_total,
        "mailFailed": mail_failed,
        "hostApi": {
            "catalogEnabled": catalog_enabled,
            "sources": sources,
            "publishedPlugins": count("published"),
        },
    })))
}

#[derive(Deserialize)]
struct AuditQuery {
    plugin: Option<String>,
    actor: Option<String>,
    action: Option<String>,
    from: Option<String>,
    to: Option<String>,
    page: Option<i64>,
}

/// 解析 RFC3339 时刻参数（空/缺省 → None）。前端把本地日期换算为 UTC 时刻，
/// 使"今天"等边界跟随用户时区而非数据库时区。
fn parse_instant_param(
    v: &Option<String>,
    label: &str,
) -> ApiResult<Option<chrono::DateTime<chrono::Utc>>> {
    match v.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(s) => chrono::DateTime::parse_from_rfc3339(s)
            .map(|d| Some(d.with_timezone(&chrono::Utc)))
            .map_err(|_| AppError::BadRequest(format!("{label}格式不合法"))),
    }
}

async fn audit_logs(
    State(state): State<AppState>,
    _admin: AdminUser,
    Query(q): Query<AuditQuery>,
) -> ApiResult<Json<Value>> {
    let page_size: i64 = 50;
    let page = q.page.unwrap_or(1).max(1);
    let plugin = q.plugin.clone().unwrap_or_default();
    let actor = q.actor.clone().unwrap_or_default();
    let action = q.action.clone().unwrap_or_default();
    let from = parse_instant_param(&q.from, "开始日期")?;
    let to = parse_instant_param(&q.to, "结束日期")?;
    let rows: Vec<(String, String, String, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>, chrono::DateTime<chrono::Utc>, String)> = sqlx::query_as(
        "SELECT actor, actor_role, action, plugin_id, submission_id, from_status, to_status, reason, created_at, event_id
         FROM audit_logs
         WHERE ($1 = '' OR plugin_id ILIKE '%' || $1 || '%')
           AND ($2 = '' OR actor ILIKE '%' || $2 || '%')
           AND ($3 = '' OR action ILIKE '%' || $3 || '%')
           AND ($6::timestamptz IS NULL OR created_at >= $6)
           AND ($7::timestamptz IS NULL OR created_at < $7)
         ORDER BY created_at DESC LIMIT $4 OFFSET $5",
    )
    .bind(&plugin)
    .bind(&actor)
    .bind(&action)
    .bind(page_size)
    .bind((page - 1) * page_size)
    .bind(from)
    .bind(to)
    .fetch_all(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM audit_logs
         WHERE ($1 = '' OR plugin_id ILIKE '%' || $1 || '%')
           AND ($2 = '' OR actor ILIKE '%' || $2 || '%')
           AND ($3 = '' OR action ILIKE '%' || $3 || '%')
           AND ($4::timestamptz IS NULL OR created_at >= $4)
           AND ($5::timestamptz IS NULL OR created_at < $5)",
    )
    .bind(&plugin)
    .bind(&actor)
    .bind(&action)
    .bind(from)
    .bind(to)
    .fetch_one(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    let items: Vec<Value> = rows
        .into_iter()
        .map(
            |(actor, role, action, plugin_id, submission_id, from, to, reason, created, event_id)| {
                json!({
                    "eventId": event_id,
                    "actor": actor, "actorRole": role, "action": action, "pluginId": plugin_id,
                    "submissionId": submission_id, "fromStatus": from, "toStatus": to,
                    "reason": reason, "createdAt": created,
                })
            },
        )
        .collect();
    Ok(Json(
        json!({ "items": items, "total": total.0, "page": page, "pageSize": page_size }),
    ))
}

// —— 站点 / SMTP / AI 配置 ——
