use sea_orm::{ConnectionTrait, DbBackend, DbErr, Statement};
use serde_json::json;
use uuid::Uuid;

pub use super::generation_budget::ensure_generation_budget_available;
pub use super::generation_cost_reports::list_operator_costs_page;

use crate::{
    models::GenerationJob, repositories::generation_cost_estimates::estimate_generation_cost,
};

pub async fn record_generation_cost_log(
    db: &impl ConnectionTrait,
    job: &GenerationJob,
) -> Result<(), DbErr> {
    let Some(output) = job.output_json.as_ref() else {
        return Ok(());
    };
    let estimate = estimate_generation_cost(job);
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into generation_cost_logs
          (id, workspace_id, generation_job_id, storybook_id, provider, job_type, status,
           estimated_input_units, estimated_output_units, image_count, estimated_cost_micros,
           currency, metadata_json, created_at)
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, now())
        on conflict (generation_job_id, status) do nothing
        "#,
        [
            Uuid::new_v4().into(),
            job.workspace_id.into(),
            job.id.into(),
            job.storybook_id.into(),
            estimate.provider.into(),
            job.job_type.clone().into(),
            job.status.clone().into(),
            estimate.estimated_input_units.into(),
            estimate.estimated_output_units.into(),
            estimate.image_count.into(),
            estimate.estimated_cost_micros.into(),
            estimate.currency.into(),
            json!({
                "schema_version": "generation.cost.estimate.v1",
                "source": "server_estimate",
                "mode": output.get("mode").and_then(|value| value.as_str()).unwrap_or(job.job_type.as_str()),
                "retryable": output
                    .get("error")
                    .and_then(|value| value.get("retryable"))
                    .and_then(|value| value.as_bool())
            })
            .into(),
        ],
    ))
    .await?;
    Ok(())
}
