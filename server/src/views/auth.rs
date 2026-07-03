use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::api::organization::{ClassroomRecord, SchoolRecord};

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_at: DateTime<Utc>,
    pub teacher: TeacherAuthSummary,
    pub current_school: SchoolRecord,
    pub default_classroom: Option<ClassroomRecord>,
    pub must_change_password: bool,
}

#[derive(Debug, Serialize)]
pub struct CurrentSessionResponse {
    pub session: AuthSessionSummary,
    pub teacher: TeacherAuthSummary,
    pub current_school: SchoolRecord,
    pub default_classroom: Option<ClassroomRecord>,
}

#[derive(Debug, Serialize)]
pub struct AuthSessionSummary {
    pub teacher_id: Uuid,
    pub school_id: Option<Uuid>,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct TeacherAuthSummary {
    pub id: Uuid,
    pub school_id: Option<Uuid>,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub role: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct LogoutResponse {
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct EmailVerificationResponse {
    pub status: String,
    pub email: String,
    pub expires_at: DateTime<Utc>,
}
