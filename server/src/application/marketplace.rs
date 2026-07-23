use axum::http::HeaderMap;
use loco_rs::app::AppContext;
use uuid::Uuid;

#[cfg(feature = "db")]
use serde_json::json;

use crate::{
    domains::common,
    error::ApiError,
    models::{MarketplaceQuery, MarketplaceTemplate, Storybook},
};

#[cfg(not(feature = "db"))]
use crate::models::{StorybookPage, StorybookRole, StorybookStatus, StorybookType, Visibility};

pub async fn list_templates(
    ctx: &AppContext,
    headers: &HeaderMap,
    query: MarketplaceQuery,
) -> Result<(Vec<MarketplaceTemplate>, crate::models::PaginationMeta), ApiError> {
    #[cfg(feature = "db")]
    {
        common::dev_token_user_id(headers)?;
        return crate::repositories::market::list_templates(&ctx.db, query)
            .await
            .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_login(&state, headers)?;
        let q = query.q.as_deref().unwrap_or_default().to_lowercase();
        let state = state.read().expect("state lock poisoned");
        let items = state
            .templates
            .iter()
            .filter(|template| {
                query
                    .source
                    .as_deref()
                    .is_none_or(|source| template.source_type == source)
                    && query
                        .supports_customization
                        .is_none_or(|value| template.supports_customization == value)
                    && (q.is_empty()
                        || template.title.to_lowercase().contains(&q)
                        || template.summary.to_lowercase().contains(&q))
            })
            .cloned()
            .collect();
        Ok(common::paginate_vec(items, query.limit, query.offset))
    }
}

pub async fn get_template(
    ctx: &AppContext,
    headers: &HeaderMap,
    template_id: Uuid,
) -> Result<MarketplaceTemplate, ApiError> {
    #[cfg(feature = "db")]
    {
        common::dev_token_user_id(headers)?;
        return crate::repositories::market::find_template(&ctx.db, template_id)
            .await
            .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_login(&state, headers)?;
        state
            .read()
            .expect("state lock poisoned")
            .templates
            .iter()
            .find(|item| item.id == template_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("template"))
    }
}

pub async fn copy_template(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    template_id: Uuid,
) -> Result<Storybook, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let template = crate::repositories::market::find_template(&ctx.db, template_id)
            .await
            .map_err(common::db_error)?;
        let template_title = template.title.clone();
        let template_source_type = template.source_type.clone();
        let book = crate::repositories::storybooks::create_from_marketplace_template(
            &ctx.db,
            workspace_id,
            template,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "marketplace_template.copied",
            "storybook",
            Some(book.id),
            json!({
                "template_id": template_id,
                "template_title": template_title,
                "source_type": template_source_type,
                "storybook_title": book.title,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(book);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_editor(&state, headers, workspace_id)?;
        let mut state = state.write().expect("state lock poisoned");
        let template = state
            .templates
            .iter()
            .find(|item| item.id == template_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("template"))?;
        let book = Storybook {
            id: Uuid::new_v4(),
            workspace_id,
            title: template.title,
            storybook_type: StorybookType::Plain,
            status: StorybookStatus::Draft,
            visibility: Visibility::Private,
            source: "marketplace".to_string(),
            source_title: Some(template.source_label),
            target_child_id: None,
            creator_name: state.current_user.display_name.clone(),
            updated_at: "刚刚".to_string(),
            age_group: template.age_group,
            use_scene: template.use_scene,
            teaching_goal: template.summary,
            cover_tone: "柔和、安静".to_string(),
            pages: mock_pages(),
            roles: mock_roles(),
        };
        state.storybooks.push(book.clone());
        Ok(book)
    }
}

#[cfg(not(feature = "db"))]
fn shared_state(ctx: &AppContext) -> Result<crate::state::SharedState, ApiError> {
    ctx.shared_store
        .get::<crate::state::SharedState>()
        .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))
}

#[cfg(not(feature = "db"))]
fn mock_pages() -> Vec<StorybookPage> {
    vec![StorybookPage {
        id: Uuid::new_v4(),
        page_number: 1,
        title: "第一页".to_string(),
        body: "老师确认故事方案后，孩子们一起进入故事。".to_string(),
        illustration_prompt: "温暖教室，老师和孩子围坐阅读。".to_string(),
        status: "ready".to_string(),
    }]
}

#[cfg(not(feature = "db"))]
fn mock_roles() -> Vec<StorybookRole> {
    vec![StorybookRole {
        id: Uuid::new_v4(),
        name: "老师形象".to_string(),
        role_type: "teacher".to_string(),
        appearance: "温柔、清楚、适合幼儿园场景".to_string(),
        story_function: "引导故事推进".to_string(),
        needs_consistency: true,
        reference_image_url: None,
        reference_image_prompt: None,
        reference_status: "not_started".to_string(),
    }]
}
