use std::env;

use serde::Serialize;
use uuid::Uuid;

const WORKSPACE_STORAGE_QUOTA_BYTES_ENV: &str = "KINDLEAF_WORKSPACE_STORAGE_QUOTA_BYTES";
const USER_STORAGE_QUOTA_BYTES_ENV: &str = "KINDLEAF_USER_STORAGE_QUOTA_BYTES";
const PERSONAL_STORAGE_QUOTA_BYTES_ENV: &str = "KINDLEAF_PERSONAL_STORAGE_QUOTA_BYTES";
const SCHOOL_STORAGE_QUOTA_BYTES_ENV: &str = "KINDLEAF_SCHOOL_STORAGE_QUOTA_BYTES";
const STORAGE_QUOTA_WARNING_PERCENT_ENV: &str = "KINDLEAF_STORAGE_QUOTA_WARNING_PERCENT";
const DEFAULT_PERSONAL_STORAGE_QUOTA_BYTES: u64 = 200 * 1024 * 1024;
const DEFAULT_SCHOOL_STORAGE_QUOTA_BYTES: u64 = 5 * 1024 * 1024 * 1024;
const DEFAULT_STORAGE_QUOTA_WARNING_PERCENT: f64 = 80.0;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkspaceStorageQuotaSummary {
    pub workspace_id: Uuid,
    pub workspace_type: String,
    pub quota_bytes: u64,
    pub used_bytes: u64,
    pub remaining_bytes: u64,
    pub used_percent: f64,
    pub warning_percent: f64,
    pub warning: bool,
    pub exceeded: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UserStorageQuotaSummary {
    pub user_id: Uuid,
    pub quota_bytes: u64,
    pub used_bytes: u64,
    pub remaining_bytes: u64,
    pub used_percent: f64,
    pub warning_percent: f64,
    pub warning: bool,
    pub exceeded: bool,
    pub personal_workspace_count: u64,
}

pub fn storage_quota_bytes_for_workspace_type(workspace_type: &str) -> u64 {
    if let Some(value) = configured_quota_bytes(env::var(WORKSPACE_STORAGE_QUOTA_BYTES_ENV).ok()) {
        return value;
    }
    match workspace_type.trim() {
        "school" => configured_quota_bytes(env::var(SCHOOL_STORAGE_QUOTA_BYTES_ENV).ok())
            .unwrap_or(DEFAULT_SCHOOL_STORAGE_QUOTA_BYTES),
        _ => configured_quota_bytes(env::var(PERSONAL_STORAGE_QUOTA_BYTES_ENV).ok())
            .unwrap_or(DEFAULT_PERSONAL_STORAGE_QUOTA_BYTES),
    }
}

pub fn user_storage_quota_bytes() -> u64 {
    configured_quota_bytes(env::var(USER_STORAGE_QUOTA_BYTES_ENV).ok())
        .unwrap_or_else(|| storage_quota_bytes_for_workspace_type("personal"))
}

pub fn storage_quota_warning_percent() -> f64 {
    env::var(STORAGE_QUOTA_WARNING_PERCENT_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| *value > 0.0 && *value <= 100.0)
        .unwrap_or(DEFAULT_STORAGE_QUOTA_WARNING_PERCENT)
}

pub fn workspace_storage_quota_summary(
    workspace_id: Uuid,
    workspace_type: &str,
    used_bytes: u64,
) -> WorkspaceStorageQuotaSummary {
    let quota_bytes = storage_quota_bytes_for_workspace_type(workspace_type);
    workspace_storage_quota_summary_with_limits(
        workspace_id,
        workspace_type,
        used_bytes,
        quota_bytes,
        storage_quota_warning_percent(),
    )
}

pub fn user_storage_quota_summary(
    user_id: Uuid,
    used_bytes: u64,
    personal_workspace_count: u64,
) -> UserStorageQuotaSummary {
    let quota_bytes = user_storage_quota_bytes();
    let warning_percent = storage_quota_warning_percent();
    let remaining_bytes = quota_bytes.saturating_sub(used_bytes);
    let used_percent = if quota_bytes == 0 {
        0.0
    } else {
        (used_bytes as f64 / quota_bytes as f64) * 100.0
    };
    UserStorageQuotaSummary {
        user_id,
        quota_bytes,
        used_bytes,
        remaining_bytes,
        used_percent,
        warning_percent,
        warning: used_percent >= warning_percent,
        exceeded: used_bytes >= quota_bytes,
        personal_workspace_count,
    }
}

fn configured_quota_bytes(value: Option<String>) -> Option<u64> {
    value
        .as_deref()
        .map(str::trim)
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
}

fn workspace_storage_quota_summary_with_limits(
    workspace_id: Uuid,
    workspace_type: &str,
    used_bytes: u64,
    quota_bytes: u64,
    warning_percent: f64,
) -> WorkspaceStorageQuotaSummary {
    let remaining_bytes = quota_bytes.saturating_sub(used_bytes);
    let used_percent = if quota_bytes == 0 {
        0.0
    } else {
        (used_bytes as f64 / quota_bytes as f64) * 100.0
    };
    WorkspaceStorageQuotaSummary {
        workspace_id,
        workspace_type: workspace_type.to_string(),
        quota_bytes,
        used_bytes,
        remaining_bytes,
        used_percent,
        warning_percent,
        warning: used_percent >= warning_percent,
        exceeded: used_bytes >= quota_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_storage_quota_summary_reports_usage_state() {
        let workspace_id = Uuid::new_v4();
        let summary =
            workspace_storage_quota_summary_with_limits(workspace_id, "personal", 85, 100, 80.0);

        assert_eq!(summary.workspace_id, workspace_id);
        assert_eq!(summary.quota_bytes, 100);
        assert_eq!(summary.used_bytes, 85);
        assert_eq!(summary.remaining_bytes, 15);
        assert!(summary.warning);
        assert!(!summary.exceeded);
    }

    #[test]
    fn workspace_storage_quota_summary_marks_exceeded_at_limit() {
        let summary =
            workspace_storage_quota_summary_with_limits(Uuid::new_v4(), "school", 100, 100, 80.0);

        assert_eq!(summary.remaining_bytes, 0);
        assert!(summary.warning);
        assert!(summary.exceeded);
    }

    #[test]
    fn user_storage_quota_summary_reports_personal_usage_state() {
        let user_id = Uuid::new_v4();
        let summary = user_storage_quota_summary(user_id, 10, 1);

        assert_eq!(summary.user_id, user_id);
        assert_eq!(summary.used_bytes, 10);
        assert_eq!(summary.personal_workspace_count, 1);
        assert!(summary.quota_bytes > summary.used_bytes);
        assert!(!summary.exceeded);
    }
}
