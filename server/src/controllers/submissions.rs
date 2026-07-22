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
    models::{CreateSubmissionRequest, Envelope, MarketplaceSubmission, SubmissionListQuery},
};

pub fn routes() -> Routes {
    Routes::new()
        .add(
            "/api/workspaces/{workspace_id}/submissions",
            get(list_submissions).post(create_submission),
        )
        .add(
            "/api/workspaces/{workspace_id}/submissions/{submission_id}/privacy-confirm",
            post(confirm_submission_privacy),
        )
}

async fn list_submissions(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Query(query): Query<SubmissionListQuery>,
) -> Result<Json<Envelope<Vec<MarketplaceSubmission>>>, ApiError> {
    let (submissions, meta) =
        application::submissions::list(&ctx, &headers, workspace_id, query).await?;
    Ok(Json(Envelope::with_meta(submissions, meta)))
}

async fn create_submission(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Json(payload): Json<CreateSubmissionRequest>,
) -> Result<(StatusCode, Json<Envelope<MarketplaceSubmission>>), ApiError> {
    let submission =
        application::submissions::create(&ctx, &headers, workspace_id, payload).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(submission))))
}

async fn confirm_submission_privacy(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, submission_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Envelope<MarketplaceSubmission>>, ApiError> {
    let submission =
        application::submissions::confirm_privacy(&ctx, &headers, workspace_id, submission_id)
            .await?;
    Ok(Json(Envelope::new(submission)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_include_submissions_api() {
        let registered_routes = routes();
        let uris = registered_routes
            .handlers
            .iter()
            .map(|handler| handler.uri.as_str())
            .collect::<Vec<_>>();

        assert!(uris.contains(&"/api/workspaces/{workspace_id}/submissions"));
        assert!(uris.contains(
            &"/api/workspaces/{workspace_id}/submissions/{submission_id}/privacy-confirm"
        ));
    }
}
