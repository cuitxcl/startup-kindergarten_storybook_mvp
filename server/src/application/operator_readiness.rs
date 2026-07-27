use axum::http::HeaderMap;
use loco_rs::app::AppContext;
#[cfg(feature = "db")]
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde::Serialize;

use crate::{
    application::operator_readiness_checks::{
        ReadinessCheck, app_host_status_for_trial, demo_seed_enabled,
        readiness_auth_token_ttl_status, readiness_check, readiness_generation_provider_config,
        readiness_generation_provider_secrets, readiness_secret_status,
    },
    domains::common,
    error::ApiError,
};

#[cfg(feature = "db")]
use crate::application::operator_readiness_checks::{
    readiness_database_schema, readiness_generation_budget,
};

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

#[derive(Debug, Serialize)]
pub struct ReadinessResponse {
    pub ready: bool,
    pub mode: String,
    pub checks: Vec<ReadinessCheck>,
    pub provider: crate::services::generation_provider::GenerationProviderSummary,
    pub storage: crate::services::storage::StorageSummary,
}

#[cfg(not(feature = "db"))]
fn shared_state(ctx: &AppContext) -> Result<crate::state::SharedState, ApiError> {
    ctx.shared_store
        .get::<crate::state::SharedState>()
        .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))
}
