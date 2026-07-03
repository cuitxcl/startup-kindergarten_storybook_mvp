use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ChildrenStore {
    pub children: BTreeMap<Uuid, ChildRecord>,
    pub parents: BTreeMap<Uuid, ParentRecord>,
    pub photos: BTreeMap<Uuid, ChildPhotoRecord>,
    pub parent_intake_links: BTreeMap<Uuid, ParentIntakeLinkRecord>,
    pub parent_intakes: BTreeMap<Uuid, ParentIntakeRecord>,
}

impl ChildrenStore {
    pub fn empty() -> Self {
        Self {
            children: BTreeMap::new(),
            parents: BTreeMap::new(),
            photos: BTreeMap::new(),
            parent_intake_links: BTreeMap::new(),
            parent_intakes: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ChildRecord {
    pub id: Uuid,
    pub school_id: Option<Uuid>,
    pub classroom_id: Option<Uuid>,
    pub primary_teacher_id: Uuid,
    pub primary_parent_id: Option<Uuid>,
    pub name: String,
    pub nickname: Option<String>,
    pub age: Option<i32>,
    pub age_group: Option<String>,
    pub gender_expression: Option<String>,
    pub hair: Option<String>,
    pub skin_tone: Option<String>,
    pub usual_outfit: Option<String>,
    pub favorite_color: Option<String>,
    pub interest_tags: Vec<String>,
    pub teacher_observation_tags: Vec<String>,
    pub teaching_focus: Option<String>,
    pub profile_completion_status: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ParentRecord {
    pub id: Uuid,
    pub name: String,
    pub relationship_to_child: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChildPhotoRecord {
    pub id: Uuid,
    pub child_id: Uuid,
    pub image_asset_id: Uuid,
    pub photo_type: String,
    pub is_primary: bool,
    pub consent_status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ParentIntakeRecord {
    pub id: Uuid,
    pub invite_token: String,
    pub parent_name: String,
    pub relationship_to_child: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub child_name: String,
    pub child_payload: IntakeChildPayload,
    pub parent_character_profile: Option<ParentCharacterProfileInput>,
    pub photo_asset_ids: Vec<Uuid>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub accepted_child_id: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ParentIntakeLinkRecord {
    pub id: Uuid,
    pub invite_token: String,
    pub child_id: Option<Uuid>,
    pub classroom_id: Option<Uuid>,
    pub status: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct IntakeChildPayload {
    pub name: String,
    pub nickname: Option<String>,
    pub age: Option<i32>,
    pub age_group: Option<String>,
    pub gender_expression: Option<String>,
    pub hair: Option<String>,
    pub skin_tone: Option<String>,
    pub usual_outfit: Option<String>,
    pub favorite_color: Option<String>,
    #[serde(default)]
    pub interest_tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ParentCharacterProfileInput {
    pub role: String,
    pub hair: Option<String>,
    pub outfit_top: Option<String>,
    #[serde(default)]
    pub visual_must_keep: Vec<String>,
}
