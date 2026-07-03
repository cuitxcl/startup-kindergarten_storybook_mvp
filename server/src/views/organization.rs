use serde::Serialize;

use crate::api::organization::{ClassroomRecord, SchoolRecord, TeacherRecord};

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
