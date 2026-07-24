use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use uuid::Uuid;

use crate::models::GenerationCostSummary;

pub async fn ensure_generation_budget_available(
    db: &DatabaseConnection,
    workspace_id: Option<Uuid>,
) -> Result<(), DbErr> {
    let Some(limit) = budget_limit_micros() else {
        return Ok(());
    };
    let used = succeeded_cost_micros(db, workspace_id).await?;
    if used >= limit {
        return Err(DbErr::Custom(format!(
            "generation_budget_exceeded: 生成预算已用尽，当前已用 {used} micros，预算上限 {limit} micros"
        )));
    }
    Ok(())
}

async fn succeeded_cost_micros(
    db: &DatabaseConnection,
    workspace_id: Option<Uuid>,
) -> Result<i64, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select coalesce(sum(estimated_cost_micros), 0)::bigint as succeeded_cost_micros
            from generation_cost_logs
            where status = 'succeeded'
              and ($1::uuid is null or workspace_id = $1)
            "#,
            [workspace_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("generation_cost_budget".to_string()))?;
    row.try_get("", "succeeded_cost_micros")
}

pub(crate) fn with_budget_status(summary: GenerationCostSummary) -> GenerationCostSummary {
    with_budget_limit(summary, budget_limit_micros())
}

pub(crate) fn with_budget_limit(
    mut summary: GenerationCostSummary,
    limit: Option<i64>,
) -> GenerationCostSummary {
    let Some(limit) = limit else {
        return summary;
    };
    summary.budget_limit_micros = Some(limit);
    let used_percent = if limit > 0 {
        (summary.succeeded_cost_micros.max(0) as f64 / limit as f64) * 100.0
    } else {
        0.0
    };
    let warning_percent = budget_warning_percent();
    summary.budget_used_percent = Some(used_percent);
    summary.budget_warning_percent = Some(warning_percent);
    summary.budget_warning = used_percent >= warning_percent;
    summary.budget_exceeded = summary.succeeded_cost_micros >= limit;
    summary
}

pub(crate) fn budget_limit_micros() -> Option<i64> {
    std::env::var("KINDLEAF_COST_BUDGET_LIMIT_MICROS")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value > 0)
}

fn budget_warning_percent() -> f64 {
    std::env::var("KINDLEAF_COST_BUDGET_WARNING_PERCENT")
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| *value > 0.0)
        .unwrap_or(80.0)
        .clamp(1.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_status_marks_equal_limit_as_exceeded() {
        let summary = with_budget_limit(
            GenerationCostSummary {
                total_cost_micros: 100,
                succeeded_cost_micros: 100,
                failed_jobs: 0,
                total_jobs: 1,
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
            Some(100),
        );

        assert_eq!(summary.budget_limit_micros, Some(100));
        assert_eq!(summary.budget_used_percent, Some(100.0));
        assert_eq!(summary.budget_warning_percent, Some(80.0));
        assert!(summary.budget_warning);
        assert!(summary.budget_exceeded);
    }

    #[test]
    fn budget_status_warns_before_limit_is_exceeded() {
        let summary = with_budget_limit(
            GenerationCostSummary {
                total_cost_micros: 80,
                succeeded_cost_micros: 80,
                failed_jobs: 0,
                total_jobs: 1,
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
            Some(100),
        );

        assert_eq!(summary.budget_used_percent, Some(80.0));
        assert!(summary.budget_warning);
        assert!(!summary.budget_exceeded);
    }
}
