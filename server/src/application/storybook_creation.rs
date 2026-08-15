use axum::http::HeaderMap;
use loco_rs::app::AppContext;
use sea_orm::TransactionTrait;
use serde_json::json;
use uuid::Uuid;

use crate::{
    domains::common,
    error::ApiError,
    models::{
        CreateGenerationJobRequest, CreateStorybookCreationSessionRequest, CreateStorybookRequest,
        CreationDirectionsResponse, CreationGenerationStep, CreationGenerationSummary,
        CreationMaterial, CreationMaterialsResponse, CreationOutline, CreationOutlinePage,
        CreationOutlineResponse, CreationSessionUpdateResponse,
        CreationStorybookGenerationResponse, CreationUnderstanding,
        GenerateCreationStorybookRequest, GenerateDirectionsRequest, GenerateOutlineRequest,
        PatchCreationMaterialsRequest, RefreshUnderstandingRequest, SelectDirectionRequest,
        SelectDirectionResponse, StoryDirection, StorybookCreationSession,
        StorybookCreationSessionListItem, StorybookCreationSessionListQuery,
        UpdateCreationOutlineRequest, UpdateOutlinePageRequest, UpdateOutlinePageResponse,
        UpdateOutlineResponse, UpdateStorybookCreationSessionRequest,
        UpdateVisualPreferencesRequest, VisualPreferences, VisualPreferencesResponse,
    },
    page_aspect::normalize_page_aspect_ratio,
    services::generation_provider::{ConfiguredGenerationProvider, GenerationRequest},
};

pub async fn create(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    payload: CreateStorybookCreationSessionRequest,
) -> Result<StorybookCreationSession, ApiError> {
    common::require_editor_db(ctx, headers, workspace_id).await?;
    let actor_id = common::actor_user_id(headers)?;
    let quick_idea = validate_quick_idea(payload.quick_idea)?;
    let use_scene = clean_or(payload.use_scene, "家庭共读");
    let age_group = clean_or(payload.age_group, "4-5 岁");
    let page_count = normalize_page_count(payload.page_count);
    let visual_preferences = VisualPreferences {
        style: clean_or(payload.style, "watercolor"),
        page_aspect_ratio: normalize_page_aspect_ratio(payload.page_aspect_ratio.as_deref()),
        visual_complexity: validate_enum(
            payload.visual_complexity,
            &["simple", "standard", "rich"],
            "standard",
            "visual_complexity",
        )?,
        character_consistency: validate_enum(
            payload.character_consistency,
            &["auto", "speed", "confirm_character"],
            "auto",
            "character_consistency",
        )?,
    };
    let (understanding, materials) =
        create_understanding(&quick_idea, &use_scene, &age_group, &[]).await;
    crate::repositories::storybook_creation_sessions::create(
        &ctx.db,
        workspace_id,
        actor_id,
        quick_idea,
        use_scene,
        age_group,
        page_count,
        understanding,
        materials,
        visual_preferences,
    )
    .await
    .map_err(common::db_error)
}

pub async fn list(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    query: StorybookCreationSessionListQuery,
) -> Result<
    (
        Vec<StorybookCreationSessionListItem>,
        crate::models::PaginationMeta,
    ),
    ApiError,
> {
    let workspace = common::require_workspace_db(ctx, headers, workspace_id).await?;
    if let Some(status) = query.status.as_deref() {
        validate_session_list_status(status)?;
    }
    let actor_id = common::actor_user_id(headers)?;
    let can_view_all = matches!(workspace.role, crate::models::WorkspaceRole::SchoolAdmin);
    crate::repositories::storybook_creation_sessions::list(
        &ctx.db,
        workspace_id,
        actor_id,
        can_view_all,
        query,
    )
    .await
    .map_err(common::db_error)
}

pub async fn latest(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    query: StorybookCreationSessionListQuery,
) -> Result<Option<StorybookCreationSession>, ApiError> {
    let workspace = common::require_workspace_db(ctx, headers, workspace_id).await?;
    let actor_id = common::actor_user_id(headers)?;
    let created_by = if matches!(workspace.role, crate::models::WorkspaceRole::SchoolAdmin) {
        query.created_by.unwrap_or(actor_id)
    } else {
        actor_id
    };
    crate::repositories::storybook_creation_sessions::latest_active(
        &ctx.db,
        workspace_id,
        created_by,
    )
    .await
    .map_err(common::db_error)
}

pub async fn get(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
) -> Result<StorybookCreationSession, ApiError> {
    let workspace = common::require_workspace_db(ctx, headers, workspace_id).await?;
    let session =
        crate::repositories::storybook_creation_sessions::find(&ctx.db, workspace_id, session_id)
            .await
            .map_err(common::db_error)?;
    ensure_session_visible(&workspace, &session, common::actor_user_id(headers)?)?;
    Ok(session)
}

pub async fn update(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
    payload: UpdateStorybookCreationSessionRequest,
) -> Result<CreationSessionUpdateResponse, ApiError> {
    let mut session = editable_session(ctx, headers, workspace_id, session_id).await?;
    ensure_not_terminal_for_edit(&session)?;
    if matches!(session.status.as_str(), "generating") {
        return Err(ApiError::state_conflict("生成中不能修改基础输入"));
    }
    if let Some(value) = payload.quick_idea {
        session.quick_idea = validate_quick_idea(value)?;
    }
    if let Some(value) = payload.use_scene {
        session.use_scene = common::required(value, "use_scene")?;
    }
    if let Some(value) = payload.age_group {
        session.age_group = common::required(value, "age_group")?;
    }
    if payload.page_count.is_some() {
        session.page_count = normalize_page_count(payload.page_count);
    }
    session.status = "draft".to_string();
    session.directions.clear();
    session.selected_direction_id = None;
    session.outline = None;
    session.requires_understanding_refresh = true;
    session.requires_direction_refresh = true;
    session.requires_outline_refresh = true;
    session.generation_summary = empty_generation_summary();
    let session = save(ctx, &session).await?;
    Ok(CreationSessionUpdateResponse {
        id: session.id,
        status: session.status,
        requires_understanding_refresh: session.requires_understanding_refresh,
        requires_direction_refresh: session.requires_direction_refresh,
        requires_outline_refresh: session.requires_outline_refresh,
        updated_at: session.updated_at,
    })
}

pub async fn refresh_understanding(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
    payload: RefreshUnderstandingRequest,
) -> Result<StorybookCreationSession, ApiError> {
    let mut session = editable_session(ctx, headers, workspace_id, session_id).await?;
    ensure_not_generating_or_ready(&session)?;
    let preserved = if payload.preserve_user_materials.unwrap_or(true) {
        session
            .materials
            .iter()
            .filter(|item| item.source == "user_added")
            .cloned()
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let (understanding, mut materials) = create_understanding(
        &session.quick_idea,
        &session.use_scene,
        &session.age_group,
        &preserved,
    )
    .await;
    if payload.preserve_user_materials.unwrap_or(true) {
        materials.extend(preserved);
    }
    session.understanding = understanding;
    session.materials = dedupe_materials(materials);
    session.directions.clear();
    session.selected_direction_id = None;
    session.outline = None;
    session.status = "understanding_ready".to_string();
    session.requires_understanding_refresh = false;
    session.requires_direction_refresh = true;
    session.requires_outline_refresh = true;
    session.generation_summary = empty_generation_summary();
    save(ctx, &session).await
}

pub async fn patch_materials(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
    payload: PatchCreationMaterialsRequest,
) -> Result<CreationMaterialsResponse, ApiError> {
    let mut session = editable_session(ctx, headers, workspace_id, session_id).await?;
    ensure_not_generating_or_ready(&session)?;
    for op in payload.operations {
        match op.op.as_str() {
            "add" => {
                let label = common::required(op.label.unwrap_or_default(), "label")?;
                let material_type = validate_material_type(
                    op.material_type.unwrap_or_else(|| "custom".to_string()),
                )?;
                session.materials.push(CreationMaterial {
                    id: next_material_id(&session.materials),
                    label,
                    material_type,
                    source: "user_added".to_string(),
                    confidence: None,
                    locked: op.locked.unwrap_or(true),
                });
            }
            "update" => {
                let id = common::required(op.id.unwrap_or_default(), "id")?;
                let material = session
                    .materials
                    .iter_mut()
                    .find(|item| item.id == id)
                    .ok_or_else(|| ApiError::not_found("material"))?;
                if let Some(label) = op.label {
                    material.label = common::required(label, "label")?;
                }
                if let Some(material_type) = op.material_type {
                    material.material_type = validate_material_type(material_type)?;
                }
                if let Some(locked) = op.locked {
                    material.locked = locked;
                }
            }
            "remove" => {
                let id = common::required(op.id.unwrap_or_default(), "id")?;
                let before = session.materials.len();
                session.materials.retain(|item| item.id != id);
                if before == session.materials.len() {
                    return Err(ApiError::not_found("material"));
                }
            }
            _ => return Err(ApiError::validation("op", "不支持的素材操作")),
        }
    }
    session.materials = dedupe_materials(session.materials);
    session.requires_direction_refresh = true;
    session.requires_outline_refresh = true;
    if matches!(
        session.status.as_str(),
        "directions_ready" | "direction_selected" | "outline_ready" | "failed"
    ) {
        session.status = "understanding_ready".to_string();
        session.directions.clear();
        session.selected_direction_id = None;
        session.outline = None;
        session.generation_summary = empty_generation_summary();
    }
    let session = save(ctx, &session).await?;
    Ok(CreationMaterialsResponse {
        id: session.id,
        status: session.status,
        materials: session.materials,
        requires_direction_refresh: session.requires_direction_refresh,
        requires_outline_refresh: session.requires_outline_refresh,
        updated_at: session.updated_at,
    })
}

pub async fn generate_directions(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
    payload: GenerateDirectionsRequest,
) -> Result<CreationDirectionsResponse, ApiError> {
    let mut session = editable_session(ctx, headers, workspace_id, session_id).await?;
    ensure_status_any(
        &session,
        &["understanding_ready", "directions_ready", "failed"],
    )?;
    if let Some(reason) = payload.refresh_reason.as_deref()
        && ![
            "initial",
            "user_clicked_refresh",
            "user_added_material",
            "user_changed_idea",
        ]
        .contains(&reason)
    {
        return Err(ApiError::validation(
            "refresh_reason",
            "不支持的方向刷新原因",
        ));
    }
    let count = payload.direction_count.unwrap_or(3).clamp(1, 5);
    session.directions = create_directions(&session, count).await;
    session.selected_direction_id = None;
    session.outline = None;
    session.status = "directions_ready".to_string();
    session.requires_direction_refresh = false;
    session.requires_outline_refresh = true;
    let session = save(ctx, &session).await?;
    Ok(CreationDirectionsResponse {
        status: session.status,
        directions: session.directions,
        next_action: "select_direction".to_string(),
    })
}

pub async fn select_direction(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
    payload: SelectDirectionRequest,
) -> Result<SelectDirectionResponse, ApiError> {
    let mut session = editable_session(ctx, headers, workspace_id, session_id).await?;
    ensure_status_any(&session, &["directions_ready"])?;
    let selected = session
        .directions
        .iter()
        .find(|item| item.id == payload.direction_id)
        .cloned()
        .ok_or_else(|| ApiError::validation("direction_id", "故事方向不存在"))?;
    session.selected_direction_id = Some(selected.id.clone());
    session.outline = None;
    session.status = "direction_selected".to_string();
    session.requires_outline_refresh = true;
    let session = save(ctx, &session).await?;
    Ok(SelectDirectionResponse {
        status: session.status,
        selected_direction_id: selected.id.clone(),
        selected_direction: selected,
        next_action: "generate_outline".to_string(),
    })
}

pub async fn generate_outline(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
    payload: GenerateOutlineRequest,
) -> Result<CreationOutlineResponse, ApiError> {
    let mut session = editable_session(ctx, headers, workspace_id, session_id).await?;
    ensure_status_any(&session, &["direction_selected"])?;
    let page_count = normalize_page_count(payload.page_count.or(Some(session.page_count)));
    session.page_count = page_count;
    let outline = create_outline(&session, page_count).await?;
    session.outline = Some(outline.clone());
    session.status = "outline_ready".to_string();
    session.requires_outline_refresh = false;
    session.generation_summary = empty_generation_summary();
    let session = save(ctx, &session).await?;
    Ok(CreationOutlineResponse {
        status: session.status,
        outline,
        next_action: "confirm_outline".to_string(),
    })
}

pub async fn update_outline_page(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
    page_number: u32,
    payload: UpdateOutlinePageRequest,
) -> Result<UpdateOutlinePageResponse, ApiError> {
    let mut session = editable_session(ctx, headers, workspace_id, session_id).await?;
    ensure_status_any(&session, &["outline_ready", "failed"])?;
    let mut outline = session
        .outline
        .clone()
        .ok_or_else(|| ApiError::state_conflict("还没有可编辑的大纲"))?;
    let instruction = common::required(payload.instruction, "instruction")?;
    let page = outline
        .pages
        .iter_mut()
        .find(|item| item.page_number == page_number)
        .ok_or_else(|| ApiError::not_found("outline_page"))?;
    page.summary = format!("{}（已按要求调整：{}）", page.summary, instruction);
    let response_page = page.clone();
    session.outline = Some(outline);
    session.status = "outline_ready".to_string();
    session.generation_summary = empty_generation_summary();
    save(ctx, &session).await?;
    Ok(UpdateOutlinePageResponse {
        page: response_page,
        requires_storybook_regeneration: session.storybook_id.is_some(),
    })
}

pub async fn update_outline(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
    payload: UpdateCreationOutlineRequest,
) -> Result<UpdateOutlineResponse, ApiError> {
    let mut session = editable_session(ctx, headers, workspace_id, session_id).await?;
    ensure_status_any(&session, &["outline_ready", "failed"])?;
    validate_outline_pages(&payload.pages)?;
    let outline = CreationOutline {
        summary: common::required(payload.summary, "summary")?,
        pages: payload.pages,
        review_points: payload.review_points,
        generation_source: Some("user_edited".to_string()),
        quality_flags: Vec::new(),
    };
    session.outline = Some(outline.clone());
    session.status = "outline_ready".to_string();
    session.requires_outline_refresh = false;
    let had_storybook = session.storybook_id.is_some();
    save(ctx, &session).await?;
    Ok(UpdateOutlineResponse {
        status: "outline_ready".to_string(),
        outline,
        requires_storybook_regeneration: had_storybook,
    })
}

pub async fn update_visual_preferences(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
    payload: UpdateVisualPreferencesRequest,
) -> Result<VisualPreferencesResponse, ApiError> {
    let mut session = editable_session(ctx, headers, workspace_id, session_id).await?;
    ensure_not_generating_or_ready(&session)?;
    if let Some(style) = payload.style {
        session.visual_preferences.style = common::required(style, "style")?;
    }
    if let Some(page_aspect_ratio) = payload.page_aspect_ratio {
        session.visual_preferences.page_aspect_ratio =
            normalize_page_aspect_ratio(Some(&page_aspect_ratio));
    }
    if let Some(value) = payload.visual_complexity {
        session.visual_preferences.visual_complexity = validate_enum(
            Some(value),
            &["simple", "standard", "rich"],
            "standard",
            "visual_complexity",
        )?;
    }
    if let Some(value) = payload.character_consistency {
        session.visual_preferences.character_consistency = validate_enum(
            Some(value),
            &["auto", "speed", "confirm_character"],
            "auto",
            "character_consistency",
        )?;
    }
    let requires_storybook_regeneration = session.storybook_id.is_some();
    if requires_storybook_regeneration {
        session.generation_summary = empty_generation_summary();
    }
    let session = save(ctx, &session).await?;
    Ok(VisualPreferencesResponse {
        id: session.id,
        status: session.status,
        visual_preferences: session.visual_preferences,
        requires_storybook_regeneration,
        updated_at: session.updated_at,
    })
}

pub async fn generate_storybook(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
    payload: GenerateCreationStorybookRequest,
) -> Result<CreationStorybookGenerationResponse, ApiError> {
    let workspace = common::require_editor_db(ctx, headers, workspace_id).await?;
    let actor_id = common::actor_user_id(headers)?;
    let idempotency_key = common::required(payload.idempotency_key, "idempotency_key")?;
    if idempotency_key.len() < 8 {
        return Err(ApiError::validation(
            "idempotency_key",
            "幂等键至少需要 8 个字符",
        ));
    }
    let generation_mode = common::required(payload.generation_mode, "generation_mode")?;
    if generation_mode != "full_draft" {
        return Err(ApiError::validation(
            "generation_mode",
            "当前仅支持 full_draft",
        ));
    }

    crate::repositories::generation_costs::ensure_generation_budget_available(
        &ctx.db,
        Some(workspace_id),
    )
    .await
    .map_err(common::db_error)?;

    let include_images = payload.include_images.unwrap_or(true);
    let txn = ctx.db.begin().await.map_err(common::db_error)?;
    let mut session = crate::repositories::storybook_creation_sessions::find_for_update(
        &txn,
        workspace_id,
        session_id,
    )
    .await
    .map_err(common::db_error)?;
    ensure_session_visible(&workspace, &session, actor_id)?;

    if session.status == "generating" {
        txn.commit().await.map_err(common::db_error)?;
        return Ok(generation_response(&session, include_images));
    }
    if session.status == "storybook_ready" {
        txn.commit().await.map_err(common::db_error)?;
        return Ok(CreationStorybookGenerationResponse {
            status: session.status,
            storybook_id: session.storybook_id,
            job_id: session.last_job_id,
            generation_summary: session.generation_summary,
            steps: generation_steps(include_images, "succeeded"),
            next_action: "open_review_workspace".to_string(),
        });
    }
    if session.idempotency_key.as_deref() == Some(idempotency_key.as_str())
        && session.storybook_id.is_some()
    {
        txn.commit().await.map_err(common::db_error)?;
        return Ok(generation_response(&session, include_images));
    }
    ensure_status_any(&session, &["outline_ready", "failed"])?;
    let outline = session
        .outline
        .clone()
        .ok_or_else(|| ApiError::state_conflict("生成绘本前需要先确认大纲"))?;
    let selected_direction = selected_direction(&session)?;
    let storybook_id = match session.storybook_id {
        Some(storybook_id) => storybook_id,
        None => {
            let storybook_id =
                crate::repositories::storybook_creation_sessions::create_storybook_shell_in_tx(
                    &txn,
                    workspace_id,
                    actor_id,
                    CreateStorybookRequest {
                        title: selected_direction.title.clone(),
                        age_group: session.understanding.age_group.clone(),
                        use_scene: session.understanding.scene.clone(),
                        teaching_goal: session.understanding.goal.clone(),
                        cover_tone: Some(session.visual_preferences.style.clone()),
                        page_aspect_ratio: Some(
                            session.visual_preferences.page_aspect_ratio.clone(),
                        ),
                    },
                )
                .await
                .map_err(common::db_error)?;
            storybook_id
        }
    };
    crate::repositories::storybook_creation_sessions::replace_storybook_content(
        &txn,
        storybook_id,
        &outline,
        &session.materials,
        &selected_direction,
        &session.visual_preferences,
    )
    .await
    .map_err(common::db_error)?;

    let input_json = json!({
        "creation_session_id": session.id,
        "storybook_id": storybook_id,
        "quick_idea": session.quick_idea,
        "understanding": session.understanding,
        "materials": session.materials,
        "selected_direction": selected_direction,
        "outline": outline,
        "visual_preferences": session.visual_preferences,
        "generation_mode": generation_mode,
        "include_images": include_images,
        "idempotency_key": idempotency_key,
    });
    let job = crate::repositories::storybook_creation_sessions::enqueue_creation_job_in_tx(
        &txn,
        workspace_id,
        actor_id,
        CreateGenerationJobRequest {
            job_type: "creation_storybook_generate".to_string(),
            storybook_id: Some(storybook_id),
            input_json,
        },
    )
    .await
    .map_err(common::db_error)?;

    session.status = "generating".to_string();
    session.storybook_id = Some(storybook_id);
    session.last_job_id = Some(job.id);
    session.idempotency_key = Some(idempotency_key);
    session.generation_summary = CreationGenerationSummary {
        text_generation_status: "generating".to_string(),
        image_generation_status: if include_images { "pending" } else { "skipped" }.to_string(),
        quality_notice: None,
        recoverable_actions: Vec::new(),
    };
    crate::repositories::storybook_creation_sessions::save_in_tx(&txn, &session)
        .await
        .map_err(common::db_error)?;
    txn.commit().await.map_err(common::db_error)?;

    crate::workers::generation::enqueue_generation_job(ctx, workspace_id, job.id)
        .await
        .map_err(|err| ApiError::state_conflict(format!("生成任务入队失败：{err}")))?;

    Ok(generation_response(&session, include_images))
}

pub async fn abandon(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
) -> Result<CreationSessionUpdateResponse, ApiError> {
    let mut session = editable_session(ctx, headers, workspace_id, session_id).await?;
    if session.status == "storybook_ready" {
        return Err(ApiError::state_conflict("已生成绘本的会话不能放弃"));
    }
    if session.status != "abandoned" {
        session.status = "abandoned".to_string();
        session = save(ctx, &session).await?;
    }
    Ok(CreationSessionUpdateResponse {
        id: session.id,
        status: session.status,
        requires_understanding_refresh: session.requires_understanding_refresh,
        requires_direction_refresh: session.requires_direction_refresh,
        requires_outline_refresh: session.requires_outline_refresh,
        updated_at: session.updated_at,
    })
}

async fn editable_session(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
) -> Result<StorybookCreationSession, ApiError> {
    common::require_editor_db(ctx, headers, workspace_id).await?;
    get(ctx, headers, workspace_id, session_id).await
}

async fn save(
    ctx: &AppContext,
    session: &StorybookCreationSession,
) -> Result<StorybookCreationSession, ApiError> {
    crate::repositories::storybook_creation_sessions::save(&ctx.db, session)
        .await
        .map_err(common::db_error)
}

fn ensure_session_visible(
    workspace: &crate::models::Workspace,
    session: &StorybookCreationSession,
    actor_id: Uuid,
) -> Result<(), ApiError> {
    if matches!(workspace.role, crate::models::WorkspaceRole::SchoolAdmin)
        || session.created_by == actor_id
    {
        Ok(())
    } else {
        Err(ApiError::forbidden("只能访问自己创建的共创会话"))
    }
}

fn ensure_status_any(session: &StorybookCreationSession, allowed: &[&str]) -> Result<(), ApiError> {
    if allowed.contains(&session.status.as_str()) {
        Ok(())
    } else {
        Err(ApiError::state_conflict(format!(
            "当前状态 {} 不能执行该操作",
            session.status
        )))
    }
}

fn ensure_not_terminal_for_edit(session: &StorybookCreationSession) -> Result<(), ApiError> {
    if matches!(session.status.as_str(), "storybook_ready" | "abandoned") {
        Err(ApiError::state_conflict("当前会话已结束，不能继续编辑"))
    } else {
        Ok(())
    }
}

fn ensure_not_generating_or_ready(session: &StorybookCreationSession) -> Result<(), ApiError> {
    if matches!(
        session.status.as_str(),
        "generating" | "storybook_ready" | "abandoned"
    ) {
        Err(ApiError::state_conflict("当前状态不能编辑共创内容"))
    } else {
        Ok(())
    }
}

fn validate_quick_idea(value: String) -> Result<String, ApiError> {
    let value = common::required(value, "quick_idea")?;
    if value.chars().count() < 8 {
        return Err(ApiError::validation(
            "quick_idea",
            "故事想法至少需要 8 个字符",
        ));
    }
    Ok(value)
}

fn clean_or(value: Option<String>, fallback: &str) -> String {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn validate_enum(
    value: Option<String>,
    allowed: &[&str],
    fallback: &str,
    field: &'static str,
) -> Result<String, ApiError> {
    let value = clean_or(value, fallback);
    if allowed.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(ApiError::validation(field, "不支持的枚举值"))
    }
}

fn normalize_page_count(value: Option<u32>) -> u32 {
    value.unwrap_or(6).clamp(4, 12)
}

async fn create_understanding(
    quick_idea: &str,
    use_scene: &str,
    age_group: &str,
    preserved_materials: &[CreationMaterial],
) -> (CreationUnderstanding, Vec<CreationMaterial>) {
    if ConfiguredGenerationProvider::ready_for_text() {
        let provider = ConfiguredGenerationProvider::from_env();
        let input = json!({
            "quick_idea": quick_idea,
            "use_scene": use_scene,
            "age_group": age_group,
            "preserved_user_materials": preserved_materials,
        });
        match provider
            .generate(GenerationRequest {
                job_type: "creation_understanding",
                input: &input,
            })
            .await
            .and_then(|output| parse_understanding_output(output, "ai"))
        {
            Ok(result) => return result,
            Err(err) => {
                let (mut understanding, materials) = fallback_understanding(
                    quick_idea,
                    use_scene,
                    age_group,
                    Some(err.safe_message()),
                );
                understanding
                    .quality_flags
                    .push("ai_output_rejected".to_string());
                return (understanding, materials);
            }
        }
    }
    fallback_understanding(
        quick_idea,
        use_scene,
        age_group,
        Some("real_text_provider_not_ready".to_string()),
    )
}

async fn create_directions(session: &StorybookCreationSession, count: u32) -> Vec<StoryDirection> {
    if ConfiguredGenerationProvider::ready_for_text() {
        let provider = ConfiguredGenerationProvider::from_env();
        let input = json!({
            "quick_idea": session.quick_idea,
            "understanding": session.understanding,
            "materials": session.materials,
            "direction_count": count,
        });
        match provider
            .generate(GenerationRequest {
                job_type: "creation_directions",
                input: &input,
            })
            .await
            .and_then(|output| parse_directions_output(output, count, &session.materials))
        {
            Ok(directions) => return directions,
            Err(err) => {
                return build_directions(
                    session,
                    count,
                    Some(format!("ai_output_rejected:{}", err.safe_message())),
                );
            }
        }
    }
    build_directions(
        session,
        count,
        Some("real_text_provider_not_ready".to_string()),
    )
}

async fn create_outline(
    session: &StorybookCreationSession,
    page_count: u32,
) -> Result<CreationOutline, ApiError> {
    if ConfiguredGenerationProvider::ready_for_text() {
        let provider = ConfiguredGenerationProvider::from_env();
        let selected_direction = selected_direction(session)?;
        let input = json!({
            "quick_idea": session.quick_idea,
            "understanding": session.understanding,
            "materials": session.materials,
            "selected_direction": selected_direction,
            "visual_preferences": session.visual_preferences,
            "page_count": page_count,
        });
        match provider
            .generate(GenerationRequest {
                job_type: "creation_outline",
                input: &input,
            })
            .await
            .and_then(|output| parse_outline_output(output, page_count, &session.materials))
        {
            Ok(outline) => return Ok(outline),
            Err(err) => {
                return build_outline(
                    session,
                    page_count,
                    Some(format!("ai_output_rejected:{}", err.safe_message())),
                );
            }
        }
    }
    build_outline(
        session,
        page_count,
        Some("real_text_provider_not_ready".to_string()),
    )
}

fn parse_understanding_output(
    output: serde_json::Value,
    generation_source: &str,
) -> Result<
    (CreationUnderstanding, Vec<CreationMaterial>),
    crate::services::generation_provider::GenerationProviderError,
> {
    let mut understanding: CreationUnderstanding = serde_json::from_value(
        output.get("understanding").cloned().unwrap_or_default(),
    )
    .map_err(|err| {
        crate::services::generation_provider::GenerationProviderError::new(format!(
            "creation_understanding.understanding 解析失败：{err}"
        ))
    })?;
    let materials: Vec<CreationMaterial> = serde_json::from_value(
        output.get("materials").cloned().unwrap_or_default(),
    )
    .map_err(|err| {
        crate::services::generation_provider::GenerationProviderError::new(format!(
            "creation_understanding.materials 解析失败：{err}"
        ))
    })?;
    if materials.is_empty() {
        return Err(
            crate::services::generation_provider::GenerationProviderError::new(
                "creation_understanding.materials 不能为空",
            ),
        );
    }
    understanding.generation_source = Some(generation_source.to_string());
    understanding.quality_flags = output
        .get("quality_flags")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok((understanding, normalize_material_ids(materials)))
}

fn parse_directions_output(
    output: serde_json::Value,
    count: u32,
    materials: &[CreationMaterial],
) -> Result<Vec<StoryDirection>, crate::services::generation_provider::GenerationProviderError> {
    let mut directions: Vec<StoryDirection> = serde_json::from_value(
        output.get("directions").cloned().unwrap_or_default(),
    )
    .map_err(|err| {
        crate::services::generation_provider::GenerationProviderError::new(format!(
            "creation_directions.directions 解析失败：{err}"
        ))
    })?;
    if directions.len() < count as usize {
        return Err(
            crate::services::generation_provider::GenerationProviderError::new(
                "creation_directions.directions 数量不足",
            ),
        );
    }
    let allowed_ids = materials
        .iter()
        .map(|item| item.id.as_str())
        .collect::<Vec<_>>();
    for (index, direction) in directions.iter_mut().enumerate() {
        direction.generation_source = Some("ai".to_string());
        direction.quality_flags = output
            .get("quality_flags")
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if direction.id.trim().is_empty() {
            direction.id = format!("dir_{}", index + 1);
        }
        direction
            .material_ids
            .retain(|id| allowed_ids.contains(&id.as_str()));
        if direction.material_ids.is_empty() {
            direction.material_ids = materials
                .iter()
                .take(2)
                .map(|item| item.id.clone())
                .collect();
        }
    }
    Ok(directions.into_iter().take(count as usize).collect())
}

fn parse_outline_output(
    output: serde_json::Value,
    page_count: u32,
    materials: &[CreationMaterial],
) -> Result<CreationOutline, crate::services::generation_provider::GenerationProviderError> {
    let mut outline: CreationOutline = serde_json::from_value(
        output.get("outline").cloned().unwrap_or_default(),
    )
    .map_err(|err| {
        crate::services::generation_provider::GenerationProviderError::new(format!(
            "creation_outline.outline 解析失败：{err}"
        ))
    })?;
    if outline.pages.len() != page_count as usize {
        return Err(
            crate::services::generation_provider::GenerationProviderError::new(
                "creation_outline.outline.pages 数量与 page_count 不一致",
            ),
        );
    }
    let allowed_ids = materials
        .iter()
        .map(|item| item.id.as_str())
        .collect::<Vec<_>>();
    for (index, page) in outline.pages.iter_mut().enumerate() {
        page.page_number = (index + 1) as u32;
        page.material_ids
            .retain(|id| allowed_ids.contains(&id.as_str()));
        if page.material_ids.is_empty() {
            page.material_ids = materials
                .iter()
                .take(2)
                .map(|item| item.id.clone())
                .collect();
        }
    }
    outline.generation_source = Some("ai".to_string());
    outline.quality_flags = output
        .get("quality_flags")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(outline)
}

fn fallback_understanding(
    quick_idea: &str,
    use_scene: &str,
    age_group: &str,
    reason: Option<String>,
) -> (CreationUnderstanding, Vec<CreationMaterial>) {
    let mut understanding = understand(quick_idea, use_scene, age_group);
    understanding.generation_source = Some("fallback".to_string());
    if let Some(reason) = reason {
        understanding.quality_flags.push(reason);
    }
    (understanding, extract_materials(quick_idea))
}

fn understand(quick_idea: &str, use_scene: &str, age_group: &str) -> CreationUnderstanding {
    CreationUnderstanding {
        summary: format!(
            "为{}创作一本关于{}的专属成长故事。",
            age_group,
            compact_idea(quick_idea)
        ),
        target_user: if quick_idea.contains("老师") || quick_idea.contains("班") {
            "teacher".to_string()
        } else {
            "parent".to_string()
        },
        goal: infer_goal(quick_idea),
        tone: if quick_idea.contains("温柔") {
            "温柔、鼓励、不说教".to_string()
        } else {
            "清楚、轻松、有陪伴感".to_string()
        },
        scene: use_scene.to_string(),
        age_group: age_group.to_string(),
        generation_source: None,
        quality_flags: Vec::new(),
    }
}

fn normalize_material_ids(materials: Vec<CreationMaterial>) -> Vec<CreationMaterial> {
    materials
        .into_iter()
        .enumerate()
        .map(|(index, mut material)| {
            if material.id.trim().is_empty() {
                material.id = format!("mat_{}", index + 1);
            }
            material
        })
        .collect()
}

fn extract_materials(quick_idea: &str) -> Vec<CreationMaterial> {
    let mut labels = Vec::new();
    for token in [
        "乐乐",
        "红色小汽车",
        "星星班",
        "老师",
        "妈妈",
        "爸爸",
        "午睡室",
    ] {
        if quick_idea.contains(token) {
            labels.push(token);
        }
    }
    if labels.is_empty() {
        labels.push("主角");
    }
    labels
        .into_iter()
        .enumerate()
        .map(|(index, label)| CreationMaterial {
            id: format!("mat_{}", index + 1),
            label: label.to_string(),
            material_type: if matches!(label, "乐乐" | "老师" | "妈妈" | "爸爸" | "主角")
            {
                "character"
            } else if label.contains("班") || label.contains("室") {
                "place"
            } else {
                "object"
            }
            .to_string(),
            source: "ai_extracted".to_string(),
            confidence: Some(0.86),
            locked: true,
        })
        .collect()
}

fn build_directions(
    session: &StorybookCreationSession,
    count: u32,
    reason: Option<String>,
) -> Vec<StoryDirection> {
    let material_ids = session
        .materials
        .iter()
        .filter(|item| item.locked)
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let subject = material_subject(&session.materials);
    let templates = [
        ("小小练习", "gentle", "适合情绪安抚和慢慢尝试"),
        ("一次轻松任务", "playful", "适合更活泼的亲子或课堂共读"),
        ("被理解的改变", "warm", "适合强调陪伴、表达和复盘"),
        ("今天的新约定", "clear", "适合把故事落到一个可执行小行动"),
        ("勇敢试试看", "encouraging", "适合给孩子更多主动感"),
    ];
    templates
        .iter()
        .take(count as usize)
        .enumerate()
        .map(|(index, (suffix, tone, fit))| StoryDirection {
            id: format!("dir_{}", index + 1),
            title: format!("{}的{}", subject, suffix),
            summary: format!(
                "围绕{}，让孩子在被理解中完成一次具体、温柔的成长尝试。",
                compact_idea(&session.quick_idea)
            ),
            fit_reason: (*fit).to_string(),
            personal_hook: format!(
                "把{}放在故事转折点，让用户补充的素材真正影响情节。",
                subject
            ),
            material_ids: material_ids.clone(),
            tone: (*tone).to_string(),
            generation_source: Some("fallback".to_string()),
            quality_flags: reason.iter().cloned().collect(),
        })
        .collect()
}

fn build_outline(
    session: &StorybookCreationSession,
    page_count: u32,
    reason: Option<String>,
) -> Result<CreationOutline, ApiError> {
    let direction = selected_direction(session)?;
    let subject = material_subject(&session.materials);
    let mut pages = Vec::new();
    for page_number in 1..=page_count {
        let summary = match page_number {
            1 => format!(
                "{}出现在熟悉的场景里，故事从一个真实的小瞬间开始。",
                subject
            ),
            2 => format!("{}遇到一点舍不得或不确定，大人先接住情绪。", subject),
            3 => format!(
                "故事加入{}，让问题变成可以尝试的小任务。",
                direction.personal_hook
            ),
            n if n == page_count => {
                format!("{}完成一个小小改变，留下可以继续练习的温柔约定。", subject)
            }
            _ => format!("{}在陪伴下试一次新办法，素材继续参与情节。", subject),
        };
        pages.push(CreationOutlinePage {
            page_number,
            summary,
            material_ids: direction.material_ids.clone(),
        });
    }
    Ok(CreationOutline {
        summary: direction.summary.clone(),
        pages,
        review_points: vec![
            format!("是否保留{}", subject),
            "是否避免说教".to_string(),
            format!("是否适合{}孩子共读", session.age_group),
        ],
        generation_source: Some("fallback".to_string()),
        quality_flags: reason.into_iter().collect(),
    })
}

fn selected_direction(session: &StorybookCreationSession) -> Result<StoryDirection, ApiError> {
    let selected_id = session
        .selected_direction_id
        .as_deref()
        .ok_or_else(|| ApiError::state_conflict("还没有选择故事方向"))?;
    session
        .directions
        .iter()
        .find(|item| item.id == selected_id)
        .cloned()
        .ok_or_else(|| ApiError::state_conflict("已选故事方向已失效，请重新选择"))
}

fn validate_outline_pages(pages: &[CreationOutlinePage]) -> Result<(), ApiError> {
    if pages.is_empty() {
        return Err(ApiError::validation("pages", "大纲至少需要 1 页"));
    }
    let mut page_numbers = pages
        .iter()
        .map(|page| page.page_number)
        .collect::<Vec<_>>();
    page_numbers.sort_unstable();
    for (index, page_number) in page_numbers.iter().enumerate() {
        let expected = (index + 1) as u32;
        if *page_number != expected {
            return Err(ApiError::validation(
                "pages",
                "大纲页码必须从 1 开始连续且不能重复",
            ));
        }
    }
    for page in pages {
        if page.summary.trim().is_empty() {
            return Err(ApiError::validation("summary", "页面摘要不能为空"));
        }
    }
    Ok(())
}

fn validate_session_list_status(status: &str) -> Result<(), ApiError> {
    let allowed = [
        "active",
        "all",
        "draft",
        "understanding_ready",
        "directions_ready",
        "direction_selected",
        "outline_ready",
        "generating",
        "storybook_ready",
        "failed",
        "abandoned",
    ];
    if allowed.contains(&status) {
        Ok(())
    } else {
        Err(ApiError::validation("status", "不支持的共创会话状态筛选"))
    }
}

fn validate_material_type(value: String) -> Result<String, ApiError> {
    let value = common::required(value, "type")?;
    let allowed = [
        "character",
        "object",
        "place",
        "event",
        "theme",
        "emotion",
        "custom",
    ];
    if allowed.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(ApiError::validation("type", "不支持的素材类型"))
    }
}

fn next_material_id(materials: &[CreationMaterial]) -> String {
    let next = materials
        .iter()
        .filter_map(|item| item.id.strip_prefix("mat_"))
        .filter_map(|value| value.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    format!("mat_{next}")
}

fn dedupe_materials(materials: Vec<CreationMaterial>) -> Vec<CreationMaterial> {
    let mut result = Vec::new();
    for material in materials {
        if !result
            .iter()
            .any(|item: &CreationMaterial| item.label == material.label)
        {
            result.push(material);
        }
    }
    result
}

fn compact_idea(value: &str) -> String {
    value.chars().take(28).collect()
}

fn infer_goal(value: &str) -> String {
    if value.contains("分享") {
        "帮助孩子理解分享和轮流".to_string()
    } else if value.contains("午睡") {
        "帮助孩子建立安静入睡的安全感".to_string()
    } else if value.contains("情绪") {
        "帮助孩子表达和理解自己的感受".to_string()
    } else {
        "把真实生活里的小问题变成适合共读的成长故事".to_string()
    }
}

fn material_subject(materials: &[CreationMaterial]) -> String {
    materials
        .iter()
        .find(|item| item.material_type == "character")
        .or_else(|| materials.first())
        .map(|item| item.label.clone())
        .unwrap_or_else(|| "主角".to_string())
}

fn generation_response(
    session: &StorybookCreationSession,
    include_images: bool,
) -> CreationStorybookGenerationResponse {
    CreationStorybookGenerationResponse {
        status: session.status.clone(),
        storybook_id: session.storybook_id,
        job_id: session.last_job_id,
        generation_summary: session.generation_summary.clone(),
        steps: generation_steps_from_summary(&session.generation_summary, include_images),
        next_action: if session.status == "storybook_ready" {
            "open_review_workspace"
        } else {
            "poll_generation_job"
        }
        .to_string(),
    }
}

fn generation_steps_from_summary(
    summary: &CreationGenerationSummary,
    include_images: bool,
) -> Vec<CreationGenerationStep> {
    let text_status = match summary.text_generation_status.as_str() {
        "succeeded" => "succeeded",
        "failed" => "failed",
        "generating" => "queued",
        _ => "queued",
    };
    let image_status = if include_images {
        match summary.image_generation_status.as_str() {
            "queued" | "generating" => "queued",
            "partial_failed" => "partial_failed",
            "failed" => "failed",
            "succeeded" => "succeeded",
            "skipped" => "skipped",
            _ => "pending",
        }
    } else {
        "skipped"
    };
    let mut steps = vec![
        step("story_text", "生成故事文本", text_status),
        step("roles", "整理角色设定", text_status),
        step("pages", "生成分页内容", text_status),
    ];
    steps.push(step("images", "生成封面和插图", image_status));
    steps
}

fn empty_generation_summary() -> CreationGenerationSummary {
    CreationGenerationSummary {
        text_generation_status: "not_started".to_string(),
        image_generation_status: "not_started".to_string(),
        quality_notice: None,
        recoverable_actions: Vec::new(),
    }
}

fn generation_steps(include_images: bool, status: &str) -> Vec<CreationGenerationStep> {
    let mut steps = vec![
        step("story_text", "生成故事文本", status),
        step("roles", "整理角色设定", status),
        step("pages", "生成分页内容", status),
    ];
    steps.push(step(
        "images",
        "生成封面和插图",
        if include_images { status } else { "skipped" },
    ));
    steps
}

fn step(key: &str, label: &str, status: &str) -> CreationGenerationStep {
    CreationGenerationStep {
        key: key.to_string(),
        label: label.to_string(),
        status: status.to_string(),
    }
}
