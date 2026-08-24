use axum::http::HeaderMap;
use loco_rs::app::AppContext;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use serde_json::json;

#[cfg(not(feature = "db"))]
use crate::models::{StorybookStatus, Visibility};
use crate::{
    application::storybook_inputs::storybook_status_name,
    domains::common,
    error::ApiError,
    models::{
        BuildCustomizationPlanRequest, DeriveCustomBatchItem, DeriveCustomBatchRequest,
        DeriveCustomBatchResponse, DeriveCustomRequest, Storybook, StorybookCustomizationRun,
        StorybookType,
    },
};

pub async fn build_customization_plan(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    payload: BuildCustomizationPlanRequest,
) -> Result<serde_json::Value, ApiError> {
    #[cfg(feature = "db")]
    {
        let workspace = common::require_editor_db(ctx, headers, workspace_id).await?;
        let source = crate::repositories::storybooks::find(&ctx.db, workspace_id, storybook_id)
            .await
            .map_err(common::db_error)?;
        if source.storybook_type != StorybookType::Plain {
            return Err(ApiError::state_conflict("只有普通绘本可以派生定制绘本"));
        }
        crate::repositories::storybook_customization::ensure_source_ready_for_customization(
            &source,
        )
        .map_err(common::db_error)?;
        let target_children =
            validate_customization_plan_payload(ctx, headers, workspace_id, &workspace, &payload)
                .await?;
        let confirmed_photo_references =
            confirmed_photo_references_for_plan(&ctx.db, workspace_id, &payload).await?;
        return Ok(build_source_customization_plan(
            &source,
            &payload,
            &target_children,
            &confirmed_photo_references,
        ));
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_editor(&state, headers, workspace_id)?;
        let state = state.read().expect("state lock poisoned");
        let source = state
            .storybooks
            .iter()
            .find(|item| item.workspace_id == workspace_id && item.id == storybook_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("storybook"))?;
        if source.storybook_type != StorybookType::Plain {
            return Err(ApiError::state_conflict("只有普通绘本可以派生定制绘本"));
        }
        ensure_in_memory_source_ready(&source)?;
        let target_children = in_memory_target_children(&state, workspace_id, &payload)?;
        Ok(build_source_customization_plan(
            &source,
            &payload,
            &target_children,
            &[],
        ))
    }
}

pub async fn get_customization_run(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    run_id: Uuid,
) -> Result<StorybookCustomizationRun, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        return crate::repositories::storybook_customization_runs::find_run(
            &ctx.db,
            workspace_id,
            run_id,
        )
        .await
        .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_editor(&state, headers, workspace_id)?;
        Err(ApiError::not_found("storybook_customization_run"))
    }
}

pub async fn cancel_customization_run(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    run_id: Uuid,
) -> Result<StorybookCustomizationRun, ApiError> {
    #[cfg(feature = "db")]
    {
        let actor_id = common::actor_user_id(headers)?;
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let run = crate::repositories::storybook_customization_runs::find_run(
            &ctx.db,
            workspace_id,
            run_id,
        )
        .await
        .map_err(common::db_error)?;
        if !matches!(run.status.as_str(), "queued" | "running") {
            return Err(ApiError::state_conflict_with_code(
                "customization_run_not_cancelable",
                "只有排队中或制作中的定制任务可以取消",
            ));
        }
        crate::repositories::generation::cancel_customization_run_jobs(
            &ctx.db,
            workspace_id,
            run_id,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::storybook_customization_runs::cancel_active_items(
            &ctx.db,
            workspace_id,
            run_id,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::storybook_customization_runs::finish_run(
            &ctx.db,
            workspace_id,
            run_id,
            None,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "storybook.custom_run_canceled",
            "storybook_customization_run",
            Some(run_id),
            json!({ "source_storybook_id": run.source_storybook_id }),
        )
        .await
        .map_err(common::db_error)?;
        return crate::repositories::storybook_customization_runs::find_run(
            &ctx.db,
            workspace_id,
            run_id,
        )
        .await
        .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_editor(&state, headers, workspace_id)?;
        Err(ApiError::not_found("storybook_customization_run"))
    }
}

pub async fn retry_customization_run_item(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    run_id: Uuid,
    item_id: Uuid,
) -> Result<StorybookCustomizationRun, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let actor_id = common::actor_user_id(headers)?;
        let item = crate::repositories::storybook_customization_runs::find_run_item(
            &ctx.db,
            workspace_id,
            run_id,
            item_id,
        )
        .await
        .map_err(common::db_error)?;
        if item.status != "failed" {
            return Err(ApiError::state_conflict_with_code(
                "run_item_not_retryable",
                "只有失败的定制项可以重试",
            ));
        }

        let source =
            crate::repositories::storybooks::find(&ctx.db, workspace_id, item.source_storybook_id)
                .await
                .map_err(common::db_error)?;
        ensure_source_ready_for_customization_api(&source)?;
        let customization_plan = item
            .generation_input_snapshot
            .get("customization_plan")
            .cloned();
        ensure_source_snapshot_current(&source, customization_plan.as_ref())?;
        ensure_customization_photo_references_active(
            &ctx.db,
            workspace_id,
            customization_plan.as_ref(),
        )
        .await?;
        crate::repositories::storybook_customization_runs::mark_item_retrying(
            &ctx.db,
            workspace_id,
            item_id,
        )
        .await
        .map_err(common::db_error)?;

        if let Err(_err) = enqueue_customization_derivation(
            ctx,
            workspace_id,
            actor_id,
            item.source_storybook_id,
            run_id,
            item_id,
            item.target_child_id,
            &retry_intensity_from_snapshot(&item.generation_input_snapshot),
            item.primary_material.clone(),
            customization_plan,
        )
        .await
        {
            let failure_reason = "定制任务暂时无法入队，请重试".to_string();
            crate::repositories::storybook_customization_runs::mark_item_failed(
                &ctx.db,
                workspace_id,
                item_id,
                &failure_reason,
            )
            .await
            .map_err(common::db_error)?;
        }
        crate::repositories::storybook_customization_runs::finish_run(
            &ctx.db,
            workspace_id,
            run_id,
            None,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "storybook.custom_run_item_requeued",
            "storybook_customization_run_item",
            Some(item_id),
            json!({
                "customization_run_id": run_id,
                "target_child_id": item.target_child_id,
            }),
        )
        .await
        .map_err(common::db_error)?;

        return crate::repositories::storybook_customization_runs::find_run(
            &ctx.db,
            workspace_id,
            run_id,
        )
        .await
        .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_editor(&state, headers, workspace_id)?;
        Err(ApiError::not_found("storybook_customization_run_item"))
    }
}

pub async fn abandon_customization_run_item(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    run_id: Uuid,
    item_id: Uuid,
) -> Result<StorybookCustomizationRun, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let actor_id = common::actor_user_id(headers)?;
        let item = crate::repositories::storybook_customization_runs::find_run_item(
            &ctx.db,
            workspace_id,
            run_id,
            item_id,
        )
        .await
        .map_err(common::db_error)?;
        if item.status != "failed" {
            return Err(ApiError::state_conflict_with_code(
                "run_item_not_abandonable",
                "只有失败的定制项可以放弃",
            ));
        }

        crate::repositories::storybook_customization_runs::mark_item_canceled(
            &ctx.db,
            workspace_id,
            item_id,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::storybook_customization_runs::finish_run(
            &ctx.db,
            workspace_id,
            run_id,
            None,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "storybook.custom_run_item_abandoned",
            "storybook_customization_run_item",
            Some(item_id),
            json!({
                "customization_run_id": run_id,
                "target_child_id": item.target_child_id,
                "previous_failure_reason": item.failure_reason,
            }),
        )
        .await
        .map_err(common::db_error)?;

        return crate::repositories::storybook_customization_runs::find_run(
            &ctx.db,
            workspace_id,
            run_id,
        )
        .await
        .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_editor(&state, headers, workspace_id)?;
        Err(ApiError::not_found("storybook_customization_run_item"))
    }
}

pub async fn derive_custom(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    payload: DeriveCustomRequest,
) -> Result<Storybook, ApiError> {
    #[cfg(feature = "db")]
    {
        let workspace = common::require_editor_db(ctx, headers, workspace_id).await?;
        validate_custom_single_payload(&payload)?;
        let target_child = if let Some(classrooms) =
            common::child_classroom_scope(ctx, headers, workspace_id, &workspace).await?
        {
            crate::repositories::children::find_for_classrooms(
                &ctx.db,
                workspace_id,
                payload.child_id,
                &classrooms,
            )
            .await
            .map_err(common::db_error)?
        } else {
            crate::repositories::children::find(&ctx.db, workspace_id, payload.child_id)
                .await
                .map_err(common::db_error)?
        };
        let child_id = payload.child_id;
        let target_child_nickname = target_child.nickname.clone();
        let intensity = payload.intensity.clone();
        let primary_material = payload.primary_material.clone();
        let submitted_plan = attach_single_run_identity(
            payload.customization_plan.clone(),
            child_id,
            Some(target_child_nickname),
            primary_material.clone(),
        );
        let confirmed_photo_references = validate_confirmed_photo_reference_ids(
            &ctx.db,
            workspace_id,
            &confirmed_photo_reference_ids_from_plan(submitted_plan.as_ref()),
        )
        .await?;
        let customization_plan =
            synchronize_photo_reference_placements(submitted_plan, &confirmed_photo_references);
        let actor_id = common::actor_user_id(headers)?;
        let source = crate::repositories::storybooks::find(&ctx.db, workspace_id, storybook_id)
            .await
            .map_err(common::db_error)?;
        ensure_source_ready_for_customization_api(&source)?;
        ensure_customization_plan_mode(customization_plan.as_ref(), "single")?;
        ensure_source_snapshot_current(&source, customization_plan.as_ref())?;
        ensure_customization_photo_references_active(
            &ctx.db,
            workspace_id,
            customization_plan.as_ref(),
        )
        .await?;
        ensure_customization_photo_references_placed(customization_plan.as_ref())?;
        if let Some(existing_run) =
            crate::repositories::storybook_customization_runs::find_matching_run(
                &ctx.db,
                workspace_id,
                storybook_id,
                "single",
                1,
                customization_plan.as_ref(),
            )
            .await
            .map_err(common::db_error)?
        {
            if let Some(output_storybook_id) = existing_run
                .items
                .iter()
                .find(|item| item.target_child_id == child_id)
                .and_then(|item| item.output_storybook_id)
            {
                return crate::repositories::storybooks::find(
                    &ctx.db,
                    workspace_id,
                    output_storybook_id,
                )
                .await
                .map_err(common::db_error);
            }
            return Err(ApiError::state_conflict_with_code_and_details(
                "customization_run_active",
                "这份定制计划已经有制作运行，请恢复现有进度。",
                json!({
                    "customization_run_id": existing_run.id,
                    "status": existing_run.status,
                    "next_action": "restore_customization_run",
                }),
            ));
        }
        let run_id = match crate::repositories::storybook_customization_runs::create_run(
            &ctx.db,
            crate::repositories::storybook_customization_runs::CreateCustomizationRunInput {
                workspace_id,
                source_storybook_id: storybook_id,
                created_by: actor_id,
                mode: "single".to_string(),
                customization_plan: customization_plan.clone(),
                requested_count: 1,
            },
        )
        .await
        {
            Ok(run_id) => run_id,
            Err(err) => {
                if let Some(existing_run) =
                    crate::repositories::storybook_customization_runs::find_matching_run(
                        &ctx.db,
                        workspace_id,
                        storybook_id,
                        "single",
                        1,
                        customization_plan.as_ref(),
                    )
                    .await
                    .map_err(common::db_error)?
                {
                    return Err(ApiError::state_conflict_with_code_and_details(
                        "customization_run_active",
                        "这份定制计划已经有制作运行，请恢复现有进度。",
                        json!({
                            "customization_run_id": existing_run.id,
                            "status": existing_run.status,
                            "next_action": "restore_customization_run",
                        }),
                    ));
                }
                return Err(common::db_error(err));
            }
        };
        let run_item_id = crate::repositories::storybook_customization_runs::create_run_item(
            &ctx.db,
            crate::repositories::storybook_customization_runs::CreateRunItemInput {
                workspace_id,
                run_id,
                source_storybook_id: storybook_id,
                target_child_id: child_id,
                primary_material: primary_material.clone(),
                generation_input_snapshot:
                    crate::repositories::storybook_customization_runs::run_item_snapshot(
                        storybook_id,
                        child_id,
                        &intensity,
                        primary_material.as_deref(),
                        customization_plan.as_ref(),
                    ),
            },
        )
        .await
        .map_err(common::db_error)?;
        let book_result = crate::repositories::storybooks::derive_custom(
            &ctx.db,
            workspace_id,
            storybook_id,
            actor_id,
            DeriveCustomRequest {
                child_id,
                intensity: intensity.clone(),
                primary_material: primary_material.clone(),
                customization_plan: customization_plan.clone(),
            },
        )
        .await;
        let book = match book_result {
            Ok(book) => {
                crate::repositories::storybook_customization_runs::mark_item_succeeded(
                    &ctx.db,
                    workspace_id,
                    run_item_id,
                    book.id,
                )
                .await
                .map_err(common::db_error)?;
                crate::repositories::storybook_customization_runs::finish_run(
                    &ctx.db,
                    workspace_id,
                    run_id,
                    None,
                )
                .await
                .map_err(common::db_error)?;
                crate::repositories::storybooks::find(&ctx.db, workspace_id, book.id)
                    .await
                    .map_err(common::db_error)?
            }
            Err(err) => {
                let failure_reason = err.to_string();
                crate::repositories::storybook_customization_runs::mark_item_failed(
                    &ctx.db,
                    workspace_id,
                    run_item_id,
                    &failure_reason,
                )
                .await
                .map_err(common::db_error)?;
                crate::repositories::storybook_customization_runs::finish_run(
                    &ctx.db,
                    workspace_id,
                    run_id,
                    Some(&failure_reason),
                )
                .await
                .map_err(common::db_error)?;
                return Err(common::db_error(err));
            }
        };
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "storybook.custom_derived",
            "storybook",
            Some(book.id),
            json!({
                "source_storybook_id": storybook_id,
                "customization_run_id": run_id,
                "customization_run_item_id": run_item_id,
                "target_child_id": child_id,
                "intensity": intensity,
                "title": book.title,
                "primary_material": primary_material,
                "customization_plan": customization_plan,
                "customization_plan_summary": customization_plan_audit_summary(customization_plan.as_ref()),
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(book);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_editor(&state, headers, workspace_id)?;
        validate_custom_single_payload(&payload)?;
        let mut state = state.write().expect("state lock poisoned");
        let source = state
            .storybooks
            .iter()
            .find(|item| item.workspace_id == workspace_id && item.id == storybook_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("storybook"))?;
        if source.storybook_type != StorybookType::Plain {
            return Err(ApiError::state_conflict("只有普通绘本可以派生定制绘本"));
        }
        ensure_in_memory_source_ready(&source)?;
        let child = state
            .children
            .iter()
            .find(|item| item.workspace_id == workspace_id && item.id == payload.child_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("child"))?;
        let book = build_mock_custom_book(
            workspace_id,
            source,
            child.id,
            &child.nickname,
            &payload.intensity,
        );
        state.storybooks.push(book.clone());
        Ok(book)
    }
}

fn retry_intensity_from_snapshot(snapshot: &serde_json::Value) -> String {
    snapshot
        .get("intensity")
        .and_then(|value| value.as_str())
        .or_else(|| {
            snapshot
                .get("customization_plan")
                .and_then(|plan| plan.get("intensity"))
                .and_then(|value| value.as_str())
        })
        .unwrap_or("standard")
        .to_string()
}

#[cfg(feature = "db")]
async fn existing_batch_response(
    db: &sea_orm::DatabaseConnection,
    source_storybook_id: Uuid,
    run: StorybookCustomizationRun,
) -> Result<DeriveCustomBatchResponse, sea_orm::DbErr> {
    let mut storybooks = Vec::new();
    let mut items = Vec::with_capacity(run.items.len());
    for item in &run.items {
        let storybook = match item.output_storybook_id {
            Some(output_storybook_id) => {
                let book = crate::repositories::storybooks::find(
                    db,
                    run.workspace_id,
                    output_storybook_id,
                )
                .await?;
                storybooks.push(book.clone());
                Some(book)
            }
            None => None,
        };
        items.push(DeriveCustomBatchItem {
            child_id: item.target_child_id,
            run_item_id: Some(item.id),
            status: if storybook.is_some() {
                "created".to_string()
            } else {
                item.status.clone()
            },
            storybook,
            failure_reason: item.failure_reason.clone(),
        });
    }
    Ok(DeriveCustomBatchResponse {
        source_storybook_id,
        run_id: Some(run.id),
        requested_count: run.requested_count,
        created_count: storybooks.len(),
        storybooks,
        items,
    })
}

#[cfg(feature = "db")]
async fn enqueue_customization_derivation(
    ctx: &AppContext,
    workspace_id: Uuid,
    actor_id: Uuid,
    source_storybook_id: Uuid,
    run_id: Uuid,
    run_item_id: Uuid,
    child_id: Uuid,
    intensity: &str,
    primary_material: Option<String>,
    customization_plan: Option<serde_json::Value>,
) -> Result<(), ApiError> {
    let job = crate::repositories::generation::create_generation_job_record(
        &ctx.db,
        workspace_id,
        actor_id,
        crate::models::CreateGenerationJobRequest {
            job_type: "storybook_customization_derive".to_string(),
            storybook_id: Some(source_storybook_id),
            input_json: json!({
                "customization_run_id": run_id,
                "customization_run_item_id": run_item_id,
                "source_storybook_id": source_storybook_id,
                "target_child_id": child_id,
                "intensity": intensity,
                "primary_material": primary_material,
                "customization_plan": customization_plan,
            }),
        },
    )
    .await
    .map_err(common::db_error)?;
    if let Err(err) =
        crate::workers::generation::enqueue_generation_job(ctx, workspace_id, job.id).await
    {
        let _ =
            crate::repositories::generation::cancel_generation_job(&ctx.db, workspace_id, job.id)
                .await;
        let _ = crate::repositories::storybook_customization_runs::mark_canceled_item_failed(
            &ctx.db,
            workspace_id,
            run_item_id,
            "定制任务暂时无法入队，请重试",
        )
        .await;
        return Err(ApiError::state_conflict(format!(
            "定制制作任务入队失败：{err}"
        )));
    }
    Ok(())
}

#[cfg(feature = "db")]
fn attach_single_run_identity(
    customization_plan: Option<serde_json::Value>,
    child_id: Uuid,
    target_child_nickname: Option<String>,
    primary_material: Option<String>,
) -> Option<serde_json::Value> {
    let mut plan = customization_plan.unwrap_or_else(|| serde_json::json!({}));
    if let Some(object) = plan.as_object_mut() {
        object.insert(
            "target_child_id".to_string(),
            serde_json::Value::String(child_id.to_string()),
        );
        if let Some(nickname) = target_child_nickname {
            object.insert(
                "target_child_nickname".to_string(),
                serde_json::Value::String(nickname),
            );
        }
        object.insert(
            "primary_material".to_string(),
            serde_json::Value::String(primary_material.unwrap_or_else(|| "profile".to_string())),
        );
    }
    Some(plan)
}

#[cfg(feature = "db")]
fn attach_batch_material_choices(
    customization_plan: Option<serde_json::Value>,
    child_ids: &[Uuid],
    material_choices: &HashMap<Uuid, String>,
) -> Option<serde_json::Value> {
    let mut plan = customization_plan.unwrap_or_else(|| serde_json::json!({}));
    if let Some(object) = plan.as_object_mut() {
        object.insert(
            "target_child_ids".to_string(),
            serde_json::Value::Array(
                child_ids
                    .iter()
                    .map(|child_id| serde_json::Value::String(child_id.to_string()))
                    .collect(),
            ),
        );
        object.insert(
            "material_choices".to_string(),
            serde_json::Value::Object(
                material_choices
                    .iter()
                    .map(|(child_id, choice)| {
                        (
                            child_id.to_string(),
                            serde_json::Value::String(choice.clone()),
                        )
                    })
                    .collect(),
            ),
        );
    }
    Some(plan)
}

#[cfg(feature = "db")]
fn attach_batch_target_children(
    customization_plan: Option<serde_json::Value>,
    target_children: &[(Uuid, String)],
) -> Option<serde_json::Value> {
    let mut plan = customization_plan.unwrap_or_else(|| serde_json::json!({}));
    if let Some(object) = plan.as_object_mut() {
        object.insert(
            "target_children".to_string(),
            serde_json::Value::Array(
                target_children
                    .iter()
                    .map(|(child_id, nickname)| {
                        serde_json::json!({
                            "id": child_id,
                            "nickname": nickname,
                        })
                    })
                    .collect(),
            ),
        );
    }
    Some(plan)
}

#[cfg(feature = "db")]
async fn validate_customization_plan_payload(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    workspace: &crate::models::Workspace,
    payload: &BuildCustomizationPlanRequest,
) -> Result<Vec<(Uuid, String)>, ApiError> {
    if !matches!(payload.mode.as_str(), "single" | "batch") {
        return Err(ApiError::validation(
            "mode",
            "定制计划模式只能是 single 或 batch",
        ));
    }
    let target_child_ids = if payload.mode == "single" {
        vec![
            payload
                .target_child_id
                .ok_or_else(|| ApiError::validation("target_child_id", "请选择定制对象"))?,
        ]
    } else {
        if payload.target_child_ids.is_empty() {
            return Err(ApiError::validation(
                "target_child_ids",
                "请选择至少一个定制对象",
            ));
        }
        if payload.target_child_ids.len() > 30 {
            return Err(ApiError::validation(
                "target_child_ids",
                "一次最多为 30 个儿童生成定制绘本",
            ));
        }
        payload.target_child_ids.clone()
    };
    let unique = target_child_ids.iter().copied().collect::<HashSet<_>>();
    if unique.len() != target_child_ids.len() {
        return Err(ApiError::validation(
            "target_child_ids",
            "定制对象不能重复选择",
        ));
    }
    let mut target_children = Vec::with_capacity(target_child_ids.len());
    if let Some(classrooms) =
        common::child_classroom_scope(ctx, headers, workspace_id, workspace).await?
    {
        for child_id in &target_child_ids {
            let child = crate::repositories::children::find_for_classrooms(
                &ctx.db,
                workspace_id,
                *child_id,
                &classrooms,
            )
            .await
            .map_err(common::db_error)?;
            target_children.push((child.id, child.nickname));
        }
    } else {
        for child_id in &target_child_ids {
            let child = crate::repositories::children::find(&ctx.db, workspace_id, *child_id)
                .await
                .map_err(common::db_error)?;
            target_children.push((child.id, child.nickname));
        }
    }
    Ok(target_children)
}

#[cfg(not(feature = "db"))]
fn in_memory_target_children(
    state: &crate::state::AppState,
    workspace_id: Uuid,
    payload: &BuildCustomizationPlanRequest,
) -> Result<Vec<(Uuid, String)>, ApiError> {
    if !matches!(payload.mode.as_str(), "single" | "batch") {
        return Err(ApiError::validation(
            "mode",
            "定制计划模式只能是 single 或 batch",
        ));
    }
    let target_child_ids = if payload.mode == "single" {
        vec![
            payload
                .target_child_id
                .ok_or_else(|| ApiError::validation("target_child_id", "请选择定制对象"))?,
        ]
    } else {
        payload.target_child_ids.clone()
    };
    let mut target_children = Vec::with_capacity(target_child_ids.len());
    for child_id in target_child_ids {
        let child = state
            .children
            .iter()
            .find(|item| item.workspace_id == workspace_id && item.id == child_id)
            .ok_or_else(|| ApiError::not_found("child"))?;
        target_children.push((child.id, child.nickname.clone()));
    }
    Ok(target_children)
}

fn build_source_customization_plan(
    source: &Storybook,
    payload: &BuildCustomizationPlanRequest,
    target_children: &[(Uuid, String)],
    confirmed_photo_references: &[crate::models::StorybookAssetReference],
) -> serde_json::Value {
    let optional_keep_page_ids = payload
        .optional_keep_page_ids
        .iter()
        .map(Uuid::to_string)
        .collect::<HashSet<_>>();
    let preview_pages = source
        .pages
        .iter()
        .take(6)
        .map(|page| {
            json!({
                "id": page.id,
                "page_number": page.page_number,
                "title": page.title,
                "status": page.status,
            })
        })
        .collect::<Vec<_>>();
    let mut page_plan = source
        .pages
        .iter()
        .map(|page| {
            let page_id = page.id.to_string();
            let base_decision = if page.page_number == 1 {
                "keep"
            } else if page.page_number % 3 == 0 {
                "redraw_required"
            } else if optional_keep_page_ids.contains(&page_id) {
                "prefer_keep"
            } else {
                "personalize"
            };
            let reason = match base_decision {
                "keep" => "保留开场节奏和来源书主线。",
                "prefer_keep" => "用户选择尽量保持，制作时只在图文冲突时重绘。",
                "redraw_required" => "定制对象或画面元素变化较明显，需要重绘。",
                _ => "替换为对象版本，保持原书阅读节奏。",
            };
            json!({
                "source_page_id": page.id,
                "page_number": page.page_number,
                "decision": base_decision,
                "title": page.title,
                "reason": reason,
            })
        })
        .collect::<Vec<serde_json::Value>>();
    attach_photo_references_to_page_plan(&mut page_plan, confirmed_photo_references);
    json!({
        "entry_type": "from_storybook",
        "mode": payload.mode,
        "source_storybook_id": source.id,
        "source_storybook_title": source.title,
        "source_snapshot": {
            "storybook_id": source.id,
            "title": source.title,
            "status": storybook_status_name(&source.status),
            "updated_at": source.updated_at,
            "page_count": source.pages.len(),
            "page_ids": source.pages.iter().map(|page| page.id.to_string()).collect::<Vec<_>>(),
            "preview_pages": preview_pages,
        },
        "target_child_id": payload.target_child_id,
        "target_child_nickname": target_children.first().map(|(_, nickname)| nickname),
        "target_child_ids": payload.target_child_ids.iter().map(Uuid::to_string).collect::<Vec<_>>(),
        "target_children": target_children.iter().map(|(id, nickname)| json!({
            "id": id,
            "nickname": nickname,
        })).collect::<Vec<_>>(),
        "primary_material": payload.primary_material,
        "page_plan": page_plan,
        "optional_keep_page_ids": payload.optional_keep_page_ids.iter().map(Uuid::to_string).collect::<Vec<_>>(),
        "confirmed_photo_reference_ids": payload.confirmed_photo_reference_ids.iter().map(Uuid::to_string).collect::<Vec<_>>(),
        "confirmed_photo_references": confirmed_photo_references.iter().map(|reference| confirmed_photo_reference_json(reference, &page_plan)).collect::<Vec<_>>(),
    })
}

#[cfg(feature = "db")]
async fn confirmed_photo_references_for_plan(
    db: &sea_orm::DatabaseConnection,
    workspace_id: Uuid,
    payload: &BuildCustomizationPlanRequest,
) -> Result<Vec<crate::models::StorybookAssetReference>, ApiError> {
    validate_confirmed_photo_reference_ids(db, workspace_id, &payload.confirmed_photo_reference_ids)
        .await
}

#[cfg(feature = "db")]
async fn validate_confirmed_photo_reference_ids(
    db: &sea_orm::DatabaseConnection,
    workspace_id: Uuid,
    confirmed_photo_reference_ids: &[Uuid],
) -> Result<Vec<crate::models::StorybookAssetReference>, ApiError> {
    if confirmed_photo_reference_ids.is_empty() {
        return Ok(Vec::new());
    }
    let references = crate::repositories::storybook_creation_assets::list_by_ids(
        db,
        workspace_id,
        confirmed_photo_reference_ids,
    )
    .await
    .map_err(common::db_error)?;
    let found_ids = references
        .iter()
        .map(|reference| reference.id)
        .collect::<HashSet<_>>();
    let mut revoked_ids = confirmed_photo_reference_ids
        .iter()
        .copied()
        .filter(|id| !found_ids.contains(id))
        .collect::<Vec<_>>();
    revoked_ids.extend(
        references
            .iter()
            .filter(|reference| matches!(reference.status.as_str(), "revoked" | "unused"))
            .map(|reference| reference.id),
    );
    if !revoked_ids.is_empty() {
        return Err(ApiError::state_conflict_with_code_and_details(
            "asset_revoked",
            "照片素材已被移除，请重新预览后再制作。",
            json!({
                "revoked_asset_reference_ids": revoked_ids,
                "next_action": "refresh_customization_plan",
            }),
        ));
    }
    let mut blocking_ids = Vec::new();
    blocking_ids.extend(
        references
            .iter()
            .filter(|reference| {
                reference.status != "ready"
                    || !matches!(
                        reference
                            .visual_reference
                            .as_ref()
                            .map(|item| item.status.as_str()),
                        Some("confirmed")
                    )
            })
            .map(|reference| reference.id),
    );
    if !blocking_ids.is_empty() {
        return Err(ApiError::state_conflict_with_code_and_details(
            "visual_reference_required",
            "先确认照片的同画风参考，再生成定制计划。",
            json!({
                "blocking_asset_reference_ids": blocking_ids,
                "next_action": "confirm_visual_reference",
            }),
        ));
    }
    Ok(references)
}

#[cfg(feature = "db")]
async fn ensure_customization_photo_references_active(
    db: &sea_orm::DatabaseConnection,
    workspace_id: Uuid,
    customization_plan: Option<&serde_json::Value>,
) -> Result<(), ApiError> {
    let ids = confirmed_photo_reference_ids_from_plan(customization_plan);
    validate_confirmed_photo_reference_ids(db, workspace_id, &ids).await?;
    Ok(())
}

#[cfg(feature = "db")]
fn confirmed_photo_reference_ids_from_plan(
    customization_plan: Option<&serde_json::Value>,
) -> Vec<Uuid> {
    let mut ids = Vec::new();
    if let Some(plan) = customization_plan {
        if let Some(values) = plan
            .get("confirmed_photo_reference_ids")
            .and_then(|value| value.as_array())
        {
            ids.extend(
                values
                    .iter()
                    .filter_map(|value| value.as_str().and_then(|id| Uuid::parse_str(id).ok())),
            );
        }
        if let Some(references) = plan
            .get("confirmed_photo_references")
            .and_then(|value| value.as_array())
        {
            ids.extend(references.iter().filter_map(|reference| {
                reference
                    .get("asset_reference_id")
                    .and_then(|value| value.as_str())
                    .and_then(|id| Uuid::parse_str(id).ok())
            }));
        }
    }
    ids.into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
}

fn attach_photo_references_to_page_plan(
    page_plan: &mut [serde_json::Value],
    confirmed_photo_references: &[crate::models::StorybookAssetReference],
) {
    let customizable_page_indexes = page_plan
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            item.get("decision")
                .and_then(|value| value.as_str())
                .is_some_and(|decision| matches!(decision, "personalize" | "redraw_required"))
                .then_some(index)
        })
        .collect::<Vec<_>>();
    let mut character_reference_ids_by_page = vec![Vec::new(); page_plan.len()];

    for reference in confirmed_photo_references
        .iter()
        .filter(|reference| reference.kind == "person")
    {
        if reference.usage.as_deref() == Some("main_character") {
            for page_index in &customizable_page_indexes {
                character_reference_ids_by_page[*page_index]
                    .push(serde_json::Value::String(reference.id.to_string()));
            }
        }
    }

    for (index, item) in page_plan.iter_mut().enumerate() {
        let should_place = customizable_page_indexes.contains(&index);
        if should_place && let Some(object) = item.as_object_mut() {
            object.insert(
                "character_reference_ids".to_string(),
                serde_json::Value::Array(character_reference_ids_by_page[index].clone()),
            );
            object.insert(
                "prop_reference_ids".to_string(),
                serde_json::Value::Array(Vec::new()),
            );
            // Non-main characters, props, and scenes require an explicit page selection.
            object.insert(
                "scene_reference_ids".to_string(),
                serde_json::Value::Array(Vec::new()),
            );
        }
    }
}

fn photo_reference_type(kind: &str) -> &'static str {
    match kind {
        "person" => "character_reference",
        "scene" => "scene_reference",
        _ => "prop_reference",
    }
}

fn photo_reference_type_label(kind: &str) -> &'static str {
    match kind {
        "person" => "角色参考",
        "scene" => "场景参考",
        _ => "道具参考",
    }
}

fn planned_photo_pages(
    reference: &crate::models::StorybookAssetReference,
    page_plan: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    let reference_type = photo_reference_type(reference.kind.as_str());
    let reference_id = reference.id.to_string();
    let reference_ids_field = match reference_type {
        "character_reference" => "character_reference_ids",
        "prop_reference" => "prop_reference_ids",
        _ => "scene_reference_ids",
    };
    page_plan
        .iter()
        .filter(|item| {
            item.get(reference_ids_field)
                .and_then(|value| value.as_array())
                .is_some_and(|ids| {
                    ids.iter()
                        .any(|id| id.as_str() == Some(reference_id.as_str()))
                })
        })
        .map(|item| {
            json!({
                "source_page_id": item.get("source_page_id").and_then(|value| value.as_str()),
                "page_number": item.get("page_number").and_then(|value| value.as_u64()),
                "title": item.get("title").and_then(|value| value.as_str()),
                "decision": item.get("decision").and_then(|value| value.as_str()),
                "reason": item.get("reason").and_then(|value| value.as_str()),
            })
        })
        .collect()
}

fn confirmed_photo_reference_json(
    reference: &crate::models::StorybookAssetReference,
    page_plan: &[serde_json::Value],
) -> serde_json::Value {
    let reference_type = photo_reference_type(reference.kind.as_str());
    let planned_pages = planned_photo_pages(reference, page_plan);
    let placement_scope = "page";
    let unplaced_reason = if planned_pages.is_empty() {
        Some("page_selection_required")
    } else {
        None
    };
    json!({
        "asset_reference_id": reference.id,
        "asset_id": reference.asset_id,
        "visual_reference_id": reference.visual_reference.as_ref().map(|item| item.id),
        "kind": reference.kind,
        "display_name": reference.display_name,
        "usage": reference.usage,
        "reference_type": reference_type,
        "reference_type_label": photo_reference_type_label(reference.kind.as_str()),
        "placement_scope": placement_scope,
        "planned_pages": planned_pages,
        "unplaced_reason": unplaced_reason,
    })
}

fn synchronize_photo_reference_placements(
    customization_plan: Option<serde_json::Value>,
    confirmed_photo_references: &[crate::models::StorybookAssetReference],
) -> Option<serde_json::Value> {
    let mut plan = customization_plan?;
    let reference_fields = confirmed_photo_references
        .iter()
        .map(|reference| {
            let field = match photo_reference_type(reference.kind.as_str()) {
                "character_reference" => "character_reference_ids",
                "scene_reference" => "scene_reference_ids",
                _ => "prop_reference_ids",
            };
            (reference.id.to_string(), field)
        })
        .collect::<HashMap<_, _>>();

    if let Some(page_plan) = plan
        .get_mut("page_plan")
        .and_then(serde_json::Value::as_array_mut)
    {
        for page in page_plan {
            let is_customizable = page
                .get("decision")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|decision| matches!(decision, "personalize" | "redraw_required"));
            let Some(page_object) = page.as_object_mut() else {
                continue;
            };
            for field in [
                "character_reference_ids",
                "prop_reference_ids",
                "scene_reference_ids",
            ] {
                let valid_ids = page_object
                    .get(field)
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(serde_json::Value::as_str)
                    .filter(|id| is_customizable && reference_fields.get(*id) == Some(&field))
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                page_object.insert(
                    field.to_string(),
                    serde_json::Value::Array(
                        valid_ids
                            .into_iter()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );
            }
        }
    }

    let page_plan = plan
        .get("page_plan")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(object) = plan.as_object_mut() {
        object.insert(
            "confirmed_photo_reference_ids".to_string(),
            json!(
                confirmed_photo_references
                    .iter()
                    .map(|reference| reference.id.to_string())
                    .collect::<Vec<_>>()
            ),
        );
        object.insert(
            "confirmed_photo_references".to_string(),
            json!(
                confirmed_photo_references
                    .iter()
                    .map(|reference| confirmed_photo_reference_json(reference, &page_plan))
                    .collect::<Vec<_>>()
            ),
        );
    }
    Some(plan)
}

pub async fn derive_custom_batch(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    payload: DeriveCustomBatchRequest,
) -> Result<DeriveCustomBatchResponse, ApiError> {
    #[cfg(feature = "db")]
    {
        let workspace = common::require_editor_db(ctx, headers, workspace_id).await?;
        validate_custom_batch_payload(&payload)?;
        let source = crate::repositories::storybooks::find(&ctx.db, workspace_id, storybook_id)
            .await
            .map_err(common::db_error)?;
        ensure_source_ready_for_customization_api(&source)?;
        let target_children = if let Some(classrooms) =
            common::child_classroom_scope(ctx, headers, workspace_id, &workspace).await?
        {
            let mut children = Vec::with_capacity(payload.child_ids.len());
            for child_id in &payload.child_ids {
                let child = crate::repositories::children::find_for_classrooms(
                    &ctx.db,
                    workspace_id,
                    *child_id,
                    &classrooms,
                )
                .await
                .map_err(common::db_error)?;
                children.push((child.id, child.nickname));
            }
            children
        } else {
            let mut children = Vec::with_capacity(payload.child_ids.len());
            for child_id in &payload.child_ids {
                let child = crate::repositories::children::find(&ctx.db, workspace_id, *child_id)
                    .await
                    .map_err(common::db_error)?;
                children.push((child.id, child.nickname));
            }
            children
        };
        let submitted_plan = attach_batch_target_children(
            attach_batch_material_choices(
                payload.customization_plan.clone(),
                &payload.child_ids,
                &payload.material_choices,
            ),
            &target_children,
        );
        let confirmed_photo_references = validate_confirmed_photo_reference_ids(
            &ctx.db,
            workspace_id,
            &confirmed_photo_reference_ids_from_plan(submitted_plan.as_ref()),
        )
        .await?;
        let run_customization_plan =
            synchronize_photo_reference_placements(submitted_plan, &confirmed_photo_references);
        let single_target_plan = payload.child_ids.len() == 1
            && run_customization_plan
                .as_ref()
                .and_then(|plan| plan.get("mode"))
                .and_then(|value| value.as_str())
                == Some("single");
        let run_mode = if single_target_plan {
            "single"
        } else {
            "batch"
        };
        if !single_target_plan {
            ensure_customization_plan_mode(run_customization_plan.as_ref(), "batch")?;
        }
        ensure_source_snapshot_current(&source, run_customization_plan.as_ref())?;
        ensure_customization_photo_references_active(
            &ctx.db,
            workspace_id,
            run_customization_plan.as_ref(),
        )
        .await?;
        ensure_customization_photo_references_placed(run_customization_plan.as_ref())?;

        let storybooks = Vec::new();
        let mut items = Vec::with_capacity(payload.child_ids.len());
        let actor_id = common::actor_user_id(headers)?;
        if let Some(existing_run) =
            crate::repositories::storybook_customization_runs::find_matching_run(
                &ctx.db,
                workspace_id,
                storybook_id,
                run_mode,
                payload.child_ids.len(),
                run_customization_plan.as_ref(),
            )
            .await
            .map_err(common::db_error)?
        {
            return existing_batch_response(&ctx.db, storybook_id, existing_run)
                .await
                .map_err(common::db_error);
        }
        let run_id = match crate::repositories::storybook_customization_runs::create_run(
            &ctx.db,
            crate::repositories::storybook_customization_runs::CreateCustomizationRunInput {
                workspace_id,
                source_storybook_id: storybook_id,
                created_by: actor_id,
                mode: run_mode.to_string(),
                customization_plan: run_customization_plan.clone(),
                requested_count: payload.child_ids.len(),
            },
        )
        .await
        {
            Ok(run_id) => run_id,
            Err(err) => {
                if let Some(existing_run) =
                    crate::repositories::storybook_customization_runs::find_matching_run(
                        &ctx.db,
                        workspace_id,
                        storybook_id,
                        run_mode,
                        payload.child_ids.len(),
                        run_customization_plan.as_ref(),
                    )
                    .await
                    .map_err(common::db_error)?
                {
                    return existing_batch_response(&ctx.db, storybook_id, existing_run)
                        .await
                        .map_err(common::db_error);
                }
                return Err(common::db_error(err));
            }
        };
        for child_id in &payload.child_ids {
            let primary_material = payload.material_choices.get(child_id).cloned();
            let customization_plan = merge_primary_material(
                run_customization_plan.clone(),
                primary_material.clone(),
                *child_id,
            );
            let run_item_id = crate::repositories::storybook_customization_runs::create_run_item(
                &ctx.db,
                crate::repositories::storybook_customization_runs::CreateRunItemInput {
                    workspace_id,
                    run_id,
                    source_storybook_id: storybook_id,
                    target_child_id: *child_id,
                    primary_material: primary_material.clone(),
                    generation_input_snapshot:
                        crate::repositories::storybook_customization_runs::run_item_snapshot(
                            storybook_id,
                            *child_id,
                            &payload.intensity,
                            primary_material.as_deref(),
                            customization_plan.as_ref(),
                        ),
                },
            )
            .await
            .map_err(common::db_error)?;
            if let Err(_err) = enqueue_customization_derivation(
                ctx,
                workspace_id,
                actor_id,
                storybook_id,
                run_id,
                run_item_id,
                *child_id,
                &payload.intensity,
                primary_material,
                customization_plan,
            )
            .await
            {
                let failure_reason = "定制任务暂时无法入队，请重试".to_string();
                crate::repositories::storybook_customization_runs::mark_item_failed(
                    &ctx.db,
                    workspace_id,
                    run_item_id,
                    &failure_reason,
                )
                .await
                .map_err(common::db_error)?;
                items.push(DeriveCustomBatchItem {
                    child_id: *child_id,
                    run_item_id: Some(run_item_id),
                    status: "failed".to_string(),
                    storybook: None,
                    failure_reason: Some(failure_reason),
                });
            } else {
                items.push(DeriveCustomBatchItem {
                    child_id: *child_id,
                    run_item_id: Some(run_item_id),
                    status: "queued".to_string(),
                    storybook: None,
                    failure_reason: None,
                });
            }
        }
        crate::repositories::storybook_customization_runs::finish_run(
            &ctx.db,
            workspace_id,
            run_id,
            None,
        )
        .await
        .map_err(common::db_error)?;

        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
                "storybook.custom_batch_queued",
            "storybook",
            Some(storybook_id),
            json!({
                "source_storybook_id": storybook_id,
                "customization_run_id": run_id,
                "target_child_ids": &payload.child_ids,
                "intensity": &payload.intensity,
                "created_count": 0,
                "customization_plan": &run_customization_plan,
                "customization_plan_summary": customization_plan_audit_summary(run_customization_plan.as_ref()),
                "material_choices_count": payload.material_choices.len(),
            }),
        )
        .await
        .map_err(common::db_error)?;

        return Ok(DeriveCustomBatchResponse {
            source_storybook_id: storybook_id,
            run_id: Some(run_id),
            requested_count: payload.child_ids.len(),
            created_count: 0,
            storybooks,
            items,
        });
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_editor(&state, headers, workspace_id)?;
        validate_custom_batch_payload(&payload)?;
        let mut state = state.write().expect("state lock poisoned");
        let source = state
            .storybooks
            .iter()
            .find(|item| item.workspace_id == workspace_id && item.id == storybook_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("storybook"))?;
        if source.storybook_type != StorybookType::Plain {
            return Err(ApiError::state_conflict("只有普通绘本可以派生定制绘本"));
        }
        ensure_in_memory_source_ready(&source)?;

        let mut storybooks = Vec::with_capacity(payload.child_ids.len());
        let mut items = Vec::with_capacity(payload.child_ids.len());
        for child_id in &payload.child_ids {
            let child = state
                .children
                .iter()
                .find(|item| item.workspace_id == workspace_id && item.id == *child_id)
                .cloned()
                .ok_or_else(|| ApiError::not_found("child"))?;
            let book = build_mock_custom_book(
                workspace_id,
                source.clone(),
                child.id,
                &child.nickname,
                &payload.intensity,
            );
            state.storybooks.push(book.clone());
            items.push(DeriveCustomBatchItem {
                child_id: *child_id,
                run_item_id: None,
                status: "created".to_string(),
                storybook: Some(book.clone()),
                failure_reason: None,
            });
            storybooks.push(book);
        }

        Ok(DeriveCustomBatchResponse {
            source_storybook_id: storybook_id,
            run_id: None,
            requested_count: payload.child_ids.len(),
            created_count: storybooks.len(),
            storybooks,
            items,
        })
    }
}

#[cfg(not(feature = "db"))]
fn ensure_in_memory_source_ready(source: &Storybook) -> Result<(), ApiError> {
    if !matches!(
        source.status,
        StorybookStatus::Exportable | StorybookStatus::Listed
    ) {
        return Err(ApiError::state_conflict(
            "普通绘本尚未完成验收，暂不能生成定制版本",
        ));
    }
    if source.pages.is_empty() || source.roles.is_empty() {
        return Err(ApiError::state_conflict(
            "普通绘本的分页或角色资料不完整，暂不能生成定制版本",
        ));
    }
    if source.pages.iter().any(|page| {
        matches!(
            page.status.as_str(),
            "generating" | "failed" | "needs_regeneration"
        )
    }) {
        return Err(ApiError::state_conflict(
            "普通绘本仍有插图未完成或需要重绘，暂不能生成定制版本",
        ));
    }
    if source.quality.status == crate::models::StorybookQualityStatus::Blocked {
        return Err(ApiError::state_conflict(
            "普通绘本存在质量阻断项，修正后才能生成定制版本",
        ));
    }
    Ok(())
}

fn validate_custom_single_payload(payload: &DeriveCustomRequest) -> Result<(), ApiError> {
    if !matches!(payload.intensity.as_str(), "quick" | "standard") {
        return Err(ApiError::validation(
            "intensity",
            "定制强度只能是 quick 或 standard",
        ));
    }
    if payload
        .primary_material
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        return Err(ApiError::validation(
            "primary_material",
            "请确认主素材，或选择只使用称呼",
        ));
    }
    Ok(())
}

#[cfg(feature = "db")]
fn ensure_customization_plan_mode(
    customization_plan: Option<&serde_json::Value>,
    expected_mode: &str,
) -> Result<(), ApiError> {
    let Some(actual_mode) = customization_plan
        .and_then(|plan| plan.get("mode"))
        .and_then(|value| value.as_str())
    else {
        return Ok(());
    };
    if actual_mode == expected_mode {
        return Ok(());
    }
    Err(ApiError::state_conflict_with_code_and_details(
        "plan_mode_mismatch",
        "定制计划模式和本次制作模式不一致，请重新预览变化后再制作。",
        json!({
            "expected_mode": expected_mode,
            "actual_mode": actual_mode,
            "next_action": "refresh_customization_plan",
        }),
    ))
}

#[cfg(feature = "db")]
fn ensure_source_ready_for_customization_api(source: &Storybook) -> Result<(), ApiError> {
    if source.storybook_type != StorybookType::Plain {
        return Err(ApiError::state_conflict("只有普通绘本可以派生定制绘本"));
    }
    if !matches!(
        source.status,
        crate::models::StorybookStatus::Exportable | crate::models::StorybookStatus::Listed
    ) {
        return Err(ApiError::state_conflict(
            "普通绘本尚未完成验收，暂不能生成定制版本",
        ));
    }
    if source.pages.is_empty() || source.roles.is_empty() {
        return Err(ApiError::state_conflict(
            "普通绘本的分页或角色资料不完整，暂不能生成定制版本",
        ));
    }
    if source.pages.iter().any(|page| {
        matches!(
            page.status.as_str(),
            "generating" | "failed" | "needs_regeneration"
        )
    }) {
        return Err(ApiError::state_conflict(
            "普通绘本仍有插图未完成或需要重绘，暂不能生成定制版本",
        ));
    }
    if source.quality.status == crate::models::StorybookQualityStatus::Blocked {
        return Err(ApiError::state_conflict(
            "普通绘本存在质量阻断项，修正后才能生成定制版本",
        ));
    }
    Ok(())
}

fn validate_custom_batch_payload(payload: &DeriveCustomBatchRequest) -> Result<(), ApiError> {
    if payload.child_ids.is_empty() {
        return Err(ApiError::validation("child_ids", "请选择至少一个儿童档案"));
    }
    if payload.child_ids.len() > 30 {
        return Err(ApiError::validation(
            "child_ids",
            "一次最多为 30 个儿童生成定制绘本",
        ));
    }
    let unique: HashSet<Uuid> = payload.child_ids.iter().copied().collect();
    if unique.len() != payload.child_ids.len() {
        return Err(ApiError::validation("child_ids", "儿童档案不能重复选择"));
    }
    if !matches!(payload.intensity.as_str(), "quick" | "standard") {
        return Err(ApiError::validation(
            "intensity",
            "定制强度只能是 quick 或 standard",
        ));
    }
    for child_id in &payload.child_ids {
        if payload
            .material_choices
            .get(child_id)
            .map(|item| item.trim().is_empty())
            .unwrap_or(true)
        {
            return Err(ApiError::validation(
                "material_choices",
                "请为每个儿童确认一个主素材，或选择只使用称呼",
            ));
        }
    }
    Ok(())
}

#[cfg(feature = "db")]
fn ensure_source_snapshot_current(
    source: &Storybook,
    customization_plan: Option<&serde_json::Value>,
) -> Result<(), ApiError> {
    let Some(snapshot) = customization_plan
        .and_then(|plan| plan.get("source_snapshot"))
        .and_then(|value| value.as_object())
    else {
        return Ok(());
    };

    let expected_storybook_id = snapshot
        .get("storybook_id")
        .and_then(|value| value.as_str());
    let expected_updated_at = snapshot.get("updated_at").and_then(|value| value.as_str());
    let expected_page_count = snapshot.get("page_count").and_then(|value| value.as_u64());
    let expected_page_ids = snapshot
        .get("page_ids")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        });
    let current_page_ids = source
        .pages
        .iter()
        .map(|page| page.id.to_string())
        .collect::<Vec<_>>();
    let current_status = storybook_status_name(&source.status);
    let storybook_id_matches = expected_storybook_id
        .and_then(|value| Uuid::parse_str(value).ok())
        .is_none_or(|expected| expected == source.id);
    let updated_at_matches =
        expected_updated_at.is_none_or(|expected| expected == source.updated_at);
    let page_count_matches =
        expected_page_count.is_none_or(|expected| expected as usize == source.pages.len());
    let page_ids_matches = expected_page_ids
        .as_ref()
        .is_none_or(|expected| expected == &current_page_ids);
    if storybook_id_matches && updated_at_matches && page_count_matches && page_ids_matches {
        return Ok(());
    }

    Err(ApiError::state_conflict_with_code_and_details(
        "source_revision_conflict",
        "来源绘本已更新，请重新预览变化后再制作定制绘本。",
        json!({
            "source_storybook_id": source.id,
            "expected_updated_at": expected_updated_at,
            "current_updated_at": source.updated_at,
            "expected_page_count": expected_page_count,
            "current_page_count": source.pages.len(),
            "current_status": current_status,
            "next_action": "refresh_source_plan",
        }),
    ))
}

#[cfg(feature = "db")]
fn ensure_customization_photo_references_placed(
    customization_plan: Option<&serde_json::Value>,
) -> Result<(), ApiError> {
    let Some(references) = customization_plan
        .and_then(|plan| plan.get("confirmed_photo_references"))
        .and_then(|value| value.as_array())
    else {
        return Ok(());
    };
    let unplaced = references
        .iter()
        .filter(|reference| {
            let planned_pages_empty = reference
                .get("planned_pages")
                .and_then(|value| value.as_array())
                .is_none_or(|pages| pages.is_empty());
            let has_unplaced_reason = reference
                .get("unplaced_reason")
                .and_then(|value| value.as_str())
                .is_some_and(|reason| !reason.trim().is_empty());
            planned_pages_empty || has_unplaced_reason
        })
        .map(|reference| {
            json!({
                "asset_reference_id": reference.get("asset_reference_id").and_then(|value| value.as_str()),
                "display_name": reference.get("display_name").and_then(|value| value.as_str()),
                "unplaced_reason": reference.get("unplaced_reason").and_then(|value| value.as_str()).unwrap_or("no_planned_pages"),
            })
        })
        .collect::<Vec<_>>();
    if unplaced.is_empty() {
        return Ok(());
    }
    Err(ApiError::state_conflict_with_code_and_details(
        "material_unplaced",
        "确认使用的照片还没有故事落点，请重新预览、撤销照片引用或改为不使用。",
        json!({
            "unplaced_photo_references": unplaced,
            "next_action": "refresh_customization_plan",
        }),
    ))
}

#[cfg(feature = "db")]
fn merge_primary_material(
    customization_plan: Option<serde_json::Value>,
    primary_material: Option<String>,
    child_id: Uuid,
) -> Option<serde_json::Value> {
    let mut plan = customization_plan.unwrap_or_else(|| serde_json::json!({}));
    let target_child_nickname = target_child_nickname_from_plan(&plan, child_id);
    if let Some(object) = plan.as_object_mut() {
        object.insert(
            "primary_material".to_string(),
            serde_json::Value::String(primary_material.unwrap_or_else(|| "只使用称呼".to_string())),
        );
        object.insert(
            "target_child_id".to_string(),
            serde_json::Value::String(child_id.to_string()),
        );
        if let Some(nickname) = target_child_nickname {
            object.insert(
                "target_child_nickname".to_string(),
                serde_json::Value::String(nickname),
            );
        }
    }
    Some(plan)
}

#[cfg(feature = "db")]
fn target_child_nickname_from_plan(plan: &serde_json::Value, child_id: Uuid) -> Option<String> {
    let child_id = child_id.to_string();
    plan.get("target_children")
        .and_then(|value| value.as_array())
        .and_then(|children| {
            children.iter().find_map(|child| {
                let id_matches = child
                    .get("id")
                    .and_then(|value| value.as_str())
                    .is_some_and(|value| value == child_id);
                if id_matches {
                    child
                        .get("nickname")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                } else {
                    None
                }
            })
        })
}

#[cfg(feature = "db")]
fn customization_plan_audit_summary(plan: Option<&serde_json::Value>) -> serde_json::Value {
    let Some(plan) = plan else {
        return serde_json::json!({
            "provided": false,
            "page_plan_count": 0,
            "confirmed_photo_reference_count": 0,
            "optional_keep_page_count": 0,
            "source_snapshot_present": false,
            "source_snapshot_page_count": 0,
            "source_snapshot_preview_page_count": 0,
        });
    };
    let page_plan_count = plan
        .get("page_plan")
        .and_then(|value| value.as_array())
        .map(|items| items.len())
        .unwrap_or(0);
    let confirmed_photo_reference_count = plan
        .get("confirmed_photo_reference_ids")
        .and_then(|value| value.as_array())
        .map(|items| items.len())
        .unwrap_or(0);
    let optional_keep_page_count = plan
        .get("optional_keep_page_ids")
        .and_then(|value| value.as_array())
        .map(|items| items.len())
        .unwrap_or(0);
    let source_snapshot = plan
        .get("source_snapshot")
        .and_then(|value| value.as_object());
    let source_snapshot_page_count = source_snapshot
        .and_then(|snapshot| snapshot.get("page_count"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let source_snapshot_preview_page_count = source_snapshot
        .and_then(|snapshot| snapshot.get("preview_pages"))
        .and_then(|value| value.as_array())
        .map(|items| items.len())
        .unwrap_or(0);
    serde_json::json!({
        "provided": true,
        "entry_type": plan.get("entry_type").and_then(|value| value.as_str()),
        "mode": plan.get("mode").and_then(|value| value.as_str()),
        "primary_material": plan.get("primary_material").and_then(|value| value.as_str()),
        "page_plan_count": page_plan_count,
        "confirmed_photo_reference_count": confirmed_photo_reference_count,
        "optional_keep_page_count": optional_keep_page_count,
        "source_snapshot_present": source_snapshot.is_some(),
        "source_snapshot_page_count": source_snapshot_page_count,
        "source_snapshot_preview_page_count": source_snapshot_preview_page_count,
        "source_snapshot_updated_at": source_snapshot
            .and_then(|snapshot| snapshot.get("updated_at"))
            .and_then(|value| value.as_str()),
    })
}

#[cfg(not(feature = "db"))]
fn shared_state(ctx: &AppContext) -> Result<crate::state::SharedState, ApiError> {
    ctx.shared_store
        .get::<crate::state::SharedState>()
        .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))
}

#[cfg(not(feature = "db"))]
fn build_mock_custom_book(
    workspace_id: Uuid,
    source: Storybook,
    child_id: Uuid,
    child_nickname: &str,
    intensity: &str,
) -> Storybook {
    let mut book = source.clone();
    book.id = Uuid::new_v4();
    book.workspace_id = workspace_id;
    book.storybook_type = StorybookType::Custom;
    book.status = StorybookStatus::Editing;
    book.visibility = Visibility::Private;
    book.source = format!("derived:{intensity}");
    book.source_title = Some(source.title);
    book.target_child_id = Some(child_id);
    book.customization_run_id = None;
    book.customization_run_item_id = None;
    book.title = format!("{child_nickname}的定制故事");
    book.teacher_review_status = "pending".to_string();
    book.teacher_reviewed_by = None;
    book.teacher_reviewed_at = None;
    book.updated_at = "刚刚".to_string();
    book
}

#[cfg(all(test, feature = "db"))]
mod tests {
    use super::*;
    use crate::models::{
        StorybookAssetReference, StorybookAssetSummary, StorybookPage, StorybookRole,
        StorybookVisualReferenceSummary,
    };
    use axum::{body::to_bytes, response::IntoResponse};

    #[test]
    fn customization_plan_audit_summary_counts_traceable_inputs() {
        let plan = serde_json::json!({
            "entry_type": "from_storybook",
            "mode": "single",
            "primary_material": "profile",
            "page_plan": [
                { "page_number": 1, "decision": "keep" },
                { "page_number": 2, "decision": "prefer_keep" }
            ],
            "optional_keep_page_ids": ["page-2"],
            "confirmed_photo_reference_ids": ["asset-reference-1", "asset-reference-2"],
            "source_snapshot": {
                "updated_at": "2026-08-21T02:00:00Z",
                "page_count": 6,
                "preview_pages": [
                    { "id": "page-1", "status": "ready" },
                    { "id": "page-2", "status": "ready" }
                ]
            }
        });

        let summary = customization_plan_audit_summary(Some(&plan));

        assert_eq!(summary["provided"], true);
        assert_eq!(summary["entry_type"], "from_storybook");
        assert_eq!(summary["mode"], "single");
        assert_eq!(summary["primary_material"], "profile");
        assert_eq!(summary["page_plan_count"], 2);
        assert_eq!(summary["optional_keep_page_count"], 1);
        assert_eq!(summary["confirmed_photo_reference_count"], 2);
        assert_eq!(summary["source_snapshot_present"], true);
        assert_eq!(summary["source_snapshot_page_count"], 6);
        assert_eq!(summary["source_snapshot_preview_page_count"], 2);
        assert_eq!(
            summary["source_snapshot_updated_at"],
            "2026-08-21T02:00:00Z"
        );
    }

    #[test]
    fn customization_plan_audit_summary_handles_missing_plan() {
        let summary = customization_plan_audit_summary(None);

        assert_eq!(summary["provided"], false);
        assert_eq!(summary["page_plan_count"], 0);
        assert_eq!(summary["optional_keep_page_count"], 0);
        assert_eq!(summary["confirmed_photo_reference_count"], 0);
        assert_eq!(summary["source_snapshot_present"], false);
        assert_eq!(summary["source_snapshot_page_count"], 0);
        assert_eq!(summary["source_snapshot_preview_page_count"], 0);
    }

    #[tokio::test]
    async fn single_customization_payload_requires_confirmed_primary_material() {
        let err = validate_custom_single_payload(&DeriveCustomRequest {
            child_id: Uuid::new_v4(),
            intensity: "standard".to_string(),
            primary_material: None,
            customization_plan: None,
        })
        .expect_err("single customization should require primary material");

        let response = err.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should serialize");
        let body: serde_json::Value = serde_json::from_slice(&bytes).expect("body should be json");
        assert_eq!(body["error"]["code"], "validation_error");
        assert_eq!(body["error"]["field"], "primary_material");
    }

    #[tokio::test]
    async fn single_customization_payload_rejects_unknown_intensity() {
        let err = validate_custom_single_payload(&DeriveCustomRequest {
            child_id: Uuid::new_v4(),
            intensity: "slow".to_string(),
            primary_material: Some("profile".to_string()),
            customization_plan: None,
        })
        .expect_err("single customization should validate intensity");

        let response = err.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should serialize");
        let body: serde_json::Value = serde_json::from_slice(&bytes).expect("body should be json");
        assert_eq!(body["error"]["code"], "validation_error");
        assert_eq!(body["error"]["field"], "intensity");
    }

    #[tokio::test]
    async fn customization_plan_mode_guard_blocks_single_batch_mismatch() {
        let plan = serde_json::json!({
            "entry_type": "from_storybook",
            "mode": "batch"
        });

        let err = ensure_customization_plan_mode(Some(&plan), "single")
            .expect_err("batch plan should not run as single");
        let response = err.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should serialize");
        let body: serde_json::Value = serde_json::from_slice(&bytes).expect("body should be json");
        assert_eq!(body["error"]["code"], "plan_mode_mismatch");
        assert_eq!(body["error"]["details"]["expected_mode"], "single");
        assert_eq!(body["error"]["details"]["actual_mode"], "batch");
    }

    #[tokio::test]
    async fn customization_plan_mode_guard_blocks_batch_single_mismatch() {
        let plan = serde_json::json!({
            "entry_type": "from_storybook",
            "mode": "single"
        });

        let err = ensure_customization_plan_mode(Some(&plan), "batch")
            .expect_err("single plan should not run as batch");
        let response = err.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should serialize");
        let body: serde_json::Value = serde_json::from_slice(&bytes).expect("body should be json");
        assert_eq!(body["error"]["code"], "plan_mode_mismatch");
        assert_eq!(body["error"]["details"]["expected_mode"], "batch");
        assert_eq!(body["error"]["details"]["actual_mode"], "single");
    }

    #[test]
    fn source_ready_guard_accepts_exportable_plain_storybook() {
        ensure_source_ready_for_customization_api(&test_storybook())
            .expect("exportable plain storybook should be customizable");
    }

    #[test]
    fn source_ready_guard_rejects_custom_storybook() {
        let mut source = test_storybook();
        source.storybook_type = StorybookType::Custom;

        ensure_source_ready_for_customization_api(&source)
            .expect_err("custom storybook should not be customized again");
    }

    #[test]
    fn source_ready_guard_rejects_unfinished_storybook() {
        let mut source = test_storybook();
        source.status = crate::models::StorybookStatus::Editing;

        ensure_source_ready_for_customization_api(&source)
            .expect_err("unfinished storybook should not be customizable");
    }

    #[test]
    fn source_ready_guard_rejects_incomplete_storybook_content() {
        let mut source = test_storybook();
        source.roles.clear();

        ensure_source_ready_for_customization_api(&source)
            .expect_err("storybook without roles should not be customizable");
    }

    #[test]
    fn merge_primary_material_freezes_batch_child_input() {
        let child_id = Uuid::new_v4();
        let plan = merge_primary_material(
            Some(serde_json::json!({
                "entry_type": "from_storybook",
                "mode": "batch",
                "target_children": [
                    { "id": child_id, "nickname": "乐乐" }
                ],
            })),
            Some("profile".to_string()),
            child_id,
        )
        .expect("plan should be returned");

        assert_eq!(plan["primary_material"], "profile");
        assert_eq!(plan["target_child_id"], child_id.to_string());
        assert_eq!(plan["target_child_nickname"], "乐乐");
    }

    #[test]
    fn merge_primary_material_defaults_to_name_only() {
        let child_id = Uuid::new_v4();
        let plan = merge_primary_material(None, None, child_id).expect("plan should be returned");

        assert_eq!(plan["primary_material"], "只使用称呼");
        assert_eq!(plan["target_child_id"], child_id.to_string());
    }

    #[test]
    fn source_snapshot_guard_accepts_current_source() {
        let source = test_storybook();
        let page_ids = source
            .pages
            .iter()
            .map(|page| page.id.to_string())
            .collect::<Vec<_>>();
        let plan = serde_json::json!({
            "source_snapshot": {
                "storybook_id": source.id,
                "updated_at": source.updated_at,
                "page_count": source.pages.len(),
                "page_ids": page_ids,
            }
        });

        ensure_source_snapshot_current(&source, Some(&plan))
            .expect("current source snapshot should pass");
    }

    #[tokio::test]
    async fn source_snapshot_guard_rejects_stale_source_with_actionable_code() {
        let source = test_storybook();
        let page_ids = source
            .pages
            .iter()
            .map(|page| page.id.to_string())
            .collect::<Vec<_>>();
        let plan = serde_json::json!({
            "source_snapshot": {
                "storybook_id": source.id,
                "updated_at": "2026-08-20 09:00",
                "page_count": source.pages.len(),
                "page_ids": page_ids,
            }
        });

        let err = ensure_source_snapshot_current(&source, Some(&plan))
            .expect_err("stale source snapshot should conflict");
        let response = err.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should serialize");
        let body: serde_json::Value = serde_json::from_slice(&bytes).expect("body should be json");
        assert_eq!(body["error"]["code"], "source_revision_conflict");
        assert_eq!(
            body["error"]["details"]["next_action"],
            "refresh_source_plan"
        );
        assert_eq!(
            body["error"]["details"]["current_updated_at"],
            source.updated_at
        );
    }

    #[test]
    fn retry_intensity_prefers_frozen_snapshot_value() {
        let snapshot = serde_json::json!({
            "intensity": "quick",
            "customization_plan": {
                "intensity": "standard"
            }
        });

        assert_eq!(retry_intensity_from_snapshot(&snapshot), "quick");
    }

    #[test]
    fn batch_material_choices_become_part_of_run_identity_plan() {
        let child_id = Uuid::new_v4();
        let choices = HashMap::from([(child_id, "profile".to_string())]);
        let plan = attach_batch_target_children(
            attach_batch_material_choices(
                Some(serde_json::json!({
                    "entry_type": "from_storybook",
                    "mode": "batch",
                })),
                &[child_id],
                &choices,
            ),
            &[(child_id, "乐乐".to_string())],
        )
        .expect("plan should be returned");

        assert_eq!(plan["target_child_ids"][0], child_id.to_string());
        assert_eq!(plan["material_choices"][child_id.to_string()], "profile");
        assert_eq!(plan["target_children"][0]["id"], child_id.to_string());
        assert_eq!(plan["target_children"][0]["nickname"], "乐乐");
    }

    #[test]
    fn single_run_identity_plan_freezes_child_and_primary_material() {
        let child_id = Uuid::new_v4();
        let plan = attach_single_run_identity(
            None,
            child_id,
            Some("乐乐".to_string()),
            Some("profile".to_string()),
        )
        .expect("plan should be returned");

        assert_eq!(plan["target_child_id"], child_id.to_string());
        assert_eq!(plan["target_child_nickname"], "乐乐");
        assert_eq!(plan["primary_material"], "profile");
    }

    #[test]
    fn source_customization_plan_freezes_target_nickname() {
        let source = test_storybook();
        let child_id = Uuid::new_v4();
        let plan = build_source_customization_plan(
            &source,
            &BuildCustomizationPlanRequest {
                mode: "single".to_string(),
                target_child_id: Some(child_id),
                target_child_ids: Vec::new(),
                primary_material: Some("profile".to_string()),
                optional_keep_page_ids: vec![source.pages[1].id],
                confirmed_photo_reference_ids: Vec::new(),
            },
            &[(child_id, "乐乐".to_string())],
            &[],
        );

        assert_eq!(plan["target_child_id"], child_id.to_string());
        assert_eq!(plan["target_child_nickname"], "乐乐");
        assert_eq!(plan["target_children"][0]["id"], child_id.to_string());
        assert_eq!(plan["target_children"][0]["nickname"], "乐乐");
        assert_eq!(plan["page_plan"][1]["decision"], "prefer_keep");
    }

    #[test]
    fn confirmed_photo_reference_summary_enters_customization_plan() {
        let reference = test_asset_reference("person", "爸爸", "main_character");
        let page_plan = vec![
            serde_json::json!({
                "source_page_id": Uuid::new_v4(),
                "page_number": 1,
                "title": "第一页",
                "decision": "keep",
            }),
            serde_json::json!({
                "source_page_id": Uuid::new_v4(),
                "page_number": 2,
                "title": "第二页",
                "decision": "personalize",
                "reason": "替换为对象版本，保持原书阅读节奏。",
                "character_reference_ids": [reference.id.to_string()],
                "prop_reference_ids": [],
                "scene_reference_ids": [],
            }),
        ];
        let summary = confirmed_photo_reference_json(&reference, &page_plan);

        assert_eq!(summary["asset_reference_id"], reference.id.to_string());
        assert_eq!(
            summary["visual_reference_id"],
            reference.visual_reference.as_ref().unwrap().id.to_string()
        );
        assert_eq!(summary["reference_type"], "character_reference");
        assert_eq!(summary["reference_type_label"], "角色参考");
        assert_eq!(summary["display_name"], "爸爸");
        assert_eq!(summary["planned_pages"].as_array().unwrap().len(), 1);
        assert_eq!(summary["planned_pages"][0]["page_number"], 2);
        assert_eq!(summary["unplaced_reason"], serde_json::Value::Null);
    }

    #[test]
    fn non_main_photo_references_require_an_explicit_page_selection() {
        let source = test_storybook();
        let child_id = Uuid::new_v4();
        let character_reference = test_asset_reference("person", "爸爸", "main_character");
        let prop_reference = test_asset_reference("object", "小汽车", "story_object");
        let scene_reference = test_asset_reference("scene", "幼儿园操场", "background_scene");
        let character_reference_id = character_reference.id.to_string();
        let prop_reference_id = prop_reference.id.to_string();
        let scene_reference_id = scene_reference.id.to_string();
        let plan = build_source_customization_plan(
            &source,
            &BuildCustomizationPlanRequest {
                mode: "single".to_string(),
                target_child_id: Some(child_id),
                target_child_ids: Vec::new(),
                primary_material: Some("profile".to_string()),
                optional_keep_page_ids: Vec::new(),
                confirmed_photo_reference_ids: vec![
                    character_reference.id,
                    prop_reference.id,
                    scene_reference.id,
                ],
            },
            &[(child_id, "乐乐".to_string())],
            &[character_reference, prop_reference, scene_reference],
        );

        assert_eq!(
            plan["confirmed_photo_references"][0]["planned_pages"][0]["page_number"],
            2
        );
        assert_eq!(
            plan["page_plan"][1]["character_reference_ids"][0],
            character_reference_id
        );
        assert!(
            plan["page_plan"][1]["prop_reference_ids"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(plan["page_plan"][1].get("asset_reference_ids").is_none());
        assert!(
            plan["page_plan"][1]["scene_reference_ids"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        for reference_type in ["prop_reference", "scene_reference"] {
            let reference = plan["confirmed_photo_references"]
                .as_array()
                .unwrap()
                .iter()
                .find(|reference| reference["reference_type"] == reference_type)
                .unwrap();
            assert!(reference["planned_pages"].as_array().unwrap().is_empty());
            assert_eq!(reference["unplaced_reason"], "page_selection_required");
        }
        ensure_customization_photo_references_placed(Some(&plan))
            .expect_err("unselected photo references must block the run");
        assert_ne!(prop_reference_id, scene_reference_id);
    }

    #[test]
    fn photo_reference_placement_is_derived_from_valid_typed_pages() {
        let prop_reference = test_asset_reference("object", "小汽车", "story_object");
        let scene_reference = test_asset_reference("scene", "幼儿园操场", "background_scene");
        let prop_id = prop_reference.id.to_string();
        let scene_id = scene_reference.id.to_string();
        let plan = synchronize_photo_reference_placements(Some(json!({
            "confirmed_photo_references": [
                { "asset_reference_id": prop_id, "reference_type": "scene_reference", "planned_pages": [{ "page_number": 99 }] },
                { "asset_reference_id": scene_id, "reference_type": "prop_reference", "planned_pages": [{ "page_number": 99 }] }
            ],
            "page_plan": [
                { "page_number": 1, "decision": "keep", "prop_reference_ids": [prop_id], "scene_reference_ids": [] },
                { "page_number": 2, "decision": "personalize", "prop_reference_ids": [scene_id, prop_id], "scene_reference_ids": [scene_id] }
            ]
        })), &[prop_reference, scene_reference])
        .expect("plan should remain present");

        assert!(
            plan["page_plan"][0]["prop_reference_ids"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(plan["page_plan"][1]["prop_reference_ids"], json!([prop_id]));
        assert_eq!(
            plan["page_plan"][1]["scene_reference_ids"],
            json!([scene_id])
        );
        assert_eq!(
            plan["confirmed_photo_references"][0]["planned_pages"][0]["page_number"],
            2
        );
        assert_eq!(
            plan["confirmed_photo_references"][1]["planned_pages"][0]["page_number"],
            2
        );
        assert_eq!(
            plan["confirmed_photo_references"][0]["reference_type"],
            "prop_reference"
        );
        assert_eq!(
            plan["confirmed_photo_references"][1]["reference_type"],
            "scene_reference"
        );
        ensure_customization_photo_references_placed(Some(&plan))
            .expect("normalized references should pass the placement gate");
    }

    #[test]
    fn confirmed_photo_reference_ids_are_extracted_from_plan_snapshot() {
        let first_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();
        let plan = serde_json::json!({
            "confirmed_photo_reference_ids": [first_id.to_string()],
            "confirmed_photo_references": [
                {
                    "asset_reference_id": second_id.to_string(),
                    "display_name": "彩虹书包"
                },
                {
                    "asset_reference_id": first_id.to_string(),
                    "display_name": "重复引用"
                }
            ]
        });

        let ids = confirmed_photo_reference_ids_from_plan(Some(&plan));

        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&first_id));
        assert!(ids.contains(&second_id));
    }

    #[tokio::test]
    async fn customization_photo_references_without_pages_are_blocked() {
        let reference_id = Uuid::new_v4();
        let plan = serde_json::json!({
            "confirmed_photo_references": [
                {
                    "asset_reference_id": reference_id,
                    "display_name": "小汽车",
                    "planned_pages": [],
                    "unplaced_reason": "no_customized_page_available"
                }
            ]
        });

        let err = ensure_customization_photo_references_placed(Some(&plan))
            .expect_err("unplaced photo should block run");
        let response = err.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should serialize");
        let body: serde_json::Value = serde_json::from_slice(&bytes).expect("body should be json");
        assert_eq!(body["error"]["code"], "material_unplaced");
        assert_eq!(
            body["error"]["details"]["unplaced_photo_references"][0]["asset_reference_id"],
            reference_id.to_string()
        );
        assert_eq!(
            body["error"]["details"]["next_action"],
            "refresh_customization_plan"
        );
    }

    fn test_storybook() -> Storybook {
        let page_one_id = Uuid::new_v4();
        let page_two_id = Uuid::new_v4();
        Storybook {
            id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            title: "一起玩小汽车".to_string(),
            storybook_type: StorybookType::Plain,
            status: crate::models::StorybookStatus::Exportable,
            visibility: crate::models::Visibility::Private,
            source: "blank".to_string(),
            source_title: None,
            target_child_id: None,
            customization_run_id: None,
            customization_run_item_id: None,
            customization_plan: None,
            creator_name: "林老师".to_string(),
            updated_at: "2026-08-21 09:00".to_string(),
            age_group: "4-5 岁".to_string(),
            use_scene: "规则引导".to_string(),
            teaching_goal: "学习轮流与分享".to_string(),
            cover_tone: "温暖、清楚".to_string(),
            page_aspect_ratio: "portrait_4_5".to_string(),
            teacher_review_status: "confirmed".to_string(),
            teacher_reviewed_by: Some(Uuid::new_v4()),
            teacher_reviewed_at: Some("2026-08-21 09:05".to_string()),
            pages: vec![
                StorybookPage {
                    id: page_one_id,
                    page_number: 1,
                    title: "第一页".to_string(),
                    body: "内容".to_string(),
                    illustration_prompt: "提示".to_string(),
                    status: "ready".to_string(),
                    review_status: "unchecked".to_string(),
                    reviewed_by: None,
                    reviewed_at: None,
                    image_url: Some("/images/page-1.png".to_string()),
                    selected_image_variant_id: Some(Uuid::new_v4()),
                },
                StorybookPage {
                    id: page_two_id,
                    page_number: 2,
                    title: "第二页".to_string(),
                    body: "内容".to_string(),
                    illustration_prompt: "提示".to_string(),
                    status: "ready".to_string(),
                    review_status: "unchecked".to_string(),
                    reviewed_by: None,
                    reviewed_at: None,
                    image_url: Some("/images/page-2.png".to_string()),
                    selected_image_variant_id: Some(Uuid::new_v4()),
                },
            ],
            roles: vec![StorybookRole {
                id: Uuid::new_v4(),
                name: "老师".to_string(),
                role_type: "teacher".to_string(),
                appearance: "温和".to_string(),
                story_function: "帮助孩子".to_string(),
                needs_consistency: false,
                reference_image_url: Some("/images/role.png".to_string()),
                reference_image_prompt: Some("温和老师".to_string()),
                reference_status: "ready".to_string(),
                selected_image_variant_id: Some(Uuid::new_v4()),
            }],
            quality: Default::default(),
        }
    }

    fn test_asset_reference(
        kind: &str,
        display_name: &str,
        usage: &str,
    ) -> StorybookAssetReference {
        StorybookAssetReference {
            id: Uuid::new_v4(),
            asset_id: Uuid::new_v4(),
            asset: StorybookAssetSummary {
                id: Uuid::new_v4(),
                storage_key: "/storybook-assets/source.png".to_string(),
                status: "ready".to_string(),
                processing_message: None,
                content_type: "image/png".to_string(),
                byte_size: 128,
                width: Some(2),
                height: Some(2),
                visibility_scope: "creation_session".to_string(),
                retention_policy: "session_scoped".to_string(),
            },
            kind: kind.to_string(),
            display_name: display_name.to_string(),
            usage: Some(usage.to_string()),
            status: "ready".to_string(),
            material_id: Some("m1".to_string()),
            preview_url: Some("/api/assets/preview".to_string()),
            visual_reference: Some(StorybookVisualReferenceSummary {
                id: Uuid::new_v4(),
                status: "confirmed".to_string(),
                generation_job_id: Some(Uuid::new_v4()),
                preview_url: Some("/api/generated/reference.png".to_string()),
                failure_reason: None,
                confirmed_at: None,
                confirmed_by: Some(Uuid::new_v4()),
            }),
            revoked_at: None,
            revoked_by: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }
}
