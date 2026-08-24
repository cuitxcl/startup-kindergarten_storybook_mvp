use sea_orm::DbErr;

use crate::models::{
    Storybook, StorybookPageQuality, StorybookQualityCheck, StorybookQualityReport,
    StorybookQualityStatus, StorybookRole, StorybookStatus, StorybookType, UNEXPECTED_ANIMAL_NAMES,
    Visibility,
};

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

pub fn normalize_page_status(value: &str) -> &'static str {
    match value.trim() {
        "ready" => "ready",
        "generating" => "generating",
        "failed" => "failed",
        "needs_regeneration" => "needs_regeneration",
        _ => "draft",
    }
}

pub fn normalize_reference_status(value: &str) -> &'static str {
    match value.trim() {
        "generating" => "generating",
        "ready" => "ready",
        "failed" => "failed",
        "needs_regeneration" => "needs_regeneration",
        _ => "not_started",
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
    if book.pages.iter().any(|page| page.status == "draft") {
        return Err(DbErr::Custom(
            "仍有分页插图未生成，完成后才能标记可交付".to_string(),
        ));
    }
    if book.pages.iter().any(|page| page.status == "failed") {
        return Err(DbErr::Custom(
            "存在插图生成失败的分页，重新生成后才能标记可交付".to_string(),
        ));
    }
    if book
        .pages
        .iter()
        .any(|page| page.status == "needs_regeneration")
    {
        return Err(DbErr::Custom(
            "存在待重新生成插图的分页，完成后才能标记可交付".to_string(),
        ));
    }
    if book.teacher_review_status != "confirmed" {
        return Err(DbErr::Custom("老师复核确认后才能标记可交付".to_string()));
    }
    Ok(())
}

pub fn ensure_delivery_access_ready(book: &Storybook) -> Result<(), DbErr> {
    ensure_deliverable_ready(book)?;
    let quality = storybook_quality_report(book);
    if quality.status == StorybookQualityStatus::Blocked {
        return Err(DbErr::Custom(
            "生成质量检查存在阻断项，请先修正后再导出或创建分享链接".to_string(),
        ));
    }
    Ok(())
}

pub fn ensure_visibility_matches_status(
    status: &StorybookStatus,
    visibility: &Visibility,
) -> Result<(), DbErr> {
    match visibility {
        Visibility::MarketSubmission if status != &StorybookStatus::Submitted => Err(
            DbErr::Custom("market_submission 可见性只能由投稿流程设置".to_string()),
        ),
        Visibility::MarketListed if status != &StorybookStatus::Listed => Err(DbErr::Custom(
            "market_listed 可见性只能由上架流程设置".to_string(),
        )),
        Visibility::Private | Visibility::Workspace
            if matches!(status, StorybookStatus::Submitted | StorybookStatus::Listed) =>
        {
            Err(DbErr::Custom(
                "已投稿或已上架绘本不能通过普通更新修改可见性".to_string(),
            ))
        }
        _ => Ok(()),
    }
}

pub fn ensure_teacher_review_ready(book: &Storybook) -> Result<(), DbErr> {
    let quality = storybook_quality_report(book);
    if quality.status == StorybookQualityStatus::Blocked {
        return Err(DbErr::Custom(
            "生成质量检查存在阻断项，请先修正后再确认老师复核".to_string(),
        ));
    }
    Ok(())
}

pub fn storybook_quality_report(book: &Storybook) -> StorybookQualityReport {
    let mut checks = Vec::new();
    let mut page_reports = Vec::new();
    let consistency_roles: Vec<_> = book
        .roles
        .iter()
        .filter(|role| role_needs_reference(book, role))
        .collect();

    checks.push(quality_check(
        "structure",
        "内容结构",
        if book.pages.is_empty() || book.roles.is_empty() {
            StorybookQualityStatus::Blocked
        } else {
            StorybookQualityStatus::Passed
        },
        if book.pages.is_empty() {
            "还没有分页内容，无法判断绘本质量。"
        } else if book.roles.is_empty() {
            "还没有角色或道具设定，后续插图容易不一致。"
        } else {
            "已包含分页内容和角色/道具设定。"
        },
    ));

    let missing_reference_roles: Vec<_> = consistency_roles
        .iter()
        .filter(|role| role.reference_image_url.is_none())
        .map(|role| role.name.clone())
        .collect();
    let stale_reference_roles: Vec<_> = consistency_roles
        .iter()
        .filter(|role| role.reference_image_url.is_some() && role.reference_status != "ready")
        .map(|role| role.name.clone())
        .collect();
    checks.push(quality_check(
        "role_references",
        "角色参考图",
        if consistency_roles.is_empty() {
            StorybookQualityStatus::Passed
        } else if missing_reference_roles.is_empty() && stale_reference_roles.is_empty() {
            StorybookQualityStatus::Passed
        } else {
            StorybookQualityStatus::NeedsReview
        },
        if consistency_roles.is_empty() {
            "没有跨页重复出现的角色或道具需要参考图；只出现一次的事物无需参考图。".to_string()
        } else if missing_reference_roles.is_empty() && stale_reference_roles.is_empty() {
            "跨页重复出现的角色/道具都已有参考图；只出现一次的事物无需参考图。".to_string()
        } else if missing_reference_roles.is_empty() {
            format!(
                "以下角色/道具已有参考图但建议更新：{}；当前已有图仍可用于生成。",
                stale_reference_roles.join("、")
            )
        } else if stale_reference_roles.is_empty() {
            format!(
                "以下跨页出现的角色/道具还需要先生成参考图：{}。",
                missing_reference_roles.join("、")
            )
        } else {
            format!(
                "缺少参考图：{}；建议更新参考图：{}。",
                missing_reference_roles.join("、"),
                stale_reference_roles.join("、")
            )
        },
    ));

    let mut blocked_pages = 0;
    let mut review_pages = 0;
    for page in &book.pages {
        let mut issues = Vec::new();
        let mut suggestions = Vec::new();
        let combined_text = format!("{} {} {}", page.title, page.body, page.illustration_prompt);

        if page.status == "generating" {
            issues.push("插图仍在生成中。".to_string());
        } else if page.status == "failed" {
            issues.push("插图生成失败，需要重新生成。".to_string());
        } else if page.status == "needs_regeneration" {
            issues.push("插图需要重新生成，完成后才能交付。".to_string());
        }

        let page_roles: Vec<_> = book
            .roles
            .iter()
            .filter(|role| text_contains(&combined_text, &role.name))
            .collect();
        let page_mentions_confirmed_role = !page_roles.is_empty();
        if page_mentions_confirmed_role
            && !page_roles
                .iter()
                .any(|role| text_contains(&page.illustration_prompt, &role.name))
        {
            issues.push("插图描述没有明确带入已确认角色/道具名称。".to_string());
        }

        for role in &page_roles {
            if text_contains(&combined_text, &role.name)
                && !text_contains(&page.illustration_prompt, &role.name)
            {
                issues.push(format!(
                    "正文出现了「{}」，但插图描述没有同步这个名称。",
                    role.name
                ));
            }
            if text_contains(&page.illustration_prompt, &role.name)
                && role.reference_image_url.is_none()
                && !role_appearance_has_prompt_hint(role, &page.illustration_prompt)
            {
                suggestions.push(format!(
                    "「{}」的插图描述可以补充外观关键词，便于跨页保持一致。",
                    role.name
                ));
            }
            let mention_count = page.illustration_prompt.matches(&role.name).count();
            // 插图已生成且页面完成时，生图风险已经落定，不再提示点名次数；
            // 页面被标记为待重绘后建议会重新出现，因为下次生成仍要看这段描述。
            if mention_count > 2 && page.status != "ready" {
                suggestions.push(format!(
                    "「{}」在插图描述中被提到 {} 次，模型容易把同一角色画成多个，建议合并为一组动作。",
                    role.name, mention_count
                ));
            }
        }

        let named_roles_in_prompt = page_roles
            .iter()
            .filter(|role| text_contains(&page.illustration_prompt, &role.name))
            .count();
        if named_roles_in_prompt > 4 && page.status != "ready" {
            suggestions.push(format!(
                "本页插图描述涉及 {} 个有名角色/道具，画面容易割裂，建议精简到 4 个以内，其余用「队伍延伸到画面外」带过。",
                named_roles_in_prompt
            ));
        }

        let role_names = book
            .roles
            .iter()
            .map(|role| role.name.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        for animal in UNEXPECTED_ANIMAL_NAMES {
            if text_contains(&page.illustration_prompt, animal)
                && !text_contains(&role_names, animal)
            {
                issues.push(format!(
                    "插图描述出现了未确认形象「{}」，请先修正提示词或重新生成。",
                    animal
                ));
            }
        }

        let status = if issues.iter().any(|issue| {
            issue.contains("没有明确带入")
                || issue.contains("生成失败")
                || issue.contains("需要重新生成")
                || issue.contains("仍在生成中")
                || issue.contains("未确认形象")
        }) {
            blocked_pages += 1;
            StorybookQualityStatus::Blocked
        } else if !issues.is_empty() || !suggestions.is_empty() {
            review_pages += 1;
            StorybookQualityStatus::NeedsReview
        } else {
            StorybookQualityStatus::Passed
        };

        page_reports.push(StorybookPageQuality {
            page_id: page.id,
            page_number: page.page_number,
            status,
            issues,
            suggestions,
        });
    }

    checks.push(quality_check(
        "page_prompts",
        "分页一致性",
        if blocked_pages > 0 {
            StorybookQualityStatus::Blocked
        } else if review_pages > 0 {
            StorybookQualityStatus::NeedsReview
        } else if page_reports.is_empty() {
            StorybookQualityStatus::Blocked
        } else {
            StorybookQualityStatus::Passed
        },
        if page_reports.is_empty() {
            "还没有可检查的分页。".to_string()
        } else if blocked_pages > 0 {
            format!("{blocked_pages} 个分页存在阻断问题，需要先修正提示词或重新生成。")
        } else if review_pages > 0 {
            format!("{review_pages} 个分页需要老师复核或补充描述。")
        } else {
            "分页描述已带入角色/道具名称，没有发现明显一致性问题。".to_string()
        },
    ));

    let status = if checks
        .iter()
        .any(|check| check.status == StorybookQualityStatus::Blocked)
    {
        StorybookQualityStatus::Blocked
    } else if checks
        .iter()
        .any(|check| check.status == StorybookQualityStatus::NeedsReview)
    {
        StorybookQualityStatus::NeedsReview
    } else {
        StorybookQualityStatus::Passed
    };
    let summary = match status {
        StorybookQualityStatus::Passed => "系统检查通过，建议老师做最终阅读确认。".to_string(),
        StorybookQualityStatus::NeedsReview => {
            "系统发现需要复核的项目，建议老师确认后再导出或分享。".to_string()
        }
        StorybookQualityStatus::Blocked => {
            "系统发现阻断问题，请先修正角色、提示词或重新生成。".to_string()
        }
    };

    StorybookQualityReport {
        status,
        summary,
        checks,
        pages: page_reports,
    }
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

fn quality_check(
    key: &str,
    label: &str,
    status: StorybookQualityStatus,
    message: impl Into<String>,
) -> StorybookQualityCheck {
    StorybookQualityCheck {
        key: key.to_string(),
        label: label.to_string(),
        status,
        message: message.into(),
    }
}

fn text_contains(text: &str, value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && text.contains(value)
}

fn role_needs_reference(book: &Storybook, role: &StorybookRole) -> bool {
    role.needs_consistency && role_page_usage_count(book, role) >= 2
}

fn role_page_usage_count(book: &Storybook, role: &StorybookRole) -> usize {
    book.pages
        .iter()
        .filter(|page| {
            let text = format!("{} {} {}", page.title, page.body, page.illustration_prompt);
            text_contains(&text, &role.name)
        })
        .count()
}

fn role_appearance_has_prompt_hint(role: &StorybookRole, prompt: &str) -> bool {
    role.appearance
        .split(['，', ',', '、', '；', ';', ' '])
        .map(str::trim)
        .filter(|part| part.chars().count() >= 2)
        .take(3)
        .any(|part| prompt.contains(part))
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
            selected_image_variant_id: None,
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
        align_quality_fixture(&mut book);
        book.teacher_review_status = "confirmed".to_string();
        assert!(ensure_deliverable_ready(&book).is_ok());

        book.pages[0].status = "generating".to_string();
        assert!(ensure_deliverable_ready(&book).is_err());

        book = test_storybook();
        align_quality_fixture(&mut book);
        book.teacher_review_status = "confirmed".to_string();
        book.pages[0].status = "failed".to_string();
        assert!(ensure_deliverable_ready(&book).is_err());

        book.pages.clear();
        assert!(ensure_deliverable_ready(&book).is_err());

        book = test_storybook();
        align_quality_fixture(&mut book);
        book.teacher_review_status = "confirmed".to_string();
        book.roles.clear();
        assert!(ensure_deliverable_ready(&book).is_err());
    }

    #[test]
    fn deliverable_check_requires_review_and_current_images() {
        let mut book = test_storybook();
        align_quality_fixture(&mut book);

        assert!(ensure_deliverable_ready(&book).is_err());

        book.teacher_review_status = "confirmed".to_string();
        book.pages[0].status = "needs_regeneration".to_string();
        assert!(ensure_deliverable_ready(&book).is_err());
    }

    #[test]
    fn status_normalizers_fallback_unknown_values() {
        assert_eq!(normalize_page_status("ready"), "ready");
        assert_eq!(normalize_page_status("complete"), "draft");
        assert_eq!(normalize_reference_status("failed"), "failed");
        assert_eq!(normalize_reference_status("complete"), "not_started");
    }

    #[test]
    fn visibility_must_match_marketplace_statuses() {
        assert!(
            ensure_visibility_matches_status(
                &StorybookStatus::Submitted,
                &Visibility::MarketSubmission
            )
            .is_ok()
        );
        assert!(
            ensure_visibility_matches_status(&StorybookStatus::Listed, &Visibility::MarketListed)
                .is_ok()
        );
        assert!(
            ensure_visibility_matches_status(
                &StorybookStatus::Editing,
                &Visibility::MarketSubmission
            )
            .is_err()
        );
        assert!(
            ensure_visibility_matches_status(&StorybookStatus::Submitted, &Visibility::Workspace)
                .is_err()
        );
    }

    #[test]
    fn quality_report_blocks_page_prompt_without_confirmed_role() {
        let mut book = test_storybook();
        book.pages[0].body = "林老师带孩子练习轮流等待。".to_string();
        book.pages[0].illustration_prompt = "教室里摆着一辆红色小汽车。".to_string();
        let report = storybook_quality_report(&book);

        assert_eq!(report.status, StorybookQualityStatus::Blocked);
        assert!(report.summary.contains("阻断"));
        assert!(
            report.pages[0]
                .issues
                .iter()
                .any(|issue| issue.contains("没有明确带入"))
        );
    }

    #[test]
    fn quality_report_passes_when_reference_and_page_prompt_are_aligned() {
        let mut book = test_storybook();
        align_quality_fixture(&mut book);

        let report = storybook_quality_report(&book);

        assert_eq!(report.status, StorybookQualityStatus::Passed);
        assert!(report.pages[0].issues.is_empty());
    }

    #[test]
    fn quality_report_blocks_unconfirmed_substitute_character() {
        let mut book = test_storybook();
        align_quality_fixture(&mut book);
        book.pages[0].illustration_prompt =
            "林老师，温和、稳定，和小兔一起在教室里引导孩子轮流等待。".to_string();

        let report = storybook_quality_report(&book);

        assert_eq!(report.status, StorybookQualityStatus::Blocked);
        assert!(
            report.pages[0]
                .issues
                .iter()
                .any(|issue| issue.contains("未确认形象"))
        );
    }

    #[test]
    fn quality_report_blocks_page_while_image_is_generating() {
        let mut book = test_storybook();
        align_quality_fixture(&mut book);
        book.pages[0].status = "generating".to_string();

        let report = storybook_quality_report(&book);

        assert_eq!(report.status, StorybookQualityStatus::Blocked);
        assert!(
            report.pages[0]
                .issues
                .iter()
                .any(|issue| issue.contains("仍在生成中"))
        );
    }

    #[test]
    fn quality_report_blocks_page_needing_regeneration() {
        let mut book = test_storybook();
        align_quality_fixture(&mut book);
        book.pages[0].status = "needs_regeneration".to_string();

        let report = storybook_quality_report(&book);

        assert_eq!(report.status, StorybookQualityStatus::Blocked);
        assert!(
            report.pages[0]
                .issues
                .iter()
                .any(|issue| issue.contains("需要重新生成"))
        );
    }

    #[test]
    fn teacher_review_check_rejects_blocked_quality() {
        let mut book = test_storybook();
        book.pages[0].body = "林老师带孩子练习轮流等待。".to_string();
        book.pages[0].illustration_prompt = "教室里摆着一辆红色小汽车。".to_string();

        let err =
            ensure_teacher_review_ready(&book).expect_err("blocked quality should reject review");

        assert!(err.to_string().contains("生成质量检查存在阻断项"));
    }

    #[test]
    fn teacher_review_check_allows_non_blocked_quality() {
        let mut book = test_storybook();
        align_quality_fixture(&mut book);

        assert!(ensure_teacher_review_ready(&book).is_ok());
    }

    fn align_quality_fixture(book: &mut Storybook) {
        book.roles[0].reference_image_url =
            Some("/api/workspaces/demo/generation-jobs/demo/image".to_string());
        book.roles[0].reference_status = "ready".to_string();
        book.pages[0].body = "林老师带孩子练习轮流等待。".to_string();
        book.pages[0].illustration_prompt =
            "林老师，温和、稳定，在教室里引导孩子轮流等待。".to_string();
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
            customization_run_id: None,
            customization_run_item_id: None,
            customization_plan: None,
            creator_name: "林老师".to_string(),
            updated_at: "今天 09:00".to_string(),
            age_group: "4-5 岁".to_string(),
            use_scene: "规则引导".to_string(),
            teaching_goal: "学习轮流与分享".to_string(),
            cover_tone: "温暖、清楚".to_string(),
            page_aspect_ratio: "portrait_4_5".to_string(),
            teacher_review_status: "pending".to_string(),
            teacher_reviewed_by: None,
            teacher_reviewed_at: None,
            pages: vec![StorybookPage {
                id: Uuid::new_v4(),
                page_number: 1,
                title: "第一页".to_string(),
                body: "内容".to_string(),
                illustration_prompt: "提示".to_string(),
                status: "ready".to_string(),
                review_status: "unchecked".to_string(),
                reviewed_by: None,
                reviewed_at: None,
                image_url: None,
                selected_image_variant_id: None,
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
                selected_image_variant_id: None,
            }],
            quality: Default::default(),
        }
    }
}
