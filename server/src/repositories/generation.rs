use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

use crate::models::{
    CreateGenerationJobRequest, CreateImageTaskRequest, GenerationCostListQuery, GenerationCostLog,
    GenerationCostReport, GenerationCostSummary, GenerationJob, PaginationMeta,
};
use crate::services::generation_provider::{
    ConfiguredGenerationProvider, GenerationProviderError, GenerationRequest, ImageGenerationMode,
    ImageGenerationRequest, ImageReference,
};

const ALLOWED_JOB_TYPES: &[&str] = &[
    "storybook_plan",
    "storybook_roles",
    "storybook_pages",
    "storybook_page_image",
    "storybook_role_reference_image",
    "customization_plan",
];
const INLINE_WORKER_ID: &str = "inline-mock-executor";

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

    move_to_running(db, job.id, "failed", INLINE_WORKER_ID).await?;

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
            })
            .await
        {
            Ok(output_json) => complete_and_apply_running_job(db, job.id, output_json).await?,
            Err(err) => fail_running_job(db, job.id, provider_name, &job.job_type, err).await?,
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
            Err(err) => fail_running_job(db, job.id, provider_name, &job.job_type, err).await?,
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
    let mut processed = requeue_stale_jobs_scoped(db, workspace_id, age_minutes).await?;
    let limit = limit.max(1);
    let worker_id = "kindleaf-scheduler";

    for _ in 0..limit {
        let Some(job) = claim_next_ready_job_scoped(db, worker_id, workspace_id).await? else {
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
    payload: CreateGenerationJobRequest,
) -> Result<GenerationJob, DbErr> {
    ensure_generation_budget_available(db, Some(workspace_id)).await?;
    let job_type = payload.job_type.trim();
    if !ALLOWED_JOB_TYPES.contains(&job_type) {
        return Err(DbErr::Custom(format!("不支持的生成任务类型：{job_type}")));
    }
    if let Some(storybook_id) = payload.storybook_id {
        ensure_storybook_in_workspace(db, workspace_id, storybook_id).await?;
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

    enqueue_job(
        db,
        workspace_id,
        payload.storybook_id,
        job_type,
        payload.input_json,
    )
    .await
}

pub async fn create_page_image_job_record(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    page_id: Uuid,
    payload: CreateImageTaskRequest,
) -> Result<GenerationJob, DbErr> {
    ensure_generation_budget_available(db, Some(workspace_id)).await?;
    let page_prompt = page_prompt(db, workspace_id, storybook_id, page_id).await?;
    let prompt = payload
        .prompt
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(page_prompt);
    let reference_images = page_image_reference_images(db, storybook_id, &payload).await?;
    let image_mode =
        normalize_image_mode(payload.image_mode.as_deref(), !reference_images.is_empty());
    let edit_instruction = clean_optional_text(payload.edit_instruction);
    let strength = payload.strength.map(|value| value.clamp(0.0, 1.0));
    let input_json = json!({
        "page_id": page_id,
        "prompt": prompt,
        "mode": "storybook_page_image",
        "image_mode": image_mode.as_str(),
        "reference_role_ids": payload.reference_role_ids,
        "reference_images": reference_images,
        "edit_instruction": edit_instruction,
        "strength": strength
    });
    enqueue_job(
        db,
        workspace_id,
        Some(storybook_id),
        "storybook_page_image",
        input_json,
    )
    .await
}

pub async fn create_role_reference_image_job_record(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    role_id: Uuid,
    payload: CreateImageTaskRequest,
) -> Result<GenerationJob, DbErr> {
    ensure_generation_budget_available(db, Some(workspace_id)).await?;
    let role_prompt = role_reference_prompt(db, workspace_id, storybook_id, role_id).await?;
    let prompt = payload
        .prompt
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(role_prompt);
    let reference_images = clean_reference_image_urls(&payload.reference_image_urls)
        .into_iter()
        .map(|url| ImageReference {
            url,
            source: "direct".to_string(),
            role_id: None,
            label: None,
        })
        .collect::<Vec<_>>();
    let image_mode =
        normalize_image_mode(payload.image_mode.as_deref(), !reference_images.is_empty());
    let input_json = json!({
        "role_id": role_id,
        "prompt": prompt,
        "mode": "storybook_role_reference_image",
        "image_mode": image_mode.as_str(),
        "reference_images": reference_images,
        "edit_instruction": clean_optional_text(payload.edit_instruction),
        "strength": payload.strength.map(|value| value.clamp(0.0, 1.0))
    });
    enqueue_job(
        db,
        workspace_id,
        Some(storybook_id),
        "storybook_role_reference_image",
        input_json,
    )
    .await
}

async fn page_image_reference_images(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    payload: &CreateImageTaskRequest,
) -> Result<Vec<ImageReference>, DbErr> {
    let mut references = Vec::new();
    let selected_roles = payload
        .reference_role_ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();

    if !payload.reference_role_ids.is_empty() {
        let rows = db
            .query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                select id, name, reference_image_url
                from storybook_roles
                where storybook_id = $1
                  and reference_image_url is not null
                "#,
                [storybook_id.into()],
            ))
            .await?;
        for row in rows {
            let role_id: Uuid = row.try_get("", "id")?;
            if !selected_roles.contains(&role_id) {
                continue;
            }
            let Some(url) =
                clean_optional_text(row.try_get::<Option<String>>("", "reference_image_url")?)
            else {
                continue;
            };
            references.push(ImageReference {
                url,
                source: "storybook_role".to_string(),
                role_id: Some(role_id.to_string()),
                label: row.try_get("", "name").ok(),
            });
        }
    }

    for url in clean_reference_image_urls(&payload.reference_image_urls) {
        if references.iter().any(|item| item.url == url) {
            continue;
        }
        references.push(ImageReference {
            url,
            source: "direct".to_string(),
            role_id: None,
            label: None,
        });
    }

    Ok(references)
}

struct PageImageRequestInput {
    prompt: String,
    reference_images: Vec<ImageReference>,
    edit_instruction: Option<String>,
    image_mode: ImageGenerationMode,
    strength: Option<f32>,
}

struct ImageJobTarget {
    target_id: String,
    target_type: &'static str,
}

fn is_image_job(job_type: &str) -> bool {
    matches!(
        job_type,
        "storybook_page_image" | "storybook_role_reference_image"
    )
}

fn image_target_from_job(job: &GenerationJob) -> Result<ImageJobTarget, DbErr> {
    if job.job_type == "storybook_role_reference_image" {
        let role_id = job
            .input_json
            .get("role_id")
            .and_then(|value| value.as_str())
            .ok_or_else(|| DbErr::Custom("角色参考图任务缺少 role_id，无法执行".to_string()))?;
        Ok(ImageJobTarget {
            target_id: role_id.to_string(),
            target_type: "role",
        })
    } else {
        let page_id = job
            .input_json
            .get("page_id")
            .and_then(|value| value.as_str())
            .ok_or_else(|| DbErr::Custom("插图任务缺少 page_id，无法执行".to_string()))?;
        Ok(ImageJobTarget {
            target_id: page_id.to_string(),
            target_type: "page",
        })
    }
}

fn image_request_from_job(job: &GenerationJob) -> Result<PageImageRequestInput, DbErr> {
    let prompt = job
        .input_json
        .get("prompt")
        .and_then(|value| value.as_str())
        .ok_or_else(|| DbErr::Custom("插图任务缺少 prompt，无法执行".to_string()))?
        .to_string();
    let reference_images = job
        .input_json
        .get("reference_images")
        .and_then(|value| serde_json::from_value::<Vec<ImageReference>>(value.clone()).ok())
        .unwrap_or_default();
    let image_mode = normalize_image_mode(
        job.input_json
            .get("image_mode")
            .and_then(|value| value.as_str()),
        !reference_images.is_empty(),
    );
    let edit_instruction = job
        .input_json
        .get("edit_instruction")
        .and_then(|value| value.as_str())
        .and_then(|value| clean_optional_text(Some(value.to_string())));
    let strength = job
        .input_json
        .get("strength")
        .and_then(|value| value.as_f64())
        .map(|value| (value as f32).clamp(0.0, 1.0));

    Ok(PageImageRequestInput {
        prompt,
        reference_images,
        edit_instruction,
        image_mode,
        strength,
    })
}

fn normalize_image_mode(value: Option<&str>, has_reference_images: bool) -> ImageGenerationMode {
    match value.map(str::trim) {
        Some("edit_image") => ImageGenerationMode::EditImage,
        Some("reference_image") => ImageGenerationMode::ReferenceImage,
        _ if has_reference_images => ImageGenerationMode::ReferenceImage,
        _ => ImageGenerationMode::TextToImage,
    }
}

fn clean_reference_image_urls(urls: &[String]) -> Vec<String> {
    let mut cleaned = Vec::new();
    for url in urls {
        let Some(url) = clean_optional_text(Some(url.clone())) else {
            continue;
        };
        if cleaned.iter().any(|item| item == &url) {
            continue;
        }
        cleaned.push(url);
    }
    cleaned
}

fn clean_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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

    let output_json = json!({
        "schema_version": "generation.canceled.v1",
        "message": "生成任务已取消"
    });
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update generation_jobs
            set status = 'canceled',
                output_json = $3,
                last_error = null,
                next_run_at = null,
                locked_by = null,
                locked_at = null,
                finished_at = now()
            where workspace_id = $1
              and id = $2
              and status in ('queued', 'failed')
            returning
              id, workspace_id, storybook_id, job_type, status, input_json, output_json,
              attempt_count, last_error, next_run_at, locked_by, locked_at, created_at, finished_at
            "#,
            [workspace_id.into(), job_id.into(), output_json.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::Custom("generation_job_not_cancelable".to_string()))?;
    job_from_row(row)
}

async fn requeue_stale_jobs_scoped(
    db: &DatabaseConnection,
    workspace_id: Option<Uuid>,
    age_minutes: i64,
) -> Result<u64, DbErr> {
    let age_minutes = age_minutes.max(1);
    let row = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update generation_jobs
            set status = 'queued',
                last_error = coalesce(last_error, '任务已超时，由调度器重新入队'),
                locked_by = null,
                locked_at = null,
                next_run_at = null
            where status = 'running'
              and locked_at is not null
              and locked_at < now() - ($1::text)::interval
              and ($2::uuid is null or workspace_id = $2)
            "#,
            [format!("{age_minutes} minutes").into(), workspace_id.into()],
        ))
        .await?;
    Ok(row.rows_affected())
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

    let running = move_to_running(db, job.id, job.status.as_str(), INLINE_WORKER_ID).await?;
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
            })
            .await
        {
            Ok(output_json) => {
                let completed = complete_and_apply_running_job(db, job.id, output_json).await?;
                if completed.status == "succeeded"
                    && job_type == "storybook_page_image"
                    && let Some(storybook_id) = completed.storybook_id
                {
                    if let Some(page_id) = job
                        .input_json
                        .get("page_id")
                        .and_then(|value| value.as_str())
                        .and_then(|value| Uuid::parse_str(value).ok())
                    {
                        db.execute(Statement::from_sql_and_values(
                            DbBackend::Postgres,
                            r#"
                            update storybook_pages
                            set status = 'ready'
                            where id = $1 and storybook_id = $2
                            "#,
                            [page_id.into(), storybook_id.into()],
                        ))
                        .await?;
                    }
                }
                completed
            }
            Err(err) => fail_running_job(db, job.id, provider_name, &job_type, err).await?,
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
            Err(err) => fail_running_job(db, job.id, provider_name, &job_type, err).await?,
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
            Err(err) => fail_running_job(db, job.id, provider_name, &job_type, err).await?,
        }
    };

    Ok(updated)
}

async fn enqueue_job(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Option<Uuid>,
    job_type: &str,
    input_json: JsonValue,
) -> Result<GenerationJob, DbErr> {
    let id = Uuid::new_v4();
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into generation_jobs
              (id, workspace_id, storybook_id, job_type, status, input_json, created_at)
            values ($1, $2, $3, $4, 'queued', $5, now())
            returning
              id, workspace_id, storybook_id, job_type, status, input_json, output_json,
              attempt_count, last_error, next_run_at, locked_by, locked_at, created_at, finished_at
            "#,
            [
                id.into(),
                workspace_id.into(),
                storybook_id.into(),
                job_type.into(),
                input_json.into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("generation_job".to_string()))?;

    job_from_row(row)
}

async fn move_to_running(
    db: &DatabaseConnection,
    job_id: Uuid,
    from_status: &str,
    worker_id: &str,
) -> Result<GenerationJob, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
        update generation_jobs
        set status = 'running',
            attempt_count = attempt_count + 1,
            last_error = null,
            next_run_at = null,
            locked_by = $3,
            locked_at = now(),
            finished_at = null
        where id = $1 and status = $2
        returning
          id, workspace_id, storybook_id, job_type, status, input_json, output_json,
          attempt_count, last_error, next_run_at, locked_by, locked_at, created_at, finished_at
        "#,
            [job_id.into(), from_status.into(), worker_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::Custom("任务状态已变化，无法执行".to_string()))?;

    job_from_row(row)
}

async fn complete_running_job(
    db: &DatabaseConnection,
    job_id: Uuid,
    output_json: JsonValue,
) -> Result<GenerationJob, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update generation_jobs
            set status = 'succeeded',
                output_json = $2,
                last_error = null,
                next_run_at = null,
                locked_by = null,
                locked_at = null,
                finished_at = now()
            where id = $1 and status = 'running'
            returning
              id, workspace_id, storybook_id, job_type, status, input_json, output_json,
              attempt_count, last_error, next_run_at, locked_by, locked_at, created_at, finished_at
            "#,
            [job_id.into(), output_json.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("generation_job".to_string()))?;

    job_from_row(row)
}

async fn complete_and_apply_running_job(
    db: &DatabaseConnection,
    job_id: Uuid,
    output_json: JsonValue,
) -> Result<GenerationJob, DbErr> {
    let job = complete_running_job(db, job_id, output_json).await?;
    record_generation_cost_log(db, &job).await?;
    apply_completed_generation(db, &job).await?;
    Ok(job)
}

async fn apply_completed_generation(
    db: &DatabaseConnection,
    job: &GenerationJob,
) -> Result<(), DbErr> {
    if job.status != "succeeded" {
        return Ok(());
    }
    let Some(storybook_id) = job.storybook_id else {
        return Ok(());
    };
    let Some(output) = job.output_json.as_ref() else {
        return Ok(());
    };

    match job.job_type.as_str() {
        "storybook_roles" => replace_roles_from_generation(db, storybook_id, output).await,
        "storybook_pages" => replace_pages_from_generation(db, storybook_id, output).await,
        "storybook_role_reference_image" => {
            apply_role_reference_image(db, storybook_id, job, output).await
        }
        _ => Ok(()),
    }
}

async fn apply_role_reference_image(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    job: &GenerationJob,
    output: &JsonValue,
) -> Result<(), DbErr> {
    let role_id = job
        .input_json
        .get("role_id")
        .and_then(|value| value.as_str())
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| DbErr::Custom("角色参考图任务缺少 role_id，无法写回".to_string()))?;
    let image_url = output
        .get("image")
        .and_then(|value| value.get("image_url"))
        .and_then(|value| value.as_str())
        .ok_or_else(|| DbErr::Custom("角色参考图输出缺少 image_url".to_string()))?;
    let prompt = output
        .get("image")
        .and_then(|value| value.get("prompt"))
        .and_then(|value| value.as_str())
        .unwrap_or_default();

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_roles
        set reference_image_url = $3,
            reference_image_prompt = $4,
            reference_status = 'ready'
        where storybook_id = $1 and id = $2
        "#,
        [
            storybook_id.into(),
            role_id.into(),
            image_url.to_string().into(),
            prompt.to_string().into(),
        ],
    ))
    .await?;
    touch_storybook(db, storybook_id).await
}

async fn replace_roles_from_generation(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    output: &JsonValue,
) -> Result<(), DbErr> {
    let Some(roles) = output.get("roles").and_then(|value| value.as_array()) else {
        return Ok(());
    };
    if roles.is_empty() {
        return Ok(());
    }

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "delete from storybook_roles where storybook_id = $1",
        [storybook_id.into()],
    ))
    .await?;

    for role in roles {
        let id = Uuid::new_v4();
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into storybook_roles
              (id, storybook_id, name, role_type, appearance, story_function, needs_consistency,
               reference_image_url, reference_image_prompt, reference_status)
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            [
                id.into(),
                storybook_id.into(),
                json_text(role, "name", "未命名角色").into(),
                json_text(role, "role_type", "supporting").into(),
                json_text(role, "appearance", "待确认外观").into(),
                json_text(role, "story_function", "参与故事推进").into(),
                role.get("needs_consistency")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true)
                    .into(),
                json_optional_text(role, "reference_image_url").into(),
                json_optional_text(role, "reference_image_prompt").into(),
                json_text(role, "reference_status", "not_started").into(),
            ],
        ))
        .await?;
    }

    touch_storybook(db, storybook_id).await
}

async fn replace_pages_from_generation(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    output: &JsonValue,
) -> Result<(), DbErr> {
    let Some(pages) = output.get("pages").and_then(|value| value.as_array()) else {
        return Ok(());
    };
    if pages.is_empty() {
        return Ok(());
    }

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "delete from storybook_pages where storybook_id = $1",
        [storybook_id.into()],
    ))
    .await?;

    for (index, page) in pages.iter().enumerate() {
        let id = Uuid::new_v4();
        let page_number = page
            .get("page_number")
            .and_then(|value| value.as_i64())
            .unwrap_or((index + 1) as i64)
            .max(1) as i32;
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into storybook_pages
              (id, storybook_id, page_number, title, body, illustration_prompt, status)
            values ($1, $2, $3, $4, $5, $6, $7)
            "#,
            [
                id.into(),
                storybook_id.into(),
                page_number.into(),
                json_text(page, "title", "未命名分页").into(),
                json_text(page, "body", "待补充分页正文。").into(),
                json_text(page, "illustration_prompt", "待补充插图描述。").into(),
                json_text(page, "status", "draft").into(),
            ],
        ))
        .await?;
    }

    touch_storybook(db, storybook_id).await
}

async fn touch_storybook(db: &DatabaseConnection, storybook_id: Uuid) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "update storybooks set updated_at = now() where id = $1",
        [storybook_id.into()],
    ))
    .await?;
    Ok(())
}

fn json_text(value: &JsonValue, key: &str, fallback: &str) -> String {
    value
        .get(key)
        .and_then(|item| item.as_str())
        .filter(|item| !item.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn json_optional_text(value: &JsonValue, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|item| item.as_str())
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

async fn fail_running_job(
    db: &DatabaseConnection,
    job_id: Uuid,
    provider_name: &str,
    job_type: &str,
    err: GenerationProviderError,
) -> Result<GenerationJob, DbErr> {
    let safe_message = err.safe_message();
    let next_run_interval = if err.retryable {
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
            "retryable": err.retryable
        }
    });
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update generation_jobs
            set status = 'failed',
                output_json = $2,
                last_error = $3,
                next_run_at = case when $4::text is null then null else now() + ($4::text)::interval end,
                locked_by = null,
                locked_at = null,
                finished_at = now()
            where id = $1 and status = 'running'
            returning
              id, workspace_id, storybook_id, job_type, status, input_json, output_json,
              attempt_count, last_error, next_run_at, locked_by, locked_at, created_at, finished_at
            "#,
            [
                job_id.into(),
                output_json.into(),
                safe_message.into(),
                next_run_interval.into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("generation_job".to_string()))?;

    let job = job_from_row(row)?;
    record_generation_cost_log(db, &job).await?;
    Ok(job)
}

async fn record_generation_cost_log(
    db: &DatabaseConnection,
    job: &GenerationJob,
) -> Result<(), DbErr> {
    let Some(output) = job.output_json.as_ref() else {
        return Ok(());
    };
    let estimate = estimate_generation_cost(job);
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into generation_cost_logs
          (id, workspace_id, generation_job_id, storybook_id, provider, job_type, status,
           estimated_input_units, estimated_output_units, image_count, estimated_cost_micros,
           currency, metadata_json, created_at)
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, now())
        on conflict (generation_job_id, status) do nothing
        "#,
        [
            Uuid::new_v4().into(),
            job.workspace_id.into(),
            job.id.into(),
            job.storybook_id.into(),
            estimate.provider.into(),
            job.job_type.clone().into(),
            job.status.clone().into(),
            estimate.estimated_input_units.into(),
            estimate.estimated_output_units.into(),
            estimate.image_count.into(),
            estimate.estimated_cost_micros.into(),
            estimate.currency.into(),
            json!({
                "schema_version": "generation.cost.estimate.v1",
                "source": "server_estimate",
                "mode": output.get("mode").and_then(|value| value.as_str()).unwrap_or(job.job_type.as_str()),
                "retryable": output
                    .get("error")
                    .and_then(|value| value.get("retryable"))
                    .and_then(|value| value.as_bool())
            })
            .into(),
        ],
    ))
    .await?;
    Ok(())
}

#[derive(Debug, PartialEq)]
struct GenerationCostEstimate {
    provider: String,
    estimated_input_units: i32,
    estimated_output_units: i32,
    image_count: i32,
    estimated_cost_micros: i64,
    currency: String,
}

#[derive(Clone, Debug, PartialEq)]
struct GenerationCostPricing {
    deepseek_input_unit_micros: i64,
    deepseek_output_unit_micros: i64,
    seedream_image_micros: i64,
    currency: String,
}

impl Default for GenerationCostPricing {
    fn default() -> Self {
        Self {
            deepseek_input_unit_micros: 1,
            deepseek_output_unit_micros: 4,
            seedream_image_micros: 40_000,
            currency: "USD".to_string(),
        }
    }
}

impl GenerationCostPricing {
    fn from_env() -> Self {
        let defaults = Self::default();
        Self {
            deepseek_input_unit_micros: env_i64(
                "KINDLEAF_COST_DEEPSEEK_INPUT_UNIT_MICROS",
                defaults.deepseek_input_unit_micros,
            ),
            deepseek_output_unit_micros: env_i64(
                "KINDLEAF_COST_DEEPSEEK_OUTPUT_UNIT_MICROS",
                defaults.deepseek_output_unit_micros,
            ),
            seedream_image_micros: env_i64(
                "KINDLEAF_COST_SEEDREAM_IMAGE_MICROS",
                defaults.seedream_image_micros,
            ),
            currency: std::env::var("KINDLEAF_COST_CURRENCY")
                .ok()
                .map(|value| value.trim().to_ascii_uppercase())
                .filter(|value| !value.is_empty())
                .unwrap_or(defaults.currency),
        }
    }
}

fn estimate_generation_cost(job: &GenerationJob) -> GenerationCostEstimate {
    estimate_generation_cost_with_pricing(job, &GenerationCostPricing::from_env())
}

fn estimate_generation_cost_with_pricing(
    job: &GenerationJob,
    pricing: &GenerationCostPricing,
) -> GenerationCostEstimate {
    let output = job.output_json.as_ref();
    let provider = output
        .and_then(|value| value.get("provider"))
        .and_then(|value| value.as_str())
        .unwrap_or("unknown")
        .to_string();
    let status = job.status.as_str();
    let (estimated_input_units, estimated_output_units) =
        output.and_then(provider_usage_units).unwrap_or_else(|| {
            (
                estimate_json_units(&job.input_json),
                output.map(estimate_json_units).unwrap_or_default(),
            )
        });
    let image_count = if is_image_job(&job.job_type) && status == "succeeded" {
        1
    } else {
        0
    };
    let estimated_cost_micros = if status != "succeeded" || provider == "mock" {
        0
    } else if provider == "seedream" && is_image_job(&job.job_type) {
        pricing.seedream_image_micros * i64::from(image_count)
    } else if provider == "deepseek" {
        i64::from(estimated_input_units) * pricing.deepseek_input_unit_micros
            + i64::from(estimated_output_units) * pricing.deepseek_output_unit_micros
    } else {
        0
    };

    GenerationCostEstimate {
        provider,
        estimated_input_units,
        estimated_output_units,
        image_count,
        estimated_cost_micros,
        currency: pricing.currency.clone(),
    }
}

fn env_i64(key: &str, fallback: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value >= 0)
        .unwrap_or(fallback)
}

fn estimate_json_units(value: &JsonValue) -> i32 {
    let text = serde_json::to_string(value).unwrap_or_default();
    ((text.chars().count() as f64) / 4.0).ceil().max(0.0) as i32
}

fn provider_usage_units(output: &JsonValue) -> Option<(i32, i32)> {
    let usage = output.get("provider_usage")?;
    let input = usage
        .get("prompt_tokens")
        .or_else(|| usage.get("input_tokens"))
        .and_then(|value| value.as_i64())
        .unwrap_or(0)
        .max(0) as i32;
    let output = usage
        .get("completion_tokens")
        .or_else(|| usage.get("output_tokens"))
        .and_then(|value| value.as_i64())
        .unwrap_or(0)
        .max(0) as i32;
    if input == 0 && output == 0 {
        None
    } else {
        Some((input, output))
    }
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
    claim_next_ready_job_scoped(db, worker_id, None).await
}

async fn claim_next_ready_job_scoped(
    db: &DatabaseConnection,
    worker_id: &str,
    workspace_id: Option<Uuid>,
) -> Result<Option<GenerationJob>, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update generation_jobs
            set status = 'running',
                attempt_count = attempt_count + 1,
                last_error = null,
                next_run_at = null,
                locked_by = $1,
                locked_at = now(),
                finished_at = null
            where id = (
                select id
                from generation_jobs
                where status in ('queued', 'failed')
                  and (next_run_at is null or next_run_at <= now())
                  and (locked_at is null or locked_at < now() - interval '15 minutes')
                  and ($2::uuid is null or workspace_id = $2)
                order by created_at asc
                for update skip locked
                limit 1
            )
            returning
              id, workspace_id, storybook_id, job_type, status, input_json, output_json,
              attempt_count, last_error, next_run_at, locked_by, locked_at, created_at, finished_at
            "#,
            [worker_id.into(), workspace_id.into()],
        ))
        .await?;

    row.map(job_from_row).transpose()
}

pub async fn find_job(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    job_id: Uuid,
) -> Result<GenerationJob, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select
              id, workspace_id, storybook_id, job_type, status, input_json, output_json,
              attempt_count, last_error, next_run_at, locked_by, locked_at, created_at, finished_at
            from generation_jobs
            where workspace_id = $1 and id = $2
            limit 1
            "#,
            [workspace_id.into(), job_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("generation_job".to_string()))?;
    job_from_row(row)
}

pub async fn list_jobs_page(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Option<Uuid>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<(Vec<GenerationJob>, PaginationMeta), DbErr> {
    let limit = limit.unwrap_or(50).clamp(1, 100);
    let offset = offset.unwrap_or(0);
    let total: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select count(*) as count
            from generation_jobs
            where workspace_id = $1
              and ($2::uuid is null or storybook_id = $2)
            "#,
            [workspace_id.into(), storybook_id.into()],
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);

    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select
              id, workspace_id, storybook_id, job_type, status, input_json, output_json,
              attempt_count, last_error, next_run_at, locked_by, locked_at, created_at, finished_at
            from generation_jobs
            where workspace_id = $1
              and ($2::uuid is null or storybook_id = $2)
            order by created_at desc
            limit $3 offset $4
            "#,
            [
                workspace_id.into(),
                storybook_id.into(),
                (limit as i64).into(),
                (offset as i64).into(),
            ],
        ))
        .await?;

    let total = total.max(0) as usize;
    Ok((
        rows.into_iter()
            .map(job_from_row)
            .collect::<Result<Vec<_>, _>>()?,
        PaginationMeta {
            total,
            limit,
            offset: offset.min(total),
            has_more: offset.saturating_add(limit) < total,
        },
    ))
}

pub async fn list_operator_costs_page(
    db: &DatabaseConnection,
    query: GenerationCostListQuery,
) -> Result<(GenerationCostReport, PaginationMeta), DbErr> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0);
    let workspace_id = query.workspace_id;
    let provider = clean_filter(query.provider);
    let job_type = clean_filter(query.job_type);
    let status = clean_filter(query.status);

    let count_row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select count(*) as count
            from generation_cost_logs gcl
            where ($1::uuid is null or gcl.workspace_id = $1)
              and ($2::text is null or gcl.provider = $2)
              and ($3::text is null or gcl.job_type = $3)
              and ($4::text is null or gcl.status = $4)
            "#,
            [
                workspace_id.into(),
                provider.clone().into(),
                job_type.clone().into(),
                status.clone().into(),
            ],
        ))
        .await?;
    let total: i64 = count_row
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);

    let summary = cost_summary(db, workspace_id, &provider, &job_type, &status).await?;
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select
              gcl.id, gcl.workspace_id, w.name as workspace_name,
              gcl.generation_job_id, gcl.storybook_id, s.title as storybook_title,
              gcl.provider, gcl.job_type, gcl.status,
              gcl.estimated_input_units, gcl.estimated_output_units,
              gcl.image_count, gcl.estimated_cost_micros, gcl.currency,
              coalesce(gcl.metadata_json, '{}'::jsonb) as metadata_json,
              gcl.created_at
            from generation_cost_logs gcl
            left join workspaces w on w.id = gcl.workspace_id
            left join storybooks s on s.id = gcl.storybook_id
            where ($1::uuid is null or gcl.workspace_id = $1)
              and ($2::text is null or gcl.provider = $2)
              and ($3::text is null or gcl.job_type = $3)
              and ($4::text is null or gcl.status = $4)
            order by gcl.created_at desc
            limit $5 offset $6
            "#,
            [
                workspace_id.into(),
                provider.into(),
                job_type.into(),
                status.into(),
                (limit as i64).into(),
                (offset as i64).into(),
            ],
        ))
        .await?;
    let items = rows
        .into_iter()
        .map(cost_log_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    let total = total.max(0) as usize;

    Ok((
        GenerationCostReport { summary, items },
        PaginationMeta {
            total,
            limit,
            offset: offset.min(total),
            has_more: offset.saturating_add(limit) < total,
        },
    ))
}

async fn cost_summary(
    db: &DatabaseConnection,
    workspace_id: Option<Uuid>,
    provider: &Option<String>,
    job_type: &Option<String>,
    status: &Option<String>,
) -> Result<GenerationCostSummary, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select
              coalesce(sum(estimated_cost_micros), 0)::bigint as total_cost_micros,
              coalesce(sum(case when status = 'succeeded' then estimated_cost_micros else 0 end), 0)::bigint as succeeded_cost_micros,
              coalesce(sum(case when status = 'failed' then 1 else 0 end), 0)::bigint as failed_jobs,
              count(*)::bigint as total_jobs,
              coalesce(sum(estimated_input_units), 0)::bigint as total_input_units,
              coalesce(sum(estimated_output_units), 0)::bigint as total_output_units,
              coalesce(sum(image_count), 0)::bigint as total_images,
              coalesce(max(currency), 'USD') as currency
            from generation_cost_logs
            where ($1::uuid is null or workspace_id = $1)
              and ($2::text is null or provider = $2)
              and ($3::text is null or job_type = $3)
              and ($4::text is null or status = $4)
            "#,
            [
                workspace_id.into(),
                provider.clone().into(),
                job_type.clone().into(),
                status.clone().into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("generation_cost_summary".to_string()))?;

    Ok(with_budget_status(GenerationCostSummary {
        total_cost_micros: row.try_get("", "total_cost_micros")?,
        succeeded_cost_micros: row.try_get("", "succeeded_cost_micros")?,
        failed_jobs: row.try_get("", "failed_jobs")?,
        total_jobs: row.try_get("", "total_jobs")?,
        total_input_units: row.try_get("", "total_input_units")?,
        total_output_units: row.try_get("", "total_output_units")?,
        total_images: row.try_get("", "total_images")?,
        currency: row.try_get("", "currency")?,
        budget_limit_micros: None,
        budget_used_percent: None,
        budget_warning_percent: None,
        budget_warning: false,
        budget_exceeded: false,
    }))
}

async fn ensure_generation_budget_available(
    db: &DatabaseConnection,
    workspace_id: Option<Uuid>,
) -> Result<(), DbErr> {
    let Some(limit) = budget_limit_micros() else {
        return Ok(());
    };
    let used = succeeded_cost_micros(db, workspace_id).await?;
    if used >= limit {
        return Err(DbErr::Custom(format!(
            "generation_budget_exceeded: 生成预算已用尽，当前已用 {used} micros，预算上限 {limit} micros"
        )));
    }
    Ok(())
}

async fn succeeded_cost_micros(
    db: &DatabaseConnection,
    workspace_id: Option<Uuid>,
) -> Result<i64, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select coalesce(sum(estimated_cost_micros), 0)::bigint as succeeded_cost_micros
            from generation_cost_logs
            where status = 'succeeded'
              and ($1::uuid is null or workspace_id = $1)
            "#,
            [workspace_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("generation_cost_budget".to_string()))?;
    row.try_get("", "succeeded_cost_micros")
}

fn with_budget_status(summary: GenerationCostSummary) -> GenerationCostSummary {
    with_budget_limit(summary, budget_limit_micros())
}

fn with_budget_limit(
    mut summary: GenerationCostSummary,
    limit: Option<i64>,
) -> GenerationCostSummary {
    let Some(limit) = limit else {
        return summary;
    };
    summary.budget_limit_micros = Some(limit);
    let used_percent = if limit > 0 {
        (summary.succeeded_cost_micros.max(0) as f64 / limit as f64) * 100.0
    } else {
        0.0
    };
    let warning_percent = budget_warning_percent();
    summary.budget_used_percent = Some(used_percent);
    summary.budget_warning_percent = Some(warning_percent);
    summary.budget_warning = used_percent >= warning_percent;
    summary.budget_exceeded = summary.succeeded_cost_micros >= limit;
    summary
}

fn budget_limit_micros() -> Option<i64> {
    std::env::var("KINDLEAF_COST_BUDGET_LIMIT_MICROS")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value > 0)
}

fn budget_warning_percent() -> f64 {
    std::env::var("KINDLEAF_COST_BUDGET_WARNING_PERCENT")
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| *value > 0.0)
        .unwrap_or(80.0)
        .clamp(1.0, 100.0)
}

async fn page_prompt(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    page_id: Uuid,
) -> Result<String, DbErr> {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        select p.illustration_prompt
        from storybook_pages p
        join storybooks s on s.id = p.storybook_id
        where s.workspace_id = $1 and s.id = $2 and p.id = $3
        limit 1
        "#,
        [workspace_id.into(), storybook_id.into(), page_id.into()],
    ))
    .await?
    .ok_or_else(|| DbErr::RecordNotFound("page".to_string()))?
    .try_get("", "illustration_prompt")
}

async fn role_reference_prompt(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    role_id: Uuid,
) -> Result<String, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select r.name, r.role_type, r.appearance, coalesce(r.story_function, '') as story_function
            from storybook_roles r
            join storybooks s on s.id = r.storybook_id
            where s.workspace_id = $1 and s.id = $2 and r.id = $3
            limit 1
            "#,
            [workspace_id.into(), storybook_id.into(), role_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("role".to_string()))?;
    let name: String = row.try_get("", "name")?;
    let role_type: String = row.try_get("", "role_type")?;
    let appearance: String = row.try_get("", "appearance")?;
    let story_function: String = row.try_get("", "story_function")?;
    Ok(format!(
        "为幼儿园绘本角色生成单独参考图。角色名：{name}；角色类型：{role_type}；外观：{appearance}；故事作用：{story_function}。要求：白底或简洁背景，儿童绘本风格，全身或半身清晰，便于后续分页插图保持一致。"
    ))
}

fn job_from_row(row: sea_orm::QueryResult) -> Result<GenerationJob, DbErr> {
    Ok(GenerationJob {
        id: row.try_get("", "id")?,
        workspace_id: row.try_get("", "workspace_id")?,
        storybook_id: row.try_get("", "storybook_id")?,
        job_type: row.try_get("", "job_type")?,
        status: row.try_get("", "status")?,
        input_json: row.try_get::<JsonValue>("", "input_json")?,
        output_json: row.try_get("", "output_json")?,
        attempt_count: row.try_get("", "attempt_count")?,
        last_error: row.try_get("", "last_error")?,
        next_run_at: row.try_get("", "next_run_at")?,
        locked_by: row.try_get("", "locked_by")?,
        locked_at: row.try_get("", "locked_at")?,
        created_at: row.try_get::<DateTime<Utc>>("", "created_at")?,
        finished_at: row.try_get("", "finished_at")?,
    })
}

fn clean_filter(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn cost_log_from_row(row: sea_orm::QueryResult) -> Result<GenerationCostLog, DbErr> {
    Ok(GenerationCostLog {
        id: row.try_get("", "id")?,
        workspace_id: row.try_get("", "workspace_id")?,
        workspace_name: row.try_get("", "workspace_name")?,
        generation_job_id: row.try_get("", "generation_job_id")?,
        storybook_id: row.try_get("", "storybook_id")?,
        storybook_title: row.try_get("", "storybook_title")?,
        provider: row.try_get("", "provider")?,
        job_type: row.try_get("", "job_type")?,
        status: row.try_get("", "status")?,
        estimated_input_units: row.try_get("", "estimated_input_units")?,
        estimated_output_units: row.try_get("", "estimated_output_units")?,
        image_count: row.try_get("", "image_count")?,
        estimated_cost_micros: row.try_get("", "estimated_cost_micros")?,
        currency: row.try_get("", "currency")?,
        metadata_json: row.try_get::<JsonValue>("", "metadata_json")?,
        created_at: row.try_get::<DateTime<Utc>>("", "created_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(
        job_type: &str,
        status: &str,
        input_json: JsonValue,
        output_json: JsonValue,
    ) -> GenerationJob {
        GenerationJob {
            id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            storybook_id: Some(Uuid::new_v4()),
            job_type: job_type.to_string(),
            status: status.to_string(),
            input_json,
            output_json: Some(output_json),
            attempt_count: 1,
            last_error: None,
            next_run_at: None,
            locked_by: None,
            locked_at: None,
            created_at: Utc::now(),
            finished_at: Some(Utc::now()),
        }
    }

    #[test]
    fn mock_generation_cost_is_zero() {
        let estimate = estimate_generation_cost(&job(
            "storybook_plan",
            "succeeded",
            json!({"theme": "排队洗手"}),
            json!({"provider": "mock", "mode": "storybook_plan"}),
        ));

        assert_eq!(estimate.provider, "mock");
        assert_eq!(estimate.estimated_cost_micros, 0);
    }

    #[test]
    fn deepseek_text_generation_cost_uses_input_and_output_units() {
        let estimate = estimate_generation_cost(&job(
            "storybook_plan",
            "succeeded",
            json!({"theme": "排队洗手", "age_group": "4-5 岁"}),
            json!({"provider": "deepseek", "plan": {"title": "一起洗手", "summary": "孩子学会排队洗手"}}),
        ));

        assert_eq!(estimate.provider, "deepseek");
        assert!(estimate.estimated_input_units > 0);
        assert!(estimate.estimated_output_units > 0);
        assert!(estimate.estimated_cost_micros > 0);
        assert_eq!(estimate.image_count, 0);
    }

    #[test]
    fn deepseek_text_generation_cost_prefers_provider_usage() {
        let estimate = estimate_generation_cost(&job(
            "storybook_plan",
            "succeeded",
            json!({"theme": "排队洗手", "age_group": "4-5 岁"}),
            json!({
                "provider": "deepseek",
                "provider_usage": {
                    "prompt_tokens": 120,
                    "completion_tokens": 80,
                    "total_tokens": 200
                },
                "plan": {"title": "一起洗手"}
            }),
        ));

        assert_eq!(estimate.estimated_input_units, 120);
        assert_eq!(estimate.estimated_output_units, 80);
        assert_eq!(estimate.estimated_cost_micros, 440);
    }

    #[test]
    fn deepseek_text_generation_cost_uses_configured_pricing() {
        let estimate = estimate_generation_cost_with_pricing(
            &job(
                "storybook_plan",
                "succeeded",
                json!({"theme": "排队洗手"}),
                json!({
                    "provider": "deepseek",
                    "provider_usage": {
                        "prompt_tokens": 10,
                        "completion_tokens": 20
                    }
                }),
            ),
            &GenerationCostPricing {
                deepseek_input_unit_micros: 2,
                deepseek_output_unit_micros: 5,
                seedream_image_micros: 40_000,
                currency: "CNY".to_string(),
            },
        );

        assert_eq!(estimate.estimated_input_units, 10);
        assert_eq!(estimate.estimated_output_units, 20);
        assert_eq!(estimate.estimated_cost_micros, 120);
        assert_eq!(estimate.currency, "CNY");
    }

    #[test]
    fn seedream_image_generation_cost_counts_one_image() {
        let estimate = estimate_generation_cost(&job(
            "storybook_page_image",
            "succeeded",
            json!({"prompt": "温暖幼儿园教室"}),
            json!({"provider": "seedream", "image": {"image_url": "/api/image.png"}}),
        ));

        assert_eq!(estimate.provider, "seedream");
        assert_eq!(estimate.image_count, 1);
        assert_eq!(estimate.estimated_cost_micros, 40_000);
    }

    #[test]
    fn seedream_image_generation_cost_uses_configured_pricing() {
        let estimate = estimate_generation_cost_with_pricing(
            &job(
                "storybook_page_image",
                "succeeded",
                json!({"prompt": "温暖幼儿园教室"}),
                json!({"provider": "seedream", "image": {"image_url": "/api/image.png"}}),
            ),
            &GenerationCostPricing {
                deepseek_input_unit_micros: 1,
                deepseek_output_unit_micros: 4,
                seedream_image_micros: 88_000,
                currency: "USD".to_string(),
            },
        );

        assert_eq!(estimate.image_count, 1);
        assert_eq!(estimate.estimated_cost_micros, 88_000);
    }

    #[test]
    fn failed_generation_cost_is_zero() {
        let estimate = estimate_generation_cost(&job(
            "storybook_plan",
            "failed",
            json!({"theme": "排队洗手"}),
            json!({"provider": "deepseek", "error": {"retryable": true}}),
        ));

        assert_eq!(estimate.provider, "deepseek");
        assert_eq!(estimate.estimated_cost_micros, 0);
    }

    #[test]
    fn budget_status_marks_equal_limit_as_exceeded() {
        let summary = with_budget_limit(
            GenerationCostSummary {
                total_cost_micros: 100,
                succeeded_cost_micros: 100,
                failed_jobs: 0,
                total_jobs: 1,
                total_input_units: 0,
                total_output_units: 0,
                total_images: 0,
                currency: "USD".to_string(),
                budget_limit_micros: None,
                budget_used_percent: None,
                budget_warning_percent: None,
                budget_warning: false,
                budget_exceeded: false,
            },
            Some(100),
        );

        assert_eq!(summary.budget_limit_micros, Some(100));
        assert_eq!(summary.budget_used_percent, Some(100.0));
        assert_eq!(summary.budget_warning_percent, Some(80.0));
        assert!(summary.budget_warning);
        assert!(summary.budget_exceeded);
    }

    #[test]
    fn budget_status_warns_before_limit_is_exceeded() {
        let summary = with_budget_limit(
            GenerationCostSummary {
                total_cost_micros: 80,
                succeeded_cost_micros: 80,
                failed_jobs: 0,
                total_jobs: 1,
                total_input_units: 0,
                total_output_units: 0,
                total_images: 0,
                currency: "USD".to_string(),
                budget_limit_micros: None,
                budget_used_percent: None,
                budget_warning_percent: None,
                budget_warning: false,
                budget_exceeded: false,
            },
            Some(100),
        );

        assert_eq!(summary.budget_used_percent, Some(80.0));
        assert!(summary.budget_warning);
        assert!(!summary.budget_exceeded);
    }
}
