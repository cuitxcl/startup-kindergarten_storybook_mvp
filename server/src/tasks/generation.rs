use loco_rs::prelude::*;

#[derive(Debug)]
pub struct RequeueGenerationJobs;

#[async_trait]
impl Task for RequeueGenerationJobs {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "requeue_generation_jobs".to_string(),
            detail: "Requeue stale generation jobs and recover locked tasks".to_string(),
        }
    }

    async fn run(&self, app_context: &AppContext, vars: &task::Vars) -> Result<()> {
        let age_minutes = vars
            .cli
            .get("age")
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(15);
        let limit = vars
            .cli
            .get("limit")
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(10);
        #[cfg(feature = "db")]
        {
            let recovered = crate::repositories::generation::process_generation_backlog(
                &app_context.db,
                age_minutes,
                limit,
            )
            .await
            .map_err(loco_rs::Error::from)?;
            println!(
                "processed {recovered} generation job(s) with age threshold {age_minutes} minute(s) and limit {limit}"
            );
        }

        #[cfg(not(feature = "db"))]
        {
            let _ = (app_context, age_minutes, limit);
        }

        Ok(())
    }
}
