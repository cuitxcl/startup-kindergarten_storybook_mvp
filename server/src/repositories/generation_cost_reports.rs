use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::{
    models::{
        GenerationCostListQuery, GenerationCostLog, GenerationCostReport, GenerationCostSummary,
        PaginationMeta,
    },
    repositories::generation_budget::with_budget_status,
};

pub async fn list_operator_costs_page(
    db: &DatabaseConnection,
    query: GenerationCostListQuery,
) -> Result<(GenerationCostReport, PaginationMeta), DbErr> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0);
    let workspace_id = query.workspace_id;
    let provider = clean_filter(query.provider);
    let job_type = clean_filter(query.job_type);
    let status = clean_filter(query.status);

    let count_row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select count(*) as count
            from generation_cost_logs gcl
            where ($1::uuid is null or gcl.workspace_id = $1)
              and ($2::text is null or gcl.provider = $2)
              and ($3::text is null or gcl.job_type = $3)
              and ($4::text is null or gcl.status = $4)
            "#,
            [
                workspace_id.into(),
                provider.clone().into(),
                job_type.clone().into(),
                status.clone().into(),
            ],
        ))
        .await?;
    let total: i64 = count_row
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);

    let summary = cost_summary(db, workspace_id, &provider, &job_type, &status).await?;
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select
              gcl.id, gcl.workspace_id, w.name as workspace_name,
              gcl.generation_job_id, gcl.storybook_id, s.title as storybook_title,
              gcl.provider, gcl.job_type, gcl.status,
              gcl.estimated_input_units, gcl.estimated_output_units,
              gcl.image_count, gcl.estimated_cost_micros, gcl.currency,
              coalesce(gcl.metadata_json, '{}'::jsonb) as metadata_json,
              gcl.created_at
            from generation_cost_logs gcl
            left join workspaces w on w.id = gcl.workspace_id
            left join storybooks s on s.id = gcl.storybook_id
            where ($1::uuid is null or gcl.workspace_id = $1)
              and ($2::text is null or gcl.provider = $2)
              and ($3::text is null or gcl.job_type = $3)
              and ($4::text is null or gcl.status = $4)
            order by gcl.created_at desc
            limit $5 offset $6
            "#,
            [
                workspace_id.into(),
                provider.into(),
                job_type.into(),
                status.into(),
                (limit as i64).into(),
                (offset as i64).into(),
            ],
        ))
        .await?;
    let items = rows
        .into_iter()
        .map(cost_log_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    let total = total.max(0) as usize;

    Ok((
        GenerationCostReport { summary, items },
        PaginationMeta {
            total,
            limit,
            offset: offset.min(total),
            has_more: offset.saturating_add(limit) < total,
        },
    ))
}

async fn cost_summary(
    db: &DatabaseConnection,
    workspace_id: Option<Uuid>,
    provider: &Option<String>,
    job_type: &Option<String>,
    status: &Option<String>,
) -> Result<GenerationCostSummary, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select
              coalesce(sum(estimated_cost_micros), 0)::bigint as total_cost_micros,
              coalesce(sum(case when status = 'succeeded' then estimated_cost_micros else 0 end), 0)::bigint as succeeded_cost_micros,
              coalesce(sum(case when status = 'failed' then 1 else 0 end), 0)::bigint as failed_jobs,
              count(*)::bigint as total_jobs,
              coalesce(sum(estimated_input_units), 0)::bigint as total_input_units,
              coalesce(sum(estimated_output_units), 0)::bigint as total_output_units,
              coalesce(sum(image_count), 0)::bigint as total_images,
              coalesce(max(currency), 'USD') as currency
            from generation_cost_logs
            where ($1::uuid is null or workspace_id = $1)
              and ($2::text is null or provider = $2)
              and ($3::text is null or job_type = $3)
              and ($4::text is null or status = $4)
            "#,
            [
                workspace_id.into(),
                provider.clone().into(),
                job_type.clone().into(),
                status.clone().into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("generation_cost_summary".to_string()))?;

    Ok(with_budget_status(GenerationCostSummary {
        total_cost_micros: row.try_get("", "total_cost_micros")?,
        succeeded_cost_micros: row.try_get("", "succeeded_cost_micros")?,
        failed_jobs: row.try_get("", "failed_jobs")?,
        total_jobs: row.try_get("", "total_jobs")?,
        total_input_units: row.try_get("", "total_input_units")?,
        total_output_units: row.try_get("", "total_output_units")?,
        total_images: row.try_get("", "total_images")?,
        currency: row.try_get("", "currency")?,
        budget_limit_micros: None,
        budget_used_percent: None,
        budget_warning_percent: None,
        budget_warning: false,
        budget_exceeded: false,
    }))
}

fn clean_filter(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn cost_log_from_row(row: sea_orm::QueryResult) -> Result<GenerationCostLog, DbErr> {
    Ok(GenerationCostLog {
        id: row.try_get("", "id")?,
        workspace_id: row.try_get("", "workspace_id")?,
        workspace_name: row.try_get("", "workspace_name")?,
        generation_job_id: row.try_get("", "generation_job_id")?,
        storybook_id: row.try_get("", "storybook_id")?,
        storybook_title: row.try_get("", "storybook_title")?,
        provider: row.try_get("", "provider")?,
        job_type: row.try_get("", "job_type")?,
        status: row.try_get("", "status")?,
        estimated_input_units: row.try_get("", "estimated_input_units")?,
        estimated_output_units: row.try_get("", "estimated_output_units")?,
        image_count: row.try_get("", "image_count")?,
        estimated_cost_micros: row.try_get("", "estimated_cost_micros")?,
        currency: row.try_get("", "currency")?,
        metadata_json: row.try_get::<JsonValue>("", "metadata_json")?,
        created_at: row.try_get::<DateTime<Utc>>("", "created_at")?,
    })
}
