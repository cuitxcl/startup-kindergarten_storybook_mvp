use super::{ApiError, SharedState, now};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, patch, post, put},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use uuid::Uuid;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route(
            "/children/{child_id}/character-profiles",
            get(list_character_profiles).post(create_character_profile),
        )
        .route(
            "/character-profiles/{profile_id}",
            get(get_character_profile).patch(update_character_profile),
        )
        .route(
            "/parents/{parent_id}/character-profiles",
            post(create_parent_character_profile),
        )
        .route(
            "/storybooks/{storybook_id}/roles",
            get(list_storybook_roles),
        )
        .route(
            "/storybooks/{storybook_id}/roles/{role_key}",
            patch(update_storybook_role),
        )
        .route(
            "/storybooks/{storybook_id}/replace-roles",
            post(replace_storybook_roles),
        )
        .route(
            "/storybooks/{storybook_id}/props",
            get(list_prop_profiles).post(create_prop_profile),
        )
        .route("/prop-profiles/{prop_id}", patch(update_prop_profile))
        .route(
            "/storybook-pages/{page_id}/visual-subjects",
            put(put_page_visual_subjects),
        )
        .route("/reference-images/generate", post(generate_reference_image))
        .route(
            "/reference-images/{reference_image_id}",
            get(get_reference_image),
        )
        .route(
            "/reference-images/{reference_image_id}/activate",
            post(activate_reference_image),
        )
}

#[derive(Clone, Debug)]
pub struct VisualConsistencyStore {
    pub character_profiles: BTreeMap<Uuid, CharacterProfileRecord>,
    pub parent_character_profiles: BTreeMap<Uuid, ParentCharacterProfileRecord>,
    pub prop_profiles: BTreeMap<Uuid, PropProfileRecord>,
    pub reference_images: BTreeMap<Uuid, ReferenceImageRecord>,
    pub storybook_roles: BTreeMap<Uuid, StorybookRoleRecord>,
    pub page_visual_subjects: BTreeMap<Uuid, Vec<PageVisualSubjectRecord>>,
}

impl VisualConsistencyStore {
    pub fn demo() -> Self {
        Self {
            character_profiles: BTreeMap::new(),
            parent_character_profiles: BTreeMap::new(),
            prop_profiles: BTreeMap::new(),
            reference_images: BTreeMap::new(),
            storybook_roles: BTreeMap::new(),
            page_visual_subjects: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CharacterProfileRecord {
    pub id: Uuid,
    pub child_id: Uuid,
    pub version: i32,
    pub name: String,
    pub nickname: Option<String>,
    pub age_group: String,
    pub gender_expression: Option<String>,
    pub hair: String,
    pub skin_tone: Option<String>,
    pub face_shape: Option<String>,
    pub body_proportion: String,
    pub outfit_top: Option<String>,
    pub outfit_bottom: Option<String>,
    pub shoe: Option<String>,
    pub accessory: Option<String>,
    pub signature_colors: Vec<String>,
    pub interest_elements: Vec<String>,
    pub visual_must_keep: Vec<String>,
    pub negative_rules: Vec<String>,
    pub source_photo_id: Option<Uuid>,
    pub active_reference_image_id: Option<Uuid>,
    pub status: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ParentCharacterProfileRecord {
    pub id: Uuid,
    pub parent_id: Uuid,
    pub child_id: Option<Uuid>,
    pub version: i32,
    pub role: String,
    pub name: String,
    pub hair: Option<String>,
    pub skin_tone: Option<String>,
    pub face_shape: Option<String>,
    pub body_proportion: Option<String>,
    pub outfit_top: Option<String>,
    pub outfit_bottom: Option<String>,
    pub accessory: Option<String>,
    pub visual_must_keep: Vec<String>,
    pub negative_rules: Vec<String>,
    pub active_reference_image_id: Option<Uuid>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PropProfileRecord {
    pub id: Uuid,
    pub storybook_id: Option<Uuid>,
    pub child_id: Option<Uuid>,
    pub name: String,
    pub shape: Option<String>,
    pub primary_color: Option<String>,
    pub secondary_color: Option<String>,
    pub material_style: Option<String>,
    pub size_description: Option<String>,
    pub visual_must_keep: Vec<String>,
    pub negative_rules: Vec<String>,
    pub active_reference_image_id: Option<Uuid>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReferenceImageRecord {
    pub id: Uuid,
    pub subject_type: String,
    pub character_profile_id: Option<Uuid>,
    pub parent_character_profile_id: Option<Uuid>,
    pub prop_profile_id: Option<Uuid>,
    pub image_asset_id: Uuid,
    pub source_task_id: Option<Uuid>,
    pub style_id: String,
    pub review_status: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StorybookRoleRecord {
    pub id: Uuid,
    pub storybook_id: Uuid,
    pub role_key: String,
    pub role_type: String,
    pub display_name: String,
    pub child_id: Option<Uuid>,
    pub character_profile_id: Option<Uuid>,
    pub parent_character_profile_id: Option<Uuid>,
    pub prop_profile_id: Option<Uuid>,
    pub replacement_source_role_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PageVisualSubjectRecord {
    pub id: Uuid,
    pub storybook_page_id: Uuid,
    pub subject_type: String,
    pub storybook_role_id: Option<Uuid>,
    pub prop_profile_id: Option<Uuid>,
    pub importance: String,
    pub placement_hint: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCharacterProfileRequest {
    pub name: Option<String>,
    pub nickname: Option<String>,
    pub age_group: Option<String>,
    pub gender_expression: Option<String>,
    pub hair: String,
    pub skin_tone: Option<String>,
    pub face_shape: Option<String>,
    pub body_proportion: String,
    pub outfit_top: Option<String>,
    pub outfit_bottom: Option<String>,
    pub shoe: Option<String>,
    pub accessory: Option<String>,
    #[serde(default)]
    pub signature_colors: Vec<String>,
    #[serde(default)]
    pub interest_elements: Vec<String>,
    #[serde(default)]
    pub visual_must_keep: Vec<String>,
    #[serde(default)]
    pub negative_rules: Vec<String>,
    pub source_photo_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCharacterProfileRequest {
    pub hair: Option<String>,
    pub body_proportion: Option<String>,
    pub outfit_top: Option<String>,
    pub outfit_bottom: Option<String>,
    pub accessory: Option<String>,
    pub visual_must_keep: Option<Vec<String>>,
    pub negative_rules: Option<Vec<String>>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateParentCharacterProfileRequest {
    pub child_id: Option<Uuid>,
    pub version: Option<i32>,
    pub role: String,
    pub name: String,
    pub hair: Option<String>,
    pub skin_tone: Option<String>,
    pub face_shape: Option<String>,
    pub body_proportion: Option<String>,
    pub outfit_top: Option<String>,
    pub outfit_bottom: Option<String>,
    pub accessory: Option<String>,
    #[serde(default)]
    pub visual_must_keep: Vec<String>,
    #[serde(default)]
    pub negative_rules: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStorybookRoleRequest {
    pub display_name: Option<String>,
    pub child_id: Option<Uuid>,
    pub character_profile_id: Option<Uuid>,
    pub parent_character_profile_id: Option<Uuid>,
    pub prop_profile_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct ReplaceRolesRequest {
    #[serde(default)]
    pub replacements: Vec<RoleReplacementRequest>,
}

#[derive(Debug, Deserialize)]
pub struct RoleReplacementRequest {
    pub role_key: String,
    pub role_type: String,
    pub child_id: Option<Uuid>,
    pub character_profile_id: Option<Uuid>,
    pub parent_character_profile_id: Option<Uuid>,
    pub prop_profile_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePropProfileRequest {
    pub child_id: Option<Uuid>,
    pub name: String,
    pub shape: Option<String>,
    pub primary_color: Option<String>,
    pub secondary_color: Option<String>,
    pub material_style: Option<String>,
    pub size_description: Option<String>,
    #[serde(default)]
    pub visual_must_keep: Vec<String>,
    #[serde(default)]
    pub negative_rules: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePropProfileRequest {
    pub shape: Option<String>,
    pub primary_color: Option<String>,
    pub secondary_color: Option<String>,
    pub material_style: Option<String>,
    pub size_description: Option<String>,
    pub visual_must_keep: Option<Vec<String>>,
    pub negative_rules: Option<Vec<String>>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PutPageVisualSubjectsRequest {
    #[serde(default)]
    pub subjects: Vec<PageVisualSubjectInput>,
}

#[derive(Debug, Deserialize)]
pub struct PageVisualSubjectInput {
    pub subject_type: String,
    pub storybook_role_id: Option<Uuid>,
    pub prop_profile_id: Option<Uuid>,
    pub importance: String,
    pub placement_hint: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GenerateReferenceImageRequest {
    pub subject_type: String,
    pub character_profile_id: Option<Uuid>,
    pub parent_character_profile_id: Option<Uuid>,
    pub prop_profile_id: Option<Uuid>,
    pub style_id: String,
}

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct ReplaceRolesResponse {
    pub storybook_id: Uuid,
    pub changed_roles: Vec<String>,
    pub affected_page_ids: Vec<Uuid>,
    pub image_policy_result: String,
}

async fn list_character_profiles(
    State(state): State<SharedState>,
    Path(child_id): Path<Uuid>,
) -> Result<Json<ListResponse<CharacterProfileRecord>>, ApiError> {
    let state = state.read().expect("state lock poisoned");
    validate_child_visible(&state, child_id)?;
    let mut items = state
        .visuals
        .character_profiles
        .values()
        .filter(|profile| profile.child_id == child_id)
        .cloned()
        .collect::<Vec<_>>();
    items.sort_by(|a, b| b.version.cmp(&a.version));
    Ok(Json(list_response(items)))
}

async fn create_character_profile(
    State(state): State<SharedState>,
    Path(child_id): Path<Uuid>,
    Json(payload): Json<CreateCharacterProfileRequest>,
) -> Result<Json<CharacterProfileRecord>, ApiError> {
    let hair = required_trimmed(payload.hair, "hair")?;
    let body_proportion = required_trimmed(payload.body_proportion, "body_proportion")?;
    let age_group = required_trimmed(payload.age_group.unwrap_or_default(), "age_group")?;
    let visual_must_keep = normalize_rules(payload.visual_must_keep, "visual_must_keep")?;
    let negative_rules = normalize_rules(payload.negative_rules, "negative_rules")?;
    let mut state = state.write().expect("state lock poisoned");
    let child = validate_child_visible(&state, child_id)?.clone();
    if let Some(source_photo_id) = payload.source_photo_id {
        validate_child_photo(&state, child_id, source_photo_id)?;
    }
    let version = next_character_version(&state, child_id);
    let profile = CharacterProfileRecord {
        id: Uuid::new_v4(),
        child_id,
        version,
        name: payload
            .name
            .and_then(normalize_optional_owned)
            .unwrap_or_else(|| child.name.clone()),
        nickname: payload.nickname.and_then(normalize_optional_owned),
        age_group,
        gender_expression: payload.gender_expression.and_then(normalize_optional_owned),
        hair,
        skin_tone: payload.skin_tone.and_then(normalize_optional_owned),
        face_shape: payload.face_shape.and_then(normalize_optional_owned),
        body_proportion,
        outfit_top: payload.outfit_top.and_then(normalize_optional_owned),
        outfit_bottom: payload.outfit_bottom.and_then(normalize_optional_owned),
        shoe: payload.shoe.and_then(normalize_optional_owned),
        accessory: payload.accessory.and_then(normalize_optional_owned),
        signature_colors: normalize_rules(payload.signature_colors, "signature_colors")?,
        interest_elements: normalize_rules(payload.interest_elements, "interest_elements")?,
        visual_must_keep,
        negative_rules,
        source_photo_id: payload.source_photo_id,
        active_reference_image_id: None,
        status: "draft".to_string(),
        created_by: state.organization.current_teacher_id,
        created_at: now(),
    };
    state
        .visuals
        .character_profiles
        .insert(profile.id, profile.clone());
    Ok(Json(profile))
}

async fn get_character_profile(
    State(state): State<SharedState>,
    Path(profile_id): Path<Uuid>,
) -> Result<Json<CharacterProfileRecord>, ApiError> {
    let state = state.read().expect("state lock poisoned");
    let profile = state
        .visuals
        .character_profiles
        .get(&profile_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("character_profile"))?;
    validate_child_visible(&state, profile.child_id)?;
    Ok(Json(profile))
}

async fn update_character_profile(
    State(state): State<SharedState>,
    Path(profile_id): Path<Uuid>,
    Json(payload): Json<UpdateCharacterProfileRequest>,
) -> Result<Json<CharacterProfileRecord>, ApiError> {
    validate_optional_status(
        payload.status.as_deref(),
        &["draft", "active", "superseded"],
        "status",
    )?;
    let mut state = state.write().expect("state lock poisoned");
    let child_id = state
        .visuals
        .character_profiles
        .get(&profile_id)
        .map(|profile| profile.child_id)
        .ok_or_else(|| ApiError::not_found("character_profile"))?;
    validate_child_visible(&state, child_id)?;
    let profile = state
        .visuals
        .character_profiles
        .get_mut(&profile_id)
        .unwrap();
    if profile.status == "superseded" {
        return Err(ApiError::state_conflict("已废弃角色卡不可编辑"));
    }
    if profile.status == "active" && character_profile_visual_fields_changed(&payload) {
        return Err(ApiError::state_conflict(
            "active 角色卡的视觉特征变更必须新建版本",
        ));
    }
    if let Some(hair) = payload.hair {
        profile.hair = required_trimmed(hair, "hair")?;
    }
    if let Some(body_proportion) = payload.body_proportion {
        profile.body_proportion = required_trimmed(body_proportion, "body_proportion")?;
    }
    apply_optional_string(&mut profile.outfit_top, payload.outfit_top);
    apply_optional_string(&mut profile.outfit_bottom, payload.outfit_bottom);
    apply_optional_string(&mut profile.accessory, payload.accessory);
    if let Some(rules) = payload.visual_must_keep {
        profile.visual_must_keep = normalize_rules(rules, "visual_must_keep")?;
    }
    if let Some(rules) = payload.negative_rules {
        profile.negative_rules = normalize_rules(rules, "negative_rules")?;
    }
    if let Some(status) = payload.status {
        profile.status = status;
    }
    Ok(Json(profile.clone()))
}

async fn create_parent_character_profile(
    State(state): State<SharedState>,
    Path(parent_id): Path<Uuid>,
    Json(payload): Json<CreateParentCharacterProfileRequest>,
) -> Result<Json<ParentCharacterProfileRecord>, ApiError> {
    let role = required_trimmed(payload.role, "role")?;
    let name = required_trimmed(payload.name, "name")?;
    let mut state = state.write().expect("state lock poisoned");
    validate_parent(&state, parent_id)?;
    if let Some(child_id) = payload.child_id {
        validate_child_visible(&state, child_id)?;
    }
    let version = payload
        .version
        .unwrap_or_else(|| next_parent_character_version(&state, parent_id, payload.child_id));
    if parent_character_version_exists(&state, parent_id, payload.child_id, version) {
        return Err(ApiError::validation(
            "version",
            "同一家长和儿童下角色卡版本不能重复",
        ));
    }
    let profile = ParentCharacterProfileRecord {
        id: Uuid::new_v4(),
        parent_id,
        child_id: payload.child_id,
        version,
        role,
        name,
        hair: payload.hair.and_then(normalize_optional_owned),
        skin_tone: payload.skin_tone.and_then(normalize_optional_owned),
        face_shape: payload.face_shape.and_then(normalize_optional_owned),
        body_proportion: payload.body_proportion.and_then(normalize_optional_owned),
        outfit_top: payload.outfit_top.and_then(normalize_optional_owned),
        outfit_bottom: payload.outfit_bottom.and_then(normalize_optional_owned),
        accessory: payload.accessory.and_then(normalize_optional_owned),
        visual_must_keep: normalize_rules(payload.visual_must_keep, "visual_must_keep")?,
        negative_rules: normalize_rules(payload.negative_rules, "negative_rules")?,
        active_reference_image_id: None,
        status: "draft".to_string(),
        created_at: now(),
    };
    state
        .visuals
        .parent_character_profiles
        .insert(profile.id, profile.clone());
    Ok(Json(profile))
}

async fn list_storybook_roles(
    State(state): State<SharedState>,
    Path(storybook_id): Path<Uuid>,
) -> Result<Json<ListResponse<StorybookRoleRecord>>, ApiError> {
    let mut state = state.write().expect("state lock poisoned");
    validate_storybook_visible(&state, storybook_id)?;
    ensure_storybook_roles(&mut state, storybook_id)?;
    let mut items = state
        .visuals
        .storybook_roles
        .values()
        .filter(|role| role.storybook_id == storybook_id)
        .cloned()
        .collect::<Vec<_>>();
    items.sort_by(|a, b| a.role_key.cmp(&b.role_key));
    Ok(Json(list_response(items)))
}

async fn update_storybook_role(
    State(state): State<SharedState>,
    Path((storybook_id, role_key)): Path<(Uuid, String)>,
    Json(payload): Json<UpdateStorybookRoleRequest>,
) -> Result<Json<StorybookRoleRecord>, ApiError> {
    let mut state = state.write().expect("state lock poisoned");
    validate_storybook_visible(&state, storybook_id)?;
    ensure_storybook_roles(&mut state, storybook_id)?;
    validate_role_targets(&state, &payload)?;
    let role = find_role_mut(&mut state, storybook_id, &role_key)?;
    if let Some(display_name) = payload.display_name {
        role.display_name = required_trimmed(display_name, "display_name")?;
    }
    if payload.child_id.is_some() {
        role.child_id = payload.child_id;
        role.role_type = "child".to_string();
    }
    if payload.character_profile_id.is_some() {
        role.character_profile_id = payload.character_profile_id;
        role.role_type = "child".to_string();
    }
    if payload.parent_character_profile_id.is_some() {
        role.parent_character_profile_id = payload.parent_character_profile_id;
        role.role_type = "parent".to_string();
    }
    if payload.prop_profile_id.is_some() {
        role.prop_profile_id = payload.prop_profile_id;
        role.role_type = "prop".to_string();
    }
    role.updated_at = now();
    let role = role.clone();
    sync_role_manifest(&mut state, storybook_id);
    mark_affected_pages_stale(&mut state, storybook_id, &[role.id]);
    Ok(Json(role))
}

async fn replace_storybook_roles(
    State(state): State<SharedState>,
    Path(storybook_id): Path<Uuid>,
    Json(payload): Json<ReplaceRolesRequest>,
) -> Result<Json<ReplaceRolesResponse>, ApiError> {
    if payload.replacements.is_empty() {
        return Err(ApiError::validation("replacements", "至少提供一个角色替换"));
    }
    let mut state = state.write().expect("state lock poisoned");
    validate_storybook_visible(&state, storybook_id)?;
    ensure_storybook_roles(&mut state, storybook_id)?;
    let mut changed_roles = Vec::new();
    for replacement in payload.replacements {
        validate_role_replacement(&state, &replacement)?;
        let role = find_role_mut(&mut state, storybook_id, &replacement.role_key)?;
        role.role_type = replacement.role_type;
        role.child_id = replacement.child_id;
        role.character_profile_id = replacement.character_profile_id;
        role.parent_character_profile_id = replacement.parent_character_profile_id;
        role.prop_profile_id = replacement.prop_profile_id;
        role.updated_at = now();
        changed_roles.push(role.role_key.clone());
    }
    sync_role_manifest(&mut state, storybook_id);
    let changed_role_ids = state
        .visuals
        .storybook_roles
        .values()
        .filter(|role| role.storybook_id == storybook_id && changed_roles.contains(&role.role_key))
        .map(|role| role.id)
        .collect::<Vec<_>>();
    mark_affected_pages_stale(&mut state, storybook_id, &changed_role_ids);
    let affected_page_ids = state
        .storybooks
        .pages
        .get(&storybook_id)
        .map(|pages| pages.iter().map(|page| page.id).collect::<Vec<_>>())
        .unwrap_or_default();
    Ok(Json(ReplaceRolesResponse {
        storybook_id,
        changed_roles,
        affected_page_ids,
        image_policy_result: "marked_stale".to_string(),
    }))
}

async fn list_prop_profiles(
    State(state): State<SharedState>,
    Path(storybook_id): Path<Uuid>,
) -> Result<Json<ListResponse<PropProfileRecord>>, ApiError> {
    let state = state.read().expect("state lock poisoned");
    validate_storybook_visible(&state, storybook_id)?;
    let mut items = state
        .visuals
        .prop_profiles
        .values()
        .filter(|prop| prop.storybook_id == Some(storybook_id))
        .cloned()
        .collect::<Vec<_>>();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(list_response(items)))
}

async fn create_prop_profile(
    State(state): State<SharedState>,
    Path(storybook_id): Path<Uuid>,
    Json(payload): Json<CreatePropProfileRequest>,
) -> Result<Json<PropProfileRecord>, ApiError> {
    let name = required_trimmed(payload.name, "name")?;
    let visual_must_keep = normalize_rules(payload.visual_must_keep, "visual_must_keep")?;
    let negative_rules = normalize_rules(payload.negative_rules, "negative_rules")?;
    let mut state = state.write().expect("state lock poisoned");
    validate_storybook_visible(&state, storybook_id)?;
    if let Some(child_id) = payload.child_id {
        validate_child_visible(&state, child_id)?;
    }
    let created_at = now();
    let prop = PropProfileRecord {
        id: Uuid::new_v4(),
        storybook_id: Some(storybook_id),
        child_id: payload.child_id,
        name,
        shape: payload.shape.and_then(normalize_optional_owned),
        primary_color: payload.primary_color.and_then(normalize_optional_owned),
        secondary_color: payload.secondary_color.and_then(normalize_optional_owned),
        material_style: payload.material_style.and_then(normalize_optional_owned),
        size_description: payload.size_description.and_then(normalize_optional_owned),
        visual_must_keep,
        negative_rules,
        active_reference_image_id: None,
        status: "draft".to_string(),
        created_at,
        updated_at: created_at,
    };
    state.visuals.prop_profiles.insert(prop.id, prop.clone());
    Ok(Json(prop))
}

async fn update_prop_profile(
    State(state): State<SharedState>,
    Path(prop_id): Path<Uuid>,
    Json(payload): Json<UpdatePropProfileRequest>,
) -> Result<Json<PropProfileRecord>, ApiError> {
    validate_optional_status(
        payload.status.as_deref(),
        &["draft", "active", "archived"],
        "status",
    )?;
    let mut state = state.write().expect("state lock poisoned");
    let storybook_id = state
        .visuals
        .prop_profiles
        .get(&prop_id)
        .and_then(|prop| prop.storybook_id)
        .ok_or_else(|| ApiError::not_found("prop_profile"))?;
    validate_storybook_visible(&state, storybook_id)?;
    let prop = state.visuals.prop_profiles.get_mut(&prop_id).unwrap();
    if prop.active_reference_image_id.is_some() && prop_profile_visual_fields_changed(&payload) {
        return Err(ApiError::state_conflict(
            "active 道具参考图存在时，视觉特征变更必须重新生成参考图",
        ));
    }
    apply_optional_string(&mut prop.shape, payload.shape);
    apply_optional_string(&mut prop.primary_color, payload.primary_color);
    apply_optional_string(&mut prop.secondary_color, payload.secondary_color);
    apply_optional_string(&mut prop.material_style, payload.material_style);
    apply_optional_string(&mut prop.size_description, payload.size_description);
    if let Some(rules) = payload.visual_must_keep {
        prop.visual_must_keep = normalize_rules(rules, "visual_must_keep")?;
    }
    if let Some(rules) = payload.negative_rules {
        prop.negative_rules = normalize_rules(rules, "negative_rules")?;
    }
    if let Some(status) = payload.status {
        prop.status = status;
    }
    prop.updated_at = now();
    Ok(Json(prop.clone()))
}

async fn put_page_visual_subjects(
    State(state): State<SharedState>,
    Path(page_id): Path<Uuid>,
    Json(payload): Json<PutPageVisualSubjectsRequest>,
) -> Result<Json<ListResponse<PageVisualSubjectRecord>>, ApiError> {
    let mut state = state.write().expect("state lock poisoned");
    let storybook_id = storybook_id_for_page(&state, page_id)?;
    validate_storybook_visible(&state, storybook_id)?;
    let mut subjects = Vec::new();
    let mut subject_keys = std::collections::BTreeSet::new();
    for input in payload.subjects {
        validate_importance(&input.importance)?;
        match input.subject_type.as_str() {
            "storybook_role" => {
                let role_id = input.storybook_role_id.ok_or_else(|| {
                    ApiError::validation("storybook_role_id", "storybook_role 主体必须提供 role")
                })?;
                if !subject_keys.insert(("storybook_role", role_id)) {
                    return Err(ApiError::validation(
                        "storybook_role_id",
                        "同一页面不能重复添加同一个角色主体",
                    ));
                }
                let role = state
                    .visuals
                    .storybook_roles
                    .get(&role_id)
                    .ok_or_else(|| ApiError::not_found("storybook_role"))?;
                if role.storybook_id != storybook_id {
                    return Err(ApiError::validation(
                        "storybook_role_id",
                        "角色必须属于同一本读本",
                    ));
                }
                subjects.push(PageVisualSubjectRecord {
                    id: Uuid::new_v4(),
                    storybook_page_id: page_id,
                    subject_type: input.subject_type,
                    storybook_role_id: Some(role_id),
                    prop_profile_id: None,
                    importance: input.importance,
                    placement_hint: input.placement_hint.and_then(normalize_optional_owned),
                    created_at: now(),
                });
            }
            "prop" => {
                let prop_id = input.prop_profile_id.ok_or_else(|| {
                    ApiError::validation("prop_profile_id", "prop 主体必须提供 prop_profile_id")
                })?;
                if !subject_keys.insert(("prop", prop_id)) {
                    return Err(ApiError::validation(
                        "prop_profile_id",
                        "同一页面不能重复添加同一个道具主体",
                    ));
                }
                let prop = state
                    .visuals
                    .prop_profiles
                    .get(&prop_id)
                    .ok_or_else(|| ApiError::not_found("prop_profile"))?;
                if prop.storybook_id != Some(storybook_id) {
                    return Err(ApiError::validation(
                        "prop_profile_id",
                        "道具必须属于同一本读本",
                    ));
                }
                subjects.push(PageVisualSubjectRecord {
                    id: Uuid::new_v4(),
                    storybook_page_id: page_id,
                    subject_type: input.subject_type,
                    storybook_role_id: None,
                    prop_profile_id: Some(prop_id),
                    importance: input.importance,
                    placement_hint: input.placement_hint.and_then(normalize_optional_owned),
                    created_at: now(),
                });
            }
            _ => return Err(ApiError::validation("subject_type", "视觉主体类型不合法")),
        }
    }
    state
        .visuals
        .page_visual_subjects
        .insert(page_id, subjects.clone());
    Ok(Json(list_response(subjects)))
}

async fn generate_reference_image(
    State(state): State<SharedState>,
    Json(payload): Json<GenerateReferenceImageRequest>,
) -> Result<Json<ReferenceImageRecord>, ApiError> {
    validate_subject_target(&payload)?;
    let mut state = state.write().expect("state lock poisoned");
    validate_reference_subject_visible(&state, &payload)?;
    let reference = ReferenceImageRecord {
        id: Uuid::new_v4(),
        subject_type: payload.subject_type,
        character_profile_id: payload.character_profile_id,
        parent_character_profile_id: payload.parent_character_profile_id,
        prop_profile_id: payload.prop_profile_id,
        image_asset_id: Uuid::new_v4(),
        source_task_id: None,
        style_id: required_trimmed(payload.style_id, "style_id")?,
        review_status: "approved".to_string(),
        is_active: false,
        created_at: now(),
    };
    state
        .visuals
        .reference_images
        .insert(reference.id, reference.clone());
    Ok(Json(reference))
}

async fn get_reference_image(
    State(state): State<SharedState>,
    Path(reference_image_id): Path<Uuid>,
) -> Result<Json<ReferenceImageRecord>, ApiError> {
    let state = state.read().expect("state lock poisoned");
    let reference = state
        .visuals
        .reference_images
        .get(&reference_image_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("reference_image"))?;
    validate_reference_record_visible(&state, &reference)?;
    Ok(Json(reference))
}

async fn activate_reference_image(
    State(state): State<SharedState>,
    Path(reference_image_id): Path<Uuid>,
) -> Result<Json<ReferenceImageRecord>, ApiError> {
    let mut state = state.write().expect("state lock poisoned");
    let reference = state
        .visuals
        .reference_images
        .get(&reference_image_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("reference_image"))?;
    validate_reference_record_visible(&state, &reference)?;
    if reference.review_status != "approved" {
        return Err(ApiError::state_conflict("只有审核通过的参考图可以启用"));
    }
    for other in state.visuals.reference_images.values_mut() {
        if other.subject_type == reference.subject_type
            && other.style_id == reference.style_id
            && other.character_profile_id == reference.character_profile_id
            && other.parent_character_profile_id == reference.parent_character_profile_id
            && other.prop_profile_id == reference.prop_profile_id
        {
            other.is_active = false;
        }
    }
    let active_reference = state
        .visuals
        .reference_images
        .get_mut(&reference_image_id)
        .unwrap();
    active_reference.is_active = true;
    let active_reference = active_reference.clone();
    if let Some(profile_id) = active_reference.character_profile_id {
        if let Some(profile) = state.visuals.character_profiles.get_mut(&profile_id) {
            profile.active_reference_image_id = Some(reference_image_id);
            profile.status = "active".to_string();
        }
    }
    if let Some(profile_id) = active_reference.parent_character_profile_id {
        if let Some(profile) = state.visuals.parent_character_profiles.get_mut(&profile_id) {
            profile.active_reference_image_id = Some(reference_image_id);
            profile.status = "active".to_string();
        }
    }
    if let Some(prop_id) = active_reference.prop_profile_id {
        if let Some(prop) = state.visuals.prop_profiles.get_mut(&prop_id) {
            prop.active_reference_image_id = Some(reference_image_id);
            prop.status = "active".to_string();
            prop.updated_at = now();
        }
    }
    Ok(Json(active_reference))
}

fn validate_child_visible(
    state: &crate::api::AppState,
    child_id: Uuid,
) -> Result<&crate::api::children::ChildRecord, ApiError> {
    let child = state
        .children
        .children
        .get(&child_id)
        .ok_or_else(|| ApiError::not_found("child"))?;
    if child.school_id != Some(state.organization.current_school_id) {
        return Err(ApiError::forbidden("不能访问其他园所的儿童档案"));
    }
    Ok(child)
}

fn validate_parent(state: &crate::api::AppState, parent_id: Uuid) -> Result<(), ApiError> {
    let parent = state
        .children
        .parents
        .get(&parent_id)
        .ok_or_else(|| ApiError::not_found("parent"))?;
    if parent.status != "active" {
        return Err(ApiError::validation("parent_id", "家长状态必须为 active"));
    }
    Ok(())
}

fn validate_child_photo(
    state: &crate::api::AppState,
    child_id: Uuid,
    photo_id: Uuid,
) -> Result<(), ApiError> {
    let photo = state
        .children
        .photos
        .get(&photo_id)
        .ok_or_else(|| ApiError::not_found("child_photo"))?;
    if photo.child_id != child_id || photo.consent_status == "revoked" {
        return Err(ApiError::validation(
            "source_photo_id",
            "照片必须属于儿童且未撤销授权",
        ));
    }
    Ok(())
}

fn validate_storybook_visible(
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
    Ok(())
}

fn storybook_id_for_page(state: &crate::api::AppState, page_id: Uuid) -> Result<Uuid, ApiError> {
    state
        .storybooks
        .pages
        .iter()
        .find_map(|(storybook_id, pages)| {
            pages
                .iter()
                .any(|page| page.id == page_id)
                .then_some(*storybook_id)
        })
        .ok_or_else(|| ApiError::not_found("storybook_page"))
}

fn ensure_storybook_roles(
    state: &mut crate::api::AppState,
    storybook_id: Uuid,
) -> Result<(), ApiError> {
    if state
        .visuals
        .storybook_roles
        .values()
        .any(|role| role.storybook_id == storybook_id)
    {
        return Ok(());
    }
    let storybook = state
        .storybooks
        .storybooks
        .get(&storybook_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("storybook"))?;
    let now = now();
    let display_name = storybook
        .role_manifest_json
        .get("protagonist")
        .and_then(|role| role.get("display_name"))
        .and_then(Value::as_str)
        .unwrap_or("小朋友")
        .to_string();
    let role = StorybookRoleRecord {
        id: Uuid::new_v4(),
        storybook_id,
        role_key: "protagonist".to_string(),
        role_type: if storybook.child_id.is_some() {
            "child".to_string()
        } else {
            "default_character".to_string()
        },
        display_name,
        child_id: storybook.child_id,
        character_profile_id: None,
        parent_character_profile_id: None,
        prop_profile_id: None,
        replacement_source_role_id: None,
        created_at: now,
        updated_at: now,
    };
    state.visuals.storybook_roles.insert(role.id, role);
    Ok(())
}

fn find_role_mut<'a>(
    state: &'a mut crate::api::AppState,
    storybook_id: Uuid,
    role_key: &str,
) -> Result<&'a mut StorybookRoleRecord, ApiError> {
    state
        .visuals
        .storybook_roles
        .values_mut()
        .find(|role| role.storybook_id == storybook_id && role.role_key == role_key)
        .ok_or_else(|| ApiError::not_found("storybook_role"))
}

fn validate_role_targets(
    state: &crate::api::AppState,
    payload: &UpdateStorybookRoleRequest,
) -> Result<(), ApiError> {
    if let Some(child_id) = payload.child_id {
        validate_child_visible(state, child_id)?;
    }
    if let Some(profile_id) = payload.character_profile_id {
        let profile = state
            .visuals
            .character_profiles
            .get(&profile_id)
            .ok_or_else(|| ApiError::not_found("character_profile"))?;
        validate_child_visible(state, profile.child_id)?;
    }
    if let Some(profile_id) = payload.parent_character_profile_id {
        let profile = state
            .visuals
            .parent_character_profiles
            .get(&profile_id)
            .ok_or_else(|| ApiError::not_found("parent_character_profile"))?;
        validate_parent(state, profile.parent_id)?;
    }
    if let Some(prop_id) = payload.prop_profile_id {
        let prop = state
            .visuals
            .prop_profiles
            .get(&prop_id)
            .ok_or_else(|| ApiError::not_found("prop_profile"))?;
        if let Some(storybook_id) = prop.storybook_id {
            validate_storybook_visible(state, storybook_id)?;
        }
    }
    Ok(())
}

fn character_profile_visual_fields_changed(payload: &UpdateCharacterProfileRequest) -> bool {
    payload.hair.is_some()
        || payload.body_proportion.is_some()
        || payload.outfit_top.is_some()
        || payload.outfit_bottom.is_some()
        || payload.accessory.is_some()
        || payload.visual_must_keep.is_some()
        || payload.negative_rules.is_some()
}

fn prop_profile_visual_fields_changed(payload: &UpdatePropProfileRequest) -> bool {
    payload.shape.is_some()
        || payload.primary_color.is_some()
        || payload.secondary_color.is_some()
        || payload.material_style.is_some()
        || payload.size_description.is_some()
        || payload.visual_must_keep.is_some()
        || payload.negative_rules.is_some()
}

fn mark_affected_pages_stale(
    state: &mut crate::api::AppState,
    storybook_id: Uuid,
    changed_role_ids: &[Uuid],
) {
    if changed_role_ids.is_empty() {
        return;
    }
    let affected_page_ids = state
        .visuals
        .page_visual_subjects
        .iter()
        .filter_map(|(page_id, subjects)| {
            subjects
                .iter()
                .any(|subject| {
                    subject
                        .storybook_role_id
                        .is_some_and(|role_id| changed_role_ids.contains(&role_id))
                })
                .then_some(*page_id)
        })
        .collect::<Vec<_>>();
    if affected_page_ids.is_empty() {
        return;
    }
    if let Some(pages) = state.storybooks.pages.get_mut(&storybook_id) {
        for page in pages {
            if affected_page_ids.contains(&page.id) {
                page.current_image_asset_id = None;
                page.current_image_task_id = None;
                page.illustration_status = "not_started".to_string();
                page.updated_at = now();
            }
        }
    }
    if let Some(storybook) = state.storybooks.storybooks.get_mut(&storybook_id) {
        storybook.illustration_status = "not_started".to_string();
        storybook.updated_at = now();
    }
}

fn validate_role_replacement(
    state: &crate::api::AppState,
    payload: &RoleReplacementRequest,
) -> Result<(), ApiError> {
    if !["child", "parent", "teacher", "default_character", "prop"]
        .contains(&payload.role_type.as_str())
    {
        return Err(ApiError::validation("role_type", "角色类型不合法"));
    }
    validate_role_targets(
        state,
        &UpdateStorybookRoleRequest {
            display_name: None,
            child_id: payload.child_id,
            character_profile_id: payload.character_profile_id,
            parent_character_profile_id: payload.parent_character_profile_id,
            prop_profile_id: payload.prop_profile_id,
        },
    )
}

fn sync_role_manifest(state: &mut crate::api::AppState, storybook_id: Uuid) {
    let roles = state
        .visuals
        .storybook_roles
        .values()
        .filter(|role| role.storybook_id == storybook_id)
        .map(|role| {
            (
                role.role_key.clone(),
                json!({
                    "role_key": role.role_key,
                    "role_type": role.role_type,
                    "display_name": role.display_name,
                    "child_id": role.child_id,
                    "character_profile_id": role.character_profile_id,
                    "parent_character_profile_id": role.parent_character_profile_id,
                    "prop_profile_id": role.prop_profile_id
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    if let Some(storybook) = state.storybooks.storybooks.get_mut(&storybook_id) {
        storybook.role_manifest_json = Value::Object(roles);
        storybook.updated_at = now();
    }
}

fn validate_subject_target(payload: &GenerateReferenceImageRequest) -> Result<(), ApiError> {
    let provided = [
        payload.character_profile_id.is_some(),
        payload.parent_character_profile_id.is_some(),
        payload.prop_profile_id.is_some(),
    ]
    .into_iter()
    .filter(|provided| *provided)
    .count();
    if provided != 1 {
        return Err(ApiError::validation(
            "subject_type",
            "必须且只能提供一个参考图主体",
        ));
    }
    match payload.subject_type.as_str() {
        "child_character" if payload.character_profile_id.is_some() => Ok(()),
        "parent_character" if payload.parent_character_profile_id.is_some() => Ok(()),
        "prop" if payload.prop_profile_id.is_some() => Ok(()),
        _ => Err(ApiError::validation(
            "subject_type",
            "主体类型和目标 ID 不匹配",
        )),
    }
}

fn validate_reference_subject_visible(
    state: &crate::api::AppState,
    payload: &GenerateReferenceImageRequest,
) -> Result<(), ApiError> {
    if let Some(profile_id) = payload.character_profile_id {
        let profile = state
            .visuals
            .character_profiles
            .get(&profile_id)
            .ok_or_else(|| ApiError::not_found("character_profile"))?;
        validate_child_visible(state, profile.child_id)?;
        if profile.visual_must_keep.len() < 3 {
            return Err(ApiError::validation(
                "visual_must_keep",
                "生成参考图前至少需要 3 条 must_keep",
            ));
        }
    }
    if let Some(profile_id) = payload.parent_character_profile_id {
        let profile = state
            .visuals
            .parent_character_profiles
            .get(&profile_id)
            .ok_or_else(|| ApiError::not_found("parent_character_profile"))?;
        validate_parent(state, profile.parent_id)?;
    }
    if let Some(prop_id) = payload.prop_profile_id {
        let prop = state
            .visuals
            .prop_profiles
            .get(&prop_id)
            .ok_or_else(|| ApiError::not_found("prop_profile"))?;
        if let Some(storybook_id) = prop.storybook_id {
            validate_storybook_visible(state, storybook_id)?;
        }
    }
    Ok(())
}

fn validate_reference_record_visible(
    state: &crate::api::AppState,
    reference: &ReferenceImageRecord,
) -> Result<(), ApiError> {
    validate_reference_subject_visible(
        state,
        &GenerateReferenceImageRequest {
            subject_type: reference.subject_type.clone(),
            character_profile_id: reference.character_profile_id,
            parent_character_profile_id: reference.parent_character_profile_id,
            prop_profile_id: reference.prop_profile_id,
            style_id: reference.style_id.clone(),
        },
    )
}

fn next_character_version(state: &crate::api::AppState, child_id: Uuid) -> i32 {
    state
        .visuals
        .character_profiles
        .values()
        .filter(|profile| profile.child_id == child_id)
        .map(|profile| profile.version)
        .max()
        .unwrap_or(0)
        + 1
}

fn next_parent_character_version(
    state: &crate::api::AppState,
    parent_id: Uuid,
    child_id: Option<Uuid>,
) -> i32 {
    state
        .visuals
        .parent_character_profiles
        .values()
        .filter(|profile| profile.parent_id == parent_id && profile.child_id == child_id)
        .map(|profile| profile.version)
        .max()
        .unwrap_or(0)
        + 1
}

fn parent_character_version_exists(
    state: &crate::api::AppState,
    parent_id: Uuid,
    child_id: Option<Uuid>,
    version: i32,
) -> bool {
    state
        .visuals
        .parent_character_profiles
        .values()
        .any(|profile| {
            profile.parent_id == parent_id
                && profile.child_id == child_id
                && profile.version == version
        })
}

fn validate_importance(importance: &str) -> Result<(), ApiError> {
    if ["primary", "medium", "low"].contains(&importance) {
        Ok(())
    } else {
        Err(ApiError::validation("importance", "重要性枚举不合法"))
    }
}

fn validate_optional_status(
    value: Option<&str>,
    allowed: &[&str],
    field: &'static str,
) -> Result<(), ApiError> {
    if let Some(value) = value {
        if !allowed.contains(&value) {
            return Err(ApiError::validation(field, "状态枚举不合法"));
        }
    }
    Ok(())
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

fn apply_optional_string(target: &mut Option<String>, value: Option<String>) {
    if let Some(value) = value {
        *target = normalize_optional_owned(value);
    }
}

fn normalize_rules(values: Vec<String>, field: &'static str) -> Result<Vec<String>, ApiError> {
    let mut normalized = Vec::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        if value.chars().count() > 80 {
            return Err(ApiError::validation(field, "单条规则不能超过 80 字"));
        }
        if !normalized.iter().any(|existing| existing == value) {
            normalized.push(value.to_string());
        }
    }
    Ok(normalized)
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

    async fn first_child_id(app: axum::Router) -> String {
        let (_, children) = get_json(app, "/api/children").await;
        children["items"][0]["id"].as_str().unwrap().to_string()
    }

    async fn first_parent_id(app: axum::Router) -> String {
        let child_id = first_child_id(app.clone()).await;
        let (_, child) = get_json(app, &format!("/api/children/{child_id}")).await;
        child["primary_parent"]["id"].as_str().unwrap().to_string()
    }

    async fn create_storybook(app: axum::Router) -> Value {
        let (_, cases) = get_json(app.clone(), "/api/cases").await;
        let case_id = cases["items"][0]["id"].as_str().unwrap();
        let (status, body) = request_json(
            app,
            "POST",
            "/api/storybooks/generate",
            json!({
                "content_type": "plain_storybook",
                "case_storybook_id": case_id
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        body["storybook"].clone()
    }

    #[tokio::test]
    async fn creates_character_profile_and_requires_must_keep_for_reference() {
        let app = test_app();
        let child_id = first_child_id(app.clone()).await;
        let (status, profile) = request_json(
            app.clone(),
            "POST",
            &format!("/api/children/{child_id}/character-profiles"),
            json!({
                "hair": "黑色短发",
                "age_group": "5-6",
                "body_proportion": "幼儿比例",
                "visual_must_keep": ["黑色短发", "黄色卫衣"]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(profile["version"], 1);

        let profile_id = profile["id"].as_str().unwrap();
        let (status, body) = request_json(
            app,
            "POST",
            "/api/reference-images/generate",
            json!({
                "subject_type": "child_character",
                "character_profile_id": profile_id,
                "style_id": "storybook_flat_v1"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["details"][0]["field"], "visual_must_keep");
    }

    #[tokio::test]
    async fn character_profile_requires_age_group() {
        let app = test_app();
        let child_id = first_child_id(app.clone()).await;
        let (status, body) = request_json(
            app,
            "POST",
            &format!("/api/children/{child_id}/character-profiles"),
            json!({
                "hair": "黑色短发",
                "body_proportion": "幼儿比例",
                "visual_must_keep": ["黑色短发", "黄色卫衣", "圆脸"]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["details"][0]["field"], "age_group");
    }

    #[tokio::test]
    async fn activates_reference_image_for_character_profile() {
        let app = test_app();
        let child_id = first_child_id(app.clone()).await;
        let (_, profile) = request_json(
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
        assert_eq!(status, StatusCode::OK);
        let reference_id = reference["id"].as_str().unwrap();
        let (status, active) = request_json(
            app.clone(),
            "POST",
            &format!("/api/reference-images/{reference_id}/activate"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(active["is_active"], true);

        let (_, updated_profile) =
            get_json(app, &format!("/api/character-profiles/{profile_id}")).await;
        assert_eq!(updated_profile["active_reference_image_id"], reference_id);
    }

    #[tokio::test]
    async fn rejects_visual_edits_to_active_character_profile() {
        let app = test_app();
        let child_id = first_child_id(app.clone()).await;
        let (_, profile) = request_json(
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
        let profile_id = profile["id"].as_str().unwrap();
        let (_, reference) = request_json(
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
        let reference_id = reference["id"].as_str().unwrap();
        let (status, _) = request_json(
            app.clone(),
            "POST",
            &format!("/api/reference-images/{reference_id}/activate"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, body) = request_json(
            app,
            "PATCH",
            &format!("/api/character-profiles/{profile_id}"),
            json!({ "hair": "棕色短发" }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["code"], "STATE_CONFLICT");
    }

    #[tokio::test]
    async fn creates_parent_character_and_prop_profiles() {
        let app = test_app();
        let parent_id = first_parent_id(app.clone()).await;
        let (status, parent_profile) = request_json(
            app.clone(),
            "POST",
            &format!("/api/parents/{parent_id}/character-profiles"),
            json!({
                "role": "妈妈",
                "name": "张女士",
                "visual_must_keep": ["黑色长发"]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(parent_profile["role"], "妈妈");

        let storybook = create_storybook(app.clone()).await;
        let storybook_id = storybook["id"].as_str().unwrap();
        let (status, prop) = request_json(
            app,
            "POST",
            &format!("/api/storybooks/{storybook_id}/props"),
            json!({
                "name": "小熊玩偶",
                "shape": "圆头小熊",
                "primary_color": "棕色",
                "visual_must_keep": ["圆耳朵", "棕色毛绒", "米色肚子"]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(prop["name"], "小熊玩偶");
    }

    #[tokio::test]
    async fn rejects_duplicate_parent_character_version() {
        let app = test_app();
        let parent_id = first_parent_id(app.clone()).await;
        let (status, first) = request_json(
            app.clone(),
            "POST",
            &format!("/api/parents/{parent_id}/character-profiles"),
            json!({
                "version": 1,
                "role": "妈妈",
                "name": "张女士",
                "visual_must_keep": ["黑色长发"]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{first}");

        let (status, body) = request_json(
            app,
            "POST",
            &format!("/api/parents/{parent_id}/character-profiles"),
            json!({
                "version": 1,
                "role": "妈妈",
                "name": "张女士",
                "visual_must_keep": ["黑色长发"]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["details"][0]["field"], "version");
    }

    #[tokio::test]
    async fn rejects_visual_edits_to_prop_with_active_reference() {
        let app = test_app();
        let storybook = create_storybook(app.clone()).await;
        let storybook_id = storybook["id"].as_str().unwrap();
        let (_, prop) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybooks/{storybook_id}/props"),
            json!({
                "name": "小熊玩偶",
                "shape": "圆头小熊",
                "visual_must_keep": ["圆耳朵", "棕色毛绒", "米色肚子"]
            }),
        )
        .await;
        let prop_id = prop["id"].as_str().unwrap();
        let (_, reference) = request_json(
            app.clone(),
            "POST",
            "/api/reference-images/generate",
            json!({
                "subject_type": "prop",
                "prop_profile_id": prop_id,
                "style_id": "storybook_flat_v1"
            }),
        )
        .await;
        let reference_id = reference["id"].as_str().unwrap();
        let (status, _) = request_json(
            app.clone(),
            "POST",
            &format!("/api/reference-images/{reference_id}/activate"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, body) = request_json(
            app,
            "PATCH",
            &format!("/api/prop-profiles/{prop_id}"),
            json!({ "shape": "方头小熊" }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["code"], "STATE_CONFLICT");
    }

    #[tokio::test]
    async fn replaces_storybook_role_and_syncs_manifest() {
        let app = test_app();
        let storybook = create_storybook(app.clone()).await;
        let storybook_id = storybook["id"].as_str().unwrap();
        let (_, detail) = get_json(app.clone(), &format!("/api/storybooks/{storybook_id}")).await;
        let page_id = detail["pages"][0]["id"].as_str().unwrap();
        let (_, roles) = get_json(
            app.clone(),
            &format!("/api/storybooks/{storybook_id}/roles"),
        )
        .await;
        let role_id = roles["items"][0]["id"].as_str().unwrap();
        let (status, _) = request_json(
            app.clone(),
            "PUT",
            &format!("/api/storybook-pages/{page_id}/visual-subjects"),
            json!({
                "subjects": [{
                    "subject_type": "storybook_role",
                    "storybook_role_id": role_id,
                    "importance": "primary"
                }]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, image_task) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybook-pages/{page_id}/image-tasks"),
            json!({
                "style_id": "storybook_flat_v1",
                "prompt_template_version": "page_image_v1"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{image_task}");

        let child_id = first_child_id(app.clone()).await;
        let (_, profile) = request_json(
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
        let profile_id = profile["id"].as_str().unwrap();
        let (status, response) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybooks/{storybook_id}/replace-roles"),
            json!({
                "replacements": [{
                    "role_key": "protagonist",
                    "role_type": "child",
                    "child_id": child_id,
                    "character_profile_id": profile_id
                }]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(response["changed_roles"][0], "protagonist");

        let (_, detail) = get_json(app, &format!("/api/storybooks/{storybook_id}")).await;
        assert_eq!(
            detail["role_manifest_json"]["protagonist"]["character_profile_id"],
            profile_id
        );
        assert_eq!(detail["pages"][0]["current_image_task_id"], Value::Null);
        assert_eq!(detail["pages"][0]["illustration_status"], "not_started");
    }

    #[tokio::test]
    async fn puts_page_visual_subjects_for_role_and_prop() {
        let app = test_app();
        let storybook = create_storybook(app.clone()).await;
        let storybook_id = storybook["id"].as_str().unwrap();
        let (_, roles) = get_json(
            app.clone(),
            &format!("/api/storybooks/{storybook_id}/roles"),
        )
        .await;
        let role_id = roles["items"][0]["id"].as_str().unwrap();
        let (_, prop) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybooks/{storybook_id}/props"),
            json!({
                "name": "小熊玩偶",
                "visual_must_keep": ["圆耳朵", "棕色毛绒", "米色肚子"]
            }),
        )
        .await;
        let prop_id = prop["id"].as_str().unwrap();
        let (_, detail) = get_json(app.clone(), &format!("/api/storybooks/{storybook_id}")).await;
        let page_id = detail["pages"][0]["id"].as_str().unwrap();
        let (status, subjects) = request_json(
            app,
            "PUT",
            &format!("/api/storybook-pages/{page_id}/visual-subjects"),
            json!({
                "subjects": [
                    {
                        "subject_type": "storybook_role",
                        "storybook_role_id": role_id,
                        "importance": "primary",
                        "placement_hint": "画面中央"
                    },
                    {
                        "subject_type": "prop",
                        "prop_profile_id": prop_id,
                        "importance": "medium"
                    }
                ]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(subjects["total"], 2);
    }

    #[tokio::test]
    async fn rejects_duplicate_page_visual_subjects() {
        let app = test_app();
        let storybook = create_storybook(app.clone()).await;
        let storybook_id = storybook["id"].as_str().unwrap();
        let (_, roles) = get_json(
            app.clone(),
            &format!("/api/storybooks/{storybook_id}/roles"),
        )
        .await;
        let role_id = roles["items"][0]["id"].as_str().unwrap();
        let (_, detail) = get_json(app.clone(), &format!("/api/storybooks/{storybook_id}")).await;
        let page_id = detail["pages"][0]["id"].as_str().unwrap();
        let (status, body) = request_json(
            app,
            "PUT",
            &format!("/api/storybook-pages/{page_id}/visual-subjects"),
            json!({
                "subjects": [
                    {
                        "subject_type": "storybook_role",
                        "storybook_role_id": role_id,
                        "importance": "primary"
                    },
                    {
                        "subject_type": "storybook_role",
                        "storybook_role_id": role_id,
                        "importance": "medium"
                    }
                ]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["details"][0]["field"], "storybook_role_id");
    }
}
