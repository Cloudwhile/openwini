/// 开发者名下插件详情页近 N 天访客序列
async fn dev_visits(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<crate::admin_api::VisitsQuery>,
) -> ApiResult<Json<Value>> {
    let days = q.days.unwrap_or(14).clamp(1, 90);
    let series = crate::visits::developer_series(&state, days, user.dev.id).await?;
    let uv = crate::visits::developer_window_uv(&state, days, user.dev.id).await?;
    Ok(Json(crate::admin_api::visits_payload(days, series, uv)))
}

fn ensure_not_banned(user: &CurrentUser) -> ApiResult<()> {
    if user.dev.is_banned {
        return Err(AppError::Forbidden(
            "你的开发者账号已被封禁，无法执行该操作".into(),
        ));
    }
    Ok(())
}

fn ensure_not_system(user: &CurrentUser) -> ApiResult<()> {
    if user.dev.is_system {
        return Err(AppError::Forbidden("系统账号不支持该操作".into()));
    }
    Ok(())
}

/// 把 anyhow 错误转为 AppError：带 CodedError 的按错误码透传（前端本地化），其余原样中文兜底
fn coded_bad_request(e: anyhow::Error) -> AppError {
    if let Some(c) = e.downcast_ref::<crate::github::CodedError>() {
        return AppError::BadRequestCoded {
            message: c.message.clone(),
            code: c.code.to_string(),
            params: c.params.clone(),
        };
    }
    AppError::BadRequest(format!("{e}"))
}

async fn update_profile(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    ensure_not_banned(&user)?;
    let name = body
        .get("displayName")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if name.is_empty() || name.chars().count() > 200 {
        return Err(AppError::BadRequest(
            "显示名称必须非空且不超过 200 字符".into(),
        ));
    }
    let preferred_language = body
        .get("preferredLanguage")
        .and_then(|v| v.as_str())
        .unwrap_or(&user.dev.preferred_language)
        .trim()
        .to_ascii_lowercase();
    if !matches!(preferred_language.as_str(), "zh" | "en") {
        return Err(AppError::BadRequest(
            "语言偏好只能是 zh 或 en".into(),
        ));
    }
    sqlx::query(
        "UPDATE developers SET display_name=$1, preferred_language=$2, updated_at=now() WHERE id=$3",
    )
        .bind(&name)
        .bind(&preferred_language)
        .bind(user.dev.id)
        .execute(state.pool())
        .await
        .map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "ok": true })))
}

async fn disconnect_github(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<StatusCode204> {
    ensure_not_banned(&user)?;
    sqlx::query("UPDATE developers SET github_token_encrypted=NULL, updated_at=now() WHERE id=$1")
        .bind(user.dev.id)
        .execute(state.pool())
        .await
        .map_err(anyhow::Error::from)?;
    // 已发布插件不受影响；只是无法提交新申请，直到重新授权
    Ok(StatusCode204)
}

struct StatusCode204;
impl axum::response::IntoResponse for StatusCode204 {
    fn into_response(self) -> axum::response::Response {
        axum::http::StatusCode::NO_CONTENT.into_response()
    }
}
