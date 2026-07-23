use axum::http::HeaderMap;
#[cfg(not(feature = "db"))]
use chrono::Utc;
use loco_rs::app::AppContext;
use serde::Deserialize;
use uuid::Uuid;

#[cfg(feature = "db")]
use serde_json::json;

#[cfg(feature = "db")]
use crate::workers::generation::{enqueue_generation_job, enqueue_generation_page_image_job};

use crate::{
    domains::common,
    error::ApiError,
    models::{
        CreateGenerationJobRequest, CreateImageTaskRequest, GenerationJob, GenerationJobListQuery,
        PaginationMeta, WorkspaceRole,
    },
};

#[derive(Debug, Deserialize)]
pub struct RecoverGenerationJobsRequest {
    #[serde(default)]
    pub age_minutes: Option<i64>,
    #[serde(default)]
    pub limit: Option<usize>,
}

pub async fn create_page_image_task(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    page_id: Uuid,
    payload: CreateImageTaskRequest,
) -> Result<GenerationJob, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let queued = crate::repositories::generation::create_page_image_job_record(
            &ctx.db,
            workspace_id,
            storybook_id,
            page_id,
            payload,
        )
        .await
        .map_err(common::db_error)?;
        enqueue_generation_page_image_job(ctx, workspace_id, queued.id)
            .await
            .map_err(|err| ApiError::state_conflict(format!("插图任务入队失败：{err}")))?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "generation_job.created",
            "generation_job",
            Some(queued.id),
            json!({
                "storybook_id": storybook_id,
                "page_id": page_id,
                "job_type": queued.job_type,
                "status": queued.status,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(queued);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_editor(&state, headers, workspace_id)?;
        find_storybook(&state, workspace_id, storybook_id)?;
        let job_id = Uuid::new_v4();
        let prompt = payload.prompt.unwrap_or_default();
        let image_mode = payload.image_mode.unwrap_or_else(|| {
            if payload.reference_image_urls.is_empty() {
                "text_to_image".to_string()
            } else {
                "reference_image".to_string()
            }
        });
        let reference_role_ids = payload.reference_role_ids;
        let reference_images = payload
            .reference_image_urls
            .into_iter()
            .map(|url| {
                serde_json::json!({
                    "url": url,
                    "source": "direct",
                    "role_id": null,
                    "label": null
                })
            })
            .collect::<Vec<_>>();
        let edit_instruction = payload.edit_instruction;
        let strength = payload.strength;
        let output_json = serde_json::json!({
            "image": {
                "page_id": page_id,
                "image_url": format!("/generated-images/mock-{job_id}.png"),
                "alt_text": "幼儿园教室里的温暖共读场景",
                "prompt": prompt,
                "image_mode": image_mode,
                "reference_images": reference_images,
                "edit_instruction": edit_instruction,
                "strength": strength,
                "style_notes": ["温暖纸感", "儿童绘本", "角色外观保持一致"]
            },
            "message": "插图任务已完成，当前为 mock 图片结果"
        });
        Ok(GenerationJob {
            id: job_id,
            workspace_id,
            storybook_id: Some(storybook_id),
            job_type: "storybook_page_image".to_string(),
            status: "succeeded".to_string(),
            input_json: serde_json::json!({
                "page_id": page_id,
                "prompt": output_json["image"]["prompt"].clone(),
                "mode": "storybook_page_image",
                "image_mode": output_json["image"]["image_mode"].clone(),
                "reference_role_ids": reference_role_ids,
                "reference_images": output_json["image"]["reference_images"].clone(),
                "edit_instruction": output_json["image"]["edit_instruction"].clone(),
                "strength": output_json["image"]["strength"].clone()
            }),
            output_json: Some(output_json),
            attempt_count: 1,
            last_error: None,
            next_run_at: None,
            locked_by: None,
            locked_at: None,
            created_at: Utc::now(),
            finished_at: Some(Utc::now()),
        })
    }
}

pub async fn create_role_reference_image_task(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    role_id: Uuid,
    payload: CreateImageTaskRequest,
) -> Result<GenerationJob, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let queued = crate::repositories::generation::create_role_reference_image_job_record(
            &ctx.db,
            workspace_id,
            storybook_id,
            role_id,
            payload,
        )
        .await
        .map_err(common::db_error)?;
        enqueue_generation_page_image_job(ctx, workspace_id, queued.id)
            .await
            .map_err(|err| ApiError::state_conflict(format!("角色参考图任务入队失败：{err}")))?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "generation_job.created",
            "generation_job",
            Some(queued.id),
            json!({
                "storybook_id": storybook_id,
                "role_id": role_id,
                "job_type": queued.job_type,
                "status": queued.status,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(queued);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_editor(&state, headers, workspace_id)?;
        let book = find_storybook(&state, workspace_id, storybook_id)?;
        let role = book
            .roles
            .iter()
            .find(|role| role.id == role_id)
            .ok_or_else(|| ApiError::not_found("role"))?;
        let job_id = Uuid::new_v4();
        let prompt = payload.prompt.unwrap_or_else(|| {
            format!(
                "为幼儿园绘本角色生成参考图：{}，{}",
                role.name, role.appearance
            )
        });
        let output_json = serde_json::json!({
            "image": {
                "target_id": role_id,
                "target_type": "role",
                "role_id": role_id,
                "image_url": format!("/generated-images/mock-{job_id}.png"),
                "alt_text": "AI 生成的角色参考图",
                "prompt": prompt,
                "image_mode": payload.image_mode.unwrap_or_else(|| "text_to_image".to_string()),
                "reference_images": payload.reference_image_urls
                    .into_iter()
                    .map(|url| serde_json::json!({
                        "url": url,
                        "source": "direct",
                        "role_id": null,
                        "label": null
                    }))
                    .collect::<Vec<_>>(),
                "edit_instruction": payload.edit_instruction,
                "strength": payload.strength,
                "style_notes": ["角色参考图", "后续插图保持一致"]
            },
            "message": "角色参考图任务已完成，当前为 mock 图片结果"
        });
        Ok(GenerationJob {
            id: job_id,
            workspace_id,
            storybook_id: Some(storybook_id),
            job_type: "storybook_role_reference_image".to_string(),
            status: "succeeded".to_string(),
            input_json: serde_json::json!({
                "role_id": role_id,
                "prompt": output_json["image"]["prompt"].clone(),
                "mode": "storybook_role_reference_image",
                "image_mode": output_json["image"]["image_mode"].clone(),
                "reference_images": output_json["image"]["reference_images"].clone(),
                "edit_instruction": output_json["image"]["edit_instruction"].clone(),
                "strength": output_json["image"]["strength"].clone()
            }),
            output_json: Some(output_json),
            attempt_count: 1,
            last_error: None,
            next_run_at: None,
            locked_by: None,
            locked_at: None,
            created_at: Utc::now(),
            finished_at: Some(Utc::now()),
        })
    }
}

pub async fn create_job(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    payload: CreateGenerationJobRequest,
) -> Result<GenerationJob, ApiError> {
    #[cfg(feature = "db")]
    {
        let workspace = common::require_editor_db(ctx, headers, workspace_id).await?;
        let job_type = common::required(payload.job_type, "job_type")?;
        if job_type == "customization_plan" {
            if payload.storybook_id.is_none() {
                return Err(ApiError::validation(
                    "storybook_id",
                    "定制方案需要选择普通绘本",
                ));
            }
            let child_id = payload
                .input_json
                .get("child_id")
                .and_then(|value| value.as_str())
                .ok_or_else(|| ApiError::validation("child_id", "定制方案需要选择儿童档案"))?;
            let child_id = Uuid::parse_str(child_id)
                .map_err(|_| ApiError::validation("child_id", "儿童档案 ID 格式不正确"))?;
            match common::child_classroom_scope(ctx, headers, workspace_id, &workspace).await? {
                Some(classrooms) => {
                    crate::repositories::children::find_for_classrooms(
                        &ctx.db,
                        workspace_id,
                        child_id,
                        &classrooms,
                    )
                    .await
                    .map_err(common::db_error)?;
                }
                None => {
                    crate::repositories::children::find(&ctx.db, workspace_id, child_id)
                        .await
                        .map_err(common::db_error)?;
                }
            }
        }
        let queued = crate::repositories::generation::create_generation_job_record(
            &ctx.db,
            workspace_id,
            CreateGenerationJobRequest {
                job_type,
                storybook_id: payload.storybook_id,
                input_json: payload.input_json,
            },
        )
        .await
        .map_err(common::db_error)?;
        enqueue_generation_job(ctx, workspace_id, queued.id)
            .await
            .map_err(|err| ApiError::state_conflict(format!("生成任务入队失败：{err}")))?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "generation_job.created",
            "generation_job",
            Some(queued.id),
            json!({
                "storybook_id": queued.storybook_id,
                "job_type": queued.job_type,
                "status": queued.status,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(queued);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_editor(&state, headers, workspace_id)?;
        let job_type = common::required(payload.job_type, "job_type")?;
        Ok(GenerationJob {
            id: Uuid::new_v4(),
            workspace_id,
            storybook_id: payload.storybook_id,
            job_type,
            status: "succeeded".to_string(),
            input_json: payload.input_json,
            output_json: Some(serde_json::json!({"message": "生成任务已完成，当前为 mock 结果"})),
            attempt_count: 1,
            last_error: None,
            next_run_at: None,
            locked_by: None,
            locked_at: None,
            created_at: Utc::now(),
            finished_at: Some(Utc::now()),
        })
    }
}

pub async fn list_jobs(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    query: GenerationJobListQuery,
) -> Result<(Vec<GenerationJob>, PaginationMeta), ApiError> {
    #[cfg(feature = "db")]
    {
        let workspace = common::require_workspace_db(ctx, headers, workspace_id).await?;
        let (mut jobs, meta) = crate::repositories::generation::list_jobs_page(
            &ctx.db,
            workspace_id,
            query.storybook_id,
            query.limit,
            query.offset,
        )
        .await
        .map_err(common::db_error)?;
        if workspace.role == WorkspaceRole::SchoolTeacher {
            jobs = jobs.into_iter().map(redact_generation_job_input).collect();
        }
        let jobs = jobs
            .into_iter()
            .map(|job| with_generation_image_download_url(job, workspace_id))
            .collect::<Vec<_>>();
        return Ok((jobs, meta));
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        Ok(common::paginate_vec(Vec::new(), query.limit, query.offset))
    }
}

pub async fn get_job(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    job_id: Uuid,
) -> Result<GenerationJob, ApiError> {
    #[cfg(feature = "db")]
    {
        let workspace = common::require_workspace_db(ctx, headers, workspace_id).await?;
        let mut job = crate::repositories::generation::find_job(&ctx.db, workspace_id, job_id)
            .await
            .map_err(common::db_error)?;
        if workspace.role == WorkspaceRole::SchoolTeacher {
            job = redact_generation_job_input(job);
        }
        return Ok(with_generation_image_download_url(job, workspace_id));
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        Ok(GenerationJob {
            id: job_id,
            workspace_id,
            storybook_id: None,
            job_type: "storybook_page_image".to_string(),
            status: "succeeded".to_string(),
            input_json: serde_json::json!({}),
            output_json: Some(serde_json::json!({
                "message": "当前为 mock 任务状态"
            })),
            attempt_count: 1,
            last_error: None,
            next_run_at: None,
            locked_by: None,
            locked_at: None,
            created_at: Utc::now(),
            finished_at: Some(Utc::now()),
        })
    }
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

pub fn public_generated_image_file(file_name: &str) -> Result<(String, Vec<u8>), ApiError> {
    let safe_name = file_name.trim();
    if !valid_generated_image_file_name(safe_name) {
        return Err(ApiError::not_found("generated_image"));
    }
    Err(ApiError::not_found("generated_image"))
}

fn read_generation_image_file(job: &GenerationJob) -> Result<(String, Vec<u8>), ApiError> {
    if job.status != "succeeded" || job.job_type != "storybook_page_image" {
        return Err(ApiError::not_found("generated_image"));
    }
    let Some(file_name) = generation_image_file_name(job) else {
        return Err(ApiError::not_found("generated_image"));
    };
    let bytes = crate::services::storage::read_generated_image(&file_name)
        .map_err(|_| ApiError::not_found("generated_image"))?;
    Ok((file_name, bytes))
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

#[cfg(feature = "db")]
fn redact_generation_job_input(mut job: GenerationJob) -> GenerationJob {
    job.input_json = json!({
        "redacted": true,
        "reason": "limited_workspace_role"
    });
    job
}

fn with_generation_image_download_url(mut job: GenerationJob, workspace_id: Uuid) -> GenerationJob {
    if job.status == "succeeded"
        && job.job_type == "storybook_page_image"
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

#[cfg(not(feature = "db"))]
fn find_storybook(
    state: &crate::state::SharedState,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<crate::models::Storybook, ApiError> {
    state
        .read()
        .expect("state lock poisoned")
        .storybooks
        .iter()
        .find(|item| item.workspace_id == workspace_id && item.id == storybook_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("storybook"))
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
