use axum::extract::{Path, Query, State};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{FromRow, Row};

use crate::auth::CurrentUser;
use crate::error::{ApiResult, AppError};
use crate::mailer::{self, MailVars};
use crate::state::AppState;
use crate::submission as sub;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/dev/profile", put(update_profile))
        .route("/dev/permissions", post(create_permission))
        .route("/dev/github/disconnect", post(disconnect_github))
        .route("/dev/repos", get(list_repos))
        .route("/dev/repos/validate", post(validate_repo))
        .route("/dev/manifest-preview", post(manifest_preview))
        .route("/dev/plugins", get(list_plugins).post(create_plugin))
        .route(
            "/dev/plugins/{plugin_id}",
            get(plugin_detail).put(edit_plugin),
        )
        .route("/dev/plugins/{plugin_id}/submit", post(submit_plugin))
        .route("/dev/plugins/{plugin_id}/cancel", post(cancel_plugin))
        .route("/dev/plugins/{plugin_id}/pause", post(pause_plugin))
        .route("/dev/plugins/{plugin_id}/resume", post(resume_plugin_dev))
        .route("/dev/plugins/{plugin_id}/unlist", post(unlist_plugin))
        .route(
            "/dev/plugins/{plugin_id}/versions",
            post(submit_new_version),
        )
        .route("/dev/stats", get(dev_stats))
        .route("/dev/visits", get(dev_visits))
}

include!("developer_api_core.rs");
include!("developer_api_submissions.rs");
include!("developer_api_permissions.rs");
