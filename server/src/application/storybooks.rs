use axum::http::HeaderMap;
use loco_rs::app::AppContext;
use uuid::Uuid;

#[cfg(feature = "db")]
use serde_json::json;

use crate::{
    application::storybook_inputs::{
        clean_optional, clean_teacher_review_status, storybook_status_name, storybook_type_name,
        visibility_name,
    },
    domains::common,
    error::ApiError,
    models::{
        CreateStorybookRequest, DuplicateStorybookRequest, Storybook, StorybookListQuery,
        StorybookStatus, UpdateStorybookRequest,
    },
    page_aspect::normalize_page_aspect_ratio,
};

#[cfg(not(feature = "db"))]
use crate::models::{StorybookPage, StorybookRole, StorybookType, Visibility};

pub use crate::application::storybook_customization::{derive_custom, derive_custom_batch};
pub use crate::application::storybook_editing::{update_page, update_role};

pub async fn list(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    query: StorybookListQuery,
) -> Result<(Vec<Storybook>, crate::models::PaginationMeta), ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
        return crate::repositories::storybooks::list_by_workspace(&ctx.db, workspace_id, query)
            .await
            .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        let q = query.q.as_deref().unwrap_or_default().to_lowercase();
        let state = state.read().expect("state lock poisoned");
        let items = state
            .storybooks
            .iter()
            .filter(|book| {
                book.workspace_id == workspace_id
                    && query
                        .storybook_type
                        .as_deref()
                        .is_none_or(|value| storybook_type_name(&book.storybook_type) == value)
                    && query
                        .status
                        .as_deref()
                        .is_none_or(|value| storybook_status_name(&book.status) == value)
                    && query
                        .target_child_id
                        .is_none_or(|value| book.target_child_id == Some(value))
                    && (q.is_empty()
                        || book.title.to_lowercase().contains(&q)
                        || book.teaching_goal.to_lowercase().contains(&q))
            })
            .cloned()
            .collect();
        Ok(common::paginate_vec(items, query.limit, query.offset))
    }
}

pub async fn create(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    payload: CreateStorybookRequest,
) -> Result<Storybook, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let title = common::required(payload.title, "title")?;
        let age_group = common::required(payload.age_group, "age_group")?;
        let use_scene = common::required(payload.use_scene, "use_scene")?;
        let teaching_goal = common::required(payload.teaching_goal, "teaching_goal")?;
        let actor_id = common::actor_user_id(headers)?;
        let book = crate::repositories::storybooks::create_plain(
            &ctx.db,
            workspace_id,
            actor_id,
            CreateStorybookRequest {
                title,
                age_group,
                use_scene,
                teaching_goal,
                cover_tone: payload.cover_tone,
                page_aspect_ratio: Some(normalize_page_aspect_ratio(
                    payload.page_aspect_ratio.as_deref(),
                )),
            },
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "storybook.created",
            "storybook",
            Some(book.id),
            json!({
                "title": book.title,
                "type": storybook_type_name(&book.storybook_type),
                "status": storybook_status_name(&book.status),
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
        let book = Storybook {
            id: Uuid::new_v4(),
            workspace_id,
            title: common::required(payload.title, "title")?,
            storybook_type: StorybookType::Plain,
            status: StorybookStatus::PlanPending,
            visibility: Visibility::Private,
            source: "blank".to_string(),
            source_title: None,
            target_child_id: None,
            creator_name: state.current_user.display_name.clone(),
            updated_at: "刚刚".to_string(),
            age_group: common::required(payload.age_group, "age_group")?,
            use_scene: common::required(payload.use_scene, "use_scene")?,
            teaching_goal: common::required(payload.teaching_goal, "teaching_goal")?,
            cover_tone: "温暖、清楚".to_string(),
            page_aspect_ratio: normalize_page_aspect_ratio(payload.page_aspect_ratio.as_deref()),
            teacher_review_status: "pending".to_string(),
            teacher_reviewed_by: None,
            teacher_reviewed_at: None,
            pages: mock_pages(),
            roles: mock_roles(),
            quality: Default::default(),
        };
        state.storybooks.push(book.clone());
        Ok(book)
    }
}

pub async fn get(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<Storybook, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
        return crate::repositories::storybooks::find(&ctx.db, workspace_id, storybook_id)
            .await
            .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        find_storybook(&state, workspace_id, storybook_id)
    }
}

pub async fn update(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    payload: UpdateStorybookRequest,
) -> Result<Storybook, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let payload = UpdateStorybookRequest {
            title: clean_optional(payload.title, "title")?,
            status: payload.status,
            visibility: payload.visibility,
            teacher_review_status: clean_teacher_review_status(payload.teacher_review_status)?,
            age_group: clean_optional(payload.age_group, "age_group")?,
            use_scene: clean_optional(payload.use_scene, "use_scene")?,
            teaching_goal: clean_optional(payload.teaching_goal, "teaching_goal")?,
            cover_tone: clean_optional(payload.cover_tone, "cover_tone")?,
            page_aspect_ratio: payload
                .page_aspect_ratio
                .map(|value| normalize_page_aspect_ratio(Some(&value))),
        };
        let book = crate::repositories::storybooks::update(
            &ctx.db,
            workspace_id,
            storybook_id,
            payload,
            common::actor_user_id(headers)?,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "storybook.updated",
            "storybook",
            Some(book.id),
            json!({
                "title": book.title,
                "status": storybook_status_name(&book.status),
                "visibility": visibility_name(&book.visibility),
                "teacher_review_status": book.teacher_review_status,
                "age_group": book.age_group,
                "use_scene": book.use_scene,
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
        let book = state
            .storybooks
            .iter_mut()
            .find(|item| item.workspace_id == workspace_id && item.id == storybook_id)
            .ok_or_else(|| ApiError::not_found("storybook"))?;
        if let Some(value) = payload.title {
            book.title = common::required(value, "title")?;
        }
        if let Some(value) = payload.status {
            if value == StorybookStatus::Exportable {
                ensure_storybook_ready_to_deliver(book)?;
            }
            book.status = value;
        }
        if let Some(value) = payload.visibility {
            book.visibility = value;
        }
        if let Some(value) = clean_teacher_review_status(payload.teacher_review_status)? {
            if value == "confirmed" {
                crate::repositories::storybook_rules::ensure_teacher_review_ready(book)
                    .map_err(common::db_error)?;
                book.teacher_review_status = "confirmed".to_string();
                book.teacher_reviewed_by = Some(common::actor_user_id(headers)?);
                book.teacher_reviewed_at = Some("刚刚".to_string());
            } else {
                book.teacher_review_status = "pending".to_string();
                book.teacher_reviewed_by = None;
                book.teacher_reviewed_at = None;
            }
        }
        if let Some(value) = payload.age_group {
            book.age_group = common::required(value, "age_group")?;
        }
        if let Some(value) = payload.use_scene {
            book.use_scene = common::required(value, "use_scene")?;
        }
        if let Some(value) = payload.teaching_goal {
            book.teaching_goal = common::required(value, "teaching_goal")?;
        }
        if let Some(value) = payload.cover_tone {
            book.cover_tone = common::required(value, "cover_tone")?;
        }
        if let Some(value) = payload.page_aspect_ratio {
            book.page_aspect_ratio = normalize_page_aspect_ratio(Some(&value));
        }
        book.updated_at = "刚刚".to_string();
        Ok(book.clone())
    }
}

pub async fn duplicate(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    payload: DuplicateStorybookRequest,
) -> Result<Storybook, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let requested_title = clean_optional(payload.title, "title")?;
        let actor_id = common::actor_user_id(headers)?;
        let book = crate::repositories::storybooks::duplicate(
            &ctx.db,
            workspace_id,
            storybook_id,
            actor_id,
            requested_title,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "storybook.duplicated",
            "storybook",
            Some(book.id),
            json!({
                "source_storybook_id": storybook_id,
                "title": book.title,
                "type": storybook_type_name(&book.storybook_type),
                "status": storybook_status_name(&book.status),
                "visibility": visibility_name(&book.visibility),
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
        let source = state
            .storybooks
            .iter()
            .find(|item| item.workspace_id == workspace_id && item.id == storybook_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("storybook"))?;
        let source_title = source.title.clone();
        let requested_title = clean_optional(payload.title, "title")?;
        let mut book = source;
        book.id = Uuid::new_v4();
        book.title = requested_title.unwrap_or_else(|| format!("{} 副本", source_title));
        book.status = StorybookStatus::Draft;
        book.visibility = Visibility::Private;
        book.source = "duplicate".to_string();
        book.source_title = Some(source_title);
        book.teacher_review_status = "pending".to_string();
        book.teacher_reviewed_by = None;
        book.teacher_reviewed_at = None;
        book.updated_at = "刚刚".to_string();
        for page in &mut book.pages {
            page.id = Uuid::new_v4();
        }
        for role in &mut book.roles {
            role.id = Uuid::new_v4();
        }
        state.storybooks.push(book.clone());
        Ok(book)
    }
}

pub async fn delete(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<(), ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let book = crate::repositories::storybooks::find(&ctx.db, workspace_id, storybook_id)
            .await
            .map_err(common::db_error)?;
        // 已投稿/已上架的绘本被 marketplace 投稿或模板引用，不能直接删除。
        if matches!(
            book.status,
            StorybookStatus::Submitted | StorybookStatus::Listed
        ) {
            return Err(ApiError::state_conflict(
                "已投稿或已上架的绘本不能直接删除，请先撤回投稿或下架",
            ));
        }
        crate::repositories::storybooks::delete(&ctx.db, workspace_id, storybook_id)
            .await
            .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "storybook.deleted",
            "storybook",
            Some(storybook_id),
            json!({
                "title": book.title,
                "status": storybook_status_name(&book.status),
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(());
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_editor(&state, headers, workspace_id)?;
        let mut state = state.write().expect("state lock poisoned");
        let index = state
            .storybooks
            .iter()
            .position(|item| item.workspace_id == workspace_id && item.id == storybook_id)
            .ok_or_else(|| ApiError::not_found("storybook"))?;
        if matches!(
            state.storybooks[index].status,
            StorybookStatus::Submitted | StorybookStatus::Listed
        ) {
            return Err(ApiError::state_conflict(
                "已投稿或已上架的绘本不能直接删除，请先撤回投稿或下架",
            ));
        }
        state.storybooks.remove(index);
        Ok(())
    }
}

#[cfg(not(feature = "db"))]
fn ensure_storybook_ready_to_deliver(book: &Storybook) -> Result<(), ApiError> {
    if !matches!(
        book.status,
        StorybookStatus::Editing | StorybookStatus::ImagePending | StorybookStatus::Exportable
    ) {
        return Err(ApiError::state_conflict("绘本需要完成编辑后才能标记可交付"));
    }
    if book.pages.is_empty() {
        return Err(ApiError::state_conflict(
            "绘本至少需要一个分页才能标记可交付",
        ));
    }
    if book.roles.is_empty() {
        return Err(ApiError::state_conflict(
            "绘本至少需要一个角色或道具设定才能标记可交付",
        ));
    }
    if book.pages.iter().any(|page| page.status == "generating") {
        return Err(ApiError::state_conflict(
            "仍有插图正在生成，完成后才能标记可交付",
        ));
    }
    Ok(())
}

#[cfg(not(feature = "db"))]
fn shared_state(ctx: &AppContext) -> Result<crate::state::SharedState, ApiError> {
    ctx.shared_store
        .get::<crate::state::SharedState>()
        .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))
}

#[cfg(not(feature = "db"))]
fn find_storybook(
    state: &crate::state::SharedState,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<Storybook, ApiError> {
    state
        .read()
        .expect("state lock poisoned")
        .storybooks
        .iter()
        .find(|item| item.workspace_id == workspace_id && item.id == storybook_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("storybook"))
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
        image_url: None,
        selected_image_variant_id: None,
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
        selected_image_variant_id: None,
    }]
}
