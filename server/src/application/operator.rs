use axum::http::HeaderMap;
use loco_rs::app::AppContext;
#[cfg(feature = "db")]
use serde_json::json;
use uuid::Uuid;

use crate::{
    domains::common,
    error::ApiError,
    models::{
        AuditLogEntry, GenerationCostListQuery, GenerationCostReport, ListQuery,
        MarketplaceSubmission, MarketplaceTemplate, PaginationMeta, SubmissionListQuery,
        UpdateMarketplaceTemplateRequest,
    },
};

#[cfg(not(feature = "db"))]
use crate::models::GenerationCostSummary;

pub use crate::application::operator_readiness::{ReadinessResponse, readiness};

pub async fn list_submissions(
    ctx: &AppContext,
    headers: &HeaderMap,
    query: SubmissionListQuery,
) -> Result<(Vec<MarketplaceSubmission>, PaginationMeta), ApiError> {
    validate_submission_status(query.status.as_deref())?;

    #[cfg(feature = "db")]
    {
        common::require_operator_db(ctx, headers).await?;
        return crate::repositories::market::list_operator_submissions_page(
            &ctx.db,
            query.status.as_deref(),
            query.limit,
            query.offset,
        )
        .await
        .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_login(&state, headers)?;
        let items = state
            .read()
            .expect("state lock poisoned")
            .submissions
            .iter()
            .filter(|item| {
                query
                    .status
                    .as_deref()
                    .is_none_or(|status| item.status == status)
            })
            .cloned()
            .collect();
        Ok(common::paginate_vec(items, query.limit, query.offset))
    }
}

pub async fn list_audit_logs(
    ctx: &AppContext,
    headers: &HeaderMap,
    query: ListQuery,
) -> Result<(Vec<AuditLogEntry>, PaginationMeta), ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_operator_db(ctx, headers).await?;
        return crate::repositories::audit::list_all_page(&ctx.db, query.limit, query.offset)
            .await
            .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_login(&state, headers)?;
        Ok(common::paginate_vec(Vec::new(), query.limit, query.offset))
    }
}

pub async fn list_generation_costs(
    ctx: &AppContext,
    headers: &HeaderMap,
    query: GenerationCostListQuery,
) -> Result<(GenerationCostReport, PaginationMeta), ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_operator_db(ctx, headers).await?;
        return crate::repositories::generation_costs::list_operator_costs_page(&ctx.db, query)
            .await
            .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_login(&state, headers)?;
        let limit = query.limit.unwrap_or(50).clamp(1, 100);
        let offset = query.offset.unwrap_or(0);
        Ok((
            GenerationCostReport {
                summary: GenerationCostSummary {
                    total_cost_micros: 0,
                    succeeded_cost_micros: 0,
                    failed_jobs: 0,
                    total_jobs: 0,
                    total_input_units: 0,
                    total_output_units: 0,
                    total_images: 0,
                    currency: "USD".to_string(),
                    budget_limit_micros: None,
                    budget_used_percent: None,
                    budget_warning_percent: None,
                    budget_warning: false,
                    budget_exceeded: false,
                },
                items: vec![],
            },
            PaginationMeta {
                total: 0,
                limit,
                offset,
                has_more: false,
            },
        ))
    }
}

pub async fn generation_provider(
    ctx: &AppContext,
    headers: &HeaderMap,
) -> Result<crate::services::generation_provider::GenerationProviderSummary, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_operator_db(ctx, headers).await?;
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_login(&state, headers)?;
    }

    Ok(crate::services::generation_provider::ConfiguredGenerationProvider::from_env().summary())
}

pub async fn storage(
    ctx: &AppContext,
    headers: &HeaderMap,
) -> Result<crate::services::storage::StorageSummary, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_operator_db(ctx, headers).await?;
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_login(&state, headers)?;
    }

    Ok(crate::services::storage::storage_summary())
}

pub async fn update_template(
    ctx: &AppContext,
    headers: &HeaderMap,
    template_id: Uuid,
    payload: UpdateMarketplaceTemplateRequest,
) -> Result<MarketplaceTemplate, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_operator_db(ctx, headers).await?;
        let template = crate::repositories::market::update_template(&ctx.db, template_id, payload)
            .await
            .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            None,
            Some(common::actor_user_id(headers)?),
            "marketplace_template.updated",
            "marketplace_template",
            Some(template.id),
            json!({
                "template_title": template.title,
                "source_type": template.source_type,
                "supports_customization": template.supports_customization,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(template);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_login(&state, headers)?;
        let mut state = state.write().expect("state lock poisoned");
        let template = state
            .templates
            .iter_mut()
            .find(|item| item.id == template_id)
            .ok_or_else(|| ApiError::not_found("template"))?;
        if let Some(title) = payload.title {
            template.title = title.trim().to_string();
        }
        if let Some(summary) = payload.summary {
            template.summary = summary.trim().to_string();
        }
        if let Some(age_group) = payload.age_group {
            template.age_group = age_group.trim().to_string();
        }
        if let Some(use_scene) = payload.use_scene {
            template.use_scene = use_scene.trim().to_string();
        }
        if let Some(supports_customization) = payload.supports_customization {
            template.supports_customization = supports_customization;
        }
        if let Some(tags) = payload.tags {
            template.tags = tags
                .into_iter()
                .map(|tag| tag.trim().to_string())
                .filter(|tag| !tag.is_empty())
                .take(12)
                .collect();
        }
        if template.title.is_empty()
            || template.summary.is_empty()
            || template.age_group.is_empty()
            || template.use_scene.is_empty()
        {
            return Err(ApiError::validation(
                "template",
                "模板标题、摘要、年龄段和场景不能为空",
            ));
        }
        Ok(template.clone())
    }
}

pub async fn approve_submission(
    ctx: &AppContext,
    headers: &HeaderMap,
    submission_id: Uuid,
) -> Result<MarketplaceTemplate, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_operator_db(ctx, headers).await?;
        let template = crate::repositories::market::approve_submission(&ctx.db, submission_id)
            .await
            .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            None,
            Some(common::actor_user_id(headers)?),
            "marketplace_submission.approved",
            "marketplace_submission",
            Some(submission_id),
            json!({
                "template_id": template.id,
                "template_title": template.title,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(template);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_login(&state, headers)?;
        let mut state = state.write().expect("state lock poisoned");
        let submission = state
            .submissions
            .iter_mut()
            .find(|item| item.id == submission_id)
            .ok_or_else(|| ApiError::not_found("submission"))?;
        submission.status = "listed".to_string();
        submission.updated_at = "刚刚".to_string();
        let template = MarketplaceTemplate {
            id: Uuid::new_v4(),
            title: submission.title.clone(),
            summary: format!("来自园所投稿：{}", submission.source_storybook_title),
            source_type: "school_submission".to_string(),
            source_label: "园所投稿".to_string(),
            source_storybook_id: None,
            age_group: "4-5 岁".to_string(),
            use_scene: "园所共创".to_string(),
            page_count: 6,
            supports_customization: true,
            tags: vec!["园所共创".to_string()],
        };
        state.templates.push(template.clone());
        Ok(template)
    }
}

pub async fn reject_submission(
    ctx: &AppContext,
    headers: &HeaderMap,
    submission_id: Uuid,
) -> Result<MarketplaceSubmission, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_operator_db(ctx, headers).await?;
        let item = crate::repositories::market::reject_submission(&ctx.db, submission_id)
            .await
            .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            None,
            Some(common::actor_user_id(headers)?),
            "marketplace_submission.rejected",
            "marketplace_submission",
            Some(submission_id),
            json!({
                "title": item.title,
                "status": item.status,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(item);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_login(&state, headers)?;
        let mut state = state.write().expect("state lock poisoned");
        let submission = state
            .submissions
            .iter_mut()
            .find(|item| item.id == submission_id)
            .ok_or_else(|| ApiError::not_found("submission"))?;
        submission.status = "rejected".to_string();
        submission.updated_at = "刚刚".to_string();
        Ok(submission.clone())
    }
}

fn validate_submission_status(status: Option<&str>) -> Result<(), ApiError> {
    match status.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("draft" | "submitted" | "approved" | "listed" | "rejected") => Ok(()),
        Some(_) => Err(ApiError::validation(
            "status",
            "状态只能是 draft、submitted、approved、listed 或 rejected",
        )),
    }
}
