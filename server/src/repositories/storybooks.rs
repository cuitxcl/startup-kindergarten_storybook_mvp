use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use uuid::Uuid;

use crate::models::{
    CreateStorybookRequest, DeriveCustomRequest, MarketplaceTemplate, PaginationMeta, Storybook,
    StorybookListQuery, StorybookPage, StorybookRole, StorybookStatus, UpdatePageRequest,
    UpdateRoleRequest, UpdateStorybookRequest,
};
use crate::repositories::storybook_rules::{
    ensure_deliverable_ready, ensure_status_transition, ensure_teacher_review_ready,
    storybook_status_name, visibility_name,
};

pub async fn seed_demo_storybooks(db: &DatabaseConnection) -> Result<(), DbErr> {
    for (
        id,
        workspace_id,
        title,
        storybook_type,
        status,
        visibility,
        source,
        teaching_goal,
        use_scene,
    ) in [
        (
            "40000000-0000-0000-0000-000000000001",
            "10000000-0000-0000-0000-000000000001",
            "一起玩小汽车",
            "plain",
            "exportable",
            "private",
            "blank",
            "学习轮流与分享",
            "规则引导",
        ),
        (
            "40000000-0000-0000-0000-000000000002",
            "10000000-0000-0000-0000-000000000001",
            "乐乐学会一起玩",
            "custom",
            "editing",
            "private",
            "derived:balanced",
            "把轮流等待迁移到家庭场景",
            "家庭共读",
        ),
        (
            "40000000-0000-0000-0000-000000000003",
            "20000000-0000-0000-0000-000000000001",
            "午睡小小约定",
            "plain",
            "submitted",
            "market_submission",
            "blank",
            "建立睡前整理和安静入睡流程",
            "午睡习惯",
        ),
        (
            "40000000-0000-0000-0000-000000000004",
            "20000000-0000-0000-0000-000000000002",
            "排队像小火车",
            "plain",
            "exportable",
            "workspace",
            "blank",
            "理解排队和等待",
            "规则引导",
        ),
    ] {
        execute(
            db,
            &format!(
                r#"
                insert into storybooks
                  (id, workspace_id, storybook_type, status, visibility, source, title, age_group, use_scene, teaching_goal, cover_tone, creator_id, created_at, updated_at)
                values
                  ('{id}', '{workspace_id}', '{storybook_type}', '{status}', '{visibility}', '{source}', '{title}', '4-5 岁', '{use_scene}', '{teaching_goal}', '温暖、清楚', '00000000-0000-0000-0000-000000000001', now(), now())
                on conflict (id) do update
                  set status = excluded.status,
                      visibility = excluded.visibility,
                      teaching_goal = excluded.teaching_goal,
                      updated_at = now();
                "#
            ),
        )
        .await?;

        crate::repositories::storybook_factory::seed_default_pages_and_roles(db, uuid(id)?).await?;
    }

    Ok(())
}

pub async fn list_by_workspace(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    query: StorybookListQuery,
) -> Result<(Vec<Storybook>, PaginationMeta), DbErr> {
    crate::repositories::storybook_queries::list_by_workspace(db, workspace_id, query).await
}

pub async fn create_plain(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    payload: CreateStorybookRequest,
) -> Result<Storybook, DbErr> {
    crate::repositories::storybook_factory::create_plain(db, workspace_id, payload).await
}

pub async fn create_from_marketplace_template(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    template: MarketplaceTemplate,
) -> Result<Storybook, DbErr> {
    crate::repositories::storybook_factory::create_from_marketplace_template(
        db,
        workspace_id,
        template,
    )
    .await
}

pub async fn find(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<Storybook, DbErr> {
    crate::repositories::storybook_queries::find(db, workspace_id, storybook_id).await
}

pub async fn find_any(db: &DatabaseConnection, storybook_id: Uuid) -> Result<Storybook, DbErr> {
    crate::repositories::storybook_queries::find_any(db, storybook_id).await
}

pub async fn update(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    payload: UpdateStorybookRequest,
    actor_user_id: Uuid,
) -> Result<Storybook, DbErr> {
    let mut book = find(db, workspace_id, storybook_id).await?;
    if let Some(value) = payload.title {
        book.title = value;
    }
    if let Some(value) = payload.status {
        ensure_status_transition(&book.status, &value)?;
        if value == StorybookStatus::Exportable {
            ensure_deliverable_ready(&book)?;
        }
        book.status = value;
    }
    if let Some(value) = payload.visibility {
        book.visibility = value;
    }
    let teacher_review_status_changed = payload.teacher_review_status.is_some();
    if let Some(value) = payload.teacher_review_status {
        book.teacher_review_status = value;
        if book.teacher_review_status == "confirmed" {
            ensure_teacher_review_ready(&book)?;
            book.teacher_reviewed_by = Some(actor_user_id);
            book.teacher_reviewed_at = Some("now".to_string());
        } else {
            book.teacher_reviewed_by = None;
            book.teacher_reviewed_at = None;
        }
    }
    if let Some(value) = payload.age_group {
        book.age_group = value;
    }
    if let Some(value) = payload.use_scene {
        book.use_scene = value;
    }
    if let Some(value) = payload.teaching_goal {
        book.teaching_goal = value;
    }
    if let Some(value) = payload.cover_tone {
        book.cover_tone = value;
    }
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybooks
        set title = $3,
            status = $4,
            visibility = $5,
            age_group = $6,
            use_scene = $7,
            teaching_goal = $8,
            cover_tone = $9,
            teacher_review_status = case when $12::boolean then $10::text else teacher_review_status end,
            teacher_reviewed_by = case when $12::boolean then case when $10::text = 'confirmed' then $11::uuid else null end else teacher_reviewed_by end,
            teacher_reviewed_at = case when $12::boolean then case when $10::text = 'confirmed' then now() else null end else teacher_reviewed_at end,
            updated_at = now()
        where workspace_id = $1 and id = $2
        "#,
        [
            workspace_id.into(),
            storybook_id.into(),
            book.title.clone().into(),
            storybook_status_name(&book.status).into(),
            visibility_name(&book.visibility).into(),
            book.age_group.clone().into(),
            book.use_scene.clone().into(),
            book.teaching_goal.clone().into(),
            book.cover_tone.clone().into(),
            book.teacher_review_status.clone().into(),
            actor_user_id.into(),
            teacher_review_status_changed.into(),
        ],
    ))
    .await?;
    find(db, workspace_id, storybook_id).await
}

pub async fn duplicate(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    requested_title: Option<String>,
) -> Result<Storybook, DbErr> {
    crate::repositories::storybook_factory::duplicate(
        db,
        workspace_id,
        storybook_id,
        requested_title,
    )
    .await
}

pub async fn update_page(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    page_id: Uuid,
    payload: UpdatePageRequest,
) -> Result<StorybookPage, DbErr> {
    crate::repositories::storybook_editing::update_page(
        db,
        workspace_id,
        storybook_id,
        page_id,
        payload,
    )
    .await
}

pub async fn update_role(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    role_id: Uuid,
    payload: UpdateRoleRequest,
) -> Result<StorybookRole, DbErr> {
    crate::repositories::storybook_editing::update_role(
        db,
        workspace_id,
        storybook_id,
        role_id,
        payload,
    )
    .await
}

pub async fn derive_custom(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    source_storybook_id: Uuid,
    payload: DeriveCustomRequest,
) -> Result<Storybook, DbErr> {
    crate::repositories::storybook_customization::derive_custom(
        db,
        workspace_id,
        source_storybook_id,
        payload,
    )
    .await
}

fn uuid(value: &str) -> Result<Uuid, DbErr> {
    Uuid::parse_str(value).map_err(|err| DbErr::Custom(err.to_string()))
}

async fn execute(db: &DatabaseConnection, sql: &str) -> Result<(), DbErr> {
    db.execute(Statement::from_string(DbBackend::Postgres, sql.to_string()))
        .await?;
    Ok(())
}
