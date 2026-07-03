use serde::Serialize;
use uuid::Uuid;

use crate::api::images::{
    GenerationCostLogRecord, ImageAssetRecord, ImageGenerationOutputRecord,
    ImageGenerationTaskRecord,
};

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct ImageTaskDetailResponse {
    #[serde(flatten)]
    pub task: ImageGenerationTaskRecord,
    pub outputs: Vec<ImageOutputWithAsset>,
    pub cost: Option<GenerationCostLogRecord>,
}

#[derive(Debug, Serialize)]
pub struct ImageOutputWithAsset {
    #[serde(flatten)]
    pub output: ImageGenerationOutputRecord,
    pub image_asset: ImageAssetRecord,
}

#[derive(Debug, Serialize)]
pub struct StorybookImageTaskResponse {
    pub task_id: Uuid,
    pub task_type: String,
    pub status: String,
    pub page_task_count: usize,
    pub skipped_page_ids: Vec<Uuid>,
}
