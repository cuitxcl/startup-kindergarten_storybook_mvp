#[cfg(not(feature = "db"))]
use chrono::{DateTime, Utc};
#[cfg(not(feature = "db"))]
use loco_rs::app::AppContext;
#[cfg(feature = "db")]
use serde_json::json;
use uuid::Uuid;

pub use super::delivery_exports::{
    create_export, get_export, list_exports, public_export_file, workspace_export_file,
};
pub use super::delivery_share_links::{
    create_public_export, create_share_link, get_public_export, get_public_share, list_share_links,
    public_share_export_file, revoke_share_link,
};

#[cfg(not(feature = "db"))]
use crate::models::ShareLink;
use crate::{
    domains::common,
    error::ApiError,
    models::{ExportJob, Storybook, StorybookStatus},
};

pub(crate) fn read_export_job_file(job: &ExportJob) -> Result<(String, Vec<u8>), ApiError> {
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

pub(crate) fn with_workspace_export_download_url(
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

pub(crate) fn with_share_export_download_url(mut job: ExportJob, token: &str) -> ExportJob {
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

pub(crate) fn ensure_storybook_deliverable(book: &Storybook) -> Result<(), ApiError> {
    if matches!(
        book.status,
        StorybookStatus::Exportable | StorybookStatus::Listed
    ) {
        crate::repositories::storybook_rules::ensure_delivery_access_ready(book)
            .map_err(common::db_error)?;
        return Ok(());
    }
    Err(ApiError::state_conflict(
        "绘本还未标记为可交付，不能导出或创建分享链接",
    ))
}

#[cfg(feature = "db")]
pub(crate) fn delivery_error(err: sea_orm::DbErr) -> ApiError {
    match err {
        sea_orm::DbErr::Custom(message) if delivery_privacy_risk_labels(&message).is_some() => {
            let risks = delivery_privacy_risk_labels(&message).unwrap_or_default();
            ApiError::state_conflict(format!("绘本内容可能包含{}，请先修改后再导出或分享", risks))
        }
        other => common::db_error(other),
    }
}

#[cfg(feature = "db")]
pub(crate) fn delivery_privacy_risk_labels(message: &str) -> Option<&str> {
    message.strip_prefix("delivery_privacy_risk:")
}

#[cfg(feature = "db")]
pub(crate) async fn log_delivery_privacy_blocked(
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

pub(crate) fn valid_export_file_name(file_name: &str) -> bool {
    let Some(id) = file_name.strip_suffix(".pdf") else {
        return false;
    };
    Uuid::parse_str(id).is_ok()
}

#[cfg(not(feature = "db"))]
pub(crate) fn shared_state(ctx: &AppContext) -> Result<crate::state::SharedState, ApiError> {
    ctx.shared_store
        .get::<crate::state::SharedState>()
        .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))
}

#[cfg(not(feature = "db"))]
pub(crate) fn find_storybook(
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
pub(crate) fn share_link_active(link: &ShareLink) -> bool {
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
pub(crate) fn mock_workspace_export(
    workspace_id: Uuid,
    storybook_id: Uuid,
    export_id: Uuid,
) -> ExportJob {
    ExportJob {
        id: export_id,
        storybook_id,
        created_by: None,
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
pub(crate) fn mock_share_export(token: &str, storybook_id: Uuid, export_id: Uuid) -> ExportJob {
    ExportJob {
        id: export_id,
        storybook_id,
        created_by: None,
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
