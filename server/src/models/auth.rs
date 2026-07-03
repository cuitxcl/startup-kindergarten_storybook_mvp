use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::models::organization::OrganizationStore;

#[derive(Clone, Debug)]
pub struct AuthStore {
    pub credentials: BTreeMap<Uuid, TeacherCredentialRecord>,
    pub sessions: BTreeMap<String, AuthSessionRecord>,
}

impl AuthStore {
    pub fn demo(organization: &OrganizationStore) -> Self {
        let mut credentials = BTreeMap::new();
        for teacher in organization.teachers.values() {
            credentials.insert(
                teacher.id,
                TeacherCredentialRecord {
                    teacher_id: teacher.id,
                    password_hash: demo_password_hash("password123"),
                    must_change_password: false,
                    last_login_at: None,
                },
            );
        }
        Self {
            credentials,
            sessions: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TeacherCredentialRecord {
    pub teacher_id: Uuid,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub must_change_password: bool,
    pub last_login_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AuthSessionRecord {
    pub token: String,
    pub teacher_id: Uuid,
    pub school_id: Option<Uuid>,
    pub status: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

pub fn demo_password_hash(password: &str) -> String {
    format!("demo-hash:{password}")
}
