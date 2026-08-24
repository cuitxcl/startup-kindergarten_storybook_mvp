use axum::http::HeaderMap;
#[cfg(not(feature = "db"))]
use chrono::Utc;
use loco_rs::app::AppContext;
#[cfg(feature = "db")]
use serde_json::json;
use uuid::Uuid;

#[cfg(not(feature = "db"))]
use crate::application::delivery::{
    find_storybook, mock_share_export, share_link_active, shared_state,
};
#[cfg(feature = "db")]
use crate::workers::export::enqueue_export_job;
use crate::{
    application::delivery::{
        delivery_error, delivery_privacy_risk_labels, ensure_storybook_deliverable_for_operation,
        ensure_storybook_evidence_ready, log_delivery_privacy_blocked, read_export_job_file,
        with_share_export_download_url,
    },
    domains::common,
    error::ApiError,
    models::{CreateShareLinkRequest, ExportJob, ListQuery, PaginationMeta, ShareLink, Storybook},
};

#[cfg(not(feature = "db"))]
use crate::application::delivery::ensure_storybook_deliverable;

pub async fn create_share_link(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    payload: CreateShareLinkRequest,
) -> Result<ShareLink, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
        let actor_id = common::actor_user_id(headers)?;
        let book = crate::repositories::storybooks::find(&ctx.db, workspace_id, storybook_id)
            .await
            .map_err(common::db_error)?;
        ensure_storybook_deliverable_for_operation(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            &book,
            "share_link",
        )
        .await?;
        ensure_storybook_evidence_ready(&ctx.db, &book).await?;
        let link = match crate::repositories::delivery::create_share_link(
            &ctx.db,
            workspace_id,
            storybook_id,
            payload.expires_at,
        )
        .await
        {
            Ok(link) => link,
            Err(sea_orm::DbErr::Custom(message))
                if delivery_privacy_risk_labels(&message).is_some() =>
            {
                let risks = delivery_privacy_risk_labels(&message).unwrap_or_default();
                log_delivery_privacy_blocked(
                    &ctx.db,
                    Some(workspace_id),
                    Some(actor_id),
                    storybook_id,
                    "share_link",
                    risks,
                )
                .await?;
                return Err(delivery_error(sea_orm::DbErr::Custom(message)));
            }
            Err(err) => return Err(delivery_error(err)),
        };
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "storybook.share_link_created",
            "share_link",
            Some(link.id),
            json!({
                "storybook_id": storybook_id,
                "status": link.status,
                "expires_at": link.expires_at,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(link);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        let book = find_storybook(&state, workspace_id, storybook_id)?;
        ensure_storybook_deliverable(&book)?;
        let token = Uuid::new_v4().simple().to_string();
        let link = ShareLink {
            id: Uuid::new_v4(),
            storybook_id,
            token: token.clone(),
            url: format!("/link/share/{token}"),
            status: if payload
                .expires_at
                .is_some_and(|expires_at| expires_at <= Utc::now())
            {
                "expired".to_string()
            } else {
                "active".to_string()
            },
            access_count: 0,
            last_accessed_at: None,
            expires_at: payload.expires_at.map(|value| value.to_rfc3339()),
        };
        state
            .write()
            .expect("state lock poisoned")
            .share_links
            .insert(token, link.clone());
        Ok(link)
    }
}

pub async fn list_share_links(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    query: ListQuery,
) -> Result<(Vec<ShareLink>, PaginationMeta), ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0);
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
        return crate::repositories::delivery::list_share_links(
            &ctx.db,
            workspace_id,
            storybook_id,
            limit,
            offset,
        )
        .await
        .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        find_storybook(&state, workspace_id, storybook_id)?;
        let state = state.read().expect("state lock poisoned");
        let links = state
            .share_links
            .values()
            .filter(|item| item.storybook_id == storybook_id && share_link_active(item))
            .cloned()
            .collect();
        Ok(common::paginate_vec(links, Some(limit), Some(offset)))
    }
}

pub async fn revoke_share_link(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    share_link_id: Uuid,
) -> Result<ShareLink, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
        let link = crate::repositories::delivery::revoke_share_link(
            &ctx.db,
            workspace_id,
            storybook_id,
            share_link_id,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "storybook.share_link_revoked",
            "share_link",
            Some(link.id),
            json!({
                "storybook_id": storybook_id,
                "status": link.status,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(link);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        find_storybook(&state, workspace_id, storybook_id)?;
        let mut state = state.write().expect("state lock poisoned");
        let link = state
            .share_links
            .values_mut()
            .find(|item| item.id == share_link_id && item.storybook_id == storybook_id)
            .ok_or_else(|| ApiError::not_found("share_link"))?;
        if !share_link_active(link) {
            return Err(ApiError::not_found("share_link"));
        }
        link.status = "revoked".to_string();
        Ok(link.clone())
    }
}

pub async fn get_public_share(ctx: &AppContext, token: String) -> Result<Storybook, ApiError> {
    #[cfg(feature = "db")]
    {
        let book = crate::repositories::delivery::storybook_by_share_token(&ctx.db, &token)
            .await
            .map_err(common::db_error)?;
        ensure_storybook_deliverable_for_operation(
            &ctx.db,
            Some(book.workspace_id),
            None,
            &book,
            "public_share",
        )
        .await?;
        crate::repositories::delivery::record_share_link_access(&ctx.db, &token)
            .await
            .map_err(common::db_error)?;
        return Ok(book);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        let state = state.read().expect("state lock poisoned");
        let link = state
            .share_links
            .get(&token)
            .ok_or_else(|| ApiError::not_found("share_link"))?;
        if !share_link_active(link) {
            return Err(ApiError::not_found("share_link"));
        }
        state
            .storybooks
            .iter()
            .find(|item| item.id == link.storybook_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("storybook"))
    }
}

pub async fn create_public_export(ctx: &AppContext, token: String) -> Result<ExportJob, ApiError> {
    #[cfg(feature = "db")]
    {
        let shared_storybook =
            crate::repositories::delivery::storybook_by_share_token(&ctx.db, &token)
                .await
                .map_err(delivery_error)?;
        ensure_storybook_deliverable_for_operation(
            &ctx.db,
            Some(shared_storybook.workspace_id),
            None,
            &shared_storybook,
            "public_export",
        )
        .await?;
        ensure_storybook_evidence_ready(&ctx.db, &shared_storybook).await?;
        let job = match crate::repositories::delivery::create_export_by_share_token(&ctx.db, &token)
            .await
        {
            Ok(job) => job,
            Err(sea_orm::DbErr::Custom(message))
                if delivery_privacy_risk_labels(&message).is_some() =>
            {
                let risks = delivery_privacy_risk_labels(&message).unwrap_or_default();
                log_delivery_privacy_blocked(
                    &ctx.db,
                    Some(shared_storybook.workspace_id),
                    None,
                    shared_storybook.id,
                    "public_export",
                    risks,
                )
                .await?;
                return Err(delivery_error(sea_orm::DbErr::Custom(message)));
            }
            Err(err) => return Err(delivery_error(err)),
        };
        enqueue_export_job(ctx, job.id)
            .await
            .map_err(|err| ApiError::state_conflict(format!("公开导出任务入队失败：{err}")))?;
        crate::repositories::audit::log(
            &ctx.db,
            None,
            None,
            "share_link.public_export_created",
            "export_job",
            Some(job.id),
            json!({
                "storybook_id": job.storybook_id,
                "share_token_suffix": token.chars().rev().take(6).collect::<String>(),
                "status": job.status,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(with_share_export_download_url(job, &token));
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        let state = state.read().expect("state lock poisoned");
        let link = state
            .share_links
            .get(&token)
            .ok_or_else(|| ApiError::not_found("share_link"))?;
        if !share_link_active(link) {
            return Err(ApiError::not_found("share_link"));
        }
        let export_id = Uuid::new_v4();
        Ok(mock_share_export(&token, link.storybook_id, export_id))
    }
}

pub async fn get_public_export(
    ctx: &AppContext,
    token: String,
    export_id: Uuid,
) -> Result<ExportJob, ApiError> {
    #[cfg(feature = "db")]
    {
        let job =
            crate::repositories::delivery::find_export_by_share_token(&ctx.db, &token, export_id)
                .await
                .map_err(common::db_error)?;
        return Ok(with_share_export_download_url(job, &token));
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        let state = state.read().expect("state lock poisoned");
        let link = state
            .share_links
            .get(&token)
            .ok_or_else(|| ApiError::not_found("share_link"))?;
        if !share_link_active(link) {
            return Err(ApiError::not_found("share_link"));
        }
        Ok(mock_share_export(&token, link.storybook_id, export_id))
    }
}

pub async fn public_share_export_file(
    ctx: &AppContext,
    token: String,
    export_id: Uuid,
) -> Result<(String, Vec<u8>), ApiError> {
    #[cfg(feature = "db")]
    {
        let job =
            crate::repositories::delivery::find_export_by_share_token(&ctx.db, &token, export_id)
                .await
                .map_err(common::db_error)?;
        return read_export_job_file(&job);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        let state = state.read().expect("state lock poisoned");
        let link = state
            .share_links
            .get(&token)
            .ok_or_else(|| ApiError::not_found("share_link"))?;
        if link.status != "active" {
            return Err(ApiError::not_found("share_link"));
        }
        read_export_job_file(&mock_share_export(&token, link.storybook_id, export_id))
    }
}
