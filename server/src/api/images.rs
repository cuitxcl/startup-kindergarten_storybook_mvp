use super::{ApiError, SharedState, now};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use uuid::Uuid;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/assets/upload-intents", post(create_upload_intent))
        .route("/assets", post(create_asset))
        .route("/assets/{asset_id}", get(get_asset))
        .route(
            "/storybook-pages/{page_id}/image-tasks",
            post(create_page_image_task),
        )
        .route(
            "/storybooks/{storybook_id}/image-tasks",
            post(create_storybook_image_task),
        )
        .route("/image-tasks/{task_id}", get(get_image_task))
        .route(
            "/image-tasks/{task_id}/review-events",
            get(list_review_events),
        )
        .route("/image-tasks/{task_id}/retry", post(retry_image_task))
        .route(
            "/image-outputs/{output_id}/select",
            post(select_image_output),
        )
        .route(
            "/image-outputs/{output_id}/review",
            post(review_image_output),
        )
        .route("/admin/generation-costs", get(list_generation_costs))
}

#[derive(Clone, Debug)]
pub struct ImageGenerationStore {
    pub upload_intents: BTreeMap<Uuid, UploadIntentRecord>,
    pub assets: BTreeMap<Uuid, ImageAssetRecord>,
    pub tasks: BTreeMap<Uuid, ImageGenerationTaskRecord>,
    pub outputs: BTreeMap<Uuid, ImageGenerationOutputRecord>,
    pub review_events: BTreeMap<Uuid, ImageReviewEventRecord>,
    pub cost_logs: BTreeMap<Uuid, GenerationCostLogRecord>,
    pub image_provider: ImageProviderKind,
}

impl ImageGenerationStore {
    pub fn demo() -> Self {
        Self {
            upload_intents: BTreeMap::new(),
            assets: BTreeMap::new(),
            tasks: BTreeMap::new(),
            outputs: BTreeMap::new(),
            review_events: BTreeMap::new(),
            cost_logs: BTreeMap::new(),
            image_provider: ImageProviderKind::Seedream(SeedreamImageProvider::default()),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ImageProviderKind {
    Seedream(SeedreamImageProvider),
    Fake(FakeSeedreamImageProvider),
}

impl ImageProviderKind {
    fn provider_name(&self) -> &'static str {
        match self {
            Self::Seedream(provider) => provider.provider_name(),
            Self::Fake(provider) => provider.provider_name(),
        }
    }

    fn model_name(&self) -> &'static str {
        match self {
            Self::Seedream(provider) => provider.model_name(),
            Self::Fake(provider) => provider.model_name(),
        }
    }

    fn generate(&self, input: ImageGenerationInput) -> ImageGenerationProviderOutput {
        match self {
            Self::Seedream(provider) => provider.generate(input),
            Self::Fake(provider) => provider.generate(input),
        }
    }
}

pub trait ImageGenerationProvider {
    fn provider_name(&self) -> &'static str;
    fn model_name(&self) -> &'static str;
    fn generate(&self, input: ImageGenerationInput) -> ImageGenerationProviderOutput;
}

#[derive(Clone, Debug, Default)]
pub struct SeedreamImageProvider;

impl ImageGenerationProvider for SeedreamImageProvider {
    fn provider_name(&self) -> &'static str {
        "seedream"
    }

    fn model_name(&self) -> &'static str {
        "seedream-v1"
    }

    fn generate(&self, input: ImageGenerationInput) -> ImageGenerationProviderOutput {
        deterministic_seedream_output("seedream", "seedream-v1", input)
    }
}

#[derive(Clone, Debug, Default)]
pub struct FakeSeedreamImageProvider;

impl ImageGenerationProvider for FakeSeedreamImageProvider {
    fn provider_name(&self) -> &'static str {
        "fake_seedream"
    }

    fn model_name(&self) -> &'static str {
        "fake-seedream-v1"
    }

    fn generate(&self, input: ImageGenerationInput) -> ImageGenerationProviderOutput {
        deterministic_seedream_output("fake_seedream", "fake-seedream-v1", input)
    }
}

#[derive(Clone, Debug)]
pub struct ImageGenerationInput {
    pub task_type: String,
    pub storybook_id: Option<Uuid>,
    pub storybook_page_id: Option<Uuid>,
    pub style_id: String,
    pub prompt_template_version: String,
    pub scene_spec_json: Option<Value>,
}

#[derive(Clone, Debug)]
pub struct ImageGenerationProviderOutput {
    pub image_url: String,
    pub width: i32,
    pub height: i32,
    pub raw_prompt_text: String,
    pub provider_request_id: String,
    pub review_status: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct UploadIntentRecord {
    pub id: Uuid,
    pub asset_type: String,
    pub filename: String,
    pub mime_type: String,
    pub file_size: i64,
    pub checksum: Option<String>,
    pub upload_url: String,
    pub storage_key: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImageAssetRecord {
    pub id: Uuid,
    pub asset_type: String,
    pub storage_url: String,
    pub storage_key: Option<String>,
    pub mime_type: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub file_size: Option<i64>,
    pub checksum: Option<String>,
    pub review_result: Option<String>,
    pub metadata_json: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImageGenerationTaskRecord {
    pub id: Uuid,
    pub idempotency_key: Option<String>,
    pub task_type: String,
    pub parent_task_id: Option<Uuid>,
    pub retry_of_task_id: Option<Uuid>,
    pub school_id: Option<Uuid>,
    pub teacher_id: Option<Uuid>,
    pub storybook_id: Option<Uuid>,
    pub storybook_page_id: Option<Uuid>,
    pub character_profile_id: Option<Uuid>,
    pub character_profile_version: Option<i32>,
    pub reference_image_id: Option<Uuid>,
    pub style_id: String,
    pub prompt_template_version: String,
    pub scene_spec_json: Option<Value>,
    pub input_snapshot_json: Value,
    pub raw_prompt_text: Option<String>,
    pub provider_name: Option<String>,
    pub model_name: Option<String>,
    pub provider_request_id: Option<String>,
    pub status: String,
    pub failure_reason: Option<String>,
    pub retry_count: i32,
    pub max_retries: i32,
    pub queued_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImageGenerationOutputRecord {
    pub id: Uuid,
    pub task_id: Uuid,
    pub image_asset_id: Uuid,
    pub candidate_index: i32,
    pub is_selected: bool,
    pub review_status: String,
    pub quality_notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImageReviewEventRecord {
    pub id: Uuid,
    pub task_id: Uuid,
    pub output_id: Option<Uuid>,
    pub reviewer_teacher_id: Option<Uuid>,
    pub review_action: String,
    pub reason_code: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GenerationCostLogRecord {
    pub id: Uuid,
    pub task_id: Uuid,
    pub school_id: Option<Uuid>,
    pub teacher_id: Option<Uuid>,
    pub storybook_id: Option<Uuid>,
    pub storybook_page_id: Option<Uuid>,
    pub provider_name: String,
    pub model_name: String,
    pub input_units: Option<Decimal>,
    pub output_units: Option<Decimal>,
    pub input_cost: Decimal,
    pub output_cost: Decimal,
    pub total_cost: Decimal,
    pub currency: String,
    pub billed_units_json: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUploadIntentRequest {
    pub asset_type: String,
    pub filename: String,
    pub mime_type: String,
    pub file_size: i64,
    pub checksum: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAssetRequest {
    pub asset_type: String,
    pub storage_url: String,
    pub storage_key: Option<String>,
    pub mime_type: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub file_size: Option<i64>,
    pub checksum: Option<String>,
    #[serde(default)]
    pub metadata_json: Value,
}

#[derive(Debug, Deserialize)]
pub struct CreatePageImageTaskRequest {
    pub style_id: String,
    pub prompt_template_version: String,
    #[serde(default)]
    pub reference_image_ids: Vec<Uuid>,
    pub regeneration_reason: Option<String>,
    pub override_scene_spec_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreateStorybookImageTaskRequest {
    pub style_id: String,
    pub prompt_template_version: String,
    #[serde(default)]
    pub page_ids: Vec<Uuid>,
    pub skip_locked_pages: Option<bool>,
    pub only_pages_without_current_image: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct RetryImageTaskRequest {
    pub retry_reason: Option<String>,
    pub override_scene_spec_json: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewImageOutputRequest {
    pub review_action: String,
    pub reason_code: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CostQuery {
    pub provider_name: Option<String>,
    pub storybook_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct ImageTaskDetailResponse {
    #[serde(flatten)]
    pub task: ImageGenerationTaskRecord,
    pub outputs: Vec<ImageOutputWithAsset>,
    pub cost: Option<GenerationCostLogRecord>,
}

#[derive(Debug, Serialize)]
pub struct ImageOutputWithAsset {
    #[serde(flatten)]
    pub output: ImageGenerationOutputRecord,
    pub image_asset: ImageAssetRecord,
}

#[derive(Debug, Serialize)]
pub struct StorybookImageTaskResponse {
    pub task_id: Uuid,
    pub task_type: String,
    pub status: String,
    pub page_task_count: usize,
    pub skipped_page_ids: Vec<Uuid>,
}

async fn create_upload_intent(
    State(state): State<SharedState>,
    Json(payload): Json<CreateUploadIntentRequest>,
) -> Result<Json<UploadIntentRecord>, ApiError> {
    validate_asset_type(&payload.asset_type)?;
    validate_mime_type(&payload.mime_type)?;
    if payload.file_size <= 0 {
        return Err(ApiError::validation("file_size", "文件大小必须大于 0"));
    }
    let id = Uuid::new_v4();
    let storage_key = format!("uploads/{}/{}", payload.asset_type, id);
    let intent = UploadIntentRecord {
        id,
        asset_type: payload.asset_type,
        filename: required_trimmed(payload.filename, "filename")?,
        mime_type: payload.mime_type,
        file_size: payload.file_size,
        checksum: payload.checksum.and_then(normalize_optional_owned),
        upload_url: format!("https://upload.local/{storage_key}"),
        storage_key,
        status: "created".to_string(),
        created_at: now(),
    };
    let mut state = state.write().expect("state lock poisoned");
    state.images.upload_intents.insert(id, intent.clone());
    Ok(Json(intent))
}

async fn create_asset(
    State(state): State<SharedState>,
    Json(payload): Json<CreateAssetRequest>,
) -> Result<Json<ImageAssetRecord>, ApiError> {
    validate_asset_type(&payload.asset_type)?;
    if let Some(mime_type) = payload.mime_type.as_deref() {
        validate_mime_type(mime_type)?;
    }
    let asset = ImageAssetRecord {
        id: Uuid::new_v4(),
        asset_type: payload.asset_type,
        storage_url: required_trimmed(payload.storage_url, "storage_url")?,
        storage_key: payload.storage_key.and_then(normalize_optional_owned),
        mime_type: payload.mime_type.and_then(normalize_optional_owned),
        width: payload.width,
        height: payload.height,
        file_size: payload.file_size,
        checksum: payload.checksum.and_then(normalize_optional_owned),
        review_result: None,
        metadata_json: payload.metadata_json,
        created_at: now(),
    };
    let mut state = state.write().expect("state lock poisoned");
    state.images.assets.insert(asset.id, asset.clone());
    Ok(Json(asset))
}

async fn get_asset(
    State(state): State<SharedState>,
    Path(asset_id): Path<Uuid>,
) -> Result<Json<ImageAssetRecord>, ApiError> {
    let state = state.read().expect("state lock poisoned");
    let asset = state
        .images
        .assets
        .get(&asset_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("asset"))?;
    Ok(Json(asset))
}

async fn create_page_image_task(
    State(state): State<SharedState>,
    Path(page_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<CreatePageImageTaskRequest>,
) -> Result<Json<ImageTaskDetailResponse>, ApiError> {
    let mut state = state.write().expect("state lock poisoned");
    let idempotency_key = idempotency_key_from_headers(&headers)?;
    let (storybook_id, page) = page_snapshot(&state, page_id)?;
    validate_storybook_ready_for_images(&state, storybook_id)?;
    if page.scene_spec_status != "ready" {
        return Err(ApiError::state_conflict(
            "页面 scene_spec_status 必须为 ready",
        ));
    }
    validate_reference_images_for_page(
        &state,
        storybook_id,
        &payload.reference_image_ids,
        &payload.style_id,
    )?;
    let scene_spec_json = payload
        .override_scene_spec_json
        .clone()
        .or_else(|| page.scene_spec_json.clone());
    let fingerprint = page_image_task_fingerprint(&payload, scene_spec_json.as_ref());
    if let Some(existing) = find_idempotent_image_task(
        &state,
        storybook_id,
        Some(page_id),
        idempotency_key.as_deref(),
        &fingerprint,
    )? {
        return Ok(Json(task_detail(&state, existing)));
    }
    let task = build_task(
        &state,
        "page_image_generation",
        Some(storybook_id),
        Some(page_id),
        None,
        None,
        payload.style_id,
        payload.prompt_template_version,
        scene_spec_json,
        json!({
            "page": page,
            "reference_image_ids": payload.reference_image_ids,
            "regeneration_reason": payload.regeneration_reason,
            "idempotency_fingerprint": fingerprint
        }),
    )?;
    let task = with_idempotency_key(task, idempotency_key);
    let detail = run_seedream_task(&mut state, task)?;
    mark_page_running_or_ready(&mut state, storybook_id, page_id, &detail);
    Ok(Json(detail))
}

async fn create_storybook_image_task(
    State(state): State<SharedState>,
    Path(storybook_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<CreateStorybookImageTaskRequest>,
) -> Result<Json<StorybookImageTaskResponse>, ApiError> {
    let mut state = state.write().expect("state lock poisoned");
    let idempotency_key = idempotency_key_from_headers(&headers)?;
    validate_storybook_ready_for_images(&state, storybook_id)?;
    let page_ids = eligible_page_ids(&state, storybook_id, &payload);
    let fingerprint = storybook_image_task_fingerprint(&payload, &page_ids);
    if let Some(existing) = find_idempotent_image_task(
        &state,
        storybook_id,
        None,
        idempotency_key.as_deref(),
        &fingerprint,
    )? {
        let skipped_page_ids = existing
            .input_snapshot_json
            .get("skipped_page_ids")
            .and_then(Value::as_array)
            .map(|ids| {
                ids.iter()
                    .filter_map(Value::as_str)
                    .filter_map(|id| Uuid::parse_str(id).ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let page_task_count = existing
            .input_snapshot_json
            .get("page_task_count")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        return Ok(Json(StorybookImageTaskResponse {
            task_id: existing.id,
            task_type: existing.task_type,
            status: existing.status,
            page_task_count,
            skipped_page_ids,
        }));
    }
    let requested = if payload.page_ids.is_empty() {
        state
            .storybooks
            .pages
            .get(&storybook_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|page| page.id)
            .collect::<Vec<_>>()
    } else {
        payload.page_ids.clone()
    };
    let skipped_page_ids = requested
        .iter()
        .copied()
        .filter(|page_id| !page_ids.contains(page_id))
        .collect::<Vec<_>>();
    let parent_task = build_task(
        &state,
        "storybook_image_generation",
        Some(storybook_id),
        None,
        None,
        None,
        payload.style_id.clone(),
        payload.prompt_template_version.clone(),
        None,
        json!({
            "storybook_id": storybook_id,
            "page_ids": page_ids,
            "skipped_page_ids": skipped_page_ids,
            "page_task_count": page_ids.len(),
            "idempotency_fingerprint": fingerprint
        }),
    )?;
    let parent_task_id = parent_task.id;
    let mut parent_task = parent_task;
    parent_task.idempotency_key = idempotency_key;
    parent_task.status = "succeeded".to_string();
    parent_task.provider_name = Some(state.images.image_provider.provider_name().to_string());
    parent_task.model_name = Some(state.images.image_provider.model_name().to_string());
    parent_task.completed_at = Some(now());
    state.images.tasks.insert(parent_task_id, parent_task);
    for page_id in &page_ids {
        let (_, page) = page_snapshot(&state, *page_id)?;
        let child_task = build_task(
            &state,
            "page_image_generation",
            Some(storybook_id),
            Some(*page_id),
            Some(parent_task_id),
            None,
            payload.style_id.clone(),
            payload.prompt_template_version.clone(),
            page.scene_spec_json.clone(),
            json!({ "page": page, "parent_task_id": parent_task_id }),
        )?;
        let detail = run_seedream_task(&mut state, child_task)?;
        mark_page_running_or_ready(&mut state, storybook_id, *page_id, &detail);
    }
    update_storybook_illustration_status(&mut state, storybook_id);
    Ok(Json(StorybookImageTaskResponse {
        task_id: parent_task_id,
        task_type: "storybook_image_generation".to_string(),
        status: "succeeded".to_string(),
        page_task_count: page_ids.len(),
        skipped_page_ids,
    }))
}

async fn get_image_task(
    State(state): State<SharedState>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<ImageTaskDetailResponse>, ApiError> {
    let state = state.read().expect("state lock poisoned");
    let task = visible_task(&state, task_id)?.clone();
    Ok(Json(task_detail(&state, task)))
}

async fn list_review_events(
    State(state): State<SharedState>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<ListResponse<ImageReviewEventRecord>>, ApiError> {
    let state = state.read().expect("state lock poisoned");
    visible_task(&state, task_id)?;
    let mut items = state
        .images
        .review_events
        .values()
        .filter(|event| event.task_id == task_id)
        .cloned()
        .collect::<Vec<_>>();
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(Json(list_response(items)))
}

async fn retry_image_task(
    State(state): State<SharedState>,
    Path(task_id): Path<Uuid>,
    Json(payload): Json<RetryImageTaskRequest>,
) -> Result<Json<ImageTaskDetailResponse>, ApiError> {
    let mut state = state.write().expect("state lock poisoned");
    let original = visible_task(&state, task_id)?.clone();
    if original.retry_count >= original.max_retries {
        return Err(ApiError {
            status: axum::http::StatusCode::CONFLICT,
            code: "RETRY_LIMIT_EXCEEDED",
            message: "任务已超过最大重试次数".to_string(),
            details: vec![],
        });
    }
    let retry_task = build_task(
        &state,
        &original.task_type,
        original.storybook_id,
        original.storybook_page_id,
        original.parent_task_id,
        Some(original.id),
        original.style_id.clone(),
        original.prompt_template_version.clone(),
        payload
            .override_scene_spec_json
            .clone()
            .or_else(|| original.scene_spec_json.clone()),
        json!({
            "retry_of_task_id": original.id,
            "retry_reason": payload.retry_reason,
            "previous_input_snapshot": original.input_snapshot_json
        }),
    )?;
    let detail = run_seedream_task(&mut state, retry_task)?;
    if let (Some(storybook_id), Some(page_id)) =
        (detail.task.storybook_id, detail.task.storybook_page_id)
    {
        mark_page_running_or_ready(&mut state, storybook_id, page_id, &detail);
    }
    Ok(Json(detail))
}

async fn select_image_output(
    State(state): State<SharedState>,
    Path(output_id): Path<Uuid>,
) -> Result<Json<ImageGenerationOutputRecord>, ApiError> {
    let mut state = state.write().expect("state lock poisoned");
    let output = state
        .images
        .outputs
        .get(&output_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("image_output"))?;
    if output.review_status == "rejected" {
        return Err(ApiError::state_conflict("已拒绝候选图不能选中"));
    }
    let task = visible_task(&state, output.task_id)?.clone();
    validate_output_belongs_to_current_page_task(&state, &task)?;
    for candidate in state.images.outputs.values_mut() {
        if candidate.task_id == output.task_id {
            candidate.is_selected = false;
        }
    }
    let selected = state.images.outputs.get_mut(&output_id).unwrap();
    selected.is_selected = true;
    selected.review_status = "approved".to_string();
    let selected = selected.clone();
    if let (Some(storybook_id), Some(page_id)) = (task.storybook_id, task.storybook_page_id) {
        if let Some(pages) = state.storybooks.pages.get_mut(&storybook_id) {
            if let Some(page) = pages.iter_mut().find(|page| page.id == page_id) {
                page.current_image_asset_id = Some(selected.image_asset_id);
                page.current_image_task_id = Some(task.id);
                page.illustration_status = "ready".to_string();
                page.updated_at = now();
            }
        }
        update_storybook_illustration_status(&mut state, storybook_id);
    }
    append_review_event(
        &mut state,
        task.id,
        Some(output_id),
        "select_candidate",
        None,
        None,
    );
    Ok(Json(selected))
}

async fn review_image_output(
    State(state): State<SharedState>,
    Path(output_id): Path<Uuid>,
    Json(payload): Json<ReviewImageOutputRequest>,
) -> Result<Json<ImageGenerationOutputRecord>, ApiError> {
    validate_review_action(&payload.review_action)?;
    if payload.review_action == "reject"
        && payload
            .reason_code
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        return Err(ApiError::validation(
            "reason_code",
            "拒绝候选图必须提供 reason_code",
        ));
    }
    let mut state = state.write().expect("state lock poisoned");
    let output = state
        .images
        .outputs
        .get_mut(&output_id)
        .ok_or_else(|| ApiError::not_found("image_output"))?;
    output.review_status = match payload.review_action.as_str() {
        "approve" => "approved",
        "reject" => "rejected",
        "request_retry" => "rejected",
        "select_candidate" => "approved",
        _ => "pending",
    }
    .to_string();
    output.quality_notes = payload.notes.clone();
    let output = output.clone();
    append_review_event(
        &mut state,
        output.task_id,
        Some(output_id),
        &payload.review_action,
        payload.reason_code,
        payload.notes,
    );
    Ok(Json(output))
}

async fn list_generation_costs(
    State(state): State<SharedState>,
    Query(query): Query<CostQuery>,
) -> Result<Json<ListResponse<GenerationCostLogRecord>>, ApiError> {
    validate_cost_admin(&state)?;
    let state = state.read().expect("state lock poisoned");
    let mut items = state
        .images
        .cost_logs
        .values()
        .filter(|cost| {
            query
                .provider_name
                .as_deref()
                .is_none_or(|provider| cost.provider_name == provider)
        })
        .filter(|cost| {
            query
                .storybook_id
                .is_none_or(|id| cost.storybook_id == Some(id))
        })
        .cloned()
        .collect::<Vec<_>>();
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(Json(list_response(items)))
}

fn build_task(
    state: &crate::api::AppState,
    task_type: &str,
    storybook_id: Option<Uuid>,
    storybook_page_id: Option<Uuid>,
    parent_task_id: Option<Uuid>,
    retry_of_task_id: Option<Uuid>,
    style_id: String,
    prompt_template_version: String,
    scene_spec_json: Option<Value>,
    input_snapshot_json: Value,
) -> Result<ImageGenerationTaskRecord, ApiError> {
    validate_task_type(task_type)?;
    if task_type == "page_image_generation" && storybook_page_id.is_none() {
        return Err(ApiError::validation(
            "storybook_page_id",
            "单页图片任务必须绑定页面",
        ));
    }
    let now = now();
    Ok(ImageGenerationTaskRecord {
        id: Uuid::new_v4(),
        idempotency_key: None,
        task_type: task_type.to_string(),
        parent_task_id,
        retry_of_task_id,
        school_id: Some(state.organization.current_school_id),
        teacher_id: Some(state.organization.current_teacher_id),
        storybook_id,
        storybook_page_id,
        character_profile_id: None,
        character_profile_version: None,
        reference_image_id: None,
        style_id: required_trimmed(style_id, "style_id")?,
        prompt_template_version: required_trimmed(
            prompt_template_version,
            "prompt_template_version",
        )?,
        scene_spec_json,
        input_snapshot_json,
        raw_prompt_text: None,
        provider_name: None,
        model_name: None,
        provider_request_id: None,
        status: "queued".to_string(),
        failure_reason: None,
        retry_count: retry_of_task_id
            .and_then(|task_id| {
                state
                    .images
                    .tasks
                    .get(&task_id)
                    .map(|task| task.retry_count + 1)
            })
            .unwrap_or(0),
        max_retries: 2,
        queued_at: now,
        started_at: None,
        completed_at: None,
        created_at: now,
        updated_at: now,
    })
}

fn run_seedream_task(
    state: &mut crate::api::AppState,
    mut task: ImageGenerationTaskRecord,
) -> Result<ImageTaskDetailResponse, ApiError> {
    let provider_output = state.images.image_provider.generate(ImageGenerationInput {
        task_type: task.task_type.clone(),
        storybook_id: task.storybook_id,
        storybook_page_id: task.storybook_page_id,
        style_id: task.style_id.clone(),
        prompt_template_version: task.prompt_template_version.clone(),
        scene_spec_json: task.scene_spec_json.clone(),
    });
    let started_at = now();
    task.status = if provider_output.review_status == "pending" {
        "needs_review".to_string()
    } else {
        "succeeded".to_string()
    };
    task.provider_name = Some(state.images.image_provider.provider_name().to_string());
    task.model_name = Some(state.images.image_provider.model_name().to_string());
    task.provider_request_id = Some(provider_output.provider_request_id.clone());
    task.raw_prompt_text = Some(provider_output.raw_prompt_text.clone());
    task.started_at = Some(started_at);
    task.completed_at = Some(now());
    task.updated_at = now();

    let asset = ImageAssetRecord {
        id: Uuid::new_v4(),
        asset_type: "storybook_page_image".to_string(),
        storage_url: provider_output.image_url,
        storage_key: None,
        mime_type: Some("image/png".to_string()),
        width: Some(provider_output.width),
        height: Some(provider_output.height),
        file_size: None,
        checksum: None,
        review_result: Some(provider_output.review_status.clone()),
        metadata_json: json!({
            "provider_request_id": provider_output.provider_request_id,
            "provider": state.images.image_provider.provider_name()
        }),
        created_at: now(),
    };
    let output = ImageGenerationOutputRecord {
        id: Uuid::new_v4(),
        task_id: task.id,
        image_asset_id: asset.id,
        candidate_index: 0,
        is_selected: provider_output.review_status == "approved",
        review_status: provider_output.review_status,
        quality_notes: None,
        created_at: now(),
    };
    let cost = GenerationCostLogRecord {
        id: Uuid::new_v4(),
        task_id: task.id,
        school_id: task.school_id,
        teacher_id: task.teacher_id,
        storybook_id: task.storybook_id,
        storybook_page_id: task.storybook_page_id,
        provider_name: state.images.image_provider.provider_name().to_string(),
        model_name: state.images.image_provider.model_name().to_string(),
        input_units: Some(Decimal::new(1, 0)),
        output_units: Some(Decimal::new(1, 0)),
        input_cost: Decimal::new(2, 2),
        output_cost: Decimal::new(8, 2),
        total_cost: Decimal::new(10, 2),
        currency: "USD".to_string(),
        billed_units_json: json!({ "image_count": 1 }),
        created_at: now(),
    };
    state.images.assets.insert(asset.id, asset.clone());
    state.images.outputs.insert(output.id, output.clone());
    state.images.cost_logs.insert(cost.id, cost.clone());
    state.images.tasks.insert(task.id, task.clone());
    Ok(ImageTaskDetailResponse {
        task,
        outputs: vec![ImageOutputWithAsset {
            output,
            image_asset: asset,
        }],
        cost: Some(cost),
    })
}

fn deterministic_seedream_output(
    provider_name: &str,
    model_name: &str,
    input: ImageGenerationInput,
) -> ImageGenerationProviderOutput {
    let page_suffix = input
        .storybook_page_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "batch".to_string());
    ImageGenerationProviderOutput {
        image_url: format!("https://example.com/{provider_name}/{page_suffix}.png"),
        width: 1024,
        height: 1024,
        raw_prompt_text: format!(
            "{}:{}:{}:{}",
            provider_name, model_name, input.task_type, input.style_id
        ),
        provider_request_id: Uuid::new_v4().to_string(),
        review_status: "pending".to_string(),
    }
}

fn mark_page_running_or_ready(
    state: &mut crate::api::AppState,
    storybook_id: Uuid,
    page_id: Uuid,
    detail: &ImageTaskDetailResponse,
) {
    if let Some(pages) = state.storybooks.pages.get_mut(&storybook_id) {
        if let Some(page) = pages.iter_mut().find(|page| page.id == page_id) {
            page.current_image_task_id = Some(detail.task.id);
            page.illustration_status = if detail.task.status == "succeeded" {
                "ready".to_string()
            } else {
                "needs_review".to_string()
            };
            if let Some(output) = detail
                .outputs
                .iter()
                .find(|output| output.output.is_selected)
            {
                page.current_image_asset_id = Some(output.image_asset.id);
            }
            page.updated_at = now();
        }
    }
}

fn update_storybook_illustration_status(state: &mut crate::api::AppState, storybook_id: Uuid) {
    let statuses = state
        .storybooks
        .pages
        .get(&storybook_id)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|page| page.illustration_status)
        .collect::<Vec<_>>();
    let status = if statuses.iter().all(|status| status == "ready") {
        "ready"
    } else if statuses
        .iter()
        .any(|status| status == "needs_review" || status == "failed")
    {
        "partial_failed"
    } else if statuses
        .iter()
        .any(|status| status == "queued" || status == "running")
    {
        "running"
    } else {
        "not_started"
    };
    if let Some(storybook) = state.storybooks.storybooks.get_mut(&storybook_id) {
        storybook.illustration_status = status.to_string();
        storybook.updated_at = now();
    }
}

fn page_snapshot(
    state: &crate::api::AppState,
    page_id: Uuid,
) -> Result<(Uuid, crate::api::storybooks::StorybookPageRecord), ApiError> {
    state
        .storybooks
        .pages
        .iter()
        .find_map(|(storybook_id, pages)| {
            pages
                .iter()
                .find(|page| page.id == page_id)
                .cloned()
                .map(|page| (*storybook_id, page))
        })
        .ok_or_else(|| ApiError::not_found("storybook_page"))
}

fn validate_storybook_ready_for_images(
    state: &crate::api::AppState,
    storybook_id: Uuid,
) -> Result<(), ApiError> {
    let storybook = state
        .storybooks
        .storybooks
        .get(&storybook_id)
        .ok_or_else(|| ApiError::not_found("storybook"))?;
    if storybook.school_id != Some(state.organization.current_school_id) {
        return Err(ApiError::forbidden("不能访问其他园所的读本"));
    }
    if storybook.story_status != "story_ready" {
        return Err(ApiError::state_conflict("故事未 ready，不能生成图片"));
    }
    Ok(())
}

fn eligible_page_ids(
    state: &crate::api::AppState,
    storybook_id: Uuid,
    payload: &CreateStorybookImageTaskRequest,
) -> Vec<Uuid> {
    let requested = payload.page_ids.clone();
    state
        .storybooks
        .pages
        .get(&storybook_id)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|page| requested.is_empty() || requested.contains(&page.id))
        .filter(|page| !(payload.skip_locked_pages.unwrap_or(false) && page.is_locked))
        .filter(|page| {
            !(payload.only_pages_without_current_image.unwrap_or(false)
                && page.current_image_asset_id.is_some())
        })
        .filter(|page| page.scene_spec_status == "ready")
        .map(|page| page.id)
        .collect()
}

fn validate_reference_images_for_page(
    state: &crate::api::AppState,
    storybook_id: Uuid,
    reference_image_ids: &[Uuid],
    style_id: &str,
) -> Result<(), ApiError> {
    for reference_image_id in reference_image_ids {
        let reference = state
            .visuals
            .reference_images
            .get(reference_image_id)
            .ok_or_else(|| ApiError::not_found("reference_image"))?;
        if !reference.is_active || reference.review_status != "approved" {
            return Err(ApiError::state_conflict("只能使用已启用且审核通过的参考图"));
        }
        if reference.style_id != style_id {
            return Err(ApiError::validation(
                "reference_image_ids",
                "参考图画风必须和本次生成 style_id 一致",
            ));
        }
        if !reference_belongs_to_storybook_subject(state, storybook_id, reference) {
            return Err(ApiError::validation(
                "reference_image_ids",
                "参考图必须属于当前读本的角色或关键道具",
            ));
        }
    }
    Ok(())
}

fn reference_belongs_to_storybook_subject(
    state: &crate::api::AppState,
    storybook_id: Uuid,
    reference: &crate::api::visuals::ReferenceImageRecord,
) -> bool {
    if let Some(profile_id) = reference.character_profile_id {
        return state.visuals.storybook_roles.values().any(|role| {
            role.storybook_id == storybook_id && role.character_profile_id == Some(profile_id)
        });
    }
    if let Some(profile_id) = reference.parent_character_profile_id {
        return state.visuals.storybook_roles.values().any(|role| {
            role.storybook_id == storybook_id
                && role.parent_character_profile_id == Some(profile_id)
        });
    }
    if let Some(prop_id) = reference.prop_profile_id {
        return state
            .visuals
            .prop_profiles
            .get(&prop_id)
            .is_some_and(|prop| prop.storybook_id == Some(storybook_id));
    }
    false
}

fn find_idempotent_image_task(
    state: &crate::api::AppState,
    storybook_id: Uuid,
    storybook_page_id: Option<Uuid>,
    idempotency_key: Option<&str>,
    fingerprint: &Value,
) -> Result<Option<ImageGenerationTaskRecord>, ApiError> {
    let Some(idempotency_key) = idempotency_key else {
        return Ok(None);
    };
    let existing = state.images.tasks.values().find(|task| {
        task.storybook_id == Some(storybook_id)
            && task.storybook_page_id == storybook_page_id
            && task.idempotency_key.as_deref() == Some(idempotency_key)
    });
    if let Some(task) = existing {
        if task.input_snapshot_json.get("idempotency_fingerprint") == Some(fingerprint) {
            return Ok(Some(task.clone()));
        }
        return Err(idempotency_conflict());
    }
    Ok(None)
}

fn page_image_task_fingerprint(
    payload: &CreatePageImageTaskRequest,
    scene_spec_json: Option<&Value>,
) -> Value {
    json!({
        "style_id": payload.style_id,
        "prompt_template_version": payload.prompt_template_version,
        "reference_image_ids": payload.reference_image_ids,
        "regeneration_reason": payload.regeneration_reason,
        "scene_spec_json": scene_spec_json
    })
}

fn storybook_image_task_fingerprint(
    payload: &CreateStorybookImageTaskRequest,
    page_ids: &[Uuid],
) -> Value {
    json!({
        "style_id": payload.style_id,
        "prompt_template_version": payload.prompt_template_version,
        "requested_page_ids": payload.page_ids,
        "eligible_page_ids": page_ids,
        "skip_locked_pages": payload.skip_locked_pages.unwrap_or(false),
        "only_pages_without_current_image": payload.only_pages_without_current_image.unwrap_or(false)
    })
}

fn with_idempotency_key(
    mut task: ImageGenerationTaskRecord,
    idempotency_key: Option<String>,
) -> ImageGenerationTaskRecord {
    task.idempotency_key = idempotency_key;
    task
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

fn visible_task(
    state: &crate::api::AppState,
    task_id: Uuid,
) -> Result<&ImageGenerationTaskRecord, ApiError> {
    let task = state
        .images
        .tasks
        .get(&task_id)
        .ok_or_else(|| ApiError::not_found("image_task"))?;
    if let Some(storybook_id) = task.storybook_id {
        validate_storybook_ready_for_images(state, storybook_id)?;
    }
    Ok(task)
}

fn task_detail(
    state: &crate::api::AppState,
    task: ImageGenerationTaskRecord,
) -> ImageTaskDetailResponse {
    let outputs = state
        .images
        .outputs
        .values()
        .filter(|output| output.task_id == task.id)
        .filter_map(|output| {
            state
                .images
                .assets
                .get(&output.image_asset_id)
                .cloned()
                .map(|asset| ImageOutputWithAsset {
                    output: output.clone(),
                    image_asset: asset,
                })
        })
        .collect::<Vec<_>>();
    let cost = state
        .images
        .cost_logs
        .values()
        .find(|cost| cost.task_id == task.id)
        .cloned();
    ImageTaskDetailResponse {
        task,
        outputs,
        cost,
    }
}

fn validate_output_belongs_to_current_page_task(
    state: &crate::api::AppState,
    task: &ImageGenerationTaskRecord,
) -> Result<(), ApiError> {
    let (Some(storybook_id), Some(page_id)) = (task.storybook_id, task.storybook_page_id) else {
        return Err(ApiError::state_conflict("只有页面图片任务输出可以被选中"));
    };
    let page = state
        .storybooks
        .pages
        .get(&storybook_id)
        .and_then(|pages| pages.iter().find(|page| page.id == page_id))
        .ok_or_else(|| ApiError::not_found("storybook_page"))?;
    if page.current_image_task_id != Some(task.id) {
        return Err(ApiError::state_conflict(
            "只能选择页面最近有效图片任务的候选图",
        ));
    }
    Ok(())
}

fn append_review_event(
    state: &mut crate::api::AppState,
    task_id: Uuid,
    output_id: Option<Uuid>,
    review_action: &str,
    reason_code: Option<String>,
    notes: Option<String>,
) {
    let event = ImageReviewEventRecord {
        id: Uuid::new_v4(),
        task_id,
        output_id,
        reviewer_teacher_id: Some(state.organization.current_teacher_id),
        review_action: review_action.to_string(),
        reason_code,
        notes,
        created_at: now(),
    };
    state.images.review_events.insert(event.id, event);
}

fn validate_cost_admin(state: &SharedState) -> Result<(), ApiError> {
    let state = state.read().expect("state lock poisoned");
    let teacher = state
        .organization
        .teachers
        .get(&state.organization.current_teacher_id)
        .ok_or_else(|| ApiError::not_found("teacher"))?;
    if teacher.role == "school_admin" || teacher.role == "operator" {
        Ok(())
    } else {
        Err(ApiError::forbidden("需要园所管理员权限"))
    }
}

fn validate_asset_type(asset_type: &str) -> Result<(), ApiError> {
    if [
        "child_photo",
        "reference_image",
        "storybook_page_image",
        "export_pdf",
        "qrcode",
    ]
    .contains(&asset_type)
    {
        Ok(())
    } else {
        Err(ApiError::validation("asset_type", "资产类型不合法"))
    }
}

fn validate_mime_type(mime_type: &str) -> Result<(), ApiError> {
    if ["image/png", "image/jpeg", "application/pdf"].contains(&mime_type) {
        Ok(())
    } else {
        Err(ApiError::validation("mime_type", "文件类型不支持"))
    }
}

fn validate_task_type(task_type: &str) -> Result<(), ApiError> {
    if [
        "reference_image_generation",
        "prop_reference_generation",
        "page_image_generation",
        "storybook_image_generation",
    ]
    .contains(&task_type)
    {
        Ok(())
    } else {
        Err(ApiError::validation("task_type", "任务类型不合法"))
    }
}

fn validate_review_action(review_action: &str) -> Result<(), ApiError> {
    if ["approve", "reject", "request_retry", "select_candidate"].contains(&review_action) {
        Ok(())
    } else {
        Err(ApiError::validation("review_action", "审核动作不合法"))
    }
}

fn required_trimmed(value: String, field: &'static str) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ApiError::validation(field, "不能为空"));
    }
    Ok(value.to_string())
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

#[cfg(test)]
mod tests {
    use super::{FakeSeedreamImageProvider, ImageProviderKind};
    use crate::api::{AppState, router};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::{Value, json};
    use std::sync::{Arc, RwLock};
    use tower::ServiceExt;

    fn test_app() -> axum::Router {
        let mut state = AppState::demo();
        state.images.image_provider = ImageProviderKind::Fake(FakeSeedreamImageProvider);
        router(Arc::new(RwLock::new(state)))
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
        let request = Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    async fn create_storybook(app: axum::Router) -> Value {
        let (_, cases) = get_json(app.clone(), "/api/cases").await;
        let case_id = cases["items"][0]["id"].as_str().unwrap();
        let (status, body) = request_json(
            app.clone(),
            "POST",
            "/api/storybooks/generate",
            json!({
                "content_type": "plain_storybook",
                "case_storybook_id": case_id
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let storybook_id = body["storybook"]["id"].as_str().unwrap();
        let (_, detail) = get_json(app, &format!("/api/storybooks/{storybook_id}")).await;
        detail
    }

    async fn first_child_id(app: axum::Router) -> String {
        let (_, children) = get_json(app, "/api/children").await;
        children["items"][0]["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn creates_upload_intent_and_asset() {
        let app = test_app();
        let (status, intent) = request_json(
            app.clone(),
            "POST",
            "/api/assets/upload-intents",
            json!({
                "asset_type": "child_photo",
                "filename": "lele.png",
                "mime_type": "image/png",
                "file_size": 1024,
                "checksum": "sha256:demo"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(intent["status"], "created");

        let (status, asset) = request_json(
            app,
            "POST",
            "/api/assets",
            json!({
                "asset_type": "child_photo",
                "storage_url": "https://example.com/lele.png",
                "mime_type": "image/png",
                "width": 512,
                "height": 512
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(asset["asset_type"], "child_photo");
    }

    #[tokio::test]
    async fn creates_page_image_task_with_seedream_provider() {
        let app = test_app();
        let detail = create_storybook(app.clone()).await;
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
        assert_eq!(status, StatusCode::OK);
        assert_eq!(task["provider_name"], "fake_seedream");
        assert_eq!(task["outputs"].as_array().unwrap().len(), 1);
        assert_eq!(task["outputs"][0]["image_asset"]["width"], 1024);

        let task_id = task["id"].as_str().unwrap();
        let (_, fetched) = get_json(app, &format!("/api/image-tasks/{task_id}")).await;
        assert_eq!(fetched["outputs"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn page_image_task_creation_is_idempotent_per_key_and_payload() {
        let app = test_app();
        let detail = create_storybook(app.clone()).await;
        let page_id = detail["pages"][0]["id"].as_str().unwrap();
        let uri = format!("/api/storybook-pages/{page_id}/image-tasks");
        let payload = json!({
            "style_id": "storybook_flat_v1",
            "prompt_template_version": "page_image_v1"
        });
        let (status, first) = request_json_with_idempotency_key(
            app.clone(),
            "POST",
            &uri,
            payload.clone(),
            "page-image-key-1",
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{first}");
        let (status, second) = request_json_with_idempotency_key(
            app.clone(),
            "POST",
            &uri,
            payload,
            "page-image-key-1",
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{second}");
        assert_eq!(second["id"], first["id"]);
        assert_eq!(second["outputs"][0]["id"], first["outputs"][0]["id"]);

        let task_id = first["id"].as_str().unwrap();
        let (_, task_detail) = get_json(app.clone(), &format!("/api/image-tasks/{task_id}")).await;
        assert_eq!(task_detail["outputs"].as_array().unwrap().len(), 1);

        let (status, conflict) = request_json_with_idempotency_key(
            app,
            "POST",
            &uri,
            json!({
                "style_id": "storybook_flat_v2",
                "prompt_template_version": "page_image_v1"
            }),
            "page-image-key-1",
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
        assert_eq!(conflict["error"]["code"], "IDEMPOTENCY_CONFLICT");
    }

    #[tokio::test]
    async fn validates_page_task_reference_images_against_storybook_roles() {
        let app = test_app();
        let detail = create_storybook(app.clone()).await;
        let storybook_id = detail["id"].as_str().unwrap();
        let page_id = detail["pages"][0]["id"].as_str().unwrap();
        let child_id = first_child_id(app.clone()).await;
        let (status, profile) = request_json(
            app.clone(),
            "POST",
            &format!("/api/children/{child_id}/character-profiles"),
            json!({
                "hair": "黑色短发",
                "age_group": "5-6",
                "body_proportion": "幼儿比例",
                "visual_must_keep": ["黑色短发", "黄色卫衣", "圆脸"]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{profile}");
        let profile_id = profile["id"].as_str().unwrap();
        let (status, reference) = request_json(
            app.clone(),
            "POST",
            "/api/reference-images/generate",
            json!({
                "subject_type": "child_character",
                "character_profile_id": profile_id,
                "style_id": "storybook_flat_v1"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{reference}");
        let reference_id = reference["id"].as_str().unwrap();
        let (status, active) = request_json(
            app.clone(),
            "POST",
            &format!("/api/reference-images/{reference_id}/activate"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{active}");

        let (status, body) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybook-pages/{page_id}/image-tasks"),
            json!({
                "style_id": "storybook_flat_v1",
                "prompt_template_version": "page_image_v1",
                "reference_image_ids": [reference_id]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["details"][0]["field"], "reference_image_ids");

        let (_, roles) = get_json(
            app.clone(),
            &format!("/api/storybooks/{storybook_id}/roles"),
        )
        .await;
        let role_key = roles["items"][0]["role_key"].as_str().unwrap();
        let (status, role) = request_json(
            app.clone(),
            "PATCH",
            &format!("/api/storybooks/{storybook_id}/roles/{role_key}"),
            json!({
                "child_id": child_id,
                "character_profile_id": profile_id
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{role}");

        let (status, body) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybook-pages/{page_id}/image-tasks"),
            json!({
                "style_id": "storybook_flat_v2",
                "prompt_template_version": "page_image_v1",
                "reference_image_ids": [reference_id]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

        let (status, task) = request_json(
            app,
            "POST",
            &format!("/api/storybook-pages/{page_id}/image-tasks"),
            json!({
                "style_id": "storybook_flat_v1",
                "prompt_template_version": "page_image_v1",
                "reference_image_ids": [reference_id]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{task}");
        assert_eq!(
            task["input_snapshot_json"]["reference_image_ids"][0],
            reference_id
        );
    }

    #[tokio::test]
    async fn selects_output_and_updates_page_current_image() {
        let app = test_app();
        let detail = create_storybook(app.clone()).await;
        let storybook_id = detail["id"].as_str().unwrap();
        let page_id = detail["pages"][0]["id"].as_str().unwrap();
        let (_, task) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybook-pages/{page_id}/image-tasks"),
            json!({
                "style_id": "storybook_flat_v1",
                "prompt_template_version": "page_image_v1"
            }),
        )
        .await;
        let output_id = task["outputs"][0]["id"].as_str().unwrap();
        let (status, selected) = request_json(
            app.clone(),
            "POST",
            &format!("/api/image-outputs/{output_id}/select"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(selected["is_selected"], true);

        let (_, updated) = get_json(app, &format!("/api/storybooks/{storybook_id}")).await;
        assert_eq!(
            updated["pages"][0]["current_image_asset_id"],
            selected["image_asset_id"]
        );
    }

    #[tokio::test]
    async fn rejects_selecting_output_from_stale_page_task() {
        let app = test_app();
        let detail = create_storybook(app.clone()).await;
        let page_id = detail["pages"][0]["id"].as_str().unwrap();
        let (_, first_task) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybook-pages/{page_id}/image-tasks"),
            json!({
                "style_id": "storybook_flat_v1",
                "prompt_template_version": "page_image_v1"
            }),
        )
        .await;
        let stale_output_id = first_task["outputs"][0]["id"].as_str().unwrap();
        let (_, second_task) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybook-pages/{page_id}/image-tasks"),
            json!({
                "style_id": "storybook_flat_v1",
                "prompt_template_version": "page_image_v1",
                "regeneration_reason": "composition_mismatch"
            }),
        )
        .await;

        let (status, body) = request_json(
            app.clone(),
            "POST",
            &format!("/api/image-outputs/{stale_output_id}/select"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["code"], "STATE_CONFLICT");

        let current_output_id = second_task["outputs"][0]["id"].as_str().unwrap();
        let (status, selected) = request_json(
            app,
            "POST",
            &format!("/api/image-outputs/{current_output_id}/select"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{selected}");
        assert_eq!(selected["is_selected"], true);
    }

    #[tokio::test]
    async fn reviews_output_and_requires_reason_for_reject() {
        let app = test_app();
        let detail = create_storybook(app.clone()).await;
        let page_id = detail["pages"][0]["id"].as_str().unwrap();
        let (_, task) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybook-pages/{page_id}/image-tasks"),
            json!({
                "style_id": "storybook_flat_v1",
                "prompt_template_version": "page_image_v1"
            }),
        )
        .await;
        let output_id = task["outputs"][0]["id"].as_str().unwrap();
        let (status, body) = request_json(
            app.clone(),
            "POST",
            &format!("/api/image-outputs/{output_id}/review"),
            json!({ "review_action": "reject" }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["details"][0]["field"], "reason_code");

        let (status, reviewed) = request_json(
            app,
            "POST",
            &format!("/api/image-outputs/{output_id}/review"),
            json!({
                "review_action": "reject",
                "reason_code": "character_inconsistent",
                "notes": "发型不一致"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(reviewed["review_status"], "rejected");
    }

    #[tokio::test]
    async fn retries_task_and_lists_costs() {
        let app = test_app();
        let detail = create_storybook(app.clone()).await;
        let page_id = detail["pages"][0]["id"].as_str().unwrap();
        let (_, task) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybook-pages/{page_id}/image-tasks"),
            json!({
                "style_id": "storybook_flat_v1",
                "prompt_template_version": "page_image_v1"
            }),
        )
        .await;
        let task_id = task["id"].as_str().unwrap();
        let (status, retry) = request_json(
            app.clone(),
            "POST",
            &format!("/api/image-tasks/{task_id}/retry"),
            json!({
                "retry_reason": "composition_mismatch",
                "override_scene_spec_json": {
                    "location": "教室",
                    "action": "一起分享",
                    "composition": "中景"
                }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(retry["retry_count"], 1);
        assert_eq!(retry["retry_of_task_id"], task_id);

        let (status, costs) = get_json(
            app,
            "/api/admin/generation-costs?provider_name=fake_seedream",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(costs["total"], 2);
    }

    #[tokio::test]
    async fn creates_storybook_batch_image_task() {
        let app = test_app();
        let detail = create_storybook(app.clone()).await;
        let storybook_id = detail["id"].as_str().unwrap();
        let (status, batch) = request_json(
            app,
            "POST",
            &format!("/api/storybooks/{storybook_id}/image-tasks"),
            json!({
                "style_id": "storybook_flat_v1",
                "prompt_template_version": "page_image_v1",
                "skip_locked_pages": true
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(batch["task_type"], "storybook_image_generation");
        assert_eq!(batch["page_task_count"], 6);
    }

    #[tokio::test]
    async fn storybook_image_task_creation_is_idempotent_per_key_and_payload() {
        let app = test_app();
        let detail = create_storybook(app.clone()).await;
        let storybook_id = detail["id"].as_str().unwrap();
        let uri = format!("/api/storybooks/{storybook_id}/image-tasks");
        let payload = json!({
            "style_id": "storybook_flat_v1",
            "prompt_template_version": "page_image_v1",
            "skip_locked_pages": true,
            "only_pages_without_current_image": false
        });
        let (status, first) = request_json_with_idempotency_key(
            app.clone(),
            "POST",
            &uri,
            payload.clone(),
            "storybook-image-key-1",
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{first}");
        let (status, second) = request_json_with_idempotency_key(
            app.clone(),
            "POST",
            &uri,
            payload,
            "storybook-image-key-1",
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{second}");
        assert_eq!(second["task_id"], first["task_id"]);
        assert_eq!(second["page_task_count"], first["page_task_count"]);

        let (status, conflict) = request_json_with_idempotency_key(
            app,
            "POST",
            &uri,
            json!({
                "style_id": "storybook_flat_v2",
                "prompt_template_version": "page_image_v1",
                "skip_locked_pages": true,
                "only_pages_without_current_image": false
            }),
            "storybook-image-key-1",
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
        assert_eq!(conflict["error"]["code"], "IDEMPOTENCY_CONFLICT");
    }
}
