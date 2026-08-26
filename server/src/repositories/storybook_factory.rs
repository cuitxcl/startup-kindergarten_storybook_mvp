use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use uuid::Uuid;

use crate::models::{CreateStorybookRequest, MarketplaceTemplate, Storybook};
use crate::page_aspect::normalize_page_aspect_ratio;
use crate::repositories::storybook_rules::storybook_type_name;

fn normal_storybook_style_settings(
    story_style_id: Option<&str>,
    visual_style_id: Option<&str>,
    visual_style_version: Option<i32>,
) -> (String, String, i32, String) {
    let story_style_id = story_style_id
        .filter(|id| crate::creative_presets::story_style(id).is_some())
        .unwrap_or(crate::creative_presets::DEFAULT_STORY_STYLE_ID)
        .to_string();
    let requested_version =
        visual_style_version.unwrap_or(crate::creative_presets::DEFAULT_VISUAL_STYLE_VERSION);
    let visual_style_id = visual_style_id
        .filter(|id| crate::creative_presets::visual_prompt(id, requested_version).is_some())
        .unwrap_or(crate::creative_presets::DEFAULT_VISUAL_STYLE_ID)
        .to_string();
    let visual_style_version =
        crate::creative_presets::visual_prompt(&visual_style_id, requested_version)
            .map(|_| requested_version)
            .unwrap_or(crate::creative_presets::DEFAULT_VISUAL_STYLE_VERSION);
    let cover_tone = crate::creative_presets::visual_prompt(&visual_style_id, visual_style_version)
        .expect("default visual style preset must exist")
        .to_string();

    (
        story_style_id,
        visual_style_id,
        visual_style_version,
        cover_tone,
    )
}

pub async fn create_plain(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    creator_id: Uuid,
    payload: CreateStorybookRequest,
) -> Result<Storybook, DbErr> {
    let storybook_id = Uuid::new_v4();
    let page_aspect_ratio = normalize_page_aspect_ratio(payload.page_aspect_ratio.as_deref());
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybooks
          (id, workspace_id, storybook_type, status, visibility, source, title, age_group, use_scene, teaching_goal, cover_tone, story_style_id, visual_style_id, visual_style_version, page_aspect_ratio, creator_id, created_at, updated_at)
        values ($1, $2, 'plain', 'plan_pending', 'private', 'blank', $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, now(), now())
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
            page_aspect_ratio.into(),
            creator_id.into(),
        ],
    ))
    .await?;
    crate::repositories::storybook_queries::find(db, workspace_id, storybook_id).await
}

pub async fn create_from_marketplace_template(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    creator_id: Uuid,
    template: MarketplaceTemplate,
) -> Result<Storybook, DbErr> {
    let storybook_id = Uuid::new_v4();
    let (story_style_id, visual_style_id, visual_style_version, cover_tone) =
        normal_storybook_style_settings(None, None, None);
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybooks
          (id, workspace_id, storybook_type, status, visibility, source, title, age_group, use_scene, teaching_goal, cover_tone, story_style_id, visual_style_id, visual_style_version, page_aspect_ratio, source_storybook_id, creator_id, created_at, updated_at)
        values ($1, $2, 'plain', 'draft', 'private', 'marketplace', $3, $4, $5, $6, $7, $8, $9, $10, 'portrait_4_5', $11, $12, now(), now())
        "#,
        [
            storybook_id.into(),
            workspace_id.into(),
            template.title.clone().into(),
            template.age_group.clone().into(),
            template.use_scene.clone().into(),
            template.summary.clone().into(),
            cover_tone.into(),
            story_style_id.into(),
            visual_style_id.into(),
            visual_style_version.into(),
            template.source_storybook_id.into(),
            creator_id.into(),
        ],
    ))
    .await?;
    if let Some(source_storybook_id) = template.source_storybook_id {
        clone_pages_and_roles(db, source_storybook_id, storybook_id).await?;
    } else {
        seed_default_pages_and_roles(db, storybook_id).await?;
    }
    crate::repositories::storybook_queries::find(db, workspace_id, storybook_id).await
}

pub async fn duplicate(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    creator_id: Uuid,
    requested_title: Option<String>,
) -> Result<Storybook, DbErr> {
    let source =
        crate::repositories::storybook_queries::find(db, workspace_id, storybook_id).await?;
    let new_id = Uuid::new_v4();
    let title = requested_title.unwrap_or_else(|| format!("{} 副本", source.title));
    let (story_style_id, visual_style_id, visual_style_version, cover_tone) =
        normal_storybook_style_settings(
            source.story_style_id.as_deref(),
            source.visual_style_id.as_deref(),
            source.visual_style_version,
        );
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybooks
          (id, workspace_id, storybook_type, status, visibility, source, source_storybook_id, target_child_id, title, age_group, use_scene, teaching_goal, cover_tone, story_style_id, visual_style_id, visual_style_version, page_aspect_ratio, creator_id, created_at, updated_at)
        values ($1, $2, $3, 'draft', 'private', 'duplicate', $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, now(), now())
        "#,
        [
            new_id.into(),
            workspace_id.into(),
            storybook_type_name(&source.storybook_type).into(),
            storybook_id.into(),
            source.target_child_id.into(),
            title.into(),
            source.age_group.into(),
            source.use_scene.into(),
            source.teaching_goal.into(),
            cover_tone.into(),
            story_style_id.into(),
            visual_style_id.into(),
            visual_style_version.into(),
            source.page_aspect_ratio.into(),
            creator_id.into(),
        ],
    ))
    .await?;
    clone_pages_and_roles(db, storybook_id, new_id).await?;
    crate::repositories::storybook_queries::find(db, workspace_id, new_id).await
}

pub(crate) async fn seed_default_pages_and_roles(
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
            values ($1, $2, 1, '故事从这里开始', '林老师带着孩子们翻开绘本，先一起确认今天要练习的小约定。', '温暖幼儿园教室，林老师温柔清楚，穿浅色围裙，和孩子们围坐阅读绘本。', 'draft')
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
            insert into storybook_roles
              (id, storybook_id, name, role_type, appearance, story_function, needs_consistency, reference_status)
            values ($1, $2, '林老师', 'teacher', '温柔清楚，穿浅色围裙，常和孩子平视交流，适合幼儿园共读场景', '在故事中引导孩子理解规则、情绪或生活习惯', true, 'not_started')
            "#,
            [role_id.into(), storybook_id.into()],
        ))
        .await?;
    }

    normalize_placeholder_page_and_role(db, storybook_id).await?;

    Ok(())
}

async fn normalize_placeholder_page_and_role(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_roles
        set name = '林老师',
            appearance = '温柔清楚，穿浅色围裙，常和孩子平视交流，适合幼儿园共读场景',
            story_function = '在故事中引导孩子理解规则、情绪或生活习惯',
            role_type = 'teacher'
        where storybook_id = $1 and name = '老师形象'
        "#,
        [storybook_id.into()],
    ))
    .await?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_pages
        set title = '故事从这里开始',
            body = '林老师带着孩子们翻开绘本，先一起确认今天要练习的小约定。',
            illustration_prompt = '温暖幼儿园教室，林老师温柔清楚，穿浅色围裙，和孩子们围坐阅读绘本。',
            status = case when status = 'ready' then 'draft' else status end
        where storybook_id = $1
          and page_number = 1
          and title = '第一页'
          and body = '老师确认故事方案后，孩子们一起进入故事。'
        "#,
        [storybook_id.into()],
    ))
    .await?;
    Ok(())
}

pub(crate) async fn clone_pages_and_roles(
    db: &DatabaseConnection,
    source_storybook_id: Uuid,
    target_storybook_id: Uuid,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybook_pages (id, storybook_id, page_number, title, body, illustration_prompt, status)
        select gen_random_uuid(), $2, page_number, title, body, illustration_prompt,
               case when status in ('generating', 'ready') then 'needs_regeneration' else status end
        from storybook_pages
        where storybook_id = $1
        "#,
        [source_storybook_id.into(), target_storybook_id.into()],
    ))
    .await?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybook_roles
          (id, storybook_id, name, role_type, appearance, story_function, needs_consistency,
           reference_image_url, reference_image_prompt, reference_status)
        select gen_random_uuid(), $2, name, role_type, appearance, story_function, needs_consistency,
               null, null,
               case when reference_status in ('generating', 'ready') then 'not_started' else reference_status end
        from storybook_roles
        where storybook_id = $1
        "#,
        [source_storybook_id.into(), target_storybook_id.into()],
    ))
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::normal_storybook_style_settings;

    #[test]
    fn duplicate_style_settings_keep_a_valid_source_preset() {
        let settings =
            normal_storybook_style_settings(Some("growth_guidance"), Some("pixar_3d"), Some(1));

        assert_eq!(settings.0, "growth_guidance");
        assert_eq!(settings.1, "pixar_3d");
        assert_eq!(settings.2, 1);
        assert!(settings.3.contains("皮克斯3D"));
    }

    #[test]
    fn template_style_settings_fall_back_to_one_consistent_default() {
        let settings = normal_storybook_style_settings(
            Some("not-a-story-style"),
            Some("not-a-visual-style"),
            Some(99),
        );

        assert_eq!(settings.0, "daily_warmth");
        assert_eq!(settings.1, "watercolor_book");
        assert_eq!(settings.2, 1);
        assert!(settings.3.contains("水彩绘本"));
    }
}
