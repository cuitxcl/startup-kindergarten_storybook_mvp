use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
};
use loco_rs::{app::AppContext, controller::Routes};
use uuid::Uuid;

use crate::{
    application,
    error::ApiError,
    models::{CreateShareLinkRequest, Envelope, ExportJob, ListQuery, ShareLink, Storybook},
};

pub fn routes() -> Routes {
    Routes::new()
        .add(
            "/api/workspaces/{workspace_id}/storybooks/{storybook_id}/exports",
            get(list_exports).post(create_export),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybooks/{storybook_id}/exports/{export_id}",
            get(get_export),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybooks/{storybook_id}/exports/{export_id}/download",
            get(download_workspace_export),
        )
        .add("/exports/{file_name}", get(download_export))
        .add(
            "/api/workspaces/{workspace_id}/storybooks/{storybook_id}/share-links",
            get(list_share_links).post(create_share_link),
        )
        .add(
            "/api/workspaces/{workspace_id}/storybooks/{storybook_id}/share-links/{share_link_id}/revoke",
            post(revoke_share_link),
        )
        .add("/api/share-links/{token}", get(get_share_link))
        .add("/api/share-links/{token}/exports", post(create_share_export))
        .add(
            "/api/share-links/{token}/exports/{export_id}",
            get(get_share_export),
        )
        .add(
            "/api/share-links/{token}/exports/{export_id}/download",
            get(download_share_export),
        )
}

async fn create_export(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, storybook_id)): Path<(Uuid, Uuid)>,
) -> Result<(StatusCode, Json<Envelope<ExportJob>>), ApiError> {
    let job =
        application::delivery::create_export(&ctx, &headers, workspace_id, storybook_id).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(job))))
}

async fn list_exports(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, storybook_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Envelope<Vec<ExportJob>>>, ApiError> {
    let (jobs, meta) =
        application::delivery::list_exports(&ctx, &headers, workspace_id, storybook_id, query)
            .await?;
    Ok(Json(Envelope::with_meta(jobs, meta)))
}

async fn get_export(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, storybook_id, export_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<Envelope<ExportJob>>, ApiError> {
    let job =
        application::delivery::get_export(&ctx, &headers, workspace_id, storybook_id, export_id)
            .await?;
    Ok(Json(Envelope::new(job)))
}

async fn download_workspace_export(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, storybook_id, export_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Response, ApiError> {
    let (file_name, bytes) = application::delivery::workspace_export_file(
        &ctx,
        &headers,
        workspace_id,
        storybook_id,
        export_id,
    )
    .await?;
    pdf_response(&file_name, bytes)
}

async fn download_export(Path(file_name): Path<String>) -> Result<Response, ApiError> {
    let (file_name, bytes) = application::delivery::public_export_file(&file_name)?;
    pdf_response(&file_name, bytes)
}

async fn create_share_link(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, storybook_id)): Path<(Uuid, Uuid)>,
    payload: Option<Json<CreateShareLinkRequest>>,
) -> Result<(StatusCode, Json<Envelope<ShareLink>>), ApiError> {
    let payload = payload.map(|Json(payload)| payload).unwrap_or_default();
    let link = application::delivery::create_share_link(
        &ctx,
        &headers,
        workspace_id,
        storybook_id,
        payload,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(link))))
}

async fn list_share_links(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, storybook_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Envelope<Vec<ShareLink>>>, ApiError> {
    let (links, meta) =
        application::delivery::list_share_links(&ctx, &headers, workspace_id, storybook_id, query)
            .await?;
    Ok(Json(Envelope::with_meta(links, meta)))
}

async fn revoke_share_link(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, storybook_id, share_link_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<Envelope<ShareLink>>, ApiError> {
    let link = application::delivery::revoke_share_link(
        &ctx,
        &headers,
        workspace_id,
        storybook_id,
        share_link_id,
    )
    .await?;
    Ok(Json(Envelope::new(link)))
}

async fn get_share_link(
    State(ctx): State<AppContext>,
    Path(token): Path<String>,
) -> Result<Json<Envelope<Storybook>>, ApiError> {
    let book = application::delivery::get_public_share(&ctx, token).await?;
    Ok(Json(Envelope::new(book)))
}

async fn create_share_export(
    State(ctx): State<AppContext>,
    Path(token): Path<String>,
) -> Result<(StatusCode, Json<Envelope<ExportJob>>), ApiError> {
    let job = application::delivery::create_public_export(&ctx, token).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(job))))
}

async fn get_share_export(
    State(ctx): State<AppContext>,
    Path((token, export_id)): Path<(String, Uuid)>,
) -> Result<Json<Envelope<ExportJob>>, ApiError> {
    let job = application::delivery::get_public_export(&ctx, token, export_id).await?;
    Ok(Json(Envelope::new(job)))
}

async fn download_share_export(
    State(ctx): State<AppContext>,
    Path((token, export_id)): Path<(String, Uuid)>,
) -> Result<Response, ApiError> {
    let (file_name, bytes) =
        application::delivery::public_share_export_file(&ctx, token, export_id).await?;
    pdf_response(&file_name, bytes)
}

fn pdf_response(file_name: &str, bytes: Vec<u8>) -> Result<Response, ApiError> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "application/pdf")
        .header(
            axum::http::header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{file_name}\""),
        )
        .body(Body::from(bytes))
        .map_err(|_| ApiError::state_conflict("无法返回导出文件"))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_include_delivery_api() {
        let registered_routes = routes();
        let uris = registered_routes
            .handlers
            .iter()
            .map(|handler| handler.uri.as_str())
            .collect::<Vec<_>>();

        assert!(uris.contains(&"/api/workspaces/{workspace_id}/storybooks/{storybook_id}/exports"));
        assert!(uris.contains(
            &"/api/workspaces/{workspace_id}/storybooks/{storybook_id}/exports/{export_id}"
        ));
        assert!(uris.contains(
            &"/api/workspaces/{workspace_id}/storybooks/{storybook_id}/exports/{export_id}/download"
        ));
        assert!(uris.contains(&"/exports/{file_name}"));
        assert!(
            uris.contains(&"/api/workspaces/{workspace_id}/storybooks/{storybook_id}/share-links")
        );
        assert!(uris.contains(
            &"/api/workspaces/{workspace_id}/storybooks/{storybook_id}/share-links/{share_link_id}/revoke"
        ));
        assert!(uris.contains(&"/api/share-links/{token}"));
        assert!(uris.contains(&"/api/share-links/{token}/exports"));
        assert!(uris.contains(&"/api/share-links/{token}/exports/{export_id}"));
        assert!(uris.contains(&"/api/share-links/{token}/exports/{export_id}/download"));
    }
}
