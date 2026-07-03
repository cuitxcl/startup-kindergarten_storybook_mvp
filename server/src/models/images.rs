use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::api::images::{ImageProviderKind, SeedreamImageProvider};

#[derive(Clone, Debug)]
pub struct ImageGenerationStore {
    pub upload_intents: BTreeMap<Uuid, UploadIntentRecord>,
    pub assets: BTreeMap<Uuid, ImageAssetRecord>,
    pub tasks: BTreeMap<Uuid, ImageGenerationTaskRecord>,
    pub outputs: BTreeMap<Uuid, ImageGenerationOutputRecord>,
    pub review_events: BTreeMap<Uuid, ImageReviewEventRecord>,
    pub cost_logs: BTreeMap<Uuid, GenerationCostLogRecord>,
    pub image_provider: ImageProviderKind,
}

impl ImageGenerationStore {
    pub fn demo() -> Self {
        Self {
            upload_intents: BTreeMap::new(),
            assets: BTreeMap::new(),
            tasks: BTreeMap::new(),
            outputs: BTreeMap::new(),
            review_events: BTreeMap::new(),
            cost_logs: BTreeMap::new(),
            image_provider: ImageProviderKind::Seedream(SeedreamImageProvider),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct UploadIntentRecord {
    pub id: Uuid,
    pub asset_type: String,
    pub filename: String,
    pub mime_type: String,
    pub file_size: i64,
    pub checksum: Option<String>,
    pub upload_url: String,
    pub storage_key: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImageAssetRecord {
    pub id: Uuid,
    pub asset_type: String,
    pub storage_url: String,
    pub storage_key: Option<String>,
    pub mime_type: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub file_size: Option<i64>,
    pub checksum: Option<String>,
    pub review_result: Option<String>,
    pub metadata_json: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImageGenerationTaskRecord {
    pub id: Uuid,
    pub idempotency_key: Option<String>,
    pub task_type: String,
    pub parent_task_id: Option<Uuid>,
    pub retry_of_task_id: Option<Uuid>,
    pub school_id: Option<Uuid>,
    pub teacher_id: Option<Uuid>,
    pub storybook_id: Option<Uuid>,
    pub storybook_page_id: Option<Uuid>,
    pub character_profile_id: Option<Uuid>,
    pub character_profile_version: Option<i32>,
    pub reference_image_id: Option<Uuid>,
    pub style_id: String,
    pub prompt_template_version: String,
    pub scene_spec_json: Option<Value>,
    pub input_snapshot_json: Value,
    pub raw_prompt_text: Option<String>,
    pub provider_name: Option<String>,
    pub model_name: Option<String>,
    pub provider_request_id: Option<String>,
    pub status: String,
    pub failure_reason: Option<String>,
    pub retry_count: i32,
    pub max_retries: i32,
    pub queued_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImageGenerationOutputRecord {
    pub id: Uuid,
    pub task_id: Uuid,
    pub image_asset_id: Uuid,
    pub candidate_index: i32,
    pub is_selected: bool,
    pub review_status: String,
    pub quality_notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImageReviewEventRecord {
    pub id: Uuid,
    pub task_id: Uuid,
    pub output_id: Option<Uuid>,
    pub reviewer_teacher_id: Option<Uuid>,
    pub review_action: String,
    pub reason_code: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GenerationCostLogRecord {
    pub id: Uuid,
    pub task_id: Uuid,
    pub school_id: Option<Uuid>,
    pub teacher_id: Option<Uuid>,
    pub storybook_id: Option<Uuid>,
    pub storybook_page_id: Option<Uuid>,
    pub provider_name: String,
    pub model_name: String,
    pub input_units: Option<Decimal>,
    pub output_units: Option<Decimal>,
    pub input_cost: Decimal,
    pub output_cost: Decimal,
    pub total_cost: Decimal,
    pub currency: String,
    pub billed_units_json: Value,
    pub created_at: DateTime<Utc>,
}
