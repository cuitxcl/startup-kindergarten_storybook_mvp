use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: usize,
}

#[derive(Debug, Serialize, Clone)]
pub struct SharedLibraryItem {
    pub storybook_id: Uuid,
    pub title: String,
    pub content_type: String,
    pub theme: String,
    pub teaching_goal: Option<String>,
    pub share_scope: String,
    pub page_count: usize,
    pub anonymized: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct SubmitPlatformReviewResponse {
    pub storybook_id: Uuid,
    pub share_scope: String,
    pub review_status: String,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ShareLinkListItem {
    pub id: Uuid,
    pub storybook_id: Uuid,
    pub share_scope: String,
    pub url: String,
    pub qrcode_asset_id: Option<Uuid>,
    pub anonymize_child_name: bool,
    pub anonymize_parent_info: bool,
    pub status: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
