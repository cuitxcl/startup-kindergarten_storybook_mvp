use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::models::{
    CreateStorybookRequest, DeriveCustomRequest, MarketplaceTemplate, PaginationMeta, Storybook,
    StorybookListQuery, StorybookPage, StorybookRole, StorybookStatus, StorybookType,
    UpdatePageRequest, UpdateRoleRequest, UpdateStorybookRequest, Visibility,
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

        seed_default_pages_and_roles(db, uuid(id)?).await?;
    }

    Ok(())
}

pub async fn list_by_workspace(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    query: StorybookListQuery,
) -> Result<(Vec<Storybook>, PaginationMeta), DbErr> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0);
    let q_filter = query
        .q
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let total: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select count(*) as count
            from storybooks s
            where s.workspace_id = $1
              and ($2::text is null or s.storybook_type = $2)
              and ($3::text is null or s.status = $3)
              and ($4::uuid is null or s.target_child_id = $4)
              and (
                $5::text is null
                or s.title ilike '%' || $5 || '%'
                or coalesce(s.teaching_goal, '') ilike '%' || $5 || '%'
              )
            "#,
            [
                workspace_id.into(),
                query.storybook_type.clone().into(),
                query.status.clone().into(),
                query.target_child_id.into(),
                q_filter.clone().into(),
            ],
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);

    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select
              s.id, s.workspace_id, s.storybook_type, s.status, s.visibility, s.source,
              s.source_storybook_id, s.target_child_id, s.title, coalesce(s.age_group, '') as age_group,
              coalesce(s.use_scene, '') as use_scene, coalesce(s.teaching_goal, '') as teaching_goal,
              coalesce(s.cover_tone, '') as cover_tone, s.updated_at,
              coalesce(u.display_name, '林老师') as creator_name,
              source.title as source_title
            from storybooks s
            left join users u on u.id = s.creator_id
            left join storybooks source on source.id = s.source_storybook_id
            where s.workspace_id = $1
              and ($2::text is null or s.storybook_type = $2)
              and ($3::text is null or s.status = $3)
              and ($4::uuid is null or s.target_child_id = $4)
              and (
                $5::text is null
                or s.title ilike '%' || $5 || '%'
                or coalesce(s.teaching_goal, '') ilike '%' || $5 || '%'
              )
            order by s.updated_at desc, s.title
            limit $6 offset $7
            "#,
            [
                workspace_id.into(),
                query.storybook_type.into(),
                query.status.into(),
                query.target_child_id.into(),
                q_filter.into(),
                (limit as i64).into(),
                (offset as i64).into(),
            ],
        ))
        .await?;

    let items = storybooks_from_rows(db, rows).await?;
    let total = total.max(0) as usize;
    Ok((
        items,
        PaginationMeta {
            total,
            limit,
            offset: offset.min(total),
            has_more: offset.saturating_add(limit) < total,
        },
    ))
}

pub async fn create_plain(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    payload: CreateStorybookRequest,
) -> Result<Storybook, DbErr> {
    let storybook_id = Uuid::new_v4();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybooks
          (id, workspace_id, storybook_type, status, visibility, source, title, age_group, use_scene, teaching_goal, cover_tone, creator_id, created_at, updated_at)
        values ($1, $2, 'plain', 'plan_pending', 'private', 'blank', $3, $4, $5, $6, '温暖、清楚', '00000000-0000-0000-0000-000000000001', now(), now())
        "#,
        [
            storybook_id.into(),
            workspace_id.into(),
            payload.title.into(),
            payload.age_group.into(),
            payload.use_scene.into(),
            payload.teaching_goal.into(),
        ],
    ))
    .await?;
    seed_default_pages_and_roles(db, storybook_id).await?;
    find(db, workspace_id, storybook_id).await
}

pub async fn create_from_marketplace_template(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    template: MarketplaceTemplate,
) -> Result<Storybook, DbErr> {
    let storybook_id = Uuid::new_v4();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybooks
          (id, workspace_id, storybook_type, status, visibility, source, title, age_group, use_scene, teaching_goal, cover_tone, source_storybook_id, creator_id, created_at, updated_at)
        values ($1, $2, 'plain', 'draft', 'private', 'marketplace', $3, $4, $5, $6, '柔和、安静', $7, '00000000-0000-0000-0000-000000000001', now(), now())
        "#,
        [
            storybook_id.into(),
            workspace_id.into(),
            template.title.clone().into(),
            template.age_group.clone().into(),
            template.use_scene.clone().into(),
            template.summary.clone().into(),
            template.source_storybook_id.into(),
        ],
    ))
    .await?;
    if let Some(source_storybook_id) = template.source_storybook_id {
        clone_pages_and_roles(db, source_storybook_id, storybook_id).await?;
    } else {
        seed_default_pages_and_roles(db, storybook_id).await?;
    }
    find(db, workspace_id, storybook_id).await
}

pub async fn find(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<Storybook, DbErr> {
    query_storybooks(db, Some(workspace_id))
        .await?
        .into_iter()
        .find(|book| book.id == storybook_id)
        .ok_or_else(|| DbErr::RecordNotFound("storybook".to_string()))
}

pub async fn find_any(db: &DatabaseConnection, storybook_id: Uuid) -> Result<Storybook, DbErr> {
    query_storybooks(db, None)
        .await?
        .into_iter()
        .find(|book| book.id == storybook_id)
        .ok_or_else(|| DbErr::RecordNotFound("storybook".to_string()))
}

pub async fn update(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    payload: UpdateStorybookRequest,
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
        ],
    ))
    .await?;
    find(db, workspace_id, storybook_id).await
}

pub async fn duplicate(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<Storybook, DbErr> {
    let source = find(db, workspace_id, storybook_id).await?;
    let new_id = Uuid::new_v4();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybooks
          (id, workspace_id, storybook_type, status, visibility, source, source_storybook_id, target_child_id, title, age_group, use_scene, teaching_goal, cover_tone, creator_id, created_at, updated_at)
        values ($1, $2, $3, 'draft', 'private', 'duplicate', $4, $5, $6, $7, $8, $9, $10, '00000000-0000-0000-0000-000000000001', now(), now())
        "#,
        [
            new_id.into(),
            workspace_id.into(),
            storybook_type_name(&source.storybook_type).into(),
            storybook_id.into(),
            source.target_child_id.into(),
            format!("{} 副本", source.title).into(),
            source.age_group.into(),
            source.use_scene.into(),
            source.teaching_goal.into(),
            source.cover_tone.into(),
        ],
    ))
    .await?;
    clone_pages_and_roles(db, storybook_id, new_id).await?;
    find(db, workspace_id, new_id).await
}

pub async fn update_page(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    page_id: Uuid,
    payload: UpdatePageRequest,
) -> Result<StorybookPage, DbErr> {
    let book = find(db, workspace_id, storybook_id).await?;
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
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "update storybooks set updated_at = now() where workspace_id = $1 and id = $2",
        [workspace_id.into(), storybook_id.into()],
    ))
    .await?;
    Ok(page)
}

pub async fn update_role(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    role_id: Uuid,
    payload: UpdateRoleRequest,
) -> Result<StorybookRole, DbErr> {
    let mut role = find(db, workspace_id, storybook_id)
        .await?
        .roles
        .into_iter()
        .find(|role| role.id == role_id)
        .ok_or_else(|| DbErr::RecordNotFound("role".to_string()))?;

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

    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update storybook_roles
            set name = $3,
                role_type = $4,
                appearance = $5,
                story_function = $6,
                needs_consistency = $7
            where storybook_id = $1 and id = $2
            returning id, name, role_type, appearance, coalesce(story_function, '') as story_function, needs_consistency
            "#,
            [
                storybook_id.into(),
                role_id.into(),
                role.name.clone().into(),
                role.role_type.clone().into(),
                role.appearance.clone().into(),
                role.story_function.clone().into(),
                role.needs_consistency.into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("role".to_string()))?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "update storybooks set updated_at = now() where workspace_id = $1 and id = $2",
        [workspace_id.into(), storybook_id.into()],
    ))
    .await?;

    Ok(StorybookRole {
        id: row.try_get("", "id")?,
        name: row.try_get("", "name")?,
        role_type: row.try_get("", "role_type")?,
        appearance: row.try_get("", "appearance")?,
        story_function: row.try_get("", "story_function")?,
        needs_consistency: row.try_get("", "needs_consistency")?,
    })
}

pub async fn derive_custom(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    source_storybook_id: Uuid,
    payload: DeriveCustomRequest,
) -> Result<Storybook, DbErr> {
    let source = find(db, workspace_id, source_storybook_id).await?;
    if source.storybook_type != StorybookType::Plain {
        return Err(DbErr::Custom("只有普通绘本可以派生定制绘本".to_string()));
    }
    let child = child_profile_for_custom(db, workspace_id, payload.child_id).await?;
    let plan_strategy = customization_strategy(payload.customization_plan.as_ref());
    let customization =
        build_custom_storybook_customization(&source, &child, &payload.intensity, plan_strategy);
    let new_id = Uuid::new_v4();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybooks
          (id, workspace_id, storybook_type, status, visibility, source, source_storybook_id, target_child_id, title, age_group, use_scene, teaching_goal, cover_tone, creator_id, created_at, updated_at)
        values ($1, $2, 'custom', 'editing', 'private', $3, $4, $5, $6, $7, $8, $9, $10, '00000000-0000-0000-0000-000000000001', now(), now())
        "#,
        [
            new_id.into(),
            workspace_id.into(),
            customization.source.into(),
            source_storybook_id.into(),
            payload.child_id.into(),
            customization.title.into(),
            customization.age_group.into(),
            customization.use_scene.into(),
            customization.teaching_goal.into(),
            customization.cover_tone.into(),
        ],
    ))
    .await?;
    clone_pages_and_roles(db, source_storybook_id, new_id).await?;
    apply_child_customization(db, new_id, &child, &payload.intensity).await?;
    find(db, workspace_id, new_id).await
}

fn build_custom_storybook_customization(
    source: &Storybook,
    child: &CustomChildProfile,
    intensity: &str,
    plan_strategy: Option<String>,
) -> CustomStorybookCustomization {
    let mut teaching_goal = source.teaching_goal.clone();
    if let Some(strategy) = plan_strategy {
        teaching_goal.push_str(&format!("；定制方案：{strategy}"));
    }

    CustomStorybookCustomization {
        source: format!("derived:{intensity}"),
        title: format!("{}的定制故事", child.nickname),
        age_group: source.age_group.clone(),
        use_scene: source.use_scene.clone(),
        teaching_goal,
        cover_tone: source.cover_tone.clone(),
    }
}

fn customization_strategy(plan: Option<&JsonValue>) -> Option<String> {
    plan.and_then(|value| {
        value
            .get("customization")
            .and_then(|customization| customization.get("strategy"))
            .or_else(|| value.get("strategy"))
            .and_then(|strategy| strategy.as_str())
            .filter(|strategy| !strategy.trim().is_empty())
            .map(|strategy| strategy.trim().to_string())
    })
}

pub(crate) struct CustomStorybookCustomization {
    pub source: String,
    pub title: String,
    pub age_group: String,
    pub use_scene: String,
    pub teaching_goal: String,
    pub cover_tone: String,
}

async fn query_storybooks(
    db: &DatabaseConnection,
    workspace_id: Option<Uuid>,
) -> Result<Vec<Storybook>, DbErr> {
    let rows = if let Some(workspace_id) = workspace_id {
        db.query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select
              s.id, s.workspace_id, s.storybook_type, s.status, s.visibility, s.source,
              s.source_storybook_id, s.target_child_id, s.title, coalesce(s.age_group, '') as age_group,
              coalesce(s.use_scene, '') as use_scene, coalesce(s.teaching_goal, '') as teaching_goal,
              coalesce(s.cover_tone, '') as cover_tone, s.updated_at,
              coalesce(u.display_name, '林老师') as creator_name,
              source.title as source_title
            from storybooks s
            left join users u on u.id = s.creator_id
            left join storybooks source on source.id = s.source_storybook_id
            where s.workspace_id = $1
            order by s.updated_at desc, s.title
            "#,
            [workspace_id.into()],
        ))
        .await?
    } else {
        db.query_all(Statement::from_string(
            DbBackend::Postgres,
            r#"
            select
              s.id, s.workspace_id, s.storybook_type, s.status, s.visibility, s.source,
              s.source_storybook_id, s.target_child_id, s.title, coalesce(s.age_group, '') as age_group,
              coalesce(s.use_scene, '') as use_scene, coalesce(s.teaching_goal, '') as teaching_goal,
              coalesce(s.cover_tone, '') as cover_tone, s.updated_at,
              coalesce(u.display_name, '林老师') as creator_name,
              source.title as source_title
            from storybooks s
            left join users u on u.id = s.creator_id
            left join storybooks source on source.id = s.source_storybook_id
            order by s.updated_at desc, s.title
            "#
            .to_string(),
        ))
        .await?
    };

    storybooks_from_rows(db, rows).await
}

async fn storybooks_from_rows(
    db: &DatabaseConnection,
    rows: Vec<sea_orm::QueryResult>,
) -> Result<Vec<Storybook>, DbErr> {
    let mut books = Vec::with_capacity(rows.len());
    for row in rows {
        let id = row.try_get("", "id")?;
        books.push(Storybook {
            id,
            workspace_id: row.try_get("", "workspace_id")?,
            title: row.try_get("", "title")?,
            storybook_type: parse_storybook_type(&row.try_get::<String>("", "storybook_type")?),
            status: parse_storybook_status(&row.try_get::<String>("", "status")?),
            visibility: parse_visibility(&row.try_get::<String>("", "visibility")?),
            source: row.try_get("", "source")?,
            source_title: row.try_get("", "source_title")?,
            target_child_id: row.try_get("", "target_child_id")?,
            creator_name: row.try_get("", "creator_name")?,
            updated_at: row
                .try_get::<DateTime<Utc>>("", "updated_at")?
                .format("%Y-%m-%d %H:%M")
                .to_string(),
            age_group: row.try_get("", "age_group")?,
            use_scene: row.try_get("", "use_scene")?,
            teaching_goal: row.try_get("", "teaching_goal")?,
            cover_tone: row.try_get("", "cover_tone")?,
            pages: pages_for(db, id).await?,
            roles: roles_for(db, id).await?,
        });
    }
    Ok(books)
}

async fn pages_for(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<Vec<StorybookPage>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, page_number, title, body, illustration_prompt, status
            from storybook_pages
            where storybook_id = $1
            order by page_number
            "#,
            [storybook_id.into()],
        ))
        .await?;
    rows.into_iter()
        .map(|row| {
            let page_number: i32 = row.try_get("", "page_number")?;
            Ok(StorybookPage {
                id: row.try_get("", "id")?,
                page_number: page_number.max(0) as u32,
                title: row.try_get("", "title")?,
                body: row.try_get("", "body")?,
                illustration_prompt: row.try_get("", "illustration_prompt")?,
                status: row.try_get("", "status")?,
            })
        })
        .collect()
}

async fn roles_for(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<Vec<StorybookRole>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, name, role_type, appearance, coalesce(story_function, '') as story_function, needs_consistency
            from storybook_roles
            where storybook_id = $1
            order by role_type, name
            "#,
            [storybook_id.into()],
        ))
        .await?;
    rows.into_iter()
        .map(|row| {
            Ok(StorybookRole {
                id: row.try_get("", "id")?,
                name: row.try_get("", "name")?,
                role_type: row.try_get("", "role_type")?,
                appearance: row.try_get("", "appearance")?,
                story_function: row.try_get("", "story_function")?,
                needs_consistency: row.try_get("", "needs_consistency")?,
            })
        })
        .collect()
}

async fn seed_default_pages_and_roles(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<(), DbErr> {
    let page_count: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "select count(*) as count from storybook_pages where storybook_id = $1",
            [storybook_id.into()],
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);
    if page_count == 0 {
        let page_id = Uuid::new_v4();
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into storybook_pages (id, storybook_id, page_number, title, body, illustration_prompt, status)
            values ($1, $2, 1, '第一页', '老师确认故事方案后，孩子们一起进入故事。', '温暖教室，老师和孩子围坐阅读。', 'ready')
            "#,
            [page_id.into(), storybook_id.into()],
        ))
        .await?;
    }

    let role_count: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "select count(*) as count from storybook_roles where storybook_id = $1",
            [storybook_id.into()],
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);
    if role_count == 0 {
        let role_id = Uuid::new_v4();
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into storybook_roles (id, storybook_id, name, role_type, appearance, story_function, needs_consistency)
            values ($1, $2, '老师形象', 'teacher', '温柔、清楚、适合幼儿园场景', '引导故事推进', true)
            "#,
            [role_id.into(), storybook_id.into()],
        ))
        .await?;
    }

    Ok(())
}

async fn clone_pages_and_roles(
    db: &DatabaseConnection,
    source_storybook_id: Uuid,
    target_storybook_id: Uuid,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybook_pages (id, storybook_id, page_number, title, body, illustration_prompt, status)
        select gen_random_uuid(), $2, page_number, title, body, illustration_prompt, status
        from storybook_pages
        where storybook_id = $1
        "#,
        [source_storybook_id.into(), target_storybook_id.into()],
    ))
    .await?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybook_roles (id, storybook_id, name, role_type, appearance, story_function, needs_consistency)
        select gen_random_uuid(), $2, name, role_type, appearance, story_function, needs_consistency
        from storybook_roles
        where storybook_id = $1
        "#,
        [source_storybook_id.into(), target_storybook_id.into()],
    ))
    .await?;
    Ok(())
}

struct CustomChildProfile {
    nickname: String,
    interests: Vec<String>,
    traits: Vec<String>,
    focus: String,
}

async fn child_profile_for_custom(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    child_id: Uuid,
) -> Result<CustomChildProfile, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
        select nickname, interests, traits, coalesce(focus, '') as focus
        from children
        where workspace_id = $1 and id = $2 and status = 'active'
        "#,
            [workspace_id.into(), child_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("child".to_string()))?;
    Ok(CustomChildProfile {
        nickname: row.try_get("", "nickname")?,
        interests: json_string_list(row.try_get("", "interests")?),
        traits: json_string_list(row.try_get("", "traits")?),
        focus: row.try_get("", "focus")?,
    })
}

async fn apply_child_customization(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    child: &CustomChildProfile,
    intensity: &str,
) -> Result<(), DbErr> {
    let interest_text = child.interests.join("、");
    let trait_text = child.traits.join("、");
    let first_interest = child
        .interests
        .first()
        .cloned()
        .unwrap_or_else(|| "喜欢的活动".to_string());
    let focus = if child.focus.trim().is_empty() {
        "当前教学目标"
    } else {
        child.focus.as_str()
    };

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_roles
        set name = $2,
            role_type = 'protagonist',
            appearance = $3,
            story_function = $4,
            needs_consistency = true
        where id = (
            select id
            from storybook_roles
            where storybook_id = $1 and role_type in ('protagonist', 'peer', 'supporting')
            order by case role_type when 'protagonist' then 0 when 'peer' then 1 else 2 end, name
            limit 1
        )
        "#,
        [
            storybook_id.into(),
            child.nickname.clone().into(),
            format!(
                "{}，带有孩子熟悉的兴趣元素：{}",
                if trait_text.is_empty() {
                    "幼儿园孩子"
                } else {
                    &trait_text
                },
                if interest_text.is_empty() {
                    "日常游戏"
                } else {
                    &interest_text
                }
            )
            .into(),
            format!("以{}的视角练习{}", child.nickname, focus).into(),
        ],
    ))
    .await?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_pages
        set title = case when page_number = 1 then $2 else title end,
            body = body || $3,
            illustration_prompt = illustration_prompt || $4,
            status = 'needs_regeneration'
        where storybook_id = $1
        "#,
        [
            storybook_id.into(),
            format!("{}来到故事里", child.nickname).into(),
            format!(
                "\n\n定制改写：这一版会称呼{}，结合{}，重点练习{}。",
                child.nickname,
                if interest_text.is_empty() {
                    "孩子熟悉的生活经验"
                } else {
                    &interest_text
                },
                focus
            )
            .into(),
            format!(
                "；定制版加入{}熟悉的{}元素，保持角色跨页一致",
                child.nickname, first_interest
            )
            .into(),
        ],
    ))
    .await?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybooks
        set teaching_goal = teaching_goal || $2,
            cover_tone = $3,
            updated_at = now()
        where id = $1
        "#,
        [
            storybook_id.into(),
            format!("；定制关注：{}（{}）", focus, intensity).into(),
            format!(
                "定制给{}，融合{}",
                child.nickname,
                if interest_text.is_empty() {
                    "孩子日常经验"
                } else {
                    &interest_text
                }
            )
            .into(),
        ],
    ))
    .await?;

    Ok(())
}

fn json_string_list(value: JsonValue) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .filter(|item| !item.trim().is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_storybook_type(value: &str) -> StorybookType {
    match value {
        "custom" => StorybookType::Custom,
        _ => StorybookType::Plain,
    }
}

fn parse_storybook_status(value: &str) -> StorybookStatus {
    match value {
        "plan_pending" => StorybookStatus::PlanPending,
        "roles_pending" => StorybookStatus::RolesPending,
        "editing" => StorybookStatus::Editing,
        "image_pending" => StorybookStatus::ImagePending,
        "exportable" => StorybookStatus::Exportable,
        "submitted" => StorybookStatus::Submitted,
        "listed" => StorybookStatus::Listed,
        _ => StorybookStatus::Draft,
    }
}

fn ensure_status_transition(from: &StorybookStatus, to: &StorybookStatus) -> Result<(), DbErr> {
    if is_allowed_status_transition(from, to) {
        Ok(())
    } else {
        Err(DbErr::Custom(format!(
            "非法绘本状态流转：{} -> {}",
            storybook_status_name(from),
            storybook_status_name(to)
        )))
    }
}

fn ensure_deliverable_ready(book: &Storybook) -> Result<(), DbErr> {
    if book.pages.is_empty() {
        return Err(DbErr::Custom(
            "绘本至少需要一个分页才能标记可交付".to_string(),
        ));
    }
    if book.roles.is_empty() {
        return Err(DbErr::Custom(
            "绘本至少需要一个角色或道具设定才能标记可交付".to_string(),
        ));
    }
    if book.pages.iter().any(|page| page.status == "generating") {
        return Err(DbErr::Custom(
            "仍有插图正在生成，完成后才能标记可交付".to_string(),
        ));
    }
    Ok(())
}

fn is_allowed_status_transition(from: &StorybookStatus, to: &StorybookStatus) -> bool {
    use StorybookStatus::{
        Draft, Editing, Exportable, ImagePending, Listed, PlanPending, RolesPending, Submitted,
    };

    if from == to {
        return true;
    }

    matches!(
        (from, to),
        (Draft, PlanPending)
            | (PlanPending, RolesPending)
            | (RolesPending, Editing)
            | (Editing, ImagePending)
            | (Editing, Exportable)
            | (ImagePending, Exportable)
            | (ImagePending, Editing)
            | (Exportable, Editing)
            | (Exportable, Submitted)
            | (Submitted, Listed)
    )
}

fn parse_visibility(value: &str) -> Visibility {
    match value {
        "workspace" => Visibility::Workspace,
        "market_submission" => Visibility::MarketSubmission,
        "market_listed" => Visibility::MarketListed,
        _ => Visibility::Private,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{StorybookPage, StorybookRole};

    #[test]
    fn storybook_status_transition_allows_expected_path() {
        assert!(is_allowed_status_transition(
            &StorybookStatus::Draft,
            &StorybookStatus::PlanPending
        ));
        assert!(is_allowed_status_transition(
            &StorybookStatus::PlanPending,
            &StorybookStatus::RolesPending
        ));
        assert!(is_allowed_status_transition(
            &StorybookStatus::RolesPending,
            &StorybookStatus::Editing
        ));
        assert!(is_allowed_status_transition(
            &StorybookStatus::Editing,
            &StorybookStatus::ImagePending
        ));
        assert!(is_allowed_status_transition(
            &StorybookStatus::ImagePending,
            &StorybookStatus::Exportable
        ));
        assert!(is_allowed_status_transition(
            &StorybookStatus::Exportable,
            &StorybookStatus::Submitted
        ));
        assert!(is_allowed_status_transition(
            &StorybookStatus::Submitted,
            &StorybookStatus::Listed
        ));
    }

    #[test]
    fn storybook_status_transition_allows_editing_recovery() {
        assert!(is_allowed_status_transition(
            &StorybookStatus::Exportable,
            &StorybookStatus::Editing
        ));
        assert!(is_allowed_status_transition(
            &StorybookStatus::ImagePending,
            &StorybookStatus::Editing
        ));
        assert!(is_allowed_status_transition(
            &StorybookStatus::Editing,
            &StorybookStatus::Exportable
        ));
    }

    #[test]
    fn storybook_status_transition_rejects_skips_and_backwards_jumps() {
        assert!(!is_allowed_status_transition(
            &StorybookStatus::Draft,
            &StorybookStatus::Listed
        ));
        assert!(!is_allowed_status_transition(
            &StorybookStatus::PlanPending,
            &StorybookStatus::Exportable
        ));
        assert!(!is_allowed_status_transition(
            &StorybookStatus::Submitted,
            &StorybookStatus::Editing
        ));
    }

    #[test]
    fn deliverable_check_requires_content_and_no_running_pages() {
        let mut book = test_storybook();
        assert!(ensure_deliverable_ready(&book).is_ok());

        book.pages[0].status = "generating".to_string();
        assert!(ensure_deliverable_ready(&book).is_err());

        book.pages.clear();
        assert!(ensure_deliverable_ready(&book).is_err());

        book = test_storybook();
        book.roles.clear();
        assert!(ensure_deliverable_ready(&book).is_err());
    }

    #[test]
    fn build_custom_storybook_customization_keeps_source_story_context() {
        let source = test_storybook();
        let child = CustomChildProfile {
            nickname: "乐乐".to_string(),
            interests: vec!["积木车".to_string(), "唱歌".to_string()],
            traits: vec!["热情".to_string()],
            focus: "轮流和表达需求".to_string(),
        };

        let customization = build_custom_storybook_customization(&source, &child, "balanced", None);

        assert_eq!(customization.source, "derived:balanced");
        assert_eq!(customization.title, "乐乐的定制故事");
        assert_eq!(customization.age_group, source.age_group);
        assert_eq!(customization.use_scene, source.use_scene);
        assert_eq!(customization.teaching_goal, source.teaching_goal);
        assert_eq!(customization.cover_tone, source.cover_tone);
    }

    #[test]
    fn customization_strategy_reads_confirmed_generation_plan() {
        let plan = serde_json::json!({
            "customization": {
                "strategy": "保留母本主线，替换孩子称呼和兴趣道具。"
            }
        });

        assert_eq!(
            customization_strategy(Some(&plan)).as_deref(),
            Some("保留母本主线，替换孩子称呼和兴趣道具。")
        );
    }

    fn test_storybook() -> Storybook {
        Storybook {
            id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            title: "一起玩小汽车".to_string(),
            storybook_type: StorybookType::Plain,
            status: StorybookStatus::Editing,
            visibility: Visibility::Private,
            source: "blank".to_string(),
            source_title: None,
            target_child_id: None,
            creator_name: "林老师".to_string(),
            updated_at: "今天 09:00".to_string(),
            age_group: "4-5 岁".to_string(),
            use_scene: "规则引导".to_string(),
            teaching_goal: "学习轮流与分享".to_string(),
            cover_tone: "温暖、清楚".to_string(),
            pages: vec![StorybookPage {
                id: Uuid::new_v4(),
                page_number: 1,
                title: "第一页".to_string(),
                body: "内容".to_string(),
                illustration_prompt: "提示".to_string(),
                status: "ready".to_string(),
            }],
            roles: vec![StorybookRole {
                id: Uuid::new_v4(),
                name: "林老师".to_string(),
                role_type: "teacher".to_string(),
                appearance: "温和、稳定".to_string(),
                story_function: "引导孩子轮流等待".to_string(),
                needs_consistency: true,
            }],
        }
    }
}

fn storybook_status_name(value: &StorybookStatus) -> &'static str {
    match value {
        StorybookStatus::Draft => "draft",
        StorybookStatus::PlanPending => "plan_pending",
        StorybookStatus::RolesPending => "roles_pending",
        StorybookStatus::Editing => "editing",
        StorybookStatus::ImagePending => "image_pending",
        StorybookStatus::Exportable => "exportable",
        StorybookStatus::Submitted => "submitted",
        StorybookStatus::Listed => "listed",
    }
}

fn storybook_type_name(value: &StorybookType) -> &'static str {
    match value {
        StorybookType::Plain => "plain",
        StorybookType::Custom => "custom",
    }
}

fn visibility_name(value: &Visibility) -> &'static str {
    match value {
        Visibility::Private => "private",
        Visibility::Workspace => "workspace",
        Visibility::MarketSubmission => "market_submission",
        Visibility::MarketListed => "market_listed",
    }
}

fn uuid(value: &str) -> Result<Uuid, DbErr> {
    Uuid::parse_str(value).map_err(|err| DbErr::Custom(err.to_string()))
}

async fn execute(db: &DatabaseConnection, sql: &str) -> Result<(), DbErr> {
    db.execute(Statement::from_string(DbBackend::Postgres, sql.to_string()))
        .await?;
    Ok(())
}
