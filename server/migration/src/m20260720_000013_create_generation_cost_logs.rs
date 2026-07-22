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
                create table if not exists generation_cost_logs (
                  id uuid primary key,
                  workspace_id uuid not null,
                  generation_job_id uuid not null,
                  storybook_id uuid null,
                  provider text not null,
                  job_type text not null,
                  status text not null,
                  estimated_input_units integer not null default 0,
                  estimated_output_units integer not null default 0,
                  image_count integer not null default 0,
                  estimated_cost_micros bigint not null default 0,
                  currency text not null default 'USD',
                  metadata_json jsonb null,
                  created_at timestamptz not null default now()
                );

                create unique index if not exists uidx_generation_cost_logs_job_status
                  on generation_cost_logs (generation_job_id, status);

                create index if not exists idx_generation_cost_logs_workspace_created
                  on generation_cost_logs (workspace_id, created_at desc);

                create index if not exists idx_generation_cost_logs_provider_job_type
                  on generation_cost_logs (provider, job_type, created_at desc);
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
                drop index if exists idx_generation_cost_logs_provider_job_type;
                drop index if exists idx_generation_cost_logs_workspace_created;
                drop index if exists uidx_generation_cost_logs_job_status;
                drop table if exists generation_cost_logs;
                "#,
            )
            .await?;
        Ok(())
    }
}
