use super::{ApiError, SharedState, auth::AuthenticatedTeacher};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::views::dashboard::{
    ActivityItem, ChildSummary, ClassroomSummary, ContentItemSummary, ListResponse,
    RecommendedCaseSummary, SchoolSummary, TeacherDashboardResponse, TeacherSummary, WorkCounts,
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/dashboard/teacher", get(get_teacher_dashboard))
        .route("/content-items", get(list_content_items))
        .route(
            "/content-items/{storybook_id}/activity",
            get(list_content_item_activity),
        )
}

#[derive(Debug, Deserialize)]
pub struct DashboardQuery {
    pub classroom_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct ContentItemsQuery {
    pub keyword: Option<String>,
    pub child_id: Option<Uuid>,
    pub content_type: Option<String>,
    pub status: Option<String>,
    pub illustration_status: Option<String>,
    pub share_scope: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

async fn get_teacher_dashboard(
    auth: AuthenticatedTeacher,
    State(state): State<SharedState>,
    Query(query): Query<DashboardQuery>,
) -> Result<Json<TeacherDashboardResponse>, ApiError> {
    let state = state.read().expect("state lock poisoned");
    let school_id = auth.school_id;
    let teacher_id = auth.teacher_id;
    let teacher = state
        .organization
        .teachers
        .get(&teacher_id)
        .ok_or_else(|| ApiError::not_found("teacher"))?;
    let school = state
        .organization
        .schools
        .get(&school_id)
        .ok_or_else(|| ApiError::not_found("school"))?;
    if let Some(classroom_id) = query.classroom_id {
        let classroom = state
            .organization
            .classrooms
            .get(&classroom_id)
            .ok_or_else(|| ApiError::not_found("classroom"))?;
        if classroom.school_id != school_id {
            return Err(ApiError::forbidden("不能查看其他园所的班级看板"));
        }
    }

    let classroom_id = query.classroom_id.or_else(|| {
        state
            .organization
            .classrooms
            .values()
            .find(|classroom| {
                classroom.teacher_id == Some(teacher_id) && classroom.status == "active"
            })
            .map(|classroom| classroom.id)
    });
    let current_classroom = classroom_id.and_then(|classroom_id| {
        state
            .organization
            .classrooms
            .get(&classroom_id)
            .map(|classroom| {
                let child_count = state
                    .children
                    .children
                    .values()
                    .filter(|child| {
                        child.classroom_id == Some(classroom_id) && child.status == "active"
                    })
                    .count();
                ClassroomSummary {
                    id: classroom.id,
                    name: classroom.name.clone(),
                    child_count,
                }
            })
    });

    let mut storybooks = visible_storybooks(&state, school_id);
    storybooks.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    let work_counts = WorkCounts {
        story_generating: storybooks
            .iter()
            .filter(|storybook| storybook.story_status == "story_generating")
            .count(),
        needs_illustration: storybooks
            .iter()
            .filter(|storybook| {
                storybook.story_status == "story_ready"
                    && matches!(
                        storybook.illustration_status.as_str(),
                        "not_started" | "queued" | "running" | "partial_failed" | "failed"
                    )
            })
            .count(),
        ready_to_export: storybooks
            .iter()
            .filter(|storybook| {
                storybook.story_status == "story_ready"
                    && storybook.status == "ready"
                    && storybook.export_status == "not_exported"
            })
            .count(),
        children_missing_profile: state
            .children
            .children
            .values()
            .filter(|child| child.school_id == Some(school_id) && child.status == "active")
            .filter(|child| {
                child.profile_completion_status == "missing_required"
                    || child.profile_completion_status == "usable"
            })
            .count(),
        running_image_tasks: state
            .images
            .tasks
            .values()
            .filter(|task| task.school_id == Some(school_id))
            .filter(|task| matches!(task.status.as_str(), "queued" | "running" | "needs_review"))
            .count(),
    };
    let recent_storybooks = storybooks
        .iter()
        .take(6)
        .map(|storybook| content_item_summary(&state, storybook))
        .collect();
    let mut recommended_cases = state
        .content
        .case_storybooks
        .values()
        .filter(|case| case.status == "published")
        .map(|case| RecommendedCaseSummary {
            id: case.id,
            title: case.title.clone(),
            content_type: case.content_type.clone(),
            theme: case.theme.clone(),
            teaching_goal: case.teaching_goal.clone(),
            target_age_group: case.target_age_group.clone(),
            cover_image_asset_id: case.cover_image_asset_id,
        })
        .collect::<Vec<_>>();
    recommended_cases.sort_by(|a, b| a.title.cmp(&b.title));
    recommended_cases.truncate(6);

    Ok(Json(TeacherDashboardResponse {
        teacher: TeacherSummary {
            id: teacher.id,
            name: teacher.name.clone(),
            role: teacher.role.clone(),
        },
        current_school: SchoolSummary {
            id: school.id,
            name: school.name.clone(),
        },
        current_classroom,
        work_counts,
        recent_storybooks,
        recommended_cases,
    }))
}

async fn list_content_items(
    auth: AuthenticatedTeacher,
    State(state): State<SharedState>,
    Query(query): Query<ContentItemsQuery>,
) -> Result<Json<ListResponse<ContentItemSummary>>, ApiError> {
    validate_optional(
        query.content_type.as_deref(),
        &["plain_storybook", "custom_storybook"],
        "content_type",
    )?;
    validate_optional(
        query.status.as_deref(),
        &["draft", "generating", "ready", "exporting", "archived"],
        "status",
    )?;
    validate_optional(
        query.illustration_status.as_deref(),
        &[
            "not_started",
            "queued",
            "running",
            "ready",
            "partial_failed",
            "failed",
        ],
        "illustration_status",
    )?;
    validate_optional(
        query.share_scope.as_deref(),
        &[
            "private",
            "family",
            "school",
            "platform_review",
            "platform_public",
        ],
        "share_scope",
    )?;
    let state = state.read().expect("state lock poisoned");
    let school_id = auth.school_id;
    if let Some(child_id) = query.child_id {
        let child = state
            .children
            .children
            .get(&child_id)
            .ok_or_else(|| ApiError::not_found("child"))?;
        if child.school_id != Some(school_id) {
            return Err(ApiError::forbidden("不能访问其他园所的儿童档案"));
        }
    }

    let keyword = query
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut items = visible_storybooks(&state, school_id)
        .into_iter()
        .filter(|storybook| {
            query
                .content_type
                .as_deref()
                .is_none_or(|value| storybook.content_type == value)
        })
        .filter(|storybook| {
            query
                .status
                .as_deref()
                .is_none_or(|value| storybook.status == value)
        })
        .filter(|storybook| {
            query
                .illustration_status
                .as_deref()
                .is_none_or(|value| storybook.illustration_status == value)
        })
        .filter(|storybook| {
            query
                .share_scope
                .as_deref()
                .is_none_or(|value| storybook.share_scope == value)
        })
        .filter(|storybook| {
            query
                .child_id
                .is_none_or(|child_id| storybook.child_id == Some(child_id))
        })
        .filter(|storybook| {
            keyword.is_none_or(|keyword| {
                storybook.title.contains(keyword)
                    || storybook.theme.contains(keyword)
                    || storybook
                        .teaching_goal
                        .as_deref()
                        .is_some_and(|goal| goal.contains(keyword))
            })
        })
        .map(|storybook| content_item_summary(&state, storybook))
        .collect::<Vec<_>>();
    items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(Json(paginate(items, query.page, query.page_size)))
}

async fn list_content_item_activity(
    auth: AuthenticatedTeacher,
    State(state): State<SharedState>,
    Path(storybook_id): Path<Uuid>,
) -> Result<Json<ListResponse<ActivityItem>>, ApiError> {
    let state = state.read().expect("state lock poisoned");
    let school_id = auth.school_id;
    let storybook = state
        .storybooks
        .storybooks
        .get(&storybook_id)
        .ok_or_else(|| ApiError::not_found("storybook"))?;
    if !storybook_visible_to_school(storybook, school_id) {
        return Err(ApiError::forbidden("不能访问该读本活动"));
    }

    let mut items = vec![ActivityItem {
        id: format!("storybook:{}", storybook.id),
        activity_type: "storybook_updated".to_string(),
        occurred_at: storybook.updated_at,
        summary: format!("读本《{}》已更新", storybook.title),
        metadata_json: json!({
            "storybook_id": storybook.id,
            "story_status": storybook.story_status,
            "illustration_status": storybook.illustration_status
        }),
    }];
    for page in state
        .storybooks
        .pages
        .get(&storybook.id)
        .into_iter()
        .flatten()
    {
        items.push(ActivityItem {
            id: format!("page:{}", page.id),
            activity_type: "page_updated".to_string(),
            occurred_at: page.updated_at,
            summary: format!("第 {} 页内容已更新", page.page_number),
            metadata_json: json!({
                "page_id": page.id,
                "page_number": page.page_number,
                "scene_spec_status": page.scene_spec_status,
                "illustration_status": page.illustration_status
            }),
        });
    }
    for task in state
        .images
        .tasks
        .values()
        .filter(|task| task.storybook_id == Some(storybook.id))
    {
        items.push(ActivityItem {
            id: format!("image_task:{}", task.id),
            activity_type: "image_task_updated".to_string(),
            occurred_at: task.updated_at,
            summary: format!("图片任务 {}：{}", task.task_type, task.status),
            metadata_json: json!({
                "task_id": task.id,
                "task_type": task.task_type,
                "storybook_page_id": task.storybook_page_id,
                "status": task.status,
                "provider_name": task.provider_name,
                "model_name": task.model_name
            }),
        });
    }
    for output in state
        .images
        .outputs
        .values()
        .filter(|output| output.is_selected)
    {
        let Some(task) = state.images.tasks.get(&output.task_id) else {
            continue;
        };
        if task.storybook_id == Some(storybook.id) {
            items.push(ActivityItem {
                id: format!("image_output:{}", output.id),
                activity_type: "image_output_selected".to_string(),
                occurred_at: output.created_at,
                summary: "已选择候选图作为页面当前图".to_string(),
                metadata_json: json!({
                    "output_id": output.id,
                    "task_id": output.task_id,
                    "image_asset_id": output.image_asset_id,
                    "candidate_index": output.candidate_index
                }),
            });
        }
    }
    for export in state
        .delivery
        .exports
        .values()
        .filter(|export| export.storybook_id == storybook.id)
    {
        items.push(ActivityItem {
            id: format!("export:{}", export.id),
            activity_type: "export_updated".to_string(),
            occurred_at: export.updated_at,
            summary: format!("{} 导出任务：{}", export.export_type, export.status),
            metadata_json: json!({
                "export_id": export.id,
                "export_type": export.export_type,
                "status": export.status,
                "asset_id": export.asset_id
            }),
        });
    }
    for share in state
        .delivery
        .share_links
        .values()
        .filter(|share| share.storybook_id == storybook.id)
    {
        items.push(ActivityItem {
            id: format!("share_link:{}", share.id),
            activity_type: "share_link_updated".to_string(),
            occurred_at: share.updated_at,
            summary: format!("{} 分享链接：{}", share.share_scope, share.status),
            metadata_json: json!({
                "share_link_id": share.id,
                "share_scope": share.share_scope,
                "status": share.status,
                "anonymize_child_name": share.anonymize_child_name,
                "anonymize_parent_info": share.anonymize_parent_info
            }),
        });
    }
    items.sort_by(|a, b| b.occurred_at.cmp(&a.occurred_at));
    Ok(Json(paginate(items, Some(1), Some(50))))
}

fn visible_storybooks(
    state: &crate::api::AppState,
    school_id: Uuid,
) -> Vec<&crate::api::storybooks::StorybookRecord> {
    state
        .storybooks
        .storybooks
        .values()
        .filter(|storybook| storybook_visible_to_school(storybook, school_id))
        .collect()
}

fn storybook_visible_to_school(
    storybook: &crate::api::storybooks::StorybookRecord,
    school_id: Uuid,
) -> bool {
    storybook.school_id == Some(school_id)
        || matches!(
            storybook.share_scope.as_str(),
            "school" | "platform_review" | "platform_public"
        )
}

fn content_item_summary(
    state: &crate::api::AppState,
    storybook: &crate::api::storybooks::StorybookRecord,
) -> ContentItemSummary {
    let child = storybook.child_id.and_then(|child_id| {
        state
            .children
            .children
            .get(&child_id)
            .map(|child| ChildSummary {
                id: child.id,
                name: child.name.clone(),
                classroom_id: child.classroom_id,
                profile_completion_status: child.profile_completion_status.clone(),
            })
    });
    let page_count = state
        .storybooks
        .pages
        .get(&storybook.id)
        .map_or(0, Vec::len);
    let pending_image_task_count = state
        .images
        .tasks
        .values()
        .filter(|task| task.storybook_id == Some(storybook.id))
        .filter(|task| matches!(task.status.as_str(), "queued" | "running" | "needs_review"))
        .count();
    ContentItemSummary {
        storybook_id: storybook.id,
        title: storybook.title.clone(),
        content_type: storybook.content_type.clone(),
        theme: storybook.theme.clone(),
        child,
        story_status: storybook.story_status.clone(),
        illustration_status: storybook.illustration_status.clone(),
        status: storybook.status.clone(),
        export_status: storybook.export_status.clone(),
        share_status: storybook.share_status.clone(),
        share_scope: storybook.share_scope.clone(),
        page_count,
        pending_image_task_count,
        updated_at: storybook.updated_at,
    }
}

fn paginate<T>(items: Vec<T>, page: Option<u32>, page_size: Option<u32>) -> ListResponse<T> {
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(20).clamp(1, 100);
    let total = items.len();
    let start = ((page - 1) * page_size) as usize;
    let paged = items
        .into_iter()
        .skip(start)
        .take(page_size as usize)
        .collect();
    ListResponse {
        items: paged,
        page,
        page_size,
        total,
    }
}

fn validate_optional(
    value: Option<&str>,
    allowed: &[&str],
    field: &'static str,
) -> Result<(), ApiError> {
    if let Some(value) = value {
        if !allowed.contains(&value) {
            return Err(ApiError::validation(field, "枚举值不合法"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{self, AppState};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::Value;
    use std::sync::{Arc, RwLock};
    use tower::ServiceExt;

    fn test_state() -> Arc<RwLock<AppState>> {
        Arc::new(RwLock::new(AppState::demo()))
    }

    fn test_app(state: Arc<RwLock<AppState>>) -> axum::Router {
        api::router(state)
    }

    fn test_token(state: &Arc<RwLock<AppState>>) -> String {
        api::auth::issue_demo_token_for_tests(state)
    }

    async fn json_request(
        app: axum::Router,
        method: &str,
        uri: String,
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
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, value)
    }

    async fn get_json(app: axum::Router, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
        let mut request = Request::builder().uri(uri);
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let request = request.body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, value)
    }

    async fn create_storybook(app: axum::Router) -> Uuid {
        let case_id = crate::api::demo_uuid(31);
        let child_id = crate::api::demo_uuid(10);
        let (status, body) = json_request(
            app,
            "POST",
            "/api/storybooks/generate".to_string(),
            json!({
                "content_type": "custom_storybook",
                "child_id": child_id,
                "case_storybook_id": case_id,
                "title_override": "乐乐的分享工作台故事",
                "style_id": "storybook_flat_v1",
                "reading_age_group": "5-6",
                "teaching_goal": "练习分享"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        serde_json::from_value(body["storybook"]["id"].clone()).unwrap()
    }

    #[tokio::test]
    async fn returns_teacher_dashboard_with_work_counts_and_cases() {
        let state = test_state();
        let token = test_token(&state);
        let app = test_app(state);
        create_storybook(app.clone()).await;

        let (status, body) = get_json(app, "/api/dashboard/teacher", Some(&token)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["teacher"]["name"], "王老师");
        assert_eq!(body["current_school"]["name"], "Kindleaf 幼儿园");
        assert_eq!(body["work_counts"]["needs_illustration"], 1);
        assert_eq!(body["work_counts"]["ready_to_export"], 1);
        assert_eq!(body["recent_storybooks"].as_array().unwrap().len(), 1);
        assert_eq!(body["recommended_cases"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn lists_content_items_with_filters_and_pagination() {
        let state = test_state();
        let token = test_token(&state);
        let app = test_app(state);
        let storybook_id = create_storybook(app.clone()).await;

        let (status, body) = get_json(
            app,
            "/api/content-items?content_type=custom_storybook&page_size=1",
            Some(&token),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["total"], 1);
        assert_eq!(body["items"][0]["storybook_id"], storybook_id.to_string());
        assert_eq!(body["items"][0]["page_count"], 6);
        assert_eq!(body["items"][0]["child"]["name"], "乐乐");
    }

    #[tokio::test]
    async fn returns_content_item_activity_stream() {
        let state = test_state();
        let token = test_token(&state);
        let app = test_app(state);
        let storybook_id = create_storybook(app.clone()).await;
        let (status, export) = json_request(
            app.clone(),
            "POST",
            format!("/api/storybooks/{storybook_id}/exports"),
            json!({
                "export_type": "pdf",
                "include_teacher_tips": false,
                "page_size": "A4",
                "quality": "print",
                "allow_text_only": true
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{export}");
        let (status, share) = json_request(
            app.clone(),
            "POST",
            format!("/api/storybooks/{storybook_id}/share-links"),
            json!({
                "share_scope": "family",
                "anonymize_child_name": false,
                "anonymize_parent_info": true
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{share}");

        let (status, body) = get_json(
            app,
            &format!("/api/content-items/{storybook_id}/activity"),
            Some(&token),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let items = body["items"].as_array().unwrap();
        assert!(
            items
                .iter()
                .any(|item| item["activity_type"] == "storybook_updated")
        );
        assert!(
            items
                .iter()
                .any(|item| item["activity_type"] == "page_updated")
        );
        assert!(
            items
                .iter()
                .any(|item| item["activity_type"] == "export_updated")
        );
        assert!(
            items
                .iter()
                .any(|item| item["activity_type"] == "share_link_updated")
        );
    }

    #[tokio::test]
    async fn dashboard_requires_authenticated_session() {
        let state = test_state();
        let app = test_app(state);
        let (status, body) = get_json(app, "/api/dashboard/teacher", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
        assert_eq!(body["error"]["code"], "UNAUTHORIZED");
    }
}
