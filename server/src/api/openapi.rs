use axum::{
    Json, Router,
    response::{Html, Redirect},
    routing::get,
};
use serde_json::{Value, json};

use super::SharedState;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/api-docs/openapi.json", get(openapi_json))
        .route("/swagger-ui", get(swagger_ui_redirect))
        .route("/swagger-ui/", get(swagger_ui))
}

async fn openapi_json() -> Json<Value> {
    Json(openapi_document())
}

async fn swagger_ui_redirect() -> Redirect {
    Redirect::permanent("/swagger-ui/")
}

async fn swagger_ui() -> Html<&'static str> {
    Html(
        r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Kindergarten Storybook API</title>
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui.css" />
  <style>
    body { margin: 0; background: #ffffff; }
    .swagger-ui .topbar { display: none; }
  </style>
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
  <script>
    window.ui = SwaggerUIBundle({
      url: "/api-docs/openapi.json",
      dom_id: "#swagger-ui",
      deepLinking: true,
      persistAuthorization: true,
      displayRequestDuration: true
    });
  </script>
</body>
</html>"##,
    )
}

fn openapi_document() -> Value {
    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Kindergarten Storybook Server API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "OpenAPI documentation for the kindergarten storybook MVP server."
        },
        "servers": [
            { "url": "/", "description": "Current server" }
        ],
        "tags": [
            { "name": "Auth" },
            { "name": "Organization" },
            { "name": "Dashboard" },
            { "name": "Children" },
            { "name": "Content" },
            { "name": "Storybooks" },
            { "name": "Visuals" },
            { "name": "Images" },
            { "name": "Delivery" },
            { "name": "Admin" }
        ],
        "paths": paths(),
        "components": components()
    })
}

fn paths() -> Value {
    let mut paths = serde_json::Map::new();

    add(&mut paths, "/api/auth/register/send-code", "post", op("Send registration email code", "Auth", false, Some("SendRegistrationCodeRequest"), "EmailVerificationResponse"));
    add(&mut paths, "/api/auth/register", "post", op("Register teacher account", "Auth", false, Some("RegisterRequest"), "AuthResponse"));
    add(&mut paths, "/api/auth/login", "post", op("Login", "Auth", false, Some("LoginRequest"), "AuthResponse"));
    add(&mut paths, "/api/auth/me", "get", op("Get current session", "Auth", true, None, "CurrentSessionResponse"));
    add(&mut paths, "/api/auth/refresh", "post", op("Refresh current session", "Auth", true, None, "AuthResponse"));
    add(&mut paths, "/api/auth/logout", "post", op("Logout", "Auth", true, None, "LogoutResponse"));

    add(&mut paths, "/api/schools/current", "get", op("Get current school", "Organization", true, None, "SchoolRecord"));
    add(&mut paths, "/api/schools/current", "patch", op("Update current school", "Organization", true, Some("UpdateSchoolRequest"), "SchoolRecord"));
    add(&mut paths, "/api/classrooms", "get", op("List classrooms", "Organization", true, None, "ClassroomListResponse"));
    add(&mut paths, "/api/classrooms", "post", op("Create classroom", "Organization", true, Some("CreateClassroomRequest"), "ClassroomRecord"));
    add(&mut paths, "/api/classrooms/{classroom_id}", "patch", op("Update classroom", "Organization", true, Some("UpdateClassroomRequest"), "ClassroomRecord"));
    add(&mut paths, "/api/teachers/me", "get", op("Get current teacher", "Organization", true, None, "CurrentTeacherResponse"));
    add(&mut paths, "/api/teachers", "get", op("List teachers", "Organization", true, None, "TeacherListResponse"));

    add(&mut paths, "/api/dashboard/teacher", "get", op("Get teacher dashboard", "Dashboard", true, None, "TeacherDashboardResponse"));
    add(&mut paths, "/api/content-items", "get", op("List content items", "Dashboard", true, None, "ContentItemListResponse"));
    add(&mut paths, "/api/content-items/{storybook_id}/activity", "get", op("List content item activity", "Dashboard", true, None, "ActivityListResponse"));

    add(&mut paths, "/api/children", "get", op("List children", "Children", true, None, "ChildListResponse"));
    add(&mut paths, "/api/children", "post", op("Create child", "Children", true, Some("CreateChildRequest"), "ChildRecord"));
    add(&mut paths, "/api/children/{child_id}", "get", op("Get child", "Children", true, None, "ChildDetailResponse"));
    add(&mut paths, "/api/children/{child_id}", "patch", op("Update child", "Children", true, Some("UpdateChildRequest"), "ChildRecord"));
    add(&mut paths, "/api/children/{child_id}/photos", "post", op("Add child photo", "Children", true, Some("AddChildPhotoRequest"), "ChildPhotoRecord"));
    add(&mut paths, "/api/children/{child_id}/photos/{photo_id}", "patch", op("Update child photo", "Children", true, Some("UpdateChildPhotoRequest"), "ChildPhotoRecord"));
    add(&mut paths, "/api/parent-intakes", "post", op("Create parent intake", "Children", false, Some("CreateParentIntakeRequest"), "ParentIntakeRecord"));
    add(&mut paths, "/api/parent-intakes", "get", op("List parent intakes", "Children", true, None, "ParentIntakeListResponse"));
    add(&mut paths, "/api/parent-intake-links", "post", op("Create parent intake link", "Children", true, Some("CreateParentIntakeLinkRequest"), "ParentIntakeLinkRecord"));
    add(&mut paths, "/api/parent-intakes/{intake_id}/accept", "post", op("Accept parent intake", "Children", true, None, "AcceptParentIntakeResponse"));

    add(&mut paths, "/api/cases", "get", op("List published cases", "Content", true, None, "CaseListResponse"));
    add(&mut paths, "/api/cases/{case_id}", "get", op("Get case", "Content", true, None, "CaseDetailResponse"));
    add(&mut paths, "/api/cases/{case_id}/clone", "post", op("Clone case", "Content", true, Some("CloneCaseRequest"), "CloneCaseResponse"));
    add(&mut paths, "/api/story-templates", "get", op("List story templates", "Content", true, None, "StoryTemplateListResponse"));
    add(&mut paths, "/api/story-templates", "post", op("Create story template", "Content", true, Some("CreateTemplateRequest"), "StoryTemplateRecord"));
    add(&mut paths, "/api/story-templates/{template_id}", "get", op("Get story template", "Content", true, None, "StoryTemplateRecord"));
    add(&mut paths, "/api/story-templates/{template_id}", "patch", op("Update story template", "Content", true, Some("UpdateTemplateRequest"), "StoryTemplateRecord"));

    add(&mut paths, "/api/storybooks/generate", "post", op("Generate storybook", "Storybooks", true, Some("GenerateStorybookRequest"), "GenerateStorybookResponse"));
    add(&mut paths, "/api/storybooks", "get", op("List storybooks", "Storybooks", true, None, "StorybookListResponse"));
    add(&mut paths, "/api/storybooks/{storybook_id}", "get", op("Get storybook", "Storybooks", true, None, "StorybookDetailResponse"));
    add(&mut paths, "/api/storybooks/{storybook_id}", "patch", op("Update storybook", "Storybooks", true, Some("UpdateStorybookRequest"), "StorybookRecord"));
    add(&mut paths, "/api/storybooks/{storybook_id}/duplicate", "post", op("Duplicate storybook", "Storybooks", true, Some("DuplicateStorybookRequest"), "StorybookRecord"));
    add(&mut paths, "/api/storybooks/{storybook_id}/derive-custom", "post", op("Derive custom storybook", "Storybooks", true, Some("DeriveCustomStorybookRequest"), "StorybookRecord"));
    add(&mut paths, "/api/storybooks/{storybook_id}/pages", "get", op("List storybook pages", "Storybooks", true, None, "StorybookPageListResponse"));
    add(&mut paths, "/api/storybooks/{storybook_id}/pages", "post", op("Add storybook page", "Storybooks", true, Some("AddPageRequest"), "StorybookPageRecord"));
    add(&mut paths, "/api/storybooks/{storybook_id}/pages/{page_id}", "patch", op("Update storybook page", "Storybooks", true, Some("UpdatePageRequest"), "StorybookPageRecord"));
    add(&mut paths, "/api/storybooks/{storybook_id}/pages/{page_id}", "delete", op("Delete storybook page", "Storybooks", true, None, "StatusResponse"));
    add(&mut paths, "/api/storybooks/{storybook_id}/pages/{page_id}/rewrite", "post", op("Rewrite storybook page", "Storybooks", true, Some("RewritePageRequest"), "StorybookPageRecord"));

    add(&mut paths, "/api/children/{child_id}/character-profiles", "get", op("List character profiles", "Visuals", true, None, "CharacterProfileListResponse"));
    add(&mut paths, "/api/children/{child_id}/character-profiles", "post", op("Create character profile", "Visuals", true, Some("CreateCharacterProfileRequest"), "CharacterProfileRecord"));
    add(&mut paths, "/api/character-profiles/{profile_id}", "get", op("Get character profile", "Visuals", true, None, "CharacterProfileRecord"));
    add(&mut paths, "/api/character-profiles/{profile_id}", "patch", op("Update character profile", "Visuals", true, Some("UpdateCharacterProfileRequest"), "CharacterProfileRecord"));
    add(&mut paths, "/api/parents/{parent_id}/character-profiles", "post", op("Create parent character profile", "Visuals", true, Some("CreateParentCharacterProfileRequest"), "ParentCharacterProfileRecord"));
    add(&mut paths, "/api/storybooks/{storybook_id}/roles", "get", op("List storybook roles", "Visuals", true, None, "StorybookRoleListResponse"));
    add(&mut paths, "/api/storybooks/{storybook_id}/roles/{role_key}", "patch", op("Update storybook role", "Visuals", true, Some("UpdateStorybookRoleRequest"), "StorybookRoleRecord"));
    add(&mut paths, "/api/storybooks/{storybook_id}/replace-roles", "post", op("Replace storybook roles", "Visuals", true, Some("ReplaceRolesRequest"), "ReplaceRolesResponse"));
    add(&mut paths, "/api/storybooks/{storybook_id}/props", "get", op("List prop profiles", "Visuals", true, None, "PropProfileListResponse"));
    add(&mut paths, "/api/storybooks/{storybook_id}/props", "post", op("Create prop profile", "Visuals", true, Some("CreatePropProfileRequest"), "PropProfileRecord"));
    add(&mut paths, "/api/prop-profiles/{prop_id}", "patch", op("Update prop profile", "Visuals", true, Some("UpdatePropProfileRequest"), "PropProfileRecord"));
    add(&mut paths, "/api/storybook-pages/{page_id}/visual-subjects", "put", op("Replace page visual subjects", "Visuals", true, Some("PutPageVisualSubjectsRequest"), "PageVisualSubjectListResponse"));
    add(&mut paths, "/api/reference-images/generate", "post", op("Generate reference image", "Visuals", true, Some("GenerateReferenceImageRequest"), "ReferenceImageRecord"));
    add(&mut paths, "/api/reference-images/{reference_image_id}", "get", op("Get reference image", "Visuals", true, None, "ReferenceImageRecord"));
    add(&mut paths, "/api/reference-images/{reference_image_id}/activate", "post", op("Activate reference image", "Visuals", true, None, "ReferenceImageRecord"));

    add(&mut paths, "/api/assets/upload-intents", "post", op("Create upload intent", "Images", true, Some("CreateUploadIntentRequest"), "UploadIntentRecord"));
    add(&mut paths, "/api/assets", "post", op("Create image asset", "Images", true, Some("CreateAssetRequest"), "ImageAssetRecord"));
    add(&mut paths, "/api/assets/{asset_id}", "get", op("Get image asset", "Images", true, None, "ImageAssetRecord"));
    add(&mut paths, "/api/storybook-pages/{page_id}/image-tasks", "post", op("Create page image task", "Images", true, Some("CreatePageImageTaskRequest"), "ImageTaskDetailResponse"));
    add(&mut paths, "/api/storybooks/{storybook_id}/image-tasks", "post", op("Create storybook image task", "Images", true, Some("CreateStorybookImageTaskRequest"), "StorybookImageTaskResponse"));
    add(&mut paths, "/api/image-tasks/{task_id}", "get", op("Get image task", "Images", true, None, "ImageTaskDetailResponse"));
    add(&mut paths, "/api/image-tasks/{task_id}/review-events", "get", op("List image review events", "Images", true, None, "ImageReviewEventListResponse"));
    add(&mut paths, "/api/image-tasks/{task_id}/retry", "post", op("Retry image task", "Images", true, Some("RetryImageTaskRequest"), "ImageTaskDetailResponse"));
    add(&mut paths, "/api/image-outputs/{output_id}/select", "post", op("Select image output", "Images", true, None, "ImageGenerationOutputRecord"));
    add(&mut paths, "/api/image-outputs/{output_id}/review", "post", op("Review image output", "Images", true, Some("ReviewImageOutputRequest"), "ImageGenerationOutputRecord"));
    add(&mut paths, "/api/admin/generation-costs", "get", op("List generation costs", "Admin", true, None, "GenerationCostListResponse"));

    add(&mut paths, "/api/storybooks/{storybook_id}/exports", "post", op("Create storybook export", "Delivery", true, Some("CreateExportRequest"), "StorybookExportRecord"));
    add(&mut paths, "/api/storybooks/{storybook_id}/exports", "get", op("List storybook exports", "Delivery", true, None, "StorybookExportListResponse"));
    add(&mut paths, "/api/exports/{export_id}", "get", op("Get storybook export", "Delivery", true, None, "StorybookExportRecord"));
    add(&mut paths, "/api/storybooks/{storybook_id}/share-links", "post", op("Create share link", "Delivery", true, Some("CreateShareLinkRequest"), "StorybookShareLinkRecord"));
    add(&mut paths, "/api/storybooks/{storybook_id}/share-links", "get", op("List share links", "Delivery", true, None, "ShareLinkListResponse"));
    add(&mut paths, "/api/share-links/{share_link_id}", "patch", op("Update share link", "Delivery", true, Some("UpdateShareLinkRequest"), "StorybookShareLinkRecord"));
    add(&mut paths, "/api/shared-library", "get", op("List shared library", "Delivery", true, None, "SharedLibraryListResponse"));
    add(&mut paths, "/api/shared-library/{storybook_id}/clone", "post", op("Clone shared storybook", "Delivery", true, Some("CloneSharedStorybookRequest"), "StorybookRecord"));
    add(&mut paths, "/api/storybooks/{storybook_id}/submit-platform-review", "post", op("Submit platform review", "Delivery", true, None, "SubmitPlatformReviewResponse"));

    Value::Object(paths)
}

fn add(paths: &mut serde_json::Map<String, Value>, path: &str, method: &str, operation: Value) {
    let entry = paths
        .entry(path.to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    entry
        .as_object_mut()
        .expect("path item is an object")
        .insert(method.to_string(), operation);
}

fn op(
    summary: &str,
    tag: &str,
    secured: bool,
    request_schema: Option<&str>,
    response_schema: &str,
) -> Value {
    let mut operation = serde_json::Map::new();
    operation.insert("tags".to_string(), json!([tag]));
    operation.insert("summary".to_string(), json!(summary));
    operation.insert("parameters".to_string(), json!(common_parameters()));
    if secured {
        operation.insert("security".to_string(), json!([{ "bearerAuth": [] }]));
    }
    if let Some(schema) = request_schema {
        operation.insert(
            "requestBody".to_string(),
            json!({
                "required": true,
                "content": {
                    "application/json": {
                        "schema": ref_schema(schema)
                    }
                }
            }),
        );
    }
    operation.insert(
        "responses".to_string(),
        json!({
            "200": {
                "description": "Success",
                "content": {
                    "application/json": {
                        "schema": ref_schema(response_schema)
                    }
                }
            },
            "400": error_response("Validation error"),
            "401": error_response("Unauthorized"),
            "403": error_response("Forbidden"),
            "404": error_response("Not found"),
            "409": error_response("State conflict")
        }),
    );
    Value::Object(operation)
}

fn common_parameters() -> Vec<Value> {
    vec![
        json!({
            "name": "Idempotency-Key",
            "in": "header",
            "required": false,
            "schema": { "type": "string" },
            "description": "Optional idempotency key for supported POST endpoints."
        }),
        json!({
            "name": "Authorization",
            "in": "header",
            "required": false,
            "schema": { "type": "string", "example": "Bearer <token>" }
        }),
    ]
}

fn error_response(description: &str) -> Value {
    json!({
        "description": description,
        "content": {
            "application/json": {
                "schema": ref_schema("ErrorEnvelope")
            }
        }
    })
}

fn ref_schema(name: &str) -> Value {
    json!({ "$ref": format!("#/components/schemas/{name}") })
}

fn components() -> Value {
    json!({
        "securitySchemes": {
            "bearerAuth": {
                "type": "http",
                "scheme": "bearer"
            }
        },
        "schemas": schemas()
    })
}

fn schemas() -> Value {
    let mut schemas = serde_json::Map::new();

    for name in [
        "AuthResponse", "CurrentSessionResponse", "LogoutResponse", "EmailVerificationResponse",
        "SchoolRecord", "ClassroomRecord", "TeacherRecord", "CurrentTeacherResponse",
        "TeacherDashboardResponse", "ContentItemListResponse", "ActivityListResponse",
        "ChildRecord", "ChildDetailResponse", "ChildPhotoRecord", "ParentRecord",
        "ParentIntakeRecord", "ParentIntakeLinkRecord", "AcceptParentIntakeResponse",
        "CaseStorybookRecord", "CaseDetailResponse", "CloneCaseResponse",
        "StoryTemplateRecord", "StorybookRecord", "GenerateStorybookResponse",
        "StorybookDetailResponse", "StorybookPageRecord", "StatusResponse",
        "CharacterProfileRecord", "ParentCharacterProfileRecord", "StorybookRoleRecord",
        "ReplaceRolesResponse", "PropProfileRecord", "PageVisualSubjectRecord",
        "ReferenceImageRecord", "UploadIntentRecord", "ImageAssetRecord",
        "ImageTaskDetailResponse", "StorybookImageTaskResponse",
        "ImageGenerationOutputRecord", "ImageReviewEventRecord", "GenerationCostLogRecord",
        "StorybookExportRecord", "StorybookShareLinkRecord", "ShareLinkListItem",
        "SharedLibraryItem", "SubmitPlatformReviewResponse",
    ] {
        schemas.insert(name.to_string(), object_schema());
    }

    for (name, item) in [
        ("ClassroomListResponse", "ClassroomRecord"),
        ("TeacherListResponse", "TeacherRecord"),
        ("ChildListResponse", "ChildRecord"),
        ("ParentIntakeListResponse", "ParentIntakeRecord"),
        ("CaseListResponse", "CaseStorybookRecord"),
        ("StoryTemplateListResponse", "StoryTemplateRecord"),
        ("StorybookListResponse", "StorybookRecord"),
        ("StorybookPageListResponse", "StorybookPageRecord"),
        ("CharacterProfileListResponse", "CharacterProfileRecord"),
        ("StorybookRoleListResponse", "StorybookRoleRecord"),
        ("PropProfileListResponse", "PropProfileRecord"),
        ("PageVisualSubjectListResponse", "PageVisualSubjectRecord"),
        ("ImageReviewEventListResponse", "ImageReviewEventRecord"),
        ("GenerationCostListResponse", "GenerationCostLogRecord"),
        ("StorybookExportListResponse", "StorybookExportRecord"),
        ("ShareLinkListResponse", "ShareLinkListItem"),
        ("SharedLibraryListResponse", "SharedLibraryItem"),
    ] {
        schemas.insert(name.to_string(), list_schema(item));
    }

    for name in [
        "LoginRequest", "SendRegistrationCodeRequest", "RegisterRequest", "UpdateSchoolRequest", "CreateClassroomRequest",
        "UpdateClassroomRequest", "CreateChildRequest", "UpdateChildRequest",
        "AddChildPhotoRequest", "UpdateChildPhotoRequest", "CreateParentIntakeRequest",
        "CreateParentIntakeLinkRequest", "CloneCaseRequest", "CreateTemplateRequest",
        "UpdateTemplateRequest", "GenerateStorybookRequest", "UpdateStorybookRequest",
        "DuplicateStorybookRequest", "DeriveCustomStorybookRequest", "AddPageRequest",
        "UpdatePageRequest", "RewritePageRequest", "CreateCharacterProfileRequest",
        "UpdateCharacterProfileRequest", "CreateParentCharacterProfileRequest",
        "UpdateStorybookRoleRequest", "ReplaceRolesRequest", "CreatePropProfileRequest",
        "UpdatePropProfileRequest", "PutPageVisualSubjectsRequest",
        "GenerateReferenceImageRequest", "CreateUploadIntentRequest", "CreateAssetRequest",
        "CreatePageImageTaskRequest", "CreateStorybookImageTaskRequest",
        "RetryImageTaskRequest", "ReviewImageOutputRequest", "CreateExportRequest",
        "CreateShareLinkRequest", "UpdateShareLinkRequest", "CloneSharedStorybookRequest",
    ] {
        schemas.insert(name.to_string(), object_schema());
    }

    schemas.insert("ErrorEnvelope".to_string(), error_schema());
    Value::Object(schemas)
}

fn object_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": true
    })
}

fn list_schema(item: &str) -> Value {
    json!({
        "type": "object",
        "required": ["items", "page", "page_size", "total"],
        "properties": {
            "items": {
                "type": "array",
                "items": ref_schema(item)
            },
            "page": { "type": "integer", "format": "uint32" },
            "page_size": { "type": "integer", "format": "uint32" },
            "total": { "type": "integer", "format": "uint64" }
        }
    })
}

fn error_schema() -> Value {
    json!({
        "type": "object",
        "required": ["error"],
        "properties": {
            "error": {
                "type": "object",
                "required": ["code", "message", "details", "request_id"],
                "properties": {
                    "code": { "type": "string" },
                    "message": { "type": "string" },
                    "details": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["field", "message"],
                            "properties": {
                                "field": { "type": "string" },
                                "message": { "type": "string" }
                            }
                        }
                    },
                    "request_id": { "type": "string" }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use crate::api::{AppState, router};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::Value;
    use std::sync::{Arc, RwLock};
    use tower::ServiceExt;

    fn test_app() -> axum::Router {
        router(Arc::new(RwLock::new(AppState::test_fixture())))
    }

    #[tokio::test]
    async fn serves_openapi_json() {
        let request = Request::builder()
            .method("GET")
            .uri("/api-docs/openapi.json")
            .body(Body::empty())
            .unwrap();
        let response = test_app().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["openapi"], "3.0.3");
        assert!(body["paths"]["/api/auth/login"]["post"].is_object());
        assert!(body["paths"]["/api/storybooks/{storybook_id}"]["get"].is_object());
        assert!(body["components"]["securitySchemes"]["bearerAuth"].is_object());
    }

    #[tokio::test]
    async fn serves_swagger_ui() {
        let request = Request::builder()
            .method("GET")
            .uri("/swagger-ui/")
            .body(Body::empty())
            .unwrap();
        let response = test_app().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(body.contains("/api-docs/openapi.json"));
        assert!(body.contains("SwaggerUIBundle"));
    }
}
