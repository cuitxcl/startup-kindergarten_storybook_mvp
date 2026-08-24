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
        BuildCustomizationPlanRequest, CreateStorybookRequest, DeriveCustomBatchRequest,
        DeriveCustomBatchResponse, DeriveCustomRequest, DuplicateStorybookRequest, Envelope,
        Storybook, StorybookCustomizationRun, StorybookListQuery, StorybookPage, StorybookRole,
        UpdatePageRequest, UpdateRoleRequest, UpdateStorybookRequest,
    },
};

pub fn routes() -> Routes {
    Routes::new()
        .add(
            "/api/workspaces/{workspace_id}/storybooks",
            get(list_storybooks).post(create_storybook),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybooks/{storybook_id}",
            get(get_storybook)
                .patch(update_storybook)
                .delete(delete_storybook),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybooks/{storybook_id}/duplicate",
            post(duplicate_storybook),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybooks/{storybook_id}/pages/{page_id}",
            axum::routing::patch(update_page),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybooks/{storybook_id}/roles/{role_id}",
            axum::routing::patch(update_role),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybooks/{storybook_id}/derive-custom",
            post(derive_custom),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybooks/{storybook_id}/customization-plan",
            post(build_customization_plan),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybooks/{storybook_id}/derive-custom-batch",
            post(derive_custom_batch),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-customization-runs/{run_id}",
            get(get_customization_run),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-customization-runs/{run_id}/cancel",
            post(cancel_customization_run),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-customization-runs/{run_id}/items/{item_id}/retry",
            post(retry_customization_run_item),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybook-customization-runs/{run_id}/items/{item_id}/abandon",
            post(abandon_customization_run_item),
        )
}

async fn list_storybooks(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Query(query): Query<StorybookListQuery>,
) -> Result<Json<Envelope<Vec<Storybook>>>, ApiError> {
    let (storybooks, meta) =
        application::storybooks::list(&ctx, &headers, workspace_id, query).await?;
    Ok(Json(Envelope::with_meta(storybooks, meta)))
}

async fn create_storybook(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Json(payload): Json<CreateStorybookRequest>,
) -> Result<(StatusCode, Json<Envelope<Storybook>>), ApiError> {
    let book = application::storybooks::create(&ctx, &headers, workspace_id, payload).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(book))))
}

async fn get_storybook(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, storybook_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Envelope<Storybook>>, ApiError> {
    let book = application::storybooks::get(&ctx, &headers, workspace_id, storybook_id).await?;
    Ok(Json(Envelope::new(book)))
}

async fn update_storybook(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, storybook_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateStorybookRequest>,
) -> Result<Json<Envelope<Storybook>>, ApiError> {
    let book = application::storybooks::update(&ctx, &headers, workspace_id, storybook_id, payload)
        .await?;
    Ok(Json(Envelope::new(book)))
}

async fn delete_storybook(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, storybook_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    application::storybooks::delete(&ctx, &headers, workspace_id, storybook_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn duplicate_storybook(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, storybook_id)): Path<(Uuid, Uuid)>,
    payload: Option<Json<DuplicateStorybookRequest>>,
) -> Result<(StatusCode, Json<Envelope<Storybook>>), ApiError> {
    let book = application::storybooks::duplicate(
        &ctx,
        &headers,
        workspace_id,
        storybook_id,
        payload.map(|Json(payload)| payload).unwrap_or_default(),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(book))))
}

async fn update_page(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, storybook_id, page_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(payload): Json<UpdatePageRequest>,
) -> Result<Json<Envelope<StorybookPage>>, ApiError> {
    let page = application::storybooks::update_page(
        &ctx,
        &headers,
        workspace_id,
        storybook_id,
        page_id,
        payload,
    )
    .await?;
    Ok(Json(Envelope::new(page)))
}

async fn update_role(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, storybook_id, role_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(payload): Json<UpdateRoleRequest>,
) -> Result<Json<Envelope<StorybookRole>>, ApiError> {
    let role = application::storybooks::update_role(
        &ctx,
        &headers,
        workspace_id,
        storybook_id,
        role_id,
        payload,
    )
    .await?;
    Ok(Json(Envelope::new(role)))
}

async fn derive_custom(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, storybook_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<DeriveCustomRequest>,
) -> Result<(StatusCode, Json<Envelope<Storybook>>), ApiError> {
    let book =
        application::storybooks::derive_custom(&ctx, &headers, workspace_id, storybook_id, payload)
            .await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(book))))
}

async fn build_customization_plan(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, storybook_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<BuildCustomizationPlanRequest>,
) -> Result<Json<Envelope<serde_json::Value>>, ApiError> {
    let plan = application::storybook_customization::build_customization_plan(
        &ctx,
        &headers,
        workspace_id,
        storybook_id,
        payload,
    )
    .await?;
    Ok(Json(Envelope::new(plan)))
}

async fn derive_custom_batch(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, storybook_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<DeriveCustomBatchRequest>,
) -> Result<(StatusCode, Json<Envelope<DeriveCustomBatchResponse>>), ApiError> {
    let response = application::storybooks::derive_custom_batch(
        &ctx,
        &headers,
        workspace_id,
        storybook_id,
        payload,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(response))))
}

async fn get_customization_run(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, run_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Envelope<StorybookCustomizationRun>>, ApiError> {
    let run = application::storybook_customization::get_customization_run(
        &ctx,
        &headers,
        workspace_id,
        run_id,
    )
    .await?;
    Ok(Json(Envelope::new(run)))
}

async fn cancel_customization_run(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, run_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Envelope<StorybookCustomizationRun>>, ApiError> {
    let run = application::storybook_customization::cancel_customization_run(
        &ctx,
        &headers,
        workspace_id,
        run_id,
    )
    .await?;
    Ok(Json(Envelope::new(run)))
}

async fn retry_customization_run_item(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, run_id, item_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<Envelope<StorybookCustomizationRun>>, ApiError> {
    let run = application::storybook_customization::retry_customization_run_item(
        &ctx,
        &headers,
        workspace_id,
        run_id,
        item_id,
    )
    .await?;
    Ok(Json(Envelope::new(run)))
}

async fn abandon_customization_run_item(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, run_id, item_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<Envelope<StorybookCustomizationRun>>, ApiError> {
    let run = application::storybook_customization::abandon_customization_run_item(
        &ctx,
        &headers,
        workspace_id,
        run_id,
        item_id,
    )
    .await?;
    Ok(Json(Envelope::new(run)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_include_storybooks_api() {
        let registered_routes = routes();
        let uris = registered_routes
            .handlers
            .iter()
            .map(|handler| handler.uri.as_str())
            .collect::<Vec<_>>();

        assert!(uris.contains(&"/api/workspaces/{workspace_id}/storybooks"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/storybooks/{storybook_id}"));
        assert!(
            uris.contains(&"/api/workspaces/{workspace_id}/storybooks/{storybook_id}/duplicate")
        );
        assert!(
            uris.contains(
                &"/api/workspaces/{workspace_id}/storybooks/{storybook_id}/pages/{page_id}"
            )
        );
        assert!(
            uris.contains(
                &"/api/workspaces/{workspace_id}/storybooks/{storybook_id}/roles/{role_id}"
            )
        );
        assert!(
            uris.contains(
                &"/api/workspaces/{workspace_id}/storybooks/{storybook_id}/derive-custom"
            )
        );
        assert!(uris.contains(
            &"/api/workspaces/{workspace_id}/storybooks/{storybook_id}/customization-plan"
        ));
        assert!(uris.contains(
            &"/api/workspaces/{workspace_id}/storybooks/{storybook_id}/derive-custom-batch"
        ));
        assert!(
            uris.contains(&"/api/workspaces/{workspace_id}/storybook-customization-runs/{run_id}")
        );
        assert!(uris.contains(
            &"/api/workspaces/{workspace_id}/storybook-customization-runs/{run_id}/items/{item_id}/retry"
        ));
        assert!(uris.contains(
            &"/api/workspaces/{workspace_id}/storybook-customization-runs/{run_id}/cancel"
        ));
        assert!(uris.contains(
            &"/api/workspaces/{workspace_id}/storybook-customization-runs/{run_id}/items/{item_id}/abandon"
        ));
    }
}
