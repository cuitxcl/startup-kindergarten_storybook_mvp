use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use uuid::Uuid;

use crate::models::{StorybookPage, StorybookRole, UpdatePageRequest, UpdateRoleRequest};
use crate::repositories::storybook_rules::role_edit_requires_page_regeneration;

pub async fn update_page(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    page_id: Uuid,
    payload: UpdatePageRequest,
) -> Result<StorybookPage, DbErr> {
    let book = crate::repositories::storybook_queries::find(db, workspace_id, storybook_id).await?;
    let mut page = book
        .pages
        .into_iter()
        .find(|page| page.id == page_id)
        .ok_or_else(|| DbErr::RecordNotFound("page".to_string()))?;
    if let Some(value) = payload.title {
        page.title = value;
    }
    if let Some(value) = payload.body {
        page.body = value;
    }
    if let Some(value) = payload.illustration_prompt {
        page.illustration_prompt = value;
    }
    if let Some(value) = payload.status {
        page.status = value;
    }
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_pages
        set title = $3,
            body = $4,
            illustration_prompt = $5,
            status = $6
        where storybook_id = $1 and id = $2
        "#,
        [
            storybook_id.into(),
            page_id.into(),
            page.title.clone().into(),
            page.body.clone().into(),
            page.illustration_prompt.clone().into(),
            page.status.clone().into(),
        ],
    ))
    .await?;
    touch_storybook(db, workspace_id, storybook_id).await?;
    Ok(page)
}

pub async fn update_role(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    role_id: Uuid,
    payload: UpdateRoleRequest,
) -> Result<StorybookRole, DbErr> {
    let mut role = crate::repositories::storybook_queries::find(db, workspace_id, storybook_id)
        .await?
        .roles
        .into_iter()
        .find(|role| role.id == role_id)
        .ok_or_else(|| DbErr::RecordNotFound("role".to_string()))?;
    let old_role = role.clone();
    let explicit_reference_status = payload.reference_status.is_some();

    if let Some(value) = payload.name {
        role.name = value;
    }
    if let Some(value) = payload.role_type {
        role.role_type = value;
    }
    if let Some(value) = payload.appearance {
        role.appearance = value;
    }
    if let Some(value) = payload.story_function {
        role.story_function = value;
    }
    if let Some(value) = payload.needs_consistency {
        role.needs_consistency = value;
    }
    if payload.reference_image_url.is_some() {
        role.reference_image_url = clean_optional_text(payload.reference_image_url);
    }
    if payload.reference_image_prompt.is_some() {
        role.reference_image_prompt = clean_optional_text(payload.reference_image_prompt);
    }
    if let Some(value) = payload.reference_status {
        role.reference_status = value;
    }
    let should_mark_pages_for_regeneration = role_edit_requires_page_regeneration(&old_role, &role);
    if should_mark_pages_for_regeneration && !explicit_reference_status {
        role.reference_status = if role.reference_image_url.is_some() {
            "needs_regeneration".to_string()
        } else {
            "not_started".to_string()
        };
    }

    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update storybook_roles
            set name = $3,
                role_type = $4,
                appearance = $5,
                story_function = $6,
                needs_consistency = $7,
                reference_image_url = $8,
                reference_image_prompt = $9,
                reference_status = $10
            where storybook_id = $1 and id = $2
            returning id, name, role_type, appearance, coalesce(story_function, '') as story_function, needs_consistency,
                      reference_image_url, reference_image_prompt, reference_status
            "#,
            [
                storybook_id.into(),
                role_id.into(),
                role.name.clone().into(),
                role.role_type.clone().into(),
                role.appearance.clone().into(),
                role.story_function.clone().into(),
                role.needs_consistency.into(),
                role.reference_image_url.clone().into(),
                role.reference_image_prompt.clone().into(),
                role.reference_status.clone().into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("role".to_string()))?;
    touch_storybook(db, workspace_id, storybook_id).await?;
    if should_mark_pages_for_regeneration {
        mark_storybook_pages_need_regeneration(db, storybook_id).await?;
    }

    Ok(StorybookRole {
        id: row.try_get("", "id")?,
        name: row.try_get("", "name")?,
        role_type: row.try_get("", "role_type")?,
        appearance: row.try_get("", "appearance")?,
        story_function: row.try_get("", "story_function")?,
        needs_consistency: row.try_get("", "needs_consistency")?,
        reference_image_url: row.try_get("", "reference_image_url")?,
        reference_image_prompt: row.try_get("", "reference_image_prompt")?,
        reference_status: row.try_get("", "reference_status")?,
    })
}

async fn mark_storybook_pages_need_regeneration(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_pages
        set status = 'needs_regeneration'
        where storybook_id = $1
          and status not in ('generating', 'needs_regeneration')
        "#,
        [storybook_id.into()],
    ))
    .await?;
    Ok(())
}

async fn touch_storybook(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "update storybooks set updated_at = now() where workspace_id = $1 and id = $2",
        [workspace_id.into(), storybook_id.into()],
    ))
    .await?;
    Ok(())
}

fn clean_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
