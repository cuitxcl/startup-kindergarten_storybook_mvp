use axum::http::HeaderMap;
#[cfg(not(feature = "db"))]
use chrono::Utc;
use loco_rs::app::AppContext;
#[cfg(feature = "db")]
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use uuid::Uuid;

use crate::{
    domains::common,
    error::ApiError,
    models::{GenerationJob, WorkspaceRole},
};

pub async fn generation_image_file(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    job_id: Uuid,
) -> Result<(String, Vec<u8>), ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
        let job = crate::repositories::generation::find_job(&ctx.db, workspace_id, job_id)
            .await
            .map_err(common::db_error)?;
        if job.job_type == "storybook_visual_reference" {
            ensure_visual_reference_image_viewer(ctx, headers, workspace_id, job_id).await?;
        }
        return read_generation_image_file(&job);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        let job = GenerationJob {
            id: job_id,
            workspace_id,
            storybook_id: None,
            created_by: None,
            job_type: "storybook_page_image".to_string(),
            status: "succeeded".to_string(),
            input_json: serde_json::json!({}),
            output_json: Some(serde_json::json!({
                "image": {
                    "image_url": format!("/generated-images/mock-{job_id}.png")
                }
            })),
            attempt_count: 1,
            last_error: None,
            next_run_at: None,
            locked_by: None,
            locked_at: None,
            created_at: Utc::now(),
            finished_at: Some(Utc::now()),
        };
        read_generation_image_file(&job)
    }
}

#[cfg(feature = "db")]
async fn ensure_visual_reference_image_viewer(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    job_id: Uuid,
) -> Result<(), ApiError> {
    let workspace = common::require_workspace_db(ctx, headers, workspace_id).await?;
    let actor_id = common::actor_user_id(headers)?;
    let row = ctx
        .db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select s.created_by
            from storybook_visual_references v
            join storybook_asset_references r
              on r.id = v.asset_reference_id and r.workspace_id = v.workspace_id
            join storybook_creation_sessions s
              on s.id = r.creation_session_id and s.workspace_id = r.workspace_id
            where v.workspace_id = $1
              and v.generation_job_id = $2
              and v.is_active = true
            limit 1
            "#,
            [workspace_id.into(), job_id.into()],
        ))
        .await
        .map_err(common::db_error)?
        .ok_or_else(|| ApiError::not_found("generated_image"))?;
    let created_by: Uuid = row.try_get("", "created_by").map_err(common::db_error)?;
    if created_by == actor_id || matches!(workspace.role, WorkspaceRole::SchoolAdmin) {
        Ok(())
    } else {
        Err(ApiError::forbidden("只能查看自己创建的专属绘本参考图"))
    }
}

pub fn public_generated_image_file(file_name: &str) -> Result<(String, Vec<u8>), ApiError> {
    let safe_name = file_name.trim();
    if !valid_generated_image_file_name(safe_name) {
        return Err(ApiError::not_found("generated_image"));
    }
    Err(ApiError::not_found("generated_image"))
}

fn read_generation_image_file(job: &GenerationJob) -> Result<(String, Vec<u8>), ApiError> {
    if job.status != "succeeded" || !is_downloadable_image_job(&job.job_type) {
        return Err(ApiError::not_found("generated_image"));
    }
    let Some(file_name) = generation_image_file_name(job) else {
        return Err(ApiError::not_found("generated_image"));
    };
    let bytes = crate::services::storage::read_generated_image(&file_name)
        .map_err(|_| ApiError::not_found("generated_image"))?;
    Ok((file_name, bytes))
}

pub fn with_generation_image_download_url(
    mut job: GenerationJob,
    workspace_id: Uuid,
) -> GenerationJob {
    if job.status == "succeeded"
        && is_downloadable_image_job(&job.job_type)
        && let Some(output) = job.output_json.as_mut()
        && let Some(image) = output
            .get_mut("image")
            .and_then(|value| value.as_object_mut())
        && image.get("image_url").is_some()
    {
        image.insert(
            "image_url".to_string(),
            serde_json::json!(generation_image_download_url(workspace_id, job.id)),
        );
    }
    job
}

fn generation_image_download_url(workspace_id: Uuid, job_id: Uuid) -> String {
    format!("/api/workspaces/{workspace_id}/generation-jobs/{job_id}/image")
}

fn is_downloadable_image_job(job_type: &str) -> bool {
    matches!(
        job_type,
        "storybook_cover_image"
            | "storybook_page_image"
            | "storybook_role_reference_image"
            | "storybook_visual_reference"
    )
}

fn generation_image_file_name(job: &GenerationJob) -> Option<String> {
    let url = job
        .output_json
        .as_ref()?
        .get("image")?
        .get("image_url")?
        .as_str()?;
    let file_name = url.rsplit('/').next()?;
    valid_generated_image_file_name(file_name).then(|| file_name.to_string())
}

fn valid_generated_image_file_name(file_name: &str) -> bool {
    let Some(name) = file_name.strip_suffix(".png") else {
        return false;
    };
    let Some((provider, id)) = name.split_once('-') else {
        return false;
    };
    matches!(provider, "mock" | "seedream") && Uuid::parse_str(id).is_ok()
}

#[cfg(not(feature = "db"))]
fn shared_state(ctx: &AppContext) -> Result<crate::state::SharedState, ApiError> {
    ctx.shared_store
        .get::<crate::state::SharedState>()
        .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_image_file_name_requires_provider_and_uuid_png() {
        let id = Uuid::new_v4();
        assert!(valid_generated_image_file_name(&format!("mock-{id}.png")));
        assert!(valid_generated_image_file_name(&format!(
            "seedream-{id}.png"
        )));
        assert!(!valid_generated_image_file_name(&format!("other-{id}.png")));
        assert!(!valid_generated_image_file_name("mock-page-1.png"));
        assert!(!valid_generated_image_file_name("../mock-secret.png"));
        assert!(!valid_generated_image_file_name(&format!("mock-{id}.jpg")));
    }
}
