use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

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

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "UNAUTHORIZED",
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
                request_id: "request-id-unavailable".to_string(),
            },
        };
        (self.status, Json(body)).into_response()
    }
}
