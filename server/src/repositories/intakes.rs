use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

pub use super::intake_links::{
    create_link, get_public_link, list_links_page, resolve_link_workspace, revoke_active_links,
    revoke_link,
};
pub use super::intake_submissions::{confirm, list_page_by_workspace, submit_parent_intake};

use crate::models::{PaginationMeta, ParentIntake, ParentIntakeLink, PublicParentIntakeLink};

pub const DEFAULT_INTAKE_WORKSPACE_ID: Uuid = Uuid::from_u128(0x20000000000000000000000000000001);

pub(crate) async fn ensure_workspace_exists(
    db: &DatabaseConnection,
    workspace_id: Uuid,
) -> Result<(), DbErr> {
    let exists = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id
            from workspaces
            where id = $1
            limit 1
            "#,
            [workspace_id.into()],
        ))
        .await?
        .is_some();

    if exists {
        Ok(())
    } else {
        Err(DbErr::RecordNotFound("workspace".to_string()))
    }
}

pub(crate) async fn resolve_active_link_classroom_id(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    token: &str,
) -> Result<Option<Uuid>, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select classroom_id
            from parent_intake_links
            where workspace_id = $1
              and token = $2
              and status = 'active'
              and (expires_at is null or expires_at > now())
            limit 1
            "#,
            [workspace_id.into(), token.to_string().into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("parent_intake_link".to_string()))?;
    row.try_get("", "classroom_id")
}

pub(crate) async fn resolve_classroom_id(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    classroom_name: Option<&str>,
) -> Result<Option<Uuid>, DbErr> {
    let Some(name) = optional_trimmed(classroom_name) else {
        return Ok(None);
    };
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id
            from classrooms
            where workspace_id = $1
              and name = $2
              and status = 'active'
            limit 1
            "#,
            [workspace_id.into(), name.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("classroom".to_string()))?;
    Ok(Some(row.try_get("", "id")?))
}

pub(crate) fn optional_trimmed(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(crate) fn pagination_meta(total: usize, limit: usize, offset: usize) -> PaginationMeta {
    PaginationMeta {
        total,
        limit,
        offset: offset.min(total),
        has_more: offset.saturating_add(limit) < total,
    }
}

pub(crate) fn intake_from_row(row: sea_orm::QueryResult) -> Result<ParentIntake, DbErr> {
    let interests: JsonValue = row.try_get("", "interests")?;
    let created_at: DateTime<Utc> = row.try_get("", "created_at")?;
    let updated_at: DateTime<Utc> = row.try_get("", "updated_at")?;
    Ok(ParentIntake {
        id: row.try_get("", "id")?,
        workspace_id: row.try_get("", "workspace_id")?,
        child_nickname: row.try_get("", "child_nickname")?,
        age_group: row.try_get("", "age_group")?,
        classroom: row.try_get("", "classroom")?,
        interests: json_string_array(interests),
        status: row.try_get("", "status")?,
        confirmed_child_id: row.try_get("", "confirmed_child_id")?,
        created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
        updated_at: updated_at.format("%Y-%m-%d %H:%M").to_string(),
    })
}

pub(crate) fn link_from_row(row: sea_orm::QueryResult) -> Result<ParentIntakeLink, DbErr> {
    let created_at: DateTime<Utc> = row.try_get("", "created_at")?;
    let updated_at: DateTime<Utc> = row.try_get("", "updated_at")?;
    let expires_at: Option<DateTime<Utc>> = row.try_get("", "expires_at")?;
    let last_accessed_at: Option<DateTime<Utc>> = row.try_get("", "last_accessed_at")?;
    let token: String = row.try_get("", "token")?;
    let stored_status: String = row.try_get("", "status")?;
    let effective_status =
        if stored_status == "active" && expires_at.is_some_and(|value| value <= Utc::now()) {
            "expired".to_string()
        } else {
            stored_status
        };
    Ok(ParentIntakeLink {
        id: row.try_get("", "id")?,
        workspace_id: row.try_get("", "workspace_id")?,
        url: format!("/link/intake/{token}"),
        token,
        label: row.try_get("", "label")?,
        classroom: row.try_get("", "classroom")?,
        status: effective_status,
        expires_at: expires_at.map(|value| value.format("%Y-%m-%d %H:%M").to_string()),
        access_count: row.try_get("", "access_count")?,
        last_accessed_at: last_accessed_at.map(|value| value.format("%Y-%m-%d %H:%M").to_string()),
        created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
        updated_at: updated_at.format("%Y-%m-%d %H:%M").to_string(),
    })
}

pub(crate) fn public_link_from_row(
    row: sea_orm::QueryResult,
) -> Result<PublicParentIntakeLink, DbErr> {
    let expires_at: Option<DateTime<Utc>> = row.try_get("", "expires_at")?;
    let stored_status: String = row.try_get("", "status")?;
    let effective_status =
        if stored_status == "active" && expires_at.is_some_and(|value| value <= Utc::now()) {
            "expired".to_string()
        } else {
            stored_status
        };
    Ok(PublicParentIntakeLink {
        token: row.try_get("", "token")?,
        workspace_id: row.try_get("", "workspace_id")?,
        workspace_name: row.try_get("", "workspace_name")?,
        label: row.try_get("", "label")?,
        classroom: row.try_get("", "classroom")?,
        status: effective_status,
        expires_at: expires_at.map(|value| value.format("%Y-%m-%d %H:%M").to_string()),
    })
}

pub(crate) fn json_string_array(value: JsonValue) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}
