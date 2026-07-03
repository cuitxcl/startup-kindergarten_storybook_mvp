use serde::Serialize;
use uuid::Uuid;

use crate::api::children::{ChildPhotoRecord, ChildRecord, ParentRecord};

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct ChildDetailResponse {
    #[serde(flatten)]
    pub child: ChildRecord,
    pub classroom: Option<ClassroomSummary>,
    pub primary_parent: Option<ParentRecord>,
    pub photos: Vec<ChildPhotoRecord>,
}

#[derive(Debug, Serialize)]
pub struct ClassroomSummary {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct AcceptParentIntakeResponse {
    pub intake_id: Uuid,
    pub child_id: Uuid,
    pub parent_id: Uuid,
    pub status: String,
}
