use std::sync::{Arc, RwLock};

use crate::models::{auth, children, content, delivery, images, organization, storybooks, visuals};

pub type SharedState = Arc<RwLock<AppState>>;

#[derive(Clone, Debug)]
pub struct AppState {
    pub auth: auth::AuthStore,
    pub children: children::ChildrenStore,
    pub content: content::ContentStore,
    pub delivery: delivery::DeliveryStore,
    pub images: images::ImageGenerationStore,
    pub organization: organization::OrganizationStore,
    pub storybooks: storybooks::StorybookStore,
    pub visuals: visuals::VisualConsistencyStore,
}

impl AppState {
    pub fn empty() -> Self {
        Self {
            auth: auth::AuthStore::empty(),
            children: children::ChildrenStore::empty(),
            content: content::ContentStore::empty(),
            delivery: delivery::DeliveryStore::empty(),
            images: images::ImageGenerationStore::empty(),
            organization: organization::OrganizationStore::empty(),
            storybooks: storybooks::StorybookStore::empty(),
            visuals: visuals::VisualConsistencyStore::empty(),
        }
    }

    #[cfg(test)]
    pub fn test_fixture() -> Self {
        use serde_json::json;

        use crate::{
            commons::{now, support::test_uuid},
            models::{
                auth::{AuthSessionRecord, TeacherCredentialRecord, password_hash},
                children::{ChildPhotoRecord, ChildRecord, ParentRecord},
                content::{CasePageRecord, CaseStorybookRecord, StoryTemplateRecord},
                organization::{ClassroomRecord, SchoolRecord, TeacherRecord},
            },
        };

        let created_at = now();
        let school_id = test_uuid(1);
        let teacher_id = test_uuid(2);
        let classroom_id = test_uuid(3);
        let child_id = test_uuid(10);
        let parent_id = test_uuid(11);
        let photo_id = test_uuid(12);
        let asset_id = test_uuid(13);
        let template_id = test_uuid(30);
        let case_id = test_uuid(31);
        let cover_image_asset_id = test_uuid(32);

        let mut state = Self::empty();
        state.organization.current_school_id = school_id;
        state.organization.current_teacher_id = teacher_id;
        state.organization.schools.insert(
            school_id,
            SchoolRecord {
                id: school_id,
                name: "Kindleaf 幼儿园".to_string(),
                code: Some("kindleaf-test".to_string()),
                status: "active".to_string(),
                created_at,
                updated_at: created_at,
            },
        );
        state.organization.teachers.insert(
            teacher_id,
            TeacherRecord {
                id: teacher_id,
                school_id: Some(school_id),
                name: "王老师".to_string(),
                email: Some("teacher@example.test".to_string()),
                phone: None,
                role: "school_admin".to_string(),
                status: "active".to_string(),
                created_at,
                updated_at: created_at,
            },
        );
        state.organization.classrooms.insert(
            classroom_id,
            ClassroomRecord {
                id: classroom_id,
                school_id,
                teacher_id: Some(teacher_id),
                name: "小一班".to_string(),
                grade_level: Some("小班".to_string()),
                status: "active".to_string(),
                created_at,
                updated_at: created_at,
            },
        );
        state.auth.credentials.insert(
            teacher_id,
            TeacherCredentialRecord {
                teacher_id,
                password_hash: password_hash("password123"),
                must_change_password: false,
                last_login_at: None,
            },
        );
        state.children.parents.insert(
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
        state.children.children.insert(
            child_id,
            ChildRecord {
                id: child_id,
                school_id: Some(school_id),
                classroom_id: Some(classroom_id),
                primary_teacher_id: teacher_id,
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
        state.children.photos.insert(
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
        state.content.story_templates.insert(
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
        state.content.case_storybooks.insert(
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
        state.content.case_pages.insert(
            case_id,
            vec![
                CasePageRecord {
                    page_number: 1,
                    page_role: "cover".to_string(),
                    page_title: Some("一起分享更开心".to_string()),
                    body_text: "今天，小朋友们一起搭积木。".to_string(),
                    prompt_text: None,
                    image_url: Some("https://example.test/cases/share-cover.png".to_string()),
                },
                CasePageRecord {
                    page_number: 2,
                    page_role: "story".to_string(),
                    page_title: Some("轮流玩玩具".to_string()),
                    body_text: "大家发现，轮流玩的时候，每个人都能开心参与。".to_string(),
                    prompt_text: Some("你想和谁一起分享玩具呢？".to_string()),
                    image_url: Some("https://example.test/cases/share-page-2.png".to_string()),
                },
            ],
        );
        state.auth.sessions.insert(
            "test-token".to_string(),
            AuthSessionRecord {
                token: "test-token".to_string(),
                teacher_id,
                school_id: Some(school_id),
                status: "active".to_string(),
                issued_at: created_at,
                expires_at: created_at + chrono::Duration::hours(12),
                last_seen_at: created_at,
            },
        );
        state
    }
}

pub fn shared_state() -> SharedState {
    Arc::new(RwLock::new(AppState::empty()))
}
