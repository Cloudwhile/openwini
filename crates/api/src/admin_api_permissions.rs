pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/queue", get(queue))
        .route("/admin/queue/{submission_id}", get(queue_detail))
        .route("/admin/queue/{submission_id}/approve", post(approve))
        .route("/admin/queue/{submission_id}/reject", post(reject))
        .route("/admin/plugins", get(admin_plugins))
        .route("/admin/plugins/{plugin_id}/stop", post(stop_plugin))
        .route(
            "/admin/plugins/{plugin_id}/unpublish",
            post(unpublish_plugin),
        )
        .route("/admin/plugins/{plugin_id}/halt", post(halt_updates))
        .route("/admin/plugins/{plugin_id}/resume", post(resume_plugin))
        .route("/admin/plugins/{plugin_id}/links", put(update_links))
        .route("/admin/official/plugins", post(official_create))
        .route(
            "/admin/official/plugins/{plugin_id}/versions",
            post(official_new_version),
        )
        .route("/admin/developers", get(list_developers))
        .route("/admin/developers/{developer_id}/ban", post(ban_developer))
        .route(
            "/admin/developers/{developer_id}/unban",
            post(unban_developer),
        )
        .route("/admin/developers/{developer_id}/admin", put(set_admin))
        .route("/admin/audit", get(audit_logs))
        .route("/admin/stats", get(admin_stats))
        .route("/admin/visits", get(admin_visits))
        .route(
            "/admin/config/site",
            get(get_site_config).put(put_site_config),
        )
        .route(
            "/admin/config/smtp",
            get(get_smtp_config).put(put_smtp_config),
        )
        .route("/admin/config/smtp/test", post(smtp_test))
        .route("/admin/config/smtp/logs", get(smtp_logs))
        .route("/admin/config/ai", get(get_ai_config).put(put_ai_config))
        .route(
            "/admin/permissions",
            get(list_permissions).put(upsert_permission),
        )
        .route(
            "/admin/permissions/{name}",
            axum::routing::delete(delete_permission),
        )
}

// —— 权限标签管理（宿主权限表） ——

/// 使用中判定：被当前发布版本或待审申请声明的标签不可删除
/// （否则出索引校验会静默剔除插件、待审申请也无法通过）
async fn permission_in_use(tx: &mut PgConnection, name: &str) -> ApiResult<i64> {
    let (used,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM submissions s
         WHERE $1 = ANY(s.permissions)
           AND (s.status = 'pending' OR s.id = (SELECT current_submission_id FROM plugins WHERE id = s.plugin_id))",
    )
    .bind(name)
    .fetch_one(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?;
    Ok(used)
}

async fn list_permissions(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> ApiResult<Json<Value>> {
    let rows: Vec<(String, String, String, i64)> = sqlx::query_as(
        "SELECT k.name, k.risk, k.description,
                (SELECT COUNT(*) FROM submissions s
                  WHERE k.name = ANY(s.permissions)
                    AND (s.status = 'pending' OR s.id = (SELECT current_submission_id FROM plugins WHERE id = s.plugin_id)))
         FROM known_permissions k
         ORDER BY CASE k.risk WHEN 'high' THEN 0 WHEN 'medium' THEN 1 ELSE 2 END, k.name",
    )
    .fetch_all(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    Ok(Json(json!({
        "permissions": rows
            .into_iter()
            .map(|(name, risk, description, used_by)| json!({
                "name": name, "risk": risk, "description": description, "usedBy": used_by,
            }))
            .collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
struct PermissionBody {
    name: String,
    risk: String,
    #[serde(default)]
    description: String,
}

/// 新增或更新权限标签（name 为清单声明用的主键，创建后不可改）
async fn upsert_permission(
    State(state): State<AppState>,
    admin: AdminUser,
    Json(body): Json<PermissionBody>,
) -> ApiResult<Json<Value>> {
    let name = body.name.trim().to_string();
    if !v::valid_permission_name(&name) {
        return Err(AppError::BadRequest(
            "权限名非法：以小写字母开头，仅含小写字母/数字/下划线，长度 ≤80".into(),
        ));
    }
    if !matches!(body.risk.as_str(), "low" | "medium" | "high") {
        return Err(AppError::BadRequest(
            "风险等级必须是 low / medium / high".into(),
        ));
    }
    let description = body.description.trim().to_string();
    if description.chars().count() > 200 {
        return Err(AppError::BadRequest("用途说明不能超过 200 字符".into()));
    }

    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    let existing: Option<String> =
        sqlx::query_scalar("SELECT risk FROM known_permissions WHERE name = $1 FOR UPDATE")
            .bind(&name)
            .fetch_optional(&mut *tx)
            .await
            .map_err(anyhow::Error::from)?;
    let action = match &existing {
        // 已存在的标签：仅允许改风险/说明；使用中标签降低风险等级需要谨慎，照常记录审计
        Some(old_risk) => {
            sqlx::query("UPDATE known_permissions SET risk=$1, description=$2 WHERE name=$3")
                .bind(&body.risk)
                .bind(&description)
                .bind(&name)
                .execute(&mut *tx)
                .await
                .map_err(anyhow::Error::from)?;
            let reason = format!(
                "{name}：风险 {old_risk} → {}，说明：{}",
                body.risk,
                if description.is_empty() {
                    "（无）"
                } else {
                    &description
                }
            );
            crate::audit::write(
                &mut tx,
                &admin.0.dev.developer_id,
                "admin",
                "permission_updated",
                None,
                None,
                None,
                None,
                Some(&reason),
            )
            .await?;
            "updated"
        }
        None => {
            sqlx::query(
                "INSERT INTO known_permissions (name, risk, description) VALUES ($1,$2,$3)",
            )
            .bind(&name)
            .bind(&body.risk)
            .bind(&description)
            .execute(&mut *tx)
            .await
            .map_err(anyhow::Error::from)?;
            let reason = format!(
                "{name}：风险 {}，说明：{}",
                body.risk,
                if description.is_empty() {
                    "（无）"
                } else {
                    &description
                }
            );
            crate::audit::write(
                &mut tx,
                &admin.0.dev.developer_id,
                "admin",
                "permission_created",
                None,
                None,
                None,
                None,
                Some(&reason),
            )
            .await?;
            "created"
        }
    };
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "ok": true, "action": action })))
}

/// 删除权限标签：被当前发布版本或待审申请声明时拒绝
async fn delete_permission(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(name): Path<String>,
) -> ApiResult<Json<Value>> {
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    let existing: String =
        sqlx::query_scalar("SELECT risk FROM known_permissions WHERE name = $1 FOR UPDATE")
            .bind(&name)
            .fetch_optional(&mut *tx)
            .await
            .map_err(anyhow::Error::from)?
            .ok_or_else(|| AppError::NotFound("权限标签不存在".into()))?;
    let used_by = permission_in_use(&mut tx, &name).await?;
    if used_by > 0 {
        return Err(AppError::Conflict(format!(
            "该权限标签正被 {used_by} 个插件（当前版本或待审申请）声明，不能删除"
        )));
    }
    sqlx::query("DELETE FROM known_permissions WHERE name = $1")
        .bind(&name)
        .execute(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
    crate::audit::write(
        &mut tx,
        &admin.0.dev.developer_id,
        "admin",
        "permission_deleted",
        None,
        None,
        Some(&existing),
        None,
        Some(&name),
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "ok": true })))
}
