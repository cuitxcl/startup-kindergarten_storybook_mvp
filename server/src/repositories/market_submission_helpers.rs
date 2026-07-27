use chrono::{DateTime, Utc};
use sea_orm::DbErr;
use uuid::Uuid;

use crate::{
    models::{MarketplaceSubmission, MarketplaceTemplate},
    repositories::market_templates::source_label,
};

#[derive(Clone, Copy)]
pub(crate) enum SubmissionStatusFilter {
    Any,
    Draft,
    Submitted,
    Approved,
    Listed,
    Rejected,
}

impl SubmissionStatusFilter {
    pub(crate) fn workspace_where_sql(self) -> &'static str {
        match self {
            SubmissionStatusFilter::Any => "",
            SubmissionStatusFilter::Draft => "and status = 'draft'",
            SubmissionStatusFilter::Submitted => "and status = 'submitted'",
            SubmissionStatusFilter::Approved => "and status = 'approved'",
            SubmissionStatusFilter::Listed => "and status = 'listed'",
            SubmissionStatusFilter::Rejected => "and status = 'rejected'",
        }
    }

    pub(crate) fn alias_where_sql(self) -> &'static str {
        match self {
            SubmissionStatusFilter::Any => "",
            SubmissionStatusFilter::Draft => "and ms.status = 'draft'",
            SubmissionStatusFilter::Submitted => "and ms.status = 'submitted'",
            SubmissionStatusFilter::Approved => "and ms.status = 'approved'",
            SubmissionStatusFilter::Listed => "and ms.status = 'listed'",
            SubmissionStatusFilter::Rejected => "and ms.status = 'rejected'",
        }
    }

    pub(crate) fn operator_where_sql(self) -> &'static str {
        match self {
            SubmissionStatusFilter::Draft => "and status = 'draft'",
            _ => self.workspace_where_sql(),
        }
    }
}

pub(crate) fn submission_status_filter(
    status: Option<&str>,
) -> Result<SubmissionStatusFilter, DbErr> {
    match status.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(SubmissionStatusFilter::Any),
        Some("draft") => Ok(SubmissionStatusFilter::Draft),
        Some("submitted") => Ok(SubmissionStatusFilter::Submitted),
        Some("approved") => Ok(SubmissionStatusFilter::Approved),
        Some("listed") => Ok(SubmissionStatusFilter::Listed),
        Some("rejected") => Ok(SubmissionStatusFilter::Rejected),
        Some(other) => Err(DbErr::Custom(format!("不支持的市场投稿状态：{other}"))),
    }
}

pub(crate) fn submission_from_row(
    row: &sea_orm::QueryResult,
) -> Result<MarketplaceSubmission, DbErr> {
    Ok(MarketplaceSubmission {
        id: row.try_get("", "id")?,
        workspace_id: row.try_get("", "workspace_id")?,
        title: row.try_get("", "title")?,
        source_storybook_title: row.try_get("", "source_storybook_title")?,
        submitted_by: row.try_get("", "submitted_by")?,
        status: row.try_get("", "status")?,
        privacy_confirmed: row.try_get("", "privacy_confirmed")?,
        updated_at: row
            .try_get::<DateTime<Utc>>("", "updated_at")?
            .format("%Y-%m-%d %H:%M")
            .to_string(),
    })
}

pub(crate) fn build_template_from_submission(
    submission_id: Uuid,
    workspace_id: Uuid,
    source_storybook_id: Uuid,
    title: String,
    age_group: String,
    use_scene: String,
    summary: String,
    page_count: i32,
) -> MarketplaceTemplate {
    let source_label = source_label("school_submission").to_string();
    let tags_scene = use_scene.clone();
    MarketplaceTemplate {
        id: submission_id,
        title,
        summary,
        source_type: "school_submission".to_string(),
        source_label,
        source_storybook_id: Some(source_storybook_id),
        age_group,
        use_scene,
        page_count: page_count.max(0) as u32,
        supports_customization: true,
        tags: vec!["园所共创".to_string(), tags_scene, workspace_id.to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_template_from_submission_marks_school_submission() {
        let template = build_template_from_submission(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "午睡小小约定".to_string(),
            "4-5 岁".to_string(),
            "午睡习惯".to_string(),
            "建立睡前整理和安静入睡流程".to_string(),
            6,
        );

        assert_eq!(template.source_type, "school_submission");
        assert_eq!(template.source_label, "园所投稿");
        assert!(template.supports_customization);
        assert_eq!(template.page_count, 6);
    }

    #[test]
    fn build_template_from_submission_clamps_page_count() {
        let template = build_template_from_submission(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "午睡小小约定".to_string(),
            "4-5 岁".to_string(),
            "午睡习惯".to_string(),
            "建立睡前整理和安静入睡流程".to_string(),
            -1,
        );

        assert_eq!(template.page_count, 0);
    }
}
