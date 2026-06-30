use super::{ApiError, SharedState, demo_uuid, now};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route(
            "/schools/current",
            get(get_current_school).patch(update_current_school),
        )
        .route("/classrooms", get(list_classrooms).post(create_classroom))
        .route("/classrooms/{classroom_id}", patch(update_classroom))
        .route("/teachers/me", get(get_current_teacher))
        .route("/teachers", get(list_teachers))
}

#[derive(Clone, Debug)]
pub struct OrganizationStore {
    pub current_school_id: Uuid,
    pub current_teacher_id: Uuid,
    pub schools: BTreeMap<Uuid, SchoolRecord>,
    pub classrooms: BTreeMap<Uuid, ClassroomRecord>,
    pub teachers: BTreeMap<Uuid, TeacherRecord>,
}

impl OrganizationStore {
    pub fn demo() -> Self {
        let created_at = now();
        let school_id = demo_uuid(1);
        let teacher_id = demo_uuid(2);
        let classroom_id = demo_uuid(3);

        let mut schools = BTreeMap::new();
        schools.insert(
            school_id,
            SchoolRecord {
                id: school_id,
                name: "Kindleaf 幼儿园".to_string(),
                code: Some("kindleaf-demo".to_string()),
                status: "active".to_string(),
                created_at,
                updated_at: created_at,
            },
        );

        let mut teachers = BTreeMap::new();
        teachers.insert(
            teacher_id,
            TeacherRecord {
                id: teacher_id,
                school_id: Some(school_id),
                name: "王老师".to_string(),
                email: Some("teacher@example.com".to_string()),
                phone: None,
                role: "school_admin".to_string(),
                status: "active".to_string(),
                created_at,
                updated_at: created_at,
            },
        );

        let mut classrooms = BTreeMap::new();
        classrooms.insert(
            classroom_id,
            ClassroomRecord {
                id: classroom_id,
                school_id,
                teacher_id: Some(teacher_id),
                name: "小一班".to_string(),
                grade_level: Some("小班".to_string()),
                status: "active".to_string(),
                created_at,
                updated_at: created_at,
            },
        );

        Self {
            current_school_id: school_id,
            current_teacher_id: teacher_id,
            schools,
            classrooms,
            teachers,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SchoolRecord {
    pub id: Uuid,
    pub name: String,
    pub code: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ClassroomRecord {
    pub id: Uuid,
    pub school_id: Uuid,
    pub teacher_id: Option<Uuid>,
    pub name: String,
    pub grade_level: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TeacherRecord {
    pub id: Uuid,
    pub school_id: Option<Uuid>,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub role: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSchoolRequest {
    pub name: Option<String>,
    pub code: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ClassroomListQuery {
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateClassroomRequest {
    pub name: String,
    pub teacher_id: Option<Uuid>,
    pub grade_level: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateClassroomRequest {
    pub name: Option<String>,
    pub teacher_id: Option<Uuid>,
    pub grade_level: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TeacherListQuery {
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct CurrentTeacherResponse {
    #[serde(flatten)]
    pub teacher: TeacherRecord,
    pub current_school: SchoolRecord,
    pub default_classroom: Option<ClassroomRecord>,
}

async fn get_current_school(
    State(state): State<SharedState>,
) -> Result<Json<SchoolRecord>, ApiError> {
    let state = state.read().expect("state lock poisoned");
    let school = state
        .organization
        .schools
        .get(&state.organization.current_school_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("school"))?;
    Ok(Json(school))
}

async fn update_current_school(
    State(state): State<SharedState>,
    Json(payload): Json<UpdateSchoolRequest>,
) -> Result<Json<SchoolRecord>, ApiError> {
    validate_admin(&state)?;
    validate_optional_status(payload.status.as_deref(), &["active", "inactive"], "status")?;

    let mut state = state.write().expect("state lock poisoned");
    let school_id = state.organization.current_school_id;
    let requested_code = payload.code.as_deref().and_then(normalize_optional);
    if let Some(code) = requested_code.as_deref() {
        let duplicated = state
            .organization
            .schools
            .values()
            .any(|school| school.id != school_id && school.code.as_deref() == Some(code));
        if duplicated {
            return Err(ApiError::state_conflict("园所 code 已存在"));
        }
    }

    let school = state
        .organization
        .schools
        .get_mut(&school_id)
        .ok_or_else(|| ApiError::not_found("school"))?;
    if let Some(name) = payload.name {
        school.name = required_trimmed(name, "name")?;
    }
    if payload.code.is_some() {
        school.code = requested_code;
    }
    if let Some(status) = payload.status {
        school.status = status;
    }
    school.updated_at = now();
    Ok(Json(school.clone()))
}

async fn list_classrooms(
    State(state): State<SharedState>,
    Query(query): Query<ClassroomListQuery>,
) -> Result<Json<ListResponse<ClassroomRecord>>, ApiError> {
    validate_optional_status(query.status.as_deref(), &["active", "archived"], "status")?;
    let state = state.read().expect("state lock poisoned");
    let status = query.status.as_deref().unwrap_or("active");
    let mut items = state
        .organization
        .classrooms
        .values()
        .filter(|classroom| classroom.school_id == state.organization.current_school_id)
        .filter(|classroom| classroom.status == status)
        .cloned()
        .collect::<Vec<_>>();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(list_response(items)))
}

async fn create_classroom(
    State(state): State<SharedState>,
    Json(payload): Json<CreateClassroomRequest>,
) -> Result<Json<ClassroomRecord>, ApiError> {
    validate_admin(&state)?;
    let name = required_trimmed(payload.name, "name")?;
    let grade_level = payload.grade_level.as_deref().and_then(normalize_optional);

    let mut state = state.write().expect("state lock poisoned");
    let school_id = state.organization.current_school_id;
    validate_teacher_in_school(&state.organization, payload.teacher_id, school_id)?;
    validate_classroom_name_unique(&state.organization, school_id, None, &name)?;

    let id = Uuid::new_v4();
    let created_at = now();
    let classroom = ClassroomRecord {
        id,
        school_id,
        teacher_id: payload.teacher_id,
        name,
        grade_level,
        status: "active".to_string(),
        created_at,
        updated_at: created_at,
    };
    state.organization.classrooms.insert(id, classroom.clone());
    Ok(Json(classroom))
}

async fn update_classroom(
    State(state): State<SharedState>,
    Path(classroom_id): Path<Uuid>,
    Json(payload): Json<UpdateClassroomRequest>,
) -> Result<Json<ClassroomRecord>, ApiError> {
    validate_admin(&state)?;
    validate_optional_status(payload.status.as_deref(), &["active", "archived"], "status")?;

    let mut state = state.write().expect("state lock poisoned");
    let school_id = state.organization.current_school_id;
    validate_teacher_in_school(&state.organization, payload.teacher_id, school_id)?;
    let existing = state
        .organization
        .classrooms
        .get(&classroom_id)
        .ok_or_else(|| ApiError::not_found("classroom"))?;
    if existing.school_id != school_id {
        return Err(ApiError::forbidden("不能修改其他园所的班级"));
    }
    let next_name = payload
        .name
        .map(|name| required_trimmed(name, "name"))
        .transpose()?;
    if let Some(name) = next_name.as_deref() {
        validate_classroom_name_unique(&state.organization, school_id, Some(classroom_id), name)?;
    }

    let classroom = state
        .organization
        .classrooms
        .get_mut(&classroom_id)
        .ok_or_else(|| ApiError::not_found("classroom"))?;
    if let Some(name) = next_name {
        classroom.name = name;
    }
    if payload.teacher_id.is_some() {
        classroom.teacher_id = payload.teacher_id;
    }
    if payload.grade_level.is_some() {
        classroom.grade_level = payload.grade_level.as_deref().and_then(normalize_optional);
    }
    if let Some(status) = payload.status {
        classroom.status = status;
    }
    classroom.updated_at = now();
    Ok(Json(classroom.clone()))
}

async fn get_current_teacher(
    State(state): State<SharedState>,
) -> Result<Json<CurrentTeacherResponse>, ApiError> {
    let state = state.read().expect("state lock poisoned");
    let teacher = current_teacher(&state.organization)?.clone();
    let current_school = state
        .organization
        .schools
        .get(&state.organization.current_school_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("school"))?;
    let default_classroom = state
        .organization
        .classrooms
        .values()
        .find(|classroom| {
            classroom.school_id == current_school.id
                && classroom.teacher_id == Some(teacher.id)
                && classroom.status == "active"
        })
        .cloned();
    Ok(Json(CurrentTeacherResponse {
        teacher,
        current_school,
        default_classroom,
    }))
}

async fn list_teachers(
    State(state): State<SharedState>,
    Query(query): Query<TeacherListQuery>,
) -> Result<Json<ListResponse<TeacherRecord>>, ApiError> {
    validate_admin(&state)?;
    validate_optional_status(query.status.as_deref(), &["active", "inactive"], "status")?;
    let state = state.read().expect("state lock poisoned");
    let status = query.status.as_deref().unwrap_or("active");
    let mut items = state
        .organization
        .teachers
        .values()
        .filter(|teacher| teacher.school_id == Some(state.organization.current_school_id))
        .filter(|teacher| teacher.status == status)
        .cloned()
        .collect::<Vec<_>>();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(list_response(items)))
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

fn validate_admin(state: &SharedState) -> Result<(), ApiError> {
    let state = state.read().expect("state lock poisoned");
    let teacher = current_teacher(&state.organization)?;
    if teacher.role == "school_admin" || teacher.role == "operator" {
        Ok(())
    } else {
        Err(ApiError::forbidden("需要园所管理员权限"))
    }
}

fn current_teacher(store: &OrganizationStore) -> Result<&TeacherRecord, ApiError> {
    store
        .teachers
        .get(&store.current_teacher_id)
        .ok_or_else(|| ApiError::not_found("teacher"))
}

fn validate_teacher_in_school(
    store: &OrganizationStore,
    teacher_id: Option<Uuid>,
    school_id: Uuid,
) -> Result<(), ApiError> {
    if let Some(teacher_id) = teacher_id {
        let teacher = store
            .teachers
            .get(&teacher_id)
            .ok_or_else(|| ApiError::not_found("teacher"))?;
        if teacher.school_id != Some(school_id) || teacher.status != "active" {
            return Err(ApiError::validation(
                "teacher_id",
                "老师必须属于当前园所且为 active",
            ));
        }
    }
    Ok(())
}

fn validate_classroom_name_unique(
    store: &OrganizationStore,
    school_id: Uuid,
    exclude_classroom_id: Option<Uuid>,
    name: &str,
) -> Result<(), ApiError> {
    let duplicated = store.classrooms.values().any(|classroom| {
        classroom.school_id == school_id
            && Some(classroom.id) != exclude_classroom_id
            && classroom.name == name
    });
    if duplicated {
        return Err(ApiError::state_conflict("同一园所内班级名称已存在"));
    }
    Ok(())
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

fn normalize_optional(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
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
        router(Arc::new(RwLock::new(AppState::demo())))
    }

    async fn json_request(method: &str, uri: &str, body: Value) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = test_app().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    async fn json_request_on(
        app: axum::Router,
        method: &str,
        uri: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
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

    async fn get_json(uri: &str) -> (StatusCode, Value) {
        let request = Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        let response = test_app().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    #[tokio::test]
    async fn returns_current_school() {
        let (status, body) = get_json("/api/schools/current").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], "Kindleaf 幼儿园");
        assert_eq!(body["status"], "active");
    }

    #[tokio::test]
    async fn creates_classroom_with_validation() {
        let (status, body) = json_request(
            "POST",
            "/api/classrooms",
            json!({
                "name": "中一班",
                "grade_level": "中班"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], "中一班");
        assert_eq!(body["status"], "active");

        let (status, body) =
            json_request("POST", "/api/classrooms", json!({ "name": "   " })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
        assert_eq!(body["error"]["details"][0]["field"], "name");
    }

    #[tokio::test]
    async fn rejects_duplicate_classroom_names_in_school() {
        let app = test_app();
        let (status, body) = json_request_on(
            app.clone(),
            "POST",
            "/api/classrooms",
            json!({
                "name": "小一班",
                "grade_level": "小班"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["code"], "STATE_CONFLICT");

        let (status, created) = json_request_on(
            app.clone(),
            "POST",
            "/api/classrooms",
            json!({
                "name": "中一班",
                "grade_level": "中班"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{created}");
        let classroom_id = created["id"].as_str().unwrap();
        let (status, body) = json_request_on(
            app,
            "PATCH",
            &format!("/api/classrooms/{classroom_id}"),
            json!({ "name": "小一班" }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["code"], "STATE_CONFLICT");
    }

    #[tokio::test]
    async fn rejects_invalid_classroom_status_filter() {
        let (status, body) = get_json("/api/classrooms?status=deleted").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn lists_current_teacher_context() {
        let (status, body) = get_json("/api/teachers/me").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], "王老师");
        assert_eq!(body["current_school"]["name"], "Kindleaf 幼儿园");
        assert_eq!(body["default_classroom"]["name"], "小一班");
    }

    #[tokio::test]
    async fn rejects_teacher_list_for_non_admin() {
        let mut state = AppState::demo();
        let teacher_id = state.organization.current_teacher_id;
        state
            .organization
            .teachers
            .get_mut(&teacher_id)
            .unwrap()
            .role = "teacher".to_string();
        let app = router(Arc::new(RwLock::new(state)));
        let request = Request::builder()
            .method("GET")
            .uri("/api/teachers")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
