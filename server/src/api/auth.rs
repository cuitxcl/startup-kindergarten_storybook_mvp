use super::{ApiError, SharedState, now};
use axum::{
    Json, Router,
    body::Body,
    extract::{FromRequestParts, State},
    http::{HeaderMap, Request, request::Parts},
    middleware::Next,
    response::Response,
    routing::{get, post},
};
use serde::Deserialize;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::models::auth::demo_password_hash;
pub use crate::models::auth::{AuthSessionRecord, AuthStore, TeacherCredentialRecord};
use crate::views::auth::{
    AuthResponse, AuthSessionSummary, CurrentSessionResponse, LogoutResponse, TeacherAuthSummary,
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/me", get(me))
        .route("/auth/refresh", post(refresh_session))
        .route("/auth/logout", post(logout))
}

pub async fn require_session(
    State(state): State<SharedState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let token = bearer_token(request.headers())?.to_string();
    {
        let mut state = state.write().expect("state lock poisoned");
        authenticated_teacher_from_token(&mut state, &token)?;
    }
    Ok(next.run(request).await)
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub identifier: String,
    pub password: String,
}

#[derive(Clone, Debug)]
pub struct AuthenticatedTeacher {
    pub teacher_id: Uuid,
    pub school_id: Uuid,
}

#[cfg(test)]
pub const TEST_BEARER_TOKEN: &str = "dev-test-token";

impl FromRequestParts<SharedState> for AuthenticatedTeacher {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer_token(&parts.headers)?.to_string();
        let mut state = state.write().expect("state lock poisoned");
        authenticated_teacher_from_token(&mut state, &token)
    }
}

pub fn issue_demo_token_for_tests(state: &Arc<RwLock<crate::api::AppState>>) -> String {
    let mut state = state.write().expect("state lock poisoned");
    let teacher = state
        .organization
        .teachers
        .get(&state.organization.current_teacher_id)
        .cloned()
        .expect("demo teacher exists");
    let issued_at = now();
    let expires_at = issued_at + chrono::Duration::hours(12);
    let token = format!("dev-test-{}", Uuid::new_v4().simple());
    state.auth.sessions.insert(
        token.clone(),
        AuthSessionRecord {
            token: token.clone(),
            teacher_id: teacher.id,
            school_id: teacher.school_id,
            status: "active".to_string(),
            issued_at,
            expires_at,
            last_seen_at: issued_at,
        },
    );
    token
}

async fn login(
    State(state): State<SharedState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let identifier = required_trimmed(payload.identifier, "identifier")?.to_lowercase();
    let password = required_trimmed(payload.password, "password")?;
    let mut state = state.write().expect("state lock poisoned");
    let teacher = state
        .organization
        .teachers
        .values()
        .find(|teacher| {
            teacher.status == "active"
                && (teacher
                    .email
                    .as_deref()
                    .is_some_and(|email| email.eq_ignore_ascii_case(&identifier))
                    || teacher.phone.as_deref() == Some(identifier.as_str()))
        })
        .cloned()
        .ok_or_else(invalid_credentials)?;
    let credential = state
        .auth
        .credentials
        .get_mut(&teacher.id)
        .ok_or_else(invalid_credentials)?;
    if credential.password_hash != demo_password_hash(&password) {
        return Err(invalid_credentials());
    }
    let issued_at = now();
    let expires_at = issued_at + chrono::Duration::hours(12);
    credential.last_login_at = Some(issued_at);
    let must_change_password = credential.must_change_password;
    let token = format!("dev-{}-{}", teacher.id.simple(), Uuid::new_v4().simple());
    state.auth.sessions.insert(
        token.clone(),
        AuthSessionRecord {
            token: token.clone(),
            teacher_id: teacher.id,
            school_id: teacher.school_id,
            status: "active".to_string(),
            issued_at,
            expires_at,
            last_seen_at: issued_at,
        },
    );
    let current_school = current_school(&state, teacher.school_id)?;
    let default_classroom = default_classroom(&state, teacher.id, current_school.id);
    Ok(Json(AuthResponse {
        access_token: token,
        token_type: "Bearer".to_string(),
        expires_at,
        teacher: teacher_summary(&teacher),
        current_school,
        default_classroom,
        must_change_password,
    }))
}

async fn me(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<CurrentSessionResponse>, ApiError> {
    let token = bearer_token(&headers)?;
    let mut state = state.write().expect("state lock poisoned");
    let session = active_session_mut(&mut state, token)?.clone();
    let teacher = state
        .organization
        .teachers
        .get(&session.teacher_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("teacher"))?;
    let current_school = current_school(&state, teacher.school_id)?;
    let default_classroom = default_classroom(&state, teacher.id, current_school.id);
    Ok(Json(CurrentSessionResponse {
        session: session_summary(&session),
        teacher: teacher_summary(&teacher),
        current_school,
        default_classroom,
    }))
}

async fn refresh_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<AuthResponse>, ApiError> {
    let token = bearer_token(&headers)?;
    let mut state = state.write().expect("state lock poisoned");
    let old_session = active_session_mut(&mut state, token)?.clone();
    let teacher = state
        .organization
        .teachers
        .get(&old_session.teacher_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("teacher"))?;
    let issued_at = now();
    let expires_at = issued_at + chrono::Duration::hours(12);
    let new_token = format!("dev-{}-{}", teacher.id.simple(), Uuid::new_v4().simple());
    if let Some(session) = state.auth.sessions.get_mut(token) {
        session.status = "revoked".to_string();
        session.last_seen_at = issued_at;
    }
    state.auth.sessions.insert(
        new_token.clone(),
        AuthSessionRecord {
            token: new_token.clone(),
            teacher_id: teacher.id,
            school_id: teacher.school_id,
            status: "active".to_string(),
            issued_at,
            expires_at,
            last_seen_at: issued_at,
        },
    );
    let credential = state
        .auth
        .credentials
        .get(&teacher.id)
        .ok_or_else(invalid_credentials)?;
    let current_school = current_school(&state, teacher.school_id)?;
    let default_classroom = default_classroom(&state, teacher.id, current_school.id);
    Ok(Json(AuthResponse {
        access_token: new_token,
        token_type: "Bearer".to_string(),
        expires_at,
        teacher: teacher_summary(&teacher),
        current_school,
        default_classroom,
        must_change_password: credential.must_change_password,
    }))
}

async fn logout(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<LogoutResponse>, ApiError> {
    let token = bearer_token(&headers)?;
    let mut state = state.write().expect("state lock poisoned");
    let session = active_session_mut(&mut state, token)?;
    session.status = "revoked".to_string();
    session.last_seen_at = now();
    Ok(Json(LogoutResponse {
        status: "logged_out".to_string(),
    }))
}

fn active_session_mut<'a>(
    state: &'a mut crate::api::AppState,
    token: &str,
) -> Result<&'a mut AuthSessionRecord, ApiError> {
    let session = state
        .auth
        .sessions
        .get_mut(token)
        .ok_or_else(|| ApiError::unauthorized("登录会话无效"))?;
    if session.status != "active" {
        return Err(ApiError::unauthorized("登录会话已失效"));
    }
    if session.expires_at <= now() {
        session.status = "expired".to_string();
        return Err(ApiError::unauthorized("登录会话已过期"));
    }
    session.last_seen_at = now();
    Ok(session)
}

fn authenticated_teacher_from_token(
    state: &mut crate::api::AppState,
    token: &str,
) -> Result<AuthenticatedTeacher, ApiError> {
    #[cfg(test)]
    if token == TEST_BEARER_TOKEN {
        let teacher = state
            .organization
            .teachers
            .get(&state.organization.current_teacher_id)
            .ok_or_else(|| ApiError::not_found("teacher"))?;
        let school_id = teacher
            .school_id
            .unwrap_or(state.organization.current_school_id);
        return Ok(AuthenticatedTeacher {
            teacher_id: teacher.id,
            school_id,
        });
    }

    let session = active_session_mut(state, token)?.clone();
    let teacher = state
        .organization
        .teachers
        .get(&session.teacher_id)
        .ok_or_else(|| ApiError::not_found("teacher"))?;
    if teacher.status != "active" {
        return Err(ApiError::unauthorized("教师账号已停用"));
    }
    let school_id = teacher
        .school_id
        .or(session.school_id)
        .unwrap_or(state.organization.current_school_id);
    Ok(AuthenticatedTeacher {
        teacher_id: teacher.id,
        school_id,
    })
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)
        .ok_or_else(|| ApiError::unauthorized("缺少 Authorization 头"))?
        .to_str()
        .map_err(|_| ApiError::unauthorized("Authorization 头格式不合法"))?;
    value
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| ApiError::unauthorized("Authorization 必须使用 Bearer token"))
}

fn current_school(
    state: &crate::api::AppState,
    school_id: Option<Uuid>,
) -> Result<crate::api::organization::SchoolRecord, ApiError> {
    let school_id = school_id.unwrap_or(state.organization.current_school_id);
    state
        .organization
        .schools
        .get(&school_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("school"))
}

fn default_classroom(
    state: &crate::api::AppState,
    teacher_id: Uuid,
    school_id: Uuid,
) -> Option<crate::api::organization::ClassroomRecord> {
    state
        .organization
        .classrooms
        .values()
        .find(|classroom| {
            classroom.school_id == school_id
                && classroom.teacher_id == Some(teacher_id)
                && classroom.status == "active"
        })
        .cloned()
}

fn teacher_summary(teacher: &crate::api::organization::TeacherRecord) -> TeacherAuthSummary {
    TeacherAuthSummary {
        id: teacher.id,
        school_id: teacher.school_id,
        name: teacher.name.clone(),
        email: teacher.email.clone(),
        phone: teacher.phone.clone(),
        role: teacher.role.clone(),
        status: teacher.status.clone(),
    }
}

fn session_summary(session: &AuthSessionRecord) -> AuthSessionSummary {
    AuthSessionSummary {
        teacher_id: session.teacher_id,
        school_id: session.school_id,
        issued_at: session.issued_at,
        expires_at: session.expires_at,
        last_seen_at: session.last_seen_at,
    }
}

fn invalid_credentials() -> ApiError {
    ApiError::unauthorized("账号或密码错误")
}

fn required_trimmed(value: String, field: &'static str) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ApiError::validation(field, "不能为空"));
    }
    Ok(value.to_string())
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

    async fn json_request_on(
        app: axum::Router,
        method: &str,
        uri: &str,
        body: Value,
        token: Option<&str>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json");
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = app
            .oneshot(request.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    async fn get_json_on(app: axum::Router, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
        let mut request = Request::builder().method("GET").uri(uri);
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = app
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    #[tokio::test]
    async fn logs_in_and_reads_current_session() {
        let app = test_app();
        let (status, login) = json_request_on(
            app.clone(),
            "POST",
            "/api/auth/login",
            json!({
                "identifier": "teacher@example.com",
                "password": "password123"
            }),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{login}");
        assert_eq!(login["token_type"], "Bearer");
        assert_eq!(login["teacher"]["name"], "王老师");

        let token = login["access_token"].as_str().unwrap();
        let (status, me) = get_json_on(app, "/api/auth/me", Some(token)).await;
        assert_eq!(status, StatusCode::OK, "{me}");
        assert_eq!(me["teacher"]["email"], "teacher@example.com");
        assert_eq!(me["current_school"]["name"], "Kindleaf 幼儿园");
    }

    #[tokio::test]
    async fn rejects_invalid_credentials() {
        let (status, body) = json_request_on(
            test_app(),
            "POST",
            "/api/auth/login",
            json!({
                "identifier": "teacher@example.com",
                "password": "wrong-password"
            }),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"]["code"], "UNAUTHORIZED");
    }

    #[tokio::test]
    async fn refreshes_and_revokes_sessions() {
        let app = test_app();
        let (_, login) = json_request_on(
            app.clone(),
            "POST",
            "/api/auth/login",
            json!({
                "identifier": "teacher@example.com",
                "password": "password123"
            }),
            None,
        )
        .await;
        let token = login["access_token"].as_str().unwrap();
        let (status, refreshed) = json_request_on(
            app.clone(),
            "POST",
            "/api/auth/refresh",
            Value::Null,
            Some(token),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{refreshed}");
        let new_token = refreshed["access_token"].as_str().unwrap();
        assert_ne!(token, new_token);

        let (status, body) = get_json_on(app.clone(), "/api/auth/me", Some(token)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");

        let (status, logout) = json_request_on(
            app.clone(),
            "POST",
            "/api/auth/logout",
            Value::Null,
            Some(new_token),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{logout}");
        assert_eq!(logout["status"], "logged_out");

        let (status, body) = get_json_on(app, "/api/auth/me", Some(new_token)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    }
}
