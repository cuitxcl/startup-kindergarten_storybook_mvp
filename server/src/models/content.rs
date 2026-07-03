use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::commons::{demo_uuid, now};

#[derive(Clone, Debug)]
pub struct ContentStore {
    pub case_storybooks: BTreeMap<Uuid, CaseStorybookRecord>,
    pub story_templates: BTreeMap<Uuid, StoryTemplateRecord>,
    pub case_pages: BTreeMap<Uuid, Vec<CasePageRecord>>,
}

impl ContentStore {
    pub fn demo() -> Self {
        let created_at = now();
        let template_id = demo_uuid(30);
        let case_id = demo_uuid(31);
        let cover_image_asset_id = demo_uuid(32);
        let mut story_templates = BTreeMap::new();
        story_templates.insert(
            template_id,
            StoryTemplateRecord {
                id: template_id,
                title: "分享合作六页结构".to_string(),
                content_type: "plain_storybook".to_string(),
                theme: "分享合作".to_string(),
                teaching_goal: "帮助孩子理解轮流和合作".to_string(),
                target_age_group: Some("5-6".to_string()),
                page_count: 6,
                template_outline_json: json!({
                    "pages": [
                        {"page_role": "cover"},
                        {"page_role": "story"},
                        {"page_role": "story"},
                        {"page_role": "story"},
                        {"page_role": "story"},
                        {"page_role": "closing"}
                    ]
                }),
                default_role_manifest_json: json!({
                    "protagonist": {"role_type": "default_character", "display_name": "小朋友"}
                }),
                status: "active".to_string(),
                created_at,
                updated_at: created_at,
            },
        );

        let mut case_storybooks = BTreeMap::new();
        case_storybooks.insert(
            case_id,
            CaseStorybookRecord {
                id: case_id,
                storybook_id: None,
                template_id: Some(template_id),
                title: "一起分享更开心".to_string(),
                content_type: "plain_storybook".to_string(),
                theme: "分享合作".to_string(),
                teaching_goal: "帮助孩子理解轮流和分享".to_string(),
                target_age_group: Some("5-6".to_string()),
                cover_image_asset_id: Some(cover_image_asset_id),
                page_count: 6,
                status: "published".to_string(),
                sort_order: 10,
                created_at,
                updated_at: created_at,
            },
        );

        let mut case_pages = BTreeMap::new();
        case_pages.insert(
            case_id,
            vec![
                CasePageRecord {
                    page_number: 1,
                    page_role: "cover".to_string(),
                    page_title: Some("一起分享更开心".to_string()),
                    body_text: "今天，小朋友们一起搭积木。".to_string(),
                    prompt_text: None,
                    image_url: Some("https://example.com/cases/share-cover.png".to_string()),
                },
                CasePageRecord {
                    page_number: 2,
                    page_role: "story".to_string(),
                    page_title: Some("轮流玩玩具".to_string()),
                    body_text: "大家发现，轮流玩的时候，每个人都能开心参与。".to_string(),
                    prompt_text: Some("你想和谁一起分享玩具呢？".to_string()),
                    image_url: Some("https://example.com/cases/share-page-2.png".to_string()),
                },
            ],
        );

        Self {
            case_storybooks,
            story_templates,
            case_pages,
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
