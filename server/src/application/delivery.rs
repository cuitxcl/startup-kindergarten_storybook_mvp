use axum::http::HeaderMap;
#[cfg(not(feature = "db"))]
use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use uuid::Uuid;

#[cfg(feature = "db")]
use serde_json::json;

#[cfg(feature = "db")]
use crate::workers::export::enqueue_export_job;

use crate::{
    domains::common,
    error::ApiError,
    models::{
        CreateShareLinkRequest, ExportJob, ListQuery, PaginationMeta, ShareLink, Storybook,
        StorybookStatus,
    },
};

pub async fn create_export(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<ExportJob, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
        let actor_id = common::actor_user_id(headers)?;
        let book = crate::repositories::storybooks::find(&ctx.db, workspace_id, storybook_id)
            .await
            .map_err(common::db_error)?;
        ensure_storybook_deliverable(&book)?;
        let job =
            match crate::repositories::delivery::create_export(&ctx.db, workspace_id, storybook_id)
                .await
            {
                Ok(job) => job,
                Err(sea_orm::DbErr::Custom(message))
                    if delivery_privacy_risk_labels(&message).is_some() =>
                {
                    let risks = delivery_privacy_risk_labels(&message).unwrap_or_default();
                    log_delivery_privacy_blocked(
                        &ctx.db,
                        Some(workspace_id),
                        Some(actor_id),
                        storybook_id,
                        "export",
                        risks,
                    )
                    .await?;
                    return Err(delivery_error(sea_orm::DbErr::Custom(message)));
                }
                Err(err) => return Err(delivery_error(err)),
            };
        enqueue_export_job(ctx, job.id)
            .await
            .map_err(|err| ApiError::state_conflict(format!("导出任务入队失败：{err}")))?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "storybook.export_created",
            "export_job",
            Some(job.id),
            json!({
                "storybook_id": storybook_id,
                "status": job.status,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(with_workspace_export_download_url(
            job,
            workspace_id,
            storybook_id,
        ));
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        let book = find_storybook(&state, workspace_id, storybook_id)?;
        ensure_storybook_deliverable(&book)?;
        let export_id = Uuid::new_v4();
        Ok(mock_workspace_export(workspace_id, storybook_id, export_id))
    }
}

pub async fn list_exports(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    query: ListQuery,
) -> Result<(Vec<ExportJob>, PaginationMeta), ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0);
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
        let (jobs, meta) = crate::repositories::delivery::list_exports(
            &ctx.db,
            workspace_id,
            storybook_id,
            limit,
            offset,
        )
        .await
        .map_err(common::db_error)?;
        let jobs = jobs
            .into_iter()
            .map(|job| with_workspace_export_download_url(job, workspace_id, storybook_id))
            .collect::<Vec<_>>();
        return Ok((jobs, meta));
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        find_storybook(&state, workspace_id, storybook_id)?;
        Ok(common::paginate_vec(Vec::new(), Some(limit), Some(offset)))
    }
}

pub async fn get_export(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    export_id: Uuid,
) -> Result<ExportJob, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
        let job = crate::repositories::delivery::find_export(
            &ctx.db,
            workspace_id,
            storybook_id,
            export_id,
        )
        .await
        .map_err(common::db_error)?;
        return Ok(with_workspace_export_download_url(
            job,
            workspace_id,
            storybook_id,
        ));
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        find_storybook(&state, workspace_id, storybook_id)?;
        Ok(mock_workspace_export(workspace_id, storybook_id, export_id))
    }
}

pub async fn workspace_export_file(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    export_id: Uuid,
) -> Result<(String, Vec<u8>), ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
        let job = crate::repositories::delivery::find_export(
            &ctx.db,
            workspace_id,
            storybook_id,
            export_id,
        )
        .await
        .map_err(common::db_error)?;
        return read_export_job_file(&job);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        find_storybook(&state, workspace_id, storybook_id)?;
        read_export_job_file(&mock_workspace_export(
            workspace_id,
            storybook_id,
            export_id,
        ))
    }
}

pub fn public_export_file(file_name: &str) -> Result<(String, Vec<u8>), ApiError> {
    let safe_name = file_name.trim();
    if !valid_export_file_name(safe_name) {
        return Err(ApiError::not_found("export"));
    }
    Err(ApiError::not_found("export"))
}

pub async fn create_share_link(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    payload: CreateShareLinkRequest,
) -> Result<ShareLink, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
        let actor_id = common::actor_user_id(headers)?;
        let book = crate::repositories::storybooks::find(&ctx.db, workspace_id, storybook_id)
            .await
            .map_err(common::db_error)?;
        ensure_storybook_deliverable(&book)?;
        let link = match crate::repositories::delivery::create_share_link(
            &ctx.db,
            workspace_id,
            storybook_id,
            payload.expires_at,
        )
        .await
        {
            Ok(link) => link,
            Err(sea_orm::DbErr::Custom(message))
                if delivery_privacy_risk_labels(&message).is_some() =>
            {
                let risks = delivery_privacy_risk_labels(&message).unwrap_or_default();
                log_delivery_privacy_blocked(
                    &ctx.db,
                    Some(workspace_id),
                    Some(actor_id),
                    storybook_id,
                    "share_link",
                    risks,
                )
                .await?;
                return Err(delivery_error(sea_orm::DbErr::Custom(message)));
            }
            Err(err) => return Err(delivery_error(err)),
        };
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "storybook.share_link_created",
            "share_link",
            Some(link.id),
            json!({
                "storybook_id": storybook_id,
                "status": link.status,
                "expires_at": link.expires_at,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(link);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        let book = find_storybook(&state, workspace_id, storybook_id)?;
        ensure_storybook_deliverable(&book)?;
        let token = Uuid::new_v4().simple().to_string();
        let link = ShareLink {
            id: Uuid::new_v4(),
            storybook_id,
            token: token.clone(),
            url: format!("/link/share/{token}"),
            status: if payload
                .expires_at
                .is_some_and(|expires_at| expires_at <= Utc::now())
            {
                "expired".to_string()
            } else {
                "active".to_string()
            },
            access_count: 0,
            last_accessed_at: None,
            expires_at: payload.expires_at.map(|value| value.to_rfc3339()),
        };
        state
            .write()
            .expect("state lock poisoned")
            .share_links
            .insert(token, link.clone());
        Ok(link)
    }
}

pub async fn list_share_links(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    query: ListQuery,
) -> Result<(Vec<ShareLink>, PaginationMeta), ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0);
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
        return crate::repositories::delivery::list_share_links(
            &ctx.db,
            workspace_id,
            storybook_id,
            limit,
            offset,
        )
        .await
        .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        find_storybook(&state, workspace_id, storybook_id)?;
        let state = state.read().expect("state lock poisoned");
        let links = state
            .share_links
            .values()
            .filter(|item| item.storybook_id == storybook_id && share_link_active(item))
            .cloned()
            .collect();
        Ok(common::paginate_vec(links, Some(limit), Some(offset)))
    }
}

pub async fn revoke_share_link(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    storybook_id: Uuid,
    share_link_id: Uuid,
) -> Result<ShareLink, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
        let link = crate::repositories::delivery::revoke_share_link(
            &ctx.db,
            workspace_id,
            storybook_id,
            share_link_id,
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "storybook.share_link_revoked",
            "share_link",
            Some(link.id),
            json!({
                "storybook_id": storybook_id,
                "status": link.status,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(link);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_workspace(&state, headers, workspace_id)?;
        find_storybook(&state, workspace_id, storybook_id)?;
        let mut state = state.write().expect("state lock poisoned");
        let link = state
            .share_links
            .values_mut()
            .find(|item| item.id == share_link_id && item.storybook_id == storybook_id)
            .ok_or_else(|| ApiError::not_found("share_link"))?;
        if !share_link_active(link) {
            return Err(ApiError::not_found("share_link"));
        }
        link.status = "revoked".to_string();
        Ok(link.clone())
    }
}

pub async fn get_public_share(ctx: &AppContext, token: String) -> Result<Storybook, ApiError> {
    #[cfg(feature = "db")]
    {
        let book = crate::repositories::delivery::storybook_by_share_token(&ctx.db, &token)
            .await
            .map_err(common::db_error)?;
        crate::repositories::delivery::record_share_link_access(&ctx.db, &token)
            .await
            .map_err(common::db_error)?;
        return Ok(book);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        let state = state.read().expect("state lock poisoned");
        let link = state
            .share_links
            .get(&token)
            .ok_or_else(|| ApiError::not_found("share_link"))?;
        if !share_link_active(link) {
            return Err(ApiError::not_found("share_link"));
        }
        state
            .storybooks
            .iter()
            .find(|item| item.id == link.storybook_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("storybook"))
    }
}

pub async fn create_public_export(ctx: &AppContext, token: String) -> Result<ExportJob, ApiError> {
    #[cfg(feature = "db")]
    {
        let shared_storybook =
            crate::repositories::delivery::storybook_by_share_token(&ctx.db, &token)
                .await
                .map_err(delivery_error)?;
        let job = match crate::repositories::delivery::create_export_by_share_token(&ctx.db, &token)
            .await
        {
            Ok(job) => job,
            Err(sea_orm::DbErr::Custom(message))
                if delivery_privacy_risk_labels(&message).is_some() =>
            {
                let risks = delivery_privacy_risk_labels(&message).unwrap_or_default();
                log_delivery_privacy_blocked(
                    &ctx.db,
                    Some(shared_storybook.workspace_id),
                    None,
                    shared_storybook.id,
                    "public_export",
                    risks,
                )
                .await?;
                return Err(delivery_error(sea_orm::DbErr::Custom(message)));
            }
            Err(err) => return Err(delivery_error(err)),
        };
        enqueue_export_job(ctx, job.id)
            .await
            .map_err(|err| ApiError::state_conflict(format!("公开导出任务入队失败：{err}")))?;
        crate::repositories::audit::log(
            &ctx.db,
            None,
            None,
            "share_link.public_export_created",
            "export_job",
            Some(job.id),
            json!({
                "storybook_id": job.storybook_id,
                "share_token_suffix": token.chars().rev().take(6).collect::<String>(),
                "status": job.status,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(with_share_export_download_url(job, &token));
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        let state = state.read().expect("state lock poisoned");
        let link = state
            .share_links
            .get(&token)
            .ok_or_else(|| ApiError::not_found("share_link"))?;
        if !share_link_active(link) {
            return Err(ApiError::not_found("share_link"));
        }
        let export_id = Uuid::new_v4();
        Ok(mock_share_export(&token, link.storybook_id, export_id))
    }
}

pub async fn get_public_export(
    ctx: &AppContext,
    token: String,
    export_id: Uuid,
) -> Result<ExportJob, ApiError> {
    #[cfg(feature = "db")]
    {
        let job =
            crate::repositories::delivery::find_export_by_share_token(&ctx.db, &token, export_id)
                .await
                .map_err(common::db_error)?;
        return Ok(with_share_export_download_url(job, &token));
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        let state = state.read().expect("state lock poisoned");
        let link = state
            .share_links
            .get(&token)
            .ok_or_else(|| ApiError::not_found("share_link"))?;
        if !share_link_active(link) {
            return Err(ApiError::not_found("share_link"));
        }
        Ok(mock_share_export(&token, link.storybook_id, export_id))
    }
}

pub async fn public_share_export_file(
    ctx: &AppContext,
    token: String,
    export_id: Uuid,
) -> Result<(String, Vec<u8>), ApiError> {
    #[cfg(feature = "db")]
    {
        let job =
            crate::repositories::delivery::find_export_by_share_token(&ctx.db, &token, export_id)
                .await
                .map_err(common::db_error)?;
        return read_export_job_file(&job);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        let state = state.read().expect("state lock poisoned");
        let link = state
            .share_links
            .get(&token)
            .ok_or_else(|| ApiError::not_found("share_link"))?;
        if link.status != "active" {
            return Err(ApiError::not_found("share_link"));
        }
        read_export_job_file(&mock_share_export(&token, link.storybook_id, export_id))
    }
}

fn read_export_job_file(job: &ExportJob) -> Result<(String, Vec<u8>), ApiError> {
    if job.status != "succeeded" {
        return Err(ApiError::not_found("export"));
    }
    let file_name = format!("{}.pdf", job.id);
    if !valid_export_file_name(&file_name) {
        return Err(ApiError::not_found("export"));
    }
    let bytes = crate::services::storage::read_export_file(&file_name)
        .map_err(|_| ApiError::not_found("export"))?;
    Ok((file_name, bytes))
}

fn with_workspace_export_download_url(
    mut job: ExportJob,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> ExportJob {
    if job.file_url.is_some() && job.status == "succeeded" {
        job.file_url = Some(workspace_export_download_url(
            workspace_id,
            storybook_id,
            job.id,
        ));
    }
    job
}

fn with_share_export_download_url(mut job: ExportJob, token: &str) -> ExportJob {
    if job.file_url.is_some() && job.status == "succeeded" {
        job.file_url = Some(share_export_download_url(token, job.id));
    }
    job
}

fn workspace_export_download_url(
    workspace_id: Uuid,
    storybook_id: Uuid,
    export_id: Uuid,
) -> String {
    format!("/api/workspaces/{workspace_id}/storybooks/{storybook_id}/exports/{export_id}/download")
}

fn share_export_download_url(token: &str, export_id: Uuid) -> String {
    format!("/api/share-links/{token}/exports/{export_id}/download")
}

fn ensure_storybook_deliverable(book: &Storybook) -> Result<(), ApiError> {
    if matches!(
        book.status,
        StorybookStatus::Exportable | StorybookStatus::Listed
    ) {
        return Ok(());
    }
    Err(ApiError::state_conflict(
        "绘本还未标记为可交付，不能导出或创建分享链接",
    ))
}

#[cfg(feature = "db")]
fn delivery_error(err: sea_orm::DbErr) -> ApiError {
    match err {
        sea_orm::DbErr::Custom(message) if delivery_privacy_risk_labels(&message).is_some() => {
            let risks = delivery_privacy_risk_labels(&message).unwrap_or_default();
            ApiError::state_conflict(format!("绘本内容可能包含{}，请先修改后再导出或分享", risks))
        }
        other => common::db_error(other),
    }
}

#[cfg(feature = "db")]
fn delivery_privacy_risk_labels(message: &str) -> Option<&str> {
    message.strip_prefix("delivery_privacy_risk:")
}

#[cfg(feature = "db")]
async fn log_delivery_privacy_blocked(
    db: &sea_orm::DatabaseConnection,
    workspace_id: Option<Uuid>,
    actor_user_id: Option<Uuid>,
    storybook_id: Uuid,
    operation: &str,
    risks: &str,
) -> Result<(), ApiError> {
    crate::repositories::audit::log(
        db,
        workspace_id,
        actor_user_id,
        "storybook.delivery_privacy_blocked",
        "storybook",
        Some(storybook_id),
        json!({
            "operation": operation,
            "risk_labels": risks.split('、').collect::<Vec<_>>(),
        }),
    )
    .await
    .map_err(common::db_error)
}

fn valid_export_file_name(file_name: &str) -> bool {
    let Some(id) = file_name.strip_suffix(".pdf") else {
        return false;
    };
    Uuid::parse_str(id).is_ok()
}

#[cfg(not(feature = "db"))]
fn shared_state(ctx: &AppContext) -> Result<crate::state::SharedState, ApiError> {
    ctx.shared_store
        .get::<crate::state::SharedState>()
        .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))
}

#[cfg(not(feature = "db"))]
fn find_storybook(
    state: &crate::state::SharedState,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<Storybook, ApiError> {
    state
        .read()
        .expect("state lock poisoned")
        .storybooks
        .iter()
        .find(|item| item.workspace_id == workspace_id && item.id == storybook_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("storybook"))
}

#[cfg(not(feature = "db"))]
fn share_link_active(link: &ShareLink) -> bool {
    if link.status != "active" {
        return false;
    }
    let Some(expires_at) = &link.expires_at else {
        return true;
    };
    DateTime::parse_from_rfc3339(expires_at)
        .map(|value| value.with_timezone(&Utc) > Utc::now())
        .unwrap_or(false)
}

#[cfg(not(feature = "db"))]
fn mock_workspace_export(workspace_id: Uuid, storybook_id: Uuid, export_id: Uuid) -> ExportJob {
    ExportJob {
        id: export_id,
        storybook_id,
        status: "succeeded".to_string(),
        file_url: Some(workspace_export_download_url(
            workspace_id,
            storybook_id,
            export_id,
        )),
        last_error: None,
        created_at: Utc::now(),
        finished_at: Some(Utc::now()),
    }
}

#[cfg(not(feature = "db"))]
fn mock_share_export(token: &str, storybook_id: Uuid, export_id: Uuid) -> ExportJob {
    ExportJob {
        id: export_id,
        storybook_id,
        status: "succeeded".to_string(),
        file_url: Some(share_export_download_url(token, export_id)),
        last_error: None,
        created_at: Utc::now(),
        finished_at: Some(Utc::now()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_download_file_name_requires_uuid_pdf() {
        let export_id = Uuid::new_v4();
        assert!(valid_export_file_name(&format!("{export_id}.pdf")));
        assert!(!valid_export_file_name("storybook-1.pdf"));
        assert!(!valid_export_file_name("../secret.pdf"));
        assert!(!valid_export_file_name(&format!("{export_id}.txt")));
    }
}
