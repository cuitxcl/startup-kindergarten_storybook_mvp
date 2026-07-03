use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::api::{demo_uuid, now};

#[derive(Clone, Debug)]
pub struct ChildrenStore {
    pub children: BTreeMap<Uuid, ChildRecord>,
    pub parents: BTreeMap<Uuid, ParentRecord>,
    pub photos: BTreeMap<Uuid, ChildPhotoRecord>,
    pub parent_intake_links: BTreeMap<Uuid, ParentIntakeLinkRecord>,
    pub parent_intakes: BTreeMap<Uuid, ParentIntakeRecord>,
}

impl ChildrenStore {
    pub fn demo(organization: &crate::api::organization::OrganizationStore) -> Self {
        let created_at = now();
        let child_id = demo_uuid(10);
        let parent_id = demo_uuid(11);
        let photo_id = demo_uuid(12);
        let asset_id = demo_uuid(13);

        let mut parents = BTreeMap::new();
        parents.insert(
            parent_id,
            ParentRecord {
                id: parent_id,
                name: "张女士".to_string(),
                relationship_to_child: Some("妈妈".to_string()),
                phone: Some("13800000000".to_string()),
                email: None,
                status: "active".to_string(),
                created_at,
                updated_at: created_at,
            },
        );

        let mut children = BTreeMap::new();
        children.insert(
            child_id,
            ChildRecord {
                id: child_id,
                school_id: Some(organization.current_school_id),
                classroom_id: organization.classrooms.keys().next().copied(),
                primary_teacher_id: organization.current_teacher_id,
                primary_parent_id: Some(parent_id),
                name: "乐乐".to_string(),
                nickname: Some("乐乐".to_string()),
                age: Some(5),
                age_group: Some("5-6".to_string()),
                gender_expression: Some("男孩".to_string()),
                hair: Some("黑色短发".to_string()),
                skin_tone: Some("自然肤色".to_string()),
                usual_outfit: Some("黄色卫衣".to_string()),
                favorite_color: Some("黄色".to_string()),
                interest_tags: vec!["积木".to_string(), "画画".to_string()],
                teacher_observation_tags: vec!["愿意合作".to_string()],
                teaching_focus: Some("练习轮流和分享".to_string()),
                profile_completion_status: "complete".to_string(),
                status: "active".to_string(),
                created_at,
                updated_at: created_at,
            },
        );

        let mut photos = BTreeMap::new();
        photos.insert(
            photo_id,
            ChildPhotoRecord {
                id: photo_id,
                child_id,
                image_asset_id: asset_id,
                photo_type: "portrait".to_string(),
                is_primary: true,
                consent_status: "granted".to_string(),
                created_at,
            },
        );

        Self {
            children,
            parents,
            photos,
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
