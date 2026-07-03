use serde::Serialize;
use uuid::Uuid;

use crate::api::storybooks::{StorybookPageRecord, StorybookRecord};

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct GenerateStorybookResponse {
    pub storybook: StorybookRecord,
    pub story_task: StoryTaskSummary,
}

#[derive(Debug, Serialize)]
pub struct StoryTaskSummary {
    pub provider: String,
    pub status: String,
    pub poll_url: String,
}

#[derive(Debug, Serialize)]
pub struct StorybookDetailResponse {
    #[serde(flatten)]
    pub storybook: StorybookRecord,
    pub child: Option<ChildSummary>,
    pub source_case: Option<CaseSummary>,
    pub pages: Vec<StorybookPageRecord>,
}

#[derive(Debug, Serialize)]
pub struct ChildSummary {
    pub id: Uuid,
    pub name: String,
    pub profile_completion_status: String,
}

#[derive(Debug, Serialize)]
pub struct CaseSummary {
    pub id: Uuid,
    pub title: String,
    pub theme: String,
}
