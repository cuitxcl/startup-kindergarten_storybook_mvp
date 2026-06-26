pub mod children;
pub mod content;
pub mod organization;
pub mod storybooks;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

pub type SharedState = Arc<RwLock<AppState>>;

#[derive(Clone, Debug)]
pub struct AppState {
    pub children: children::ChildrenStore,
    pub content: content::ContentStore,
    pub organization: organization::OrganizationStore,
    pub storybooks: storybooks::StorybookStore,
}

impl AppState {
    pub fn demo() -> Self {
        let organization = organization::OrganizationStore::demo();
        Self {
            children: children::ChildrenStore::demo(&organization),
            content: content::ContentStore::demo(),
            organization,
            storybooks: storybooks::StorybookStore::demo(),
        }
    }
}

pub fn router(state: SharedState) -> axum::Router {
    axum::Router::new()
        .nest("/api", organization::router())
        .nest("/api", children::router())
        .nest("/api", content::router())
        .nest("/api", storybooks::router())
        .with_state(state)
}

#[derive(Clone, Debug, Serialize)]
pub struct ErrorEnvelope {
    pub error: ApiErrorBody,
}

#[derive(Clone, Debug, Serialize)]
pub struct ApiErrorBody {
    pub code: &'static str,
    pub message: String,
    pub details: Vec<ApiErrorDetail>,
    pub request_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ApiErrorDetail {
    pub field: String,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub details: Vec<ApiErrorDetail>,
}

impl ApiError {
    pub fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "VALIDATION_ERROR",
            message: "请求参数校验失败".to_string(),
            details: vec![ApiErrorDetail {
                field: field.into(),
                message: message.into(),
            }],
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "FORBIDDEN",
            message: message.into(),
            details: vec![],
        }
    }

    pub fn not_found(resource: &'static str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "NOT_FOUND",
            message: format!("{resource} 不存在"),
            details: vec![],
        }
    }

    pub fn state_conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "STATE_CONFLICT",
            message: message.into(),
            details: vec![],
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorEnvelope {
            error: ApiErrorBody {
                code: self.code,
                message: self.message,
                details: self.details,
                request_id: "local-dev".to_string(),
            },
        };
        (self.status, Json(body)).into_response()
    }
}

pub fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

pub fn demo_uuid(seed: u128) -> Uuid {
    Uuid::from_u128(seed)
}
