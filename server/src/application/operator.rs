use axum::http::HeaderMap;
use loco_rs::app::AppContext;
#[cfg(feature = "db")]
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde::Serialize;
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
        return crate::repositories::generation::list_operator_costs_page(&ctx.db, query)
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

pub async fn readiness(
    ctx: &AppContext,
    headers: &HeaderMap,
) -> Result<ReadinessResponse, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_operator_db(ctx, headers).await?;
        let provider =
            crate::services::generation_provider::ConfiguredGenerationProvider::from_env()
                .summary();
        let storage = crate::services::storage::storage_summary();
        let mut checks = Vec::new();

        let database_ok = ctx
            .db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "select 1 as ok",
                [],
            ))
            .await
            .is_ok();
        checks.push(readiness_check(
            "database",
            "数据库连接",
            database_ok,
            if database_ok {
                "PostgreSQL 可查询"
            } else {
                "PostgreSQL 查询失败，请检查 DATABASE_URL 和网络"
            },
        ));

        let (database_schema_ok, database_schema_message) =
            match readiness_database_schema(ctx).await {
                Ok(()) => (true, "核心业务表已完成 migration".to_string()),
                Err(err) => (false, err),
            };
        checks.push(readiness_check(
            "database_schema",
            "数据库结构",
            database_schema_ok,
            &database_schema_message,
        ));

        let app_host = std::env::var("APP_HOST").unwrap_or_else(|_| "http://127.0.0.1".to_string());
        let app_host_status = app_host_status_for_trial(&app_host);
        checks.push(readiness_check(
            "app_host",
            "外部访问域名",
            app_host_status.ok,
            &app_host_status.message,
        ));

        let auth_secret_status = readiness_secret_status(
            "KINDLEAF_AUTH_TOKEN_SECRET",
            std::env::var("KINDLEAF_AUTH_TOKEN_SECRET").ok().as_deref(),
            32,
        );
        checks.push(readiness_check(
            "auth_token",
            "登录令牌密钥",
            auth_secret_status.ok,
            &auth_secret_status.message,
        ));

        let auth_token_ttl = readiness_auth_token_ttl_status(
            std::env::var("KINDLEAF_AUTH_TOKEN_TTL_SECONDS")
                .ok()
                .as_deref(),
        );
        checks.push(readiness_check(
            "auth_token_ttl",
            "登录令牌有效期",
            auth_token_ttl.ok,
            &auth_token_ttl.message,
        ));

        let provider_secrets = readiness_generation_provider_secrets();
        checks.push(readiness_check(
            "generation_provider_secrets",
            "生成 provider 密钥",
            provider_secrets.ok,
            &provider_secrets.message,
        ));

        let provider_config = readiness_generation_provider_config(&provider);
        checks.push(readiness_check(
            "generation_provider_config",
            "生成 provider 配置",
            provider_config.ok,
            &provider_config.message,
        ));

        checks.push(readiness_check(
            "generation_provider",
            "真实生成能力",
            provider.production_ready && provider_secrets.ok && provider_config.ok,
            if provider.production_ready && provider_secrets.ok && provider_config.ok {
                "DeepSeek 文本和 Seedream 图片均已配置"
            } else if !provider_secrets.ok {
                "真实生成 provider key 缺失或仍是占位值"
            } else if !provider_config.ok {
                "真实生成 provider endpoint 或 model 配置不合法"
            } else {
                "真实生成未完整就绪，请配置 DeepSeek 和 Seedream/ARK key"
            },
        ));

        let storage_persistent = !storage.exports_dir.starts_with("tmp/")
            && storage.exports_dir != "tmp"
            && !storage.generated_images_dir.starts_with("tmp/")
            && storage.generated_images_dir != "tmp";
        let storage_writable = crate::services::storage::check_storage_writable();
        let storage_ready = storage_persistent
            && storage.filename_validation
            && storage.size_limit_enabled
            && storage_writable.is_ok();
        let storage_message = if !storage_persistent {
            "当前 storage 使用 tmp 临时目录，试点应改为持久化路径".to_string()
        } else if let Err(err) = &storage_writable {
            format!("storage 目录不可写：{err}")
        } else {
            "PDF 和图片目录已使用非临时路径，且写入探测通过".to_string()
        };
        checks.push(readiness_check(
            "storage",
            "文件存储",
            storage_ready,
            &storage_message,
        ));

        let budget_status = readiness_generation_budget(ctx).await;
        checks.push(readiness_check(
            "generation_budget",
            "生成预算",
            budget_status.ok,
            &budget_status.message,
        ));

        let demo_seed_enabled = demo_seed_enabled();
        checks.push(readiness_check(
            "demo_seed",
            "演示数据开关",
            !demo_seed_enabled,
            if demo_seed_enabled {
                "KINDLEAF_DEMO_SEED 已开启，真实试点数据库不应自动写入演示用户"
            } else {
                "KINDLEAF_DEMO_SEED 未开启"
            },
        ));

        let ready = checks.iter().all(|item| item.ok);
        return Ok(ReadinessResponse {
            ready,
            mode: if ready {
                "trial_ready"
            } else {
                "needs_attention"
            }
            .to_string(),
            checks,
            provider,
            storage,
        });
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_login(&state, headers)?;
        let provider =
            crate::services::generation_provider::ConfiguredGenerationProvider::from_env()
                .summary();
        let storage = crate::services::storage::storage_summary();
        let provider_secrets = readiness_generation_provider_secrets();
        let provider_config = readiness_generation_provider_config(&provider);
        let checks = vec![
            readiness_check("database", "数据库连接", false, "当前为内存 mock 模式"),
            readiness_check(
                "database_schema",
                "数据库结构",
                false,
                "当前为内存 mock 模式",
            ),
            readiness_check(
                "app_host",
                "外部访问域名",
                false,
                "内存 mock 模式不作为外部试点部署",
            ),
            readiness_check(
                "generation_provider_secrets",
                "生成 provider 密钥",
                provider_secrets.ok,
                &provider_secrets.message,
            ),
            readiness_check(
                "generation_provider_config",
                "生成 provider 配置",
                provider_config.ok,
                &provider_config.message,
            ),
            readiness_check(
                "generation_provider",
                "真实生成能力",
                provider.production_ready && provider_secrets.ok && provider_config.ok,
                if provider.production_ready && provider_secrets.ok && provider_config.ok {
                    "DeepSeek 文本和 Seedream 图片均已配置"
                } else if !provider_secrets.ok {
                    "真实生成 provider key 缺失或仍是占位值"
                } else if !provider_config.ok {
                    "真实生成 provider endpoint 或 model 配置不合法"
                } else {
                    "真实生成未完整就绪，请配置 DeepSeek 和 Seedream/ARK key"
                },
            ),
            readiness_check("storage", "文件存储", false, "当前为本地 mock 存储"),
            readiness_check(
                "generation_budget",
                "生成预算上限",
                false,
                "内存 mock 模式不作为试点部署",
            ),
            readiness_check(
                "demo_seed",
                "演示数据开关",
                false,
                "内存 mock 模式不作为真实试点数据库",
            ),
        ];
        Ok(ReadinessResponse {
            ready: false,
            mode: "mock_runtime".to_string(),
            checks,
            provider,
            storage,
        })
    }
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

#[derive(Debug, Serialize)]
pub struct ReadinessResponse {
    pub ready: bool,
    pub mode: String,
    pub checks: Vec<ReadinessCheck>,
    pub provider: crate::services::generation_provider::GenerationProviderSummary,
    pub storage: crate::services::storage::StorageSummary,
}

#[derive(Debug, Serialize)]
pub struct ReadinessCheck {
    pub key: &'static str,
    pub label: &'static str,
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
    ("export_jobs", "last_error"),
];

fn readiness_check(
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

struct ProviderConfigReadiness {
    ok: bool,
    message: String,
}

fn readiness_generation_provider_config(
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
async fn readiness_database_schema(ctx: &AppContext) -> Result<(), String> {
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
async fn readiness_generation_budget(ctx: &AppContext) -> BudgetReadiness {
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

struct BudgetReadiness {
    ok: bool,
    message: String,
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

struct AppHostReadiness {
    ok: bool,
    message: String,
}

fn app_host_status_for_trial(value: &str) -> AppHostReadiness {
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

struct AuthTokenTtlStatus {
    ok: bool,
    message: String,
}

struct SecretReadiness {
    ok: bool,
    message: String,
}

fn readiness_secret_status(label: &str, value: Option<&str>, min_len: usize) -> SecretReadiness {
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

fn readiness_generation_provider_secrets() -> SecretReadiness {
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

fn readiness_auth_token_ttl_status(value: Option<&str>) -> AuthTokenTtlStatus {
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

fn demo_seed_enabled() -> bool {
    matches!(
        std::env::var("KINDLEAF_DEMO_SEED")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(not(feature = "db"))]
fn shared_state(ctx: &AppContext) -> Result<crate::state::SharedState, ApiError> {
    ctx.shared_store
        .get::<crate::state::SharedState>()
        .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))
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
                crate::services::generation_provider::GenerationProviderComponent {
                    kind: "text".to_string(),
                    provider: "deepseek".to_string(),
                    configured: true,
                    ready: true,
                    model: "deepseek-v4-flash".to_string(),
                    endpoint: "https://api.deepseek.com/chat/completions".to_string(),
                    supports: vec![],
                    required_configuration: vec![],
                },
                crate::services::generation_provider::GenerationProviderComponent {
                    kind: "image".to_string(),
                    provider: "seedream".to_string(),
                    configured: true,
                    ready: true,
                    model: "doubao-seedream-5-0-lite".to_string(),
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
            ("generation_jobs".to_string(), "attempt_count".to_string()),
            ("generation_jobs".to_string(), "last_error".to_string()),
        ]);
        assert!(missing.contains(&"export_jobs.last_error".to_string()));
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
