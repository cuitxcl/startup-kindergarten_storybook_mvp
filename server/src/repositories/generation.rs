use std::collections::HashSet;

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement, TransactionTrait};
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

use crate::models::{
    CreateGenerationJobRequest, CreateImageTaskRequest, CreationGenerationSummary, GenerationJob,
    PaginationMeta,
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
    "storybook_visual_reference",
    "customization_plan",
    "storybook_customization_derive",
    "creation_storybook_generate",
];
const INLINE_WORKER_ID: &str = "inline-mock-executor";
const DEFAULT_MAX_AUTO_ATTEMPTS: i32 = 1;
const SINGLE_ACTIVE_STORYBOOK_JOB_TYPES: &[&str] =
    &["storybook_plan", "storybook_roles", "storybook_pages"];
const IMAGE_READY_STATUSES: &[&str] = &["editing", "image_pending", "exportable", "listed"];

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
    ensure_retry_job_has_current_snapshot(&job)?;

    let job = crate::repositories::generation_jobs::move_to_running(
        db,
        job.id,
        "failed",
        INLINE_WORKER_ID,
    )
    .await?;
    if job.job_type == "storybook_customization_derive" {
        let run_id = job
            .input_json
            .get("customization_run_id")
            .and_then(JsonValue::as_str)
            .and_then(|value| Uuid::parse_str(value).ok());
        let run_item_id = job
            .input_json
            .get("customization_run_item_id")
            .and_then(JsonValue::as_str)
            .and_then(|value| Uuid::parse_str(value).ok());
        if let Some(item_id) = run_item_id {
            crate::repositories::storybook_customization_runs::mark_item_retrying(
                db,
                workspace_id,
                item_id,
            )
            .await?;
        }
        if let Some(run_id) = run_id {
            crate::repositories::storybook_customization_runs::finish_run(
                db,
                workspace_id,
                run_id,
                None,
            )
            .await?;
        }
    }
    if job.job_type == "storybook_role_reference_image" {
        if let (Some(storybook_id), Some(role_id)) = (job.storybook_id, role_id_from_job(&job)) {
            mark_role_reference_status(db, storybook_id, role_id, "generating").await?;
        }
    } else if job.job_type == "storybook_visual_reference" {
        crate::repositories::storybook_creation_assets::mark_visual_reference_generating_by_job(
            db,
            workspace_id,
            job.id,
        )
        .await?;
    } else if job.job_type == "storybook_page_image" {
        if let (Some(storybook_id), Some(page_id)) = (job.storybook_id, page_id_from_job(&job)) {
            mark_page_image_status(db, storybook_id, page_id, "generating").await?;
        }
    }

    let provider = ConfiguredGenerationProvider::from_env();
    let provider_name = provider.name_for_job_type(&job.job_type);
    if matches!(
        job.job_type.as_str(),
        "creation_storybook_generate" | "storybook_customization_derive"
    ) {
        return execute_claimed_generation_record(db, job).await;
    }
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
                match complete_and_apply_running_job(db, job.id, output_json).await {
                    Ok(completed) => {
                        mark_image_job_success_status(db, &completed, &job).await?;
                        completed
                    }
                    Err(err) => {
                        let writeback_error =
                            GenerationProviderError::new(format!("生成结果写回失败：{err}"));
                        fail_running_job(
                            db,
                            job.id,
                            provider_name,
                            &job.job_type,
                            job.attempt_count,
                            writeback_error,
                        )
                        .await?
                    }
                }
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
            Ok(output_json) => {
                match complete_and_apply_running_job(db, job.id, output_json).await {
                    Ok(completed) => completed,
                    Err(err) => {
                        let writeback_error =
                            GenerationProviderError::new(format!("生成结果写回失败：{err}"));
                        fail_running_job(
                            db,
                            job.id,
                            provider_name,
                            &job.job_type,
                            job.attempt_count,
                            writeback_error,
                        )
                        .await?
                    }
                }
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
    let stopped = crate::repositories::generation_jobs::stop_exhausted_stale_jobs_scoped(
        db,
        workspace_id,
        age_minutes,
        max_auto_attempts(),
    )
    .await?;
    for job in &stopped {
        propagate_failed_generation_job_state(db, job).await?;
    }
    let requeued = crate::repositories::generation_jobs::requeue_stale_jobs_scoped(
        db,
        workspace_id,
        age_minutes,
        max_auto_attempts(),
    )
    .await?;
    let mut processed = stopped.len() as u64 + requeued;
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
    if is_image_job(job_type) && job_type != "storybook_visual_reference" {
        let storybook_id = payload
            .storybook_id
            .ok_or_else(|| DbErr::Custom("图片生成任务需要关联绘本（storybook_id）".to_string()))?;
        ensure_storybook_ready_for_image_generation(db, workspace_id, storybook_id).await?;
        if let Some((target_key, target_id)) =
            image_target_key_and_id_from_payload(job_type, storybook_id, &payload.input_json)?
            && let Some(active_job) =
                crate::repositories::generation_jobs::find_active_image_target_job(
                    db,
                    workspace_id,
                    storybook_id,
                    job_type,
                    target_key,
                    target_id,
                    max_auto_attempts(),
                )
                .await?
        {
            return Ok(active_job);
        }
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
    if is_image_job(job_type) && job_type != "storybook_visual_reference" {
        crate::repositories::storybook_image_variants::create_generating_variant_for_job(db, &job)
            .await?;
    }
    Ok(job)
}

fn image_target_key_and_id_from_payload(
    job_type: &str,
    storybook_id: Uuid,
    input_json: &JsonValue,
) -> Result<Option<(&'static str, Uuid)>, DbErr> {
    let target = if job_type == "storybook_page_image" {
        Some((
            "page_id",
            input_json
                .get("page_id")
                .and_then(|value| value.as_str())
                .and_then(|value| Uuid::parse_str(value).ok())
                .ok_or_else(|| DbErr::Custom("插图任务需要有效页面 ID（page_id）".to_string()))?,
        ))
    } else if job_type == "storybook_role_reference_image" {
        Some((
            "role_id",
            input_json
                .get("role_id")
                .and_then(|value| value.as_str())
                .and_then(|value| Uuid::parse_str(value).ok())
                .ok_or_else(|| {
                    DbErr::Custom("角色参考图任务需要有效角色 ID（role_id）".to_string())
                })?,
        ))
    } else if job_type == "storybook_cover_image" {
        Some(("cover_id", storybook_id))
    } else {
        None
    };
    Ok(target)
}

async fn enriched_generation_input(
    db: &DatabaseConnection,
    job_type: &str,
    storybook_id: Option<Uuid>,
    mut input_json: JsonValue,
) -> Result<JsonValue, DbErr> {
    if matches!(job_type, "storybook_roles" | "storybook_pages")
        && input_json.get("target_snapshot").is_none()
        && let Some(storybook_id) = storybook_id
    {
        input_json["target_snapshot"] =
            storybook_generation_snapshot(db, storybook_id, job_type).await?;
    }
    if is_image_job(job_type)
        && input_json.get("target_snapshot").is_none()
        && let Some(storybook_id) = storybook_id
    {
        input_json["target_snapshot"] =
            image_job_target_snapshot(db, storybook_id, job_type, &input_json).await?;
    }
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
        input_json["target_snapshot"] =
            storybook_page_prompt_snapshot(db, storybook_id, page_id).await?;
        if input_json.get("confirmed_roles").is_none() {
            let confirmed_roles = confirmed_roles_for_storybook(db, storybook_id).await?;
            if !confirmed_roles.is_empty() {
                input_json["confirmed_roles"] = json!(confirmed_roles);
            }
        }
    }
    Ok(input_json)
}

async fn image_job_target_snapshot(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    job_type: &str,
    input_json: &JsonValue,
) -> Result<JsonValue, DbErr> {
    if job_type == "storybook_page_image" {
        let page_id = input_json
            .get("page_id")
            .and_then(|value| value.as_str())
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(|| DbErr::Custom("插图任务需要有效页面 ID（page_id）".to_string()))?;
        let row = db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                select title, body, illustration_prompt
                from storybook_pages
                where storybook_id = $1 and id = $2
                limit 1
                "#,
                [storybook_id.into(), page_id.into()],
            ))
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("page".to_string()))?;
        return Ok(json!({
            "title": row.try_get::<String>("", "title")?,
            "body": row.try_get::<String>("", "body")?,
            "illustration_prompt": row.try_get::<String>("", "illustration_prompt")?,
        }));
    }
    if job_type == "storybook_role_reference_image" {
        let role_id = input_json
            .get("role_id")
            .and_then(|value| value.as_str())
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(|| DbErr::Custom("角色参考图任务需要有效角色 ID（role_id）".to_string()))?;
        let row = db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                select name, role_type, appearance, coalesce(story_function, '') as story_function, needs_consistency
                from storybook_roles
                where storybook_id = $1 and id = $2
                limit 1
                "#,
                [storybook_id.into(), role_id.into()],
            ))
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("role".to_string()))?;
        return Ok(json!({
            "name": row.try_get::<String>("", "name")?,
            "role_type": row.try_get::<String>("", "role_type")?,
            "appearance": row.try_get::<String>("", "appearance")?,
            "story_function": row.try_get::<String>("", "story_function")?,
            "needs_consistency": row.try_get::<bool>("", "needs_consistency")?,
        }));
    }
    Ok(json!({ "cover_id": storybook_id.to_string() }))
}

async fn storybook_generation_snapshot(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    job_type: &str,
) -> Result<JsonValue, DbErr> {
    if job_type == "storybook_roles" {
        return Ok(json!({
            "roles": roles_snapshot(db, storybook_id).await?,
        }));
    }
    Ok(json!({
        "roles": roles_snapshot(db, storybook_id).await?,
        "pages": pages_snapshot(db, storybook_id).await?,
    }))
}

async fn storybook_page_prompt_snapshot(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    page_id: Uuid,
) -> Result<JsonValue, DbErr> {
    Ok(json!({
        "page": storybook_page_for_prompt(db, storybook_id, page_id).await?,
        "roles": roles_snapshot(db, storybook_id).await?,
    }))
}

async fn roles_snapshot(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<Vec<JsonValue>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, name, role_type, appearance, coalesce(story_function, '') as story_function, needs_consistency
            from storybook_roles
            where storybook_id = $1
            order by id asc
            "#,
            [storybook_id.into()],
        ))
        .await?;
    rows.into_iter()
        .map(|row| {
            Ok(json!({
                "id": row.try_get::<Uuid>("", "id")?.to_string(),
                "name": row.try_get::<String>("", "name")?,
                "role_type": row.try_get::<String>("", "role_type")?,
                "appearance": row.try_get::<String>("", "appearance")?,
                "story_function": row.try_get::<String>("", "story_function")?,
                "needs_consistency": row.try_get::<bool>("", "needs_consistency")?,
            }))
        })
        .collect()
}

async fn pages_snapshot(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<Vec<JsonValue>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, page_number, title, body, illustration_prompt
            from storybook_pages
            where storybook_id = $1
            order by page_number asc, id asc
            "#,
            [storybook_id.into()],
        ))
        .await?;
    rows.into_iter()
        .map(|row| {
            Ok(json!({
                "id": row.try_get::<Uuid>("", "id")?.to_string(),
                "page_number": row.try_get::<i32>("", "page_number")?,
                "title": row.try_get::<String>("", "title")?,
                "body": row.try_get::<String>("", "body")?,
                "illustration_prompt": row.try_get::<String>("", "illustration_prompt")?,
            }))
        })
        .collect()
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
    ensure_storybook_ready_for_image_generation(db, workspace_id, storybook_id).await?;
    if let Some(active_job) = crate::repositories::generation_jobs::find_active_image_target_job(
        db,
        workspace_id,
        storybook_id,
        "storybook_page_image",
        "page_id",
        page_id,
        max_auto_attempts(),
    )
    .await?
    {
        return Ok(active_job);
    }
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
    ensure_storybook_ready_for_image_generation(db, workspace_id, storybook_id).await?;
    if let Some(active_job) = crate::repositories::generation_jobs::find_active_image_target_job(
        db,
        workspace_id,
        storybook_id,
        "storybook_cover_image",
        "cover_id",
        storybook_id,
        max_auto_attempts(),
    )
    .await?
    {
        return Ok(active_job);
    }
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
    ensure_storybook_ready_for_image_generation(db, workspace_id, storybook_id).await?;
    if let Some(active_job) = crate::repositories::generation_jobs::find_active_image_target_job(
        db,
        workspace_id,
        storybook_id,
        "storybook_role_reference_image",
        "role_id",
        role_id,
        max_auto_attempts(),
    )
    .await?
    {
        return Ok(active_job);
    }
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

pub async fn create_creation_storybook_image_job_records(
    db: &DatabaseConnection,
    job: &GenerationJob,
) -> Result<CreationImageEnqueueOutcome, DbErr> {
    if job.job_type != "creation_storybook_generate" || job.status != "succeeded" {
        return Ok(CreationImageEnqueueOutcome::default());
    }
    if !job
        .input_json
        .get("include_images")
        .and_then(JsonValue::as_bool)
        .unwrap_or(true)
    {
        return Ok(CreationImageEnqueueOutcome::default());
    }
    let storybook_id = job
        .storybook_id
        .ok_or_else(|| DbErr::Custom("共创绘本图片任务缺少 storybook_id".to_string()))?;
    let created_by = job
        .created_by
        .ok_or_else(|| DbErr::Custom("共创绘本图片任务缺少 created_by".to_string()))?;

    let mut outcome = CreationImageEnqueueOutcome::default();
    match create_cover_image_job_record(
        db,
        job.workspace_id,
        created_by,
        storybook_id,
        CreateImageTaskRequest {
            prompt: None,
            reference_role_ids: Vec::new(),
            reference_image_urls: Vec::new(),
            edit_instruction: None,
            image_mode: None,
            strength: None,
        },
    )
    .await
    {
        Ok(cover_job) => outcome.jobs.push(cover_job),
        Err(err) => {
            outcome.error = Some(format!("封面图片任务排队失败：{err}"));
            return Ok(outcome);
        }
    }

    let page_ids = match storybook_page_ids(db, storybook_id).await {
        Ok(page_ids) => page_ids,
        Err(err) => {
            outcome.error = Some(format!("读取分页失败：{err}"));
            return Ok(outcome);
        }
    };
    for page_id in page_ids {
        match create_page_image_job_record(
            db,
            job.workspace_id,
            created_by,
            storybook_id,
            page_id,
            CreateImageTaskRequest {
                prompt: None,
                reference_role_ids: Vec::new(),
                reference_image_urls: Vec::new(),
                edit_instruction: None,
                image_mode: None,
                strength: None,
            },
        )
        .await
        {
            Ok(page_job) => outcome.jobs.push(page_job),
            Err(err) => {
                outcome.error = Some(format!("部分分页图片任务排队失败：{err}"));
                break;
            }
        }
    }
    Ok(outcome)
}

#[derive(Default)]
pub struct CreationImageEnqueueOutcome {
    pub jobs: Vec<GenerationJob>,
    pub error: Option<String>,
}

pub async fn record_creation_image_enqueue_result(
    db: &DatabaseConnection,
    creation_job: &GenerationJob,
    image_jobs: &[GenerationJob],
    error: Option<&str>,
) -> Result<GenerationJob, DbErr> {
    let mut output = crate::repositories::generation_jobs::find_any_job(db, creation_job.id)
        .await?
        .output_json
        .unwrap_or_else(|| json!({}));
    let status = match (image_jobs.is_empty(), error.is_some()) {
        (_, false) => "queued",
        (true, true) => "failed",
        (false, true) => "partial_failed",
    };
    let image_job_ids = image_jobs
        .iter()
        .map(|job| job.id.to_string())
        .collect::<Vec<_>>();
    if let Some(object) = output.as_object_mut() {
        object.insert(
            "image_enqueue".to_string(),
            json!({
                "status": status,
                "job_count": image_jobs.len(),
                "job_ids": image_job_ids,
                "error": error,
            }),
        );
        if let Some(error) = error {
            append_quality_flag(object, format!("image_enqueue_failed:{error}"));
        }
    }
    let updated = crate::repositories::generation_jobs::update_succeeded_job_output(
        db,
        creation_job.id,
        output,
    )
    .await?;
    crate::repositories::storybook_creation_sessions::update_generation_summary_for_job(
        db,
        creation_job.workspace_id,
        creation_job.id,
        &creation_summary_after_image_enqueue(status, image_jobs.len(), error),
    )
    .await?;
    Ok(updated)
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
    if !matches!(job.status.as_str(), "queued" | "running" | "failed") {
        return Err(DbErr::Custom("generation_job_not_cancelable".to_string()));
    }

    let canceled =
        crate::repositories::generation_jobs::cancel_job(db, workspace_id, job_id).await?;
    // 图片任务在入队时已经把分页/角色和图片变体标记为生成中。
    // 取消也必须走同一套目标状态回写，否则界面会永久停留在“生成中”。
    propagate_failed_generation_job_state_with_message(db, &canceled, "生成任务已取消").await?;
    Ok(canceled)
}

pub async fn cancel_customization_run_jobs(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    run_id: Uuid,
) -> Result<usize, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id
            from generation_jobs
            where workspace_id = $1
              and job_type = 'storybook_customization_derive'
              and status in ('queued', 'running', 'failed')
              and input_json ->> 'customization_run_id' = $2
            "#,
            [workspace_id.into(), run_id.to_string().into()],
        ))
        .await?;
    let mut canceled = 0;
    for row in rows {
        let job_id: Uuid = row.try_get("", "id")?;
        cancel_generation_job(db, workspace_id, job_id).await?;
        canceled += 1;
    }
    Ok(canceled)
}

/// 清理工作区内全部失败的生成任务记录，返回清理条数。
pub async fn delete_failed_generation_jobs(
    db: &DatabaseConnection,
    workspace_id: Uuid,
) -> Result<u64, DbErr> {
    reset_failed_generation_targets(db, workspace_id).await?;
    let result = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "delete from generation_jobs where workspace_id = $1 and status = 'failed'",
            [workspace_id.into()],
        ))
        .await?;
    Ok(result.rows_affected())
}

fn writeback_job_requires_snapshot(job_type: &str) -> bool {
    matches!(
        job_type,
        "storybook_roles"
            | "storybook_pages"
            | "storybook_page_prompt"
            | "storybook_page_image"
            | "storybook_role_reference_image"
    )
}

fn ensure_retry_job_has_current_snapshot(job: &GenerationJob) -> Result<(), DbErr> {
    if writeback_job_requires_snapshot(&job.job_type)
        && job.input_json.get("target_snapshot").is_none()
    {
        return Err(DbErr::Custom(
            "历史任务缺少目标快照，无法安全重试；请重新发起生成".to_string(),
        ));
    }
    Ok(())
}

async fn reset_failed_generation_targets(
    db: &DatabaseConnection,
    workspace_id: Uuid,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_pages p
        set status = 'needs_regeneration'
        from generation_jobs j
        where j.workspace_id = $1
          and j.status = 'failed'
          and j.job_type = 'storybook_page_image'
          and j.storybook_id = p.storybook_id
          and j.input_json->>'page_id' = p.id::text
          and p.status = 'failed'
        "#,
        [workspace_id.into()],
    ))
    .await?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_roles r
        set reference_status = case
                when r.reference_image_url is null then 'not_started'
                else 'needs_regeneration'
            end
        from generation_jobs j
        where j.workspace_id = $1
          and j.status = 'failed'
          and j.job_type = 'storybook_role_reference_image'
          and j.storybook_id = r.storybook_id
          and j.input_json->>'role_id' = r.id::text
          and r.reference_status = 'failed'
        "#,
        [workspace_id.into()],
    ))
    .await?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        delete from storybook_image_variants v
        using generation_jobs j
        where j.workspace_id = $1
          and j.status = 'failed'
          and v.generation_job_id = j.id
          and v.status = 'failed'
          and not v.is_selected
        "#,
        [workspace_id.into()],
    ))
    .await?;
    Ok(())
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
    if running.job_type == "storybook_visual_reference" {
        crate::repositories::storybook_creation_assets::mark_visual_reference_generating_by_job(
            db,
            running.workspace_id,
            running.id,
        )
        .await?;
    }
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
    let updated = if job_type == "creation_storybook_generate" {
        let output_json = creation_storybook_generate_output(db, &job).await;
        complete_and_apply_running_job(db, job.id, output_json).await?
    } else if job_type == "storybook_customization_derive" {
        match customization_derive_output(db, &job).await {
            Ok(output_json) => complete_and_apply_running_job(db, job.id, output_json).await?,
            Err(err) => {
                fail_running_job(db, job.id, provider_name, &job_type, job.attempt_count, err)
                    .await?
            }
        }
    } else if is_image_job(&job_type) {
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
                match complete_and_apply_running_job(db, job.id, output_json).await {
                    Ok(completed) => {
                        mark_image_job_success_status(db, &completed, &job).await?;
                        completed
                    }
                    Err(err) => {
                        let writeback_error =
                            GenerationProviderError::new(format!("生成结果写回失败：{err}"));
                        fail_running_job(
                            db,
                            job.id,
                            provider_name,
                            &job_type,
                            job.attempt_count,
                            writeback_error,
                        )
                        .await?
                    }
                }
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
            Ok(output_json) => {
                match complete_and_apply_running_job(db, job.id, output_json).await {
                    Ok(completed) => completed,
                    Err(err) => {
                        let writeback_error =
                            GenerationProviderError::new(format!("生成结果写回失败：{err}"));
                        fail_running_job(
                            db,
                            job.id,
                            provider_name,
                            &job_type,
                            job.attempt_count,
                            writeback_error,
                        )
                        .await?
                    }
                }
            }
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
            Ok(output_json) => {
                match complete_and_apply_running_job(db, job.id, output_json).await {
                    Ok(completed) => completed,
                    Err(err) => {
                        let writeback_error =
                            GenerationProviderError::new(format!("生成结果写回失败：{err}"));
                        fail_running_job(
                            db,
                            job.id,
                            provider_name,
                            &job_type,
                            job.attempt_count,
                            writeback_error,
                        )
                        .await?
                    }
                }
            }
            Err(err) => {
                fail_running_job(db, job.id, provider_name, &job_type, job.attempt_count, err)
                    .await?
            }
        }
    };

    Ok(updated)
}

async fn customization_derive_output(
    db: &DatabaseConnection,
    job: &GenerationJob,
) -> Result<JsonValue, GenerationProviderError> {
    let parse_uuid = |key: &str| {
        job.input_json
            .get(key)
            .and_then(JsonValue::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(|| GenerationProviderError::new(format!("定制生成任务缺少有效 {key}")))
    };
    let run_id = parse_uuid("customization_run_id")?;
    let run_item_id = parse_uuid("customization_run_item_id")?;
    let source_storybook_id = parse_uuid("source_storybook_id")?;
    let child_id = parse_uuid("target_child_id")?;
    let actor_id = job
        .created_by
        .ok_or_else(|| GenerationProviderError::new("定制生成任务缺少创建者"))?;
    let intensity = job
        .input_json
        .get("intensity")
        .and_then(JsonValue::as_str)
        .unwrap_or("standard")
        .to_string();
    let primary_material = job
        .input_json
        .get("primary_material")
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    let customization_plan = job.input_json.get("customization_plan").cloned();

    ensure_customization_asset_references_active(db, job.workspace_id, customization_plan.as_ref())
        .await?;

    let claimed = crate::repositories::storybook_customization_runs::mark_item_running(
        db,
        job.workspace_id,
        run_item_id,
    )
    .await
    .map_err(|err| GenerationProviderError::new(err.to_string()))?;
    if !claimed {
        return Err(GenerationProviderError::new(
            "定制制作已取消或已结束，不再继续生成",
        ));
    }
    crate::repositories::storybook_customization_runs::finish_run(
        db,
        job.workspace_id,
        run_id,
        None,
    )
    .await
    .map_err(|err| GenerationProviderError::new(err.to_string()))?;

    match crate::repositories::storybooks::derive_custom(
        db,
        job.workspace_id,
        source_storybook_id,
        actor_id,
        crate::models::DeriveCustomRequest {
            child_id,
            intensity,
            primary_material,
            customization_plan,
        },
    )
    .await
    {
        Ok(book) => {
            let delivered = crate::repositories::storybook_customization_runs::mark_item_succeeded(
                db,
                job.workspace_id,
                run_item_id,
                book.id,
            )
            .await
            .map_err(|err| GenerationProviderError::new(err.to_string()))?;
            if !delivered {
                crate::repositories::storybooks::delete(db, job.workspace_id, book.id)
                    .await
                    .map_err(|err| GenerationProviderError::new(err.to_string()))?;
                let _ = crate::repositories::storybook_customization_runs::finish_run(
                    db,
                    job.workspace_id,
                    run_id,
                    None,
                )
                .await;
                return Err(GenerationProviderError::new(
                    "定制制作已取消，已丢弃未交付的结果",
                ));
            }
            crate::repositories::storybook_customization_runs::finish_run(
                db,
                job.workspace_id,
                run_id,
                None,
            )
            .await
            .map_err(|err| GenerationProviderError::new(err.to_string()))?;
            Ok(
                json!({ "storybook_id": book.id, "customization_run_id": run_id, "customization_run_item_id": run_item_id }),
            )
        }
        Err(err) => {
            let message = err.to_string();
            let _ = crate::repositories::storybook_customization_runs::mark_item_failed(
                db,
                job.workspace_id,
                run_item_id,
                &message,
            )
            .await;
            let _ = crate::repositories::storybook_customization_runs::finish_run(
                db,
                job.workspace_id,
                run_id,
                None,
            )
            .await;
            Err(GenerationProviderError::new(message))
        }
    }
}

async fn ensure_customization_asset_references_active(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    customization_plan: Option<&JsonValue>,
) -> Result<(), GenerationProviderError> {
    let ids = customization_asset_reference_ids(customization_plan);
    if ids.is_empty() {
        return Ok(());
    }
    let references =
        crate::repositories::storybook_creation_assets::list_by_ids(db, workspace_id, &ids)
            .await
            .map_err(|err| GenerationProviderError::new(err.to_string()))?;
    let all_confirmed = references.len() == ids.len()
        && references.iter().all(|reference| {
            reference.status == "ready"
                && matches!(
                    reference
                        .visual_reference
                        .as_ref()
                        .map(|visual| visual.status.as_str()),
                    Some("confirmed")
                )
        });
    if all_confirmed {
        Ok(())
    } else {
        Err(GenerationProviderError::new(
            "照片素材已被移除或尚未确认，请重新预览后再制作",
        ))
    }
}

fn customization_asset_reference_ids(customization_plan: Option<&JsonValue>) -> Vec<Uuid> {
    let mut ids = HashSet::new();
    let Some(plan) = customization_plan else {
        return Vec::new();
    };
    if let Some(values) = plan
        .get("confirmed_photo_reference_ids")
        .and_then(JsonValue::as_array)
    {
        ids.extend(
            values
                .iter()
                .filter_map(|value| value.as_str().and_then(|value| Uuid::parse_str(value).ok())),
        );
    }
    if let Some(references) = plan
        .get("confirmed_photo_references")
        .and_then(JsonValue::as_array)
    {
        ids.extend(references.iter().filter_map(|reference| {
            reference
                .get("asset_reference_id")
                .and_then(JsonValue::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
        }));
    }
    ids.into_iter().collect()
}

#[cfg(test)]
mod customization_derive_tests {
    use super::*;

    #[test]
    fn customization_asset_reference_ids_deduplicates_both_plan_shapes() {
        let first_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();
        let plan = json!({
            "confirmed_photo_reference_ids": [first_id.to_string()],
            "confirmed_photo_references": [
                { "asset_reference_id": first_id.to_string() },
                { "asset_reference_id": second_id.to_string() }
            ]
        });

        let ids = customization_asset_reference_ids(Some(&plan));

        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&first_id));
        assert!(ids.contains(&second_id));
    }
}

async fn complete_and_apply_running_job(
    db: &DatabaseConnection,
    job_id: Uuid,
    output_json: JsonValue,
) -> Result<GenerationJob, DbErr> {
    let existing_job = crate::repositories::generation_jobs::find_any_job(db, job_id).await?;
    ensure_job_output_is_current(db, &existing_job).await?;
    if is_image_job(&existing_job.job_type) {
        crate::repositories::generation_writeback::ensure_image_output_within_storage_quota(
            db,
            &existing_job,
            &output_json,
        )
        .await?;
    }
    let txn = db.begin().await?;
    let job = crate::repositories::generation_jobs::complete_running_job(&txn, job_id, output_json)
        .await?;
    record_generation_cost_log(&txn, &job).await?;
    if is_image_job(&job.job_type) && job.job_type != "storybook_visual_reference" {
        crate::repositories::storybook_image_variants::mark_job_variant_ready(&txn, &job).await?;
    }
    crate::repositories::generation_writeback::apply_completed_generation(&txn, &job).await?;
    txn.commit().await?;
    Ok(job)
}

async fn ensure_job_output_is_current(
    db: &DatabaseConnection,
    job: &GenerationJob,
) -> Result<(), DbErr> {
    if matches!(
        job.job_type.as_str(),
        "storybook_roles" | "storybook_pages" | "storybook_page_prompt"
    ) {
        ensure_text_target_snapshot_matches(db, job).await?;
    }
    ensure_target_snapshot_matches(db, job).await?;
    Ok(())
}

async fn ensure_target_snapshot_matches(
    db: &DatabaseConnection,
    job: &GenerationJob,
) -> Result<(), DbErr> {
    if job.job_type == "storybook_page_image" {
        let Some(storybook_id) = job.storybook_id else {
            return Ok(());
        };
        let Some(page_id) = page_id_from_job(job) else {
            return Ok(());
        };
        let Some(snapshot) = job.input_json.get("target_snapshot") else {
            return Err(DbErr::Custom(
                "插图任务缺少目标快照，无法安全写回".to_string(),
            ));
        };
        let current = db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                select title, body, illustration_prompt
                from storybook_pages
                where storybook_id = $1 and id = $2
                limit 1
                "#,
                [storybook_id.into(), page_id.into()],
            ))
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("page".to_string()))?;
        if snapshot.get("title").and_then(JsonValue::as_str)
            != Some(current.try_get::<String>("", "title")?.as_str())
            || snapshot.get("body").and_then(JsonValue::as_str)
                != Some(current.try_get::<String>("", "body")?.as_str())
            || snapshot
                .get("illustration_prompt")
                .and_then(JsonValue::as_str)
                != Some(
                    current
                        .try_get::<String>("", "illustration_prompt")?
                        .as_str(),
                )
        {
            return Err(DbErr::Custom(
                "页面内容已在图片生成期间更新，请重新生成插图".to_string(),
            ));
        }
    } else if job.job_type == "storybook_role_reference_image" {
        let Some(storybook_id) = job.storybook_id else {
            return Ok(());
        };
        let Some(role_id) = role_id_from_job(job) else {
            return Ok(());
        };
        let Some(snapshot) = job.input_json.get("target_snapshot") else {
            return Err(DbErr::Custom(
                "角色参考图任务缺少目标快照，无法安全写回".to_string(),
            ));
        };
        let current = db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                select name, role_type, appearance, coalesce(story_function, '') as story_function, needs_consistency
                from storybook_roles
                where storybook_id = $1 and id = $2
                limit 1
                "#,
                [storybook_id.into(), role_id.into()],
            ))
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("role".to_string()))?;
        if snapshot.get("name").and_then(JsonValue::as_str)
            != Some(current.try_get::<String>("", "name")?.as_str())
            || snapshot.get("role_type").and_then(JsonValue::as_str)
                != Some(current.try_get::<String>("", "role_type")?.as_str())
            || snapshot.get("appearance").and_then(JsonValue::as_str)
                != Some(current.try_get::<String>("", "appearance")?.as_str())
            || snapshot.get("story_function").and_then(JsonValue::as_str)
                != Some(current.try_get::<String>("", "story_function")?.as_str())
            || snapshot
                .get("needs_consistency")
                .and_then(JsonValue::as_bool)
                != Some(current.try_get::<bool>("", "needs_consistency")?)
        {
            return Err(DbErr::Custom(
                "角色设定已在参考图生成期间更新，请重新生成参考图".to_string(),
            ));
        }
    }
    Ok(())
}

async fn ensure_text_target_snapshot_matches(
    db: &DatabaseConnection,
    job: &GenerationJob,
) -> Result<(), DbErr> {
    let Some(storybook_id) = job.storybook_id else {
        return Ok(());
    };
    let Some(snapshot) = job.input_json.get("target_snapshot") else {
        return Err(DbErr::Custom(
            "文本生成任务缺少目标快照，无法安全写回".to_string(),
        ));
    };
    if job.job_type == "storybook_roles" {
        let current = json!({
            "roles": roles_snapshot(db, storybook_id).await?,
        });
        if snapshot != &current {
            return Err(DbErr::Custom(
                "角色设定已在生成期间更新，请重新发起角色生成".to_string(),
            ));
        }
    } else if job.job_type == "storybook_pages" {
        let current = json!({
            "roles": roles_snapshot(db, storybook_id).await?,
            "pages": pages_snapshot(db, storybook_id).await?,
        });
        if snapshot != &current {
            return Err(DbErr::Custom(
                "绘本分页或角色已在生成期间更新，请重新发起分页生成".to_string(),
            ));
        }
    } else if job.job_type == "storybook_page_prompt" {
        let page_id = job
            .input_json
            .get("page_id")
            .and_then(|value| value.as_str())
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(|| DbErr::Custom("插图描述重写任务缺少 page_id".to_string()))?;
        let current = json!({
            "page": storybook_page_for_prompt(db, storybook_id, page_id).await?,
            "roles": roles_snapshot(db, storybook_id).await?,
        });
        if snapshot != &current {
            return Err(DbErr::Custom(
                "页面或角色已在插图描述重写期间更新，请重新发起重写".to_string(),
            ));
        }
    }
    Ok(())
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
    propagate_failed_generation_job_state_with_message(db, &job, &safe_message).await?;
    record_generation_cost_log(db, &job).await?;
    Ok(job)
}

async fn propagate_failed_generation_job_state(
    db: &DatabaseConnection,
    job: &GenerationJob,
) -> Result<(), DbErr> {
    let message = job
        .last_error
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("生成任务失败");
    propagate_failed_generation_job_state_with_message(db, job, message).await
}

async fn propagate_failed_generation_job_state_with_message(
    db: &DatabaseConnection,
    job: &GenerationJob,
    message: &str,
) -> Result<(), DbErr> {
    if job.job_type == "storybook_role_reference_image" {
        crate::repositories::storybook_image_variants::mark_job_variant_failed(db, job, message)
            .await?;
        if let (Some(storybook_id), Some(role_id)) = (job.storybook_id, role_id_from_job(job)) {
            mark_role_reference_status_for_job(db, storybook_id, role_id, job.id, "failed").await?;
        }
    } else if job.job_type == "storybook_visual_reference" {
        crate::repositories::storybook_creation_assets::mark_visual_reference_failed_by_job(
            db,
            job.workspace_id,
            job.id,
            message.to_string(),
        )
        .await?;
    } else if job.job_type == "storybook_page_image" {
        crate::repositories::storybook_image_variants::mark_job_variant_failed(db, job, message)
            .await?;
        if let (Some(storybook_id), Some(page_id)) = (job.storybook_id, page_id_from_job(job)) {
            mark_page_image_status_for_job(db, storybook_id, page_id, job.id, "failed").await?;
        }
    } else if job.job_type == "storybook_cover_image" {
        crate::repositories::storybook_image_variants::mark_job_variant_failed(db, job, message)
            .await?;
    } else if job.job_type == "creation_storybook_generate" {
        if let Some(session_id) = job
            .input_json
            .get("creation_session_id")
            .and_then(|value| value.as_str())
            .and_then(|value| Uuid::parse_str(value).ok())
        {
            crate::repositories::storybook_creation_sessions::mark_storybook_job_failed(
                db,
                job.workspace_id,
                session_id,
                job.id,
            )
            .await?;
        }
    } else if job.job_type == "storybook_customization_derive" {
        let run_id = job
            .input_json
            .get("customization_run_id")
            .and_then(JsonValue::as_str)
            .and_then(|value| Uuid::parse_str(value).ok());
        let run_item_id = job
            .input_json
            .get("customization_run_item_id")
            .and_then(JsonValue::as_str)
            .and_then(|value| Uuid::parse_str(value).ok());
        if let Some(item_id) = run_item_id {
            if job.status == "canceled" {
                crate::repositories::storybook_customization_runs::mark_item_canceled(
                    db,
                    job.workspace_id,
                    item_id,
                )
                .await?;
            } else {
                crate::repositories::storybook_customization_runs::mark_item_failed(
                    db,
                    job.workspace_id,
                    item_id,
                    message,
                )
                .await?;
            }
        }
        if let Some(run_id) = run_id {
            crate::repositories::storybook_customization_runs::finish_run(
                db,
                job.workspace_id,
                run_id,
                None,
            )
            .await?;
        }
    }
    Ok(())
}

fn max_auto_attempts() -> i32 {
    std::env::var("KINDLEAF_GENERATION_MAX_AUTO_ATTEMPTS")
        .ok()
        .and_then(|value| value.trim().parse::<i32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MAX_AUTO_ATTEMPTS)
}

async fn creation_storybook_generate_output(
    db: &DatabaseConnection,
    job: &GenerationJob,
) -> JsonValue {
    if ConfiguredGenerationProvider::ready_for_text() {
        let provider = ConfiguredGenerationProvider::from_env();
        let provider_input = match creation_storybook_pages_input(db, job).await {
            Ok(input) => input,
            Err(err) => fallback_creation_pages_input(job, Some(err.to_string())),
        };
        match provider
            .generate(GenerationRequest {
                job_type: "storybook_pages",
                input: &provider_input,
            })
            .await
        {
            Ok(mut output) => {
                if let Some(object) = output.as_object_mut() {
                    object.insert(
                        "creation_session_id".to_string(),
                        job.input_json
                            .get("creation_session_id")
                            .cloned()
                            .unwrap_or(JsonValue::Null),
                    );
                    object.insert(
                        "creation_generation_source".to_string(),
                        JsonValue::String("ai".to_string()),
                    );
                    object.insert(
                        "include_images".to_string(),
                        job.input_json
                            .get("include_images")
                            .cloned()
                            .unwrap_or(JsonValue::Bool(true)),
                    );
                }
                annotate_locked_material_usage(&mut output, job);
                return output;
            }
            Err(err) => {
                return fallback_creation_storybook_generate_output(job, Some(err.safe_message()));
            }
        }
    }
    fallback_creation_storybook_generate_output(
        job,
        Some("real_text_provider_not_ready".to_string()),
    )
}

fn fallback_creation_storybook_generate_output(
    job: &GenerationJob,
    reason: Option<String>,
) -> JsonValue {
    let mut output = json!({
        "schema_version": "creation.provider.v1",
        "provider": "system",
        "mode": "creation_storybook_generate",
        "creation_generation_source": "fallback",
        "quality_flags": reason.into_iter().collect::<Vec<_>>(),
        "creation_session_id": job.input_json.get("creation_session_id").cloned().unwrap_or(JsonValue::Null),
        "storybook_id": job.storybook_id.map(|id| id.to_string()),
        "materials": job.input_json.get("materials").cloned().unwrap_or_else(|| json!([])),
        "selected_direction": job.input_json.get("selected_direction").cloned().unwrap_or_else(|| json!({})),
        "outline": job.input_json.get("outline").cloned().unwrap_or_else(|| json!({})),
        "visual_preferences": job.input_json.get("visual_preferences").cloned().unwrap_or_else(|| json!({})),
        "pages": fallback_creation_pages(job),
        "message": "共创绘本草稿已生成"
    });
    annotate_locked_material_usage(&mut output, job);
    output
}

fn annotate_locked_material_usage(output: &mut JsonValue, job: &GenerationJob) {
    let locked_labels = job
        .input_json
        .get("materials")
        .and_then(JsonValue::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| {
                    item.get("locked")
                        .and_then(JsonValue::as_bool)
                        .unwrap_or(false)
                })
                .filter_map(|item| item.get("label").and_then(JsonValue::as_str))
                .map(str::trim)
                .filter(|label| !label.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if locked_labels.is_empty() {
        return;
    }
    let content = output
        .get("pages")
        .and_then(JsonValue::as_array)
        .map(|pages| {
            pages
                .iter()
                .map(|page| {
                    format!(
                        "{} {} {}",
                        page.get("title").and_then(JsonValue::as_str).unwrap_or(""),
                        page.get("body").and_then(JsonValue::as_str).unwrap_or(""),
                        page.get("illustration_prompt")
                            .and_then(JsonValue::as_str)
                            .unwrap_or("")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let missing = locked_labels
        .into_iter()
        .filter(|label| !content.contains(label))
        .collect::<Vec<_>>();
    if let Some(object) = output.as_object_mut() {
        object.insert(
            "locked_material_usage".to_string(),
            json!({
                "status": if missing.is_empty() { "satisfied" } else { "missing" },
                "missing_labels": missing,
            }),
        );
        let has_missing = object
            .get("locked_material_usage")
            .and_then(|value| value.get("status"))
            .and_then(JsonValue::as_str)
            == Some("missing");
        if has_missing {
            append_quality_flag(
                object,
                "locked_material_missing:final_storybook".to_string(),
            );
            object.insert(
                "quality_notice".to_string(),
                JsonValue::String(
                    "草稿已生成，但有专属素材没有明显出现在成品里，建议进入验收页确认。"
                        .to_string(),
                ),
            );
        }
    }
}

async fn creation_storybook_pages_input(
    db: &DatabaseConnection,
    job: &GenerationJob,
) -> Result<JsonValue, DbErr> {
    let storybook_id = job
        .storybook_id
        .ok_or_else(|| DbErr::Custom("共创绘本生成任务缺少 storybook_id".to_string()))?;
    let roles = confirmed_roles_for_storybook(db, storybook_id).await?;
    let visual_preferences = job
        .input_json
        .get("visual_preferences")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let selected_direction = job
        .input_json
        .get("selected_direction")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let title = selected_direction
        .get("title")
        .and_then(JsonValue::as_str)
        .unwrap_or("专属故事")
        .to_string();
    Ok(json!({
        "title": title,
        "theme": job.input_json.get("quick_idea").cloned().unwrap_or(JsonValue::Null),
        "style": visual_preferences.get("style").cloned().unwrap_or_else(|| json!("watercolor")),
        "page_count": requested_creation_page_count(job),
        "plan": {
            "summary": selected_direction.get("summary").cloned().unwrap_or(JsonValue::Null),
            "outline": job.input_json.get("outline").cloned().unwrap_or_else(|| json!({})),
            "personal_hook": selected_direction.get("personal_hook").cloned().unwrap_or(JsonValue::Null)
        },
        "confirmed_roles": roles,
        "materials": job.input_json.get("materials").cloned().unwrap_or_else(|| json!([])),
        "asset_references": job.input_json.get("asset_references").cloned().unwrap_or_else(|| json!([])),
        "character_photo_references": job.input_json.get("character_photo_references").cloned().unwrap_or_else(|| json!([])),
        "prop_photo_references": job.input_json.get("prop_photo_references").cloned().unwrap_or_else(|| json!([])),
        "scene_photo_references": job.input_json.get("scene_photo_references").cloned().unwrap_or_else(|| json!([])),
        "page_evidence": job.input_json.get("page_evidence").cloned().unwrap_or_else(|| json!([])),
        "visual_preferences": visual_preferences,
        "creation_context": {
            "quick_idea": job.input_json.get("quick_idea").cloned().unwrap_or(JsonValue::Null),
            "understanding": job.input_json.get("understanding").cloned().unwrap_or(JsonValue::Null),
            "materials": job.input_json.get("materials").cloned().unwrap_or_else(|| json!([])),
            "asset_references": job.input_json.get("asset_references").cloned().unwrap_or_else(|| json!([])),
            "confirmed_photo_references": job.input_json.get("asset_references").cloned().unwrap_or_else(|| json!([])),
            "character_photo_references": job.input_json.get("character_photo_references").cloned().unwrap_or_else(|| json!([])),
            "prop_photo_references": job.input_json.get("prop_photo_references").cloned().unwrap_or_else(|| json!([])),
            "scene_photo_references": job.input_json.get("scene_photo_references").cloned().unwrap_or_else(|| json!([])),
            "page_evidence": job.input_json.get("page_evidence").cloned().unwrap_or_else(|| json!([])),
            "selected_direction": selected_direction,
            "outline": job.input_json.get("outline").cloned().unwrap_or_else(|| json!({})),
            "visual_preferences": visual_preferences,
            "product_goal": "让用户感觉这本绘本是我和 AI 一起做出来的，而且里面真的有我的故事"
        }
    }))
}

fn fallback_creation_pages_input(job: &GenerationJob, reason: Option<String>) -> JsonValue {
    json!({
        "title": job.input_json
            .get("selected_direction")
            .and_then(|value| value.get("title"))
            .and_then(JsonValue::as_str)
            .unwrap_or("专属故事"),
        "quality_flags": reason.into_iter().collect::<Vec<_>>(),
        "outline": job.input_json.get("outline").cloned().unwrap_or_else(|| json!({})),
    })
}

fn fallback_creation_pages(job: &GenerationJob) -> Vec<JsonValue> {
    let target_count = requested_creation_page_count(job) as usize;
    let mut pages = job.input_json
        .get("outline")
        .and_then(|value| value.get("pages"))
        .and_then(JsonValue::as_array)
        .map(|pages| {
            pages
                .iter()
                .enumerate()
                .map(|(index, page)| {
                    let page_number = page
                        .get("page_number")
                        .and_then(JsonValue::as_u64)
                        .unwrap_or((index + 1) as u64);
                    let summary = page
                        .get("summary")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("这一页继续推进孩子的专属故事。");
                    json!({
                        "page_number": page_number,
                        "title": fallback_page_title(summary, page_number),
                        "body": format!("{summary} 孩子在故事里被看见，也得到一次可以尝试的小办法。"),
                        "illustration_prompt": format!("儿童绘本插图，中景，{summary}，画面清楚呈现角色动作、表情和关键素材，不出现文字。"),
                        "status": "needs_regeneration"
                    })
                })
                .collect()
        })
        .unwrap_or_else(|| {
            vec![json!({
                "page_number": 1,
                "title": "专属故事开始了",
                "body": "故事从一个真实的小瞬间开始，孩子在陪伴中慢慢尝试。",
                "illustration_prompt": "儿童绘本插图，中景，孩子和大人在熟悉场景中互动，画面温暖清楚，不出现文字。",
                "status": "needs_regeneration"
            })]
        });
    while pages.len() < target_count {
        let page_number = (pages.len() + 1) as u64;
        let summary = "这一页继续推进孩子的专属故事。";
        pages.push(json!({
            "page_number": page_number,
            "title": fallback_page_title(summary, page_number),
            "body": format!("{summary} 孩子在故事里被看见，也得到一次可以尝试的小办法。"),
            "illustration_prompt": format!("儿童绘本插图，中景，{summary}，画面清楚呈现角色动作、表情和关键素材，不出现文字。"),
            "status": "needs_regeneration"
        }));
    }
    pages.truncate(target_count);
    pages
}

fn requested_creation_page_count(job: &GenerationJob) -> u64 {
    let explicit = job
        .input_json
        .get("page_count")
        .and_then(|value| match value {
            JsonValue::Number(number) => number.as_u64(),
            JsonValue::String(text) => text.trim().parse::<u64>().ok(),
            _ => None,
        });
    let outline_count = job
        .input_json
        .get("outline")
        .and_then(|value| value.get("pages"))
        .and_then(JsonValue::as_array)
        .map(|pages| pages.len() as u64);
    explicit.or(outline_count).unwrap_or(6).clamp(4, 32)
}

fn fallback_page_title(summary: &str, page_number: u64) -> String {
    let title = summary
        .split(['，', '。', ',', '.'])
        .next()
        .unwrap_or("专属故事")
        .chars()
        .take(12)
        .collect::<String>();
    if title.trim().is_empty() {
        format!("第{page_number}页")
    } else {
        title
    }
}

fn append_quality_flag(object: &mut serde_json::Map<String, JsonValue>, flag: String) {
    let entry = object
        .entry("quality_flags".to_string())
        .or_insert_with(|| json!([]));
    if let Some(flags) = entry.as_array_mut() {
        if !flags
            .iter()
            .any(|item| item.as_str() == Some(flag.as_str()))
        {
            flags.push(JsonValue::String(flag));
        }
    } else {
        *entry = json!([flag]);
    }
}

fn creation_summary_after_image_enqueue(
    status: &str,
    image_job_count: usize,
    error: Option<&str>,
) -> CreationGenerationSummary {
    let quality_notice = match status {
        "failed" => {
            Some("文字草稿已生成，但图片暂时没有排上队，你可以稍后补生成图片。".to_string())
        }
        "partial_failed" => {
            Some("文字草稿已生成，部分图片已经开始生成，少量图片需要稍后补生成。".to_string())
        }
        _ => None,
    };
    let mut recoverable_actions = vec!["open_review_workspace".to_string()];
    if error.is_some() || matches!(status, "failed" | "partial_failed") {
        recoverable_actions.push("retry_failed_images".to_string());
    }
    CreationGenerationSummary {
        text_generation_status: "succeeded".to_string(),
        image_generation_status: if image_job_count == 0 && status == "queued" {
            "skipped".to_string()
        } else {
            status.to_string()
        },
        quality_notice,
        recoverable_actions,
    }
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
        mark_page_image_status_for_job(db, storybook_id, page_id, completed.id, "ready").await?;
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

async fn mark_page_image_status_for_job(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    page_id: Uuid,
    job_id: Uuid,
    status: &str,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_pages p
        set status = $4
        where p.storybook_id = $1 and p.id = $2
          and exists (
            select 1
            from storybook_image_variants v
            where v.storybook_id = p.storybook_id
              and v.target_type = 'page_illustration'
              and v.target_id = p.id
              and v.generation_job_id = $3
              and (
                v.is_selected
                or not exists (
                  select 1
                  from storybook_image_variants newer
                  where newer.storybook_id = p.storybook_id
                    and newer.target_type = v.target_type
                    and newer.target_id = v.target_id
                    and newer.created_at > v.created_at
                )
              )
          )
        "#,
        vec![
            storybook_id.into(),
            page_id.into(),
            job_id.into(),
            status.into(),
        ],
    ))
    .await?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "update storybooks set updated_at = now() where id = $1",
        [storybook_id.into()],
    ))
    .await?;
    Ok(())
}

async fn mark_role_reference_status_for_job(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    role_id: Uuid,
    job_id: Uuid,
    status: &str,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_roles r
        set reference_status = $4
        where r.storybook_id = $1 and r.id = $2
          and exists (
            select 1
            from storybook_image_variants v
            where v.storybook_id = r.storybook_id
              and v.target_type = 'role_reference'
              and v.target_id = r.id
              and v.generation_job_id = $3
              and (
                v.is_selected
                or not exists (
                  select 1
                  from storybook_image_variants newer
                  where newer.storybook_id = r.storybook_id
                    and newer.target_type = v.target_type
                    and newer.target_id = v.target_id
                    and newer.created_at > v.created_at
                )
              )
          )
        "#,
        vec![
            storybook_id.into(),
            role_id.into(),
            job_id.into(),
            status.into(),
        ],
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

async fn storybook_page_ids(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<Vec<Uuid>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id
            from storybook_pages
            where storybook_id = $1
            order by page_number asc, id asc
            "#,
            [storybook_id.into()],
        ))
        .await?;
    rows.into_iter().map(|row| row.try_get("", "id")).collect()
}

async fn ensure_storybook_ready_for_image_generation(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<(), DbErr> {
    let status = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select status
            from storybooks
            where workspace_id = $1 and id = $2
            limit 1
            "#,
            [workspace_id.into(), storybook_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("storybook".to_string()))?
        .try_get::<String>("", "status")?;
    if !IMAGE_READY_STATUSES.contains(&status.as_str()) {
        return Err(DbErr::Custom(format!(
            "当前绘本状态为 {status}，需进入编辑或插图阶段后才能生成图片"
        )));
    }
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
