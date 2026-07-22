use loco_rs::{
    Result,
    app::{AppContext, Hooks},
    boot::{BootResult, StartMode, create_app},
    config::Config,
    controller::AppRoutes,
    environment::{Environment, resolve_from_env},
    prelude::BackgroundWorker,
    prelude::{Queue, async_trait},
    task::Tasks,
};
use tower_http::cors::CorsLayer;

use crate::controllers::routes;
#[cfg(not(feature = "db"))]
use crate::state::seed_state;

#[cfg(feature = "db")]
use migration::Migrator;

pub struct App;

#[async_trait]
impl Hooks for App {
    fn app_name() -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    async fn boot(
        mode: StartMode,
        environment: &Environment,
        config: Config,
    ) -> Result<BootResult> {
        #[cfg(feature = "db")]
        {
            return create_app::<Self, Migrator>(mode, environment, config).await;
        }

        #[cfg(not(feature = "db"))]
        create_app::<Self>(mode, environment, config).await
    }

    fn routes(_ctx: &AppContext) -> AppRoutes {
        AppRoutes::with_default_routes().add_routes(routes::routes())
    }

    async fn after_context(ctx: AppContext) -> Result<AppContext> {
        #[cfg(feature = "db")]
        {
            if demo_seed_enabled() {
                seed_demo_data(&ctx).await?;
            }
        }

        #[cfg(not(feature = "db"))]
        ctx.shared_store.insert(seed_state());
        Ok(ctx)
    }

    async fn after_routes(router: axum::Router, _ctx: &AppContext) -> Result<axum::Router> {
        Ok(router.layer(CorsLayer::permissive()))
    }

    async fn connect_workers(_ctx: &AppContext, _queue: &Queue) -> Result<()> {
        _queue
            .register(crate::workers::generation::GenerationWorker::build(_ctx))
            .await?;
        _queue
            .register(crate::workers::generation::GenerationPageImageWorker::build(_ctx))
            .await?;
        _queue
            .register(crate::workers::export::ExportWorker::build(_ctx))
            .await?;
        Ok(())
    }

    fn register_tasks(tasks: &mut Tasks) {
        tasks.register(crate::tasks::generation::RequeueGenerationJobs);
    }

    #[cfg(feature = "db")]
    async fn truncate(_ctx: &AppContext) -> Result<()> {
        Ok(())
    }

    #[cfg(feature = "db")]
    async fn seed(ctx: &AppContext, _path: &std::path::Path) -> Result<()> {
        seed_demo_data(ctx).await?;
        Ok(())
    }
}

#[cfg(feature = "db")]
async fn seed_demo_data(ctx: &AppContext) -> Result<()> {
    crate::repositories::auth::seed_demo_account(&ctx.db).await?;
    crate::repositories::organization::seed_demo_organization(&ctx.db).await?;
    crate::repositories::children::seed_demo_children(&ctx.db).await?;
    crate::repositories::storybooks::seed_demo_storybooks(&ctx.db).await?;
    crate::repositories::market::seed_demo_marketplace(&ctx.db).await?;
    Ok(())
}

fn demo_seed_enabled() -> bool {
    let env = Environment::from(resolve_from_env());
    demo_seed_enabled_for(
        &env,
        &std::env::var("KINDLEAF_DEMO_SEED").unwrap_or_default(),
    )
}

fn demo_seed_enabled_for(environment: &Environment, seed_flag: &str) -> bool {
    if !matches!(environment, Environment::Development | Environment::Test) {
        return false;
    }
    matches!(
        seed_flag.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_seed_is_off_by_default_when_flag_is_missing() {
        assert!(!demo_seed_enabled_for(&Environment::Development, ""));
    }

    #[test]
    fn demo_seed_is_on_only_for_supported_truthy_values() {
        for value in ["1", "true", "yes", "on"] {
            assert!(
                demo_seed_enabled_for(&Environment::Development, value),
                "expected {value} to enable demo seed"
            );
        }

        assert!(!demo_seed_enabled_for(&Environment::Development, "false"));
        assert!(!demo_seed_enabled_for(&Environment::Production, "1"));
    }
}
