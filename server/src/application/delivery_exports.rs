use axum::http::HeaderMap;
#[cfg(not(feature = "db"))]
use loco_rs::app::AppContext;
#[cfg(feature = "db")]
use loco_rs::app::AppContext;
#[cfg(feature = "db")]
use serde_json::json;
use uuid::Uuid;

#[cfg(feature = "db")]
use crate::workers::export::enqueue_export_job;

#[cfg(not(feature = "db"))]
use crate::application::delivery::{find_storybook, mock_workspace_export, shared_state};
use crate::{
    application::delivery::{
        delivery_error, delivery_privacy_risk_labels, ensure_storybook_deliverable_for_operation,
        ensure_storybook_evidence_ready, log_delivery_privacy_blocked, read_export_job_file,
        valid_export_file_name, with_workspace_export_download_url,
    },
    domains::common,
    error::ApiError,
    models::{ExportJob, ListQuery, PaginationMeta},
};

#[cfg(not(feature = "db"))]
use crate::application::delivery::ensure_storybook_deliverable;

pub async fn create_export(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<ExportJob, ApiError> {
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
            "export",
        )
        .await?;
        ensure_storybook_evidence_ready(&ctx.db, &book).await?;
        let job = match crate::repositories::delivery::create_export(
            &ctx.db,
            workspace_id,
            storybook_id,
            actor_id,
        )
        .await
        {
            Ok(job) => job,
            Err(sea_orm::DbErr::Custom(message))
                if delivery_privacy_risk_labels(&message).is_some() =>
            {
                let risks = delivery_privacy_risk_labels(&message).unwrap_or_default();
                log_delivery_privacy_blocked(
                    &ctx.db,
                    Some(workspace_id),
                    Some(actor_id),
                    storybook_id,
                    "export",
                    risks,
                )
                .await?;
                return Err(delivery_error(sea_orm::DbErr::Custom(message)));
            }
            Err(err) => return Err(delivery_error(err)),
        };
        enqueue_export_job(ctx, job.id)
            .await
            .map_err(|err| ApiError::state_conflict(format!("导出任务入队失败：{err}")))?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "storybook.export_created",
            "export_job",
            Some(job.id),
            json!({
                "storybook_id": storybook_id,
                "status": job.status,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(with_workspace_export_download_url(
            job,
            workspace_id,
            storybook_id,
        ));
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        let book = find_storybook(&state, workspace_id, storybook_id)?;
        ensure_storybook_deliverable(&book)?;
        let export_id = Uuid::new_v4();
        Ok(mock_workspace_export(workspace_id, storybook_id, export_id))
    }
}

pub async fn list_exports(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    query: ListQuery,
) -> Result<(Vec<ExportJob>, PaginationMeta), ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0);
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
        let (jobs, meta) = crate::repositories::delivery::list_exports(
            &ctx.db,
            workspace_id,
            storybook_id,
            limit,
            offset,
        )
        .await
        .map_err(common::db_error)?;
        let jobs = jobs
            .into_iter()
            .map(|job| with_workspace_export_download_url(job, workspace_id, storybook_id))
            .collect::<Vec<_>>();
        return Ok((jobs, meta));
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        find_storybook(&state, workspace_id, storybook_id)?;
        Ok(common::paginate_vec(Vec::new(), Some(limit), Some(offset)))
    }
}

pub async fn get_export(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    export_id: Uuid,
) -> Result<ExportJob, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
        let job = crate::repositories::delivery::find_export(
            &ctx.db,
            workspace_id,
            storybook_id,
            export_id,
        )
        .await
        .map_err(common::db_error)?;
        return Ok(with_workspace_export_download_url(
            job,
            workspace_id,
            storybook_id,
        ));
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        find_storybook(&state, workspace_id, storybook_id)?;
        Ok(mock_workspace_export(workspace_id, storybook_id, export_id))
    }
}

pub async fn workspace_export_file(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    export_id: Uuid,
) -> Result<(String, Vec<u8>), ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
        let job = crate::repositories::delivery::find_export(
            &ctx.db,
            workspace_id,
            storybook_id,
            export_id,
        )
        .await
        .map_err(common::db_error)?;
        return read_export_job_file(&job);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        find_storybook(&state, workspace_id, storybook_id)?;
        read_export_job_file(&mock_workspace_export(
            workspace_id,
            storybook_id,
            export_id,
        ))
    }
}

pub fn public_export_file(file_name: &str) -> Result<(String, Vec<u8>), ApiError> {
    let safe_name = file_name.trim();
    if !valid_export_file_name(safe_name) {
        return Err(ApiError::not_found("export"));
    }
    Err(ApiError::not_found("export"))
}
