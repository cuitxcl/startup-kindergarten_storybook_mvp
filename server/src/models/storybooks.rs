use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::api::storybooks::{DeepSeekStoryProvider, StoryProviderKind};

#[derive(Clone, Debug)]
pub struct StorybookStore {
    pub storybooks: BTreeMap<Uuid, StorybookRecord>,
    pub pages: BTreeMap<Uuid, Vec<StorybookPageRecord>>,
    pub story_provider: StoryProviderKind,
}

impl StorybookStore {
    pub fn demo() -> Self {
        Self {
            storybooks: BTreeMap::new(),
            pages: BTreeMap::new(),
            story_provider: StoryProviderKind::DeepSeek(DeepSeekStoryProvider::default()),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct StorybookRecord {
    pub id: Uuid,
    pub school_id: Option<Uuid>,
    pub teacher_id: Uuid,
    pub child_id: Option<Uuid>,
    pub story_template_id: Option<Uuid>,
    pub case_storybook_id: Option<Uuid>,
    pub source_storybook_id: Option<Uuid>,
    pub title: String,
    pub content_type: String,
    pub theme: String,
    pub teaching_goal: Option<String>,
    pub style_id: Option<String>,
    pub reading_age_group: Option<String>,
    pub generation_config_json: Value,
    pub role_manifest_json: Value,
    pub story_status: String,
    pub illustration_status: String,
    pub status: String,
    pub export_status: String,
    pub share_status: String,
    pub share_scope: String,
    pub derivation_type: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub exported_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StorybookPageRecord {
    pub id: Uuid,
    pub storybook_id: Uuid,
    pub page_number: i32,
    pub page_role: String,
    pub page_title: Option<String>,
    pub body_text: String,
    pub prompt_text: Option<String>,
    pub teacher_tip: Option<String>,
    pub scene_spec_json: Option<Value>,
    pub scene_spec_status: String,
    pub page_visual_subjects_json: Option<Value>,
    pub current_image_asset_id: Option<Uuid>,
    pub current_image_task_id: Option<Uuid>,
    pub illustration_status: String,
    pub is_locked: bool,
    pub content_source: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorybookContentType {
    PlainStorybook,
    CustomStorybook,
    EmotionStory,
    HomeCommunicationCard,
    GrowthMilestoneBook,
}

impl StorybookContentType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::PlainStorybook => "plain_storybook",
            Self::CustomStorybook => "custom_storybook",
            Self::EmotionStory => "emotion_story",
            Self::HomeCommunicationCard => "home_communication_card",
            Self::GrowthMilestoneBook => "growth_milestone_book",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorybookDerivationType {
    Original,
    FromPlainStorybook,
    FromCustomStorybook,
    FromSharedLibrary,
}

impl StorybookDerivationType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::FromPlainStorybook => "from_plain_storybook",
            Self::FromCustomStorybook => "from_custom_storybook",
            Self::FromSharedLibrary => "from_shared_library",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleReplacementInput {
    pub role_key: String,
    pub child_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleReplacementMode {
    ReuseFirst,
}

impl RoleReplacementMode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ReuseFirst => "reuse_first",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplaceRolesRequest {
    pub replacements: Vec<RoleReplacementInput>,
    pub regeneration_mode: RoleReplacementMode,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplaceRolesResponse {
    pub storybook_id: Uuid,
    pub status: String,
    pub replacement_mode: String,
    pub derived_storybook_id: Uuid,
    pub changed_roles: Vec<String>,
    pub regenerated_pages: Vec<i32>,
}
