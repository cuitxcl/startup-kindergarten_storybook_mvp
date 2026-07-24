use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

pub use super::children_mutations::{archive, create, restore, update};
pub use super::children_queries::{
    find, find_any_status_for_classrooms, find_for_classrooms, list_by_workspace,
    list_by_workspace_for_classrooms, list_page_by_workspace,
    list_page_by_workspace_for_classrooms,
};

use crate::models::ChildProfile;

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

pub(crate) async fn find_any_status(
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

pub(crate) async fn resolve_classroom_id(
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

pub(crate) fn child_from_row(row: sea_orm::QueryResult) -> Result<ChildProfile, DbErr> {
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

pub(crate) fn meaningful_count(items: &[String]) -> usize {
    items.iter().filter(|item| !item.trim().is_empty()).count()
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

pub(crate) async fn execute(db: &DatabaseConnection, sql: &str) -> Result<(), DbErr> {
    db.execute(Statement::from_string(DbBackend::Postgres, sql.to_string()))
        .await?;
    Ok(())
}
