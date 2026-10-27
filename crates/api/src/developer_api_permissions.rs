#[derive(Deserialize)]
struct NewPermission {
    name: String,
    risk: String,
    #[serde(default)]
    description: String,
}

async fn create_permission(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<NewPermission>,
) -> ApiResult<Json<Value>> {
    ensure_not_banned(&user)?;
    let name = body.name.trim();
    let description = body.description.trim();
    if !crate::validation::valid_permission_name(name)
        || !matches!(body.risk.as_str(), "low" | "medium" | "high")
        || description.encode_utf16().count() > 200
    {
        return Err(AppError::BadRequest("权限名称、风险或用途说明无效".into()));
    }
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    // The primary key arbitrates concurrent additions; existing metadata is never overwritten.
    let inserted: Option<String> = sqlx::query_scalar(
        "INSERT INTO known_permissions (name, risk, description) VALUES ($1, $2, $3)
         ON CONFLICT (name) DO NOTHING RETURNING name",
    )
    .bind(name)
    .bind(&body.risk)
    .bind(description)
    .fetch_optional(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?;
    if inserted.is_none() {
        return Err(AppError::Conflict("该权限名已存在".into()));
    }
    crate::audit::write(
        &mut tx,
        &user.dev.developer_id,
        "developer",
        "permission_created",
        None,
        None,
        None,
        None,
        Some(name),
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(Json(
        json!({ "name": name, "risk": body.risk, "description": description }),
    ))
}
