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
                  add column if not exists locked_by text null,
                  add column if not exists locked_at timestamptz null;

                create index if not exists idx_generation_jobs_claimable
                  on generation_jobs (status, next_run_at, created_at)
                  where status in ('queued', 'failed');

                create index if not exists idx_generation_jobs_locked
                  on generation_jobs (locked_at)
                  where locked_at is not null;
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
                drop index if exists idx_generation_jobs_locked;
                drop index if exists idx_generation_jobs_claimable;

                alter table generation_jobs
                  drop column if exists locked_at,
                  drop column if exists locked_by;
                "#,
            )
            .await?;
        Ok(())
    }
}
