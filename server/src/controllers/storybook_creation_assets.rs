use axum::{
    Json,
    body::Body,
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
    routing::{get, patch, post},
};
use loco_rs::{app::AppContext, controller::Routes};
use uuid::Uuid;

use crate::{
    application,
    error::ApiError,
    models::{
        Envelope, GenerateStorybookVisualReferenceRequest, StorybookAssetReferenceDeleteResponse,
        StorybookAssetReferenceResponse, StorybookAssetUploadPolicy,
        StorybookVisualReferenceResponse, UpdateStorybookAssetReferenceRequest,
    },
    repositories::storybook_creation_assets::CreateAssetReferenceInput,
};

pub fn routes() -> Routes {
    Routes::new()
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/asset-upload-policy",
            get(upload_policy),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/assets",
            post(upload_asset),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/asset-references/{asset_reference_id}",
            patch(update_asset_reference).delete(revoke_asset_reference),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/asset-references/{asset_reference_id}/visual-reference:generate",
            post(generate_visual_reference),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/visual-references/{visual_reference_id}/confirm",
            post(confirm_visual_reference),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/assets/{asset_id}/preview",
            get(preview_asset),
        )
}

async fn upload_policy(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Envelope<StorybookAssetUploadPolicy>>, ApiError> {
    let policy = application::storybook_creation_assets::upload_policy(
        &ctx,
        &headers,
        workspace_id,
        session_id,
    )
    .await?;
    Ok(Json(Envelope::new(policy)))
}

async fn upload_asset(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id)): Path<(Uuid, Uuid)>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Envelope<StorybookAssetReferenceResponse>>), ApiError> {
    let mut file_name = String::new();
    let mut file_bytes = Vec::new();
    let mut kind = String::new();
    let mut idempotency_key = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| ApiError::validation("file", format!("读取上传内容失败：{err}")))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                file_name = field.file_name().unwrap_or("photo").to_string();
                file_bytes = field
                    .bytes()
                    .await
                    .map_err(|err| {
                        ApiError::validation_with_code(
                            "file_too_large",
                            "file",
                            format!("照片文件过大或无法读取：{err}"),
                        )
                    })?
                    .to_vec();
            }
            "kind" => {
                kind = field.text().await.map_err(|err| {
                    ApiError::validation("kind", format!("读取照片类型失败：{err}"))
                })?;
            }
            "idempotency_key" => {
                idempotency_key = Some(field.text().await.map_err(|err| {
                    ApiError::validation("idempotency_key", format!("读取幂等键失败：{err}"))
                })?);
            }
            _ => {}
        }
    }

    if file_bytes.is_empty() {
        return Err(ApiError::validation("file", "请选择要上传的照片"));
    }
    let idempotency_key = idempotency_key
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::validation("idempotency_key", "幂等键不能为空"))?;
    if let Some(existing) =
        application::storybook_creation_assets::uploaded_asset_reference_by_idempotency_key(
            &ctx,
            &headers,
            workspace_id,
            session_id,
            &idempotency_key,
        )
        .await?
    {
        return Ok((StatusCode::OK, Json(Envelope::new(existing))));
    }
    let max_file_size = crate::services::storage::storybook_asset_max_file_size();
    if max_file_size > 0 && file_bytes.len() > max_file_size {
        return Err(ApiError::validation_with_code(
            "file_too_large",
            "file",
            format!("照片文件过大，最多支持 {} bytes", max_file_size),
        ));
    }
    let content_type = detect_image_content_type(&file_bytes)?;
    let kind = normalize_kind(kind)?;
    let extension = extension_for_content_type(&content_type)?;
    let storage_file_name = format!("{}.{}", Uuid::new_v4(), extension);
    let storage_key =
        crate::services::storage::save_storybook_asset(&storage_file_name, &file_bytes)
            .map_err(|err| ApiError::validation("file", err))?;
    let response = application::storybook_creation_assets::create_uploaded_asset_reference(
        &ctx,
        &headers,
        workspace_id,
        session_id,
        CreateAssetReferenceInput {
            workspace_id,
            session_id,
            uploaded_by: crate::domains::common::actor_user_id(&headers)?,
            storage_key,
            original_filename: file_name,
            content_type,
            byte_size: file_bytes.len() as i64,
            width: None,
            height: None,
            kind,
            idempotency_key: Some(idempotency_key),
        },
    )
    .await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(response))))
}

async fn update_asset_reference(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id, asset_reference_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(payload): Json<UpdateStorybookAssetReferenceRequest>,
) -> Result<Json<Envelope<StorybookAssetReferenceResponse>>, ApiError> {
    let response = application::storybook_creation_assets::update_asset_reference(
        &ctx,
        &headers,
        workspace_id,
        session_id,
        asset_reference_id,
        payload,
    )
    .await?;
    Ok(Json(Envelope::new(response)))
}

async fn generate_visual_reference(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id, asset_reference_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(payload): Json<GenerateStorybookVisualReferenceRequest>,
) -> Result<Json<Envelope<StorybookVisualReferenceResponse>>, ApiError> {
    let response = application::storybook_creation_assets::generate_visual_reference(
        &ctx,
        &headers,
        workspace_id,
        session_id,
        asset_reference_id,
        payload.idempotency_key,
    )
    .await?;
    Ok(Json(Envelope::new(response)))
}

async fn confirm_visual_reference(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id, visual_reference_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<Envelope<StorybookAssetReferenceResponse>>, ApiError> {
    let response = application::storybook_creation_assets::confirm_visual_reference(
        &ctx,
        &headers,
        workspace_id,
        session_id,
        visual_reference_id,
    )
    .await?;
    Ok(Json(Envelope::new(response)))
}

async fn revoke_asset_reference(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id, asset_reference_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<Envelope<StorybookAssetReferenceDeleteResponse>>, ApiError> {
    let response = application::storybook_creation_assets::revoke_asset_reference(
        &ctx,
        &headers,
        workspace_id,
        session_id,
        asset_reference_id,
    )
    .await?;
    Ok(Json(Envelope::new(response)))
}

async fn preview_asset(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id, asset_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Response, ApiError> {
    let session =
        crate::repositories::storybook_creation_sessions::find(&ctx.db, workspace_id, session_id)
            .await
            .map_err(crate::domains::common::db_error)?;
    let actor_id = crate::domains::common::actor_user_id(&headers)?;
    let workspace = crate::domains::common::require_editor_db(&ctx, &headers, workspace_id).await?;
    if session.created_by != actor_id
        && !matches!(workspace.role, crate::models::WorkspaceRole::SchoolAdmin)
    {
        return Err(ApiError::forbidden("无权查看这张照片"));
    }
    let asset_reference = session
        .asset_references
        .into_iter()
        .find(|reference| reference.asset_id == asset_id)
        .ok_or_else(|| ApiError::not_found("asset"))?;
    let file_name = asset_reference
        .asset
        .storage_key
        .rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::state_conflict("照片存储路径缺少文件名"))?
        .to_string();
    let bytes = crate::services::storage::read_storybook_asset(&file_name)
        .map_err(|err| ApiError::state_conflict(format!("读取照片失败：{err}")))?;
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, asset_reference.asset.content_type)
        .body(Body::from(bytes))
        .map_err(|err| ApiError::state_conflict(format!("返回照片失败：{err}")))
}

fn normalize_kind(kind: String) -> Result<String, ApiError> {
    let kind = kind.trim();
    match kind {
        "person" | "object" | "scene" => Ok(kind.to_string()),
        _ => Err(ApiError::validation(
            "kind",
            "照片类型只能是 person、object 或 scene",
        )),
    }
}

fn extension_for_content_type(content_type: &str) -> Result<&'static str, ApiError> {
    match content_type {
        "image/jpeg" => Ok("jpg"),
        "image/png" => Ok("png"),
        "image/webp" => Ok("webp"),
        _ => Err(ApiError::validation(
            "file",
            "照片只支持 JPEG、PNG 或 WebP 格式",
        )),
    }
}

fn detect_image_content_type(bytes: &[u8]) -> Result<String, ApiError> {
    const PNG_SIGNATURE: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let detected = if bytes.len() >= 3 && bytes[0] == 0xff && bytes[1] == 0xd8 && bytes[2] == 0xff {
        Some("image/jpeg")
    } else if bytes.starts_with(PNG_SIGNATURE) {
        Some("image/png")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    };
    detected.map(str::to_string).ok_or_else(|| {
        ApiError::validation_with_code(
            "unsupported_file_type",
            "file",
            "照片只支持 JPEG、PNG 或 WebP 格式",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn routes_include_storybook_creation_asset_api() {
        let registered_routes = routes();
        let uris = registered_routes
            .handlers
            .iter()
            .map(|handler| handler.uri.as_str())
            .collect::<Vec<_>>();

        assert!(uris.contains(
            &"/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/asset-upload-policy"
        ));
        assert!(uris.contains(
            &"/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/assets"
        ));
        assert!(uris.contains(
            &"/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/asset-references/{asset_reference_id}"
        ));
    }

    #[test]
    fn detect_image_content_type_uses_file_signature() {
        assert_eq!(
            detect_image_content_type(&[0xff, 0xd8, 0xff, 0x00]).expect("jpeg"),
            "image/jpeg"
        );
        assert_eq!(
            detect_image_content_type(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a])
                .expect("png"),
            "image/png"
        );
        assert_eq!(
            detect_image_content_type(b"RIFFxxxxWEBPdata").expect("webp"),
            "image/webp"
        );
        let err = detect_image_content_type(b"not an image").expect_err("invalid image");
        let body = err.into_response();
        assert_eq!(body.status(), StatusCode::BAD_REQUEST);
    }
}
