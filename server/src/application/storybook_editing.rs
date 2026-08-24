use axum::http::HeaderMap;
use loco_rs::app::AppContext;
use uuid::Uuid;

#[cfg(feature = "db")]
use serde_json::json;

use crate::{
    application::storybook_inputs::{
        clean_optional, clean_page_review_status, clean_page_status, clean_reference_status,
        page_status_name,
    },
    domains::common,
    error::ApiError,
    models::{StorybookPage, StorybookRole, UpdatePageRequest, UpdateRoleRequest},
};

pub async fn update_page(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    page_id: Uuid,
    payload: UpdatePageRequest,
) -> Result<StorybookPage, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let actor_id = common::actor_user_id(headers)?;
        let payload = UpdatePageRequest {
            title: clean_optional(payload.title, "title")?,
            body: clean_optional(payload.body, "body")?,
            illustration_prompt: clean_optional(
                payload.illustration_prompt,
                "illustration_prompt",
            )?,
            status: clean_page_status(payload.status)?,
            review_status: clean_page_review_status(payload.review_status)?,
        };
        let page = crate::repositories::storybooks::update_page(
            &ctx.db,
            workspace_id,
            storybook_id,
            page_id,
            payload,
            actor_id,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "storybook.page_updated",
            "storybook_page",
            Some(page.id),
            json!({
                "storybook_id": storybook_id,
                "page_number": page.page_number,
                "status": page_status_name(&page.status),
                "review_status": page.review_status,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(page);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_editor(&state, headers, workspace_id)?;
        let mut state = state.write().expect("state lock poisoned");
        let book = state
            .storybooks
            .iter_mut()
            .find(|item| item.workspace_id == workspace_id && item.id == storybook_id)
            .ok_or_else(|| ApiError::not_found("storybook"))?;
        let page = book
            .pages
            .iter_mut()
            .find(|item| item.id == page_id)
            .ok_or_else(|| ApiError::not_found("page"))?;
        let mut content_changed = false;
        if let Some(value) = payload.title {
            page.title = common::required(value, "title")?;
            content_changed = true;
        }
        if let Some(value) = payload.body {
            page.body = common::required(value, "body")?;
            content_changed = true;
        }
        if let Some(value) = payload.illustration_prompt {
            page.illustration_prompt = common::required(value, "illustration_prompt")?;
            content_changed = true;
        }
        if let Some(value) = payload.status {
            page.status = value;
            content_changed = true;
        }
        if let Some(value) = payload.review_status {
            page.review_status = value;
            page.reviewed_by = None;
            page.reviewed_at = Some("刚刚".to_string());
        } else if content_changed {
            page.review_status = "unchecked".to_string();
            page.reviewed_by = None;
            page.reviewed_at = None;
        }
        let page = page.clone();
        book.teacher_review_status = "pending".to_string();
        book.teacher_reviewed_by = None;
        book.teacher_reviewed_at = None;
        book.updated_at = "刚刚".to_string();
        Ok(page)
    }
}

pub async fn update_role(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    role_id: Uuid,
    payload: UpdateRoleRequest,
) -> Result<StorybookRole, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let payload = UpdateRoleRequest {
            name: clean_optional(payload.name, "name")?,
            role_type: clean_optional(payload.role_type, "role_type")?,
            appearance: clean_optional(payload.appearance, "appearance")?,
            story_function: clean_optional(payload.story_function, "story_function")?,
            needs_consistency: payload.needs_consistency,
            reference_image_url: clean_optional(
                payload.reference_image_url,
                "reference_image_url",
            )?,
            reference_image_prompt: clean_optional(
                payload.reference_image_prompt,
                "reference_image_prompt",
            )?,
            reference_status: clean_reference_status(payload.reference_status)?,
        };
        let role = crate::repositories::storybooks::update_role(
            &ctx.db,
            workspace_id,
            storybook_id,
            role_id,
            payload,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "storybook.role_updated",
            "storybook_role",
            Some(role.id),
            json!({
                "storybook_id": storybook_id,
                "name": role.name,
                "role_type": role.role_type,
                "needs_consistency": role.needs_consistency,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(role);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_editor(&state, headers, workspace_id)?;
        let mut state = state.write().expect("state lock poisoned");
        let book = state
            .storybooks
            .iter_mut()
            .find(|item| item.workspace_id == workspace_id && item.id == storybook_id)
            .ok_or_else(|| ApiError::not_found("storybook"))?;
        let role = book
            .roles
            .iter_mut()
            .find(|item| item.id == role_id)
            .ok_or_else(|| ApiError::not_found("role"))?;
        if let Some(value) = payload.name {
            role.name = common::required(value, "name")?;
        }
        if let Some(value) = payload.role_type {
            role.role_type = common::required(value, "role_type")?;
        }
        if let Some(value) = payload.appearance {
            role.appearance = common::required(value, "appearance")?;
        }
        if let Some(value) = payload.story_function {
            role.story_function = common::required(value, "story_function")?;
        }
        if let Some(value) = payload.needs_consistency {
            role.needs_consistency = value;
        }
        let role = role.clone();
        book.teacher_review_status = "pending".to_string();
        book.teacher_reviewed_by = None;
        book.teacher_reviewed_at = None;
        book.updated_at = "刚刚".to_string();
        Ok(role)
    }
}

#[cfg(not(feature = "db"))]
fn shared_state(ctx: &AppContext) -> Result<crate::state::SharedState, ApiError> {
    ctx.shared_store
        .get::<crate::state::SharedState>()
        .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))
}
