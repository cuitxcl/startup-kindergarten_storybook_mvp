use super::{ApiError, SharedState, now};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, patch, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use uuid::Uuid;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route(
            "/storybooks/{storybook_id}/exports",
            post(create_export).get(list_exports),
        )
        .route("/exports/{export_id}", get(get_export))
        .route(
            "/storybooks/{storybook_id}/share-links",
            post(create_share_link).get(list_share_links),
        )
        .route("/share-links/{share_link_id}", patch(update_share_link))
        .route("/shared-library", get(list_shared_library))
        .route(
            "/shared-library/{storybook_id}/clone",
            post(clone_shared_storybook),
        )
        .route(
            "/storybooks/{storybook_id}/submit-platform-review",
            post(submit_platform_review),
        )
}

#[derive(Clone, Debug)]
pub struct DeliveryStore {
    pub exports: BTreeMap<Uuid, StorybookExportRecord>,
    pub share_links: BTreeMap<Uuid, StorybookShareLinkRecord>,
}

impl DeliveryStore {
    pub fn demo() -> Self {
        Self {
            exports: BTreeMap::new(),
            share_links: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct StorybookExportRecord {
    pub id: Uuid,
    pub idempotency_key: Option<String>,
    #[serde(skip_serializing)]
    pub idempotency_fingerprint_json: Option<serde_json::Value>,
    pub storybook_id: Uuid,
    pub export_type: String,
    pub include_teacher_tips: bool,
    pub page_size: String,
    pub quality: String,
    pub allow_text_only: bool,
    pub status: String,
    pub asset_id: Option<Uuid>,
    pub download_url: Option<String>,
    pub failure_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StorybookShareLinkRecord {
    pub id: Uuid,
    pub idempotency_key: Option<String>,
    #[serde(skip_serializing)]
    pub idempotency_fingerprint_json: Option<serde_json::Value>,
    pub storybook_id: Uuid,
    pub share_scope: String,
    pub share_token: String,
    pub url: String,
    pub qrcode_asset_id: Option<Uuid>,
    pub anonymize_child_name: bool,
    pub anonymize_parent_info: bool,
    pub status: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateExportRequest {
    pub export_type: String,
    pub include_teacher_tips: Option<bool>,
    pub page_size: Option<String>,
    pub quality: Option<String>,
    pub allow_text_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateShareLinkRequest {
    pub share_scope: String,
    pub anonymize_child_name: Option<bool>,
    pub anonymize_parent_info: Option<bool>,
    pub expires_at: Option<DateTime<Utc>>,
    pub create_qrcode: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateShareLinkRequest {
    pub status: Option<String>,
    pub share_scope: Option<String>,
    pub anonymize_child_name: Option<bool>,
    pub anonymize_parent_info: Option<bool>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct SharedLibraryQuery {
    pub share_scope: Option<String>,
    pub content_type: Option<String>,
    pub keyword: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct CloneSharedStorybookRequest {
    pub target_child_id: Option<Uuid>,
    pub title_override: Option<String>,
    pub replace_sensitive_roles: Option<bool>,
    pub regenerate_images: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: usize,
}

#[derive(Debug, Serialize, Clone)]
pub struct SharedLibraryItem {
    pub storybook_id: Uuid,
    pub title: String,
    pub content_type: String,
    pub theme: String,
    pub teaching_goal: Option<String>,
    pub share_scope: String,
    pub page_count: usize,
    pub anonymized: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct SubmitPlatformReviewResponse {
    pub storybook_id: Uuid,
    pub share_scope: String,
    pub review_status: String,
    pub submitted_at: DateTime<Utc>,
}

async fn create_export(
    State(state): State<SharedState>,
    Path(storybook_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<CreateExportRequest>,
) -> Result<Json<StorybookExportRecord>, ApiError> {
    validate_export_type(&payload.export_type)?;
    validate_optional(
        payload.page_size.as_deref(),
        &["A4", "A5", "letter"],
        "page_size",
    )?;
    validate_optional(payload.quality.as_deref(), &["preview", "print"], "quality")?;
    let mut state = state.write().expect("state lock poisoned");
    let school_id = state.organization.current_school_id;
    let idempotency_key = idempotency_key_from_headers(&headers)?;
    let fingerprint = export_fingerprint(&payload);
    if let Some(existing) = find_idempotent_export(
        &state,
        storybook_id,
        idempotency_key.as_deref(),
        &fingerprint,
    )? {
        return Ok(Json(existing));
    }
    let storybook = state
        .storybooks
        .storybooks
        .get_mut(&storybook_id)
        .ok_or_else(|| ApiError::not_found("storybook"))?;
    if storybook.school_id != Some(school_id) {
        return Err(ApiError::forbidden("不能导出其他园所的读本"));
    }
    if storybook.status == "archived" {
        return Err(ApiError::state_conflict("已归档读本不能创建新导出"));
    }
    if storybook.story_status != "story_ready" {
        return Err(ApiError::state_conflict("故事内容就绪后才能导出"));
    }
    if storybook.illustration_status != "ready" && payload.allow_text_only != Some(true) {
        return Err(ApiError {
            status: axum::http::StatusCode::CONFLICT,
            code: "ILLUSTRATION_NOT_READY",
            message: "图片未完成，导出文字版必须显式 allow_text_only=true".to_string(),
            details: vec![],
        });
    }

    let created_at = now();
    let export = StorybookExportRecord {
        id: Uuid::new_v4(),
        idempotency_key,
        idempotency_fingerprint_json: Some(fingerprint),
        storybook_id,
        export_type: payload.export_type,
        include_teacher_tips: payload.include_teacher_tips.unwrap_or(false),
        page_size: payload.page_size.unwrap_or_else(|| "A4".to_string()),
        quality: payload.quality.unwrap_or_else(|| "print".to_string()),
        allow_text_only: payload.allow_text_only.unwrap_or(false),
        status: "queued".to_string(),
        asset_id: None,
        download_url: None,
        failure_reason: None,
        created_at,
        updated_at: created_at,
        completed_at: None,
    };
    storybook.export_status = "exporting".to_string();
    storybook.status = "exporting".to_string();
    storybook.updated_at = created_at;
    state.delivery.exports.insert(export.id, export.clone());
    Ok(Json(export))
}

async fn list_exports(
    State(state): State<SharedState>,
    Path(storybook_id): Path<Uuid>,
) -> Result<Json<ListResponse<StorybookExportRecord>>, ApiError> {
    let state = state.read().expect("state lock poisoned");
    visible_storybook(&state, storybook_id)?;
    let mut items = state
        .delivery
        .exports
        .values()
        .filter(|export| export.storybook_id == storybook_id)
        .cloned()
        .collect::<Vec<_>>();
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(Json(list_response(items)))
}

async fn get_export(
    State(state): State<SharedState>,
    Path(export_id): Path<Uuid>,
) -> Result<Json<StorybookExportRecord>, ApiError> {
    let state = state.read().expect("state lock poisoned");
    let export = state
        .delivery
        .exports
        .get(&export_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("export"))?;
    visible_storybook(&state, export.storybook_id)?;
    Ok(Json(export))
}

async fn create_share_link(
    State(state): State<SharedState>,
    Path(storybook_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<CreateShareLinkRequest>,
) -> Result<Json<StorybookShareLinkRecord>, ApiError> {
    validate_share_scope(&payload.share_scope)?;
    reject_direct_platform_public_share(&payload.share_scope)?;
    validate_share_privacy(
        &payload.share_scope,
        payload.anonymize_child_name.unwrap_or(false),
        payload.anonymize_parent_info.unwrap_or(true),
    )?;
    let mut state = state.write().expect("state lock poisoned");
    let school_id = state.organization.current_school_id;
    let idempotency_key = idempotency_key_from_headers(&headers)?;
    let fingerprint = share_link_fingerprint(&payload);
    if let Some(existing) = find_idempotent_share_link(
        &state,
        storybook_id,
        idempotency_key.as_deref(),
        &fingerprint,
    )? {
        return Ok(Json(existing));
    }
    let storybook = state
        .storybooks
        .storybooks
        .get_mut(&storybook_id)
        .ok_or_else(|| ApiError::not_found("storybook"))?;
    if storybook.school_id != Some(school_id) {
        return Err(ApiError::forbidden("不能分享其他园所的读本"));
    }
    if storybook.status == "archived" {
        return Err(ApiError::state_conflict("已归档读本不能创建分享链接"));
    }

    let created_at = now();
    let share_link_id = Uuid::new_v4();
    let token = share_link_id.simple().to_string();
    let qrcode_asset_id = payload.create_qrcode.unwrap_or(false).then(Uuid::new_v4);
    let share = StorybookShareLinkRecord {
        id: share_link_id,
        idempotency_key,
        idempotency_fingerprint_json: Some(fingerprint),
        storybook_id,
        share_scope: payload.share_scope,
        share_token: token.clone(),
        url: format!("https://kindleaf.example/share/{token}"),
        qrcode_asset_id,
        anonymize_child_name: payload.anonymize_child_name.unwrap_or(false),
        anonymize_parent_info: payload.anonymize_parent_info.unwrap_or(true),
        status: "active".to_string(),
        expires_at: payload.expires_at,
        created_at,
        updated_at: created_at,
    };
    storybook.share_status = "shared".to_string();
    storybook.share_scope = share.share_scope.clone();
    storybook.updated_at = created_at;
    state.delivery.share_links.insert(share.id, share.clone());
    Ok(Json(share))
}

async fn list_share_links(
    State(state): State<SharedState>,
    Path(storybook_id): Path<Uuid>,
) -> Result<Json<ListResponse<StorybookShareLinkRecord>>, ApiError> {
    let state = state.read().expect("state lock poisoned");
    visible_storybook(&state, storybook_id)?;
    let mut items = state
        .delivery
        .share_links
        .values()
        .filter(|share| share.storybook_id == storybook_id)
        .cloned()
        .collect::<Vec<_>>();
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(Json(list_response(items)))
}

async fn update_share_link(
    State(state): State<SharedState>,
    Path(share_link_id): Path<Uuid>,
    Json(payload): Json<UpdateShareLinkRequest>,
) -> Result<Json<StorybookShareLinkRecord>, ApiError> {
    validate_optional(payload.status.as_deref(), &["active", "disabled"], "status")?;
    if let Some(scope) = payload.share_scope.as_deref() {
        validate_share_scope(scope)?;
        reject_direct_platform_public_share(scope)?;
    }
    let mut state = state.write().expect("state lock poisoned");
    let storybook_id = state
        .delivery
        .share_links
        .get(&share_link_id)
        .map(|share| share.storybook_id)
        .ok_or_else(|| ApiError::not_found("share_link"))?;
    visible_storybook(&state, storybook_id)?;

    let share = state
        .delivery
        .share_links
        .get_mut(&share_link_id)
        .expect("share link exists");
    let next_scope = payload
        .share_scope
        .clone()
        .unwrap_or_else(|| share.share_scope.clone());
    let next_anonymize_child_name = payload
        .anonymize_child_name
        .unwrap_or(share.anonymize_child_name);
    let next_anonymize_parent_info = payload
        .anonymize_parent_info
        .unwrap_or(share.anonymize_parent_info);
    validate_share_privacy(
        &next_scope,
        next_anonymize_child_name,
        next_anonymize_parent_info,
    )?;

    share.share_scope = next_scope;
    share.anonymize_child_name = next_anonymize_child_name;
    share.anonymize_parent_info = next_anonymize_parent_info;
    if let Some(status) = payload.status {
        share.status = status;
    }
    if payload.expires_at.is_some() {
        share.expires_at = payload.expires_at;
    }
    share.updated_at = now();
    let share = share.clone();
    sync_storybook_share_state(&mut state, storybook_id);
    Ok(Json(share))
}

async fn list_shared_library(
    State(state): State<SharedState>,
    Query(query): Query<SharedLibraryQuery>,
) -> Result<Json<ListResponse<SharedLibraryItem>>, ApiError> {
    validate_optional(
        query.share_scope.as_deref(),
        &["school", "platform_public"],
        "share_scope",
    )?;
    validate_optional(
        query.content_type.as_deref(),
        &["plain_storybook", "custom_storybook"],
        "content_type",
    )?;
    let state = state.read().expect("state lock poisoned");
    let school_id = state.organization.current_school_id;
    let keyword = query
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|keyword| !keyword.is_empty());
    let mut items = state
        .storybooks
        .storybooks
        .values()
        .filter(|storybook| shared_library_visible(&state, storybook, school_id))
        .filter(|storybook| {
            query
                .share_scope
                .as_deref()
                .is_none_or(|scope| storybook.share_scope == scope)
        })
        .filter(|storybook| {
            query
                .content_type
                .as_deref()
                .is_none_or(|content_type| storybook.content_type == content_type)
        })
        .filter(|storybook| {
            keyword.is_none_or(|keyword| {
                storybook.title.contains(keyword)
                    || storybook.theme.contains(keyword)
                    || storybook
                        .teaching_goal
                        .as_deref()
                        .is_some_and(|goal| goal.contains(keyword))
            })
        })
        .map(|storybook| shared_library_item(&state, storybook))
        .collect::<Vec<_>>();
    items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(Json(paginate(items, query.page, query.page_size)))
}

async fn clone_shared_storybook(
    State(state): State<SharedState>,
    Path(storybook_id): Path<Uuid>,
    Json(payload): Json<CloneSharedStorybookRequest>,
) -> Result<Json<crate::api::storybooks::StorybookRecord>, ApiError> {
    if payload.replace_sensitive_roles != Some(true) {
        return Err(ApiError::validation(
            "replace_sensitive_roles",
            "复制共享内容必须确认替换或移除敏感角色",
        ));
    }
    let mut state = state.write().expect("state lock poisoned");
    let school_id = state.organization.current_school_id;
    let teacher_id = state.organization.current_teacher_id;
    let source = state
        .storybooks
        .storybooks
        .get(&storybook_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("storybook"))?;
    if !shared_library_visible(&state, &source, school_id) {
        return Err(ApiError::forbidden("不能复制不可见的共享内容"));
    }
    let child = match payload.target_child_id {
        Some(child_id) => {
            let child = state
                .children
                .children
                .get(&child_id)
                .cloned()
                .ok_or_else(|| ApiError::not_found("child"))?;
            if child.school_id != Some(school_id) {
                return Err(ApiError::forbidden("不能使用其他园所的儿童档案"));
            }
            Some(child)
        }
        None => None,
    };
    let created_at = now();
    let mut clone = source.clone();
    clone.id = Uuid::new_v4();
    clone.school_id = Some(school_id);
    clone.teacher_id = teacher_id;
    clone.child_id = child.as_ref().map(|child| child.id);
    clone.content_type = if child.is_some() {
        "custom_storybook".to_string()
    } else {
        source.content_type.clone()
    };
    clone.title = payload
        .title_override
        .and_then(normalize_optional_owned)
        .unwrap_or_else(|| format!("{} 改编", source.title));
    clone.source_storybook_id = Some(source.id);
    clone.derivation_type = "from_shared_library".to_string();
    clone.share_scope = "private".to_string();
    clone.share_status = "private".to_string();
    clone.export_status = "not_exported".to_string();
    clone.status = "ready".to_string();
    clone.illustration_status = if payload.regenerate_images.unwrap_or(false)
        || source.content_type == "custom_storybook"
    {
        "not_started".to_string()
    } else {
        source.illustration_status.clone()
    };
    clone.role_manifest_json = if let Some(child) = child.as_ref() {
        json!({
            "protagonist": {
                "role_key": "protagonist",
                "role_type": "child",
                "display_name": child.name,
                "child_id": child.id
            },
            "source_storybook_id": source.id,
            "sensitive_roles_replaced": true
        })
    } else {
        json!({
            "source_storybook_id": source.id,
            "sensitive_roles_replaced": true
        })
    };
    clone.created_at = created_at;
    clone.updated_at = created_at;
    clone.exported_at = None;

    let pages = state
        .storybooks
        .pages
        .get(&source.id)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|mut page| {
            page.id = Uuid::new_v4();
            page.storybook_id = clone.id;
            if payload.regenerate_images.unwrap_or(false)
                || source.content_type == "custom_storybook"
            {
                page.current_image_asset_id = None;
                page.current_image_task_id = None;
                page.illustration_status = "not_started".to_string();
            }
            page.created_at = created_at;
            page.updated_at = created_at;
            page
        })
        .collect::<Vec<_>>();
    state.storybooks.pages.insert(clone.id, pages);
    state.storybooks.storybooks.insert(clone.id, clone.clone());
    Ok(Json(clone))
}

async fn submit_platform_review(
    State(state): State<SharedState>,
    Path(storybook_id): Path<Uuid>,
) -> Result<Json<SubmitPlatformReviewResponse>, ApiError> {
    let mut state = state.write().expect("state lock poisoned");
    let school_id = state.organization.current_school_id;
    let storybook = state
        .storybooks
        .storybooks
        .get_mut(&storybook_id)
        .ok_or_else(|| ApiError::not_found("storybook"))?;
    if storybook.school_id != Some(school_id) {
        return Err(ApiError::forbidden("不能提交其他园所的读本"));
    }
    if storybook.status != "ready" {
        return Err(ApiError::state_conflict("只有 ready 读本可以提交平台审核"));
    }
    let submitted_at = now();
    storybook.share_scope = "platform_review".to_string();
    storybook.share_status = "reviewing".to_string();
    storybook.updated_at = submitted_at;
    Ok(Json(SubmitPlatformReviewResponse {
        storybook_id,
        share_scope: storybook.share_scope.clone(),
        review_status: "pending".to_string(),
        submitted_at,
    }))
}

fn visible_storybook(
    state: &crate::api::AppState,
    storybook_id: Uuid,
) -> Result<&crate::api::storybooks::StorybookRecord, ApiError> {
    let storybook = state
        .storybooks
        .storybooks
        .get(&storybook_id)
        .ok_or_else(|| ApiError::not_found("storybook"))?;
    if storybook.school_id != Some(state.organization.current_school_id) {
        return Err(ApiError::forbidden("不能访问其他园所的读本"));
    }
    Ok(storybook)
}

fn shared_library_visible(
    state: &crate::api::AppState,
    storybook: &crate::api::storybooks::StorybookRecord,
    school_id: Uuid,
) -> bool {
    let Some(share) = active_share_for_storybook(state, storybook.id) else {
        return false;
    };
    if !share_link_currently_active(share) || share.share_scope != storybook.share_scope {
        return false;
    }
    (storybook.school_id == Some(school_id) && storybook.share_scope == "school")
        || storybook.share_scope == "platform_public"
}

fn shared_library_item(
    state: &crate::api::AppState,
    storybook: &crate::api::storybooks::StorybookRecord,
) -> SharedLibraryItem {
    let page_count = state
        .storybooks
        .pages
        .get(&storybook.id)
        .map_or(0, Vec::len);
    let anonymized = active_share_for_storybook(state, storybook.id)
        .is_none_or(|share| share.anonymize_child_name && share.anonymize_parent_info);
    SharedLibraryItem {
        storybook_id: storybook.id,
        title: storybook.title.clone(),
        content_type: storybook.content_type.clone(),
        theme: storybook.theme.clone(),
        teaching_goal: storybook.teaching_goal.clone(),
        share_scope: storybook.share_scope.clone(),
        page_count,
        anonymized,
        updated_at: storybook.updated_at,
    }
}

fn active_share_for_storybook(
    state: &crate::api::AppState,
    storybook_id: Uuid,
) -> Option<&StorybookShareLinkRecord> {
    state
        .delivery
        .share_links
        .values()
        .filter(|share| share.storybook_id == storybook_id && share.status == "active")
        .max_by_key(|share| share.created_at)
}

fn share_link_currently_active(share: &StorybookShareLinkRecord) -> bool {
    share.expires_at.is_none_or(|expires_at| expires_at > now())
}

fn find_idempotent_export(
    state: &crate::api::AppState,
    storybook_id: Uuid,
    idempotency_key: Option<&str>,
    fingerprint: &serde_json::Value,
) -> Result<Option<StorybookExportRecord>, ApiError> {
    let Some(idempotency_key) = idempotency_key else {
        return Ok(None);
    };
    let existing = state.delivery.exports.values().find(|export| {
        export.storybook_id == storybook_id
            && export.idempotency_key.as_deref() == Some(idempotency_key)
    });
    if let Some(export) = existing {
        if export.idempotency_fingerprint_json.as_ref() == Some(fingerprint) {
            return Ok(Some(export.clone()));
        }
        return Err(idempotency_conflict());
    }
    Ok(None)
}

fn find_idempotent_share_link(
    state: &crate::api::AppState,
    storybook_id: Uuid,
    idempotency_key: Option<&str>,
    fingerprint: &serde_json::Value,
) -> Result<Option<StorybookShareLinkRecord>, ApiError> {
    let Some(idempotency_key) = idempotency_key else {
        return Ok(None);
    };
    let existing = state.delivery.share_links.values().find(|share| {
        share.storybook_id == storybook_id
            && share.idempotency_key.as_deref() == Some(idempotency_key)
    });
    if let Some(share) = existing {
        if share.idempotency_fingerprint_json.as_ref() == Some(fingerprint) {
            return Ok(Some(share.clone()));
        }
        return Err(idempotency_conflict());
    }
    Ok(None)
}

fn export_fingerprint(payload: &CreateExportRequest) -> serde_json::Value {
    json!({
        "export_type": payload.export_type,
        "include_teacher_tips": payload.include_teacher_tips.unwrap_or(false),
        "page_size": payload.page_size.as_deref().unwrap_or("A4"),
        "quality": payload.quality.as_deref().unwrap_or("print"),
        "allow_text_only": payload.allow_text_only.unwrap_or(false)
    })
}

fn share_link_fingerprint(payload: &CreateShareLinkRequest) -> serde_json::Value {
    json!({
        "share_scope": payload.share_scope,
        "anonymize_child_name": payload.anonymize_child_name.unwrap_or(false),
        "anonymize_parent_info": payload.anonymize_parent_info.unwrap_or(true),
        "expires_at": payload.expires_at,
        "create_qrcode": payload.create_qrcode.unwrap_or(false)
    })
}

fn idempotency_key_from_headers(headers: &HeaderMap) -> Result<Option<String>, ApiError> {
    let Some(value) = headers.get("idempotency-key") else {
        return Ok(None);
    };
    let value = value
        .to_str()
        .map_err(|_| ApiError::validation("Idempotency-Key", "幂等键必须是有效字符串"))?
        .trim();
    if value.is_empty() {
        return Err(ApiError::validation("Idempotency-Key", "幂等键不能为空"));
    }
    Ok(Some(value.to_string()))
}

fn idempotency_conflict() -> ApiError {
    ApiError {
        status: axum::http::StatusCode::CONFLICT,
        code: "IDEMPOTENCY_CONFLICT",
        message: "同一个 Idempotency-Key 的请求参数不一致".to_string(),
        details: vec![],
    }
}

fn sync_storybook_share_state(state: &mut crate::api::AppState, storybook_id: Uuid) {
    let active_share = active_share_for_storybook(state, storybook_id).cloned();
    if let Some(storybook) = state.storybooks.storybooks.get_mut(&storybook_id) {
        if let Some(share) = active_share {
            storybook.share_status = "shared".to_string();
            storybook.share_scope = share.share_scope;
        } else {
            storybook.share_status = "private".to_string();
            storybook.share_scope = "private".to_string();
        }
        storybook.updated_at = now();
    }
}

fn validate_export_type(export_type: &str) -> Result<(), ApiError> {
    if ["pdf", "flipbook", "print"].contains(&export_type) {
        Ok(())
    } else {
        Err(ApiError::validation("export_type", "导出类型枚举不合法"))
    }
}

fn validate_share_scope(share_scope: &str) -> Result<(), ApiError> {
    if ["family", "school", "platform_public"].contains(&share_scope) {
        Ok(())
    } else {
        Err(ApiError::validation("share_scope", "分享范围枚举不合法"))
    }
}

fn reject_direct_platform_public_share(share_scope: &str) -> Result<(), ApiError> {
    if share_scope == "platform_public" {
        return Err(ApiError::state_conflict(
            "平台公开共享必须先提交平台审核，不能直接创建公开链接",
        ));
    }
    Ok(())
}

fn validate_share_privacy(
    share_scope: &str,
    anonymize_child_name: bool,
    anonymize_parent_info: bool,
) -> Result<(), ApiError> {
    if share_scope != "family" && (!anonymize_child_name || !anonymize_parent_info) {
        return Err(ApiError::validation(
            "anonymize_child_name",
            "园所或平台共享必须同时脱敏儿童姓名和家长信息",
        ));
    }
    Ok(())
}

fn validate_optional(
    value: Option<&str>,
    allowed: &[&str],
    field: &'static str,
) -> Result<(), ApiError> {
    if let Some(value) = value {
        if !allowed.contains(&value) {
            return Err(ApiError::validation(field, "枚举值不合法"));
        }
    }
    Ok(())
}

fn normalize_optional_owned(value: String) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn list_response<T>(items: Vec<T>) -> ListResponse<T> {
    let total = items.len();
    ListResponse {
        items,
        page: 1,
        page_size: total as u32,
        total,
    }
}

fn paginate<T>(items: Vec<T>, page: Option<u32>, page_size: Option<u32>) -> ListResponse<T> {
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(20).clamp(1, 100);
    let total = items.len();
    let start = ((page - 1) * page_size) as usize;
    let paged = items
        .into_iter()
        .skip(start)
        .take(page_size as usize)
        .collect();
    ListResponse {
        items: paged,
        page,
        page_size,
        total,
    }
}

#[cfg(test)]
mod tests {
    use crate::api::{AppState, router};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::{Value, json};
    use std::sync::{Arc, RwLock};
    use tower::ServiceExt;

    fn test_app() -> axum::Router {
        router(Arc::new(RwLock::new(AppState::demo())))
    }

    async fn request_json(
        app: axum::Router,
        method: &str,
        uri: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    async fn request_json_with_idempotency_key(
        app: axum::Router,
        method: &str,
        uri: &str,
        body: Value,
        idempotency_key: &str,
    ) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .header("Idempotency-Key", idempotency_key)
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    async fn get_json(app: axum::Router, uri: &str) -> (StatusCode, Value) {
        let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    async fn create_storybook(app: axum::Router) -> String {
        let case_id = crate::api::demo_uuid(31);
        let (status, body) = request_json(
            app,
            "POST",
            "/api/storybooks/generate",
            json!({
                "content_type": "plain_storybook",
                "case_storybook_id": case_id,
                "title_override": "共享导出测试故事",
                "style_id": "storybook_flat_v1",
                "reading_age_group": "5-6"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["storybook"]["id"].as_str().unwrap().to_string()
    }

    async fn create_custom_storybook(app: axum::Router) -> String {
        let case_id = crate::api::demo_uuid(31);
        let child_id = crate::api::demo_uuid(10);
        let (status, body) = request_json(
            app,
            "POST",
            "/api/storybooks/generate",
            json!({
                "content_type": "custom_storybook",
                "child_id": child_id,
                "case_storybook_id": case_id,
                "title_override": "乐乐的共享故事",
                "style_id": "storybook_flat_v1",
                "reading_age_group": "5-6"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["storybook"]["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn creates_and_reads_export_job() {
        let app = test_app();
        let storybook_id = create_storybook(app.clone()).await;
        let (status, export) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybooks/{storybook_id}/exports"),
            json!({
                "export_type": "pdf",
                "include_teacher_tips": false,
                "page_size": "A4",
                "quality": "print",
                "allow_text_only": true
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{export}");
        assert_eq!(export["status"], "queued");
        assert_eq!(export["storybook_id"], storybook_id);

        let export_id = export["id"].as_str().unwrap();
        let (status, detail) = get_json(app, &format!("/api/exports/{export_id}")).await;
        assert_eq!(status, StatusCode::OK, "{detail}");
        assert_eq!(detail["export_type"], "pdf");
    }

    #[tokio::test]
    async fn export_requires_explicit_text_only_when_images_are_not_ready() {
        let app = test_app();
        let storybook_id = create_storybook(app.clone()).await;
        let (status, body) = request_json(
            app,
            "POST",
            &format!("/api/storybooks/{storybook_id}/exports"),
            json!({
                "export_type": "pdf",
                "include_teacher_tips": false,
                "page_size": "A4",
                "quality": "print"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["code"], "ILLUSTRATION_NOT_READY");
    }

    #[tokio::test]
    async fn export_creation_is_idempotent_per_key_and_payload() {
        let app = test_app();
        let storybook_id = create_storybook(app.clone()).await;
        let uri = format!("/api/storybooks/{storybook_id}/exports");
        let payload = json!({
            "export_type": "pdf",
            "include_teacher_tips": false,
            "page_size": "A4",
            "quality": "print",
            "allow_text_only": true
        });
        let (status, first) = request_json_with_idempotency_key(
            app.clone(),
            "POST",
            &uri,
            payload.clone(),
            "export-key-1",
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{first}");
        let (status, second) =
            request_json_with_idempotency_key(app.clone(), "POST", &uri, payload, "export-key-1")
                .await;
        assert_eq!(status, StatusCode::OK, "{second}");
        assert_eq!(second["id"], first["id"]);

        let (status, conflict) = request_json_with_idempotency_key(
            app,
            "POST",
            &uri,
            json!({
                "export_type": "pdf",
                "include_teacher_tips": true,
                "page_size": "A4",
                "quality": "print",
                "allow_text_only": true
            }),
            "export-key-1",
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
        assert_eq!(conflict["error"]["code"], "IDEMPOTENCY_CONFLICT");
    }

    #[tokio::test]
    async fn enforces_privacy_for_school_share_and_lists_library() {
        let app = test_app();
        let storybook_id = create_storybook(app.clone()).await;
        let (status, body) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybooks/{storybook_id}/share-links"),
            json!({
                "share_scope": "school",
                "anonymize_child_name": false,
                "anonymize_parent_info": true
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

        let (status, share) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybooks/{storybook_id}/share-links"),
            json!({
                "share_scope": "school",
                "anonymize_child_name": true,
                "anonymize_parent_info": true,
                "create_qrcode": true
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{share}");
        assert_eq!(share["status"], "active");
        assert!(share["qrcode_asset_id"].is_string());

        let (status, library) = get_json(app, "/api/shared-library?share_scope=school").await;
        assert_eq!(status, StatusCode::OK, "{library}");
        assert_eq!(library["total"], 1);
        assert_eq!(library["items"][0]["anonymized"], true);
    }

    #[tokio::test]
    async fn share_link_creation_is_idempotent_per_key_and_payload() {
        let app = test_app();
        let storybook_id = create_storybook(app.clone()).await;
        let uri = format!("/api/storybooks/{storybook_id}/share-links");
        let payload = json!({
            "share_scope": "family",
            "anonymize_child_name": false,
            "anonymize_parent_info": true
        });
        let (status, first) = request_json_with_idempotency_key(
            app.clone(),
            "POST",
            &uri,
            payload.clone(),
            "share-key-1",
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{first}");
        let (status, second) =
            request_json_with_idempotency_key(app.clone(), "POST", &uri, payload, "share-key-1")
                .await;
        assert_eq!(status, StatusCode::OK, "{second}");
        assert_eq!(second["id"], first["id"]);
        assert_eq!(second["share_token"], first["share_token"]);

        let (status, conflict) = request_json_with_idempotency_key(
            app,
            "POST",
            &uri,
            json!({
                "share_scope": "family",
                "anonymize_child_name": true,
                "anonymize_parent_info": true
            }),
            "share-key-1",
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
        assert_eq!(conflict["error"]["code"], "IDEMPOTENCY_CONFLICT");
    }

    #[tokio::test]
    async fn blocks_direct_platform_public_share_link() {
        let app = test_app();
        let storybook_id = create_storybook(app.clone()).await;
        let (status, body) = request_json(
            app,
            "POST",
            &format!("/api/storybooks/{storybook_id}/share-links"),
            json!({
                "share_scope": "platform_public",
                "anonymize_child_name": true,
                "anonymize_parent_info": true
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["code"], "STATE_CONFLICT");
    }

    #[tokio::test]
    async fn removes_disabled_share_from_library_and_clone_access() {
        let app = test_app();
        let storybook_id = create_storybook(app.clone()).await;
        let (status, share) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybooks/{storybook_id}/share-links"),
            json!({
                "share_scope": "school",
                "anonymize_child_name": true,
                "anonymize_parent_info": true
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{share}");
        let share_link_id = share["id"].as_str().unwrap();

        let (status, updated) = request_json(
            app.clone(),
            "PATCH",
            &format!("/api/share-links/{share_link_id}"),
            json!({ "status": "disabled" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{updated}");

        let (status, library) =
            get_json(app.clone(), "/api/shared-library?share_scope=school").await;
        assert_eq!(status, StatusCode::OK, "{library}");
        assert_eq!(library["total"], 0);

        let (status, body) = request_json(
            app,
            "POST",
            &format!("/api/shared-library/{storybook_id}/clone"),
            json!({
                "replace_sensitive_roles": true,
                "regenerate_images": false
            }),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }

    #[tokio::test]
    async fn clones_shared_storybook_as_private_copy() {
        let app = test_app();
        let storybook_id = create_storybook(app.clone()).await;
        let (status, _) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybooks/{storybook_id}/share-links"),
            json!({
                "share_scope": "school",
                "anonymize_child_name": true,
                "anonymize_parent_info": true
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let child_id = crate::api::demo_uuid(10).to_string();
        let (status, cloned) = request_json(
            app,
            "POST",
            &format!("/api/shared-library/{storybook_id}/clone"),
            json!({
                "target_child_id": child_id,
                "title_override": "乐乐的共享改编",
                "replace_sensitive_roles": true,
                "regenerate_images": true
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{cloned}");
        assert_eq!(cloned["share_scope"], "private");
        assert_eq!(cloned["content_type"], "custom_storybook");
        assert_eq!(cloned["illustration_status"], "not_started");
    }

    #[tokio::test]
    async fn cloning_custom_shared_storybook_requires_fresh_images() {
        let app = test_app();
        let storybook_id = create_custom_storybook(app.clone()).await;
        let (_, detail) = get_json(app.clone(), &format!("/api/storybooks/{storybook_id}")).await;
        let page_id = detail["pages"][0]["id"].as_str().unwrap();
        let (status, task) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybook-pages/{page_id}/image-tasks"),
            json!({
                "style_id": "storybook_flat_v1",
                "prompt_template_version": "page_image_v1"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{task}");
        let output_id = task["outputs"][0]["id"].as_str().unwrap();
        let (status, selected) = request_json(
            app.clone(),
            "POST",
            &format!("/api/image-outputs/{output_id}/select"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{selected}");

        let (status, _) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybooks/{storybook_id}/share-links"),
            json!({
                "share_scope": "school",
                "anonymize_child_name": true,
                "anonymize_parent_info": true
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, cloned) = request_json(
            app.clone(),
            "POST",
            &format!("/api/shared-library/{storybook_id}/clone"),
            json!({
                "replace_sensitive_roles": true,
                "regenerate_images": false
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{cloned}");
        assert_eq!(cloned["illustration_status"], "not_started");

        let cloned_id = cloned["id"].as_str().unwrap();
        let (status, cloned_detail) = get_json(app, &format!("/api/storybooks/{cloned_id}")).await;
        assert_eq!(status, StatusCode::OK, "{cloned_detail}");
        assert_eq!(
            cloned_detail["pages"][0]["current_image_asset_id"],
            Value::Null
        );
        assert_eq!(
            cloned_detail["pages"][0]["current_image_task_id"],
            Value::Null
        );
        assert_eq!(
            cloned_detail["pages"][0]["illustration_status"],
            "not_started"
        );
    }

    #[tokio::test]
    async fn submits_storybook_to_platform_review() {
        let app = test_app();
        let storybook_id = create_storybook(app.clone()).await;
        let (status, review) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybooks/{storybook_id}/submit-platform-review"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{review}");
        assert_eq!(review["share_scope"], "platform_review");
        assert_eq!(review["review_status"], "pending");

        let (status, detail) = get_json(app, &format!("/api/storybooks/{storybook_id}")).await;
        assert_eq!(status, StatusCode::OK, "{detail}");
        assert_eq!(detail["share_status"], "reviewing");
    }
}
