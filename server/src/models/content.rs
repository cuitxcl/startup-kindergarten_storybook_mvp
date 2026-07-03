use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ContentStore {
    pub case_storybooks: BTreeMap<Uuid, CaseStorybookRecord>,
    pub story_templates: BTreeMap<Uuid, StoryTemplateRecord>,
    pub case_pages: BTreeMap<Uuid, Vec<CasePageRecord>>,
}

impl ContentStore {
    pub fn empty() -> Self {
        Self {
            case_storybooks: BTreeMap::new(),
            story_templates: BTreeMap::new(),
            case_pages: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CaseStorybookRecord {
    pub id: Uuid,
    pub storybook_id: Option<Uuid>,
    pub template_id: Option<Uuid>,
    pub title: String,
    pub content_type: String,
    pub theme: String,
    pub teaching_goal: String,
    pub target_age_group: Option<String>,
    pub cover_image_asset_id: Option<Uuid>,
    pub page_count: i32,
    pub status: String,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StoryTemplateRecord {
    pub id: Uuid,
    pub title: String,
    pub content_type: String,
    pub theme: String,
    pub teaching_goal: String,
    pub target_age_group: Option<String>,
    pub page_count: i32,
    pub template_outline_json: Value,
    pub default_role_manifest_json: Value,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CasePageRecord {
    pub page_number: i32,
    pub page_role: String,
    pub page_title: Option<String>,
    pub body_text: String,
    pub prompt_text: Option<String>,
    pub image_url: Option<String>,
}
