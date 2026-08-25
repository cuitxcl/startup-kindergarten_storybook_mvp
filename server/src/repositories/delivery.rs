use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

pub use super::delivery_exports::{
    create_export, create_export_by_share_token, execute_export_job, find_export,
    find_export_by_share_token, list_exports,
};
pub use super::delivery_share_links::{
    create_share_link, list_share_links, record_share_link_access, revoke_share_link,
    storybook_by_share_token,
};

use crate::models::{ExportJob, ShareLink, Storybook, StorybookType};

pub(crate) const CUSTOM_EVIDENCE_MISSING_PREFIX: &str = "custom_evidence_missing:";
pub(crate) const DIRECT_CREATION_EVIDENCE_MISSING_PREFIX: &str =
    "direct_creation_evidence_missing:";

pub(crate) async fn ensure_storybook_custom_evidence_ready(
    db: &DatabaseConnection,
    storybook: &Storybook,
) -> Result<(), DbErr> {
    if storybook.storybook_type != StorybookType::Custom {
        return Ok(());
    }

    let run = match storybook.customization_run_id {
        Some(run_id) => Some(
            crate::repositories::storybook_customization_runs::find_run(
                db,
                storybook.workspace_id,
                run_id,
            )
            .await?,
        ),
        None => None,
    };
    let run_item = storybook.customization_run_item_id.and_then(|item_id| {
        run.as_ref()
            .and_then(|run| run.items.iter().find(|item| item.id == item_id))
    });

    if let Some(details) = custom_evidence_missing_details(storybook, run_item) {
        return Err(DbErr::Custom(format!(
            "{CUSTOM_EVIDENCE_MISSING_PREFIX}{details}"
        )));
    }

    Ok(())
}

pub(crate) async fn ensure_storybook_direct_creation_evidence_ready(
    storybook: &Storybook,
) -> Result<(), DbErr> {
    if let Some(details) = direct_creation_evidence_missing_details(storybook) {
        return Err(DbErr::Custom(format!(
            "{DIRECT_CREATION_EVIDENCE_MISSING_PREFIX}{details}"
        )));
    }
    Ok(())
}

pub(crate) fn custom_evidence_missing_details(
    storybook: &Storybook,
    run_item: Option<&crate::models::StorybookCustomizationRunItem>,
) -> Option<JsonValue> {
    if storybook.storybook_type != StorybookType::Custom {
        return None;
    }

    let mut missing = Vec::new();
    let missing_pages: Vec<u32>;
    if storybook.customization_run_id.is_none() {
        missing.push("customization_run_id");
    }
    if storybook.customization_run_item_id.is_none() {
        missing.push("customization_run_item_id");
    }

    let Some(run_item) = run_item else {
        missing.push("customization_run_item");
        missing.push("page_evidence");
        missing_pages = storybook
            .pages
            .iter()
            .map(|page| page.page_number)
            .collect();
        return Some(custom_evidence_details_json(
            storybook,
            missing,
            missing_pages,
        ));
    };

    if run_item.status != "succeeded" {
        missing.push("succeeded_run_item");
    }
    if run_item.output_storybook_id != Some(storybook.id) {
        missing.push("matching_output_storybook_id");
    }
    let snapshot = &run_item.generation_input_snapshot;
    if snapshot.get("source_storybook_id").is_none() {
        missing.push("source_storybook_id");
    }
    if snapshot.get("target_child_id").is_none() {
        missing.push("target_child_id");
    }
    if run_item.target_child_nickname.is_some()
        && snapshot
            .get("target_child_nickname")
            .and_then(|value| value.as_str())
            .is_none_or(|value| value.trim().is_empty())
    {
        missing.push("target_child_nickname");
    }
    if snapshot
        .get("primary_material")
        .and_then(|value| value.as_str())
        .is_none_or(|value| value.trim().is_empty())
    {
        missing.push("primary_material");
    }
    if snapshot.get("source_snapshot").is_none() {
        missing.push("source_snapshot");
    }
    let page_plan = snapshot
        .get("page_plan")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    if page_plan.is_empty() {
        missing.push("page_plan");
    }
    let page_evidence = run_item
        .generation_input_snapshot
        .get("page_evidence")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    if page_evidence.is_empty() {
        missing.push("page_evidence");
        missing_pages = storybook
            .pages
            .iter()
            .map(|page| page.page_number)
            .collect();
    } else {
        let evidence_page_numbers = page_evidence
            .iter()
            .filter_map(|item| {
                item.get("page_number")
                    .and_then(|value| value.as_u64())
                    .and_then(|value| u32::try_from(value).ok())
            })
            .collect::<std::collections::HashSet<_>>();
        missing_pages = storybook
            .pages
            .iter()
            .filter(|page| !evidence_page_numbers.contains(&page.page_number))
            .map(|page| page.page_number)
            .collect();
        if !missing_pages.is_empty() {
            missing.push("page_evidence_pages");
        }
    }

    if missing.is_empty() {
        None
    } else {
        Some(custom_evidence_details_json(
            storybook,
            missing,
            missing_pages,
        ))
    }
}

pub(crate) fn direct_creation_evidence_missing_details(storybook: &Storybook) -> Option<JsonValue> {
    if storybook.source != "creation_session" {
        return None;
    }

    let mut missing = Vec::new();
    let missing_pages: Vec<u32>;
    let Some(plan) = storybook.customization_plan.as_ref() else {
        missing.push("direct_creation_evidence");
        missing.push("page_evidence");
        missing_pages = storybook
            .pages
            .iter()
            .map(|page| page.page_number)
            .collect();
        return Some(direct_creation_evidence_details_json(
            storybook,
            missing,
            missing_pages,
        ));
    };

    if plan.get("entry_type").and_then(|value| value.as_str()) != Some("direct_create") {
        missing.push("entry_type");
    }
    if plan.get("creation_session_id").is_none() {
        missing.push("creation_session_id");
    }
    if plan.get("generation_job_id").is_none() {
        missing.push("generation_job_id");
    }
    if plan.get("selected_direction").is_none() {
        missing.push("selected_direction");
    }
    if plan.get("outline").is_none() {
        missing.push("outline");
    }
    if plan.get("asset_references").is_none() {
        missing.push("asset_references");
    }
    let page_evidence = plan
        .get("page_evidence")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    if page_evidence.is_empty() {
        missing.push("page_evidence");
        missing_pages = storybook
            .pages
            .iter()
            .map(|page| page.page_number)
            .collect();
    } else {
        let evidence_page_numbers = page_evidence
            .iter()
            .filter_map(|item| {
                item.get("page_number")
                    .and_then(|value| value.as_u64())
                    .and_then(|value| u32::try_from(value).ok())
            })
            .collect::<std::collections::HashSet<_>>();
        missing_pages = storybook
            .pages
            .iter()
            .filter(|page| !evidence_page_numbers.contains(&page.page_number))
            .map(|page| page.page_number)
            .collect();
        if !missing_pages.is_empty() {
            missing.push("page_evidence_pages");
        }
    }

    if missing.is_empty() {
        None
    } else {
        Some(direct_creation_evidence_details_json(
            storybook,
            missing,
            missing_pages,
        ))
    }
}

fn custom_evidence_details_json(
    storybook: &Storybook,
    missing: Vec<&'static str>,
    missing_pages: Vec<u32>,
) -> JsonValue {
    json!({
        "storybook_id": storybook.id,
        "customization_run_id": storybook.customization_run_id,
        "customization_run_item_id": storybook.customization_run_item_id,
        "missing": missing,
        "missing_pages": missing_pages,
        "next_action": "review_customization_run"
    })
}

fn direct_creation_evidence_details_json(
    storybook: &Storybook,
    missing: Vec<&'static str>,
    missing_pages: Vec<u32>,
) -> JsonValue {
    json!({
        "storybook_id": storybook.id,
        "missing": missing,
        "missing_pages": missing_pages,
        "next_action": "review_direct_creation_evidence"
    })
}

pub(crate) async fn ensure_storybook_delivery_privacy_clear(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<(), DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select concat_ws(
              ' ',
              s.title,
              s.teaching_goal,
              s.use_scene,
              s.cover_tone,
              coalesce(string_agg(distinct concat_ws(' ', sp.title, sp.body, sp.illustration_prompt), ' '), ''),
              coalesce(string_agg(distinct concat_ws(' ', sr.name, sr.appearance, sr.story_function), ' '), '')
            ) as privacy_text
            from storybooks s
            left join storybook_pages sp on sp.storybook_id = s.id
            left join storybook_roles sr on sr.storybook_id = s.id
            where s.id = $1
            group by s.id
            "#,
            [storybook_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("storybook".to_string()))?;

    let privacy_text: String = row.try_get("", "privacy_text")?;
    let risks = crate::repositories::privacy::storybook_privacy_risks(&privacy_text);
    if risks.is_empty() {
        Ok(())
    } else {
        Err(DbErr::Custom(format!(
            "delivery_privacy_risk:{}",
            risks.join("、")
        )))
    }
}

pub(crate) async fn ensure_storybook_in_workspace(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<(), DbErr> {
    let exists = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id
            from storybooks
            where workspace_id = $1 and id = $2
            limit 1
            "#,
            [workspace_id.into(), storybook_id.into()],
        ))
        .await?
        .is_some();

    if exists {
        Ok(())
    } else {
        Err(DbErr::RecordNotFound("storybook".to_string()))
    }
}

pub(crate) fn share_link_from_row(row: &sea_orm::QueryResult) -> Result<ShareLink, DbErr> {
    let token: String = row.try_get("", "token")?;
    let expires_at: Option<DateTime<Utc>> = row.try_get("", "expires_at")?;
    let stored_status: String = row.try_get("", "status")?;
    let status = if stored_status == "active" && expires_at.is_some_and(|value| value <= Utc::now())
    {
        "expired".to_string()
    } else {
        stored_status
    };
    Ok(ShareLink {
        id: row.try_get("", "id")?,
        storybook_id: row.try_get("", "storybook_id")?,
        url: format!("/link/share/{token}"),
        token,
        status,
        access_count: row.try_get("", "access_count")?,
        last_accessed_at: row
            .try_get::<Option<DateTime<Utc>>>("", "last_accessed_at")?
            .map(|value| value.format("%Y-%m-%d %H:%M").to_string()),
        expires_at: expires_at.map(|value| value.to_rfc3339()),
    })
}

pub(crate) fn export_from_row(row: sea_orm::QueryResult) -> Result<ExportJob, DbErr> {
    Ok(ExportJob {
        id: row.try_get("", "id")?,
        storybook_id: row.try_get("", "storybook_id")?,
        created_by: row.try_get("", "created_by")?,
        status: row.try_get("", "status")?,
        file_url: row.try_get("", "file_url")?,
        last_error: row.try_get("", "last_error")?,
        created_at: row.try_get::<DateTime<Utc>>("", "created_at")?,
        finished_at: row.try_get("", "finished_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        Storybook, StorybookCustomizationRunItem, StorybookPage, StorybookQualityReport,
        StorybookQualityStatus, StorybookStatus, Visibility,
    };

    #[test]
    fn plain_storybook_does_not_require_custom_evidence() {
        let storybook = test_storybook(StorybookType::Plain);

        assert!(custom_evidence_missing_details(&storybook, None).is_none());
    }

    #[test]
    fn ordinary_plain_storybook_does_not_require_direct_creation_evidence() {
        let storybook = test_storybook(StorybookType::Plain);

        assert!(direct_creation_evidence_missing_details(&storybook).is_none());
    }

    #[test]
    fn direct_creation_storybook_without_evidence_is_blocked() {
        let mut storybook = test_storybook(StorybookType::Plain);
        storybook.source = "creation_session".to_string();

        let details = direct_creation_evidence_missing_details(&storybook)
            .expect("direct creation storybook should require frozen evidence");

        assert_eq!(details["next_action"], "review_direct_creation_evidence");
        assert!(
            details["missing"]
                .as_array()
                .expect("missing should be array")
                .iter()
                .any(|item| item == "page_evidence")
        );
        assert_eq!(details["missing_pages"], json!([1, 2]));
    }

    #[test]
    fn direct_creation_storybook_with_page_evidence_is_deliverable() {
        let mut storybook = test_storybook(StorybookType::Plain);
        storybook.source = "creation_session".to_string();
        storybook.customization_plan = Some(json!({
            "entry_type": "direct_create",
            "creation_session_id": Uuid::new_v4(),
            "generation_job_id": Uuid::new_v4(),
            "selected_direction": {
                "id": "direction-1",
                "title": "一起等待"
            },
            "outline": {
                "pages": [
                    { "page_number": 1, "title": "第一页" },
                    { "page_number": 2, "title": "第二页" }
                ]
            },
            "asset_references": [
                { "id": "asset-ref-1", "usage": "story_object" }
            ],
            "page_evidence": [
                { "page_number": 1, "asset_reference_ids": ["asset-ref-1"] },
                { "page_number": 2, "asset_reference_ids": [] }
            ]
        }));

        assert!(direct_creation_evidence_missing_details(&storybook).is_none());
    }

    #[test]
    fn direct_creation_storybook_requires_outline_evidence() {
        let mut storybook = test_storybook(StorybookType::Plain);
        storybook.source = "creation_session".to_string();
        storybook.customization_plan = Some(json!({
            "entry_type": "direct_create",
            "creation_session_id": Uuid::new_v4(),
            "generation_job_id": Uuid::new_v4(),
            "selected_direction": {
                "id": "direction-1",
                "title": "一起等待"
            },
            "asset_references": [],
            "page_evidence": [
                { "page_number": 1, "asset_reference_ids": [] },
                { "page_number": 2, "asset_reference_ids": [] }
            ]
        }));

        let details = direct_creation_evidence_missing_details(&storybook)
            .expect("missing outline should block direct creation delivery");

        assert!(
            details["missing"]
                .as_array()
                .expect("missing should be array")
                .iter()
                .any(|item| item == "outline")
        );
    }

    #[test]
    fn custom_storybook_without_run_evidence_is_blocked() {
        let storybook = test_storybook(StorybookType::Custom);

        let details = custom_evidence_missing_details(&storybook, None)
            .expect("custom storybook should require run evidence");

        assert_eq!(details["next_action"], "review_customization_run");
        assert!(
            details["missing"]
                .as_array()
                .expect("missing should be array")
                .iter()
                .any(|item| item == "page_evidence")
        );
    }

    #[test]
    fn custom_storybook_with_page_evidence_is_deliverable() {
        let run_id = Uuid::new_v4();
        let run_item_id = Uuid::new_v4();
        let mut storybook = test_storybook(StorybookType::Custom);
        storybook.customization_run_id = Some(run_id);
        storybook.customization_run_item_id = Some(run_item_id);
        let run_item = test_run_item(
            &storybook,
            run_id,
            run_item_id,
            json!([
                {
                    "source_page_id": Uuid::new_v4(),
                    "page_number": 1,
                    "decision": "personalize"
                },
                {
                    "source_page_id": Uuid::new_v4(),
                    "page_number": 2,
                    "decision": "prefer_keep"
                }
            ]),
        );

        assert!(custom_evidence_missing_details(&storybook, Some(&run_item)).is_none());
    }

    #[test]
    fn custom_storybook_requires_page_evidence_for_every_page() {
        let run_id = Uuid::new_v4();
        let run_item_id = Uuid::new_v4();
        let mut storybook = test_storybook(StorybookType::Custom);
        storybook.customization_run_id = Some(run_id);
        storybook.customization_run_item_id = Some(run_item_id);
        let run_item = test_run_item(
            &storybook,
            run_id,
            run_item_id,
            json!([{
                "source_page_id": Uuid::new_v4(),
                "page_number": 1,
                "decision": "personalize"
            }]),
        );

        let details = custom_evidence_missing_details(&storybook, Some(&run_item))
            .expect("partial evidence should block custom delivery");

        assert!(
            details["missing"]
                .as_array()
                .expect("missing should be array")
                .iter()
                .any(|item| item == "page_evidence_pages")
        );
        assert_eq!(details["missing_pages"], json!([2]));
    }

    #[test]
    fn custom_storybook_with_empty_page_evidence_is_blocked() {
        let run_id = Uuid::new_v4();
        let run_item_id = Uuid::new_v4();
        let mut storybook = test_storybook(StorybookType::Custom);
        storybook.customization_run_id = Some(run_id);
        storybook.customization_run_item_id = Some(run_item_id);
        let run_item = test_run_item(&storybook, run_id, run_item_id, json!([]));

        let details = custom_evidence_missing_details(&storybook, Some(&run_item))
            .expect("empty evidence should block custom delivery");

        assert!(
            details["missing"]
                .as_array()
                .expect("missing should be array")
                .iter()
                .any(|item| item == "page_evidence")
        );
    }

    #[test]
    fn custom_storybook_requires_frozen_source_snapshot() {
        let run_id = Uuid::new_v4();
        let run_item_id = Uuid::new_v4();
        let mut storybook = test_storybook(StorybookType::Custom);
        storybook.customization_run_id = Some(run_id);
        storybook.customization_run_item_id = Some(run_item_id);
        let mut run_item = test_run_item(
            &storybook,
            run_id,
            run_item_id,
            json!([
                { "page_number": 1, "decision": "personalize" },
                { "page_number": 2, "decision": "keep" }
            ]),
        );
        if let Some(object) = run_item.generation_input_snapshot.as_object_mut() {
            object.remove("source_snapshot");
        }

        let details = custom_evidence_missing_details(&storybook, Some(&run_item))
            .expect("missing source snapshot should block custom delivery");

        assert!(
            details["missing"]
                .as_array()
                .expect("missing should be array")
                .iter()
                .any(|item| item == "source_snapshot")
        );
    }

    fn test_storybook(storybook_type: StorybookType) -> Storybook {
        Storybook {
            id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            title: "测试绘本".to_string(),
            storybook_type,
            status: StorybookStatus::Exportable,
            visibility: Visibility::Workspace,
            source: "derived:test".to_string(),
            source_title: Some("来源绘本".to_string()),
            target_child_id: Some(Uuid::new_v4()),
            customization_run_id: None,
            customization_run_item_id: None,
            customization_plan: None,
            creator_name: "老师".to_string(),
            updated_at: "2026-08-21 10:00".to_string(),
            age_group: "4-5 岁".to_string(),
            use_scene: "家庭共读".to_string(),
            teaching_goal: "练习表达".to_string(),
            cover_tone: "温暖".to_string(),
            story_style_id: Some("daily_warmth".to_string()),
            visual_style_id: Some("watercolor_book".to_string()),
            visual_style_version: Some(1),
            page_aspect_ratio: "portrait_4_5".to_string(),
            teacher_review_status: "confirmed".to_string(),
            teacher_reviewed_by: Some(Uuid::new_v4()),
            teacher_reviewed_at: Some("2026-08-21 10:00".to_string()),
            pages: vec![
                StorybookPage {
                    id: Uuid::new_v4(),
                    page_number: 1,
                    title: "第一页".to_string(),
                    body: "内容一".to_string(),
                    illustration_prompt: "画面一".to_string(),
                    status: "ready".to_string(),
                    review_status: "unchecked".to_string(),
                    reviewed_by: None,
                    reviewed_at: None,
                    image_url: None,
                    selected_image_variant_id: None,
                },
                StorybookPage {
                    id: Uuid::new_v4(),
                    page_number: 2,
                    title: "第二页".to_string(),
                    body: "内容二".to_string(),
                    illustration_prompt: "画面二".to_string(),
                    status: "ready".to_string(),
                    review_status: "unchecked".to_string(),
                    reviewed_by: None,
                    reviewed_at: None,
                    image_url: None,
                    selected_image_variant_id: None,
                },
            ],
            roles: Vec::new(),
            quality: StorybookQualityReport {
                status: StorybookQualityStatus::Passed,
                ..StorybookQualityReport::default()
            },
        }
    }

    fn test_run_item(
        storybook: &Storybook,
        run_id: Uuid,
        run_item_id: Uuid,
        page_evidence: JsonValue,
    ) -> StorybookCustomizationRunItem {
        StorybookCustomizationRunItem {
            id: run_item_id,
            workspace_id: storybook.workspace_id,
            run_id,
            source_storybook_id: Uuid::new_v4(),
            target_child_id: storybook.target_child_id.expect("target child"),
            target_child_nickname: Some("乐乐".to_string()),
            output_storybook_id: Some(storybook.id),
            output_storybook_title: Some(storybook.title.clone()),
            primary_material: Some("小汽车".to_string()),
            status: "succeeded".to_string(),
            generation_input_snapshot: json!({
                "source_storybook_id": Uuid::new_v4(),
                "target_child_id": storybook.target_child_id.expect("target child"),
                "target_child_nickname": "乐乐",
                "primary_material": "小汽车",
                "source_snapshot": {
                    "storybook_id": Uuid::new_v4(),
                    "page_count": storybook.pages.len()
                },
                "page_plan": [
                    { "page_number": 1, "decision": "personalize" },
                    { "page_number": 2, "decision": "keep" }
                ],
                "page_evidence": page_evidence,
            }),
            failure_reason: None,
            created_at: "2026-08-21 10:00:00".to_string(),
            updated_at: "2026-08-21 10:01:00".to_string(),
            completed_at: Some("2026-08-21 10:01:00".to_string()),
        }
    }
}
