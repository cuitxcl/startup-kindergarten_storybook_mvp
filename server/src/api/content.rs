use super::{ApiError, SharedState, now};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

pub use crate::models::content::{
    CasePageRecord, CaseStorybookRecord, ContentStore, StoryTemplateRecord,
};
use crate::views::content::{CaseDetailResponse, CloneCaseResponse, ListResponse};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/cases", get(list_cases))
        .route("/cases/{case_id}", get(get_case))
        .route("/cases/{case_id}/clone", post(clone_case))
        .route(
            "/story-templates",
            get(list_templates).post(create_template),
        )
        .route(
            "/story-templates/{template_id}",
            get(get_template).patch(update_template),
        )
}

#[derive(Debug, Deserialize)]
pub struct CaseListQuery {
    pub theme: Option<String>,
    pub age_group: Option<String>,
    pub content_type: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CloneCaseRequest {
    pub mode: String,
    pub title_override: Option<String>,
    pub style_id: Option<String>,
    pub reading_age_group: Option<String>,
    pub teaching_goal: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TemplateListQuery {
    pub status: Option<String>,
    pub content_type: Option<String>,
    pub theme: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTemplateRequest {
    pub title: String,
    pub content_type: String,
    pub theme: String,
    pub teaching_goal: String,
    pub target_age_group: Option<String>,
    pub page_count: i32,
    pub template_outline_json: Value,
    pub default_role_manifest_json: Option<Value>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTemplateRequest {
    pub title: Option<String>,
    pub content_type: Option<String>,
    pub theme: Option<String>,
    pub teaching_goal: Option<String>,
    pub target_age_group: Option<String>,
    pub page_count: Option<i32>,
    pub template_outline_json: Option<Value>,
    pub default_role_manifest_json: Option<Value>,
    pub status: Option<String>,
}

async fn list_cases(
    State(state): State<SharedState>,
    Query(query): Query<CaseListQuery>,
) -> Result<Json<ListResponse<CaseStorybookRecord>>, ApiError> {
    validate_optional_status(query.status.as_deref(), &["published"], "status")?;
    validate_optional_content_type(query.content_type.as_deref())?;
    let state = state.read().expect("state lock poisoned");
    let mut items = state
        .content
        .case_storybooks
        .values()
        .filter(|case| case.status == query.status.as_deref().unwrap_or("published"))
        .filter(|case| {
            query
                .theme
                .as_deref()
                .is_none_or(|theme| case.theme == theme.trim())
        })
        .filter(|case| {
            query
                .age_group
                .as_deref()
                .is_none_or(|age_group| case.target_age_group.as_deref() == Some(age_group.trim()))
        })
        .filter(|case| {
            query
                .content_type
                .as_deref()
                .is_none_or(|content_type| case.content_type == content_type)
        })
        .cloned()
        .collect::<Vec<_>>();
    items.sort_by(|a, b| a.sort_order.cmp(&b.sort_order).then(a.title.cmp(&b.title)));
    Ok(Json(list_response(items)))
}

async fn get_case(
    State(state): State<SharedState>,
    Path(case_id): Path<Uuid>,
) -> Result<Json<CaseDetailResponse>, ApiError> {
    let state = state.read().expect("state lock poisoned");
    let case_storybook = published_case(&state, case_id)?.clone();
    let pages = state
        .content
        .case_pages
        .get(&case_id)
        .cloned()
        .unwrap_or_default();
    Ok(Json(CaseDetailResponse {
        case_storybook,
        pages,
    }))
}

async fn clone_case(
    State(state): State<SharedState>,
    Path(case_id): Path<Uuid>,
    Json(payload): Json<CloneCaseRequest>,
) -> Result<Json<CloneCaseResponse>, ApiError> {
    validate_content_type(&payload.mode)?;
    let state = state.read().expect("state lock poisoned");
    let case_storybook = published_case(&state, case_id)?;
    let title = payload
        .title_override
        .and_then(normalize_optional_owned)
        .unwrap_or_else(|| case_storybook.title.clone());
    let teaching_goal = payload
        .teaching_goal
        .and_then(normalize_optional_owned)
        .unwrap_or_else(|| case_storybook.teaching_goal.clone());
    let content_type = payload.mode;
    let derivation_type = if content_type == "plain_storybook" {
        "from_case"
    } else {
        "from_case"
    };

    Ok(Json(CloneCaseResponse {
        storybook_id: Uuid::new_v4(),
        source_case_id: case_id,
        title,
        content_type,
        theme: case_storybook.theme.clone(),
        teaching_goal,
        story_status: "draft".to_string(),
        illustration_status: "not_started".to_string(),
        status: "draft".to_string(),
        derivation_type: derivation_type.to_string(),
    }))
}

async fn list_templates(
    State(state): State<SharedState>,
    Query(query): Query<TemplateListQuery>,
) -> Result<Json<ListResponse<StoryTemplateRecord>>, ApiError> {
    validate_operator(&state)?;
    validate_optional_status(
        query.status.as_deref(),
        &["draft", "active", "archived"],
        "status",
    )?;
    validate_optional_content_type(query.content_type.as_deref())?;
    let state = state.read().expect("state lock poisoned");
    let mut items = state
        .content
        .story_templates
        .values()
        .filter(|template| {
            query
                .status
                .as_deref()
                .is_none_or(|status| template.status == status)
        })
        .filter(|template| {
            query
                .content_type
                .as_deref()
                .is_none_or(|content_type| template.content_type == content_type)
        })
        .filter(|template| {
            query
                .theme
                .as_deref()
                .is_none_or(|theme| template.theme == theme.trim())
        })
        .cloned()
        .collect::<Vec<_>>();
    items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(Json(list_response(items)))
}

async fn get_template(
    State(state): State<SharedState>,
    Path(template_id): Path<Uuid>,
) -> Result<Json<StoryTemplateRecord>, ApiError> {
    validate_operator(&state)?;
    let state = state.read().expect("state lock poisoned");
    let template = state
        .content
        .story_templates
        .get(&template_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("story_template"))?;
    Ok(Json(template))
}

async fn create_template(
    State(state): State<SharedState>,
    Json(payload): Json<CreateTemplateRequest>,
) -> Result<Json<StoryTemplateRecord>, ApiError> {
    validate_operator(&state)?;
    let title = required_trimmed(payload.title, "title")?;
    let theme = required_trimmed(payload.theme, "theme")?;
    let teaching_goal = required_trimmed(payload.teaching_goal, "teaching_goal")?;
    validate_content_type(&payload.content_type)?;
    validate_page_count(payload.page_count)?;
    validate_template_outline(&payload.template_outline_json, payload.page_count)?;
    let status = payload.status.unwrap_or_else(|| "draft".to_string());
    validate_optional_status(
        Some(status.as_str()),
        &["draft", "active", "archived"],
        "status",
    )?;
    let default_role_manifest_json = payload
        .default_role_manifest_json
        .unwrap_or_else(|| json!({}));
    if status == "active"
        && default_role_manifest_json
            .as_object()
            .is_none_or(|object| object.is_empty())
    {
        return Err(ApiError::validation(
            "default_role_manifest_json",
            "active 模板必须有默认角色清单",
        ));
    }

    let created_at = now();
    let template = StoryTemplateRecord {
        id: Uuid::new_v4(),
        title,
        content_type: payload.content_type,
        theme,
        teaching_goal,
        target_age_group: payload.target_age_group.and_then(normalize_optional_owned),
        page_count: payload.page_count,
        template_outline_json: payload.template_outline_json,
        default_role_manifest_json,
        status,
        created_at,
        updated_at: created_at,
    };
    let mut state = state.write().expect("state lock poisoned");
    state
        .content
        .story_templates
        .insert(template.id, template.clone());
    Ok(Json(template))
}

async fn update_template(
    State(state): State<SharedState>,
    Path(template_id): Path<Uuid>,
    Json(payload): Json<UpdateTemplateRequest>,
) -> Result<Json<StoryTemplateRecord>, ApiError> {
    validate_operator(&state)?;
    if let Some(content_type) = payload.content_type.as_deref() {
        validate_content_type(content_type)?;
    }
    if let Some(page_count) = payload.page_count {
        validate_page_count(page_count)?;
    }
    if let Some(status) = payload.status.as_deref() {
        validate_optional_status(Some(status), &["draft", "active", "archived"], "status")?;
    }

    let mut state = state.write().expect("state lock poisoned");
    let template = state
        .content
        .story_templates
        .get_mut(&template_id)
        .ok_or_else(|| ApiError::not_found("story_template"))?;
    if let Some(title) = payload.title {
        template.title = required_trimmed(title, "title")?;
    }
    if let Some(content_type) = payload.content_type {
        template.content_type = content_type;
    }
    if let Some(theme) = payload.theme {
        template.theme = required_trimmed(theme, "theme")?;
    }
    if let Some(teaching_goal) = payload.teaching_goal {
        template.teaching_goal = required_trimmed(teaching_goal, "teaching_goal")?;
    }
    if payload.target_age_group.is_some() {
        template.target_age_group = payload.target_age_group.and_then(normalize_optional_owned);
    }
    if let Some(page_count) = payload.page_count {
        if let Some(outline) = payload.template_outline_json.as_ref() {
            validate_template_outline(outline, page_count)?;
        }
        template.page_count = page_count;
    }
    if let Some(outline) = payload.template_outline_json {
        validate_template_outline(&outline, template.page_count)?;
        template.template_outline_json = outline;
    }
    if let Some(manifest) = payload.default_role_manifest_json {
        template.default_role_manifest_json = manifest;
    }
    if let Some(status) = payload.status {
        if status == "active"
            && template
                .default_role_manifest_json
                .as_object()
                .is_none_or(|object| object.is_empty())
        {
            return Err(ApiError::validation(
                "default_role_manifest_json",
                "active 模板必须有默认角色清单",
            ));
        }
        template.status = status;
    }
    template.updated_at = now();
    Ok(Json(template.clone()))
}

fn published_case(
    state: &crate::api::AppState,
    case_id: Uuid,
) -> Result<&CaseStorybookRecord, ApiError> {
    let case_storybook = state
        .content
        .case_storybooks
        .get(&case_id)
        .ok_or_else(|| ApiError::not_found("case_storybook"))?;
    if case_storybook.status != "published" {
        return Err(ApiError::not_found("case_storybook"));
    }
    Ok(case_storybook)
}

fn validate_operator(state: &SharedState) -> Result<(), ApiError> {
    let state = state.read().expect("state lock poisoned");
    let teacher = state
        .organization
        .teachers
        .get(&state.organization.current_teacher_id)
        .ok_or_else(|| ApiError::not_found("teacher"))?;
    if teacher.role == "operator" {
        Ok(())
    } else {
        Err(ApiError::forbidden("需要平台运营权限"))
    }
}

fn validate_content_type(content_type: &str) -> Result<(), ApiError> {
    if ["plain_storybook", "custom_storybook"].contains(&content_type) {
        Ok(())
    } else {
        Err(ApiError::validation("content_type", "内容类型枚举不合法"))
    }
}

fn validate_optional_content_type(content_type: Option<&str>) -> Result<(), ApiError> {
    if let Some(content_type) = content_type {
        validate_content_type(content_type)?;
    }
    Ok(())
}

fn validate_page_count(page_count: i32) -> Result<(), ApiError> {
    if (1..=20).contains(&page_count) {
        Ok(())
    } else {
        Err(ApiError::validation(
            "page_count",
            "页数必须在 1 到 20 之间",
        ))
    }
}

fn validate_template_outline(outline: &Value, page_count: i32) -> Result<(), ApiError> {
    let pages = outline
        .get("pages")
        .and_then(Value::as_array)
        .ok_or_else(|| ApiError::validation("template_outline_json", "必须包含 pages 数组"))?;
    if pages.len() != page_count as usize {
        return Err(ApiError::validation(
            "template_outline_json",
            "pages 数量必须等于 page_count",
        ));
    }
    for (index, page) in pages.iter().enumerate() {
        let page = page
            .as_object()
            .ok_or_else(|| ApiError::validation("template_outline_json", "页面结构必须是对象"))?;
        let page_role = page
            .get("page_role")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ApiError::validation("template_outline_json", "每页必须包含 page_role")
            })?;
        validate_page_role(page_role)?;
        if index == 0 && page_role != "cover" {
            return Err(ApiError::validation(
                "template_outline_json",
                "第一页必须是 cover",
            ));
        }
        if index + 1 == pages.len() && page_role != "closing" {
            return Err(ApiError::validation(
                "template_outline_json",
                "最后一页必须是 closing",
            ));
        }
    }
    Ok(())
}

fn validate_page_role(page_role: &str) -> Result<(), ApiError> {
    if ["cover", "story", "interaction", "closing"].contains(&page_role) {
        Ok(())
    } else {
        Err(ApiError::validation(
            "template_outline_json",
            "page_role 不合法",
        ))
    }
}

fn validate_optional_status(
    value: Option<&str>,
    allowed: &[&str],
    field: &'static str,
) -> Result<(), ApiError> {
    if let Some(value) = value {
        if !allowed.contains(&value) {
            return Err(ApiError::validation(field, "状态枚举不合法"));
        }
    }
    Ok(())
}

fn required_trimmed(value: String, field: &'static str) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ApiError::validation(field, "不能为空"));
    }
    Ok(value.to_string())
}

fn normalize_optional_owned(value: String) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn list_response<T>(items: Vec<T>) -> ListResponse<T> {
    let total = items.len();
    ListResponse {
        items,
        page: 1,
        page_size: total as u32,
        total,
    }
}

#[cfg(test)]
mod tests {
    use crate::api::{AppState, router};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::{Value, json};
    use std::sync::{Arc, RwLock};
    use tower::ServiceExt;

    fn test_app() -> axum::Router {
        router(Arc::new(RwLock::new(AppState::test_fixture())))
    }

    fn operator_app() -> axum::Router {
        let mut state = AppState::test_fixture();
        let teacher_id = state.organization.current_teacher_id;
        state
            .organization
            .teachers
            .get_mut(&teacher_id)
            .unwrap()
            .role = "operator".to_string();
        router(Arc::new(RwLock::new(state)))
    }

    async fn request_json(
        app: axum::Router,
        method: &str,
        uri: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .header(
                "authorization",
                format!("Bearer {}", crate::api::auth::TEST_BEARER_TOKEN),
            )
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    async fn get_json(app: axum::Router, uri: &str) -> (StatusCode, Value) {
        let request = Request::builder()
            .method("GET")
            .header(
                "authorization",
                format!("Bearer {}", crate::api::auth::TEST_BEARER_TOKEN),
            )
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    #[tokio::test]
    async fn lists_published_cases_with_filters() {
        let (status, body) = get_json(
            test_app(),
            "/api/cases?theme=%E5%88%86%E4%BA%AB%E5%90%88%E4%BD%9C&content_type=plain_storybook",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["total"], 1);
        assert_eq!(body["items"][0]["title"], "一起分享更开心");
    }

    #[tokio::test]
    async fn returns_case_detail_with_pages() {
        let (_, list) = get_json(test_app(), "/api/cases").await;
        let case_id = list["items"][0]["id"].as_str().unwrap();
        let (status, body) = get_json(test_app(), &format!("/api/cases/{case_id}")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["title"], "一起分享更开心");
        assert_eq!(body["pages"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn clone_case_returns_draft_storybook_summary() {
        let (_, list) = get_json(test_app(), "/api/cases").await;
        let case_id = list["items"][0]["id"].as_str().unwrap();
        let (status, body) = request_json(
            test_app(),
            "POST",
            &format!("/api/cases/{case_id}/clone"),
            json!({
                "mode": "plain_storybook",
                "title_override": "新的分享故事"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["title"], "新的分享故事");
        assert_eq!(body["story_status"], "draft");
    }

    #[tokio::test]
    async fn blocks_template_access_for_non_operator() {
        let (status, body) = get_json(test_app(), "/api/story-templates").await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["error"]["code"], "FORBIDDEN");
    }

    #[tokio::test]
    async fn creates_and_updates_template_as_operator() {
        let app = operator_app();
        let (status, created) = request_json(
            app.clone(),
            "POST",
            "/api/story-templates",
            json!({
                "title": "情绪表达六页结构",
                "content_type": "plain_storybook",
                "theme": "情绪表达",
                "teaching_goal": "帮助孩子说出自己的情绪",
                "target_age_group": "5-6",
                "page_count": 2,
                "template_outline_json": {
                    "pages": [{"page_role": "cover"}, {"page_role": "closing"}]
                },
                "default_role_manifest_json": {
                    "protagonist": {"role_type": "default_character", "display_name": "小朋友"}
                },
                "status": "active"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(created["status"], "active");
        let template_id = created["id"].as_str().unwrap();

        let (status, updated) = request_json(
            app,
            "PATCH",
            &format!("/api/story-templates/{template_id}"),
            json!({
                "status": "archived"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated["status"], "archived");
    }

    #[tokio::test]
    async fn rejects_template_outline_page_count_mismatch() {
        let (status, body) = request_json(
            operator_app(),
            "POST",
            "/api/story-templates",
            json!({
                "title": "错误模板",
                "content_type": "plain_storybook",
                "theme": "分享合作",
                "teaching_goal": "测试",
                "page_count": 3,
                "template_outline_json": {
                    "pages": [{"page_role": "cover"}]
                }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
        assert_eq!(
            body["error"]["details"][0]["field"],
            "template_outline_json"
        );
    }

    #[tokio::test]
    async fn rejects_invalid_template_outline_page_roles() {
        let (status, body) = request_json(
            operator_app(),
            "POST",
            "/api/story-templates",
            json!({
                "title": "错误模板",
                "content_type": "plain_storybook",
                "theme": "分享合作",
                "teaching_goal": "测试",
                "page_count": 2,
                "template_outline_json": {
                    "pages": [{"page_role": "story"}, {"page_role": "closing"}]
                }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(
            body["error"]["details"][0]["field"],
            "template_outline_json"
        );

        let (status, body) = request_json(
            operator_app(),
            "POST",
            "/api/story-templates",
            json!({
                "title": "错误模板",
                "content_type": "plain_storybook",
                "theme": "分享合作",
                "teaching_goal": "测试",
                "page_count": 2,
                "template_outline_json": {
                    "pages": [{"page_role": "cover"}, {"page_role": "unknown"}]
                }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(
            body["error"]["details"][0]["field"],
            "template_outline_json"
        );
    }
}
