use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct DeliveryStore {
    pub exports: BTreeMap<Uuid, StorybookExportRecord>,
    pub share_links: BTreeMap<Uuid, StorybookShareLinkRecord>,
}

impl DeliveryStore {
    pub fn empty() -> Self {
        Self {
            exports: BTreeMap::new(),
            share_links: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct StorybookExportRecord {
    pub id: Uuid,
    pub idempotency_key: Option<String>,
    #[serde(skip_serializing)]
    pub idempotency_fingerprint_json: Option<serde_json::Value>,
    pub storybook_id: Uuid,
    pub export_type: String,
    pub include_teacher_tips: bool,
    pub page_size: String,
    pub quality: String,
    pub allow_text_only: bool,
    pub status: String,
    pub asset_id: Option<Uuid>,
    pub download_url: Option<String>,
    pub failure_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StorybookShareLinkRecord {
    pub id: Uuid,
    pub idempotency_key: Option<String>,
    #[serde(skip_serializing)]
    pub idempotency_fingerprint_json: Option<serde_json::Value>,
    pub storybook_id: Uuid,
    pub share_scope: String,
    pub share_token: String,
    pub url: String,
    pub qrcode_asset_id: Option<Uuid>,
    pub anonymize_child_name: bool,
    pub anonymize_parent_info: bool,
    pub status: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
