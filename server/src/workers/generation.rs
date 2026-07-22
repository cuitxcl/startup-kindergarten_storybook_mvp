#![allow(dead_code)]

use loco_rs::{
    app::AppContext,
    prelude::{BackgroundWorker, async_trait},
};
use serde::{Deserialize, Serialize};

#[cfg(feature = "db")]
use crate::repositories::generation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationJobArgs {
    pub workspace_id: uuid::Uuid,
    pub job_id: uuid::Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationPageImageArgs {
    pub workspace_id: uuid::Uuid,
    pub job_id: uuid::Uuid,
}

pub struct GenerationWorker {
    pub ctx: AppContext,
}

#[async_trait]
impl BackgroundWorker<GenerationJobArgs> for GenerationWorker {
    fn queue() -> Option<String> {
        Some("generation".to_string())
    }

    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }

    async fn perform(&self, args: GenerationJobArgs) -> loco_rs::Result<()> {
        #[cfg(feature = "db")]
        {
            return generation::execute_generation_job(
                &self.ctx.db,
                args.workspace_id,
                args.job_id,
            )
            .await
            .map(|_| ())
            .map_err(Into::into);
        }

        #[cfg(not(feature = "db"))]
        {
            let _ = args;
            Ok(())
        }
    }
}

pub struct GenerationPageImageWorker {
    pub ctx: AppContext,
}

#[async_trait]
impl BackgroundWorker<GenerationPageImageArgs> for GenerationPageImageWorker {
    fn queue() -> Option<String> {
        Some("generation".to_string())
    }

    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }

    async fn perform(&self, args: GenerationPageImageArgs) -> loco_rs::Result<()> {
        #[cfg(feature = "db")]
        {
            return generation::execute_generation_job(
                &self.ctx.db,
                args.workspace_id,
                args.job_id,
            )
            .await
            .map(|_| ())
            .map_err(Into::into);
        }

        #[cfg(not(feature = "db"))]
        {
            let _ = args;
            Ok(())
        }
    }
}

pub async fn enqueue_generation_job(
    ctx: &AppContext,
    workspace_id: uuid::Uuid,
    job_id: uuid::Uuid,
) -> loco_rs::Result<()> {
    GenerationWorker::perform_later(
        ctx,
        GenerationJobArgs {
            workspace_id,
            job_id,
        },
    )
    .await
}

pub async fn enqueue_generation_page_image_job(
    ctx: &AppContext,
    workspace_id: uuid::Uuid,
    job_id: uuid::Uuid,
) -> loco_rs::Result<()> {
    GenerationPageImageWorker::perform_later(
        ctx,
        GenerationPageImageArgs {
            workspace_id,
            job_id,
        },
    )
    .await
}
