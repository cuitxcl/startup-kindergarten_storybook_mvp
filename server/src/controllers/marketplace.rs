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
    models::{Envelope, MarketplaceQuery, MarketplaceTemplate, Storybook},
};

pub fn routes() -> Routes {
    Routes::new()
        .add("/api/marketplace/templates", get(list_templates))
        .add(
            "/api/marketplace/templates/{template_id}",
            get(get_template),
        )
        .add(
            "/api/workspaces/{workspace_id}/marketplace/templates/{template_id}/copy",
            post(copy_template),
        )
}

async fn list_templates(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Query(query): Query<MarketplaceQuery>,
) -> Result<Json<Envelope<Vec<MarketplaceTemplate>>>, ApiError> {
    let (templates, meta) = application::marketplace::list_templates(&ctx, &headers, query).await?;
    Ok(Json(Envelope::with_meta(templates, meta)))
}

async fn get_template(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(template_id): Path<Uuid>,
) -> Result<Json<Envelope<MarketplaceTemplate>>, ApiError> {
    let template = application::marketplace::get_template(&ctx, &headers, template_id).await?;
    Ok(Json(Envelope::new(template)))
}

async fn copy_template(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, template_id)): Path<(Uuid, Uuid)>,
) -> Result<(StatusCode, Json<Envelope<Storybook>>), ApiError> {
    let book =
        application::marketplace::copy_template(&ctx, &headers, workspace_id, template_id).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(book))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_include_marketplace_api() {
        let registered_routes = routes();
        let uris = registered_routes
            .handlers
            .iter()
            .map(|handler| handler.uri.as_str())
            .collect::<Vec<_>>();

        assert!(uris.contains(&"/api/marketplace/templates"));
        assert!(uris.contains(&"/api/marketplace/templates/{template_id}"));
        assert!(
            uris.contains(
                &"/api/workspaces/{workspace_id}/marketplace/templates/{template_id}/copy"
            )
        );
    }
}
