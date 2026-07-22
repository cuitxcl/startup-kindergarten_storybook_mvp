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
        ActionResponse, ChildProfile, ConfirmParentIntakeRequest, CreateParentIntakeLinkRequest,
        Envelope, ParentIntake, ParentIntakeLink, ParentIntakeLinkBulkActionQuery,
        ParentIntakeLinkListQuery, ParentIntakeListQuery, ParentIntakeRequest,
        PublicParentIntakeLink,
    },
};

pub fn routes() -> Routes {
    Routes::new()
        .add("/api/parent-intake-links/{token}", get(get_public_link))
        .add("/api/parent-intakes", post(submit_parent_intake))
        .add(
            "/api/workspaces/{workspace_id}/parent-intakes",
            get(list_parent_intakes),
        )
        .add(
            "/api/workspaces/{workspace_id}/parent-intake-links",
            get(list_parent_intake_links).post(create_parent_intake_link),
        )
        .add(
            "/api/workspaces/{workspace_id}/parent-intake-links/revoke-active",
            post(revoke_active_parent_intake_links),
        )
        .add(
            "/api/workspaces/{workspace_id}/parent-intake-links/{link_id}/revoke",
            post(revoke_parent_intake_link),
        )
        .add(
            "/api/workspaces/{workspace_id}/parent-intakes/{intake_id}/confirm",
            post(confirm_parent_intake),
        )
}

async fn list_parent_intakes(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Query(query): Query<ParentIntakeListQuery>,
) -> Result<Json<Envelope<Vec<ParentIntake>>>, ApiError> {
    let (intakes, meta) =
        application::parent_intakes::list_intakes(&ctx, &headers, workspace_id, query).await?;
    Ok(Json(Envelope::with_meta(intakes, meta)))
}

async fn list_parent_intake_links(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Query(query): Query<ParentIntakeLinkListQuery>,
) -> Result<Json<Envelope<Vec<ParentIntakeLink>>>, ApiError> {
    let (links, meta) =
        application::parent_intakes::list_links(&ctx, &headers, workspace_id, query).await?;
    Ok(Json(Envelope::with_meta(links, meta)))
}

async fn create_parent_intake_link(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Json(payload): Json<CreateParentIntakeLinkRequest>,
) -> Result<(StatusCode, Json<Envelope<ParentIntakeLink>>), ApiError> {
    let link =
        application::parent_intakes::create_link(&ctx, &headers, workspace_id, payload).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(link))))
}

async fn revoke_parent_intake_link(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, link_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Envelope<ParentIntakeLink>>, ApiError> {
    let link =
        application::parent_intakes::revoke_link(&ctx, &headers, workspace_id, link_id).await?;
    Ok(Json(Envelope::new(link)))
}

async fn revoke_active_parent_intake_links(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Query(query): Query<ParentIntakeLinkBulkActionQuery>,
) -> Result<Json<Envelope<ActionResponse>>, ApiError> {
    let response =
        application::parent_intakes::revoke_active_links(&ctx, &headers, workspace_id, query)
            .await?;
    Ok(Json(Envelope::new(response)))
}

async fn get_public_link(
    State(ctx): State<AppContext>,
    Path(token): Path<String>,
) -> Result<Json<Envelope<PublicParentIntakeLink>>, ApiError> {
    let link = application::parent_intakes::get_public_link(&ctx, token).await?;
    Ok(Json(Envelope::new(link)))
}

async fn confirm_parent_intake(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, intake_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<ConfirmParentIntakeRequest>,
) -> Result<(StatusCode, Json<Envelope<ChildProfile>>), ApiError> {
    let child =
        application::parent_intakes::confirm(&ctx, &headers, workspace_id, intake_id, payload)
            .await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(child))))
}

async fn submit_parent_intake(
    State(ctx): State<AppContext>,
    Json(payload): Json<ParentIntakeRequest>,
) -> Result<(StatusCode, Json<Envelope<ActionResponse>>), ApiError> {
    let response = application::parent_intakes::submit(&ctx, payload).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(response))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_include_parent_intake_api() {
        let registered_routes = routes();
        let uris = registered_routes
            .handlers
            .iter()
            .map(|handler| handler.uri.as_str())
            .collect::<Vec<_>>();

        assert!(uris.contains(&"/api/parent-intake-links/{token}"));
        assert!(uris.contains(&"/api/parent-intakes"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/parent-intakes"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/parent-intake-links"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/parent-intake-links/revoke-active"));
        assert!(
            uris.contains(&"/api/workspaces/{workspace_id}/parent-intakes/{intake_id}/confirm")
        );
    }
}
