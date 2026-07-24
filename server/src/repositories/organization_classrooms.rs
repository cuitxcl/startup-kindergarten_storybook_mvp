use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::{
    models::{Classroom, CreateClassroomRequest, PaginationMeta},
    repositories::organization::{classroom_from_row, pagination_meta},
};

pub async fn list_classrooms_page(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<(Vec<Classroom>, PaginationMeta), DbErr> {
    let limit = limit.unwrap_or(50).clamp(1, 100);
    let offset = offset.unwrap_or(0);
    let total = count_active_classrooms(db, workspace_id).await?;
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select c.id, c.workspace_id, c.name, coalesce(c.age_group, '') as age_group, c.status,
                   coalesce(child_counts.children, 0) as children
            from classrooms c
            left join (
              select classroom_id, count(*)::int as children
              from children
              where workspace_id = $1 and status = 'active' and classroom_id is not null
              group by classroom_id
            ) child_counts on child_counts.classroom_id = c.id
            where c.workspace_id = $1 and c.status = 'active'
            order by c.name
            limit $2 offset $3
            "#,
            [
                workspace_id.into(),
                (limit as i64).into(),
                (offset as i64).into(),
            ],
        ))
        .await?;
    Ok((
        rows.into_iter()
            .map(classroom_from_row)
            .collect::<Result<Vec<_>, _>>()?,
        pagination_meta(total, limit, offset),
    ))
}

pub async fn authorized_classrooms_for_user(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<String>, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select classroom_ids
            from workspace_members
            where workspace_id = $1
              and user_id = $2
              and status = 'active'
            limit 1
            "#,
            [workspace_id.into(), user_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("member".to_string()))?;
    let classes: JsonValue = row.try_get("", "classroom_ids")?;
    Ok(classes
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default())
}

pub async fn create_classroom(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    payload: CreateClassroomRequest,
) -> Result<Classroom, DbErr> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(DbErr::Custom("班级名称不能为空".to_string()));
    }
    let age_group = payload.age_group.trim();
    if age_group.is_empty() {
        return Err(DbErr::Custom("年龄段不能为空".to_string()));
    }
    ensure_classroom_name_available(db, workspace_id, name).await?;
    let id = Uuid::new_v4();
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into classrooms (id, workspace_id, name, age_group, status, created_at, updated_at)
            values ($1, $2, $3, $4, 'active', now(), now())
            returning id, workspace_id, name, coalesce(age_group, '') as age_group, status, 0::int as children
            "#,
            [
                id.into(),
                workspace_id.into(),
                name.into(),
                age_group.into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("classroom".to_string()))?;
    classroom_from_row(row)
}

pub async fn archive_classroom(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    classroom_id: Uuid,
) -> Result<Classroom, DbErr> {
    let classroom = find_classroom_by_id(db, workspace_id, classroom_id).await?;
    if classroom.status != "active" {
        return Err(DbErr::Custom("classroom_not_active".to_string()));
    }
    if classroom.children > 0 {
        return Err(DbErr::Custom("classroom_has_children".to_string()));
    }

    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update classrooms
            set status = 'archived',
                updated_at = now()
            where id = $1 and workspace_id = $2 and status = 'active'
            returning id, workspace_id, name, coalesce(age_group, '') as age_group, status, 0::int as children
            "#,
            [classroom_id.into(), workspace_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("classroom".to_string()))?;
    classroom_from_row(row)
}

async fn count_active_classrooms(
    db: &DatabaseConnection,
    workspace_id: Uuid,
) -> Result<usize, DbErr> {
    let total: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "select count(*) as count from classrooms where workspace_id = $1 and status = 'active'",
            [workspace_id.into()],
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);
    Ok(total.max(0) as usize)
}

async fn find_classroom_by_id(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    classroom_id: Uuid,
) -> Result<Classroom, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select c.id, c.workspace_id, c.name, coalesce(c.age_group, '') as age_group, c.status,
                   coalesce(child_counts.children, 0) as children
            from classrooms c
            left join (
              select classroom_id, count(*)::int as children
              from children
              where workspace_id = $1 and status = 'active' and classroom_id is not null
              group by classroom_id
            ) child_counts on child_counts.classroom_id = c.id
            where c.workspace_id = $1 and c.id = $2
            limit 1
            "#,
            [workspace_id.into(), classroom_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("classroom".to_string()))?;
    classroom_from_row(row)
}

pub(crate) async fn ensure_classrooms_exist(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    class_names: &[String],
) -> Result<(), DbErr> {
    if class_names.is_empty() {
        return Ok(());
    }
    let class_names_json =
        serde_json::to_value(class_names).unwrap_or_else(|_| JsonValue::Array(vec![]));
    let found_count: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select count(*) as count
            from classrooms
            where workspace_id = $1
              and status = 'active'
              and name in (select value from jsonb_array_elements_text($2::jsonb))
            "#,
            [workspace_id.into(), class_names_json.into()],
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);

    if found_count as usize == class_names.len() {
        Ok(())
    } else {
        Err(DbErr::RecordNotFound("classroom".to_string()))
    }
}

async fn ensure_classroom_name_available(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    name: &str,
) -> Result<(), DbErr> {
    let exists: bool = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select exists(
              select 1
              from classrooms
              where workspace_id = $1
                and name = $2
            ) as exists
            "#,
            [workspace_id.into(), name.into()],
        ))
        .await?
        .and_then(|row| row.try_get("", "exists").ok())
        .unwrap_or(false);

    if exists {
        Err(DbErr::Custom("classroom_exists".to_string()))
    } else {
        Ok(())
    }
}
