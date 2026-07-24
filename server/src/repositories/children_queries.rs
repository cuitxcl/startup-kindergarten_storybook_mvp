use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::{
    models::{ChildProfile, PaginationMeta},
    repositories::children::{child_from_row, find_any_status},
};

pub async fn list_by_workspace(
    db: &DatabaseConnection,
    workspace_id: Uuid,
) -> Result<Vec<ChildProfile>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select ch.id, ch.workspace_id, ch.nickname, ch.age_group, c.name as classroom,
                   ch.interests, ch.traits, ch.focus, ch.completeness, ch.status, ch.updated_at
            from children ch
            left join classrooms c on c.id = ch.classroom_id and c.workspace_id = ch.workspace_id
            where ch.workspace_id = $1 and ch.status = 'active'
            order by ch.updated_at desc, ch.nickname
            "#,
            [workspace_id.into()],
        ))
        .await?;

    rows.into_iter().map(child_from_row).collect()
}

pub async fn list_page_by_workspace(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<(Vec<ChildProfile>, PaginationMeta), DbErr> {
    query_children_page(db, workspace_id, None, limit, offset).await
}

pub async fn list_by_workspace_for_classrooms(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    classroom_names: &[String],
) -> Result<Vec<ChildProfile>, DbErr> {
    if classroom_names.is_empty() {
        return Ok(vec![]);
    }
    let children = list_by_workspace(db, workspace_id).await?;
    Ok(filter_children_by_classrooms(children, classroom_names))
}

pub async fn list_page_by_workspace_for_classrooms(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    classroom_names: &[String],
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<(Vec<ChildProfile>, PaginationMeta), DbErr> {
    if classroom_names.is_empty() {
        let limit = limit.unwrap_or(50).clamp(1, 100);
        return Ok((
            Vec::new(),
            PaginationMeta {
                total: 0,
                limit,
                offset: 0,
                has_more: false,
            },
        ));
    }
    query_children_page(db, workspace_id, Some(classroom_names), limit, offset).await
}

pub(crate) fn filter_children_by_classrooms(
    children: Vec<ChildProfile>,
    classroom_names: &[String],
) -> Vec<ChildProfile> {
    children
        .into_iter()
        .filter(|child| {
            child
                .classroom
                .as_ref()
                .is_some_and(|name| classroom_names.iter().any(|item| item == name))
        })
        .collect()
}

async fn query_children_page(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    classroom_names: Option<&[String]>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<(Vec<ChildProfile>, PaginationMeta), DbErr> {
    let limit = limit.unwrap_or(50).clamp(1, 100);
    let offset = offset.unwrap_or(0);
    let classroom_filter = classroom_names
        .map(|names| serde_json::to_value(names).unwrap_or_else(|_| JsonValue::Array(vec![])));

    let total: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select count(*) as count
            from children ch
            left join classrooms c on c.id = ch.classroom_id and c.workspace_id = ch.workspace_id
            where ch.workspace_id = $1
              and ch.status = 'active'
              and (
                $2::jsonb is null
                or c.name in (select value from jsonb_array_elements_text($2::jsonb))
              )
            "#,
            [workspace_id.into(), classroom_filter.clone().into()],
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);

    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select ch.id, ch.workspace_id, ch.nickname, ch.age_group, c.name as classroom,
                   ch.interests, ch.traits, ch.focus, ch.completeness, ch.status, ch.updated_at
            from children ch
            left join classrooms c on c.id = ch.classroom_id and c.workspace_id = ch.workspace_id
            where ch.workspace_id = $1
              and ch.status = 'active'
              and (
                $2::jsonb is null
                or c.name in (select value from jsonb_array_elements_text($2::jsonb))
              )
            order by ch.updated_at desc, ch.nickname
            limit $3 offset $4
            "#,
            [
                workspace_id.into(),
                classroom_filter.into(),
                (limit as i64).into(),
                (offset as i64).into(),
            ],
        ))
        .await?;

    let total = total.max(0) as usize;
    Ok((
        rows.into_iter()
            .map(child_from_row)
            .collect::<Result<Vec<_>, _>>()?,
        PaginationMeta {
            total,
            limit,
            offset: offset.min(total),
            has_more: offset.saturating_add(limit) < total,
        },
    ))
}

pub async fn find(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    child_id: Uuid,
) -> Result<ChildProfile, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select ch.id, ch.workspace_id, ch.nickname, ch.age_group, c.name as classroom,
                   ch.interests, ch.traits, ch.focus, ch.completeness, ch.status, ch.updated_at
            from children ch
            left join classrooms c on c.id = ch.classroom_id and c.workspace_id = ch.workspace_id
            where ch.workspace_id = $1 and ch.id = $2 and ch.status = 'active'
            limit 1
            "#,
            [workspace_id.into(), child_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("child".to_string()))?;

    child_from_row(row)
}

pub async fn find_for_classrooms(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    child_id: Uuid,
    classroom_names: &[String],
) -> Result<ChildProfile, DbErr> {
    let child = find(db, workspace_id, child_id).await?;
    let allowed = child
        .classroom
        .as_ref()
        .is_some_and(|name| classroom_names.iter().any(|item| item == name));
    if allowed {
        Ok(child)
    } else {
        Err(DbErr::RecordNotFound("child".to_string()))
    }
}

pub async fn find_any_status_for_classrooms(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    child_id: Uuid,
    classroom_names: &[String],
) -> Result<ChildProfile, DbErr> {
    let child = find_any_status(db, workspace_id, child_id).await?;
    let allowed = child
        .classroom
        .as_ref()
        .is_some_and(|name| classroom_names.iter().any(|item| item == name));
    if allowed {
        Ok(child)
    } else {
        Err(DbErr::RecordNotFound("child".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn child(nickname: &str, classroom: Option<&str>) -> ChildProfile {
        ChildProfile {
            id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            nickname: nickname.to_string(),
            age_group: "4-5 岁".to_string(),
            classroom: classroom.map(ToString::to_string),
            interests: vec![],
            traits: vec![],
            focus: "规则引导".to_string(),
            completeness: 70,
            status: "active".to_string(),
            updated_at: "2026-07-19 10:00".to_string(),
        }
    }

    #[test]
    fn filter_children_by_classrooms_keeps_only_authorized_classes() {
        let children = vec![
            child("小雨", Some("小一班")),
            child("安安", Some("中一班")),
            child("未分班", None),
        ];
        let allowed = vec!["小一班".to_string()];

        let scoped = filter_children_by_classrooms(children, &allowed);

        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].nickname, "小雨");
    }
}
