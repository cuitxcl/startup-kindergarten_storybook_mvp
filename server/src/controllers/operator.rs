use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, patch, post},
};
use loco_rs::{app::AppContext, controller::Routes};
use uuid::Uuid;

use crate::{
    application,
    error::ApiError,
    models::{
        AuditLogEntry, Envelope, GenerationCostListQuery, GenerationCostReport, ListQuery,
        MarketplaceSubmission, MarketplaceTemplate, SubmissionListQuery,
        UpdateMarketplaceTemplateRequest,
    },
};

pub fn routes() -> Routes {
    Routes::new()
        .add("/api/operator/submissions", get(list_submissions))
        .add("/api/operator/audit-logs", get(list_audit_logs))
        .add("/api/operator/generation-costs", get(list_generation_costs))
        .add(
            "/api/operator/generation-provider",
            get(get_generation_provider),
        )
        .add("/api/operator/storage", get(get_storage))
        .add("/api/operator/readiness", get(get_readiness))
        .add(
            "/api/operator/marketplace/templates/{template_id}",
            patch(update_template),
        )
        .add(
            "/api/operator/submissions/{submission_id}/approve",
            post(approve_submission),
        )
        .add(
            "/api/operator/submissions/{submission_id}/reject",
            post(reject_submission),
        )
}

async fn list_submissions(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Query(query): Query<SubmissionListQuery>,
) -> Result<Json<Envelope<Vec<MarketplaceSubmission>>>, ApiError> {
    let (items, meta) = application::operator::list_submissions(&ctx, &headers, query).await?;
    Ok(Json(Envelope::with_meta(items, meta)))
}

async fn list_audit_logs(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<Envelope<Vec<AuditLogEntry>>>, ApiError> {
    let (items, meta) = application::operator::list_audit_logs(&ctx, &headers, query).await?;
    Ok(Json(Envelope::with_meta(items, meta)))
}

async fn list_generation_costs(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Query(query): Query<GenerationCostListQuery>,
) -> Result<Json<Envelope<GenerationCostReport>>, ApiError> {
    let (report, meta) =
        application::operator::list_generation_costs(&ctx, &headers, query).await?;
    Ok(Json(Envelope::with_meta(report, meta)))
}

async fn get_generation_provider(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> Result<Json<Envelope<crate::services::generation_provider::GenerationProviderSummary>>, ApiError>
{
    let provider = application::operator::generation_provider(&ctx, &headers).await?;
    Ok(Json(Envelope::new(provider)))
}

async fn get_storage(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> Result<Json<Envelope<crate::services::storage::StorageSummary>>, ApiError> {
    let storage = application::operator::storage(&ctx, &headers).await?;
    Ok(Json(Envelope::new(storage)))
}

async fn get_readiness(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> Result<Json<Envelope<application::operator::ReadinessResponse>>, ApiError> {
    let readiness = application::operator::readiness(&ctx, &headers).await?;
    Ok(Json(Envelope::new(readiness)))
}

async fn update_template(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(template_id): Path<Uuid>,
    Json(payload): Json<UpdateMarketplaceTemplateRequest>,
) -> Result<Json<Envelope<MarketplaceTemplate>>, ApiError> {
    let template =
        application::operator::update_template(&ctx, &headers, template_id, payload).await?;
    Ok(Json(Envelope::new(template)))
}

async fn approve_submission(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(submission_id): Path<Uuid>,
) -> Result<(StatusCode, Json<Envelope<MarketplaceTemplate>>), ApiError> {
    let template = application::operator::approve_submission(&ctx, &headers, submission_id).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(template))))
}

async fn reject_submission(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(submission_id): Path<Uuid>,
) -> Result<Json<Envelope<MarketplaceSubmission>>, ApiError> {
    let item = application::operator::reject_submission(&ctx, &headers, submission_id).await?;
    Ok(Json(Envelope::new(item)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_include_operator_api() {
        let registered_routes = routes();
        let uris = registered_routes
            .handlers
            .iter()
            .map(|handler| handler.uri.as_str())
            .collect::<Vec<_>>();

        assert!(uris.contains(&"/api/operator/marketplace/templates/{template_id}"));
        assert!(uris.contains(&"/api/operator/submissions/{submission_id}/reject"));
        assert!(uris.contains(&"/api/operator/audit-logs"));
        assert!(uris.contains(&"/api/operator/generation-costs"));
        assert!(uris.contains(&"/api/operator/generation-provider"));
        assert!(uris.contains(&"/api/operator/storage"));
        assert!(uris.contains(&"/api/operator/readiness"));
    }
}
