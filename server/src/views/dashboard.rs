use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct TeacherDashboardResponse {
    pub teacher: TeacherSummary,
    pub current_school: SchoolSummary,
    pub current_classroom: Option<ClassroomSummary>,
    pub work_counts: WorkCounts,
    pub recent_storybooks: Vec<ContentItemSummary>,
    pub recommended_cases: Vec<RecommendedCaseSummary>,
}

#[derive(Debug, Serialize)]
pub struct TeacherSummary {
    pub id: Uuid,
    pub name: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct SchoolSummary {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct ClassroomSummary {
    pub id: Uuid,
    pub name: String,
    pub child_count: usize,
}

#[derive(Debug, Serialize)]
pub struct WorkCounts {
    pub story_generating: usize,
    pub needs_illustration: usize,
    pub ready_to_export: usize,
    pub children_missing_profile: usize,
    pub running_image_tasks: usize,
}

#[derive(Debug, Serialize, Clone)]
pub struct ContentItemSummary {
    pub storybook_id: Uuid,
    pub title: String,
    pub content_type: String,
    pub theme: String,
    pub child: Option<ChildSummary>,
    pub story_status: String,
    pub illustration_status: String,
    pub status: String,
    pub export_status: String,
    pub share_status: String,
    pub share_scope: String,
    pub page_count: usize,
    pub pending_image_task_count: usize,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ChildSummary {
    pub id: Uuid,
    pub name: String,
    pub classroom_id: Option<Uuid>,
    pub profile_completion_status: String,
}

#[derive(Debug, Serialize)]
pub struct RecommendedCaseSummary {
    pub id: Uuid,
    pub title: String,
    pub content_type: String,
    pub theme: String,
    pub teaching_goal: String,
    pub target_age_group: Option<String>,
    pub cover_image_asset_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct ActivityItem {
    pub id: String,
    pub activity_type: String,
    pub occurred_at: DateTime<Utc>,
    pub summary: String,
    pub metadata_json: Value,
}
