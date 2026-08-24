use axum::http::HeaderMap;
use loco_rs::app::AppContext;
use sea_orm::TransactionTrait;
use serde_json::json;
use uuid::Uuid;

use crate::{
    domains::common,
    error::ApiError,
    models::{
        CreationMaterial, StorybookAssetReferenceDeleteResponse, StorybookAssetReferenceResponse,
        StorybookAssetUploadPolicy, StorybookVisualReferenceResponse,
        UpdateStorybookAssetReferenceRequest, WorkspaceRole,
    },
    repositories::storybook_creation_assets::{
        CreateAssetReferenceInput, MAX_CREATION_ASSET_REFERENCES, UpdateAssetReferenceInput,
    },
};

pub const ACCEPTED_STORYBOOK_ASSET_CONTENT_TYPES: &[&str] =
    &["image/jpeg", "image/png", "image/webp"];

pub async fn upload_policy(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
) -> Result<StorybookAssetUploadPolicy, ApiError> {
    ensure_asset_editor(ctx, headers, workspace_id, session_id).await?;
    let remaining_slots = crate::repositories::storybook_creation_assets::remaining_slots(
        &ctx.db,
        workspace_id,
        session_id,
    )
    .await
    .map_err(common::db_error)?;
    Ok(StorybookAssetUploadPolicy {
        max_files: MAX_CREATION_ASSET_REFERENCES,
        remaining_slots,
        max_file_size_bytes: crate::services::storage::storybook_asset_max_file_size() as u64,
        accepted_content_types: ACCEPTED_STORYBOOK_ASSET_CONTENT_TYPES
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
    })
}

pub async fn create_uploaded_asset_reference(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
    input: CreateAssetReferenceInput,
) -> Result<StorybookAssetReferenceResponse, ApiError> {
    ensure_asset_editor(ctx, headers, workspace_id, session_id).await?;
    validate_idempotency_key(input.idempotency_key.as_deref())?;
    if let Some(idempotency_key) = input
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(asset_reference) =
            crate::repositories::storybook_creation_assets::find_by_idempotency_key(
                &ctx.db,
                workspace_id,
                session_id,
                idempotency_key,
            )
            .await
            .map_err(common::db_error)?
        {
            let remaining_slots = crate::repositories::storybook_creation_assets::remaining_slots(
                &ctx.db,
                workspace_id,
                session_id,
            )
            .await
            .map_err(common::db_error)?;
            return Ok(StorybookAssetReferenceResponse {
                asset_reference,
                remaining_slots,
            });
        }
    }
    let remaining_slots = crate::repositories::storybook_creation_assets::remaining_slots(
        &ctx.db,
        workspace_id,
        session_id,
    )
    .await
    .map_err(common::db_error)?;
    if remaining_slots == 0 {
        return Err(ApiError::validation_with_code(
            "photo_limit_exceeded",
            "file",
            "本次创作最多添加 5 张真实照片",
        ));
    }
    validate_content_type(&input.content_type)?;
    let asset_reference =
        crate::repositories::storybook_creation_assets::create_asset_reference(&ctx.db, input)
            .await
            .map_err(common::db_error)?;
    let remaining_slots = crate::repositories::storybook_creation_assets::remaining_slots(
        &ctx.db,
        workspace_id,
        session_id,
    )
    .await
    .map_err(common::db_error)?;
    Ok(StorybookAssetReferenceResponse {
        asset_reference,
        remaining_slots,
    })
}

pub async fn uploaded_asset_reference_by_idempotency_key(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
    idempotency_key: &str,
) -> Result<Option<StorybookAssetReferenceResponse>, ApiError> {
    ensure_asset_editor(ctx, headers, workspace_id, session_id).await?;
    validate_idempotency_key(Some(idempotency_key))?;
    let Some(asset_reference) =
        crate::repositories::storybook_creation_assets::find_by_idempotency_key(
            &ctx.db,
            workspace_id,
            session_id,
            idempotency_key.trim(),
        )
        .await
        .map_err(common::db_error)?
    else {
        return Ok(None);
    };
    let remaining_slots = crate::repositories::storybook_creation_assets::remaining_slots(
        &ctx.db,
        workspace_id,
        session_id,
    )
    .await
    .map_err(common::db_error)?;
    Ok(Some(StorybookAssetReferenceResponse {
        asset_reference,
        remaining_slots,
    }))
}

pub async fn update_asset_reference(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
    asset_reference_id: Uuid,
    payload: UpdateStorybookAssetReferenceRequest,
) -> Result<StorybookAssetReferenceResponse, ApiError> {
    ensure_asset_editor(ctx, headers, workspace_id, session_id).await?;
    let mut session =
        crate::repositories::storybook_creation_sessions::find(&ctx.db, workspace_id, session_id)
            .await
            .map_err(common::db_error)?;
    let existing_reference = crate::repositories::storybook_creation_assets::find(
        &ctx.db,
        workspace_id,
        session_id,
        asset_reference_id,
    )
    .await
    .map_err(common::db_error)?;
    let effective_kind = payload.kind.as_deref().unwrap_or(&existing_reference.kind);
    let effective_usage = payload
        .usage
        .as_deref()
        .or(existing_reference.usage.as_deref());
    validate_kind_usage(Some(effective_kind), effective_usage)?;
    let display_name = payload
        .display_name
        .as_deref()
        .map(str::trim)
        .unwrap_or_else(|| existing_reference.display_name.trim());
    if effective_usage.is_some_and(|usage| usage != "unused") && display_name.is_empty() {
        return Err(ApiError::validation(
            "display_name",
            "先给这张照片起一个名字",
        ));
    }
    let material_id = if effective_usage.is_some_and(|usage| usage != "unused") {
        Some(ensure_asset_material(
            &mut session.materials,
            effective_kind,
            display_name,
        ))
    } else {
        None
    };
    let asset_reference = crate::repositories::storybook_creation_assets::update_reference(
        &ctx.db,
        workspace_id,
        session_id,
        asset_reference_id,
        UpdateAssetReferenceInput {
            kind: payload.kind,
            display_name: if display_name.is_empty() {
                None
            } else {
                Some(display_name.to_string())
            },
            usage: payload.usage,
            material_id,
        },
    )
    .await
    .map_err(common::db_error)?;
    invalidate_session_after_asset_change(ctx, session).await?;
    let remaining_slots = crate::repositories::storybook_creation_assets::remaining_slots(
        &ctx.db,
        workspace_id,
        session_id,
    )
    .await
    .map_err(common::db_error)?;
    Ok(StorybookAssetReferenceResponse {
        asset_reference,
        remaining_slots,
    })
}

pub async fn generate_visual_reference(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
    asset_reference_id: Uuid,
    idempotency_key: String,
) -> Result<StorybookVisualReferenceResponse, ApiError> {
    let actor_id = ensure_asset_editor(ctx, headers, workspace_id, session_id).await?;
    let idempotency_key = common::required(idempotency_key, "idempotency_key")?;
    if idempotency_key.len() < 8 {
        return Err(ApiError::validation(
            "idempotency_key",
            "幂等键至少需要 8 个字符",
        ));
    }
    if let Some(asset_reference) =
        crate::repositories::storybook_creation_assets::find_by_visual_reference_idempotency_key(
            &ctx.db,
            workspace_id,
            session_id,
            asset_reference_id,
            &idempotency_key,
        )
        .await
        .map_err(common::db_error)?
    {
        let visual_reference = asset_reference
            .visual_reference
            .ok_or_else(|| ApiError::state_conflict("视觉参考任务没有创建成功"))?;
        return Ok(StorybookVisualReferenceResponse {
            visual_reference,
            next_action: "poll_visual_reference".to_string(),
        });
    }
    if crate::repositories::storybook_creation_assets::has_visual_reference_idempotency_key(
        &ctx.db,
        workspace_id,
        asset_reference_id,
        &idempotency_key,
    )
    .await
    .map_err(common::db_error)?
    {
        return Err(ApiError::state_conflict_with_code(
            "idempotency_key_replaced",
            "这次视觉参考请求已被新的重新生成替代，请刷新后查看最新参考",
        ));
    }
    let existing_reference = crate::repositories::storybook_creation_assets::find(
        &ctx.db,
        workspace_id,
        session_id,
        asset_reference_id,
    )
    .await
    .map_err(common::db_error)?;
    if existing_reference.usage.as_deref().is_none()
        || existing_reference.usage.as_deref() == Some("unused")
        || existing_reference.status == "revoked"
    {
        return Err(ApiError::state_conflict(
            "先确认这张照片的用途，再生成同画风参考",
        ));
    }
    if !usage_requires_visual_reference(existing_reference.usage.as_deref()) {
        return Err(ApiError::state_conflict_with_code(
            "visual_reference_not_required",
            "这张照片当前用途不需要生成同画风参考",
        ));
    }
    if existing_reference.display_name.trim().is_empty() {
        return Err(ApiError::validation(
            "display_name",
            "先给这张照片起一个名字",
        ));
    }
    let session =
        crate::repositories::storybook_creation_sessions::find(&ctx.db, workspace_id, session_id)
            .await
            .map_err(common::db_error)?;

    let txn = ctx.db.begin().await.map_err(common::db_error)?;
    let job = crate::repositories::generation_jobs::enqueue_job(
        &txn,
        workspace_id,
        None,
        actor_id,
        "storybook_visual_reference",
        visual_reference_generation_input(&existing_reference, &session.visual_preferences.style),
    )
    .await
    .map_err(common::db_error)?;
    let asset_reference = crate::repositories::storybook_creation_assets::create_visual_reference(
        &txn,
        workspace_id,
        session_id,
        asset_reference_id,
        Some(job.id),
        Some(idempotency_key),
    )
    .await
    .map_err(common::db_error)?;
    txn.commit().await.map_err(common::db_error)?;
    if let Err(err) =
        crate::workers::generation::enqueue_generation_job(ctx, workspace_id, job.id).await
    {
        let _ =
            crate::repositories::generation::cancel_generation_job(&ctx.db, workspace_id, job.id)
                .await;
        return Err(ApiError::state_conflict(format!(
            "视觉参考任务入队失败，已标记为可重试：{err}"
        )));
    }
    let visual_reference = asset_reference
        .visual_reference
        .ok_or_else(|| ApiError::state_conflict("视觉参考任务没有创建成功"))?;
    Ok(StorybookVisualReferenceResponse {
        visual_reference,
        next_action: "poll_visual_reference".to_string(),
    })
}

fn visual_reference_generation_input(
    asset_reference: &crate::models::StorybookAssetReference,
    visual_style: &str,
) -> serde_json::Value {
    let label = asset_reference.display_name.trim();
    let usage = asset_reference.usage.as_deref().unwrap_or("story_object");
    let kind = asset_reference.kind.as_str();
    let visual_style = visual_style.trim();
    let style_clause = if visual_style.is_empty() {
        "画风必须与本次绘本保持统一。".to_string()
    } else {
        format!("画风必须与本次绘本的“{visual_style}”保持统一。")
    };
    let usage_label = match usage {
        "main_character" => "主角",
        "story_friend" => "故事里的朋友",
        "name_only" => "只保留名字",
        "background_scene" => "故事场景",
        _ => "写进故事的关键物品",
    };
    let (reference_type, prompt, edit_instruction, generation_constraints) = match kind {
        "person" => (
            "character_reference",
            format!(
                "把这张真实人物照片转换成幼儿绘本角色设定参考。角色名称：{label}；故事用途：{usage_label}。{style_clause} 只输出单个人物主体，保留人物主体的五官、发型、肤色、服饰、体态和气质等关键可识别特征。不要沿用原照片背景，也不要沿用原照片的坐姿、躺姿、动作、镜头角度或构图。必须生成标准角色设定姿势：人物正面朝向镜头，自然站立，双臂自然下垂并与身体留出间隙，双手、十根手指、双腿和双脚完整可见；从头顶到脚底完整入画，人物居中，禁止裁切头部、手、手指、腿或脚。只生成一个正面视角的人物，不要制作多格、多角度或三视图拼图。必须移除一切背景和环境：不要出现房间、墙面、桌椅、地面、窗户、风景、其他物品或其他人物；不要保留地面投影、环境光或透视空间。优先输出透明背景 PNG；若无法输出透明背景，使用纯白色无纹理、无阴影背景。输出孤立的人物角色设定，不要生成完整绘本插图，不要做照片贴图、写实照片，也不要出现文字、logo、水印或边框。"
            ),
            "请只生成人物主体的正面全身站立角色设定：从头顶到脚底完整入画，双手、十根手指、双腿和双脚完整可见，双臂自然下垂；不要沿用原图坐姿或动作，不要裁切肢体，不要多角度拼图。完整移除原照片背景、地面、家具和其他环境元素；优先透明背景，无法透明时使用纯白无阴影背景。不要生成完整场景。".to_string(),
            vec!["isolated_person_cutout", "front_facing_full_body_standing", "complete_hands_and_feet", "transparent_or_white_background", "ignore_source_background", "no_environment", "character_design_reference"],
        ),
        "scene" => (
            "scene_reference",
            format!(
                "把这张真实地点照片转换成幼儿绘本背景/地点参考。场景名称：{label}；故事用途：{usage_label}。{style_clause} 保留空间布局、地点特征、环境色彩、光线和氛围。不要把照片中的人物作为角色主体，不要生成角色设定；画面应聚焦地点和背景元素。不要做照片贴图、写实照片，也不要出现文字、logo、水印或边框。"
            ),
            "请基于参考照片生成绘本场景参考：保留地点和环境特征，不将照片中的人物作为角色主体。".to_string(),
            vec!["background_scene_reference", "exclude_people_as_characters"],
        ),
        _ => (
            "prop_reference",
            format!(
                "把这张真实玩具、物品或宠物照片转换成幼儿绘本道具设定参考。对象名称：{label}；故事用途：{usage_label}。{style_clause} 保留主体的颜色、轮廓、材质和关键可识别特征。宠物在本阶段作为故事道具元素处理，不作为角色设定。不要生成复杂场景或完整绘本插图；请使用纯色、留白或简洁中性背景。不要做照片贴图、写实照片，也不要出现文字、logo、水印或边框。"
            ),
            "请基于参考照片生成道具设定参考：保留物品或宠物主体特征，使用简洁中性背景，不要生成复杂场景或角色设定。".to_string(),
            vec!["isolated_prop_reference", "neutral_background", "not_a_character_reference"],
        ),
    };
    json!({
        "asset_reference_id": asset_reference.id,
        "asset_id": asset_reference.asset_id,
        "kind": kind,
        "usage": usage,
        "reference_type": reference_type,
        "generation_constraints": generation_constraints,
        "visual_style": visual_style,
        "display_name": label,
        "prompt": prompt,
        "mode": "storybook_visual_reference",
        "target_type": "asset_reference",
        "target_id": asset_reference.id,
        "image_mode": "reference_image",
        "reference_images": [{
            "url": asset_reference.asset.storage_key,
            "source": "storybook_asset",
            "role_id": null,
            "label": label
        }],
        "edit_instruction": edit_instruction,
        "strength": 0.72
    })
}

pub async fn confirm_visual_reference(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
    visual_reference_id: Uuid,
) -> Result<StorybookAssetReferenceResponse, ApiError> {
    let actor_id = ensure_asset_editor(ctx, headers, workspace_id, session_id).await?;
    let asset_reference = crate::repositories::storybook_creation_assets::confirm_visual_reference(
        &ctx.db,
        workspace_id,
        session_id,
        visual_reference_id,
        actor_id,
    )
    .await
    .map_err(common::db_error)?;
    let session =
        crate::repositories::storybook_creation_sessions::find(&ctx.db, workspace_id, session_id)
            .await
            .map_err(common::db_error)?;
    invalidate_session_after_asset_change(ctx, session).await?;
    let remaining_slots = crate::repositories::storybook_creation_assets::remaining_slots(
        &ctx.db,
        workspace_id,
        session_id,
    )
    .await
    .map_err(common::db_error)?;
    Ok(StorybookAssetReferenceResponse {
        asset_reference,
        remaining_slots,
    })
}

pub async fn revoke_asset_reference(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
    asset_reference_id: Uuid,
) -> Result<StorybookAssetReferenceDeleteResponse, ApiError> {
    let actor_id = ensure_asset_editor(ctx, headers, workspace_id, session_id).await?;
    let session =
        crate::repositories::storybook_creation_sessions::find(&ctx.db, workspace_id, session_id)
            .await
            .map_err(common::db_error)?;
    let existing_reference = crate::repositories::storybook_creation_assets::find(
        &ctx.db,
        workspace_id,
        session_id,
        asset_reference_id,
    )
    .await
    .map_err(common::db_error)?;
    if let Some(job_id) = existing_reference
        .visual_reference
        .as_ref()
        .filter(|reference| matches!(reference.status.as_str(), "queued" | "generating"))
        .and_then(|reference| reference.generation_job_id)
    {
        // Stop work that has not finished before removing this creation-scoped reference.
        let _ =
            crate::repositories::generation::cancel_generation_job(&ctx.db, workspace_id, job_id)
                .await;
    }
    let asset_reference = crate::repositories::storybook_creation_assets::revoke_reference(
        &ctx.db,
        workspace_id,
        session_id,
        asset_reference_id,
        actor_id,
    )
    .await
    .map_err(common::db_error)?;
    let affected_run_ids =
        crate::repositories::storybook_customization_runs::active_run_ids_using_asset_reference(
            &ctx.db,
            workspace_id,
            asset_reference_id,
        )
        .await
        .map_err(common::db_error)?;
    for run_id in affected_run_ids {
        crate::repositories::generation::cancel_customization_run_jobs(
            &ctx.db,
            workspace_id,
            run_id,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::storybook_customization_runs::fail_active_items_for_asset_revocation(
            &ctx.db,
            workspace_id,
            run_id,
            "照片素材已被移除，请重新预览后再制作。",
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::storybook_customization_runs::finish_run(
            &ctx.db,
            workspace_id,
            run_id,
            Some("照片素材已被移除，请重新预览后再制作。"),
        )
        .await
        .map_err(common::db_error)?;
    }
    invalidate_session_after_asset_change(ctx, session).await?;
    let remaining_slots = crate::repositories::storybook_creation_assets::remaining_slots(
        &ctx.db,
        workspace_id,
        session_id,
    )
    .await
    .map_err(common::db_error)?;
    Ok(StorybookAssetReferenceDeleteResponse {
        id: asset_reference.id,
        status: asset_reference.status,
        remaining_slots,
    })
}

async fn ensure_asset_editor(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    session_id: Uuid,
) -> Result<Uuid, ApiError> {
    let workspace = common::require_editor_db(ctx, headers, workspace_id).await?;
    let actor_id = common::actor_user_id(headers)?;
    let session =
        crate::repositories::storybook_creation_sessions::find(&ctx.db, workspace_id, session_id)
            .await
            .map_err(common::db_error)?;
    if session.created_by == actor_id || matches!(workspace.role, WorkspaceRole::SchoolAdmin) {
        Ok(actor_id)
    } else {
        Err(ApiError::forbidden("只能操作自己创建的专属绘本素材"))
    }
}

fn validate_content_type(content_type: &str) -> Result<(), ApiError> {
    if ACCEPTED_STORYBOOK_ASSET_CONTENT_TYPES.contains(&content_type) {
        Ok(())
    } else {
        Err(ApiError::validation_with_code(
            "unsupported_file_type",
            "file",
            "照片只支持 JPEG、PNG 或 WebP 格式",
        ))
    }
}

fn validate_kind_usage(kind: Option<&str>, usage: Option<&str>) -> Result<(), ApiError> {
    let Some(usage) = usage else {
        return Ok(());
    };
    let kind = kind.unwrap_or("object");
    let allowed = match kind {
        "person" => ["main_character", "story_friend", "name_only", "unused"].as_slice(),
        "object" => ["story_object", "unused"].as_slice(),
        "scene" => ["background_scene", "unused"].as_slice(),
        _ => {
            return Err(ApiError::validation(
                "kind",
                "照片类型只能是 person、object 或 scene",
            ));
        }
    };
    if allowed.contains(&usage) {
        Ok(())
    } else {
        Err(ApiError::validation("usage", "照片用途和照片类型不匹配"))
    }
}

fn usage_requires_visual_reference(usage: Option<&str>) -> bool {
    matches!(
        usage,
        Some("main_character")
            | Some("story_friend")
            | Some("story_object")
            | Some("background_scene")
    )
}

fn validate_idempotency_key(idempotency_key: Option<&str>) -> Result<(), ApiError> {
    let Some(idempotency_key) = idempotency_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err(ApiError::validation("idempotency_key", "幂等键不能为空"));
    };
    if idempotency_key.len() < 8 {
        return Err(ApiError::validation(
            "idempotency_key",
            "幂等键至少需要 8 个字符",
        ));
    }
    Ok(())
}

fn ensure_asset_material(
    materials: &mut Vec<CreationMaterial>,
    kind: &str,
    display_name: &str,
) -> String {
    let material_type = match kind {
        "person" => "character",
        "scene" => "scene",
        _ => "object",
    };
    if let Some(existing) = materials
        .iter_mut()
        .find(|item| item.label == display_name && item.material_type == material_type)
    {
        existing.material_type = material_type.to_string();
        existing.source = "asset_reference".to_string();
        existing.confidence = None;
        existing.locked = true;
        return existing.id.clone();
    }
    let id = format!("m{}", materials.len() + 1);
    materials.push(CreationMaterial {
        id: id.clone(),
        label: display_name.to_string(),
        material_type: material_type.to_string(),
        source: "asset_reference".to_string(),
        confidence: None,
        locked: true,
    });
    id
}

async fn invalidate_session_after_asset_change(
    ctx: &AppContext,
    mut session: crate::models::StorybookCreationSession,
) -> Result<(), ApiError> {
    if matches!(
        session.status.as_str(),
        "generating" | "storybook_ready" | "abandoned"
    ) {
        return Ok(());
    }
    session.directions.clear();
    session.selected_direction_id = None;
    session.outline = None;
    session.requires_direction_refresh = true;
    session.requires_outline_refresh = true;
    session.status = "understanding_ready".to_string();
    crate::repositories::storybook_creation_sessions::save(&ctx.db, &session)
        .await
        .map_err(common::db_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_asset_material, usage_requires_visual_reference, validate_idempotency_key,
        validate_kind_usage, visual_reference_generation_input,
    };
    use crate::models::{CreationMaterial, StorybookAssetReference, StorybookAssetSummary};
    use chrono::Utc;
    use uuid::Uuid;

    fn asset_reference(kind: &str, usage: Option<&str>) -> StorybookAssetReference {
        StorybookAssetReference {
            id: Uuid::new_v4(),
            asset_id: Uuid::new_v4(),
            asset: StorybookAssetSummary {
                id: Uuid::new_v4(),
                storage_key: "/storybook-assets/source.png".to_string(),
                status: "ready".to_string(),
                processing_message: None,
                content_type: "image/png".to_string(),
                byte_size: 128,
                width: Some(512),
                height: Some(512),
                visibility_scope: "creation_session".to_string(),
                retention_policy: "session_scoped".to_string(),
            },
            kind: kind.to_string(),
            display_name: "爸爸".to_string(),
            usage: usage.map(str::to_string),
            status: "awaiting_reference".to_string(),
            material_id: Some("m1".to_string()),
            preview_url: None,
            visual_reference: None,
            revoked_at: None,
            revoked_by: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn visual_reference_generation_input_carries_private_asset_reference() {
        let reference = asset_reference("person", Some("main_character"));

        let input = visual_reference_generation_input(&reference, "水彩拼贴");

        assert_eq!(input["mode"], "storybook_visual_reference");
        assert_eq!(input["asset_reference_id"], reference.id.to_string());
        assert_eq!(input["target_type"], "asset_reference");
        assert_eq!(
            input["reference_images"][0]["url"],
            "/storybook-assets/source.png"
        );
        assert_eq!(input["reference_images"][0]["source"], "storybook_asset");
        assert_eq!(input["image_mode"], "reference_image");
        assert_eq!(input["reference_type"], "character_reference");
        assert!(
            input["generation_constraints"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item == "ignore_source_background"))
        );
        assert!(
            input["generation_constraints"]
                .as_array()
                .is_some_and(|items| {
                    items
                        .iter()
                        .any(|item| item == "transparent_or_white_background")
                })
        );
        assert!(
            input["prompt"]
                .as_str()
                .unwrap_or_default()
                .contains("不要沿用原照片背景")
        );
        assert!(
            input["prompt"]
                .as_str()
                .unwrap_or_default()
                .contains("透明背景 PNG")
        );
        assert!(
            input["edit_instruction"]
                .as_str()
                .unwrap_or_default()
                .contains("只生成人物主体")
        );
        assert!(
            input["prompt"]
                .as_str()
                .unwrap_or_default()
                .contains("正面朝向镜头，自然站立")
        );
        assert!(
            input["generation_constraints"]
                .as_array()
                .is_some_and(|items| {
                    items
                        .iter()
                        .any(|item| item == "front_facing_full_body_standing")
                })
        );
        assert!(
            input["prompt"]
                .as_str()
                .unwrap_or_default()
                .contains("水彩拼贴")
        );
    }

    #[test]
    fn visual_reference_generation_input_isolated_by_photo_kind() {
        let object_input = visual_reference_generation_input(
            &asset_reference("object", Some("story_object")),
            "蜡笔画",
        );
        let scene_input = visual_reference_generation_input(
            &asset_reference("scene", Some("background_scene")),
            "蜡笔画",
        );

        assert_eq!(object_input["reference_type"], "prop_reference");
        assert!(
            object_input["prompt"]
                .as_str()
                .unwrap_or_default()
                .contains("宠物在本阶段作为故事道具元素处理")
        );
        assert_eq!(scene_input["reference_type"], "scene_reference");
        assert!(
            scene_input["prompt"]
                .as_str()
                .unwrap_or_default()
                .contains("不要把照片中的人物作为角色主体")
        );
    }

    #[test]
    fn idempotency_key_is_required_and_has_minimum_length() {
        assert!(validate_idempotency_key(None).is_err());
        assert!(validate_idempotency_key(Some("short")).is_err());
        assert!(validate_idempotency_key(Some("valid-key")).is_ok());
    }

    #[test]
    fn usage_rules_match_photo_kind() {
        assert!(validate_kind_usage(Some("person"), Some("main_character")).is_ok());
        assert!(validate_kind_usage(Some("person"), Some("story_friend")).is_ok());
        assert!(validate_kind_usage(Some("person"), Some("name_only")).is_ok());
        assert!(validate_kind_usage(Some("object"), Some("story_object")).is_ok());
        assert!(validate_kind_usage(Some("scene"), Some("background_scene")).is_ok());
        assert!(validate_kind_usage(Some("object"), Some("main_character")).is_err());
        assert!(validate_kind_usage(Some("scene"), Some("story_object")).is_err());
    }

    #[test]
    fn only_visual_usages_require_visual_reference() {
        assert!(usage_requires_visual_reference(Some("main_character")));
        assert!(usage_requires_visual_reference(Some("story_friend")));
        assert!(usage_requires_visual_reference(Some("story_object")));
        assert!(usage_requires_visual_reference(Some("background_scene")));
        assert!(!usage_requires_visual_reference(Some("name_only")));
        assert!(!usage_requires_visual_reference(Some("unused")));
        assert!(!usage_requires_visual_reference(None));
    }

    #[test]
    fn asset_material_maps_scene_kind_to_scene_material() {
        let mut materials = Vec::<CreationMaterial>::new();

        let material_id = ensure_asset_material(&mut materials, "scene", "操场");

        assert_eq!(material_id, "m1");
        assert_eq!(materials[0].source, "asset_reference");
        assert_eq!(materials[0].label, "操场");
        assert_eq!(materials[0].material_type, "scene");
        assert!(materials[0].locked);
    }

    #[test]
    fn asset_material_reuses_existing_same_type_label() {
        let mut materials = vec![CreationMaterial {
            id: "m1".to_string(),
            label: "爸爸".to_string(),
            material_type: "character".to_string(),
            source: "ai_extracted".to_string(),
            confidence: Some(0.8),
            locked: true,
        }];

        let material_id = ensure_asset_material(&mut materials, "person", "爸爸");

        assert_eq!(material_id, "m1");
        assert_eq!(materials.len(), 1);
        assert_eq!(materials[0].source, "asset_reference");
        assert_eq!(materials[0].confidence, None);
        assert!(materials[0].locked);
    }
}
