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

pub(crate) fn clean_page_status(value: Option<String>) -> Result<Option<String>, ApiError> {
    clean_status_value(
        value,
        "status",
        &[
            "draft",
            "ready",
            "generating",
            "failed",
            "needs_regeneration",
        ],
        "分页状态必须是 draft、ready、generating、failed 或 needs_regeneration",
    )
}

pub(crate) fn clean_page_review_status(value: Option<String>) -> Result<Option<String>, ApiError> {
    clean_status_value(
        value,
        "review_status",
        &["unchecked", "satisfied", "needs_changes"],
        "分页验收状态必须是 unchecked、satisfied 或 needs_changes",
    )
}

pub(crate) fn clean_reference_status(value: Option<String>) -> Result<Option<String>, ApiError> {
    clean_status_value(
        value,
        "reference_status",
        &[
            "not_started",
            "generating",
            "ready",
            "failed",
            "needs_regeneration",
        ],
        "角色参考图状态必须是 not_started、generating、ready、failed 或 needs_regeneration",
    )
}

fn clean_status_value(
    value: Option<String>,
    field: &'static str,
    allowed: &[&str],
    message: &'static str,
) -> Result<Option<String>, ApiError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim().to_string();
    if allowed.contains(&value.as_str()) {
        Ok(Some(value))
    } else {
        Err(ApiError::validation(field, message))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_status_input_accepts_known_values() {
        assert_eq!(
            clean_page_status(Some("needs_regeneration".to_string())).unwrap(),
            Some("needs_regeneration".to_string())
        );
        assert!(clean_page_status(Some("done".to_string())).is_err());
    }

    #[test]
    fn page_review_status_input_accepts_known_values() {
        assert_eq!(
            clean_page_review_status(Some("satisfied".to_string())).unwrap(),
            Some("satisfied".to_string())
        );
        assert!(clean_page_review_status(Some("done".to_string())).is_err());
    }

    #[test]
    fn reference_status_input_accepts_known_values() {
        assert_eq!(
            clean_reference_status(Some("ready".to_string())).unwrap(),
            Some("ready".to_string())
        );
        assert!(clean_reference_status(Some("unknown".to_string())).is_err());
    }
}
