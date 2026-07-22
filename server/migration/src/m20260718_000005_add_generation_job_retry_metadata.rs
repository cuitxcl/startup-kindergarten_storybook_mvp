use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                alter table generation_jobs
                  add column if not exists attempt_count integer not null default 0,
                  add column if not exists last_error text null,
                  add column if not exists next_run_at timestamptz null;

                create index if not exists idx_generation_jobs_retry_ready
                  on generation_jobs (status, next_run_at, created_at)
                  where status = 'failed';
                "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                drop index if exists idx_generation_jobs_retry_ready;

                alter table generation_jobs
                  drop column if exists next_run_at,
                  drop column if exists last_error,
                  drop column if exists attempt_count;
                "#,
            )
            .await?;
        Ok(())
    }
}
