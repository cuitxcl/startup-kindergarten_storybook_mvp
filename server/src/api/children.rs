use super::{ApiError, SharedState, demo_uuid, now};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/children", get(list_children).post(create_child))
        .route("/children/{child_id}", get(get_child).patch(update_child))
        .route("/children/{child_id}/photos", post(add_child_photo))
        .route(
            "/children/{child_id}/photos/{photo_id}",
            patch(update_child_photo),
        )
        .route(
            "/parent-intakes",
            get(list_parent_intakes).post(create_parent_intake),
        )
        .route(
            "/parent-intakes/{intake_id}/accept",
            post(accept_parent_intake),
        )
}

#[derive(Clone, Debug)]
pub struct ChildrenStore {
    pub children: BTreeMap<Uuid, ChildRecord>,
    pub parents: BTreeMap<Uuid, ParentRecord>,
    pub photos: BTreeMap<Uuid, ChildPhotoRecord>,
    pub parent_intakes: BTreeMap<Uuid, ParentIntakeRecord>,
}

impl ChildrenStore {
    pub fn demo(organization: &crate::api::organization::OrganizationStore) -> Self {
        let created_at = now();
        let child_id = demo_uuid(10);
        let parent_id = demo_uuid(11);
        let photo_id = demo_uuid(12);
        let asset_id = demo_uuid(13);

        let mut parents = BTreeMap::new();
        parents.insert(
            parent_id,
            ParentRecord {
                id: parent_id,
                name: "张女士".to_string(),
                relationship_to_child: Some("妈妈".to_string()),
                phone: Some("13800000000".to_string()),
                email: None,
                status: "active".to_string(),
                created_at,
                updated_at: created_at,
            },
        );

        let mut children = BTreeMap::new();
        children.insert(
            child_id,
            ChildRecord {
                id: child_id,
                school_id: Some(organization.current_school_id),
                classroom_id: organization.classrooms.keys().next().copied(),
                primary_teacher_id: organization.current_teacher_id,
                primary_parent_id: Some(parent_id),
                name: "乐乐".to_string(),
                nickname: Some("乐乐".to_string()),
                age: Some(5),
                age_group: Some("5-6".to_string()),
                gender_expression: Some("男孩".to_string()),
                hair: Some("黑色短发".to_string()),
                skin_tone: Some("自然肤色".to_string()),
                usual_outfit: Some("黄色卫衣".to_string()),
                favorite_color: Some("黄色".to_string()),
                interest_tags: vec!["积木".to_string(), "画画".to_string()],
                teacher_observation_tags: vec!["愿意合作".to_string()],
                teaching_focus: Some("练习轮流和分享".to_string()),
                profile_completion_status: "complete".to_string(),
                status: "active".to_string(),
                created_at,
                updated_at: created_at,
            },
        );

        let mut photos = BTreeMap::new();
        photos.insert(
            photo_id,
            ChildPhotoRecord {
                id: photo_id,
                child_id,
                image_asset_id: asset_id,
                photo_type: "portrait".to_string(),
                is_primary: true,
                consent_status: "granted".to_string(),
                created_at,
            },
        );

        Self {
            children,
            parents,
            photos,
            parent_intakes: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ChildRecord {
    pub id: Uuid,
    pub school_id: Option<Uuid>,
    pub classroom_id: Option<Uuid>,
    pub primary_teacher_id: Uuid,
    pub primary_parent_id: Option<Uuid>,
    pub name: String,
    pub nickname: Option<String>,
    pub age: Option<i32>,
    pub age_group: Option<String>,
    pub gender_expression: Option<String>,
    pub hair: Option<String>,
    pub skin_tone: Option<String>,
    pub usual_outfit: Option<String>,
    pub favorite_color: Option<String>,
    pub interest_tags: Vec<String>,
    pub teacher_observation_tags: Vec<String>,
    pub teaching_focus: Option<String>,
    pub profile_completion_status: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ParentRecord {
    pub id: Uuid,
    pub name: String,
    pub relationship_to_child: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChildPhotoRecord {
    pub id: Uuid,
    pub child_id: Uuid,
    pub image_asset_id: Uuid,
    pub photo_type: String,
    pub is_primary: bool,
    pub consent_status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ParentIntakeRecord {
    pub id: Uuid,
    pub invite_token: String,
    pub parent_name: String,
    pub relationship_to_child: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub child_name: String,
    pub child_payload: IntakeChildPayload,
    pub parent_character_profile: Option<ParentCharacterProfileInput>,
    pub photo_asset_ids: Vec<Uuid>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub accepted_child_id: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct IntakeChildPayload {
    pub name: String,
    pub nickname: Option<String>,
    pub age: Option<i32>,
    pub age_group: Option<String>,
    pub gender_expression: Option<String>,
    pub hair: Option<String>,
    pub skin_tone: Option<String>,
    pub usual_outfit: Option<String>,
    pub favorite_color: Option<String>,
    #[serde(default)]
    pub interest_tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ParentCharacterProfileInput {
    pub role: String,
    pub hair: Option<String>,
    pub outfit_top: Option<String>,
    #[serde(default)]
    pub visual_must_keep: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChildListQuery {
    pub keyword: Option<String>,
    pub classroom_id: Option<Uuid>,
    pub profile_status: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateChildRequest {
    pub name: String,
    pub nickname: Option<String>,
    pub age: Option<i32>,
    pub age_group: Option<String>,
    pub classroom_id: Option<Uuid>,
    pub primary_parent_id: Option<Uuid>,
    pub gender_expression: Option<String>,
    pub hair: Option<String>,
    pub skin_tone: Option<String>,
    pub usual_outfit: Option<String>,
    pub favorite_color: Option<String>,
    #[serde(default)]
    pub interest_tags: Vec<String>,
    #[serde(default)]
    pub teacher_observation_tags: Vec<String>,
    pub teaching_focus: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateChildRequest {
    pub name: Option<String>,
    pub nickname: Option<String>,
    pub age: Option<i32>,
    pub age_group: Option<String>,
    pub classroom_id: Option<Uuid>,
    pub primary_parent_id: Option<Uuid>,
    pub gender_expression: Option<String>,
    pub hair: Option<String>,
    pub skin_tone: Option<String>,
    pub usual_outfit: Option<String>,
    pub favorite_color: Option<String>,
    pub interest_tags: Option<Vec<String>>,
    pub teacher_observation_tags: Option<Vec<String>>,
    pub teaching_focus: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddChildPhotoRequest {
    pub image_asset_id: Uuid,
    pub photo_type: String,
    pub is_primary: Option<bool>,
    pub consent_status: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateChildPhotoRequest {
    pub is_primary: Option<bool>,
    pub consent_status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateParentIntakeRequest {
    pub invite_token: String,
    pub parent: IntakeParentPayload,
    pub child: IntakeChildPayload,
    pub parent_character_profile: Option<ParentCharacterProfileInput>,
    #[serde(default)]
    pub photo_asset_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct IntakeParentPayload {
    pub name: String,
    pub relationship_to_child: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ParentIntakeListQuery {
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct ChildDetailResponse {
    #[serde(flatten)]
    pub child: ChildRecord,
    pub classroom: Option<ClassroomSummary>,
    pub primary_parent: Option<ParentRecord>,
    pub photos: Vec<ChildPhotoRecord>,
}

#[derive(Debug, Serialize)]
pub struct ClassroomSummary {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct AcceptParentIntakeResponse {
    pub intake_id: Uuid,
    pub child_id: Uuid,
    pub parent_id: Uuid,
    pub status: String,
}

async fn list_children(
    State(state): State<SharedState>,
    Query(query): Query<ChildListQuery>,
) -> Result<Json<ListResponse<ChildRecord>>, ApiError> {
    validate_optional_status(query.status.as_deref(), &["active", "archived"], "status")?;
    validate_optional_status(
        query.profile_status.as_deref(),
        &["missing_required", "usable", "complete"],
        "profile_status",
    )?;

    let state = state.read().expect("state lock poisoned");
    let school_id = state.organization.current_school_id;
    if let Some(classroom_id) = query.classroom_id {
        validate_classroom(&state, classroom_id, school_id)?;
    }

    let keyword = query
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let status = query.status.as_deref().unwrap_or("active");
    let mut items = state
        .children
        .children
        .values()
        .filter(|child| child.school_id == Some(school_id))
        .filter(|child| child.status == status)
        .filter(|child| {
            query
                .classroom_id
                .is_none_or(|id| child.classroom_id == Some(id))
        })
        .filter(|child| {
            query
                .profile_status
                .as_deref()
                .is_none_or(|profile_status| child.profile_completion_status == profile_status)
        })
        .filter(|child| {
            keyword.is_none_or(|keyword| {
                child.name.contains(keyword)
                    || child
                        .nickname
                        .as_deref()
                        .is_some_and(|nickname| nickname.contains(keyword))
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(Json(list_response(items)))
}

async fn create_child(
    State(state): State<SharedState>,
    Json(payload): Json<CreateChildRequest>,
) -> Result<Json<ChildRecord>, ApiError> {
    let name = required_trimmed(payload.name, "name")?;
    validate_age(payload.age)?;
    let interest_tags = normalize_tags(payload.interest_tags, "interest_tags")?;
    let teacher_observation_tags =
        normalize_tags(payload.teacher_observation_tags, "teacher_observation_tags")?;

    let mut state = state.write().expect("state lock poisoned");
    let school_id = state.organization.current_school_id;
    let teacher_id = state.organization.current_teacher_id;
    validate_classroom_option(&state, payload.classroom_id, school_id)?;
    validate_parent_option(&state, payload.primary_parent_id)?;

    let created_at = now();
    let mut child = ChildRecord {
        id: Uuid::new_v4(),
        school_id: Some(school_id),
        classroom_id: payload.classroom_id,
        primary_teacher_id: teacher_id,
        primary_parent_id: payload.primary_parent_id,
        name,
        nickname: payload.nickname.and_then(normalize_optional_owned),
        age: payload.age,
        age_group: payload.age_group.and_then(normalize_optional_owned),
        gender_expression: payload.gender_expression.and_then(normalize_optional_owned),
        hair: payload.hair.and_then(normalize_optional_owned),
        skin_tone: payload.skin_tone.and_then(normalize_optional_owned),
        usual_outfit: payload.usual_outfit.and_then(normalize_optional_owned),
        favorite_color: payload.favorite_color.and_then(normalize_optional_owned),
        interest_tags,
        teacher_observation_tags,
        teaching_focus: payload.teaching_focus.and_then(normalize_optional_owned),
        profile_completion_status: "missing_required".to_string(),
        status: "active".to_string(),
        created_at,
        updated_at: created_at,
    };
    refresh_profile_completion(&mut child, false);
    state.children.children.insert(child.id, child.clone());
    Ok(Json(child))
}

async fn get_child(
    State(state): State<SharedState>,
    Path(child_id): Path<Uuid>,
) -> Result<Json<ChildDetailResponse>, ApiError> {
    let state = state.read().expect("state lock poisoned");
    let child = visible_child(&state, child_id)?.clone();
    Ok(Json(child_detail(&state, child)))
}

async fn update_child(
    State(state): State<SharedState>,
    Path(child_id): Path<Uuid>,
    Json(payload): Json<UpdateChildRequest>,
) -> Result<Json<ChildRecord>, ApiError> {
    validate_age(payload.age)?;
    validate_optional_status(payload.status.as_deref(), &["active", "archived"], "status")?;
    if let Some(tags) = payload.interest_tags.as_ref() {
        normalize_tags(tags.clone(), "interest_tags")?;
    }
    if let Some(tags) = payload.teacher_observation_tags.as_ref() {
        normalize_tags(tags.clone(), "teacher_observation_tags")?;
    }

    let mut state = state.write().expect("state lock poisoned");
    let school_id = state.organization.current_school_id;
    validate_classroom_option(&state, payload.classroom_id, school_id)?;
    validate_parent_option(&state, payload.primary_parent_id)?;
    let has_primary_photo = has_primary_granted_photo(&state, child_id);

    let child = state
        .children
        .children
        .get_mut(&child_id)
        .ok_or_else(|| ApiError::not_found("child"))?;
    if child.school_id != Some(school_id) {
        return Err(ApiError::forbidden("不能访问其他园所的儿童档案"));
    }

    if let Some(name) = payload.name {
        child.name = required_trimmed(name, "name")?;
    }
    apply_optional_string(&mut child.nickname, payload.nickname);
    if payload.age.is_some() {
        child.age = payload.age;
    }
    apply_optional_string(&mut child.age_group, payload.age_group);
    if payload.classroom_id.is_some() {
        child.classroom_id = payload.classroom_id;
    }
    if payload.primary_parent_id.is_some() {
        child.primary_parent_id = payload.primary_parent_id;
    }
    apply_optional_string(&mut child.gender_expression, payload.gender_expression);
    apply_optional_string(&mut child.hair, payload.hair);
    apply_optional_string(&mut child.skin_tone, payload.skin_tone);
    apply_optional_string(&mut child.usual_outfit, payload.usual_outfit);
    apply_optional_string(&mut child.favorite_color, payload.favorite_color);
    if let Some(tags) = payload.interest_tags {
        child.interest_tags = normalize_tags(tags, "interest_tags")?;
    }
    if let Some(tags) = payload.teacher_observation_tags {
        child.teacher_observation_tags = normalize_tags(tags, "teacher_observation_tags")?;
    }
    apply_optional_string(&mut child.teaching_focus, payload.teaching_focus);
    if let Some(status) = payload.status {
        child.status = status;
    }
    refresh_profile_completion(child, has_primary_photo);
    child.updated_at = now();
    Ok(Json(child.clone()))
}

async fn add_child_photo(
    State(state): State<SharedState>,
    Path(child_id): Path<Uuid>,
    Json(payload): Json<AddChildPhotoRequest>,
) -> Result<Json<ChildPhotoRecord>, ApiError> {
    validate_photo_type(&payload.photo_type)?;
    validate_optional_status(
        Some(payload.consent_status.as_str()),
        &["pending", "granted", "revoked"],
        "consent_status",
    )?;

    let mut state = state.write().expect("state lock poisoned");
    let school_id = state.organization.current_school_id;
    let is_primary = payload.is_primary.unwrap_or(false);
    {
        let child = state
            .children
            .children
            .get(&child_id)
            .ok_or_else(|| ApiError::not_found("child"))?;
        if child.school_id != Some(school_id) {
            return Err(ApiError::forbidden("不能访问其他园所的儿童档案"));
        }
    }
    if is_primary && payload.consent_status == "revoked" {
        return Err(ApiError::validation(
            "consent_status",
            "撤销授权照片不能设为主照片",
        ));
    }
    if is_primary {
        clear_primary_photo(&mut state, child_id);
    }
    let photo = ChildPhotoRecord {
        id: Uuid::new_v4(),
        child_id,
        image_asset_id: payload.image_asset_id,
        photo_type: payload.photo_type,
        is_primary,
        consent_status: payload.consent_status,
        created_at: now(),
    };
    state.children.photos.insert(photo.id, photo.clone());
    let has_primary_photo = has_primary_granted_photo(&state, child_id);
    if let Some(child) = state.children.children.get_mut(&child_id) {
        refresh_profile_completion(child, has_primary_photo);
        child.updated_at = now();
    }
    Ok(Json(photo))
}

async fn update_child_photo(
    State(state): State<SharedState>,
    Path((child_id, photo_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateChildPhotoRequest>,
) -> Result<Json<ChildPhotoRecord>, ApiError> {
    validate_optional_status(
        payload.consent_status.as_deref(),
        &["pending", "granted", "revoked"],
        "consent_status",
    )?;
    let mut state = state.write().expect("state lock poisoned");
    visible_child(&state, child_id)?;
    {
        let photo = state
            .children
            .photos
            .get(&photo_id)
            .ok_or_else(|| ApiError::not_found("photo"))?;
        if photo.child_id != child_id {
            return Err(ApiError::not_found("photo"));
        }
    }
    if payload.is_primary == Some(true) {
        let target_consent = payload.consent_status.as_deref().or_else(|| {
            state
                .children
                .photos
                .get(&photo_id)
                .map(|photo| photo.consent_status.as_str())
        });
        if target_consent == Some("revoked") {
            return Err(ApiError::validation(
                "consent_status",
                "撤销授权照片不能设为主照片",
            ));
        }
        clear_primary_photo(&mut state, child_id);
    }
    let photo = state.children.photos.get_mut(&photo_id).unwrap();
    if let Some(is_primary) = payload.is_primary {
        photo.is_primary = is_primary;
    }
    if let Some(consent_status) = payload.consent_status {
        photo.consent_status = consent_status;
        if photo.consent_status == "revoked" {
            photo.is_primary = false;
        }
    }
    let photo = photo.clone();
    let has_primary_photo = has_primary_granted_photo(&state, child_id);
    if let Some(child) = state.children.children.get_mut(&child_id) {
        refresh_profile_completion(child, has_primary_photo);
        child.updated_at = now();
    }
    Ok(Json(photo))
}

async fn create_parent_intake(
    State(state): State<SharedState>,
    Json(payload): Json<CreateParentIntakeRequest>,
) -> Result<Json<ParentIntakeRecord>, ApiError> {
    let invite_token = required_trimmed(payload.invite_token, "invite_token")?;
    let parent_name = required_trimmed(payload.parent.name, "parent.name")?;
    let child_name = required_trimmed(payload.child.name.clone(), "child.name")?;
    validate_age(payload.child.age)?;
    let child_payload = IntakeChildPayload {
        name: child_name.clone(),
        nickname: payload.child.nickname.and_then(normalize_optional_owned),
        age: payload.child.age,
        age_group: payload.child.age_group.and_then(normalize_optional_owned),
        gender_expression: payload
            .child
            .gender_expression
            .and_then(normalize_optional_owned),
        hair: payload.child.hair.and_then(normalize_optional_owned),
        skin_tone: payload.child.skin_tone.and_then(normalize_optional_owned),
        usual_outfit: payload
            .child
            .usual_outfit
            .and_then(normalize_optional_owned),
        favorite_color: payload
            .child
            .favorite_color
            .and_then(normalize_optional_owned),
        interest_tags: normalize_tags(payload.child.interest_tags, "child.interest_tags")?,
    };

    let intake = ParentIntakeRecord {
        id: Uuid::new_v4(),
        invite_token,
        parent_name,
        relationship_to_child: payload
            .parent
            .relationship_to_child
            .and_then(normalize_optional_owned),
        phone: payload.parent.phone.and_then(normalize_optional_owned),
        email: payload.parent.email.and_then(normalize_optional_owned),
        child_name,
        child_payload,
        parent_character_profile: payload.parent_character_profile,
        photo_asset_ids: payload.photo_asset_ids,
        status: "submitted".to_string(),
        created_at: now(),
        accepted_at: None,
        accepted_child_id: None,
    };
    let mut state = state.write().expect("state lock poisoned");
    state
        .children
        .parent_intakes
        .insert(intake.id, intake.clone());
    Ok(Json(intake))
}

async fn list_parent_intakes(
    State(state): State<SharedState>,
    Query(query): Query<ParentIntakeListQuery>,
) -> Result<Json<ListResponse<ParentIntakeRecord>>, ApiError> {
    validate_optional_status(
        query.status.as_deref(),
        &["submitted", "accepted"],
        "status",
    )?;
    let state = state.read().expect("state lock poisoned");
    let status = query.status.as_deref().unwrap_or("submitted");
    let mut items = state
        .children
        .parent_intakes
        .values()
        .filter(|intake| intake.status == status)
        .cloned()
        .collect::<Vec<_>>();
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(Json(list_response(items)))
}

async fn accept_parent_intake(
    State(state): State<SharedState>,
    Path(intake_id): Path<Uuid>,
) -> Result<Json<AcceptParentIntakeResponse>, ApiError> {
    let mut state = state.write().expect("state lock poisoned");
    let school_id = state.organization.current_school_id;
    let teacher_id = state.organization.current_teacher_id;
    let intake = state
        .children
        .parent_intakes
        .get(&intake_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("parent_intake"))?;
    if intake.status != "submitted" {
        return Err(ApiError::state_conflict("家长提交已处理"));
    }

    let created_at = now();
    let parent_id = Uuid::new_v4();
    let child_id = Uuid::new_v4();
    let parent = ParentRecord {
        id: parent_id,
        name: intake.parent_name.clone(),
        relationship_to_child: intake.relationship_to_child.clone(),
        phone: intake.phone.clone(),
        email: intake.email.clone(),
        status: "active".to_string(),
        created_at,
        updated_at: created_at,
    };
    let mut child = ChildRecord {
        id: child_id,
        school_id: Some(school_id),
        classroom_id: None,
        primary_teacher_id: teacher_id,
        primary_parent_id: Some(parent_id),
        name: intake.child_payload.name.clone(),
        nickname: intake.child_payload.nickname.clone(),
        age: intake.child_payload.age,
        age_group: intake.child_payload.age_group.clone(),
        gender_expression: intake.child_payload.gender_expression.clone(),
        hair: intake.child_payload.hair.clone(),
        skin_tone: intake.child_payload.skin_tone.clone(),
        usual_outfit: intake.child_payload.usual_outfit.clone(),
        favorite_color: intake.child_payload.favorite_color.clone(),
        interest_tags: intake.child_payload.interest_tags.clone(),
        teacher_observation_tags: vec![],
        teaching_focus: None,
        profile_completion_status: "missing_required".to_string(),
        status: "active".to_string(),
        created_at,
        updated_at: created_at,
    };

    let mut first_photo = true;
    for asset_id in &intake.photo_asset_ids {
        let photo = ChildPhotoRecord {
            id: Uuid::new_v4(),
            child_id,
            image_asset_id: *asset_id,
            photo_type: "portrait".to_string(),
            is_primary: first_photo,
            consent_status: "granted".to_string(),
            created_at,
        };
        first_photo = false;
        state.children.photos.insert(photo.id, photo);
    }
    refresh_profile_completion(&mut child, !intake.photo_asset_ids.is_empty());
    state.children.parents.insert(parent_id, parent);
    state.children.children.insert(child_id, child);
    let stored_intake = state.children.parent_intakes.get_mut(&intake_id).unwrap();
    stored_intake.status = "accepted".to_string();
    stored_intake.accepted_at = Some(created_at);
    stored_intake.accepted_child_id = Some(child_id);

    Ok(Json(AcceptParentIntakeResponse {
        intake_id,
        child_id,
        parent_id,
        status: "accepted".to_string(),
    }))
}

fn child_detail(state: &crate::api::AppState, child: ChildRecord) -> ChildDetailResponse {
    let classroom = child.classroom_id.and_then(|classroom_id| {
        state
            .organization
            .classrooms
            .get(&classroom_id)
            .map(|classroom| ClassroomSummary {
                id: classroom.id,
                name: classroom.name.clone(),
            })
    });
    let primary_parent = child
        .primary_parent_id
        .and_then(|parent_id| state.children.parents.get(&parent_id).cloned());
    let mut photos = state
        .children
        .photos
        .values()
        .filter(|photo| photo.child_id == child.id && photo.consent_status != "revoked")
        .cloned()
        .collect::<Vec<_>>();
    photos.sort_by_key(|photo| (!photo.is_primary, photo.created_at));
    ChildDetailResponse {
        child,
        classroom,
        primary_parent,
        photos,
    }
}

fn visible_child(state: &crate::api::AppState, child_id: Uuid) -> Result<&ChildRecord, ApiError> {
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

fn validate_classroom(
    state: &crate::api::AppState,
    classroom_id: Uuid,
    school_id: Uuid,
) -> Result<(), ApiError> {
    let classroom = state
        .organization
        .classrooms
        .get(&classroom_id)
        .ok_or_else(|| ApiError::not_found("classroom"))?;
    if classroom.school_id != school_id || classroom.status != "active" {
        return Err(ApiError::validation(
            "classroom_id",
            "班级必须属于当前园所且为 active",
        ));
    }
    Ok(())
}

fn validate_classroom_option(
    state: &crate::api::AppState,
    classroom_id: Option<Uuid>,
    school_id: Uuid,
) -> Result<(), ApiError> {
    if let Some(classroom_id) = classroom_id {
        validate_classroom(state, classroom_id, school_id)?;
    }
    Ok(())
}

fn validate_parent_option(
    state: &crate::api::AppState,
    parent_id: Option<Uuid>,
) -> Result<(), ApiError> {
    if let Some(parent_id) = parent_id {
        let parent = state
            .children
            .parents
            .get(&parent_id)
            .ok_or_else(|| ApiError::not_found("parent"))?;
        if parent.status != "active" {
            return Err(ApiError::validation(
                "primary_parent_id",
                "家长状态必须为 active",
            ));
        }
    }
    Ok(())
}

fn has_primary_granted_photo(state: &crate::api::AppState, child_id: Uuid) -> bool {
    state.children.photos.values().any(|photo| {
        photo.child_id == child_id && photo.is_primary && photo.consent_status == "granted"
    })
}

fn clear_primary_photo(state: &mut crate::api::AppState, child_id: Uuid) {
    for photo in state.children.photos.values_mut() {
        if photo.child_id == child_id {
            photo.is_primary = false;
        }
    }
}

fn refresh_profile_completion(child: &mut ChildRecord, has_primary_photo: bool) {
    let has_visual_signal = child.hair.is_some()
        || child.usual_outfit.is_some()
        || child.favorite_color.is_some()
        || !child.interest_tags.is_empty()
        || has_primary_photo;
    let has_complete_basics = child.age.is_some()
        && child.age_group.is_some()
        && child.classroom_id.is_some()
        && has_visual_signal
        && (!child.interest_tags.is_empty() || has_primary_photo);
    child.profile_completion_status = if has_complete_basics {
        "complete".to_string()
    } else if has_visual_signal {
        "usable".to_string()
    } else {
        "missing_required".to_string()
    };
}

fn validate_age(age: Option<i32>) -> Result<(), ApiError> {
    if let Some(age) = age {
        if !(0..=8).contains(&age) {
            return Err(ApiError::validation("age", "年龄必须在 0 到 8 之间"));
        }
    }
    Ok(())
}

fn validate_photo_type(photo_type: &str) -> Result<(), ApiError> {
    if ["portrait", "daily", "other"].contains(&photo_type) {
        Ok(())
    } else {
        Err(ApiError::validation("photo_type", "照片类型枚举不合法"))
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
    if value.chars().count() > 40 {
        return Err(ApiError::validation(field, "不能超过 40 字"));
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

fn normalize_tags(tags: Vec<String>, field: &'static str) -> Result<Vec<String>, ApiError> {
    if tags.len() > 20 {
        return Err(ApiError::validation(field, "最多 20 个标签"));
    }
    let mut normalized = Vec::new();
    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty() {
            continue;
        }
        if tag.chars().count() > 20 {
            return Err(ApiError::validation(field, "单个标签不能超过 20 字"));
        }
        if !normalized.iter().any(|existing| existing == tag) {
            normalized.push(tag.to_string());
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

    #[tokio::test]
    async fn lists_children_and_filters_by_keyword() {
        let (status, body) = get_json(test_app(), "/api/children?keyword=乐").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["total"], 1);
        assert_eq!(body["items"][0]["name"], "乐乐");
    }

    #[tokio::test]
    async fn creates_child_and_calculates_profile_status() {
        let (status, body) = request_json(
            test_app(),
            "POST",
            "/api/children",
            json!({
                "name": "小米",
                "age": 4,
                "hair": "黑色短发",
                "interest_tags": ["积木", "积木", "画画"]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], "小米");
        assert_eq!(body["profile_completion_status"], "usable");
        assert_eq!(body["interest_tags"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn rejects_invalid_child_age() {
        let (status, body) = request_json(
            test_app(),
            "POST",
            "/api/children",
            json!({ "name": "小米", "age": 12 }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
        assert_eq!(body["error"]["details"][0]["field"], "age");
    }

    #[tokio::test]
    async fn adds_primary_photo_and_hides_revoked_photo_from_detail() {
        let app = test_app();
        let (_, created) = request_json(
            app.clone(),
            "POST",
            "/api/children",
            json!({ "name": "小米" }),
        )
        .await;
        let child_id = created["id"].as_str().unwrap();
        let (status, photo) = request_json(
            app.clone(),
            "POST",
            &format!("/api/children/{child_id}/photos"),
            json!({
                "image_asset_id": "00000000-0000-0000-0000-000000000088",
                "photo_type": "portrait",
                "is_primary": true,
                "consent_status": "granted"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(photo["is_primary"], true);
        let photo_id = photo["id"].as_str().unwrap();

        let (status, _) = request_json(
            app.clone(),
            "PATCH",
            &format!("/api/children/{child_id}/photos/{photo_id}"),
            json!({ "consent_status": "revoked" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, detail) = get_json(app, &format!("/api/children/{child_id}")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(detail["photos"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn accepts_parent_intake_once() {
        let app = test_app();
        let (status, intake) = request_json(
            app.clone(),
            "POST",
            "/api/parent-intakes",
            json!({
                "invite_token": "invite-demo",
                "parent": {
                    "name": "李女士",
                    "relationship_to_child": "妈妈",
                    "phone": "13900000000"
                },
                "child": {
                    "name": "小雨",
                    "age": 5,
                    "hair": "黑色长发",
                    "interest_tags": ["小兔子"]
                },
                "photo_asset_ids": ["00000000-0000-0000-0000-000000000099"]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(intake["status"], "submitted");
        let intake_id = intake["id"].as_str().unwrap();

        let (status, accepted) = request_json(
            app.clone(),
            "POST",
            &format!("/api/parent-intakes/{intake_id}/accept"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(accepted["status"], "accepted");

        let (status, body) = request_json(
            app,
            "POST",
            &format!("/api/parent-intakes/{intake_id}/accept"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"]["code"], "STATE_CONFLICT");
    }
}
