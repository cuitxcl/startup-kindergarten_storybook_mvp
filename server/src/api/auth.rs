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
#[cfg(test)]
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::models::auth::password_hash;
pub use crate::models::auth::{
    AuthSessionRecord, AuthStore, EmailVerificationCodeRecord, TeacherCredentialRecord,
};
use crate::models::organization::{ClassroomRecord, SchoolRecord, TeacherRecord};
use crate::views::auth::{
    AuthResponse, AuthSessionSummary, CurrentSessionResponse, EmailVerificationResponse,
    LogoutResponse, TeacherAuthSummary,
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/auth/register/send-code", post(send_registration_code))
        .route("/auth/register", post(register))
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

#[derive(Debug, Deserialize)]
pub struct SendRegistrationCodeRequest {
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub verification_code: String,
    pub teacher_name: String,
    pub school_name: String,
    pub classroom_name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AuthenticatedTeacher {
    pub teacher_id: Uuid,
    pub school_id: Uuid,
}

#[cfg(test)]
pub const TEST_BEARER_TOKEN: &str = "test-token";

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

#[cfg(test)]
pub fn issue_test_token(state: &Arc<RwLock<crate::api::AppState>>) -> String {
    let mut state = state.write().expect("state lock poisoned");
    let teacher = state
        .organization
        .teachers
        .get(&state.organization.current_teacher_id)
        .cloned()
        .expect("test teacher exists");
    let issued_at = now();
    let expires_at = issued_at + chrono::Duration::hours(12);
    let token = format!("test-{}", Uuid::new_v4().simple());
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

async fn send_registration_code(
    State(state): State<SharedState>,
    Json(payload): Json<SendRegistrationCodeRequest>,
) -> Result<Json<EmailVerificationResponse>, ApiError> {
    let email = normalize_email(payload.email)?;
    let mut state = state.write().expect("state lock poisoned");
    if active_teacher_email_exists(&state, &email) {
        return Err(ApiError::state_conflict("该邮箱已注册"));
    }
    let code = verification_code();
    let expires_at = now() + chrono::Duration::minutes(10);
    state.auth.email_verification_codes.insert(
        email.clone(),
        EmailVerificationCodeRecord {
            email: email.clone(),
            code: code.clone(),
            purpose: "teacher_register".to_string(),
            expires_at,
            consumed_at: None,
        },
    );
    println!("邮箱注册验证码 {email}: {code}");
    Ok(Json(EmailVerificationResponse {
        status: "sent".to_string(),
        email,
        expires_at,
    }))
}

async fn register(
    State(state): State<SharedState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let email = normalize_email(payload.email)?;
    let password = required_trimmed(payload.password, "password")?;
    if password.chars().count() < 8 {
        return Err(ApiError::validation("password", "密码至少 8 位"));
    }
    let verification_code = required_trimmed(payload.verification_code, "verification_code")?;
    let teacher_name = required_trimmed(payload.teacher_name, "teacher_name")?;
    let school_name = required_trimmed(payload.school_name, "school_name")?;
    let classroom_name = payload.classroom_name.and_then(normalize_optional_owned);

    let mut state = state.write().expect("state lock poisoned");
    if active_teacher_email_exists(&state, &email) {
        return Err(ApiError::state_conflict("该邮箱已注册"));
    }
    consume_registration_code(&mut state, &email, &verification_code)?;

    let created_at = now();
    let school_id = Uuid::new_v4();
    let teacher_id = Uuid::new_v4();
    state.organization.schools.insert(
        school_id,
        SchoolRecord {
            id: school_id,
            name: school_name,
            code: None,
            status: "active".to_string(),
            created_at,
            updated_at: created_at,
        },
    );
    state.organization.teachers.insert(
        teacher_id,
        TeacherRecord {
            id: teacher_id,
            school_id: Some(school_id),
            name: teacher_name,
            email: Some(email.clone()),
            phone: None,
            role: "school_admin".to_string(),
            status: "active".to_string(),
            created_at,
            updated_at: created_at,
        },
    );
    if state.organization.current_school_id.is_nil() {
        state.organization.current_school_id = school_id;
    }
    if state.organization.current_teacher_id.is_nil() {
        state.organization.current_teacher_id = teacher_id;
    }
    if let Some(classroom_name) = classroom_name {
        let classroom_id = Uuid::new_v4();
        state.organization.classrooms.insert(
            classroom_id,
            ClassroomRecord {
                id: classroom_id,
                school_id,
                teacher_id: Some(teacher_id),
                name: classroom_name,
                grade_level: None,
                status: "active".to_string(),
                created_at,
                updated_at: created_at,
            },
        );
    }
    state.auth.credentials.insert(
        teacher_id,
        TeacherCredentialRecord {
            teacher_id,
            password_hash: password_hash(&password),
            must_change_password: false,
            last_login_at: Some(created_at),
        },
    );
    let teacher = state
        .organization
        .teachers
        .get(&teacher_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("teacher"))?;
    issue_auth_response(&mut state, teacher)
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
    if credential.password_hash != password_hash(&password) {
        return Err(invalid_credentials());
    }
    credential.last_login_at = Some(now());
    issue_auth_response(&mut state, teacher)
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
    let new_token = format!("session-{}-{}", teacher.id.simple(), Uuid::new_v4().simple());
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

fn issue_auth_response(
    state: &mut crate::api::AppState,
    teacher: TeacherRecord,
) -> Result<Json<AuthResponse>, ApiError> {
    let issued_at = now();
    let expires_at = issued_at + chrono::Duration::hours(12);
    let token = format!("session-{}-{}", teacher.id.simple(), Uuid::new_v4().simple());
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
    let credential = state
        .auth
        .credentials
        .get(&teacher.id)
        .ok_or_else(invalid_credentials)?;
    let current_school = current_school(state, teacher.school_id)?;
    let default_classroom = default_classroom(state, teacher.id, current_school.id);
    Ok(Json(AuthResponse {
        access_token: token,
        token_type: "Bearer".to_string(),
        expires_at,
        teacher: teacher_summary(&teacher),
        current_school,
        default_classroom,
        must_change_password: credential.must_change_password,
    }))
}

fn active_teacher_email_exists(state: &crate::api::AppState, email: &str) -> bool {
    state.organization.teachers.values().any(|teacher| {
        teacher.status == "active"
            && teacher
                .email
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(email))
    })
}

fn normalize_email(value: String) -> Result<String, ApiError> {
    let email = required_trimmed(value, "email")?.to_lowercase();
    if !email.contains('@') || email.starts_with('@') || email.ends_with('@') {
        return Err(ApiError::validation("email", "邮箱格式不合法"));
    }
    Ok(email)
}

fn verification_code() -> String {
    let raw = Uuid::new_v4().as_u128() % 1_000_000;
    format!("{raw:06}")
}

fn consume_registration_code(
    state: &mut crate::api::AppState,
    email: &str,
    code: &str,
) -> Result<(), ApiError> {
    let record = state
        .auth
        .email_verification_codes
        .get_mut(email)
        .ok_or_else(|| ApiError::validation("verification_code", "请先获取邮箱验证码"))?;
    if record.purpose != "teacher_register" || record.email != email {
        return Err(ApiError::validation("verification_code", "验证码不匹配"));
    }
    if record.consumed_at.is_some() {
        return Err(ApiError::validation("verification_code", "验证码已使用"));
    }
    if record.expires_at <= now() {
        return Err(ApiError::validation("verification_code", "验证码已过期"));
    }
    if record.code != code {
        return Err(ApiError::validation("verification_code", "验证码不正确"));
    }
    record.consumed_at = Some(now());
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
                "identifier": "teacher@example.test",
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
        assert_eq!(me["teacher"]["email"], "teacher@example.test");
        assert_eq!(me["current_school"]["name"], "Kindleaf 幼儿园");
    }

    #[tokio::test]
    async fn rejects_invalid_credentials() {
        let (status, body) = json_request_on(
            test_app(),
            "POST",
            "/api/auth/login",
            json!({
                "identifier": "teacher@example.test",
                "password": "wrong-password"
            }),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"]["code"], "UNAUTHORIZED");
    }

    #[tokio::test]
    async fn registers_teacher_with_email_code() {
        let state = Arc::new(RwLock::new(AppState::empty()));
        let app = router(state.clone());
        let (status, sent) = json_request_on(
            app.clone(),
            "POST",
            "/api/auth/register/send-code",
            json!({ "email": "new-teacher@example.test" }),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(sent["status"], "sent");

        let code = state
            .read()
            .unwrap()
            .auth
            .email_verification_codes
            .get("new-teacher@example.test")
            .unwrap()
            .code
            .clone();
        let (status, body) = json_request_on(
            app.clone(),
            "POST",
            "/api/auth/register",
            json!({
                "email": "new-teacher@example.test",
                "password": "password123",
                "verification_code": code,
                "teacher_name": "新老师",
                "school_name": "新园所",
                "classroom_name": "一班"
            }),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["teacher"]["email"], "new-teacher@example.test");
        assert_eq!(body["current_school"]["name"], "新园所");
        assert!(body["access_token"].as_str().unwrap().starts_with("session-"));

        let token = body["access_token"].as_str().unwrap();
        let (status, me) = get_json_on(app, "/api/auth/me", Some(token)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(me["teacher"]["name"], "新老师");
    }

    #[tokio::test]
    async fn refreshes_and_revokes_sessions() {
        let app = test_app();
        let (_, login) = json_request_on(
            app.clone(),
            "POST",
            "/api/auth/login",
            json!({
                "identifier": "teacher@example.test",
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
