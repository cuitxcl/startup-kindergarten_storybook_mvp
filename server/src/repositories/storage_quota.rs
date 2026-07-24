use std::collections::HashSet;

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use uuid::Uuid;

use crate::services::storage::{self, UserStorageQuotaSummary, WorkspaceStorageQuotaSummary};

pub async fn workspace_storage_quota(
    db: &DatabaseConnection,
    workspace_id: Uuid,
) -> Result<WorkspaceStorageQuotaSummary, DbErr> {
    let workspace_type = workspace_type(db, workspace_id).await?;
    let used_bytes = workspace_stored_bytes(db, workspace_id).await?;
    Ok(storage::workspace_storage_quota_summary(
        workspace_id,
        &workspace_type,
        used_bytes,
    ))
}

pub async fn user_storage_quota(
    db: &DatabaseConnection,
    user_id: Uuid,
) -> Result<UserStorageQuotaSummary, DbErr> {
    let workspace_ids = personal_workspace_ids_for_user(db, user_id).await?;
    let mut used_bytes = 0_u64;
    for workspace_id in &workspace_ids {
        used_bytes = used_bytes.saturating_add(workspace_stored_bytes(db, *workspace_id).await?);
    }

    Ok(storage::user_storage_quota_summary(
        user_id,
        used_bytes,
        workspace_ids.len() as u64,
    ))
}

pub async fn ensure_workspace_storage_available(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    additional_bytes: u64,
) -> Result<(), DbErr> {
    let summary = workspace_storage_quota(db, workspace_id).await?;
    let projected = summary.used_bytes.saturating_add(additional_bytes);
    if projected > summary.quota_bytes {
        return Err(DbErr::Custom(format!(
            "storage_quota_exceeded: 存储空间不足，当前已用 {} bytes，新增 {} bytes，限额 {} bytes",
            summary.used_bytes, additional_bytes, summary.quota_bytes
        )));
    }

    if summary.workspace_type == "personal" {
        let user_id = personal_workspace_owner_user_id(db, workspace_id).await?;
        ensure_user_storage_available(db, user_id, additional_bytes).await?;
    }

    Ok(())
}

pub async fn ensure_workspace_storage_available_for_url(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    file_url: &str,
) -> Result<(), DbErr> {
    let bytes = storage::file_size_for_url(file_url).map_err(DbErr::Custom)?;
    ensure_workspace_storage_available(db, workspace_id, bytes).await
}

async fn workspace_stored_bytes(db: &DatabaseConnection, workspace_id: Uuid) -> Result<u64, DbErr> {
    let mut urls = HashSet::new();

    for url in workspace_generated_image_urls(db, workspace_id).await? {
        urls.insert(url);
    }
    for url in workspace_export_urls(db, workspace_id).await? {
        urls.insert(url);
    }

    let mut used_bytes = 0_u64;
    for url in urls {
        match storage::file_size_for_url(&url) {
            Ok(bytes) => used_bytes = used_bytes.saturating_add(bytes),
            Err(_) => continue,
        }
    }
    Ok(used_bytes)
}

async fn workspace_type(db: &DatabaseConnection, workspace_id: Uuid) -> Result<String, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select workspace_type
            from workspaces
            where id = $1
            limit 1
            "#,
            [workspace_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("workspace".to_string()))?;

    row.try_get("", "workspace_type")
}

async fn personal_workspace_ids_for_user(
    db: &DatabaseConnection,
    user_id: Uuid,
) -> Result<Vec<Uuid>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select distinct w.id
            from workspaces w
            join workspace_members wm on wm.workspace_id = w.id
            where wm.user_id = $1
              and wm.status = 'active'
              and w.status = 'active'
              and w.workspace_type = 'personal'
            "#,
            [user_id.into()],
        ))
        .await?;

    rows.into_iter().map(|row| row.try_get("", "id")).collect()
}

async fn personal_workspace_owner_user_id(
    db: &DatabaseConnection,
    workspace_id: Uuid,
) -> Result<Uuid, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select user_id
            from workspace_members
            where workspace_id = $1
              and role = 'personal_owner'
              and status = 'active'
            order by created_at asc
            limit 1
            "#,
            [workspace_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("personal workspace owner".to_string()))?;

    row.try_get("", "user_id")
}

async fn ensure_user_storage_available(
    db: &DatabaseConnection,
    user_id: Uuid,
    additional_bytes: u64,
) -> Result<(), DbErr> {
    let summary = user_storage_quota(db, user_id).await?;
    let projected = summary.used_bytes.saturating_add(additional_bytes);
    if projected <= summary.quota_bytes {
        return Ok(());
    }

    Err(DbErr::Custom(format!(
        "storage_quota_exceeded: 用户存储空间不足，当前已用 {} bytes，新增 {} bytes，限额 {} bytes",
        summary.used_bytes, additional_bytes, summary.quota_bytes
    )))
}

async fn workspace_generated_image_urls(
    db: &DatabaseConnection,
    workspace_id: Uuid,
) -> Result<Vec<String>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select output_json #>> '{image,image_url}' as file_url
            from generation_jobs
            where workspace_id = $1
              and status = 'succeeded'
              and job_type in ('storybook_page_image', 'storybook_role_reference_image')
              and output_json #>> '{image,image_url}' is not null
            "#,
            [workspace_id.into()],
        ))
        .await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| row.try_get::<String>("", "file_url").ok())
        .filter(|url| url.starts_with("/generated-images/"))
        .collect())
}

async fn workspace_export_urls(
    db: &DatabaseConnection,
    workspace_id: Uuid,
) -> Result<Vec<String>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select ej.file_url
            from export_jobs ej
            join storybooks s on s.id = ej.storybook_id
            where s.workspace_id = $1
              and ej.status = 'succeeded'
              and ej.file_url is not null
            "#,
            [workspace_id.into()],
        ))
        .await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| row.try_get::<String>("", "file_url").ok())
        .filter(|url| url.starts_with("/exports/"))
        .collect())
}
