use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use std::{collections::HashMap, path::PathBuf};
use uuid::Uuid;

use crate::{
    models::{ExportJob, PaginationMeta},
    repositories::{
        delivery::{
            ensure_storybook_delivery_privacy_clear, ensure_storybook_in_workspace, export_from_row,
        },
        delivery_share_links::storybook_by_share_token,
        storybooks,
    },
    services::{pdf::encode_storybook_pdf_with_images, storage},
};

pub async fn create_export(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    created_by: Uuid,
) -> Result<ExportJob, DbErr> {
    ensure_storybook_in_workspace(db, workspace_id, storybook_id).await?;
    enqueue_export(db, storybook_id, Some(created_by)).await
}

pub async fn create_export_by_share_token(
    db: &DatabaseConnection,
    token: &str,
) -> Result<ExportJob, DbErr> {
    let storybook = storybook_by_share_token(db, token).await?;
    enqueue_export(db, storybook.id, None).await
}

pub async fn find_export_by_share_token(
    db: &DatabaseConnection,
    token: &str,
    export_id: Uuid,
) -> Result<ExportJob, DbErr> {
    let storybook = storybook_by_share_token(db, token).await?;
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, storybook_id, created_by, status, file_url, last_error, created_at, finished_at
            from export_jobs
            where id = $1 and storybook_id = $2
            limit 1
            "#,
            [export_id.into(), storybook.id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("export_job".to_string()))?;

    export_from_row(row)
}

async fn enqueue_export(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    created_by: Option<Uuid>,
) -> Result<ExportJob, DbErr> {
    ensure_storybook_delivery_privacy_clear(db, storybook_id).await?;
    let id = Uuid::new_v4();
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into export_jobs
              (id, storybook_id, created_by, status, created_at)
            values ($1, $2, $3, 'queued', now())
            returning id, storybook_id, created_by, status, file_url, last_error, created_at, finished_at
            "#,
            [id.into(), storybook_id.into(), created_by.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("export_job".to_string()))?;

    export_from_row(row)
}

async fn complete_export(
    db: &DatabaseConnection,
    job_id: Uuid,
    storybook_id: Uuid,
) -> Result<ExportJob, DbErr> {
    mark_export_running(db, job_id).await?;

    let file_url = match write_export_file(db, job_id, storybook_id).await {
        Ok(file_url) => file_url,
        Err(err) => {
            let _ = mark_export_failed(db, job_id, &err.to_string()).await;
            return Err(err);
        }
    };

    mark_export_succeeded(db, job_id, file_url).await
}

async fn mark_export_running(db: &DatabaseConnection, job_id: Uuid) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update export_jobs
        set status = 'running',
            last_error = null
        where id = $1 and status = 'queued'
        "#,
        [job_id.into()],
    ))
    .await?;
    Ok(())
}

async fn write_export_file(
    db: &DatabaseConnection,
    export_id: Uuid,
    storybook_id: Uuid,
) -> Result<String, DbErr> {
    let storybook = storybooks::find_any(db, storybook_id).await?;
    let page_images = latest_storybook_page_image_paths(db, storybook_id).await?;
    let file_name = export_file_name(export_id);
    let pdf = encode_storybook_pdf_with_images(&storybook, &page_images);
    crate::repositories::storage_quota::ensure_workspace_storage_available_for_user(
        db,
        storybook.workspace_id,
        export_created_by(db, export_id).await?,
        pdf.len() as u64,
    )
    .await?;
    storage::save_export_file(&file_name, &pdf).map_err(DbErr::Custom)
}

async fn export_created_by(
    db: &DatabaseConnection,
    export_id: Uuid,
) -> Result<Option<Uuid>, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select created_by
            from export_jobs
            where id = $1
            limit 1
            "#,
            [export_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("export_job".to_string()))?;

    row.try_get("", "created_by")
}

async fn latest_storybook_page_image_paths(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<HashMap<Uuid, PathBuf>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select distinct on (input_json->>'page_id')
              input_json->>'page_id' as page_id,
              output_json #>> '{image,image_url}' as image_url
            from generation_jobs
            where storybook_id = $1
              and job_type = 'storybook_page_image'
              and status = 'succeeded'
              and input_json->>'page_id' is not null
              and output_json #>> '{image,image_url}' is not null
            order by input_json->>'page_id', finished_at desc nulls last, created_at desc
            "#,
            [storybook_id.into()],
        ))
        .await?;

    let mut images = HashMap::new();
    for row in rows {
        let page_id = row
            .try_get::<String>("", "page_id")
            .ok()
            .and_then(|value| Uuid::parse_str(&value).ok());
        let file_name = row
            .try_get::<String>("", "image_url")
            .ok()
            .and_then(|value| export_image_file_name(&value));
        if let (Some(page_id), Some(file_name)) = (page_id, file_name) {
            let image_path =
                storage::local_generated_image_path(&file_name).map_err(DbErr::Custom)?;
            images.insert(page_id, image_path);
        }
    }
    Ok(images)
}

fn export_image_file_name(image_url: &str) -> Option<String> {
    let file_name = image_url.rsplit('/').next()?.trim();
    let (provider, id_with_ext) = file_name.split_once('-')?;
    if !matches!(provider, "mock" | "seedream") {
        return None;
    }
    let id = id_with_ext.strip_suffix(".png")?;
    Uuid::parse_str(id).ok()?;
    Some(file_name.to_string())
}

fn export_file_name(export_id: Uuid) -> String {
    format!("{export_id}.pdf")
}

fn truncate_export_error(error: &str) -> String {
    const MAX_ERROR_CHARS: usize = 240;
    let trimmed = error.trim();
    if trimmed.chars().count() <= MAX_ERROR_CHARS {
        return trimmed.to_string();
    }
    let mut value = trimmed.chars().take(MAX_ERROR_CHARS).collect::<String>();
    value.push('…');
    value
}

async fn mark_export_succeeded(
    db: &DatabaseConnection,
    job_id: Uuid,
    file_url: String,
) -> Result<ExportJob, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update export_jobs
            set status = 'succeeded',
                file_url = $2,
                last_error = null,
                finished_at = now()
            where id = $1 and status = 'running'
            returning id, storybook_id, created_by, status, file_url, last_error, created_at, finished_at
            "#,
            [job_id.into(), file_url.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("export_job".to_string()))?;

    export_from_row(row)
}

async fn mark_export_failed(
    db: &DatabaseConnection,
    job_id: Uuid,
    error: &str,
) -> Result<ExportJob, DbErr> {
    let error = truncate_export_error(error);
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update export_jobs
            set status = 'failed',
                last_error = $2,
                finished_at = now()
            where id = $1 and status = 'running'
            returning id, storybook_id, created_by, status, file_url, last_error, created_at, finished_at
            "#,
            [job_id.into(), error.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("export_job".to_string()))?;

    export_from_row(row)
}

pub async fn execute_export_job(
    db: &DatabaseConnection,
    export_id: Uuid,
) -> Result<ExportJob, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, storybook_id, created_by, status, file_url, last_error, created_at, finished_at
            from export_jobs
            where id = $1
            limit 1
            "#,
            [export_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("export_job".to_string()))?;
    let job = export_from_row(row)?;
    if job.status != "queued" {
        return Err(DbErr::Custom(
            "只有 queued 状态的导出任务可以执行".to_string(),
        ));
    }
    complete_export(db, job.id, job.storybook_id).await
}

pub async fn find_export(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    export_id: Uuid,
) -> Result<ExportJob, DbErr> {
    ensure_storybook_in_workspace(db, workspace_id, storybook_id).await?;
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, storybook_id, created_by, status, file_url, last_error, created_at, finished_at
            from export_jobs
            where id = $1 and storybook_id = $2
            limit 1
            "#,
            [export_id.into(), storybook_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("export_job".to_string()))?;

    export_from_row(row)
}

pub async fn list_exports(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    limit: usize,
    offset: usize,
) -> Result<(Vec<ExportJob>, PaginationMeta), DbErr> {
    ensure_storybook_in_workspace(db, workspace_id, storybook_id).await?;
    let total = count_exports(db, storybook_id).await?;
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, storybook_id, created_by, status, file_url, last_error, created_at, finished_at
            from export_jobs
            where storybook_id = $1
            order by created_at desc
            limit $2 offset $3
            "#,
            [
                storybook_id.into(),
                (limit as i64).into(),
                (offset as i64).into(),
            ],
        ))
        .await?;

    let jobs = rows
        .into_iter()
        .map(export_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok((
        jobs,
        PaginationMeta {
            total,
            limit,
            offset,
            has_more: offset.saturating_add(limit) < total,
        },
    ))
}

async fn count_exports(db: &DatabaseConnection, storybook_id: Uuid) -> Result<usize, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "select count(*) as count from export_jobs where storybook_id = $1",
            [storybook_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("export_jobs_count".to_string()))?;
    let count: i64 = row.try_get("", "count")?;
    Ok(count as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn export_file_name_is_pdf_for_export_id() {
        let export_id = Uuid::new_v4();
        let file_name = export_file_name(export_id);
        assert_eq!(file_name, format!("{export_id}.pdf"));
        assert!(file_name.ends_with(".pdf"));
        assert!(!file_name.contains('/'));
        assert!(!file_name.contains('\\'));
    }

    #[test]
    fn export_error_is_truncated_for_operator_readability() {
        let error = "错误".repeat(200);
        let truncated = truncate_export_error(&error);
        assert!(truncated.chars().count() <= 241);
        assert!(truncated.ends_with('…'));
    }
}
