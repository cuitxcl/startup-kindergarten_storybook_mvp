use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::models::{AuditLogEntry, PaginationMeta};

pub async fn log(
    db: &DatabaseConnection,
    workspace_id: Option<Uuid>,
    actor_user_id: Option<Uuid>,
    action: &str,
    resource_type: &str,
    resource_id: Option<Uuid>,
    metadata: JsonValue,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into audit_logs
          (id, workspace_id, actor_user_id, action, resource_type, resource_id, metadata_json, created_at)
        values ($1, $2, $3, $4, $5, $6, $7, now())
        "#,
        [
            Uuid::new_v4().into(),
            workspace_id.into(),
            actor_user_id.into(),
            action.into(),
            resource_type.into(),
            resource_id.into(),
            metadata.into(),
        ],
    ))
    .await?;
    Ok(())
}

pub async fn list_page_by_workspace(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<(Vec<AuditLogEntry>, PaginationMeta), DbErr> {
    let limit = limit.unwrap_or(50).clamp(1, 100);
    let offset = offset.unwrap_or(0);
    let total: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "select count(*) as count from audit_logs where workspace_id = $1",
            [workspace_id.into()],
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);

    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select
              al.id,
              al.workspace_id,
              al.actor_user_id,
              u.display_name as actor_name,
              al.action,
              al.resource_type,
              al.resource_id,
              coalesce(al.metadata_json, '{}'::jsonb) as metadata_json,
              al.created_at
            from audit_logs al
            left join users u on u.id = al.actor_user_id
            where al.workspace_id = $1
            order by al.created_at desc
            limit $2 offset $3
            "#,
            [
                workspace_id.into(),
                (limit as i64).into(),
                (offset as i64).into(),
            ],
        ))
        .await?;
    let total = total.max(0) as usize;
    Ok((
        rows.into_iter()
            .map(entry_from_row)
            .collect::<Result<Vec<_>, _>>()?,
        PaginationMeta {
            total,
            limit,
            offset: offset.min(total),
            has_more: offset.saturating_add(limit) < total,
        },
    ))
}

pub async fn list_all_page(
    db: &DatabaseConnection,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<(Vec<AuditLogEntry>, PaginationMeta), DbErr> {
    let limit = limit.unwrap_or(50).clamp(1, 100);
    let offset = offset.unwrap_or(0);
    let total: i64 = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "select count(*) as count from audit_logs".to_string(),
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);

    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select
              al.id,
              al.workspace_id,
              al.actor_user_id,
              u.display_name as actor_name,
              al.action,
              al.resource_type,
              al.resource_id,
              coalesce(al.metadata_json, '{}'::jsonb) as metadata_json,
              al.created_at
            from audit_logs al
            left join users u on u.id = al.actor_user_id
            order by al.created_at desc
            limit $1 offset $2
            "#,
            [(limit as i64).into(), (offset as i64).into()],
        ))
        .await?;
    let total = total.max(0) as usize;
    Ok((
        rows.into_iter()
            .map(entry_from_row)
            .collect::<Result<Vec<_>, _>>()?,
        PaginationMeta {
            total,
            limit,
            offset: offset.min(total),
            has_more: offset.saturating_add(limit) < total,
        },
    ))
}

fn entry_from_row(row: sea_orm::QueryResult) -> Result<AuditLogEntry, DbErr> {
    Ok(AuditLogEntry {
        id: row.try_get("", "id")?,
        workspace_id: row.try_get("", "workspace_id")?,
        actor_user_id: row.try_get("", "actor_user_id")?,
        actor_name: row.try_get("", "actor_name")?,
        action: row.try_get("", "action")?,
        resource_type: row.try_get("", "resource_type")?,
        resource_id: row.try_get("", "resource_id")?,
        metadata_json: row.try_get("", "metadata_json")?,
        created_at: row.try_get("", "created_at")?,
    })
}
