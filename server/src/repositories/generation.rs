use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

use crate::models::{
    CreateGenerationJobRequest, CreateImageTaskRequest, GenerationJob, PaginationMeta,
};
use crate::repositories::generation_costs::{
    ensure_generation_budget_available, record_generation_cost_log,
};
use crate::repositories::generation_image_tasks::{
    cover_image_job_input, image_request_from_job, image_target_from_job, is_image_job,
    page_image_job_input, role_reference_image_job_input,
};
use crate::services::generation_provider::{
    ConfiguredGenerationProvider, GenerationProviderError, GenerationRequest,
    ImageGenerationRequest,
};

const ALLOWED_JOB_TYPES: &[&str] = &[
    "storybook_plan",
    "storybook_roles",
    "storybook_pages",
    "storybook_page_prompt",
    "storybook_cover_image",
    "storybook_page_image",
    "storybook_role_reference_image",
    "customization_plan",
];
const INLINE_WORKER_ID: &str = "inline-mock-executor";
const DEFAULT_MAX_AUTO_ATTEMPTS: i32 = 1;
const SINGLE_ACTIVE_STORYBOOK_JOB_TYPES: &[&str] =
    &["storybook_plan", "storybook_roles", "storybook_pages"];

pub async fn retry_failed_job(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    job_id: Uuid,
) -> Result<GenerationJob, DbErr> {
    let job = find_job(db, workspace_id, job_id).await?;
    if job.status != "failed" {
        return Err(DbErr::Custom("只有失败的生成任务可以重试".to_string()));
    }
    if !ALLOWED_JOB_TYPES.contains(&job.job_type.as_str()) {
        return Err(DbErr::Custom(format!(
            "不支持的生成任务类型：{}",
            job.job_type
        )));
    }
    if let Some(storybook_id) = job.storybook_id {
        ensure_storybook_in_workspace(db, workspace_id, storybook_id).await?;
    }

    let job = crate::repositories::generation_jobs::move_to_running(
        db,
        job.id,
        "failed",
        INLINE_WORKER_ID,
    )
    .await?;
    if job.job_type == "storybook_role_reference_image" {
        if let (Some(storybook_id), Some(role_id)) = (job.storybook_id, role_id_from_job(&job)) {
            mark_role_reference_status(db, storybook_id, role_id, "generating").await?;
        }
    } else if job.job_type == "storybook_page_image" {
        if let (Some(storybook_id), Some(page_id)) = (job.storybook_id, page_id_from_job(&job)) {
            mark_page_image_status(db, storybook_id, page_id, "generating").await?;
        }
    }

    let provider = ConfiguredGenerationProvider::from_env();
    let provider_name = provider.name_for_job_type(&job.job_type);
    let retried = if is_image_job(&job.job_type) {
        let target = image_target_from_job(&job)?;
        let image_request = image_request_from_job(&job)?;

        let image_id = job.id.to_string();
        match provider
            .generate_image(ImageGenerationRequest {
                image_id: &image_id,
                target_id: target.target_id.as_str(),
                target_type: target.target_type,
                mode: &job.job_type,
                prompt: image_request.prompt.as_str(),
                reference_images: image_request.reference_images,
                edit_instruction: image_request.edit_instruction,
                image_mode: image_request.image_mode,
                strength: image_request.strength,
                size: image_request.size,
            })
            .await
        {
            Ok(output_json) => {
                let completed = complete_and_apply_running_job(db, job.id, output_json).await?;
                mark_image_job_success_status(db, &completed, &job).await?;
                completed
            }
            Err(err) => {
                fail_running_job(
                    db,
                    job.id,
                    provider_name,
                    &job.job_type,
                    job.attempt_count,
                    err,
                )
                .await?
            }
        }
    } else {
        match provider
            .generate(GenerationRequest {
                job_type: &job.job_type,
                input: &job.input_json,
            })
            .await
        {
            Ok(output_json) => complete_and_apply_running_job(db, job.id, output_json).await?,
            Err(err) => {
                fail_running_job(
                    db,
                    job.id,
                    provider_name,
                    &job.job_type,
                    job.attempt_count,
                    err,
                )
                .await?
            }
        }
    };

    Ok(retried)
}

pub async fn execute_generation_job(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    job_id: Uuid,
) -> Result<GenerationJob, DbErr> {
    let job = find_job(db, workspace_id, job_id).await?;
    execute_generation_record(db, job).await
}

pub async fn process_generation_backlog(
    db: &DatabaseConnection,
    age_minutes: i64,
    limit: usize,
) -> Result<u64, DbErr> {
    process_generation_backlog_scoped(db, None, age_minutes, limit).await
}

pub async fn process_generation_backlog_for_workspace(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    age_minutes: i64,
    limit: usize,
) -> Result<u64, DbErr> {
    process_generation_backlog_scoped(db, Some(workspace_id), age_minutes, limit).await
}

async fn process_generation_backlog_scoped(
    db: &DatabaseConnection,
    workspace_id: Option<Uuid>,
    age_minutes: i64,
    limit: usize,
) -> Result<u64, DbErr> {
    let mut processed = crate::repositories::generation_jobs::requeue_stale_jobs_scoped(
        db,
        workspace_id,
        age_minutes,
        max_auto_attempts(),
    )
    .await?;
    let limit = limit.max(1);
    let worker_id = "kindleaf-scheduler";

    for _ in 0..limit {
        let Some(job) = crate::repositories::generation_jobs::claim_next_ready_job_scoped(
            db,
            worker_id,
            workspace_id,
            max_auto_attempts(),
        )
        .await?
        else {
            break;
        };
        let executed = execute_claimed_generation_record(db, job).await?;
        if executed.status == "succeeded" || executed.status == "failed" {
            processed += 1;
        }
    }

    Ok(processed)
}

pub async fn create_generation_job_record(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    created_by: Uuid,
    payload: CreateGenerationJobRequest,
) -> Result<GenerationJob, DbErr> {
    ensure_generation_budget_available(db, Some(workspace_id)).await?;
    let job_type = payload.job_type.trim();
    if !ALLOWED_JOB_TYPES.contains(&job_type) {
        return Err(DbErr::Custom(format!("不支持的生成任务类型：{job_type}")));
    }
    if let Some(storybook_id) = payload.storybook_id {
        ensure_storybook_in_workspace(db, workspace_id, storybook_id).await?;
        if SINGLE_ACTIVE_STORYBOOK_JOB_TYPES.contains(&job_type)
            && let Some(active_job) =
                crate::repositories::generation_jobs::find_active_storybook_job(
                    db,
                    workspace_id,
                    storybook_id,
                    job_type,
                    max_auto_attempts(),
                )
                .await?
        {
            return Ok(active_job);
        }
    }
    if job_type == "customization_plan" {
        let child_id = payload
            .input_json
            .get("child_id")
            .and_then(|value| value.as_str())
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(|| DbErr::Custom("定制方案需要有效儿童档案 ID".to_string()))?;
        ensure_child_in_workspace(db, workspace_id, child_id).await?;
    }

    let input_json =
        enriched_generation_input(db, job_type, payload.storybook_id, payload.input_json).await?;

    let job = crate::repositories::generation_jobs::enqueue_job(
        db,
        workspace_id,
        payload.storybook_id,
        created_by,
        job_type,
        input_json,
    )
    .await?;
    if is_image_job(job_type) {
        crate::repositories::storybook_image_variants::create_generating_variant_for_job(db, &job)
            .await?;
    }
    Ok(job)
}

async fn enriched_generation_input(
    db: &DatabaseConnection,
    job_type: &str,
    storybook_id: Option<Uuid>,
    mut input_json: JsonValue,
) -> Result<JsonValue, DbErr> {
    if job_type == "storybook_pages" && input_json.get("confirmed_roles").is_none() {
        if let Some(storybook_id) = storybook_id {
            let confirmed_roles = confirmed_roles_for_storybook(db, storybook_id).await?;
            if !confirmed_roles.is_empty() {
                input_json["confirmed_roles"] = json!(confirmed_roles);
            }
        }
    }
    if job_type == "storybook_page_prompt" {
        let Some(storybook_id) = storybook_id else {
            return Err(DbErr::Custom(
                "插图描述重写任务需要关联绘本（storybook_id）".to_string(),
            ));
        };
        let page_id = input_json
            .get("page_id")
            .and_then(|value| value.as_str())
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(|| {
                DbErr::Custom("插图描述重写任务需要有效页面 ID（page_id）".to_string())
            })?;
        let page = storybook_page_for_prompt(db, storybook_id, page_id).await?;
        let page_number = page
            .get("page_number")
            .and_then(|value| value.as_i64())
            .unwrap_or(0) as i32;
        let neighbor_pages = neighbor_pages_for_prompt(db, storybook_id, page_number).await?;
        input_json["neighbor_pages"] = json!(neighbor_pages);
        input_json["page"] = page;
        if input_json.get("confirmed_roles").is_none() {
            let confirmed_roles = confirmed_roles_for_storybook(db, storybook_id).await?;
            if !confirmed_roles.is_empty() {
                input_json["confirmed_roles"] = json!(confirmed_roles);
            }
        }
    }
    Ok(input_json)
}

async fn storybook_page_for_prompt(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    page_id: Uuid,
) -> Result<JsonValue, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select page_number, title, body, illustration_prompt
            from storybook_pages
            where storybook_id = $1 and id = $2
            limit 1
            "#,
            [storybook_id.into(), page_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("storybook_page".to_string()))?;
    Ok(json!({
        "page_id": page_id.to_string(),
        "page_number": row.try_get::<i32>("", "page_number")?,
        "title": row.try_get::<String>("", "title")?,
        "body": row.try_get::<String>("", "body")?,
        "illustration_prompt": row.try_get::<String>("", "illustration_prompt")?,
    }))
}

/// 单页重写需要看到前后相邻页的插图描述，保证跨页场景连续。
async fn neighbor_pages_for_prompt(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    page_number: i32,
) -> Result<Vec<JsonValue>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select page_number, title, illustration_prompt
            from storybook_pages
            where storybook_id = $1 and page_number in ($2, $3)
            order by page_number asc
            "#,
            [
                storybook_id.into(),
                (page_number - 1).into(),
                (page_number + 1).into(),
            ],
        ))
        .await?;

    rows.into_iter()
        .map(|row| {
            Ok(json!({
                "page_number": row.try_get::<i32>("", "page_number")?,
                "title": row.try_get::<String>("", "title")?,
                "illustration_prompt": row.try_get::<String>("", "illustration_prompt")?,
            }))
        })
        .collect()
}

async fn confirmed_roles_for_storybook(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<Vec<JsonValue>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select name, role_type, appearance, coalesce(story_function, '') as story_function
            from storybook_roles
            where storybook_id = $1
            order by name asc, id asc
            "#,
            [storybook_id.into()],
        ))
        .await?;

    rows.into_iter()
        .map(|row| {
            Ok(json!({
                "name": row.try_get::<String>("", "name")?,
                "role_type": row.try_get::<String>("", "role_type")?,
                "appearance": row.try_get::<String>("", "appearance")?,
                "story_function": row.try_get::<String>("", "story_function")?,
            }))
        })
        .collect()
}

pub async fn create_page_image_job_record(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    created_by: Uuid,
    storybook_id: Uuid,
    page_id: Uuid,
    payload: CreateImageTaskRequest,
) -> Result<GenerationJob, DbErr> {
    ensure_generation_budget_available(db, Some(workspace_id)).await?;
    let input_json = page_image_job_input(db, workspace_id, storybook_id, page_id, payload).await?;
    let job = crate::repositories::generation_jobs::enqueue_job(
        db,
        workspace_id,
        Some(storybook_id),
        created_by,
        "storybook_page_image",
        input_json,
    )
    .await?;
    crate::repositories::storybook_image_variants::create_generating_variant_for_job(db, &job)
        .await?;
    mark_page_image_status(db, storybook_id, page_id, "generating").await?;
    Ok(job)
}

pub async fn create_cover_image_job_record(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    created_by: Uuid,
    storybook_id: Uuid,
    payload: CreateImageTaskRequest,
) -> Result<GenerationJob, DbErr> {
    ensure_generation_budget_available(db, Some(workspace_id)).await?;
    ensure_storybook_in_workspace(db, workspace_id, storybook_id).await?;
    let input_json = cover_image_job_input(db, workspace_id, storybook_id, payload).await?;
    let job = crate::repositories::generation_jobs::enqueue_job(
        db,
        workspace_id,
        Some(storybook_id),
        created_by,
        "storybook_cover_image",
        input_json,
    )
    .await?;
    crate::repositories::storybook_image_variants::create_generating_variant_for_job(db, &job)
        .await?;
    Ok(job)
}

pub async fn create_role_reference_image_job_record(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    created_by: Uuid,
    storybook_id: Uuid,
    role_id: Uuid,
    payload: CreateImageTaskRequest,
) -> Result<GenerationJob, DbErr> {
    ensure_generation_budget_available(db, Some(workspace_id)).await?;
    let input_json =
        role_reference_image_job_input(db, workspace_id, storybook_id, role_id, payload).await?;
    let job = crate::repositories::generation_jobs::enqueue_job(
        db,
        workspace_id,
        Some(storybook_id),
        created_by,
        "storybook_role_reference_image",
        input_json,
    )
    .await?;
    crate::repositories::storybook_image_variants::create_generating_variant_for_job(db, &job)
        .await?;
    mark_role_reference_status(db, storybook_id, role_id, "generating").await?;
    Ok(job)
}

pub async fn retry_generation_job(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    job_id: Uuid,
) -> Result<GenerationJob, DbErr> {
    ensure_generation_budget_available(db, Some(workspace_id)).await?;
    retry_failed_job(db, workspace_id, job_id).await
}

pub async fn cancel_generation_job(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    job_id: Uuid,
) -> Result<GenerationJob, DbErr> {
    let job = find_job(db, workspace_id, job_id).await?;
    if !matches!(job.status.as_str(), "queued" | "failed") {
        return Err(DbErr::Custom("generation_job_not_cancelable".to_string()));
    }

    crate::repositories::generation_jobs::cancel_job(db, workspace_id, job_id).await
}

/// 清理工作区内全部失败的生成任务记录，返回清理条数。
pub async fn delete_failed_generation_jobs(
    db: &DatabaseConnection,
    workspace_id: Uuid,
) -> Result<u64, DbErr> {
    let result = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "delete from generation_jobs where workspace_id = $1 and status = 'failed'",
            [workspace_id.into()],
        ))
        .await?;
    Ok(result.rows_affected())
}

async fn execute_generation_record(
    db: &DatabaseConnection,
    job: GenerationJob,
) -> Result<GenerationJob, DbErr> {
    if job.status != "queued" && job.status != "failed" {
        return Err(DbErr::Custom(
            "只有待执行或失败的生成任务可以继续执行".to_string(),
        ));
    }
    if let Some(storybook_id) = job.storybook_id {
        ensure_storybook_in_workspace(db, job.workspace_id, storybook_id).await?;
    }

    let running = crate::repositories::generation_jobs::move_to_running(
        db,
        job.id,
        job.status.as_str(),
        INLINE_WORKER_ID,
    )
    .await?;
    execute_claimed_generation_record(db, running).await
}

async fn execute_claimed_generation_record(
    db: &DatabaseConnection,
    job: GenerationJob,
) -> Result<GenerationJob, DbErr> {
    if job.status != "running" {
        return Err(DbErr::Custom(
            "只有 running 状态的生成任务可以被执行".to_string(),
        ));
    }

    let provider = ConfiguredGenerationProvider::from_env();
    let job_type = job.job_type.clone();
    let provider_name = provider.name_for_job_type(&job_type);
    let updated = if is_image_job(&job_type) {
        let target = image_target_from_job(&job)?;
        let image_request = image_request_from_job(&job)?;

        let image_id = job.id.to_string();
        match provider
            .generate_image(ImageGenerationRequest {
                image_id: &image_id,
                target_id: target.target_id.as_str(),
                target_type: target.target_type,
                mode: &job_type,
                prompt: image_request.prompt.as_str(),
                reference_images: image_request.reference_images,
                edit_instruction: image_request.edit_instruction,
                image_mode: image_request.image_mode,
                strength: image_request.strength,
                size: image_request.size,
            })
            .await
        {
            Ok(output_json) => {
                let completed = complete_and_apply_running_job(db, job.id, output_json).await?;
                mark_image_job_success_status(db, &completed, &job).await?;
                completed
            }
            Err(err) => {
                fail_running_job(db, job.id, provider_name, &job_type, job.attempt_count, err)
                    .await?
            }
        }
    } else if job_type == "storybook_plan" && ConfiguredGenerationProvider::ready_for_text() {
        match provider
            .generate(GenerationRequest {
                job_type: &job_type,
                input: &job.input_json,
            })
            .await
        {
            Ok(output_json) => complete_and_apply_running_job(db, job.id, output_json).await?,
            Err(err) => {
                fail_running_job(db, job.id, provider_name, &job_type, job.attempt_count, err)
                    .await?
            }
        }
    } else {
        match provider
            .generate(GenerationRequest {
                job_type: &job_type,
                input: &job.input_json,
            })
            .await
        {
            Ok(output_json) => complete_and_apply_running_job(db, job.id, output_json).await?,
            Err(err) => {
                fail_running_job(db, job.id, provider_name, &job_type, job.attempt_count, err)
                    .await?
            }
        }
    };

    Ok(updated)
}

async fn complete_and_apply_running_job(
    db: &DatabaseConnection,
    job_id: Uuid,
    output_json: JsonValue,
) -> Result<GenerationJob, DbErr> {
    let existing_job = crate::repositories::generation_jobs::find_any_job(db, job_id).await?;
    if is_image_job(&existing_job.job_type) {
        crate::repositories::generation_writeback::ensure_image_output_within_storage_quota(
            db,
            &existing_job,
            &output_json,
        )
        .await?;
    }
    let job =
        crate::repositories::generation_jobs::complete_running_job(db, job_id, output_json).await?;
    record_generation_cost_log(db, &job).await?;
    if is_image_job(&job.job_type) {
        crate::repositories::storybook_image_variants::mark_job_variant_ready(db, &job).await?;
    }
    crate::repositories::generation_writeback::apply_completed_generation(db, &job).await?;
    Ok(job)
}

async fn fail_running_job(
    db: &DatabaseConnection,
    job_id: Uuid,
    provider_name: &str,
    job_type: &str,
    attempt_count: i32,
    err: GenerationProviderError,
) -> Result<GenerationJob, DbErr> {
    let safe_message = err.safe_message();
    let should_auto_retry = err.retryable && attempt_count < max_auto_attempts();
    let next_run_interval = if should_auto_retry {
        Some("30 seconds")
    } else {
        None
    };
    let output_json = json!({
        "schema_version": "generation.error.v1",
        "provider": provider_name,
        "mode": job_type,
        "message": "生成任务失败，可稍后重试",
        "error": {
            "code": "provider_failed",
            "message": safe_message.clone(),
            "retryable": err.retryable,
            "auto_retry": should_auto_retry,
            "attempt_count": attempt_count,
            "max_auto_attempts": max_auto_attempts()
        }
    });
    let job = crate::repositories::generation_jobs::fail_running_job(
        db,
        job_id,
        output_json,
        safe_message.clone(),
        next_run_interval,
    )
    .await?;
    if job.job_type == "storybook_role_reference_image" {
        crate::repositories::storybook_image_variants::mark_job_variant_failed(
            db,
            &job,
            &safe_message,
        )
        .await?;
        if let (Some(storybook_id), Some(role_id)) = (job.storybook_id, role_id_from_job(&job)) {
            mark_role_reference_status(db, storybook_id, role_id, "failed").await?;
        }
    } else if job.job_type == "storybook_page_image" {
        crate::repositories::storybook_image_variants::mark_job_variant_failed(
            db,
            &job,
            &safe_message,
        )
        .await?;
        if let (Some(storybook_id), Some(page_id)) = (job.storybook_id, page_id_from_job(&job)) {
            mark_page_image_status(db, storybook_id, page_id, "failed").await?;
        }
    } else if job.job_type == "storybook_cover_image" {
        crate::repositories::storybook_image_variants::mark_job_variant_failed(
            db,
            &job,
            &safe_message,
        )
        .await?;
    }
    record_generation_cost_log(db, &job).await?;
    Ok(job)
}

fn max_auto_attempts() -> i32 {
    std::env::var("KINDLEAF_GENERATION_MAX_AUTO_ATTEMPTS")
        .ok()
        .and_then(|value| value.trim().parse::<i32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MAX_AUTO_ATTEMPTS)
}

async fn mark_image_job_success_status(
    db: &DatabaseConnection,
    completed: &GenerationJob,
    source_job: &GenerationJob,
) -> Result<(), DbErr> {
    if completed.status == "succeeded"
        && source_job.job_type == "storybook_page_image"
        && let (Some(storybook_id), Some(page_id)) =
            (completed.storybook_id, page_id_from_job(source_job))
    {
        mark_page_image_status(db, storybook_id, page_id, "ready").await?;
    }
    Ok(())
}

async fn mark_page_image_status(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    page_id: Uuid,
    status: &str,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_pages
        set status = $3
        where storybook_id = $1 and id = $2
        "#,
        vec![storybook_id.into(), page_id.into(), status.into()],
    ))
    .await?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybooks
        set updated_at = now()
        where id = $1
        "#,
        [storybook_id.into()],
    ))
    .await?;
    Ok(())
}

async fn mark_role_reference_status(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    role_id: Uuid,
    status: &str,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_roles
        set reference_status = $3
        where storybook_id = $1 and id = $2
        "#,
        vec![storybook_id.into(), role_id.into(), status.into()],
    ))
    .await?;
    Ok(())
}

fn role_id_from_job(job: &GenerationJob) -> Option<Uuid> {
    job.input_json
        .get("role_id")
        .and_then(|value| value.as_str())
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn page_id_from_job(job: &GenerationJob) -> Option<Uuid> {
    job.input_json
        .get("page_id")
        .and_then(|value| value.as_str())
        .and_then(|value| Uuid::parse_str(value).ok())
}

async fn ensure_storybook_in_workspace(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<(), DbErr> {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        select id
        from storybooks
        where workspace_id = $1 and id = $2
        limit 1
        "#,
        [workspace_id.into(), storybook_id.into()],
    ))
    .await?
    .ok_or_else(|| DbErr::RecordNotFound("storybook".to_string()))?;
    Ok(())
}

async fn ensure_child_in_workspace(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    child_id: Uuid,
) -> Result<(), DbErr> {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        select id
        from children
        where workspace_id = $1 and id = $2 and status = 'active'
        limit 1
        "#,
        [workspace_id.into(), child_id.into()],
    ))
    .await?
    .ok_or_else(|| DbErr::RecordNotFound("child".to_string()))?;
    Ok(())
}

#[allow(dead_code)]
pub async fn claim_next_ready_job(
    db: &DatabaseConnection,
    worker_id: &str,
) -> Result<Option<GenerationJob>, DbErr> {
    crate::repositories::generation_jobs::claim_next_ready_job_scoped(
        db,
        worker_id,
        None,
        max_auto_attempts(),
    )
    .await
}

pub async fn find_job(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    job_id: Uuid,
) -> Result<GenerationJob, DbErr> {
    crate::repositories::generation_jobs::find_job(db, workspace_id, job_id).await
}

pub async fn list_jobs_page(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Option<Uuid>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<(Vec<GenerationJob>, PaginationMeta), DbErr> {
    crate::repositories::generation_jobs::list_jobs_page(
        db,
        workspace_id,
        storybook_id,
        limit,
        offset,
    )
    .await
}
