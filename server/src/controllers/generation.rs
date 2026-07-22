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
    application::{self, generation::RecoverGenerationJobsRequest},
    error::ApiError,
    models::{
        CreateGenerationJobRequest, CreateImageTaskRequest, Envelope, GenerationJob,
        GenerationJobListQuery,
    },
};

pub fn routes() -> Routes {
    Routes::new()
        .add(
            "/api/workspaces/{workspace_id}/storybooks/{storybook_id}/pages/{page_id}/image-tasks",
            post(create_page_image_task),
        )
        .add(
            "/api/workspaces/{workspace_id}/generation-jobs",
            get(list_generation_jobs).post(create_generation_job),
        )
        .add(
            "/api/workspaces/{workspace_id}/generation-jobs/{job_id}",
            get(get_generation_job),
        )
        .add(
            "/api/workspaces/{workspace_id}/generation-jobs/{job_id}/image",
            get(download_generation_image),
        )
        .add(
            "/api/workspaces/{workspace_id}/generation-jobs/{job_id}/retry",
            post(retry_generation_job),
        )
        .add(
            "/api/workspaces/{workspace_id}/generation-jobs/{job_id}/cancel",
            post(cancel_generation_job),
        )
        .add(
            "/api/workspaces/{workspace_id}/generation-jobs/recover",
            post(recover_generation_jobs),
        )
        .add(
            "/generated-images/{file_name}",
            get(download_generated_image),
        )
}

async fn create_page_image_task(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, storybook_id, page_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(payload): Json<CreateImageTaskRequest>,
) -> Result<(StatusCode, Json<Envelope<GenerationJob>>), ApiError> {
    let job = application::generation::create_page_image_task(
        &ctx,
        &headers,
        workspace_id,
        storybook_id,
        page_id,
        payload,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(job))))
}

async fn create_generation_job(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Json(payload): Json<CreateGenerationJobRequest>,
) -> Result<(StatusCode, Json<Envelope<GenerationJob>>), ApiError> {
    let job = application::generation::create_job(&ctx, &headers, workspace_id, payload).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(job))))
}

async fn list_generation_jobs(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Query(query): Query<GenerationJobListQuery>,
) -> Result<Json<Envelope<Vec<GenerationJob>>>, ApiError> {
    let (jobs, meta) =
        application::generation::list_jobs(&ctx, &headers, workspace_id, query).await?;
    Ok(Json(Envelope::with_meta(jobs, meta)))
}

async fn get_generation_job(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Envelope<GenerationJob>>, ApiError> {
    let job = application::generation::get_job(&ctx, &headers, workspace_id, job_id).await?;
    Ok(Json(Envelope::new(job)))
}

async fn retry_generation_job(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Envelope<GenerationJob>>, ApiError> {
    let job = application::generation::retry_job(&ctx, &headers, workspace_id, job_id).await?;
    Ok(Json(Envelope::new(job)))
}

async fn cancel_generation_job(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Envelope<GenerationJob>>, ApiError> {
    let job = application::generation::cancel_job(&ctx, &headers, workspace_id, job_id).await?;
    Ok(Json(Envelope::new(job)))
}

async fn recover_generation_jobs(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Json(payload): Json<RecoverGenerationJobsRequest>,
) -> Result<Json<Envelope<serde_json::Value>>, ApiError> {
    let result =
        application::generation::recover_jobs(&ctx, &headers, workspace_id, payload).await?;
    Ok(Json(Envelope::new(result)))
}

async fn download_generation_image(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, ApiError> {
    let (file_name, bytes) =
        application::generation::generation_image_file(&ctx, &headers, workspace_id, job_id)
            .await?;
    image_response(&file_name, bytes)
}

async fn download_generated_image(Path(file_name): Path<String>) -> Result<Response, ApiError> {
    let (file_name, bytes) = application::generation::public_generated_image_file(&file_name)?;
    image_response(&file_name, bytes)
}

fn image_response(file_name: &str, bytes: Vec<u8>) -> Result<Response, ApiError> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "image/png")
        .header(
            axum::http::header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{file_name}\""),
        )
        .body(Body::from(bytes))
        .map_err(|_| ApiError::state_conflict("无法返回图片文件"))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_include_generation_api() {
        let registered_routes = routes();
        let uris = registered_routes
            .handlers
            .iter()
            .map(|handler| handler.uri.as_str())
            .collect::<Vec<_>>();

        assert!(uris.contains(
            &"/api/workspaces/{workspace_id}/storybooks/{storybook_id}/pages/{page_id}/image-tasks"
        ));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/generation-jobs"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/generation-jobs/{job_id}"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/generation-jobs/{job_id}/image"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/generation-jobs/{job_id}/retry"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/generation-jobs/{job_id}/cancel"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/generation-jobs/recover"));
        assert!(uris.contains(&"/generated-images/{file_name}"));
    }
}
