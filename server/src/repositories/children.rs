use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::models::{ChildProfile, CreateChildRequest, PaginationMeta, UpdateChildRequest};

pub async fn seed_demo_children(db: &DatabaseConnection) -> Result<(), DbErr> {
    for statement in [
        r#"
        insert into children
          (id, workspace_id, classroom_id, nickname, age_group, interests, traits, focus, completeness, status, created_at, updated_at)
        values
          ('30000000-0000-0000-0000-000000000001', '10000000-0000-0000-0000-000000000001', null, '乐乐', '4-5 岁', '["积木车", "蓝色", "小火车"]'::jsonb, '["热情", "需要练习等待"]'::jsonb, '轮流和表达需求', 92, 'active', now(), now())
        on conflict (id) do update
          set nickname = excluded.nickname,
              age_group = excluded.age_group,
              interests = excluded.interests,
              traits = excluded.traits,
              focus = excluded.focus,
              completeness = excluded.completeness,
              status = excluded.status,
              updated_at = now();
        "#,
        r#"
        insert into children
          (id, workspace_id, classroom_id, nickname, age_group, interests, traits, focus, completeness, status, created_at, updated_at)
        values
          ('30000000-0000-0000-0000-000000000002', '20000000-0000-0000-0000-000000000001', '80000000-0000-0000-0000-000000000001', '小雨', '3-4 岁', '["贴纸", "小兔", "唱歌"]'::jsonb, '["慢热", "喜欢被鼓励"]'::jsonb, '入园适应和午睡', 76, 'active', now(), now())
        on conflict (id) do update
          set nickname = excluded.nickname,
              age_group = excluded.age_group,
              classroom_id = excluded.classroom_id,
              interests = excluded.interests,
              traits = excluded.traits,
              focus = excluded.focus,
              completeness = excluded.completeness,
              status = excluded.status,
              updated_at = now();
        "#,
        r#"
        insert into children
          (id, workspace_id, classroom_id, nickname, age_group, interests, traits, focus, completeness, status, created_at, updated_at)
        values
          ('30000000-0000-0000-0000-000000000003', '20000000-0000-0000-0000-000000000002', '80000000-0000-0000-0000-000000000002', '安安', '4-5 岁', '["恐龙", "搭桥", "绿色"]'::jsonb, '["好奇", "表达直接"]'::jsonb, '排队等待', 84, 'active', now(), now())
        on conflict (id) do update
          set nickname = excluded.nickname,
              age_group = excluded.age_group,
              classroom_id = excluded.classroom_id,
              interests = excluded.interests,
              traits = excluded.traits,
              focus = excluded.focus,
              completeness = excluded.completeness,
              status = excluded.status,
              updated_at = now();
        "#,
    ] {
        execute(db, statement).await?;
    }

    Ok(())
}

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

pub async fn create(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    payload: CreateChildRequest,
) -> Result<ChildProfile, DbErr> {
    let id = Uuid::new_v4();
    let classroom_id = resolve_classroom_id(db, workspace_id, payload.classroom.as_deref()).await?;
    let completeness = calculate_completeness(
        &payload.nickname,
        &payload.age_group,
        &payload.interests,
        &payload.traits,
        &payload.focus,
    );
    let interests =
        serde_json::to_value(&payload.interests).unwrap_or_else(|_| JsonValue::Array(vec![]));
    let traits = serde_json::to_value(&payload.traits).unwrap_or_else(|_| JsonValue::Array(vec![]));
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into children
              (id, workspace_id, classroom_id, nickname, age_group, interests, traits, focus, completeness, status, created_at, updated_at)
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'active', now(), now())
            returning id
            "#,
            [
                id.into(),
                workspace_id.into(),
                classroom_id.into(),
                payload.nickname.into(),
                payload.age_group.into(),
                interests.into(),
                traits.into(),
                payload.focus.into(),
                completeness.into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("child".to_string()))?;

    find(db, workspace_id, row.try_get("", "id")?).await
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

pub async fn update(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    child_id: Uuid,
    payload: UpdateChildRequest,
) -> Result<ChildProfile, DbErr> {
    let mut child = find(db, workspace_id, child_id).await?;
    if let Some(value) = payload.nickname {
        child.nickname = value;
    }
    if let Some(value) = payload.age_group {
        child.age_group = value;
    }
    if let Some(value) = payload.interests {
        child.interests = value;
    }
    if let Some(value) = payload.traits {
        child.traits = value;
    }
    if let Some(value) = payload.focus {
        child.focus = value;
    }
    let classroom_id = if let Some(classroom) = payload.classroom {
        resolve_classroom_id(db, workspace_id, Some(&classroom)).await?
    } else {
        resolve_classroom_id(db, workspace_id, child.classroom.as_deref()).await?
    };

    let interests =
        serde_json::to_value(&child.interests).unwrap_or_else(|_| JsonValue::Array(vec![]));
    let traits = serde_json::to_value(&child.traits).unwrap_or_else(|_| JsonValue::Array(vec![]));
    let completeness = calculate_completeness(
        &child.nickname,
        &child.age_group,
        &child.interests,
        &child.traits,
        &child.focus,
    );
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update children
            set nickname = $3,
                age_group = $4,
                classroom_id = $5,
                interests = $6,
                traits = $7,
                focus = $8,
                completeness = $9,
                updated_at = now()
            where workspace_id = $1 and id = $2 and status = 'active'
            returning id
            "#,
            [
                workspace_id.into(),
                child_id.into(),
                child.nickname.into(),
                child.age_group.into(),
                classroom_id.into(),
                interests.into(),
                traits.into(),
                child.focus.into(),
                completeness.into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("child".to_string()))?;

    find(db, workspace_id, row.try_get("", "id")?).await
}

pub async fn archive(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    child_id: Uuid,
) -> Result<ChildProfile, DbErr> {
    let child = find_any_status(db, workspace_id, child_id).await?;
    if child.status != "active" {
        return Err(DbErr::Custom("child_not_active".to_string()));
    }

    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update children
            set status = 'archived',
                updated_at = now()
            where workspace_id = $1 and id = $2 and status = 'active'
            returning id
            "#,
            [workspace_id.into(), child_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::Custom("child_not_active".to_string()))?;

    find_any_status(db, workspace_id, row.try_get("", "id")?).await
}

pub async fn restore(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    child_id: Uuid,
) -> Result<ChildProfile, DbErr> {
    let child = find_any_status(db, workspace_id, child_id).await?;
    if child.status != "archived" {
        return Err(DbErr::Custom("child_not_archived".to_string()));
    }

    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update children
            set status = 'active',
                updated_at = now()
            where workspace_id = $1 and id = $2 and status = 'archived'
            returning id
            "#,
            [workspace_id.into(), child_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::Custom("child_not_archived".to_string()))?;

    find(db, workspace_id, row.try_get("", "id")?).await
}

async fn find_any_status(
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
            where ch.workspace_id = $1 and ch.id = $2
            limit 1
            "#,
            [workspace_id.into(), child_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("child".to_string()))?;

    child_from_row(row)
}

async fn resolve_classroom_id(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    classroom_name: Option<&str>,
) -> Result<Option<Uuid>, DbErr> {
    let Some(name) = classroom_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
    else {
        return Ok(None);
    };
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        select id
        from classrooms
        where workspace_id = $1 and name = $2 and status = 'active'
        limit 1
        "#,
        [workspace_id.into(), name.into()],
    ))
    .await?
    .ok_or_else(|| DbErr::RecordNotFound("classroom".to_string()))?
    .try_get("", "id")
    .map(Some)
}

fn child_from_row(row: sea_orm::QueryResult) -> Result<ChildProfile, DbErr> {
    let interests: JsonValue = row.try_get("", "interests")?;
    let traits: JsonValue = row.try_get("", "traits")?;
    let updated_at: DateTime<Utc> = row.try_get("", "updated_at")?;
    let completeness: i32 = row.try_get("", "completeness")?;
    Ok(ChildProfile {
        id: row.try_get("", "id")?,
        workspace_id: row.try_get("", "workspace_id")?,
        nickname: row.try_get("", "nickname")?,
        age_group: row.try_get("", "age_group")?,
        classroom: row.try_get("", "classroom")?,
        interests: json_string_array(interests),
        traits: json_string_array(traits),
        focus: row.try_get("", "focus")?,
        completeness: completeness.clamp(0, 100) as u8,
        status: row.try_get("", "status")?,
        updated_at: updated_at.format("%Y-%m-%d %H:%M").to_string(),
    })
}

pub(crate) fn calculate_completeness(
    nickname: &str,
    age_group: &str,
    interests: &[String],
    traits: &[String],
    focus: &str,
) -> i32 {
    let mut score = 0;
    if !nickname.trim().is_empty() {
        score += 15;
    }
    if !age_group.trim().is_empty() {
        score += 15;
    }
    if !focus.trim().is_empty() {
        score += 25;
    }
    let interest_count = meaningful_count(interests);
    if interest_count >= 1 {
        score += 15;
    }
    if interest_count >= 2 {
        score += 10;
    }
    let trait_count = meaningful_count(traits);
    if trait_count >= 1 {
        score += 10;
    }
    if trait_count >= 2 {
        score += 10;
    }
    score.min(100)
}

fn meaningful_count(items: &[String]) -> usize {
    items.iter().filter(|item| !item.trim().is_empty()).count()
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

async fn execute(db: &DatabaseConnection, sql: &str) -> Result<(), DbErr> {
    db.execute(Statement::from_string(DbBackend::Postgres, sql.to_string()))
        .await?;
    Ok(())
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
