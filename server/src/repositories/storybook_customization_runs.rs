use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::models::{StorybookCustomizationRun, StorybookCustomizationRunItem};

pub struct CreateCustomizationRunInput {
    pub workspace_id: Uuid,
    pub source_storybook_id: Uuid,
    pub created_by: Uuid,
    pub mode: String,
    pub customization_plan: Option<JsonValue>,
    pub requested_count: usize,
}

pub async fn create_run(
    db: &DatabaseConnection,
    input: CreateCustomizationRunInput,
) -> Result<Uuid, DbErr> {
    let run_id = Uuid::new_v4();
    let source_snapshot = input
        .customization_plan
        .as_ref()
        .and_then(|plan| plan.get("source_snapshot"))
        .cloned();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybook_customization_runs
          (id, workspace_id, source_storybook_id, created_by, entry_type, mode, status, customization_plan, source_snapshot, requested_count, created_at, updated_at)
        values ($1, $2, $3, $4, 'from_storybook', $5, 'queued', $6, $7, $8, now(), now())
        "#,
        [
            run_id.into(),
            input.workspace_id.into(),
            input.source_storybook_id.into(),
            input.created_by.into(),
            input.mode.into(),
            input.customization_plan.into(),
            source_snapshot.into(),
            (input.requested_count as i32).into(),
        ],
    ))
    .await?;
    Ok(run_id)
}

pub struct CreateRunItemInput {
    pub workspace_id: Uuid,
    pub run_id: Uuid,
    pub source_storybook_id: Uuid,
    pub target_child_id: Uuid,
    pub primary_material: Option<String>,
    pub generation_input_snapshot: JsonValue,
}

pub async fn create_run_item(
    db: &DatabaseConnection,
    input: CreateRunItemInput,
) -> Result<Uuid, DbErr> {
    let item_id = Uuid::new_v4();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybook_customization_run_items
          (id, workspace_id, run_id, source_storybook_id, target_child_id, primary_material, status, generation_input_snapshot, created_at, updated_at)
        values ($1, $2, $3, $4, $5, $6, 'queued', $7, now(), now())
        "#,
        [
            item_id.into(),
            input.workspace_id.into(),
            input.run_id.into(),
            input.source_storybook_id.into(),
            input.target_child_id.into(),
            input.primary_material.into(),
            input.generation_input_snapshot.into(),
        ],
    ))
    .await?;
    Ok(item_id)
}

pub async fn mark_item_running(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    item_id: Uuid,
) -> Result<bool, DbErr> {
    let result = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
        update storybook_customization_run_items
        set status = 'running', updated_at = now()
        where workspace_id = $1 and id = $2 and status in ('queued', 'retrying')
        "#,
            [workspace_id.into(), item_id.into()],
        ))
        .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn mark_item_succeeded(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    item_id: Uuid,
    output_storybook_id: Uuid,
) -> Result<bool, DbErr> {
    let result = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
        update storybook_customization_run_items
        set status = 'succeeded',
            output_storybook_id = $3,
            failure_reason = null,
            completed_at = now(),
            updated_at = now()
        where workspace_id = $1 and id = $2 and status = 'running'
        "#,
            [
                workspace_id.into(),
                item_id.into(),
                output_storybook_id.into(),
            ],
        ))
        .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn mark_item_failed(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    item_id: Uuid,
    failure_reason: &str,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_customization_run_items
        set status = 'failed',
            failure_reason = $3,
            completed_at = now(),
            updated_at = now()
        where workspace_id = $1 and id = $2 and status in ('queued', 'running', 'retrying')
        "#,
        [
            workspace_id.into(),
            item_id.into(),
            failure_reason.to_string().into(),
        ],
    ))
    .await?;
    Ok(())
}

pub async fn mark_item_retrying(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    item_id: Uuid,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_customization_run_items
        set status = 'queued',
            failure_reason = null,
            completed_at = null,
            updated_at = now()
        where workspace_id = $1 and id = $2 and status = 'failed'
        "#,
        [workspace_id.into(), item_id.into()],
    ))
    .await?;
    Ok(())
}

pub async fn mark_canceled_item_failed(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    item_id: Uuid,
    failure_reason: &str,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_customization_run_items
        set status = 'failed',
            failure_reason = $3,
            completed_at = now(),
            updated_at = now()
        where workspace_id = $1 and id = $2 and status = 'canceled'
        "#,
        [
            workspace_id.into(),
            item_id.into(),
            failure_reason.to_string().into(),
        ],
    ))
    .await?;
    Ok(())
}

pub async fn fail_active_items_for_asset_revocation(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    run_id: Uuid,
    failure_reason: &str,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_customization_run_items
        set status = 'failed',
            failure_reason = $3,
            completed_at = now(),
            updated_at = now()
        where workspace_id = $1
          and run_id = $2
          and status in ('queued', 'running', 'retrying', 'canceled')
        "#,
        [
            workspace_id.into(),
            run_id.into(),
            failure_reason.to_string().into(),
        ],
    ))
    .await?;
    Ok(())
}

pub async fn mark_item_canceled(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    item_id: Uuid,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_customization_run_items
        set status = 'canceled',
            failure_reason = null,
            completed_at = now(),
            updated_at = now()
        where workspace_id = $1 and id = $2 and status in ('queued', 'running', 'retrying', 'failed')
        "#,
        [workspace_id.into(), item_id.into()],
    ))
    .await?;
    Ok(())
}

pub async fn cancel_active_items(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    run_id: Uuid,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_customization_run_items
        set status = 'canceled',
            failure_reason = null,
            completed_at = now(),
            updated_at = now()
        where workspace_id = $1
          and run_id = $2
          and status in ('queued', 'running', 'retrying')
        "#,
        [workspace_id.into(), run_id.into()],
    ))
    .await?;
    Ok(())
}

pub async fn finish_run(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    run_id: Uuid,
    failure_reason: Option<&str>,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_customization_runs
        set succeeded_count = (
                select count(*)::int
                from storybook_customization_run_items
                where workspace_id = $1 and run_id = $2 and status = 'succeeded'
            ),
            failed_count = (
                select count(*)::int
                from storybook_customization_run_items
                where workspace_id = $1 and run_id = $2 and status = 'failed'
            ),
            status = case
                when $3::text is not null then 'failed'
                when exists (
                    select 1 from storybook_customization_run_items
                    where workspace_id = $1 and run_id = $2 and status in ('queued', 'running', 'retrying')
                ) then 'running'
                when exists (
                    select 1 from storybook_customization_run_items
                    where workspace_id = $1 and run_id = $2 and status = 'failed'
                ) then 'failed'
                when not exists (
                    select 1 from storybook_customization_run_items
                    where workspace_id = $1 and run_id = $2 and status <> 'canceled'
                ) then 'canceled'
                else 'succeeded'
            end,
            failure_reason = $3,
            completed_at = case
                when $3::text is not null then now()
                when exists (
                    select 1 from storybook_customization_run_items
                    where workspace_id = $1 and run_id = $2 and status in ('queued', 'running', 'retrying')
                ) then null
                else now()
            end,
            updated_at = now()
        where workspace_id = $1 and id = $2
        "#,
        [
            workspace_id.into(),
            run_id.into(),
            failure_reason.map(str::to_string).into(),
        ],
    ))
    .await?;
    Ok(())
}

pub fn run_item_snapshot(
    source_storybook_id: Uuid,
    target_child_id: Uuid,
    intensity: &str,
    primary_material: Option<&str>,
    customization_plan: Option<&JsonValue>,
) -> JsonValue {
    let page_evidence = page_evidence_from_plan(customization_plan);
    serde_json::json!({
        "source_storybook_id": source_storybook_id,
        "target_child_id": target_child_id,
        "target_child_nickname": target_child_nickname_from_plan(customization_plan),
        "intensity": intensity,
        "primary_material": primary_material,
        "customization_plan": customization_plan,
        "source_snapshot": customization_plan.and_then(|plan| plan.get("source_snapshot")),
        "page_plan": customization_plan.and_then(|plan| plan.get("page_plan")),
        "page_evidence": page_evidence,
        "confirmed_photo_reference_ids": customization_plan.and_then(|plan| plan.get("confirmed_photo_reference_ids")),
        "confirmed_photo_references": customization_plan.and_then(|plan| plan.get("confirmed_photo_references")),
    })
}

fn target_child_nickname_from_plan(customization_plan: Option<&JsonValue>) -> Option<String> {
    customization_plan
        .and_then(|plan| plan.get("target_child_nickname"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn page_evidence_from_plan(customization_plan: Option<&JsonValue>) -> JsonValue {
    let Some(page_plan) = customization_plan
        .and_then(|plan| plan.get("page_plan"))
        .and_then(|value| value.as_array())
    else {
        return JsonValue::Array(Vec::new());
    };

    JsonValue::Array(
        page_plan
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let decision = item
                    .get("decision")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown");
                let character_reference_ids = item
                    .get("character_reference_ids")
                    .cloned()
                    .unwrap_or_else(|| JsonValue::Array(Vec::new()));
                let prop_reference_ids = item
                    .get("prop_reference_ids")
                    .cloned()
                    .unwrap_or_else(|| JsonValue::Array(Vec::new()));
                let scene_reference_ids = item
                    .get("scene_reference_ids")
                    .cloned()
                    .unwrap_or_else(|| JsonValue::Array(Vec::new()));
                let legacy_asset_reference_ids = item
                    .get("asset_reference_ids")
                    .cloned()
                    .unwrap_or_else(|| JsonValue::Array(Vec::new()));
                let asset_reference_ids = if legacy_asset_reference_ids
                    .as_array()
                    .is_some_and(|ids| !ids.is_empty())
                {
                    legacy_asset_reference_ids
                } else {
                    grouped_reference_ids(&[
                        &character_reference_ids,
                        &prop_reference_ids,
                        &scene_reference_ids,
                    ])
                };
                serde_json::json!({
                    "source_page_id": item.get("source_page_id").and_then(|value| value.as_str()),
                    "page_number": item.get("page_number").and_then(|value| value.as_u64()).unwrap_or((index + 1) as u64),
                    "title": item.get("title").and_then(|value| value.as_str()),
                    "decision": decision,
                    "reason": item.get("reason").and_then(|value| value.as_str()),
                    "requires_redraw": matches!(decision, "personalize" | "redraw_required"),
                    "asset_reference_ids": asset_reference_ids,
                    "character_reference_ids": character_reference_ids,
                    "prop_reference_ids": prop_reference_ids,
                    "scene_reference_ids": scene_reference_ids,
                    "evidence_source": "customization_plan",
                })
            })
            .collect(),
    )
}

fn grouped_reference_ids(groups: &[&JsonValue]) -> JsonValue {
    let mut ids = Vec::new();
    for group in groups {
        for id in group.as_array().into_iter().flatten() {
            if id
                .as_str()
                .is_some_and(|value| !ids.iter().any(|item| item == value))
            {
                ids.push(id.to_string());
            }
        }
    }
    JsonValue::Array(ids.into_iter().map(JsonValue::String).collect())
}

pub async fn find_run(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    run_id: Uuid,
) -> Result<StorybookCustomizationRun, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select
              id,
              workspace_id,
              source_storybook_id,
              created_by,
              entry_type,
              mode,
              status,
              customization_plan,
              source_snapshot,
              requested_count,
              succeeded_count,
              failed_count,
              failure_reason,
              created_at,
              updated_at,
              completed_at
            from storybook_customization_runs
            where workspace_id = $1 and id = $2
            "#,
            [workspace_id.into(), run_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("storybook_customization_run".to_string()))?;

    let items = list_run_items(db, workspace_id, run_id).await?;
    Ok(StorybookCustomizationRun {
        id: row.try_get("", "id")?,
        workspace_id: row.try_get("", "workspace_id")?,
        source_storybook_id: row.try_get("", "source_storybook_id")?,
        created_by: row.try_get("", "created_by")?,
        entry_type: row.try_get("", "entry_type")?,
        mode: row.try_get("", "mode")?,
        status: row.try_get("", "status")?,
        customization_plan: row.try_get("", "customization_plan")?,
        source_snapshot: row.try_get("", "source_snapshot")?,
        requested_count: non_negative_usize(row.try_get("", "requested_count")?),
        succeeded_count: non_negative_usize(row.try_get("", "succeeded_count")?),
        failed_count: non_negative_usize(row.try_get("", "failed_count")?),
        failure_reason: row.try_get("", "failure_reason")?,
        created_at: format_timestamp(row.try_get("", "created_at")?),
        updated_at: format_timestamp(row.try_get("", "updated_at")?),
        completed_at: row
            .try_get::<Option<DateTime<Utc>>>("", "completed_at")?
            .map(format_timestamp),
        items,
    })
}

pub async fn find_matching_run(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    source_storybook_id: Uuid,
    mode: &str,
    requested_count: usize,
    customization_plan: Option<&JsonValue>,
) -> Result<Option<StorybookCustomizationRun>, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                r#"
            select
              id,
              workspace_id,
              source_storybook_id,
              created_by,
              entry_type,
              mode,
              status,
              customization_plan,
              source_snapshot,
              requested_count,
              succeeded_count,
              failed_count,
              failure_reason,
              created_at,
              updated_at,
              completed_at
            from storybook_customization_runs
            where workspace_id = $1
              and source_storybook_id = $2
              and mode = $3
              and requested_count = $4
              and (
                (customization_plan is null and $5::jsonb is null)
                or customization_plan = $5::jsonb
              )
              and {}
            order by created_at desc
            limit 1
            "#,
                matching_run_status_filter_sql()
            ),
            [
                workspace_id.into(),
                source_storybook_id.into(),
                mode.to_string().into(),
                (requested_count as i32).into(),
                customization_plan.cloned().into(),
            ],
        ))
        .await?;

    let Some(row) = row else {
        return Ok(None);
    };
    let run_id: Uuid = row.try_get("", "id")?;
    let items = list_run_items(db, workspace_id, run_id).await?;
    Ok(Some(StorybookCustomizationRun {
        id: run_id,
        workspace_id: row.try_get("", "workspace_id")?,
        source_storybook_id: row.try_get("", "source_storybook_id")?,
        created_by: row.try_get("", "created_by")?,
        entry_type: row.try_get("", "entry_type")?,
        mode: row.try_get("", "mode")?,
        status: row.try_get("", "status")?,
        customization_plan: row.try_get("", "customization_plan")?,
        source_snapshot: row.try_get("", "source_snapshot")?,
        requested_count: non_negative_usize(row.try_get("", "requested_count")?),
        succeeded_count: non_negative_usize(row.try_get("", "succeeded_count")?),
        failed_count: non_negative_usize(row.try_get("", "failed_count")?),
        failure_reason: row.try_get("", "failure_reason")?,
        created_at: format_timestamp(row.try_get("", "created_at")?),
        updated_at: format_timestamp(row.try_get("", "updated_at")?),
        completed_at: row
            .try_get::<Option<DateTime<Utc>>>("", "completed_at")?
            .map(format_timestamp),
        items,
    }))
}

pub async fn active_run_ids_using_asset_reference(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    asset_reference_id: Uuid,
) -> Result<Vec<Uuid>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id
            from storybook_customization_runs
            where workspace_id = $1
              and status in ('queued', 'running')
              and coalesce(customization_plan -> 'confirmed_photo_reference_ids', '[]'::jsonb) ? $2
            "#,
            [workspace_id.into(), asset_reference_id.to_string().into()],
        ))
        .await?;
    rows.into_iter().map(|row| row.try_get("", "id")).collect()
}

fn matching_run_status_filter_sql() -> &'static str {
    "status in ('queued', 'running', 'succeeded', 'failed')"
}

pub async fn find_run_item(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    run_id: Uuid,
    item_id: Uuid,
) -> Result<StorybookCustomizationRunItem, DbErr> {
    list_run_items(db, workspace_id, run_id)
        .await?
        .into_iter()
        .find(|item| item.id == item_id)
        .ok_or_else(|| DbErr::RecordNotFound("storybook_customization_run_item".to_string()))
}

async fn list_run_items(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    run_id: Uuid,
) -> Result<Vec<StorybookCustomizationRunItem>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select
              i.id,
              i.workspace_id,
              i.run_id,
              i.source_storybook_id,
              i.target_child_id,
              c.nickname as target_child_nickname,
              i.output_storybook_id,
              output.title as output_storybook_title,
              i.primary_material,
              i.status,
              i.generation_input_snapshot,
              i.failure_reason,
              i.created_at,
              i.updated_at,
              i.completed_at
            from storybook_customization_run_items i
            left join children c
              on c.id = i.target_child_id and c.workspace_id = i.workspace_id
            left join storybooks output
              on output.id = i.output_storybook_id and output.workspace_id = i.workspace_id
            where i.workspace_id = $1 and i.run_id = $2
            order by i.created_at, i.id
            "#,
            [workspace_id.into(), run_id.into()],
        ))
        .await?;

    rows.into_iter()
        .map(|row| {
            Ok(StorybookCustomizationRunItem {
                id: row.try_get("", "id")?,
                workspace_id: row.try_get("", "workspace_id")?,
                run_id: row.try_get("", "run_id")?,
                source_storybook_id: row.try_get("", "source_storybook_id")?,
                target_child_id: row.try_get("", "target_child_id")?,
                target_child_nickname: row.try_get("", "target_child_nickname")?,
                output_storybook_id: row.try_get("", "output_storybook_id")?,
                output_storybook_title: row.try_get("", "output_storybook_title")?,
                primary_material: row.try_get("", "primary_material")?,
                status: row.try_get("", "status")?,
                generation_input_snapshot: row.try_get("", "generation_input_snapshot")?,
                failure_reason: row.try_get("", "failure_reason")?,
                created_at: format_timestamp(row.try_get("", "created_at")?),
                updated_at: format_timestamp(row.try_get("", "updated_at")?),
                completed_at: row
                    .try_get::<Option<DateTime<Utc>>>("", "completed_at")?
                    .map(format_timestamp),
            })
        })
        .collect()
}

fn format_timestamp(value: DateTime<Utc>) -> String {
    value.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn non_negative_usize(value: i32) -> usize {
    value.max(0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_item_snapshot_freezes_source_child_and_plan_inputs() {
        let source_storybook_id = Uuid::new_v4();
        let target_child_id = Uuid::new_v4();
        let plan = serde_json::json!({
            "target_child_nickname": "乐乐",
            "source_snapshot": {
                "storybook_id": source_storybook_id,
                "page_count": 6
            },
            "page_plan": [
                { "page_number": 1, "decision": "keep", "asset_reference_ids": ["asset-ref-1"] }
            ],
            "confirmed_photo_reference_ids": ["asset-ref-1"],
            "confirmed_photo_references": [
                { "asset_reference_id": "asset-ref-1", "visual_reference_id": "visual-ref-1" }
            ]
        });

        let snapshot = run_item_snapshot(
            source_storybook_id,
            target_child_id,
            "standard",
            Some("profile"),
            Some(&plan),
        );

        assert_eq!(
            snapshot["source_storybook_id"],
            source_storybook_id.to_string()
        );
        assert_eq!(snapshot["target_child_id"], target_child_id.to_string());
        assert_eq!(snapshot["target_child_nickname"], "乐乐");
        assert_eq!(snapshot["intensity"], "standard");
        assert_eq!(snapshot["primary_material"], "profile");
        assert_eq!(snapshot["source_snapshot"]["page_count"], 6);
        assert_eq!(snapshot["page_plan"].as_array().unwrap().len(), 1);
        assert_eq!(snapshot["page_evidence"].as_array().unwrap().len(), 1);
        assert_eq!(snapshot["page_evidence"][0]["decision"], "keep");
        assert_eq!(snapshot["page_evidence"][0]["requires_redraw"], false);
        assert_eq!(
            snapshot["page_evidence"][0]["asset_reference_ids"][0],
            "asset-ref-1"
        );
        assert_eq!(snapshot["confirmed_photo_reference_ids"][0], "asset-ref-1");
        assert_eq!(
            snapshot["confirmed_photo_references"][0]["visual_reference_id"],
            "visual-ref-1"
        );
    }

    #[test]
    fn page_evidence_marks_personalized_pages_as_redraw_required() {
        let plan = serde_json::json!({
            "page_plan": [
                { "page_number": 1, "decision": "personalize", "title": "孩子版本", "reason": "替换为对象版本" },
                { "page_number": 2, "decision": "redraw_required", "title": "需要重绘", "reason": "图文冲突必须重绘" }
            ]
        });

        let evidence = page_evidence_from_plan(Some(&plan));

        assert_eq!(evidence[0]["requires_redraw"], true);
        assert_eq!(evidence[1]["requires_redraw"], true);
        assert_eq!(evidence[0]["reason"], "替换为对象版本");
        assert_eq!(evidence[1]["reason"], "图文冲突必须重绘");
        assert_eq!(evidence[0]["evidence_source"], "customization_plan");
    }

    #[test]
    fn page_evidence_freezes_typed_photo_reference_ids() {
        let plan = serde_json::json!({
            "page_plan": [{
                "page_number": 2,
                "decision": "personalize",
                "character_reference_ids": ["character-ref"],
                "prop_reference_ids": ["prop-ref"],
                "scene_reference_ids": []
            }]
        });

        let evidence = page_evidence_from_plan(Some(&plan));

        assert_eq!(evidence[0]["character_reference_ids"][0], "character-ref");
        assert_eq!(evidence[0]["prop_reference_ids"][0], "prop-ref");
        assert!(
            evidence[0]["scene_reference_ids"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            evidence[0]["asset_reference_ids"].as_array().unwrap().len(),
            2
        );
    }

    #[test]
    fn non_negative_usize_clamps_database_counters() {
        assert_eq!(non_negative_usize(3), 3);
        assert_eq!(non_negative_usize(-1), 0);
    }

    #[test]
    fn matching_run_status_filter_keeps_failed_runs_idempotent() {
        let filter = matching_run_status_filter_sql();

        assert!(filter.contains("'queued'"));
        assert!(filter.contains("'running'"));
        assert!(filter.contains("'succeeded'"));
        assert!(filter.contains("'failed'"));
        assert!(!filter.contains("'canceled'"));
    }
}
