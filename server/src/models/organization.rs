use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::commons::{demo_uuid, now};

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
