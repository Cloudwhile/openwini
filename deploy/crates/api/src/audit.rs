use sqlx::PgConnection;

/// 在事务内写审计日志（与状态变更同一事务，任一步失败整体回滚）
#[allow(clippy::too_many_arguments)]
pub async fn write(
    tx: &mut PgConnection,
    actor: &str,
    actor_role: &str,
    action: &str,
    plugin_id: Option<&str>,
    submission_id: Option<i64>,
    from_status: Option<&str>,
    to_status: Option<&str>,
    reason: Option<&str>,
) -> anyhow::Result<String> {
    let event_id = format!("evt_{}", crate::crypto::random_hex(16));
    sqlx::query(
        "INSERT INTO audit_logs (event_id, actor, actor_role, action, plugin_id, submission_id, from_status, to_status, reason)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
    )
    .bind(&event_id)
    .bind(actor)
    .bind(actor_role)
    .bind(action)
    .bind(plugin_id)
    .bind(submission_id)
    .bind(from_status)
    .bind(to_status)
    .bind(reason)
    .execute(&mut *tx)
    .await?;
    Ok(event_id)
}
