use axum::extract::{Path, Query, State};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{PgConnection, Row};

use crate::auth::AdminUser;
use crate::error::{ApiResult, AppError};
use crate::mailer::{self, MailVars};
use crate::state::AppState;
use crate::submission as sub;
use crate::validation as v;

include!("admin_api_permissions.rs");
include!("admin_api_queue.rs");
include!("admin_api_plugins.rs");
include!("admin_api_official.rs");
include!("admin_api_developers.rs");
include!("admin_api_audit.rs");
include!("admin_api_config.rs");
