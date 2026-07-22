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
    models::{ChildProfile, CreateChildRequest, Envelope, ListQuery, UpdateChildRequest},
};

pub fn routes() -> Routes {
    Routes::new()
        .add(
            "/api/workspaces/{workspace_id}/children",
            get(list_children).post(create_child),
        )
        .add(
            "/api/workspaces/{workspace_id}/children/{child_id}",
            get(get_child).patch(update_child),
        )
        .add(
            "/api/workspaces/{workspace_id}/children/{child_id}/archive",
            post(archive_child),
        )
        .add(
            "/api/workspaces/{workspace_id}/children/{child_id}/restore",
            post(restore_child),
        )
}

async fn list_children(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Envelope<Vec<ChildProfile>>>, ApiError> {
    let (children, meta) = application::children::list(&ctx, &headers, workspace_id, query).await?;
    Ok(Json(Envelope::with_meta(children, meta)))
}

async fn create_child(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Json(payload): Json<CreateChildRequest>,
) -> Result<(StatusCode, Json<Envelope<ChildProfile>>), ApiError> {
    let child = application::children::create(&ctx, &headers, workspace_id, payload).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(child))))
}

async fn get_child(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, child_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Envelope<ChildProfile>>, ApiError> {
    let child = application::children::get(&ctx, &headers, workspace_id, child_id).await?;
    Ok(Json(Envelope::new(child)))
}

async fn update_child(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, child_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateChildRequest>,
) -> Result<Json<Envelope<ChildProfile>>, ApiError> {
    let child =
        application::children::update(&ctx, &headers, workspace_id, child_id, payload).await?;
    Ok(Json(Envelope::new(child)))
}

async fn archive_child(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, child_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Envelope<ChildProfile>>, ApiError> {
    let child = application::children::archive(&ctx, &headers, workspace_id, child_id).await?;
    Ok(Json(Envelope::new(child)))
}

async fn restore_child(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, child_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Envelope<ChildProfile>>, ApiError> {
    let child = application::children::restore(&ctx, &headers, workspace_id, child_id).await?;
    Ok(Json(Envelope::new(child)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_include_children_api() {
        let registered_routes = routes();
        let uris = registered_routes
            .handlers
            .iter()
            .map(|handler| handler.uri.as_str())
            .collect::<Vec<_>>();

        assert!(uris.contains(&"/api/workspaces/{workspace_id}/children"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/children/{child_id}"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/children/{child_id}/archive"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/children/{child_id}/restore"));
    }
}
