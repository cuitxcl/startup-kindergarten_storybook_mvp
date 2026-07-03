use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthStore {
    pub credentials: BTreeMap<Uuid, TeacherCredentialRecord>,
    pub sessions: BTreeMap<String, AuthSessionRecord>,
    pub email_verification_codes: BTreeMap<String, EmailVerificationCodeRecord>,
}

impl AuthStore {
    pub fn empty() -> Self {
        Self {
            credentials: BTreeMap::new(),
            sessions: BTreeMap::new(),
            email_verification_codes: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TeacherCredentialRecord {
    pub teacher_id: Uuid,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub must_change_password: bool,
    pub last_login_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthSessionRecord {
    pub token: String,
    pub teacher_id: Uuid,
    pub school_id: Option<Uuid>,
    pub status: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmailVerificationCodeRecord {
    pub email: String,
    pub code: String,
    pub purpose: String,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
}

pub fn password_hash(password: &str) -> String {
    format!("local-hash:{password}")
}
