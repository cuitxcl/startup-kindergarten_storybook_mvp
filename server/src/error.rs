use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    field: Option<String>,
    details: Option<Value>,
}

impl ApiError {
    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized", "请先登录")
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", message)
    }

    pub fn not_found(resource: &'static str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("{resource} 不存在"),
        )
    }

    pub fn validation(field: &'static str, message: impl Into<String>) -> Self {
        Self::validation_with_code("validation_error", field, message)
    }

    pub fn validation_with_code(
        code: &'static str,
        field: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code,
            message: message.into(),
            field: Some(field.to_string()),
            details: None,
        }
    }

    pub fn state_conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "state_conflict", message)
    }

    pub fn state_conflict_with_code(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message)
    }

    pub fn state_conflict_with_code_and_details(
        code: &'static str,
        message: impl Into<String>,
        details: Value,
    ) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code,
            message: message.into(),
            field: None,
            details: Some(details),
        }
    }

    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            field: None,
            details: None,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorEnvelope {
                error: ErrorBody {
                    code: self.code,
                    message: self.message,
                    field: self.field,
                    details: self.details,
                },
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::ApiError;
    use axum::{body::to_bytes, response::IntoResponse};

    #[tokio::test]
    async fn custom_validation_code_is_serialized_in_error_response() {
        let response = ApiError::validation_with_code(
            "unsupported_file_type",
            "file",
            "照片只支持 JPEG、PNG 或 WebP 格式",
        )
        .into_response();

        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should serialize");
        let body: serde_json::Value = serde_json::from_slice(&bytes).expect("body should be json");
        assert_eq!(body["error"]["code"], "unsupported_file_type");
        assert_eq!(body["error"]["field"], "file");
    }

    #[tokio::test]
    async fn state_conflict_details_are_serialized_when_present() {
        let response = ApiError::state_conflict_with_code_and_details(
            "visual_reference_required",
            "先处理照片",
            serde_json::json!({
                "blocking_asset_reference_ids": ["asset-ref-1"],
                "next_action": "confirm_visual_reference"
            }),
        )
        .into_response();

        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should serialize");
        let body: serde_json::Value = serde_json::from_slice(&bytes).expect("body should be json");
        assert_eq!(body["error"]["code"], "visual_reference_required");
        assert_eq!(
            body["error"]["details"]["next_action"],
            "confirm_visual_reference"
        );
        assert_eq!(
            body["error"]["details"]["blocking_asset_reference_ids"][0],
            "asset-ref-1"
        );
    }
}
