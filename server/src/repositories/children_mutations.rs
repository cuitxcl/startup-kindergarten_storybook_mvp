use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::{
    models::{ChildProfile, CreateChildRequest, UpdateChildRequest},
    repositories::{
        children::{calculate_completeness, find_any_status, resolve_classroom_id},
        children_queries::find,
    },
};

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
