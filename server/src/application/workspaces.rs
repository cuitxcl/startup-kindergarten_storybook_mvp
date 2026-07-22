use axum::http::HeaderMap;
use loco_rs::app::AppContext;
use uuid::Uuid;

use crate::{
    domains::common,
    error::ApiError,
    models::{DashboardResponse, StorybookListQuery, Workspace, WorkspaceRole},
    services::generation_provider::{ConfiguredGenerationProvider, GenerationProviderSummary},
};

pub async fn list(ctx: &AppContext, headers: &HeaderMap) -> Result<Vec<Workspace>, ApiError> {
    #[cfg(feature = "db")]
    {
        let user_id = common::dev_token_user_id(headers)?;
        return crate::repositories::auth::list_current_workspaces(&ctx.db, user_id)
            .await
            .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = ctx
            .shared_store
            .get::<crate::state::SharedState>()
            .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))?;
        common::require_login(&state, headers)?;
        Ok(state
            .read()
            .expect("state lock poisoned")
            .workspaces
            .clone())
    }
}

pub async fn dashboard(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
) -> Result<DashboardResponse, ApiError> {
    #[cfg(feature = "db")]
    {
        let workspace = common::require_workspace_db(ctx, headers, workspace_id).await?;
        let (storybooks, _) = crate::repositories::storybooks::list_by_workspace(
            &ctx.db,
            workspace_id,
            StorybookListQuery {
                storybook_type: None,
                status: None,
                target_child_id: None,
                q: None,
                limit: Some(12),
                offset: Some(0),
            },
        )
        .await
        .map_err(common::db_error)?;
        let scope = common::child_classroom_scope(ctx, headers, workspace_id, &workspace).await?;
        let children = match scope {
            Some(classrooms) => crate::repositories::children::list_by_workspace_for_classrooms(
                &ctx.db,
                workspace_id,
                &classrooms,
            )
            .await
            .map_err(common::db_error)?,
            None => crate::repositories::children::list_by_workspace(&ctx.db, workspace_id)
                .await
                .map_err(common::db_error)?,
        };
        let submissions = if workspace.role == WorkspaceRole::SchoolAdmin {
            crate::repositories::market::list_submissions(&ctx.db, workspace_id)
                .await
                .map_err(common::db_error)?
        } else {
            Vec::new()
        };
        return Ok(DashboardResponse {
            workspace,
            storybooks,
            children,
            submissions,
        });
    }

    #[cfg(not(feature = "db"))]
    {
        let state = ctx
            .shared_store
            .get::<crate::state::SharedState>()
            .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))?;
        let workspace = common::require_workspace(&state, headers, workspace_id)?;
        let state = state.read().expect("state lock poisoned");
        let storybooks = state
            .storybooks
            .iter()
            .filter(|item| item.workspace_id == workspace_id)
            .cloned()
            .collect::<Vec<_>>();
        let children = state
            .children
            .iter()
            .filter(|item| item.workspace_id == workspace_id)
            .cloned()
            .collect::<Vec<_>>();
        let submissions = state
            .submissions
            .iter()
            .filter(|item| item.workspace_id == workspace_id)
            .cloned()
            .collect::<Vec<_>>();
        Ok(DashboardResponse {
            workspace,
            storybooks,
            children,
            submissions,
        })
    }
}

pub async fn generation_provider(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
) -> Result<GenerationProviderSummary, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
    }

    #[cfg(not(feature = "db"))]
    {
        let state = ctx
            .shared_store
            .get::<crate::state::SharedState>()
            .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))?;
        common::require_workspace(&state, headers, workspace_id)?;
    }

    let provider = ConfiguredGenerationProvider::from_env();
    Ok(provider.summary())
}
