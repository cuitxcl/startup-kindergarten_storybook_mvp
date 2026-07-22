use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::models::{
    ActionResponse, ChildProfile, ConfirmParentIntakeRequest, CreateParentIntakeLinkRequest,
    PaginationMeta, ParentIntake, ParentIntakeLink, ParentIntakeRequest, PublicParentIntakeLink,
};
use crate::repositories::children::calculate_completeness;

pub const DEFAULT_INTAKE_WORKSPACE_ID: Uuid = Uuid::from_u128(0x20000000000000000000000000000001);

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

pub async fn submit_parent_intake(
    db: &DatabaseConnection,
    payload: ParentIntakeRequest,
) -> Result<ActionResponse, DbErr> {
    let interest_count = payload.interests.len();
    let workspace_id = payload.workspace_id.unwrap_or(DEFAULT_INTAKE_WORKSPACE_ID);
    ensure_workspace_exists(db, workspace_id).await?;
    let classroom_id = match payload.link_token.as_deref() {
        Some(token) => resolve_active_link_classroom_id(db, workspace_id, token).await?,
        None => None,
    };
    let interests =
        serde_json::to_value(payload.interests).unwrap_or_else(|_| JsonValue::Array(vec![]));
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into parent_intakes
          (id, workspace_id, classroom_id, child_nickname, age_group, interests, status, confirmed_child_id, created_at, updated_at)
        values ($1, $2, $3, $4, $5, $6, 'submitted', null, now(), now())
        "#,
        [
            Uuid::new_v4().into(),
            workspace_id.into(),
            classroom_id.into(),
            payload.child_nickname.into(),
            payload.age_group.into(),
            interests.into(),
        ],
    ))
    .await?;

    Ok(ActionResponse {
        status: "submitted".to_string(),
        message: format!("资料已提交给老师确认，包含 {interest_count} 个兴趣元素"),
    })
}

async fn ensure_workspace_exists(db: &DatabaseConnection, workspace_id: Uuid) -> Result<(), DbErr> {
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

async fn resolve_active_link_classroom_id(
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

async fn resolve_classroom_id(
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

fn optional_trimmed(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
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

async fn count_intakes_by_workspace(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    classroom: Option<&str>,
) -> Result<usize, DbErr> {
    let total: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select count(*) as count
            from parent_intakes pi
            left join classrooms c on c.id = pi.classroom_id and c.workspace_id = pi.workspace_id
            where pi.workspace_id = $1
              and ($2::text is null or c.name = $2)
            "#,
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

fn pagination_meta(total: usize, limit: usize, offset: usize) -> PaginationMeta {
    PaginationMeta {
        total,
        limit,
        offset: offset.min(total),
        has_more: offset.saturating_add(limit) < total,
    }
}

pub async fn list_page_by_workspace(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    classroom: Option<&str>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<(Vec<ParentIntake>, PaginationMeta), DbErr> {
    let limit = limit.unwrap_or(50).clamp(1, 100);
    let offset = offset.unwrap_or(0);
    let classroom_filter = optional_trimmed(classroom);
    let total = count_intakes_by_workspace(db, workspace_id, classroom_filter.as_deref()).await?;
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select pi.id, pi.workspace_id, pi.child_nickname, pi.age_group,
                   c.name as classroom, pi.interests, pi.status,
                   pi.confirmed_child_id, pi.created_at, pi.updated_at
            from parent_intakes pi
            left join classrooms c on c.id = pi.classroom_id and c.workspace_id = pi.workspace_id
            where pi.workspace_id = $1
              and ($4::text is null or c.name = $4)
            order by
              case pi.status when 'submitted' then 0 when 'confirmed' then 1 else 2 end,
              pi.created_at desc
            limit $2 offset $3
            "#,
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
            .map(intake_from_row)
            .collect::<Result<Vec<_>, _>>()?,
        pagination_meta(total, limit, offset),
    ))
}

pub async fn confirm(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    intake_id: Uuid,
    payload: ConfirmParentIntakeRequest,
) -> Result<ChildProfile, DbErr> {
    let intake = find_submitted(db, workspace_id, intake_id).await?;
    let derived = build_child_from_intake(&intake, payload);
    let child_id = Uuid::new_v4();
    let classroom_id = resolve_classroom_id(db, workspace_id, intake.classroom.as_deref()).await?;

    let child_row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into children
              (id, workspace_id, classroom_id, nickname, age_group, interests, traits, focus, completeness, status, created_at, updated_at)
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'active', now(), now())
            returning id, workspace_id, nickname, age_group, interests, traits, focus, completeness, status, updated_at
            "#,
            [
                child_id.into(),
                workspace_id.into(),
                classroom_id.into(),
                derived.nickname.into(),
                derived.age_group.into(),
                derived.interests.into(),
                derived.traits.into(),
                derived.focus.into(),
                derived.completeness.into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("child".to_string()))?;
    let inserted_child_id: Uuid = child_row.try_get("", "id")?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update parent_intakes
        set status = 'confirmed',
            confirmed_child_id = $3,
            updated_at = now()
        where workspace_id = $1 and id = $2 and status = 'submitted'
        "#,
        [workspace_id.into(), intake_id.into(), child_id.into()],
    ))
    .await?;

    crate::repositories::children::find(db, workspace_id, inserted_child_id).await
}

pub(crate) fn build_child_from_intake(
    intake: &ParentIntake,
    payload: ConfirmParentIntakeRequest,
) -> DerivedChild {
    let focus = payload
        .focus
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "家长提交资料，待老师补充关注点".to_string());
    let completeness = calculate_completeness(
        &intake.child_nickname,
        &intake.age_group,
        &intake.interests,
        &payload.traits,
        &focus,
    );
    DerivedChild {
        nickname: intake.child_nickname.clone(),
        age_group: intake.age_group.clone(),
        interests: serde_json::to_value(&intake.interests)
            .unwrap_or_else(|_| JsonValue::Array(vec![])),
        traits: serde_json::to_value(&payload.traits).unwrap_or_else(|_| JsonValue::Array(vec![])),
        focus,
        completeness,
    }
}

pub(crate) struct DerivedChild {
    pub nickname: String,
    pub age_group: String,
    pub interests: JsonValue,
    pub traits: JsonValue,
    pub focus: String,
    pub completeness: i32,
}

async fn find_submitted(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    intake_id: Uuid,
) -> Result<ParentIntake, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select pi.id, pi.workspace_id, pi.child_nickname, pi.age_group,
                   c.name as classroom, pi.interests, pi.status,
                   pi.confirmed_child_id, pi.created_at, pi.updated_at
            from parent_intakes pi
            left join classrooms c on c.id = pi.classroom_id and c.workspace_id = pi.workspace_id
            where pi.workspace_id = $1 and pi.id = $2 and pi.status = 'submitted'
            limit 1
            "#,
            [workspace_id.into(), intake_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("parent_intake".to_string()))?;

    intake_from_row(row)
}

fn intake_from_row(row: sea_orm::QueryResult) -> Result<ParentIntake, DbErr> {
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

fn link_from_row(row: sea_orm::QueryResult) -> Result<ParentIntakeLink, DbErr> {
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

fn public_link_from_row(row: sea_orm::QueryResult) -> Result<PublicParentIntakeLink, DbErr> {
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

fn json_string_array(value: JsonValue) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_intake() -> ParentIntake {
        ParentIntake {
            id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            child_nickname: "乐乐".to_string(),
            age_group: "4-5 岁".to_string(),
            classroom: None,
            interests: vec!["积木车".to_string(), "唱歌".to_string()],
            status: "submitted".to_string(),
            confirmed_child_id: None,
            created_at: "2026-07-19 10:00".to_string(),
            updated_at: "2026-07-19 10:00".to_string(),
        }
    }

    #[test]
    fn build_child_from_intake_uses_intake_data() {
        let intake = sample_intake();
        let derived = build_child_from_intake(
            &intake,
            ConfirmParentIntakeRequest {
                focus: Some("午睡适应".to_string()),
                traits: vec!["慢热".to_string(), "喜欢鼓励".to_string()],
            },
        );

        assert_eq!(derived.nickname, "乐乐");
        assert_eq!(derived.age_group, "4-5 岁");
        assert_eq!(derived.focus, "午睡适应");
        assert_eq!(
            derived.completeness,
            calculate_completeness(
                &intake.child_nickname,
                &intake.age_group,
                &intake.interests,
                &["慢热".to_string(), "喜欢鼓励".to_string()],
                "午睡适应",
            )
        );
    }

    #[test]
    fn build_child_from_intake_falls_back_to_default_focus() {
        let intake = sample_intake();
        let derived = build_child_from_intake(
            &intake,
            ConfirmParentIntakeRequest {
                focus: Some("   ".to_string()),
                traits: vec![],
            },
        );

        assert_eq!(derived.focus, "家长提交资料，待老师补充关注点");
    }
}
