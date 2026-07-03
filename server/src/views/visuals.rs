use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct ReplaceRolesResponse {
    pub storybook_id: Uuid,
    pub changed_roles: Vec<String>,
    pub affected_page_ids: Vec<Uuid>,
    pub image_policy_result: String,
}
