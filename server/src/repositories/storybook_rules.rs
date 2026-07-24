use sea_orm::DbErr;

use crate::models::{Storybook, StorybookRole, StorybookStatus, StorybookType, Visibility};

pub fn parse_storybook_type(value: &str) -> StorybookType {
    match value {
        "custom" => StorybookType::Custom,
        _ => StorybookType::Plain,
    }
}

pub fn parse_storybook_status(value: &str) -> StorybookStatus {
    match value {
        "plan_pending" => StorybookStatus::PlanPending,
        "roles_pending" => StorybookStatus::RolesPending,
        "editing" => StorybookStatus::Editing,
        "image_pending" => StorybookStatus::ImagePending,
        "exportable" => StorybookStatus::Exportable,
        "submitted" => StorybookStatus::Submitted,
        "listed" => StorybookStatus::Listed,
        _ => StorybookStatus::Draft,
    }
}

pub fn parse_visibility(value: &str) -> Visibility {
    match value {
        "workspace" => Visibility::Workspace,
        "market_submission" => Visibility::MarketSubmission,
        "market_listed" => Visibility::MarketListed,
        _ => Visibility::Private,
    }
}

pub fn storybook_status_name(value: &StorybookStatus) -> &'static str {
    match value {
        StorybookStatus::Draft => "draft",
        StorybookStatus::PlanPending => "plan_pending",
        StorybookStatus::RolesPending => "roles_pending",
        StorybookStatus::Editing => "editing",
        StorybookStatus::ImagePending => "image_pending",
        StorybookStatus::Exportable => "exportable",
        StorybookStatus::Submitted => "submitted",
        StorybookStatus::Listed => "listed",
    }
}

pub fn storybook_type_name(value: &StorybookType) -> &'static str {
    match value {
        StorybookType::Plain => "plain",
        StorybookType::Custom => "custom",
    }
}

pub fn visibility_name(value: &Visibility) -> &'static str {
    match value {
        Visibility::Private => "private",
        Visibility::Workspace => "workspace",
        Visibility::MarketSubmission => "market_submission",
        Visibility::MarketListed => "market_listed",
    }
}

pub fn ensure_status_transition(from: &StorybookStatus, to: &StorybookStatus) -> Result<(), DbErr> {
    if is_allowed_status_transition(from, to) {
        Ok(())
    } else {
        Err(DbErr::Custom(format!(
            "非法绘本状态流转：{} -> {}",
            storybook_status_name(from),
            storybook_status_name(to)
        )))
    }
}

pub fn ensure_deliverable_ready(book: &Storybook) -> Result<(), DbErr> {
    if book.pages.is_empty() {
        return Err(DbErr::Custom(
            "绘本至少需要一个分页才能标记可交付".to_string(),
        ));
    }
    if book.roles.is_empty() {
        return Err(DbErr::Custom(
            "绘本至少需要一个角色或道具设定才能标记可交付".to_string(),
        ));
    }
    if book.pages.iter().any(|page| page.status == "generating") {
        return Err(DbErr::Custom(
            "仍有插图正在生成，完成后才能标记可交付".to_string(),
        ));
    }
    Ok(())
}

pub fn role_edit_requires_page_regeneration(before: &StorybookRole, after: &StorybookRole) -> bool {
    (before.needs_consistency || after.needs_consistency)
        && role_visual_signature_changed(before, after)
}

fn role_visual_signature_changed(before: &StorybookRole, after: &StorybookRole) -> bool {
    before.name.trim() != after.name.trim()
        || before.role_type.trim() != after.role_type.trim()
        || before.appearance.trim() != after.appearance.trim()
        || before.story_function.trim() != after.story_function.trim()
        || before.needs_consistency != after.needs_consistency
}

fn is_allowed_status_transition(from: &StorybookStatus, to: &StorybookStatus) -> bool {
    use StorybookStatus::{
        Draft, Editing, Exportable, ImagePending, Listed, PlanPending, RolesPending, Submitted,
    };

    if from == to {
        return true;
    }

    matches!(
        (from, to),
        (Draft, PlanPending)
            | (PlanPending, RolesPending)
            | (RolesPending, Editing)
            | (Editing, ImagePending)
            | (Editing, Exportable)
            | (ImagePending, Exportable)
            | (ImagePending, Editing)
            | (Exportable, Editing)
            | (Exportable, Submitted)
            | (Submitted, Listed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{StorybookPage, StorybookRole};
    use uuid::Uuid;

    #[test]
    fn storybook_status_transition_allows_expected_path() {
        assert!(is_allowed_status_transition(
            &StorybookStatus::Draft,
            &StorybookStatus::PlanPending
        ));
        assert!(is_allowed_status_transition(
            &StorybookStatus::PlanPending,
            &StorybookStatus::RolesPending
        ));
        assert!(is_allowed_status_transition(
            &StorybookStatus::RolesPending,
            &StorybookStatus::Editing
        ));
        assert!(is_allowed_status_transition(
            &StorybookStatus::Editing,
            &StorybookStatus::ImagePending
        ));
        assert!(is_allowed_status_transition(
            &StorybookStatus::ImagePending,
            &StorybookStatus::Exportable
        ));
        assert!(is_allowed_status_transition(
            &StorybookStatus::Exportable,
            &StorybookStatus::Submitted
        ));
        assert!(is_allowed_status_transition(
            &StorybookStatus::Submitted,
            &StorybookStatus::Listed
        ));
    }

    #[test]
    fn role_visual_edit_requires_page_regeneration_for_consistent_roles() {
        let before = StorybookRole {
            id: Uuid::new_v4(),
            name: "小汽车".to_string(),
            role_type: "protagonist".to_string(),
            appearance: "红色玩具车".to_string(),
            story_function: "带孩子练习轮流".to_string(),
            needs_consistency: true,
            reference_image_url: Some("https://example.test/car.png".to_string()),
            reference_image_prompt: Some("红色玩具车，圆角".to_string()),
            reference_status: "ready".to_string(),
        };
        let mut after = before.clone();

        after.appearance = "蓝色玩具车".to_string();
        assert!(role_edit_requires_page_regeneration(&before, &after));

        after = before.clone();
        after.reference_image_prompt = Some("更柔和的线条".to_string());
        assert!(!role_edit_requires_page_regeneration(&before, &after));

        after = before.clone();
        after.needs_consistency = false;
        assert!(role_edit_requires_page_regeneration(&before, &after));
    }

    #[test]
    fn storybook_status_transition_allows_editing_recovery() {
        assert!(is_allowed_status_transition(
            &StorybookStatus::Exportable,
            &StorybookStatus::Editing
        ));
        assert!(is_allowed_status_transition(
            &StorybookStatus::ImagePending,
            &StorybookStatus::Editing
        ));
        assert!(is_allowed_status_transition(
            &StorybookStatus::Editing,
            &StorybookStatus::Exportable
        ));
    }

    #[test]
    fn storybook_status_transition_rejects_skips_and_backwards_jumps() {
        assert!(!is_allowed_status_transition(
            &StorybookStatus::Draft,
            &StorybookStatus::Listed
        ));
        assert!(!is_allowed_status_transition(
            &StorybookStatus::PlanPending,
            &StorybookStatus::Exportable
        ));
        assert!(!is_allowed_status_transition(
            &StorybookStatus::Submitted,
            &StorybookStatus::Editing
        ));
    }

    #[test]
    fn deliverable_check_requires_content_and_no_running_pages() {
        let mut book = test_storybook();
        assert!(ensure_deliverable_ready(&book).is_ok());

        book.pages[0].status = "generating".to_string();
        assert!(ensure_deliverable_ready(&book).is_err());

        book.pages.clear();
        assert!(ensure_deliverable_ready(&book).is_err());

        book = test_storybook();
        book.roles.clear();
        assert!(ensure_deliverable_ready(&book).is_err());
    }

    fn test_storybook() -> Storybook {
        Storybook {
            id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            title: "一起玩小汽车".to_string(),
            storybook_type: StorybookType::Plain,
            status: StorybookStatus::Editing,
            visibility: Visibility::Private,
            source: "blank".to_string(),
            source_title: None,
            target_child_id: None,
            creator_name: "林老师".to_string(),
            updated_at: "今天 09:00".to_string(),
            age_group: "4-5 岁".to_string(),
            use_scene: "规则引导".to_string(),
            teaching_goal: "学习轮流与分享".to_string(),
            cover_tone: "温暖、清楚".to_string(),
            pages: vec![StorybookPage {
                id: Uuid::new_v4(),
                page_number: 1,
                title: "第一页".to_string(),
                body: "内容".to_string(),
                illustration_prompt: "提示".to_string(),
                status: "ready".to_string(),
            }],
            roles: vec![StorybookRole {
                id: Uuid::new_v4(),
                name: "林老师".to_string(),
                role_type: "teacher".to_string(),
                appearance: "温和、稳定".to_string(),
                story_function: "引导孩子轮流等待".to_string(),
                needs_consistency: true,
                reference_image_url: None,
                reference_image_prompt: None,
                reference_status: "not_started".to_string(),
            }],
        }
    }
}
