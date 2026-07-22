#![allow(dead_code)]

use loco_rs::{
    app::AppContext,
    prelude::{BackgroundWorker, async_trait},
};
use serde::{Deserialize, Serialize};

#[cfg(feature = "db")]
use crate::repositories::delivery;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportJobArgs {
    pub export_id: uuid::Uuid,
}

pub struct ExportWorker {
    pub ctx: AppContext,
}

#[async_trait]
impl BackgroundWorker<ExportJobArgs> for ExportWorker {
    fn queue() -> Option<String> {
        Some("exports".to_string())
    }

    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }

    async fn perform(&self, args: ExportJobArgs) -> loco_rs::Result<()> {
        #[cfg(feature = "db")]
        {
            return delivery::execute_export_job(&self.ctx.db, args.export_id)
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

pub async fn enqueue_export_job(ctx: &AppContext, export_id: uuid::Uuid) -> loco_rs::Result<()> {
    ExportWorker::perform_later(ctx, ExportJobArgs { export_id }).await
}
