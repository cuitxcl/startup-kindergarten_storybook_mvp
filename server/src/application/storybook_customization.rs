use axum::http::HeaderMap;
use loco_rs::app::AppContext;
use std::collections::HashSet;
use uuid::Uuid;

#[cfg(feature = "db")]
use serde_json::json;

#[cfg(not(feature = "db"))]
use crate::models::{StorybookStatus, Visibility};
use crate::{
    domains::common,
    error::ApiError,
    models::{
        DeriveCustomBatchRequest, DeriveCustomBatchResponse, DeriveCustomRequest, Storybook,
        StorybookType,
    },
};

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
        if let Some(classrooms) =
            common::child_classroom_scope(ctx, headers, workspace_id, &workspace).await?
        {
            crate::repositories::children::find_for_classrooms(
                &ctx.db,
                workspace_id,
                payload.child_id,
                &classrooms,
            )
            .await
            .map_err(common::db_error)?;
        }
        let child_id = payload.child_id;
        let intensity = payload.intensity.clone();
        let actor_id = common::actor_user_id(headers)?;
        let book = crate::repositories::storybooks::derive_custom(
            &ctx.db,
            workspace_id,
            storybook_id,
            actor_id,
            payload,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "storybook.custom_derived",
            "storybook",
            Some(book.id),
            json!({
                "source_storybook_id": storybook_id,
                "target_child_id": child_id,
                "intensity": intensity,
                "title": book.title,
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

pub async fn derive_custom_batch(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    payload: DeriveCustomBatchRequest,
) -> Result<DeriveCustomBatchResponse, ApiError> {
    validate_custom_batch_payload(&payload)?;

    #[cfg(feature = "db")]
    {
        let workspace = common::require_editor_db(ctx, headers, workspace_id).await?;
        let source = crate::repositories::storybooks::find(&ctx.db, workspace_id, storybook_id)
            .await
            .map_err(common::db_error)?;
        if source.storybook_type != StorybookType::Plain {
            return Err(ApiError::state_conflict("只有普通绘本可以派生定制绘本"));
        }

        if let Some(classrooms) =
            common::child_classroom_scope(ctx, headers, workspace_id, &workspace).await?
        {
            for child_id in &payload.child_ids {
                crate::repositories::children::find_for_classrooms(
                    &ctx.db,
                    workspace_id,
                    *child_id,
                    &classrooms,
                )
                .await
                .map_err(common::db_error)?;
            }
        } else {
            for child_id in &payload.child_ids {
                crate::repositories::children::find(&ctx.db, workspace_id, *child_id)
                    .await
                    .map_err(common::db_error)?;
            }
        }

        let mut storybooks = Vec::with_capacity(payload.child_ids.len());
        let actor_id = common::actor_user_id(headers)?;
        for child_id in &payload.child_ids {
            let book = crate::repositories::storybooks::derive_custom(
                &ctx.db,
                workspace_id,
                storybook_id,
                actor_id,
                DeriveCustomRequest {
                    child_id: *child_id,
                    intensity: payload.intensity.clone(),
                    customization_plan: payload.customization_plan.clone(),
                },
            )
            .await
            .map_err(common::db_error)?;
            storybooks.push(book);
        }

        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "storybook.custom_batch_derived",
            "storybook",
            Some(storybook_id),
            json!({
                "source_storybook_id": storybook_id,
                "target_child_ids": &payload.child_ids,
                "intensity": &payload.intensity,
                "created_count": storybooks.len(),
            }),
        )
        .await
        .map_err(common::db_error)?;

        return Ok(DeriveCustomBatchResponse {
            source_storybook_id: storybook_id,
            requested_count: payload.child_ids.len(),
            created_count: storybooks.len(),
            storybooks,
        });
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_editor(&state, headers, workspace_id)?;
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

        let mut storybooks = Vec::with_capacity(payload.child_ids.len());
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
            storybooks.push(book);
        }

        Ok(DeriveCustomBatchResponse {
            source_storybook_id: storybook_id,
            requested_count: payload.child_ids.len(),
            created_count: storybooks.len(),
            storybooks,
        })
    }
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
    Ok(())
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
    book.title = format!("{child_nickname}的定制故事");
    book.teacher_review_status = "pending".to_string();
    book.teacher_reviewed_by = None;
    book.teacher_reviewed_at = None;
    book.updated_at = "刚刚".to_string();
    book
}
