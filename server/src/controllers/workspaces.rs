use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
    routing::get,
};
use loco_rs::{app::AppContext, controller::Routes};
use uuid::Uuid;

use crate::{
    application,
    error::ApiError,
    models::{DashboardResponse, Envelope, Workspace},
    services::generation_provider::GenerationProviderSummary,
};

pub fn routes() -> Routes {
    Routes::new()
        .add("/api/workspaces", get(workspaces))
        .add("/api/workspaces/{workspace_id}/dashboard", get(dashboard))
        .add(
            "/api/workspaces/{workspace_id}/generation-provider",
            get(get_workspace_generation_provider),
        )
}

async fn workspaces(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> Result<Json<Envelope<Vec<Workspace>>>, ApiError> {
    let workspaces = application::workspaces::list(&ctx, &headers).await?;
    Ok(Json(Envelope::new(workspaces)))
}

async fn dashboard(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Envelope<DashboardResponse>>, ApiError> {
    let dashboard = application::workspaces::dashboard(&ctx, &headers, workspace_id).await?;
    Ok(Json(Envelope::new(dashboard)))
}

async fn get_workspace_generation_provider(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Envelope<GenerationProviderSummary>>, ApiError> {
    let provider =
        application::workspaces::generation_provider(&ctx, &headers, workspace_id).await?;
    Ok(Json(Envelope::new(provider)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_include_workspace_api() {
        let registered_routes = routes();
        let uris = registered_routes
            .handlers
            .iter()
            .map(|handler| handler.uri.as_str())
            .collect::<Vec<_>>();

        assert!(uris.contains(&"/api/workspaces"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/dashboard"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/generation-provider"));
    }
}
