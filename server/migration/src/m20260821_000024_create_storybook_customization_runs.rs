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
                create table if not exists storybook_customization_runs (
                  id uuid primary key,
                  workspace_id uuid not null references workspaces(id) on delete cascade,
                  source_storybook_id uuid not null references storybooks(id) on delete cascade,
                  created_by uuid not null references users(id),
                  entry_type text not null default 'from_storybook',
                  mode text not null check (mode in ('single', 'batch')),
                  status text not null check (status in ('queued', 'running', 'succeeded', 'failed', 'canceled')),
                  customization_plan jsonb null,
                  source_snapshot jsonb null,
                  requested_count integer not null default 1,
                  succeeded_count integer not null default 0,
                  failed_count integer not null default 0,
                  failure_reason text null,
                  created_at timestamptz not null default now(),
                  updated_at timestamptz not null default now(),
                  completed_at timestamptz null
                );

                create index if not exists idx_storybook_customization_runs_workspace_source
                  on storybook_customization_runs(workspace_id, source_storybook_id, created_at desc);

                create index if not exists idx_storybook_customization_runs_status
                  on storybook_customization_runs(workspace_id, status, created_at desc);

                create unique index if not exists uq_storybook_customization_runs_active_identity
                  on storybook_customization_runs(
                    workspace_id,
                    source_storybook_id,
                    mode,
                    requested_count,
                    md5(coalesce(customization_plan::text, 'null'))
                  )
                  where status in ('queued', 'running');

                create table if not exists storybook_customization_run_items (
                  id uuid primary key,
                  workspace_id uuid not null references workspaces(id) on delete cascade,
                  run_id uuid not null references storybook_customization_runs(id) on delete cascade,
                  source_storybook_id uuid not null references storybooks(id) on delete cascade,
                  target_child_id uuid not null references children(id),
                  output_storybook_id uuid null references storybooks(id) on delete set null,
                  primary_material text null,
                  status text not null check (status in ('queued', 'running', 'succeeded', 'failed', 'canceled', 'retrying')),
                  generation_input_snapshot jsonb not null default '{}'::jsonb,
                  failure_reason text null,
                  created_at timestamptz not null default now(),
                  updated_at timestamptz not null default now(),
                  completed_at timestamptz null
                );

                create index if not exists idx_storybook_customization_run_items_run
                  on storybook_customization_run_items(run_id, created_at);

                create index if not exists idx_storybook_customization_run_items_child
                  on storybook_customization_run_items(workspace_id, target_child_id, created_at desc);

                create index if not exists idx_storybook_customization_run_items_output
                  on storybook_customization_run_items(output_storybook_id)
                  where output_storybook_id is not null;
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
                drop table if exists storybook_customization_run_items;
                drop table if exists storybook_customization_runs;
                "#,
            )
            .await?;
        Ok(())
    }
}
