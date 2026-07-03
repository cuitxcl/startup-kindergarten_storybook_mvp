use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrganizationStore {
    pub current_school_id: Uuid,
    pub current_teacher_id: Uuid,
    pub schools: BTreeMap<Uuid, SchoolRecord>,
    pub classrooms: BTreeMap<Uuid, ClassroomRecord>,
    pub teachers: BTreeMap<Uuid, TeacherRecord>,
}

impl OrganizationStore {
    pub fn empty() -> Self {
        Self {
            current_school_id: Uuid::nil(),
            current_teacher_id: Uuid::nil(),
            schools: BTreeMap::new(),
            classrooms: BTreeMap::new(),
            teachers: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SchoolRecord {
    pub id: Uuid,
    pub name: String,
    pub code: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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
