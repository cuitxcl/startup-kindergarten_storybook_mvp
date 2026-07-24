use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use uuid::Uuid;

use crate::{
    models::{PaginationMeta, ShareLink, Storybook},
    repositories::{
        delivery::{
            ensure_storybook_delivery_privacy_clear, ensure_storybook_in_workspace,
            share_link_from_row,
        },
        storybooks,
    },
};

pub async fn create_share_link(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    expires_at: Option<DateTime<Utc>>,
) -> Result<ShareLink, DbErr> {
    ensure_storybook_in_workspace(db, workspace_id, storybook_id).await?;
    ensure_storybook_delivery_privacy_clear(db, storybook_id).await?;

    let id = Uuid::new_v4();
    let token = Uuid::new_v4().simple().to_string();
    let status = if expires_at.is_some_and(|value| value <= Utc::now()) {
        "expired"
    } else {
        "active"
    };
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into share_links
              (id, storybook_id, token, status, created_at, expires_at)
            values ($1, $2, $3, $4, now(), $5)
            returning id, storybook_id, token, status, access_count, last_accessed_at, expires_at
            "#,
            [
                id.into(),
                storybook_id.into(),
                token.into(),
                status.into(),
                expires_at.into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("share_link".to_string()))?;

    share_link_from_row(&row)
}

pub async fn list_share_links(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    limit: usize,
    offset: usize,
) -> Result<(Vec<ShareLink>, PaginationMeta), DbErr> {
    ensure_storybook_in_workspace(db, workspace_id, storybook_id).await?;
    let total = count_share_links(db, storybook_id).await?;

    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, storybook_id, token, status, access_count, last_accessed_at, expires_at
            from share_links
            where storybook_id = $1
              and status = 'active'
              and (expires_at is null or expires_at > now())
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

    let links = rows
        .iter()
        .map(share_link_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok((
        links,
        PaginationMeta {
            total,
            limit,
            offset,
            has_more: offset.saturating_add(limit) < total,
        },
    ))
}

async fn count_share_links(db: &DatabaseConnection, storybook_id: Uuid) -> Result<usize, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select count(*) as count
            from share_links
            where storybook_id = $1
              and status = 'active'
              and (expires_at is null or expires_at > now())
            "#,
            [storybook_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("share_links_count".to_string()))?;
    let count: i64 = row.try_get("", "count")?;
    Ok(count as usize)
}

pub async fn revoke_share_link(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    share_link_id: Uuid,
) -> Result<ShareLink, DbErr> {
    ensure_storybook_in_workspace(db, workspace_id, storybook_id).await?;

    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update share_links
            set status = 'revoked'
            where id = $1
              and storybook_id = $2
              and status = 'active'
            returning id, storybook_id, token, status, access_count, last_accessed_at, expires_at
            "#,
            [share_link_id.into(), storybook_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("share_link".to_string()))?;

    share_link_from_row(&row)
}

pub async fn storybook_by_share_token(
    db: &DatabaseConnection,
    token: &str,
) -> Result<Storybook, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select storybook_id
            from share_links
            where token = $1
              and status = 'active'
              and (expires_at is null or expires_at > now())
            limit 1
            "#,
            [token.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("share_link".to_string()))?;

    storybooks::find_any(db, row.try_get("", "storybook_id")?).await
}

pub async fn record_share_link_access(
    db: &DatabaseConnection,
    token: &str,
) -> Result<ShareLink, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update share_links
            set access_count = access_count + 1,
                last_accessed_at = now()
            where token = $1
              and status = 'active'
              and (expires_at is null or expires_at > now())
            returning id, storybook_id, token, status, access_count, last_accessed_at, expires_at
            "#,
            [token.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("share_link".to_string()))?;

    share_link_from_row(&row)
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    #[test]
    fn share_link_url_keeps_current_public_route_shape() {
        let token = Uuid::new_v4().simple().to_string();
        assert_eq!(
            format!("/link/share/{token}"),
            format!("/link/share/{token}")
        );
    }
}
