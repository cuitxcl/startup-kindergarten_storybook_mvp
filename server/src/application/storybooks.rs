use axum::http::HeaderMap;
use loco_rs::app::AppContext;
use std::collections::HashSet;
use uuid::Uuid;

#[cfg(feature = "db")]
use serde_json::json;

use crate::{
    domains::common,
    error::ApiError,
    models::{
        CreateStorybookRequest, DeriveCustomBatchRequest, DeriveCustomBatchResponse,
        DeriveCustomRequest, Storybook, StorybookListQuery, StorybookPage, StorybookRole,
        StorybookStatus, StorybookType, UpdatePageRequest, UpdateRoleRequest,
        UpdateStorybookRequest, Visibility,
    },
};

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
        let book = crate::repositories::storybooks::create_plain(
            &ctx.db,
            workspace_id,
            CreateStorybookRequest {
                title,
                age_group,
                use_scene,
                teaching_goal,
            },
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
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
            pages: mock_pages(),
            roles: mock_roles(),
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
            age_group: clean_optional(payload.age_group, "age_group")?,
            use_scene: clean_optional(payload.use_scene, "use_scene")?,
            teaching_goal: clean_optional(payload.teaching_goal, "teaching_goal")?,
            cover_tone: clean_optional(payload.cover_tone, "cover_tone")?,
        };
        let book =
            crate::repositories::storybooks::update(&ctx.db, workspace_id, storybook_id, payload)
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
        book.updated_at = "刚刚".to_string();
        Ok(book.clone())
    }
}

pub async fn duplicate(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<Storybook, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let book = crate::repositories::storybooks::duplicate(&ctx.db, workspace_id, storybook_id)
            .await
            .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
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
        let mut book = source;
        book.id = Uuid::new_v4();
        book.title = format!("{} 副本", source_title);
        book.status = StorybookStatus::Draft;
        book.visibility = Visibility::Private;
        book.source = "duplicate".to_string();
        book.source_title = Some(source_title);
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

pub async fn update_page(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    page_id: Uuid,
    payload: UpdatePageRequest,
) -> Result<StorybookPage, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let payload = UpdatePageRequest {
            title: clean_optional(payload.title, "title")?,
            body: clean_optional(payload.body, "body")?,
            illustration_prompt: clean_optional(
                payload.illustration_prompt,
                "illustration_prompt",
            )?,
            status: payload.status,
        };
        let page = crate::repositories::storybooks::update_page(
            &ctx.db,
            workspace_id,
            storybook_id,
            page_id,
            payload,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "storybook.page_updated",
            "storybook_page",
            Some(page.id),
            json!({
                "storybook_id": storybook_id,
                "page_number": page.page_number,
                "status": page_status_name(&page.status),
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(page);
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
        let page = book
            .pages
            .iter_mut()
            .find(|item| item.id == page_id)
            .ok_or_else(|| ApiError::not_found("page"))?;
        if let Some(value) = payload.title {
            page.title = common::required(value, "title")?;
        }
        if let Some(value) = payload.body {
            page.body = common::required(value, "body")?;
        }
        if let Some(value) = payload.illustration_prompt {
            page.illustration_prompt = common::required(value, "illustration_prompt")?;
        }
        if let Some(value) = payload.status {
            page.status = value;
        }
        let page = page.clone();
        book.updated_at = "刚刚".to_string();
        Ok(page)
    }
}

pub async fn update_role(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    role_id: Uuid,
    payload: UpdateRoleRequest,
) -> Result<StorybookRole, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_editor_db(ctx, headers, workspace_id).await?;
        let payload = UpdateRoleRequest {
            name: clean_optional(payload.name, "name")?,
            role_type: clean_optional(payload.role_type, "role_type")?,
            appearance: clean_optional(payload.appearance, "appearance")?,
            story_function: clean_optional(payload.story_function, "story_function")?,
            needs_consistency: payload.needs_consistency,
        };
        let role = crate::repositories::storybooks::update_role(
            &ctx.db,
            workspace_id,
            storybook_id,
            role_id,
            payload,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "storybook.role_updated",
            "storybook_role",
            Some(role.id),
            json!({
                "storybook_id": storybook_id,
                "name": role.name,
                "role_type": role.role_type,
                "needs_consistency": role.needs_consistency,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(role);
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
        let role = book
            .roles
            .iter_mut()
            .find(|item| item.id == role_id)
            .ok_or_else(|| ApiError::not_found("role"))?;
        if let Some(value) = payload.name {
            role.name = common::required(value, "name")?;
        }
        if let Some(value) = payload.role_type {
            role.role_type = common::required(value, "role_type")?;
        }
        if let Some(value) = payload.appearance {
            role.appearance = common::required(value, "appearance")?;
        }
        if let Some(value) = payload.story_function {
            role.story_function = common::required(value, "story_function")?;
        }
        if let Some(value) = payload.needs_consistency {
            role.needs_consistency = value;
        }
        let role = role.clone();
        book.updated_at = "刚刚".to_string();
        Ok(role)
    }
}

pub async fn derive_custom(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    payload: DeriveCustomRequest,
) -> Result<Storybook, ApiError> {
    #[cfg(feature = "db")]
    {
        let workspace = common::require_editor_db(ctx, headers, workspace_id).await?;
        if let Some(classrooms) =
            common::child_classroom_scope(ctx, headers, workspace_id, &workspace).await?
        {
            crate::repositories::children::find_for_classrooms(
                &ctx.db,
                workspace_id,
                payload.child_id,
                &classrooms,
            )
            .await
            .map_err(common::db_error)?;
        }
        let child_id = payload.child_id;
        let intensity = payload.intensity.clone();
        let book = crate::repositories::storybooks::derive_custom(
            &ctx.db,
            workspace_id,
            storybook_id,
            payload,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "storybook.custom_derived",
            "storybook",
            Some(book.id),
            json!({
                "source_storybook_id": storybook_id,
                "target_child_id": child_id,
                "intensity": intensity,
                "title": book.title,
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
        if source.storybook_type != StorybookType::Plain {
            return Err(ApiError::state_conflict("只有普通绘本可以派生定制绘本"));
        }
        let child = state
            .children
            .iter()
            .find(|item| item.workspace_id == workspace_id && item.id == payload.child_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("child"))?;
        let book = build_mock_custom_book(
            workspace_id,
            source,
            child.id,
            &child.nickname,
            &payload.intensity,
        );
        state.storybooks.push(book.clone());
        Ok(book)
    }
}

pub async fn derive_custom_batch(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    payload: DeriveCustomBatchRequest,
) -> Result<DeriveCustomBatchResponse, ApiError> {
    validate_custom_batch_payload(&payload)?;

    #[cfg(feature = "db")]
    {
        let workspace = common::require_editor_db(ctx, headers, workspace_id).await?;
        let source = crate::repositories::storybooks::find(&ctx.db, workspace_id, storybook_id)
            .await
            .map_err(common::db_error)?;
        if source.storybook_type != StorybookType::Plain {
            return Err(ApiError::state_conflict("只有普通绘本可以派生定制绘本"));
        }

        if let Some(classrooms) =
            common::child_classroom_scope(ctx, headers, workspace_id, &workspace).await?
        {
            for child_id in &payload.child_ids {
                crate::repositories::children::find_for_classrooms(
                    &ctx.db,
                    workspace_id,
                    *child_id,
                    &classrooms,
                )
                .await
                .map_err(common::db_error)?;
            }
        } else {
            for child_id in &payload.child_ids {
                crate::repositories::children::find(&ctx.db, workspace_id, *child_id)
                    .await
                    .map_err(common::db_error)?;
            }
        }

        let mut storybooks = Vec::with_capacity(payload.child_ids.len());
        for child_id in &payload.child_ids {
            let book = crate::repositories::storybooks::derive_custom(
                &ctx.db,
                workspace_id,
                storybook_id,
                DeriveCustomRequest {
                    child_id: *child_id,
                    intensity: payload.intensity.clone(),
                    customization_plan: payload.customization_plan.clone(),
                },
            )
            .await
            .map_err(common::db_error)?;
            storybooks.push(book);
        }

        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "storybook.custom_batch_derived",
            "storybook",
            Some(storybook_id),
            json!({
                "source_storybook_id": storybook_id,
                "target_child_ids": &payload.child_ids,
                "intensity": &payload.intensity,
                "created_count": storybooks.len(),
            }),
        )
        .await
        .map_err(common::db_error)?;

        return Ok(DeriveCustomBatchResponse {
            source_storybook_id: storybook_id,
            requested_count: payload.child_ids.len(),
            created_count: storybooks.len(),
            storybooks,
        });
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
        if source.storybook_type != StorybookType::Plain {
            return Err(ApiError::state_conflict("只有普通绘本可以派生定制绘本"));
        }

        let mut storybooks = Vec::with_capacity(payload.child_ids.len());
        for child_id in &payload.child_ids {
            let child = state
                .children
                .iter()
                .find(|item| item.workspace_id == workspace_id && item.id == *child_id)
                .cloned()
                .ok_or_else(|| ApiError::not_found("child"))?;
            let book = build_mock_custom_book(
                workspace_id,
                source.clone(),
                child.id,
                &child.nickname,
                &payload.intensity,
            );
            state.storybooks.push(book.clone());
            storybooks.push(book);
        }

        Ok(DeriveCustomBatchResponse {
            source_storybook_id: storybook_id,
            requested_count: payload.child_ids.len(),
            created_count: storybooks.len(),
            storybooks,
        })
    }
}

fn clean_optional(value: Option<String>, field: &'static str) -> Result<Option<String>, ApiError> {
    value
        .map(|value| common::required(value, field))
        .transpose()
}

fn validate_custom_batch_payload(payload: &DeriveCustomBatchRequest) -> Result<(), ApiError> {
    if payload.child_ids.is_empty() {
        return Err(ApiError::validation("child_ids", "请选择至少一个儿童档案"));
    }
    if payload.child_ids.len() > 30 {
        return Err(ApiError::validation(
            "child_ids",
            "一次最多为 30 个儿童生成定制绘本",
        ));
    }
    let unique: HashSet<Uuid> = payload.child_ids.iter().copied().collect();
    if unique.len() != payload.child_ids.len() {
        return Err(ApiError::validation("child_ids", "儿童档案不能重复选择"));
    }
    if !matches!(payload.intensity.as_str(), "quick" | "standard") {
        return Err(ApiError::validation(
            "intensity",
            "定制强度只能是 quick 或 standard",
        ));
    }
    Ok(())
}

fn storybook_type_name(value: &StorybookType) -> &'static str {
    match value {
        StorybookType::Plain => "plain",
        StorybookType::Custom => "custom",
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

fn visibility_name(value: &Visibility) -> &'static str {
    match value {
        Visibility::Private => "private",
        Visibility::Workspace => "workspace",
        Visibility::MarketSubmission => "market_submission",
        Visibility::MarketListed => "market_listed",
    }
}

fn page_status_name(value: &str) -> &str {
    value
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
    }]
}

#[cfg(not(feature = "db"))]
fn build_mock_custom_book(
    workspace_id: Uuid,
    source: Storybook,
    child_id: Uuid,
    child_nickname: &str,
    intensity: &str,
) -> Storybook {
    let mut book = source.clone();
    book.id = Uuid::new_v4();
    book.workspace_id = workspace_id;
    book.storybook_type = StorybookType::Custom;
    book.status = StorybookStatus::Editing;
    book.visibility = Visibility::Private;
    book.source = format!("derived:{intensity}");
    book.source_title = Some(source.title);
    book.target_child_id = Some(child_id);
    book.title = format!("{child_nickname}的定制故事");
    book.updated_at = "刚刚".to_string();
    book
}
