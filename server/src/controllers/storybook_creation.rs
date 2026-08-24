use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use loco_rs::{app::AppContext, controller::Routes};
use uuid::Uuid;

use crate::{
    application,
    error::ApiError,
    models::{
        CreateStorybookCreationSessionRequest, CreationDirectionsResponse,
        CreationMaterialsResponse, CreationOutlineResponse, CreationSessionUpdateResponse,
        CreationStorybookGenerationResponse, Envelope, GenerateCreationStorybookRequest,
        GenerateDirectionsRequest, GenerateOutlineRequest, PatchCreationMaterialsRequest,
        RefreshUnderstandingRequest, ResponseWarning, SelectDirectionRequest,
        SelectDirectionResponse, StorybookCreationSession, StorybookCreationSessionListItem,
        StorybookCreationSessionListQuery, UpdateCreationOutlineRequest, UpdateOutlinePageRequest,
        UpdateOutlinePageResponse, UpdateOutlineResponse, UpdateStorybookCreationSessionRequest,
        UpdateVisualPreferencesRequest, VisualPreferencesResponse,
    },
};

pub fn routes() -> Routes {
    Routes::new()
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions",
            get(list_sessions).post(create_session),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions/latest",
            get(latest_session),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}",
            get(get_session).patch(update_session),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/understanding:refresh",
            post(refresh_understanding),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/materials",
            axum::routing::patch(patch_materials),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/directions:generate",
            post(generate_directions),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/direction",
            post(select_direction),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/outline:generate",
            post(generate_outline),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/outline/pages/{page_number}",
            axum::routing::patch(update_outline_page),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/outline",
            axum::routing::patch(update_outline),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/visual-preferences",
            axum::routing::patch(update_visual_preferences),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/storybook:generate",
            post(generate_storybook),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/abandon",
            post(abandon_session),
        )
}

async fn create_session(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Json(payload): Json<CreateStorybookCreationSessionRequest>,
) -> Result<(StatusCode, Json<Envelope<StorybookCreationSession>>), ApiError> {
    let session =
        application::storybook_creation::create(&ctx, &headers, workspace_id, payload).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(session))))
}

async fn list_sessions(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Query(query): Query<StorybookCreationSessionListQuery>,
) -> Result<Json<Envelope<Vec<StorybookCreationSessionListItem>>>, ApiError> {
    let (sessions, meta) =
        application::storybook_creation::list(&ctx, &headers, workspace_id, query).await?;
    Ok(Json(Envelope::with_meta(sessions, meta)))
}

async fn latest_session(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Query(query): Query<StorybookCreationSessionListQuery>,
) -> Result<Json<Envelope<Option<StorybookCreationSession>>>, ApiError> {
    let session =
        application::storybook_creation::latest(&ctx, &headers, workspace_id, query).await?;
    Ok(Json(Envelope::new(session)))
}

async fn get_session(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Envelope<StorybookCreationSession>>, ApiError> {
    let session =
        application::storybook_creation::get(&ctx, &headers, workspace_id, session_id).await?;
    Ok(Json(Envelope::new(session)))
}

async fn update_session(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateStorybookCreationSessionRequest>,
) -> Result<Json<Envelope<CreationSessionUpdateResponse>>, ApiError> {
    let response =
        application::storybook_creation::update(&ctx, &headers, workspace_id, session_id, payload)
            .await?;
    Ok(Json(Envelope::new(response)))
}

async fn refresh_understanding(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<RefreshUnderstandingRequest>,
) -> Result<Json<Envelope<StorybookCreationSession>>, ApiError> {
    let session = application::storybook_creation::refresh_understanding(
        &ctx,
        &headers,
        workspace_id,
        session_id,
        payload,
    )
    .await?;
    Ok(Json(Envelope::new(session)))
}

async fn patch_materials(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<PatchCreationMaterialsRequest>,
) -> Result<Json<Envelope<CreationMaterialsResponse>>, ApiError> {
    let response = application::storybook_creation::patch_materials(
        &ctx,
        &headers,
        workspace_id,
        session_id,
        payload,
    )
    .await?;
    let warnings = asset_reference_warnings(&ctx, workspace_id, session_id).await?;
    Ok(Json(Envelope::with_warnings(response, warnings)))
}

async fn generate_directions(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<GenerateDirectionsRequest>,
) -> Result<Json<Envelope<CreationDirectionsResponse>>, ApiError> {
    let response = application::storybook_creation::generate_directions(
        &ctx,
        &headers,
        workspace_id,
        session_id,
        payload,
    )
    .await?;
    let warnings = asset_reference_warnings(&ctx, workspace_id, session_id).await?;
    Ok(Json(Envelope::with_warnings(response, warnings)))
}

async fn select_direction(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<SelectDirectionRequest>,
) -> Result<Json<Envelope<SelectDirectionResponse>>, ApiError> {
    let response = application::storybook_creation::select_direction(
        &ctx,
        &headers,
        workspace_id,
        session_id,
        payload,
    )
    .await?;
    Ok(Json(Envelope::new(response)))
}

async fn generate_outline(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<GenerateOutlineRequest>,
) -> Result<Json<Envelope<CreationOutlineResponse>>, ApiError> {
    let response = application::storybook_creation::generate_outline(
        &ctx,
        &headers,
        workspace_id,
        session_id,
        payload,
    )
    .await?;
    let warnings = asset_reference_warnings(&ctx, workspace_id, session_id).await?;
    Ok(Json(Envelope::with_warnings(response, warnings)))
}

async fn asset_reference_warnings(
    ctx: &AppContext,
    workspace_id: Uuid,
    session_id: Uuid,
) -> Result<Vec<ResponseWarning>, ApiError> {
    let references =
        crate::repositories::storybook_creation_assets::blocking_references_for_generation(
            &ctx.db,
            workspace_id,
            session_id,
        )
        .await
        .map_err(crate::domains::common::db_error)?;
    if references.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![ResponseWarning {
        code: "visual_reference_pending".to_string(),
        message: format!(
            "{} 张照片还没有确认同画风参考，开始制作前需要处理。",
            references.len()
        ),
        asset_reference_ids: references
            .into_iter()
            .map(|reference| reference.id)
            .collect(),
        next_action: Some("confirm_visual_reference".to_string()),
    }])
}

async fn update_outline_page(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id, page_number)): Path<(Uuid, Uuid, u32)>,
    Json(payload): Json<UpdateOutlinePageRequest>,
) -> Result<Json<Envelope<UpdateOutlinePageResponse>>, ApiError> {
    let response = application::storybook_creation::update_outline_page(
        &ctx,
        &headers,
        workspace_id,
        session_id,
        page_number,
        payload,
    )
    .await?;
    Ok(Json(Envelope::new(response)))
}

async fn update_outline(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateCreationOutlineRequest>,
) -> Result<Json<Envelope<UpdateOutlineResponse>>, ApiError> {
    let response = application::storybook_creation::update_outline(
        &ctx,
        &headers,
        workspace_id,
        session_id,
        payload,
    )
    .await?;
    Ok(Json(Envelope::new(response)))
}

async fn update_visual_preferences(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateVisualPreferencesRequest>,
) -> Result<Json<Envelope<VisualPreferencesResponse>>, ApiError> {
    let response = application::storybook_creation::update_visual_preferences(
        &ctx,
        &headers,
        workspace_id,
        session_id,
        payload,
    )
    .await?;
    Ok(Json(Envelope::new(response)))
}

async fn generate_storybook(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<GenerateCreationStorybookRequest>,
) -> Result<Json<Envelope<CreationStorybookGenerationResponse>>, ApiError> {
    let response = application::storybook_creation::generate_storybook(
        &ctx,
        &headers,
        workspace_id,
        session_id,
        payload,
    )
    .await?;
    Ok(Json(Envelope::new(response)))
}

async fn abandon_session(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, session_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Envelope<CreationSessionUpdateResponse>>, ApiError> {
    let response =
        application::storybook_creation::abandon(&ctx, &headers, workspace_id, session_id).await?;
    Ok(Json(Envelope::new(response)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_include_storybook_creation_api() {
        let registered_routes = routes();
        let uris = registered_routes
            .handlers
            .iter()
            .map(|handler| handler.uri.as_str())
            .collect::<Vec<_>>();

        assert!(uris.contains(&"/api/workspaces/{workspace_id}/storybook-creation-sessions"));
        assert!(
            uris.contains(&"/api/workspaces/{workspace_id}/storybook-creation-sessions/latest")
        );
        assert!(
            uris.contains(
                &"/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}"
            )
        );
        assert!(uris.contains(
            &"/api/workspaces/{workspace_id}/storybook-creation-sessions/{session_id}/storybook:generate"
        ));
    }
}
