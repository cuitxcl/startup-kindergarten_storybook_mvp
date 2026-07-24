use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use uuid::Uuid;

pub use super::delivery_exports::{
    create_export, create_export_by_share_token, execute_export_job, find_export,
    find_export_by_share_token, list_exports,
};
pub use super::delivery_share_links::{
    create_share_link, list_share_links, record_share_link_access, revoke_share_link,
    storybook_by_share_token,
};

use crate::models::{ExportJob, ShareLink};

pub(crate) async fn ensure_storybook_delivery_privacy_clear(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<(), DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select concat_ws(
              ' ',
              s.title,
              s.teaching_goal,
              s.use_scene,
              s.cover_tone,
              coalesce(string_agg(distinct concat_ws(' ', sp.title, sp.body, sp.illustration_prompt), ' '), ''),
              coalesce(string_agg(distinct concat_ws(' ', sr.name, sr.appearance, sr.story_function), ' '), '')
            ) as privacy_text
            from storybooks s
            left join storybook_pages sp on sp.storybook_id = s.id
            left join storybook_roles sr on sr.storybook_id = s.id
            where s.id = $1
            group by s.id
            "#,
            [storybook_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("storybook".to_string()))?;

    let privacy_text: String = row.try_get("", "privacy_text")?;
    let risks = crate::repositories::privacy::storybook_privacy_risks(&privacy_text);
    if risks.is_empty() {
        Ok(())
    } else {
        Err(DbErr::Custom(format!(
            "delivery_privacy_risk:{}",
            risks.join("、")
        )))
    }
}

pub(crate) async fn ensure_storybook_in_workspace(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<(), DbErr> {
    let exists = db
        .query_one(Statement::from_sql_and_values(
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
        .is_some();

    if exists {
        Ok(())
    } else {
        Err(DbErr::RecordNotFound("storybook".to_string()))
    }
}

pub(crate) fn share_link_from_row(row: &sea_orm::QueryResult) -> Result<ShareLink, DbErr> {
    let token: String = row.try_get("", "token")?;
    let expires_at: Option<DateTime<Utc>> = row.try_get("", "expires_at")?;
    let stored_status: String = row.try_get("", "status")?;
    let status = if stored_status == "active" && expires_at.is_some_and(|value| value <= Utc::now())
    {
        "expired".to_string()
    } else {
        stored_status
    };
    Ok(ShareLink {
        id: row.try_get("", "id")?,
        storybook_id: row.try_get("", "storybook_id")?,
        url: format!("/link/share/{token}"),
        token,
        status,
        access_count: row.try_get("", "access_count")?,
        last_accessed_at: row
            .try_get::<Option<DateTime<Utc>>>("", "last_accessed_at")?
            .map(|value| value.format("%Y-%m-%d %H:%M").to_string()),
        expires_at: expires_at.map(|value| value.to_rfc3339()),
    })
}

pub(crate) fn export_from_row(row: sea_orm::QueryResult) -> Result<ExportJob, DbErr> {
    Ok(ExportJob {
        id: row.try_get("", "id")?,
        storybook_id: row.try_get("", "storybook_id")?,
        created_by: row.try_get("", "created_by")?,
        status: row.try_get("", "status")?,
        file_url: row.try_get("", "file_url")?,
        last_error: row.try_get("", "last_error")?,
        created_at: row.try_get::<DateTime<Utc>>("", "created_at")?,
        finished_at: row.try_get("", "finished_at")?,
    })
}
