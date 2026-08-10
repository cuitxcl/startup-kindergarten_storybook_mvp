use axum::http::HeaderMap;
#[cfg(not(feature = "db"))]
use chrono::Utc;
use loco_rs::app::AppContext;
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
        ImageVariantListQuery, PaginationMeta, StorybookImageVariant, WorkspaceRole,
    },
};

pub use crate::application::generation_image_access::{
    generation_image_file, public_generated_image_file, with_generation_image_download_url,
};
pub use crate::application::generation_job_actions::{
    RecoverGenerationJobsRequest, cancel_job, clear_failed_jobs, recover_jobs, retry_job,
};

pub async fn list_image_variants(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    query: ImageVariantListQuery,
) -> Result<Vec<StorybookImageVariant>, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
        crate::repositories::delivery::ensure_storybook_in_workspace(
            &ctx.db,
            workspace_id,
            storybook_id,
        )
        .await
        .map_err(common::db_error)?;
        let variants = crate::repositories::storybook_image_variants::list_variants(
            &ctx.db,
            workspace_id,
            storybook_id,
            query,
        )
        .await
        .map_err(common::db_error)?;
        return Ok(variants
            .into_iter()
            .map(with_variant_image_download_url)
            .collect());
    }

    #[cfg(not(feature = "db"))]
    {
        let _ = (ctx, headers, workspace_id, storybook_id, query);
        Ok(Vec::new())
    }
}

pub async fn select_image_variant(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    variant_id: Uuid,
) -> Result<StorybookImageVariant, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let actor_id = common::actor_user_id(headers)?;
        let variant = crate::repositories::storybook_image_variants::select_variant(
            &ctx.db,
            workspace_id,
            storybook_id,
            variant_id,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "storybook_image_variant.selected",
            "storybook_image_variant",
            Some(variant.id),
            json!({
                "storybook_id": storybook_id,
                "target_type": variant.target_type,
                "target_id": variant.target_id,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(with_variant_image_download_url(variant));
    }

    #[cfg(not(feature = "db"))]
    {
        let _ = (ctx, headers, workspace_id, storybook_id, variant_id);
        Err(ApiError::not_found("storybook_image_variant"))
    }
}

pub fn with_variant_image_download_url(
    mut variant: StorybookImageVariant,
) -> StorybookImageVariant {
    if variant.status == "ready"
        && variant.image_url.is_some()
        && let Some(job_id) = variant.generation_job_id
    {
        variant.image_url = Some(format!(
            "/api/workspaces/{}/generation-jobs/{}/image",
            variant.workspace_id, job_id
        ));
    }
    variant
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
        let actor_id = common::actor_user_id(headers)?;
        let queued = crate::repositories::generation::create_page_image_job_record(
            &ctx.db,
            workspace_id,
            actor_id,
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
            Some(actor_id),
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
            created_by: None,
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

pub async fn create_cover_image_task(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    payload: CreateImageTaskRequest,
) -> Result<GenerationJob, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let actor_id = common::actor_user_id(headers)?;
        let queued = crate::repositories::generation::create_cover_image_job_record(
            &ctx.db,
            workspace_id,
            actor_id,
            storybook_id,
            payload,
        )
        .await
        .map_err(common::db_error)?;
        enqueue_generation_page_image_job(ctx, workspace_id, queued.id)
            .await
            .map_err(|err| ApiError::state_conflict(format!("封面图任务入队失败：{err}")))?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "generation_job.created",
            "generation_job",
            Some(queued.id),
            json!({
                "storybook_id": storybook_id,
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
        let job_id = Uuid::new_v4();
        let prompt = payload.prompt.unwrap_or_else(|| {
            format!(
                "为幼儿园绘本《{}》生成封面插图，主题：{}，画风：{}，不要文字、水印或 logo。",
                book.title, book.teaching_goal, book.cover_tone
            )
        });
        let output_json = serde_json::json!({
            "image": {
                "target_id": storybook_id,
                "target_type": "cover",
                "cover_id": storybook_id,
                "image_url": format!("/generated-images/mock-{job_id}.png"),
                "alt_text": "AI 生成的绘本封面图",
                "prompt": prompt,
                "image_mode": payload.image_mode.unwrap_or_else(|| "text_to_image".to_string()),
                "reference_images": [],
                "edit_instruction": payload.edit_instruction,
                "strength": payload.strength,
                "style_notes": ["绘本封面", "不要文字", "整本画风一致"]
            },
            "message": "封面图任务已完成，当前为 mock 图片结果"
        });
        Ok(GenerationJob {
            id: job_id,
            workspace_id,
            storybook_id: Some(storybook_id),
            created_by: None,
            job_type: "storybook_cover_image".to_string(),
            status: "succeeded".to_string(),
            input_json: serde_json::json!({
                "cover_id": storybook_id,
                "prompt": output_json["image"]["prompt"].clone(),
                "mode": "storybook_cover_image",
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
        let actor_id = common::actor_user_id(headers)?;
        let queued = crate::repositories::generation::create_role_reference_image_job_record(
            &ctx.db,
            workspace_id,
            actor_id,
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
            Some(actor_id),
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
            created_by: None,
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
            common::actor_user_id(headers)?,
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
            created_by: None,
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
            created_by: None,
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

#[cfg(feature = "db")]
fn redact_generation_job_input(mut job: GenerationJob) -> GenerationJob {
    job.input_json = json!({
        "redacted": true,
        "reason": "limited_workspace_role"
    });
    job
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
