use axum::http::HeaderMap;
#[cfg(not(feature = "db"))]
use chrono::Utc;
use loco_rs::app::AppContext;
use serde::Deserialize;
use uuid::Uuid;

#[cfg(feature = "db")]
use serde_json::json;

use crate::{domains::common, error::ApiError, models::GenerationJob};

#[derive(Debug, Deserialize)]
pub struct RecoverGenerationJobsRequest {
    #[serde(default)]
    pub age_minutes: Option<i64>,
    #[serde(default)]
    pub limit: Option<usize>,
}
pub async fn retry_job(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    job_id: Uuid,
) -> Result<GenerationJob, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let job =
            crate::repositories::generation::retry_generation_job(&ctx.db, workspace_id, job_id)
                .await
                .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "generation_job.retried",
            "generation_job",
            Some(job.id),
            json!({
                "storybook_id": job.storybook_id,
                "job_type": job.job_type,
                "status": job.status,
                "attempt_count": job.attempt_count,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(job);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_editor(&state, headers, workspace_id)?;
        Ok(mock_terminal_job(
            workspace_id,
            job_id,
            "succeeded",
            "生成任务已重试并完成，当前为 mock 结果",
        ))
    }
}

pub async fn cancel_job(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    job_id: Uuid,
) -> Result<GenerationJob, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let job =
            crate::repositories::generation::cancel_generation_job(&ctx.db, workspace_id, job_id)
                .await
                .map_err(generation_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "generation_job.canceled",
            "generation_job",
            Some(job.id),
            json!({
                "storybook_id": job.storybook_id,
                "job_type": job.job_type,
                "status": job.status,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(job);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_editor(&state, headers, workspace_id)?;
        Ok(mock_terminal_job(
            workspace_id,
            job_id,
            "canceled",
            "生成任务已取消",
        ))
    }
}

pub async fn recover_jobs(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    payload: RecoverGenerationJobsRequest,
) -> Result<serde_json::Value, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_admin_db(ctx, headers, workspace_id).await?;
        let processed = crate::repositories::generation::process_generation_backlog_for_workspace(
            &ctx.db,
            workspace_id,
            payload.age_minutes.unwrap_or(15),
            payload.limit.unwrap_or(10),
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "generation_job.recovered",
            "generation_job",
            None,
            json!({
                "processed": processed,
                "age_minutes": payload.age_minutes.unwrap_or(15),
                "limit": payload.limit.unwrap_or(10),
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(serde_json::json!({
            "status": "ok",
            "processed": processed,
            "message": "生成队列已恢复"
        }));
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_admin(&state, headers, workspace_id)?;
        Ok(serde_json::json!({
            "status": "ok",
            "processed": 0,
            "message": "当前为 mock 恢复结果"
        }))
    }
}
#[cfg(feature = "db")]
fn generation_error(err: sea_orm::DbErr) -> ApiError {
    match err {
        sea_orm::DbErr::Custom(message) if message == "generation_job_not_cancelable" => {
            ApiError::state_conflict("只有排队中或失败待重试的生成任务可以取消")
        }
        other => common::db_error(other),
    }
}

#[cfg(not(feature = "db"))]
fn shared_state(ctx: &AppContext) -> Result<crate::state::SharedState, ApiError> {
    ctx.shared_store
        .get::<crate::state::SharedState>()
        .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))
}

#[cfg(not(feature = "db"))]
fn mock_terminal_job(
    workspace_id: Uuid,
    job_id: Uuid,
    status: &str,
    message: &str,
) -> GenerationJob {
    GenerationJob {
        id: job_id,
        workspace_id,
        storybook_id: None,
        job_type: "storybook_plan".to_string(),
        status: status.to_string(),
        input_json: serde_json::json!({}),
        output_json: Some(serde_json::json!({
            "schema_version": "generation.mock.v1",
            "provider": "mock",
            "mode": "storybook_plan",
            "message": message
        })),
        attempt_count: if status == "canceled" { 0 } else { 1 },
        last_error: None,
        next_run_at: None,
        locked_by: None,
        locked_at: None,
        created_at: Utc::now(),
        finished_at: Some(Utc::now()),
    }
}
