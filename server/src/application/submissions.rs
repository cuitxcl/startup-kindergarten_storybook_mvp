use axum::http::HeaderMap;
use loco_rs::app::AppContext;
use uuid::Uuid;

#[cfg(feature = "db")]
use serde_json::json;

use crate::{
    domains::common,
    error::ApiError,
    models::{CreateSubmissionRequest, MarketplaceSubmission, SubmissionListQuery},
};

#[cfg(not(feature = "db"))]
use crate::models::StorybookType;

pub async fn list(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    query: SubmissionListQuery,
) -> Result<(Vec<MarketplaceSubmission>, crate::models::PaginationMeta), ApiError> {
    validate_submission_status(query.status.as_deref())?;
    #[cfg(feature = "db")]
    {
        common::require_admin_db(ctx, headers, workspace_id).await?;
        return crate::repositories::market::list_submissions_page(
            &ctx.db,
            workspace_id,
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
        common::require_admin(&state, headers, workspace_id)?;
        let state = state.read().expect("state lock poisoned");
        let items = state
            .submissions
            .iter()
            .filter(|item| {
                item.workspace_id == workspace_id
                    && query
                        .status
                        .as_deref()
                        .is_none_or(|status| item.status == status)
            })
            .cloned()
            .collect();
        Ok(common::paginate_vec(items, query.limit, query.offset))
    }
}

pub async fn create(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    payload: CreateSubmissionRequest,
) -> Result<MarketplaceSubmission, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_admin_db(ctx, headers, workspace_id).await?;
        let item = crate::repositories::market::create_submission(&ctx.db, workspace_id, payload)
            .await
            .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "marketplace_submission.created",
            "marketplace_submission",
            Some(item.id),
            json!({
                "title": item.title,
                "status": item.status,
                "privacy_confirmed": item.privacy_confirmed,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(item);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_admin(&state, headers, workspace_id)?;
        let mut state = state.write().expect("state lock poisoned");
        let book = state
            .storybooks
            .iter()
            .find(|item| item.workspace_id == workspace_id && item.id == payload.storybook_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("storybook"))?;
        if book.storybook_type != StorybookType::Plain {
            return Err(ApiError::state_conflict("只有普通绘本可以投稿"));
        }
        let item = MarketplaceSubmission {
            id: Uuid::new_v4(),
            workspace_id,
            title: book.title.clone(),
            source_storybook_title: book.title,
            submitted_by: state.current_user.display_name.clone(),
            status: "draft".to_string(),
            privacy_confirmed: false,
            updated_at: "刚刚".to_string(),
        };
        state.submissions.push(item.clone());
        Ok(item)
    }
}

pub async fn confirm_privacy(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    submission_id: Uuid,
) -> Result<MarketplaceSubmission, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_admin_db(ctx, headers, workspace_id).await?;
        let actor_id = common::actor_user_id(headers)?;
        let item = match crate::repositories::market::confirm_submission_privacy(
            &ctx.db,
            workspace_id,
            submission_id,
        )
        .await
        {
            Ok(item) => item,
            Err(sea_orm::DbErr::Custom(message))
                if submission_privacy_risk_labels(&message).is_some() =>
            {
                let risks = submission_privacy_risk_labels(&message).unwrap_or_default();
                crate::repositories::audit::log(
                    &ctx.db,
                    Some(workspace_id),
                    Some(actor_id),
                    "marketplace_submission.privacy_blocked",
                    "marketplace_submission",
                    Some(submission_id),
                    json!({
                        "status": "draft",
                        "privacy_confirmed": false,
                        "risk_labels": risks.split('、').collect::<Vec<_>>(),
                    }),
                )
                .await
                .map_err(common::db_error)?;
                return Err(marketplace_error(sea_orm::DbErr::Custom(message)));
            }
            Err(err) => return Err(marketplace_error(err)),
        };
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "marketplace_submission.privacy_confirmed",
            "marketplace_submission",
            Some(item.id),
            json!({
                "status": item.status,
                "privacy_confirmed": item.privacy_confirmed,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(item);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_admin(&state, headers, workspace_id)?;
        let mut state = state.write().expect("state lock poisoned");
        let item = state
            .submissions
            .iter_mut()
            .find(|item| item.workspace_id == workspace_id && item.id == submission_id)
            .ok_or_else(|| ApiError::not_found("submission"))?;
        item.privacy_confirmed = true;
        item.status = "submitted".to_string();
        item.updated_at = "刚刚".to_string();
        Ok(item.clone())
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

#[cfg(feature = "db")]
fn marketplace_error(err: sea_orm::DbErr) -> ApiError {
    match err {
        sea_orm::DbErr::Custom(message) if submission_privacy_risk_labels(&message).is_some() => {
            let risks = submission_privacy_risk_labels(&message).unwrap_or_default();
            ApiError::state_conflict(format!(
                "投稿内容可能包含{}，请先修改绘本内容再确认隐私",
                risks
            ))
        }
        other => common::db_error(other),
    }
}

#[cfg(feature = "db")]
fn submission_privacy_risk_labels(message: &str) -> Option<&str> {
    message.strip_prefix("submission_privacy_risk:")
}

#[cfg(not(feature = "db"))]
fn shared_state(ctx: &AppContext) -> Result<crate::state::SharedState, ApiError> {
    ctx.shared_store
        .get::<crate::state::SharedState>()
        .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))
}
