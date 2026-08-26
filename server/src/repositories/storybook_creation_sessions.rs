use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

use crate::models::{
    CreateGenerationJobRequest, CreateStorybookRequest, CreationGenerationSummary,
    CreationMaterial, CreationOutline, CreationUnderstanding, GenerationJob, PaginationMeta,
    StoryDirection, StorybookAssetReference, StorybookCreationSession,
    StorybookCreationSessionListItem, StorybookCreationSessionListQuery, VisualPreferences,
};

pub async fn create(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    created_by: Uuid,
    entry_type: String,
    source_storybook_id: Option<Uuid>,
    quick_idea: String,
    use_scene: String,
    age_group: String,
    page_count: u32,
    understanding: CreationUnderstanding,
    materials: Vec<CreationMaterial>,
    visual_preferences: VisualPreferences,
    story_style_id: String,
    visual_style_id: String,
    visual_style_version: i32,
) -> Result<StorybookCreationSession, DbErr> {
    let id = Uuid::new_v4();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybook_creation_sessions
          (id, workspace_id, created_by, entry_type, source_storybook_id, status, quick_idea, use_scene, age_group, page_count,
           understanding_json, materials_json, directions_json, visual_preferences_json, story_style_id, visual_style_id, visual_style_version,
           created_at, updated_at)
        values ($1, $2, $3, $4, $5, 'understanding_ready', $6, $7, $8, $9, $10, $11, '[]'::jsonb, $12, $13, $14, $15, now(), now())
        "#,
        [
            id.into(),
            workspace_id.into(),
            created_by.into(),
            entry_type.into(),
            source_storybook_id.into(),
            quick_idea.into(),
            use_scene.into(),
            age_group.into(),
            (page_count as i32).into(),
            json!(understanding).into(),
            json!(materials).into(),
            json!(visual_preferences).into(),
            story_style_id.into(),
            visual_style_id.into(),
            visual_style_version.into(),
        ],
    ))
    .await?;
    find(db, workspace_id, id).await
}

pub async fn list(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    actor_id: Uuid,
    can_view_all: bool,
    query: StorybookCreationSessionListQuery,
) -> Result<(Vec<StorybookCreationSessionListItem>, PaginationMeta), DbErr> {
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let offset = query.offset.unwrap_or(0);
    let status = query.status.unwrap_or_else(|| "active".to_string());
    let created_by = if can_view_all {
        query.created_by
    } else {
        Some(actor_id)
    };
    let active_only = status == "active";
    let concrete_status = if active_only || status == "all" {
        None
    } else {
        Some(status.clone())
    };

    let total: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select count(*) as count
            from storybook_creation_sessions
            where workspace_id = $1
              and ($2::uuid is null or created_by = $2)
              and ($3::text is null or status = $3)
              and ($4::boolean = false or status not in ('storybook_ready', 'abandoned'))
              and entry_type = 'direct_create'
            "#,
            [
                workspace_id.into(),
                created_by.into(),
                concrete_status.clone().into(),
                active_only.into(),
            ],
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);

    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, status, quick_idea, understanding_json, directions_json, selected_direction_id,
                   storybook_id, updated_at
            from storybook_creation_sessions
            where workspace_id = $1
              and ($2::uuid is null or created_by = $2)
              and ($3::text is null or status = $3)
              and ($4::boolean = false or status not in ('storybook_ready', 'abandoned'))
              and entry_type = 'direct_create'
            order by updated_at desc
            limit $5 offset $6
            "#,
            [
                workspace_id.into(),
                created_by.into(),
                concrete_status.into(),
                active_only.into(),
                (limit as i64).into(),
                (offset as i64).into(),
            ],
        ))
        .await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let understanding: JsonValue = row.try_get("", "understanding_json")?;
        let directions: JsonValue = row.try_get("", "directions_json")?;
        let selected_direction_id: Option<String> = row.try_get("", "selected_direction_id")?;
        let selected_direction_title = selected_direction_id.as_ref().and_then(|selected| {
            directions.as_array().and_then(|items| {
                items
                    .iter()
                    .find(|item| item.get("id").and_then(JsonValue::as_str) == Some(selected))
                    .and_then(|item| item.get("title").and_then(JsonValue::as_str))
                    .map(str::to_string)
            })
        });
        items.push(StorybookCreationSessionListItem {
            id: row.try_get("", "id")?,
            status: row.try_get("", "status")?,
            quick_idea: row.try_get("", "quick_idea")?,
            understanding_summary: understanding
                .get("summary")
                .and_then(JsonValue::as_str)
                .unwrap_or_default()
                .to_string(),
            selected_direction_title,
            storybook_id: row.try_get("", "storybook_id")?,
            updated_at: row.try_get::<DateTime<Utc>>("", "updated_at")?,
        });
    }

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

pub async fn latest_active(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    created_by: Uuid,
) -> Result<Option<StorybookCreationSession>, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select *
            from storybook_creation_sessions
            where workspace_id = $1
              and created_by = $2
              and status <> 'abandoned'
              and entry_type = 'direct_create'
            order by updated_at desc
            limit 1
            "#,
            [workspace_id.into(), created_by.into()],
        ))
        .await?;
    match row.map(session_from_row).transpose()? {
        Some(session) => attach_asset_references(db, session).await.map(Some),
        None => Ok(None),
    }
}

pub async fn latest_source_asset_session(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    created_by: Uuid,
    source_storybook_id: Uuid,
) -> Result<Option<StorybookCreationSession>, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select *
            from storybook_creation_sessions
            where workspace_id = $1
              and created_by = $2
              and source_storybook_id = $3
              and entry_type = 'from_storybook_assets'
              and status not in ('storybook_ready', 'abandoned')
            order by updated_at desc
            limit 1
            "#,
            [
                workspace_id.into(),
                created_by.into(),
                source_storybook_id.into(),
            ],
        ))
        .await?;
    match row.map(session_from_row).transpose()? {
        Some(session) => attach_asset_references(db, session).await.map(Some),
        None => Ok(None),
    }
}

pub async fn find(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    session_id: Uuid,
) -> Result<StorybookCreationSession, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "select * from storybook_creation_sessions where workspace_id = $1 and id = $2",
            [workspace_id.into(), session_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("storybook_creation_session".to_string()))?;
    attach_asset_references(db, session_from_row(row)?).await
}

pub async fn find_for_update(
    db: &impl ConnectionTrait,
    workspace_id: Uuid,
    session_id: Uuid,
) -> Result<StorybookCreationSession, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select *
            from storybook_creation_sessions
            where workspace_id = $1 and id = $2
            for update
            "#,
            [workspace_id.into(), session_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("storybook_creation_session".to_string()))?;
    session_from_row(row)
}

pub async fn save(
    db: &DatabaseConnection,
    session: &StorybookCreationSession,
) -> Result<StorybookCreationSession, DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_creation_sessions
        set status = $3,
            quick_idea = $4,
            use_scene = $5,
            age_group = $6,
            page_count = $7,
            understanding_json = $8,
            materials_json = $9,
            directions_json = $10,
            selected_direction_id = $11,
            outline_json = $12,
            visual_preferences_json = $13,
            story_style_id = $14,
            visual_style_id = $15,
            visual_style_version = $16,
            storybook_id = $17,
            last_job_id = $18,
            idempotency_key = $19,
            generation_summary_json = $20,
            requires_understanding_refresh = $21,
            requires_direction_refresh = $22,
            requires_outline_refresh = $23,
            updated_at = now()
        where workspace_id = $1 and id = $2
        "#,
        [
            session.workspace_id.into(),
            session.id.into(),
            session.status.clone().into(),
            session.quick_idea.clone().into(),
            session.use_scene.clone().into(),
            session.age_group.clone().into(),
            (session.page_count as i32).into(),
            json!(session.understanding).into(),
            json!(session.materials).into(),
            json!(session.directions).into(),
            session.selected_direction_id.clone().into(),
            session.outline.as_ref().map(|value| json!(value)).into(),
            json!(session.visual_preferences).into(),
            session.story_style_id.clone().into(),
            session.visual_style_id.clone().into(),
            session.visual_style_version.into(),
            session.storybook_id.into(),
            session.last_job_id.into(),
            session.idempotency_key.clone().into(),
            json!(session.generation_summary).into(),
            session.requires_understanding_refresh.into(),
            session.requires_direction_refresh.into(),
            session.requires_outline_refresh.into(),
        ],
    ))
    .await?;
    find(db, session.workspace_id, session.id).await
}

pub async fn save_in_tx(
    db: &impl ConnectionTrait,
    session: &StorybookCreationSession,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_creation_sessions
        set status = $3,
            quick_idea = $4,
            use_scene = $5,
            age_group = $6,
            page_count = $7,
            understanding_json = $8,
            materials_json = $9,
            directions_json = $10,
            selected_direction_id = $11,
            outline_json = $12,
            visual_preferences_json = $13,
            story_style_id = $14,
            visual_style_id = $15,
            visual_style_version = $16,
            storybook_id = $17,
            last_job_id = $18,
            idempotency_key = $19,
            generation_summary_json = $20,
            requires_understanding_refresh = $21,
            requires_direction_refresh = $22,
            requires_outline_refresh = $23,
            updated_at = now()
        where workspace_id = $1 and id = $2
        "#,
        [
            session.workspace_id.into(),
            session.id.into(),
            session.status.clone().into(),
            session.quick_idea.clone().into(),
            session.use_scene.clone().into(),
            session.age_group.clone().into(),
            (session.page_count as i32).into(),
            json!(session.understanding).into(),
            json!(session.materials).into(),
            json!(session.directions).into(),
            session.selected_direction_id.clone().into(),
            session.outline.as_ref().map(|value| json!(value)).into(),
            json!(session.visual_preferences).into(),
            session.story_style_id.clone().into(),
            session.visual_style_id.clone().into(),
            session.visual_style_version.into(),
            session.storybook_id.into(),
            session.last_job_id.into(),
            session.idempotency_key.clone().into(),
            json!(session.generation_summary).into(),
            session.requires_understanding_refresh.into(),
            session.requires_direction_refresh.into(),
            session.requires_outline_refresh.into(),
        ],
    ))
    .await?;
    Ok(())
}

pub async fn replace_storybook_content(
    db: &impl ConnectionTrait,
    storybook_id: Uuid,
    outline: &CreationOutline,
    materials: &[CreationMaterial],
    direction: &StoryDirection,
    visual_preferences: &VisualPreferences,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "delete from storybook_pages where storybook_id = $1",
        [storybook_id.into()],
    ))
    .await?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "delete from storybook_roles where storybook_id = $1",
        [storybook_id.into()],
    ))
    .await?;

    for material in materials
        .iter()
        .filter(|item| item.material_type == "character")
    {
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into storybook_roles
              (id, storybook_id, name, role_type, appearance, story_function, needs_consistency, reference_status)
            values ($1, $2, $3, $4, $5, $6, true, 'not_started')
            "#,
            [
                Uuid::new_v4().into(),
                storybook_id.into(),
                material.label.clone().into(),
                "protagonist".into(),
                format!(
                    "{}，儿童绘本主角，画风 {}，保持跨页一致",
                    material.label, visual_preferences.style
                )
                .into(),
                direction.personal_hook.clone().into(),
            ],
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
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into storybook_roles
              (id, storybook_id, name, role_type, appearance, story_function, needs_consistency, reference_status)
            values ($1, $2, '主角', 'protagonist', $3, $4, true, 'not_started')
            "#,
            [
                Uuid::new_v4().into(),
                storybook_id.into(),
                format!("适合 {} 的儿童绘本主角，画风 {}", visual_preferences.page_aspect_ratio, visual_preferences.style).into(),
                direction.personal_hook.clone().into(),
            ],
        ))
        .await?;
    }

    for page in &outline.pages {
        let labels = material_labels(materials, &page.material_ids).join("、");
        let title = page_title(&page.summary, page.page_number);
        let body = format!("{} {}", page.summary, age_body_tail(page.page_number));
        let prompt = format!(
            "{}。必须出现或呼应：{}。故事私人钩子：{}。画面偏好：style={}，aspect_ratio={}，complexity={}，character_consistency={}。儿童绘本插图，不出现文字。",
            page.summary,
            if labels.is_empty() {
                "已选故事素材"
            } else {
                &labels
            },
            direction.personal_hook,
            visual_preferences.style,
            visual_preferences.page_aspect_ratio,
            visual_preferences.visual_complexity,
            visual_preferences.character_consistency
        );
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into storybook_pages (id, storybook_id, page_number, title, body, illustration_prompt, status)
            values ($1, $2, $3, $4, $5, $6, 'needs_regeneration')
            "#,
            [
                Uuid::new_v4().into(),
                storybook_id.into(),
                (page.page_number as i32).into(),
                title.into(),
                body.into(),
                prompt.into(),
            ],
        ))
        .await?;
    }

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "update storybooks set status = 'editing', updated_at = now() where id = $1",
        [storybook_id.into()],
    ))
    .await?;
    Ok(())
}

pub async fn bind_confirmed_person_reference_images(
    db: &impl ConnectionTrait,
    storybook_id: Uuid,
    workspace_id: Uuid,
    asset_references: &[StorybookAssetReference],
) -> Result<(), DbErr> {
    let mut updated = false;
    for reference in asset_references {
        let Some(visual_reference) = reference.visual_reference.as_ref() else {
            continue;
        };
        let Some(generation_job_id) = visual_reference.generation_job_id else {
            continue;
        };
        if reference.kind != "person"
            || reference.status != "ready"
            || visual_reference.status != "confirmed"
            || reference.display_name.trim().is_empty()
        {
            continue;
        }
        let result = db
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                update storybook_roles
                set reference_image_url = $3,
                    reference_image_prompt = $4,
                    reference_status = 'ready'
                where storybook_id = $1
                  and btrim(name) = btrim($2)
                  and reference_image_url is null
                "#,
                [
                    storybook_id.into(),
                    reference.display_name.clone().into(),
                    format!(
                        "/api/workspaces/{workspace_id}/generation-jobs/{generation_job_id}/image"
                    )
                    .into(),
                    format!("已确认的人物同画风参考：{}", reference.display_name.trim()).into(),
                ],
            ))
            .await?;
        updated |= result.rows_affected() > 0;
    }
    if updated {
        crate::repositories::storybook_lifecycle::mark_storybook_content_changed(db, storybook_id)
            .await?;
    }
    Ok(())
}

pub async fn create_storybook_shell_in_tx(
    db: &impl ConnectionTrait,
    workspace_id: Uuid,
    creator_id: Uuid,
    payload: CreateStorybookRequest,
) -> Result<Uuid, DbErr> {
    let storybook_id = Uuid::new_v4();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybooks
          (id, workspace_id, storybook_type, status, visibility, source, title, age_group, use_scene,
           teaching_goal, cover_tone, story_style_id, visual_style_id, visual_style_version, page_aspect_ratio, creator_id, created_at, updated_at)
        values ($1, $2, 'plain', 'plan_pending', 'private', 'creation_session', $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, now(), now())
        "#,
        [
            storybook_id.into(),
            workspace_id.into(),
            payload.title.into(),
            payload.age_group.into(),
            payload.use_scene.into(),
            payload.teaching_goal.into(),
            payload
                .cover_tone
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "温暖、清楚".to_string())
                .into(),
            payload.story_style_id.into(),
            payload.visual_style_id.into(),
            payload.visual_style_version.into(),
            crate::page_aspect::normalize_page_aspect_ratio(payload.page_aspect_ratio.as_deref())
                .into(),
            creator_id.into(),
        ],
    ))
    .await?;
    Ok(storybook_id)
}

pub async fn enqueue_creation_job_in_tx(
    db: &impl ConnectionTrait,
    workspace_id: Uuid,
    created_by: Uuid,
    payload: CreateGenerationJobRequest,
) -> Result<GenerationJob, DbErr> {
    let id = Uuid::new_v4();
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into generation_jobs
              (id, workspace_id, storybook_id, created_by, job_type, status, input_json, created_at)
            values ($1, $2, $3, $4, 'creation_storybook_generate', 'queued', $5, now())
            returning
              id, workspace_id, storybook_id, created_by, job_type, status, input_json, output_json,
              attempt_count, last_error, next_run_at, locked_by, locked_at, created_at, finished_at
            "#,
            [
                id.into(),
                workspace_id.into(),
                payload.storybook_id.into(),
                created_by.into(),
                payload.input_json.into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("generation_job".to_string()))?;
    job_from_row(row)
}

pub async fn mark_storybook_job_succeeded(
    db: &impl ConnectionTrait,
    workspace_id: Uuid,
    session_id: Uuid,
    storybook_id: Uuid,
    job_id: Uuid,
    summary: &CreationGenerationSummary,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_creation_sessions
        set status = 'storybook_ready',
            storybook_id = $3,
            last_job_id = $4,
            generation_summary_json = $5,
            updated_at = now()
        where workspace_id = $1 and id = $2 and status in ('generating', 'failed')
        "#,
        [
            workspace_id.into(),
            session_id.into(),
            storybook_id.into(),
            job_id.into(),
            json!(summary).into(),
        ],
    ))
    .await?;
    Ok(())
}

pub async fn mark_storybook_job_failed(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    session_id: Uuid,
    job_id: Uuid,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_creation_sessions
        set status = 'failed',
            last_job_id = $3,
            generation_summary_json = $4,
            updated_at = now()
        where workspace_id = $1 and id = $2 and status = 'generating'
        "#,
        [
            workspace_id.into(),
            session_id.into(),
            job_id.into(),
            json!(failed_generation_summary()).into(),
        ],
    ))
    .await?;
    Ok(())
}

pub async fn update_generation_summary_for_job(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    job_id: Uuid,
    summary: &CreationGenerationSummary,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_creation_sessions
        set generation_summary_json = $3,
            updated_at = now()
        where workspace_id = $1 and last_job_id = $2
        "#,
        [workspace_id.into(), job_id.into(), json!(summary).into()],
    ))
    .await?;
    Ok(())
}

fn session_from_row(row: sea_orm::QueryResult) -> Result<StorybookCreationSession, DbErr> {
    let status: String = row.try_get("", "status")?;
    Ok(StorybookCreationSession {
        id: row.try_get("", "id")?,
        workspace_id: row.try_get("", "workspace_id")?,
        created_by: row.try_get("", "created_by")?,
        entry_type: row
            .try_get("", "entry_type")
            .unwrap_or_else(|_| "direct_create".to_string()),
        source_storybook_id: row.try_get("", "source_storybook_id").unwrap_or(None),
        status: status.clone(),
        quick_idea: row.try_get("", "quick_idea")?,
        use_scene: row.try_get("", "use_scene")?,
        age_group: row.try_get("", "age_group")?,
        page_count: row.try_get::<i32>("", "page_count")?.max(1) as u32,
        understanding: serde_json::from_value(row.try_get::<JsonValue>("", "understanding_json")?)
            .map_err(|err| DbErr::Custom(format!("understanding_json 格式错误：{err}")))?,
        materials: serde_json::from_value(row.try_get::<JsonValue>("", "materials_json")?)
            .map_err(|err| DbErr::Custom(format!("materials_json 格式错误：{err}")))?,
        asset_references: Vec::new(),
        directions: serde_json::from_value(row.try_get::<JsonValue>("", "directions_json")?)
            .map_err(|err| DbErr::Custom(format!("directions_json 格式错误：{err}")))?,
        selected_direction_id: row.try_get("", "selected_direction_id")?,
        outline: row
            .try_get::<Option<JsonValue>>("", "outline_json")?
            .map(serde_json::from_value)
            .transpose()
            .map_err(|err| DbErr::Custom(format!("outline_json 格式错误：{err}")))?,
        visual_preferences: serde_json::from_value(
            row.try_get::<JsonValue>("", "visual_preferences_json")?,
        )
        .map_err(|err| DbErr::Custom(format!("visual_preferences_json 格式错误：{err}")))?,
        story_style_id: row
            .try_get("", "story_style_id")
            .unwrap_or_else(|_| "daily_warmth".to_string()),
        visual_style_id: row
            .try_get("", "visual_style_id")
            .unwrap_or_else(|_| "watercolor_book".to_string()),
        visual_style_version: row.try_get("", "visual_style_version").unwrap_or(1),
        storybook_id: row.try_get("", "storybook_id")?,
        last_job_id: row.try_get("", "last_job_id")?,
        idempotency_key: row.try_get("", "idempotency_key")?,
        generation_summary: serde_json::from_value(
            row.try_get::<JsonValue>("", "generation_summary_json")
                .unwrap_or_else(|_| json!(default_generation_summary_for_status(&status))),
        )
        .unwrap_or_else(|_| default_generation_summary_for_status(&status)),
        requires_understanding_refresh: row.try_get("", "requires_understanding_refresh")?,
        requires_direction_refresh: row.try_get("", "requires_direction_refresh")?,
        requires_outline_refresh: row.try_get("", "requires_outline_refresh")?,
        next_action: next_action_for_status(&status),
        created_at: row.try_get::<DateTime<Utc>>("", "created_at")?,
        updated_at: row.try_get::<DateTime<Utc>>("", "updated_at")?,
    })
}

async fn attach_asset_references(
    db: &DatabaseConnection,
    mut session: StorybookCreationSession,
) -> Result<StorybookCreationSession, DbErr> {
    session.asset_references = crate::repositories::storybook_creation_assets::list_for_session(
        db,
        session.workspace_id,
        session.id,
    )
    .await
    .or_else(|err| match err {
        DbErr::Exec(sea_orm::RuntimeErr::SqlxError(error))
            if error.to_string().contains("storybook_asset_references") =>
        {
            Ok::<Vec<StorybookAssetReference>, DbErr>(Vec::new())
        }
        other => Err(other),
    })?;
    Ok(session)
}

fn default_generation_summary_for_status(status: &str) -> CreationGenerationSummary {
    match status {
        "generating" => CreationGenerationSummary {
            text_generation_status: "generating".to_string(),
            image_generation_status: "pending".to_string(),
            quality_notice: None,
            recoverable_actions: Vec::new(),
        },
        "storybook_ready" => CreationGenerationSummary {
            text_generation_status: "succeeded".to_string(),
            image_generation_status: "pending".to_string(),
            quality_notice: None,
            recoverable_actions: vec!["open_review_workspace".to_string()],
        },
        "failed" => failed_generation_summary(),
        _ => CreationGenerationSummary {
            text_generation_status: "not_started".to_string(),
            image_generation_status: "not_started".to_string(),
            quality_notice: None,
            recoverable_actions: Vec::new(),
        },
    }
}

fn failed_generation_summary() -> CreationGenerationSummary {
    CreationGenerationSummary {
        text_generation_status: "failed".to_string(),
        image_generation_status: "not_started".to_string(),
        quality_notice: Some("生成没有完成，你可以重试，或先回到前一步调整内容。".to_string()),
        recoverable_actions: vec!["retry_generation".to_string(), "edit_outline".to_string()],
    }
}

fn job_from_row(row: sea_orm::QueryResult) -> Result<GenerationJob, DbErr> {
    Ok(GenerationJob {
        id: row.try_get("", "id")?,
        workspace_id: row.try_get("", "workspace_id")?,
        storybook_id: row.try_get("", "storybook_id")?,
        created_by: row.try_get("", "created_by")?,
        job_type: row.try_get("", "job_type")?,
        status: row.try_get("", "status")?,
        input_json: row.try_get("", "input_json")?,
        output_json: row.try_get("", "output_json")?,
        attempt_count: row.try_get("", "attempt_count")?,
        last_error: row.try_get("", "last_error")?,
        next_run_at: row.try_get("", "next_run_at")?,
        locked_by: row.try_get("", "locked_by")?,
        locked_at: row.try_get("", "locked_at")?,
        created_at: row.try_get::<DateTime<Utc>>("", "created_at")?,
        finished_at: row.try_get("", "finished_at")?,
    })
}

fn next_action_for_status(status: &str) -> Option<String> {
    match status {
        "draft" => Some("refresh_understanding".to_string()),
        "understanding_ready" => Some("generate_directions".to_string()),
        "directions_ready" => Some("select_direction".to_string()),
        "direction_selected" => Some("generate_outline".to_string()),
        "outline_ready" => Some("confirm_outline".to_string()),
        "generating" => Some("poll_generation_job".to_string()),
        "storybook_ready" => Some("open_review_workspace".to_string()),
        "failed" => Some("retry_or_edit".to_string()),
        _ => None,
    }
}

fn material_labels(materials: &[CreationMaterial], ids: &[String]) -> Vec<String> {
    ids.iter()
        .filter_map(|id| {
            materials
                .iter()
                .find(|item| &item.id == id)
                .map(|item| item.label.clone())
        })
        .collect()
}

fn page_title(summary: &str, page_number: u32) -> String {
    let clean = summary.trim();
    let mut title: String = clean.chars().take(16).collect();
    if title.is_empty() {
        title = format!("第 {page_number} 页");
    }
    title
}

fn age_body_tail(page_number: u32) -> &'static str {
    match page_number {
        1 => "故事轻轻开始，孩子能马上看见熟悉的人和物。",
        2 | 3 => "这一页保留情绪变化，让大人可以停下来和孩子聊一聊。",
        4 | 5 => "角色开始尝试一个小行动，变化要具体、温柔。",
        _ => "结尾给孩子一个可模仿的小办法。",
    }
}
