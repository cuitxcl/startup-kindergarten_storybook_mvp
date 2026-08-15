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
                create table if not exists storybook_creation_sessions (
                  id uuid primary key,
                  workspace_id uuid not null,
                  created_by uuid not null,
                  status varchar(40) not null default 'draft',
                  quick_idea text not null,
                  use_scene varchar(80) not null default '',
                  age_group varchar(40) not null default '',
                  page_count integer not null default 6,
                  understanding_json jsonb not null default '{}'::jsonb,
                  materials_json jsonb not null default '[]'::jsonb,
                  directions_json jsonb not null default '[]'::jsonb,
                  selected_direction_id varchar(80),
                  outline_json jsonb,
                  visual_preferences_json jsonb not null default '{}'::jsonb,
	                  storybook_id uuid,
	                  last_job_id uuid,
	                  idempotency_key varchar(120),
	                  generation_summary_json jsonb not null default '{"text_generation_status":"not_started","image_generation_status":"not_started","recoverable_actions":[]}'::jsonb,
	                  requires_understanding_refresh boolean not null default false,
                  requires_direction_refresh boolean not null default false,
                  requires_outline_refresh boolean not null default false,
	                  created_at timestamptz not null default now(),
	                  updated_at timestamptz not null default now()
	                );

	                alter table storybook_creation_sessions
	                  add column if not exists generation_summary_json jsonb not null
	                  default '{"text_generation_status":"not_started","image_generation_status":"not_started","recoverable_actions":[]}'::jsonb;

	                create index if not exists idx_creation_sessions_workspace_status_updated
                  on storybook_creation_sessions (workspace_id, status, updated_at desc);

                create index if not exists idx_creation_sessions_workspace_creator_status_updated
                  on storybook_creation_sessions (workspace_id, created_by, status, updated_at desc);

                create index if not exists idx_creation_sessions_storybook
                  on storybook_creation_sessions (workspace_id, storybook_id)
                  where storybook_id is not null;

	                create unique index if not exists uidx_creation_sessions_storybook
	                  on storybook_creation_sessions (workspace_id, storybook_id)
	                  where storybook_id is not null;

	                create unique index if not exists uidx_creation_sessions_idempotency
	                  on storybook_creation_sessions (workspace_id, idempotency_key)
	                  where idempotency_key is not null;

	                do $$
	                begin
	                  if not exists (select 1 from pg_constraint where conname = 'fk_creation_sessions_workspace') then
	                    alter table storybook_creation_sessions
	                      add constraint fk_creation_sessions_workspace
	                      foreign key (workspace_id) references workspaces(id) on delete cascade not valid;
	                  end if;
	                  if not exists (select 1 from pg_constraint where conname = 'fk_creation_sessions_creator') then
	                    alter table storybook_creation_sessions
	                      add constraint fk_creation_sessions_creator
	                      foreign key (created_by) references users(id) on delete restrict not valid;
	                  end if;
	                  if not exists (select 1 from pg_constraint where conname = 'fk_creation_sessions_storybook') then
	                    alter table storybook_creation_sessions
	                      add constraint fk_creation_sessions_storybook
	                      foreign key (storybook_id) references storybooks(id) on delete set null not valid;
	                  end if;
	                  if not exists (select 1 from pg_constraint where conname = 'fk_creation_sessions_last_job') then
	                    alter table storybook_creation_sessions
	                      add constraint fk_creation_sessions_last_job
	                      foreign key (last_job_id) references generation_jobs(id) on delete set null not valid;
	                  end if;
	                end $$;
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
	                alter table storybook_creation_sessions drop constraint if exists fk_creation_sessions_last_job;
	                alter table storybook_creation_sessions drop constraint if exists fk_creation_sessions_storybook;
	                alter table storybook_creation_sessions drop constraint if exists fk_creation_sessions_creator;
	                alter table storybook_creation_sessions drop constraint if exists fk_creation_sessions_workspace;
	                drop index if exists uidx_creation_sessions_idempotency;
	                drop index if exists uidx_creation_sessions_storybook;
	                drop index if exists idx_creation_sessions_storybook;
                drop index if exists idx_creation_sessions_workspace_creator_status_updated;
                drop index if exists idx_creation_sessions_workspace_status_updated;
                drop table if exists storybook_creation_sessions;
                "#,
            )
            .await?;
        Ok(())
    }
}
