use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct VisualConsistencyStore {
    pub character_profiles: BTreeMap<Uuid, CharacterProfileRecord>,
    pub parent_character_profiles: BTreeMap<Uuid, ParentCharacterProfileRecord>,
    pub prop_profiles: BTreeMap<Uuid, PropProfileRecord>,
    pub reference_images: BTreeMap<Uuid, ReferenceImageRecord>,
    pub storybook_roles: BTreeMap<Uuid, StorybookRoleRecord>,
    pub page_visual_subjects: BTreeMap<Uuid, Vec<PageVisualSubjectRecord>>,
}

impl VisualConsistencyStore {
    pub fn demo() -> Self {
        Self {
            character_profiles: BTreeMap::new(),
            parent_character_profiles: BTreeMap::new(),
            prop_profiles: BTreeMap::new(),
            reference_images: BTreeMap::new(),
            storybook_roles: BTreeMap::new(),
            page_visual_subjects: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CharacterProfileRecord {
    pub id: Uuid,
    pub child_id: Uuid,
    pub version: i32,
    pub name: String,
    pub nickname: Option<String>,
    pub age_group: String,
    pub gender_expression: Option<String>,
    pub hair: String,
    pub skin_tone: Option<String>,
    pub face_shape: Option<String>,
    pub body_proportion: String,
    pub outfit_top: Option<String>,
    pub outfit_bottom: Option<String>,
    pub shoe: Option<String>,
    pub accessory: Option<String>,
    pub signature_colors: Vec<String>,
    pub interest_elements: Vec<String>,
    pub visual_must_keep: Vec<String>,
    pub negative_rules: Vec<String>,
    pub source_photo_id: Option<Uuid>,
    pub active_reference_image_id: Option<Uuid>,
    pub status: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ParentCharacterProfileRecord {
    pub id: Uuid,
    pub parent_id: Uuid,
    pub child_id: Option<Uuid>,
    pub version: i32,
    pub role: String,
    pub name: String,
    pub hair: Option<String>,
    pub skin_tone: Option<String>,
    pub face_shape: Option<String>,
    pub body_proportion: Option<String>,
    pub outfit_top: Option<String>,
    pub outfit_bottom: Option<String>,
    pub accessory: Option<String>,
    pub visual_must_keep: Vec<String>,
    pub negative_rules: Vec<String>,
    pub active_reference_image_id: Option<Uuid>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PropProfileRecord {
    pub id: Uuid,
    pub storybook_id: Option<Uuid>,
    pub child_id: Option<Uuid>,
    pub name: String,
    pub shape: Option<String>,
    pub primary_color: Option<String>,
    pub secondary_color: Option<String>,
    pub material_style: Option<String>,
    pub size_description: Option<String>,
    pub visual_must_keep: Vec<String>,
    pub negative_rules: Vec<String>,
    pub active_reference_image_id: Option<Uuid>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReferenceImageRecord {
    pub id: Uuid,
    pub subject_type: String,
    pub character_profile_id: Option<Uuid>,
    pub parent_character_profile_id: Option<Uuid>,
    pub prop_profile_id: Option<Uuid>,
    pub image_asset_id: Uuid,
    pub source_task_id: Option<Uuid>,
    pub style_id: String,
    pub review_status: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StorybookRoleRecord {
    pub id: Uuid,
    pub storybook_id: Uuid,
    pub role_key: String,
    pub role_type: String,
    pub display_name: String,
    pub child_id: Option<Uuid>,
    pub character_profile_id: Option<Uuid>,
    pub parent_character_profile_id: Option<Uuid>,
    pub prop_profile_id: Option<Uuid>,
    pub replacement_source_role_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PageVisualSubjectRecord {
    pub id: Uuid,
    pub storybook_page_id: Uuid,
    pub subject_type: String,
    pub storybook_role_id: Option<Uuid>,
    pub prop_profile_id: Option<Uuid>,
    pub importance: String,
    pub placement_hint: Option<String>,
    pub created_at: DateTime<Utc>,
}
