use serde::Serialize;
use uuid::Uuid;

use crate::api::content::{CasePageRecord, CaseStorybookRecord};

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct CaseDetailResponse {
    #[serde(flatten)]
    pub case_storybook: CaseStorybookRecord,
    pub pages: Vec<CasePageRecord>,
}

#[derive(Debug, Serialize)]
pub struct CloneCaseResponse {
    pub storybook_id: Uuid,
    pub source_case_id: Uuid,
    pub title: String,
    pub content_type: String,
    pub theme: String,
    pub teaching_goal: String,
    pub story_status: String,
    pub illustration_status: String,
    pub status: String,
    pub derivation_type: String,
}
