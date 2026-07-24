use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use uuid::Uuid;

use crate::{
    models::{
        CreateParentIntakeLinkRequest, PaginationMeta, ParentIntakeLink, PublicParentIntakeLink,
    },
    repositories::intakes::{
        ensure_workspace_exists, link_from_row, optional_trimmed, pagination_meta,
        public_link_from_row, resolve_classroom_id,
    },
};

pub async fn create_link(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    created_by: Uuid,
    payload: CreateParentIntakeLinkRequest,
) -> Result<ParentIntakeLink, DbErr> {
    ensure_workspace_exists(db, workspace_id).await?;
    let label = payload
        .label
        .and_then(|value| {
            let value = value.trim().to_string();
            (!value.is_empty()).then_some(value)
        })
        .unwrap_or_else(|| "家长资料收集链接".to_string());
    let classroom_id = resolve_classroom_id(db, workspace_id, payload.classroom.as_deref()).await?;
    let token = format!("intake-{}", Uuid::new_v4());
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            with inserted as (
              insert into parent_intake_links
                (id, workspace_id, classroom_id, token, label, status, expires_at, created_by, created_at, updated_at)
              values ($1, $2, $3, $4, $5, 'active', $6, $7, now(), now())
              returning id, workspace_id, classroom_id, token, label, status, expires_at,
                        access_count, last_accessed_at, created_at, updated_at
            )
            select inserted.id, inserted.workspace_id, inserted.token, inserted.label,
                   c.name as classroom, inserted.status, inserted.expires_at,
                   inserted.access_count, inserted.last_accessed_at,
                   inserted.created_at, inserted.updated_at
            from inserted
            left join classrooms c on c.id = inserted.classroom_id and c.workspace_id = inserted.workspace_id
            "#,
            [
                Uuid::new_v4().into(),
                workspace_id.into(),
                classroom_id.into(),
                token.into(),
                label.into(),
                payload.expires_at.into(),
                created_by.into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("parent_intake_link".to_string()))?;

    link_from_row(row)
}

pub async fn list_links_page(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    status: Option<&str>,
    classroom: Option<&str>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<(Vec<ParentIntakeLink>, PaginationMeta), DbErr> {
    let limit = limit.unwrap_or(50).clamp(1, 100);
    let offset = offset.unwrap_or(0);
    let status_filter = link_status_filter(status)?;
    let classroom_filter = optional_trimmed(classroom);
    let total =
        count_links_by_workspace(db, workspace_id, status_filter, classroom_filter.as_deref())
            .await?;
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                r#"
            select pil.id, pil.workspace_id, pil.token, pil.label, c.name as classroom,
                   pil.status, pil.expires_at, pil.access_count, pil.last_accessed_at,
                   pil.created_at, pil.updated_at
            from parent_intake_links pil
            left join classrooms c on c.id = pil.classroom_id and c.workspace_id = pil.workspace_id
            where pil.workspace_id = $1
              and ($4::text is null or c.name = $4)
              {status_where}
            order by pil.created_at desc
            limit $2 offset $3
            "#,
                status_where = status_filter.where_sql()
            ),
            [
                workspace_id.into(),
                (limit as i64).into(),
                (offset as i64).into(),
                classroom_filter.into(),
            ],
        ))
        .await?;

    Ok((
        rows.into_iter()
            .map(link_from_row)
            .collect::<Result<Vec<_>, _>>()?,
        pagination_meta(total, limit, offset),
    ))
}

pub async fn resolve_link_workspace(db: &DatabaseConnection, token: &str) -> Result<Uuid, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select workspace_id
            from parent_intake_links
            where token = $1
              and status = 'active'
              and (expires_at is null or expires_at > now())
            limit 1
            "#,
            [token.to_string().into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("parent_intake_link".to_string()))?;
    row.try_get("", "workspace_id")
}

pub async fn get_public_link(
    db: &DatabaseConnection,
    token: &str,
) -> Result<PublicParentIntakeLink, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            with touched as (
              update parent_intake_links
              set access_count = access_count + 1,
                  last_accessed_at = now()
              where token = $1
              returning token, workspace_id, classroom_id, label, status, expires_at
            )
            select touched.token, touched.workspace_id, w.name as workspace_name,
                   c.name as classroom, touched.label, touched.status, touched.expires_at
            from touched
            join workspaces w on w.id = touched.workspace_id
            left join classrooms c on c.id = touched.classroom_id and c.workspace_id = touched.workspace_id
            "#,
            [token.to_string().into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("parent_intake_link".to_string()))?;

    public_link_from_row(row)
}

pub async fn revoke_link(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    link_id: Uuid,
) -> Result<ParentIntakeLink, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update parent_intake_links
            set status = 'revoked',
                updated_at = now()
            where workspace_id = $1 and id = $2
            returning id
            "#,
            [workspace_id.into(), link_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("parent_intake_link".to_string()))?;
    let id: Uuid = row.try_get("", "id")?;
    find_link(db, workspace_id, id).await
}

async fn find_link(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    link_id: Uuid,
) -> Result<ParentIntakeLink, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select pil.id, pil.workspace_id, pil.token, pil.label, c.name as classroom,
                   pil.status, pil.expires_at, pil.access_count, pil.last_accessed_at,
                   pil.created_at, pil.updated_at
            from parent_intake_links pil
            left join classrooms c on c.id = pil.classroom_id and c.workspace_id = pil.workspace_id
            where pil.workspace_id = $1 and pil.id = $2
            limit 1
            "#,
            [workspace_id.into(), link_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("parent_intake_link".to_string()))?;
    link_from_row(row)
}

pub async fn revoke_active_links(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    classroom: Option<&str>,
) -> Result<usize, DbErr> {
    let classroom_filter = optional_trimmed(classroom);
    let result = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update parent_intake_links pil
            set status = 'revoked',
                updated_at = now()
            where pil.workspace_id = $1
              and pil.status = 'active'
              and (pil.expires_at is null or pil.expires_at > now())
              and (
                $2::text is null
                or exists (
                  select 1
                  from classrooms c
                  where c.id = pil.classroom_id
                    and c.workspace_id = pil.workspace_id
                    and c.name = $2
                )
              )
            "#,
            [workspace_id.into(), classroom_filter.into()],
        ))
        .await?;
    Ok(result.rows_affected() as usize)
}

async fn count_links_by_workspace(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    status_filter: LinkStatusFilter,
    classroom: Option<&str>,
) -> Result<usize, DbErr> {
    let total: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                r#"
                select count(*) as count
                from parent_intake_links pil
                left join classrooms c on c.id = pil.classroom_id and c.workspace_id = pil.workspace_id
                where pil.workspace_id = $1
                  and ($2::text is null or c.name = $2)
                  {status_where}
                "#,
                status_where = status_filter.where_sql()
            ),
            [workspace_id.into(), classroom.map(str::to_string).into()],
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);
    Ok(total.max(0) as usize)
}

#[derive(Clone, Copy)]
enum LinkStatusFilter {
    Any,
    Active,
    Revoked,
    Expired,
}

impl LinkStatusFilter {
    fn where_sql(self) -> &'static str {
        match self {
            LinkStatusFilter::Any => "",
            LinkStatusFilter::Active => {
                "and pil.status = 'active' and (pil.expires_at is null or pil.expires_at > now())"
            }
            LinkStatusFilter::Revoked => "and pil.status = 'revoked'",
            LinkStatusFilter::Expired => {
                "and ((pil.status = 'active' and pil.expires_at is not null and pil.expires_at <= now()) or pil.status = 'expired')"
            }
        }
    }
}

fn link_status_filter(status: Option<&str>) -> Result<LinkStatusFilter, DbErr> {
    match status.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(LinkStatusFilter::Any),
        Some("active") => Ok(LinkStatusFilter::Active),
        Some("revoked") => Ok(LinkStatusFilter::Revoked),
        Some("expired") => Ok(LinkStatusFilter::Expired),
        Some(other) => Err(DbErr::Custom(format!("不支持的家长资料链接状态：{other}"))),
    }
}
