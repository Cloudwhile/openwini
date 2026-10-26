use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    /// 带稳定错误码的请求错误：响应体为 { error（中文兜底）, code, params }，
    /// 前端按 code + 当前语言渲染（apiErrors 词典），未知 code 回退 error 原文。
    #[error("{message}")]
    BadRequestCoded {
        message: String,
        code: String,
        params: serde_json::Value,
    },
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("请求过于频繁，请稍后再试")]
    RateLimited,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match &self {
            AppError::BadRequest(m) => {
                (StatusCode::BAD_REQUEST, Json(json!({ "error": m }))).into_response()
            }
            AppError::BadRequestCoded {
                message,
                code,
                params,
            } => (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": message, "code": code, "params": params })),
            )
                .into_response(),
            AppError::Unauthorized(m) => {
                (StatusCode::UNAUTHORIZED, Json(json!({ "error": m }))).into_response()
            }
            AppError::Forbidden(m) => {
                (StatusCode::FORBIDDEN, Json(json!({ "error": m }))).into_response()
            }
            AppError::NotFound(m) => {
                (StatusCode::NOT_FOUND, Json(json!({ "error": m }))).into_response()
            }
            AppError::Conflict(m) => {
                (StatusCode::CONFLICT, Json(json!({ "error": m }))).into_response()
            }
            AppError::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({ "error": self.to_string() })),
            )
                .into_response(),
            AppError::Internal(e) => {
                tracing::error!("internal error: {e:#}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "服务器内部错误" })),
                )
                    .into_response()
            }
        }
    }
}

pub type ApiResult<T> = Result<T, AppError>;
