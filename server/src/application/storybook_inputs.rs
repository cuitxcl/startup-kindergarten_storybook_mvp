use crate::{
    domains::common,
    error::ApiError,
    models::{StorybookStatus, StorybookType, Visibility},
};

pub(crate) fn clean_optional(
    value: Option<String>,
    field: &'static str,
) -> Result<Option<String>, ApiError> {
    value
        .map(|value| common::required(value, field))
        .transpose()
}

pub(crate) fn clean_teacher_review_status(
    value: Option<String>,
) -> Result<Option<String>, ApiError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim().to_string();
    match value.as_str() {
        "pending" | "confirmed" => Ok(Some(value)),
        _ => Err(ApiError::validation(
            "teacher_review_status",
            "老师复核状态必须是 pending 或 confirmed",
        )),
    }
}

pub(crate) fn storybook_type_name(value: &StorybookType) -> &'static str {
    match value {
        StorybookType::Plain => "plain",
        StorybookType::Custom => "custom",
    }
}

pub(crate) fn storybook_status_name(value: &StorybookStatus) -> &'static str {
    match value {
        StorybookStatus::Draft => "draft",
        StorybookStatus::PlanPending => "plan_pending",
        StorybookStatus::RolesPending => "roles_pending",
        StorybookStatus::Editing => "editing",
        StorybookStatus::ImagePending => "image_pending",
        StorybookStatus::Exportable => "exportable",
        StorybookStatus::Submitted => "submitted",
        StorybookStatus::Listed => "listed",
    }
}

pub(crate) fn visibility_name(value: &Visibility) -> &'static str {
    match value {
        Visibility::Private => "private",
        Visibility::Workspace => "workspace",
        Visibility::MarketSubmission => "market_submission",
        Visibility::MarketListed => "market_listed",
    }
}

pub(crate) fn page_status_name(value: &str) -> &str {
    value
}
