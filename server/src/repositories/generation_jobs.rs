use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

use crate::models::{GenerationJob, PaginationMeta};

pub async fn enqueue_job(
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

pub async fn move_to_running(
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

pub async fn complete_running_job(
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

pub async fn fail_running_job(
    db: &DatabaseConnection,
    job_id: Uuid,
    output_json: JsonValue,
    safe_message: String,
    next_run_interval: Option<&str>,
) -> Result<GenerationJob, DbErr> {
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

    job_from_row(row)
}

pub async fn cancel_job(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    job_id: Uuid,
) -> Result<GenerationJob, DbErr> {
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

pub async fn requeue_stale_jobs_scoped(
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

pub async fn claim_next_ready_job_scoped(
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

pub async fn find_any_job(db: &DatabaseConnection, job_id: Uuid) -> Result<GenerationJob, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select
              id, workspace_id, storybook_id, job_type, status, input_json, output_json,
              attempt_count, last_error, next_run_at, locked_by, locked_at, created_at, finished_at
            from generation_jobs
            where id = $1
            limit 1
            "#,
            [job_id.into()],
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
