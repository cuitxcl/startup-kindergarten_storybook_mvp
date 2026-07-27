use loco_rs::app::AppContext;
#[cfg(feature = "db")]
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ReadinessCheck {
    pub key: &'static str,
    pub label: &'static str,
    pub ok: bool,
    pub message: String,
}

pub(super) struct ProviderConfigReadiness {
    pub ok: bool,
    pub message: String,
}

pub(super) struct BudgetReadiness {
    pub ok: bool,
    pub message: String,
}

pub(super) struct AppHostReadiness {
    pub ok: bool,
    pub message: String,
}

pub(super) struct AuthTokenTtlStatus {
    pub ok: bool,
    pub message: String,
}

pub(super) struct SecretReadiness {
    pub ok: bool,
    pub message: String,
}

const REQUIRED_READINESS_TABLES: &[&str] = &[
    "users",
    "workspaces",
    "workspace_members",
    "classrooms",
    "children",
    "storybooks",
    "storybook_pages",
    "storybook_roles",
    "marketplace_templates",
    "marketplace_submissions",
    "share_links",
    "export_jobs",
    "generation_jobs",
    "audit_logs",
    "parent_intakes",
    "parent_intake_links",
    "generation_cost_logs",
];

const REQUIRED_READINESS_COLUMNS: &[(&str, &str)] = &[
    ("generation_jobs", "created_by"),
    ("generation_jobs", "attempt_count"),
    ("generation_jobs", "last_error"),
    ("generation_jobs", "next_run_at"),
    ("generation_jobs", "locked_by"),
    ("generation_jobs", "locked_at"),
    ("generation_cost_logs", "estimated_cost_micros"),
    ("generation_cost_logs", "metadata_json"),
    ("parent_intake_links", "access_count"),
    ("parent_intake_links", "classroom_id"),
    ("share_links", "access_count"),
    ("share_links", "last_accessed_at"),
    ("export_jobs", "created_by"),
    ("export_jobs", "last_error"),
];

pub(super) fn readiness_check(
    key: &'static str,
    label: &'static str,
    ok: bool,
    message: &str,
) -> ReadinessCheck {
    ReadinessCheck {
        key,
        label,
        ok,
        message: message.to_string(),
    }
}

pub(super) fn readiness_generation_provider_config(
    provider: &crate::services::generation_provider::GenerationProviderSummary,
) -> ProviderConfigReadiness {
    let mut invalid = vec![];
    for component in &provider.components {
        if component.model.trim().is_empty() {
            invalid.push(format!("{} model 为空", component.provider));
        }
        if !readiness_endpoint_ready(&component.endpoint) {
            invalid.push(format!(
                "{} endpoint 不合法：{}",
                component.provider, component.endpoint
            ));
        }
    }

    if invalid.is_empty() {
        ProviderConfigReadiness {
            ok: true,
            message: "DeepSeek/Seedream endpoint 与 model 配置格式正常".to_string(),
        }
    } else {
        ProviderConfigReadiness {
            ok: false,
            message: invalid.join("；"),
        }
    }
}

fn readiness_endpoint_ready(value: &str) -> bool {
    let value = value.trim();
    (value.starts_with("http://") || value.starts_with("https://"))
        && reqwest::Url::parse(value).is_ok()
}

#[cfg(feature = "db")]
pub(super) async fn readiness_database_schema(ctx: &AppContext) -> Result<(), String> {
    let table_rows = ctx
        .db
        .query_all(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            select table_name
            from information_schema.tables
            where table_schema = 'public'
            "#,
            [],
        ))
        .await
        .map_err(|err| format!("无法读取数据库结构：{err}"))?;

    let existing_tables = table_rows
        .into_iter()
        .filter_map(|row| row.try_get::<String>("", "table_name").ok())
        .collect::<Vec<_>>();
    let missing_tables = missing_readiness_tables(&existing_tables);
    if !missing_tables.is_empty() {
        return Err(format!(
            "数据库 migration 不完整，缺少核心表：{}",
            missing_tables.join(", ")
        ));
    }

    let column_rows = ctx
        .db
        .query_all(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            select table_name, column_name
            from information_schema.columns
            where table_schema = 'public'
            "#,
            [],
        ))
        .await
        .map_err(|err| format!("无法读取数据库字段结构：{err}"))?;
    let existing_columns = column_rows
        .into_iter()
        .filter_map(|row| {
            let table = row.try_get::<String>("", "table_name").ok()?;
            let column = row.try_get::<String>("", "column_name").ok()?;
            Some((table, column))
        })
        .collect::<Vec<_>>();
    let missing_columns = missing_readiness_columns(&existing_columns);
    if missing_columns.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "数据库 migration 不完整，缺少核心字段：{}",
            missing_columns.join(", ")
        ))
    }
}

fn missing_readiness_tables(existing: &[String]) -> Vec<&'static str> {
    let existing = existing
        .iter()
        .map(|name| name.as_str())
        .collect::<std::collections::HashSet<_>>();
    REQUIRED_READINESS_TABLES
        .iter()
        .copied()
        .filter(|table| !existing.contains(table))
        .collect()
}

fn missing_readiness_columns(existing: &[(String, String)]) -> Vec<String> {
    let existing = existing
        .iter()
        .map(|(table, column)| (table.as_str(), column.as_str()))
        .collect::<std::collections::HashSet<_>>();
    REQUIRED_READINESS_COLUMNS
        .iter()
        .filter(|item| !existing.contains(*item))
        .map(|(table, column)| format!("{table}.{column}"))
        .collect()
}

#[cfg(feature = "db")]
pub(super) async fn readiness_generation_budget(ctx: &AppContext) -> BudgetReadiness {
    let Some(limit) = readiness_budget_limit_micros() else {
        return BudgetReadiness {
            ok: false,
            message: "未配置 KINDLEAF_COST_BUDGET_LIMIT_MICROS，试点成本没有硬上限".to_string(),
        };
    };

    let used = match ctx
        .db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            select coalesce(sum(estimated_cost_micros), 0)::bigint as succeeded_cost_micros
            from generation_cost_logs
            where status = 'succeeded'
            "#,
            [],
        ))
        .await
    {
        Ok(Some(row)) => row.try_get::<i64>("", "succeeded_cost_micros").unwrap_or(0),
        Ok(None) => 0,
        Err(err) => {
            return BudgetReadiness {
                ok: false,
                message: format!("生成预算读取失败：{err}"),
            };
        }
    };

    readiness_budget_status(limit, used, readiness_budget_warning_percent())
}

fn readiness_budget_status(limit: i64, used: i64, warning_percent: f64) -> BudgetReadiness {
    let used_percent = if limit > 0 {
        (used.max(0) as f64 / limit as f64) * 100.0
    } else {
        0.0
    };
    if used >= limit {
        BudgetReadiness {
            ok: false,
            message: format!(
                "生成预算已达到上限：已用 {} / {} micros，新建生成任务会被拦截",
                used.max(0),
                limit
            ),
        }
    } else if used_percent >= warning_percent {
        BudgetReadiness {
            ok: true,
            message: format!(
                "生成预算已配置，但已使用 {:.1}%（预警线 {:.1}%），试点前请确认额度",
                used_percent, warning_percent
            ),
        }
    } else {
        BudgetReadiness {
            ok: true,
            message: format!(
                "生成预算已配置：已用 {:.1}%（{} / {} micros）",
                used_percent,
                used.max(0),
                limit
            ),
        }
    }
}

#[cfg(feature = "db")]
fn readiness_budget_limit_micros() -> Option<i64> {
    std::env::var("KINDLEAF_COST_BUDGET_LIMIT_MICROS")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value > 0)
}

#[cfg(feature = "db")]
fn readiness_budget_warning_percent() -> f64 {
    std::env::var("KINDLEAF_COST_BUDGET_WARNING_PERCENT")
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| *value > 0.0)
        .unwrap_or(80.0)
        .clamp(1.0, 100.0)
}

pub(super) fn app_host_status_for_trial(value: &str) -> AppHostReadiness {
    let value = value.trim().to_ascii_lowercase();
    if !value.starts_with("https://") {
        return AppHostReadiness {
            ok: false,
            message: "APP_HOST 不是 HTTPS，试点前应配置真实 HTTPS 域名".to_string(),
        };
    }
    if value.contains("localhost") || value.contains("127.0.0.1") || value.contains("0.0.0.0") {
        return AppHostReadiness {
            ok: false,
            message: "APP_HOST 仍是本地地址，试点前应配置真实 HTTPS 域名".to_string(),
        };
    }
    if value.contains("://example.com")
        || value.contains(".example.com")
        || value.contains("://example.org")
        || value.contains(".example.org")
        || value.contains("://example.net")
        || value.contains(".example.net")
    {
        return AppHostReadiness {
            ok: false,
            message: "APP_HOST 仍是 example 占位域名，请替换为真实试点域名".to_string(),
        };
    }
    AppHostReadiness {
        ok: true,
        message: "APP_HOST 已使用非本地 HTTPS 地址".to_string(),
    }
}

#[cfg(test)]
fn app_host_ready_for_trial(value: &str) -> bool {
    app_host_status_for_trial(value).ok
}

const DEFAULT_AUTH_TOKEN_TTL_SECONDS: i64 = 7 * 24 * 60 * 60;
const MAX_AUTH_TOKEN_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;

pub(super) fn readiness_secret_status(
    label: &str,
    value: Option<&str>,
    min_len: usize,
) -> SecretReadiness {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return SecretReadiness {
            ok: false,
            message: format!("{label} 未配置"),
        };
    };
    if readiness_secret_looks_like_placeholder(value) {
        return SecretReadiness {
            ok: false,
            message: format!("{label} 仍像占位值，请替换为真实试点密钥"),
        };
    }
    if value.len() < min_len {
        return SecretReadiness {
            ok: false,
            message: format!("{label} 长度不足，至少需要 {min_len} 个字符"),
        };
    }
    SecretReadiness {
        ok: true,
        message: format!("{label} 已配置"),
    }
}

pub(super) fn readiness_generation_provider_secrets() -> SecretReadiness {
    let deepseek = readiness_secret_status(
        "DEEPSEEK_API_KEY",
        std::env::var("DEEPSEEK_API_KEY").ok().as_deref(),
        1,
    );
    let seedream_value = std::env::var("SEEDREAM_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| std::env::var("ARK_API_KEY").ok());
    let seedream = readiness_secret_status(
        "SEEDREAM_API_KEY 或 ARK_API_KEY",
        seedream_value.as_deref(),
        1,
    );
    if deepseek.ok && seedream.ok {
        SecretReadiness {
            ok: true,
            message: "DeepSeek 和 Seedream/ARK 密钥已配置".to_string(),
        }
    } else {
        SecretReadiness {
            ok: false,
            message: [deepseek.message, seedream.message].join("；"),
        }
    }
}

fn readiness_secret_looks_like_placeholder(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase().replace(['_', ' '], "-");
    normalized.starts_with("your-")
        || normalized.starts_with("replace-")
        || normalized.starts_with("placeholder-")
        || normalized.ends_with("-placeholder")
        || matches!(
            normalized.as_str(),
            "api-key"
                | "test-key"
                | "demo-key"
                | "example-key"
                | "change-me"
                | "changeme"
                | "xxx"
                | "xxxx"
                | "placeholder"
        )
}

pub(super) fn readiness_auth_token_ttl_status(value: Option<&str>) -> AuthTokenTtlStatus {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None => AuthTokenTtlStatus {
            ok: true,
            message: format!(
                "未配置 KINDLEAF_AUTH_TOKEN_TTL_SECONDS，使用默认 {DEFAULT_AUTH_TOKEN_TTL_SECONDS} 秒（7 天）"
            ),
        },
        Some(raw) => match raw.parse::<i64>() {
            Ok(ttl) if ttl > 0 && ttl <= MAX_AUTH_TOKEN_TTL_SECONDS => AuthTokenTtlStatus {
                ok: true,
                message: format!("KINDLEAF_AUTH_TOKEN_TTL_SECONDS={ttl} 秒"),
            },
            Ok(ttl) if ttl > MAX_AUTH_TOKEN_TTL_SECONDS => AuthTokenTtlStatus {
                ok: false,
                message: format!(
                    "登录 token 有效期过长：{ttl} 秒；试点建议不超过 {MAX_AUTH_TOKEN_TTL_SECONDS} 秒（30 天）"
                ),
            },
            _ => AuthTokenTtlStatus {
                ok: false,
                message: format!(
                    "KINDLEAF_AUTH_TOKEN_TTL_SECONDS 必须是 1 到 {MAX_AUTH_TOKEN_TTL_SECONDS} 之间的秒数"
                ),
            },
        },
    }
}

pub(super) fn demo_seed_enabled() -> bool {
    matches!(
        std::env::var("KINDLEAF_DEMO_SEED")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_trial_host_requires_https_and_non_local() {
        assert!(app_host_ready_for_trial("https://trial.kindleaf.example"));
        assert!(!app_host_ready_for_trial("https://kindleaf.example.com"));
        assert!(!app_host_ready_for_trial("https://trial.example.org"));
        assert!(!app_host_ready_for_trial("https://trial.example.net"));
        assert!(!app_host_ready_for_trial("http://trial.kindleaf.example"));
        assert!(!app_host_ready_for_trial("https://localhost:8080"));
        assert!(!app_host_ready_for_trial("https://127.0.0.1:8080"));
        assert!(!app_host_ready_for_trial("https://0.0.0.0:8080"));
    }

    #[test]
    fn readiness_trial_host_reports_specific_failure_reason() {
        assert!(
            app_host_status_for_trial("http://trial.kindleaf.example")
                .message
                .contains("不是 HTTPS")
        );
        assert!(
            app_host_status_for_trial("https://127.0.0.1:8080")
                .message
                .contains("本地地址")
        );
        assert!(
            app_host_status_for_trial("https://kindleaf.example.com")
                .message
                .contains("占位域名")
        );
    }

    #[test]
    fn readiness_auth_token_ttl_accepts_default_and_reasonable_values() {
        assert!(readiness_auth_token_ttl_status(None).ok);
        assert!(readiness_auth_token_ttl_status(Some("604800")).ok);
        assert!(readiness_auth_token_ttl_status(Some("2592000")).ok);
    }

    #[test]
    fn readiness_auth_token_ttl_rejects_invalid_or_too_long_values() {
        assert!(!readiness_auth_token_ttl_status(Some("0")).ok);
        assert!(!readiness_auth_token_ttl_status(Some("-1")).ok);
        assert!(!readiness_auth_token_ttl_status(Some("abc")).ok);
        assert!(!readiness_auth_token_ttl_status(Some("2592001")).ok);
    }

    #[test]
    fn readiness_secret_status_rejects_missing_short_or_placeholder_values() {
        assert!(!readiness_secret_status("DEEPSEEK_API_KEY", None, 1).ok);
        assert!(!readiness_secret_status("KINDLEAF_AUTH_TOKEN_SECRET", Some("short"), 32).ok);
        assert!(!readiness_secret_status("DEEPSEEK_API_KEY", Some("your-api-key"), 1).ok);
        assert!(readiness_secret_status("DEEPSEEK_API_KEY", Some("sk-realistic-123456"), 1).ok);
    }

    #[test]
    fn readiness_secret_placeholder_detection_normalizes_values() {
        assert!(readiness_secret_looks_like_placeholder(" YOUR_API_KEY "));
        assert!(readiness_secret_looks_like_placeholder("replace_me"));
        assert!(readiness_secret_looks_like_placeholder(
            "replace-with-real-deepseek-key"
        ));
        assert!(readiness_secret_looks_like_placeholder(
            "replace-with-openssl-rand-base64-48"
        ));
        assert!(!readiness_secret_looks_like_placeholder(
            "sk-deepseek-smoke-8f2a7c3d"
        ));
    }

    #[test]
    fn readiness_generation_provider_config_validates_endpoint_and_model() {
        let summary = crate::services::generation_provider::GenerationProviderSummary {
            provider: "deepseek+seedream".to_string(),
            mode: "composite".to_string(),
            schema_version: "generation.provider.v1".to_string(),
            requires_api_key: true,
            supports_text: vec![],
            supports_image: vec![],
            real_text_ready: true,
            real_image_ready: true,
            production_ready: true,
            missing_configuration: vec![],
            components: vec![
                crate::services::generation_provider_contract::GenerationProviderComponent {
                    kind: "text".to_string(),
                    provider: "deepseek".to_string(),
                    configured: true,
                    ready: true,
                    model: "deepseek-v4-flash".to_string(),
                    endpoint: "https://api.deepseek.com/chat/completions".to_string(),
                    supports: vec![],
                    required_configuration: vec![],
                },
                crate::services::generation_provider_contract::GenerationProviderComponent {
                    kind: "image".to_string(),
                    provider: "seedream".to_string(),
                    configured: true,
                    ready: true,
                    model: "doubao-seedream-5-0-260128".to_string(),
                    endpoint: "https://ark.cn-beijing.volces.com/api/v3/images/generations"
                        .to_string(),
                    supports: vec![],
                    required_configuration: vec![],
                },
            ],
            diagnostic: "ready".to_string(),
        };
        assert!(readiness_generation_provider_config(&summary).ok);

        let mut invalid_summary = summary.clone();
        invalid_summary.components[0].endpoint = "api.deepseek.com/chat/completions".to_string();
        invalid_summary.components[1].model = " ".to_string();
        assert!(!readiness_generation_provider_config(&invalid_summary).ok);
    }

    #[test]
    fn readiness_schema_reports_missing_core_tables() {
        let all_tables = REQUIRED_READINESS_TABLES
            .iter()
            .map(|table| (*table).to_string())
            .collect::<Vec<_>>();
        assert!(missing_readiness_tables(&all_tables).is_empty());

        let missing = missing_readiness_tables(&["users".to_string(), "workspaces".to_string()]);
        assert!(missing.contains(&"storybooks"));
        assert!(missing.contains(&"generation_cost_logs"));
    }

    #[test]
    fn readiness_schema_reports_missing_core_columns() {
        let all_columns = REQUIRED_READINESS_COLUMNS
            .iter()
            .map(|(table, column)| ((*table).to_string(), (*column).to_string()))
            .collect::<Vec<_>>();
        assert!(missing_readiness_columns(&all_columns).is_empty());

        let missing = missing_readiness_columns(&[
            ("generation_jobs".to_string(), "created_by".to_string()),
            ("generation_jobs".to_string(), "attempt_count".to_string()),
            ("generation_jobs".to_string(), "last_error".to_string()),
        ]);
        assert!(missing.contains(&"export_jobs.last_error".to_string()));
        assert!(missing.contains(&"export_jobs.created_by".to_string()));
        assert!(missing.contains(&"generation_cost_logs.estimated_cost_micros".to_string()));
    }

    #[test]
    fn readiness_budget_status_flags_warning_and_exceeded() {
        let healthy = readiness_budget_status(100, 20, 80.0);
        assert!(healthy.ok);
        assert!(healthy.message.contains("已用 20.0%"));

        let warning = readiness_budget_status(100, 85, 80.0);
        assert!(warning.ok);
        assert!(warning.message.contains("预警线 80.0%"));

        let exceeded = readiness_budget_status(100, 100, 80.0);
        assert!(!exceeded.ok);
        assert!(exceeded.message.contains("已达到上限"));
    }
}
