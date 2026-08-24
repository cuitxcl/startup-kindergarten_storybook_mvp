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
                create table if not exists storybook_assets (
                  id uuid primary key,
                  workspace_id uuid not null,
                  uploaded_by uuid not null,
                  storage_key text not null,
                  original_filename text not null default '',
                  content_type varchar(120) not null,
                  byte_size bigint not null default 0,
                  width integer,
                  height integer,
                  status varchar(40) not null default 'uploaded',
                  processing_message text,
                  visibility_scope varchar(40) not null default 'creation_session',
                  retention_policy varchar(40) not null default 'session_scoped',
                  deleted_at timestamptz,
                  created_at timestamptz not null default now(),
                  updated_at timestamptz not null default now()
                );

                create table if not exists storybook_asset_references (
                  id uuid primary key,
                  workspace_id uuid not null,
                  creation_session_id uuid not null,
                  asset_id uuid not null,
                  kind varchar(40) not null,
                  display_name varchar(80) not null default '',
                  usage varchar(60),
                  status varchar(40) not null default 'awaiting_usage',
                  material_id varchar(80),
                  idempotency_key varchar(120),
                  revoked_at timestamptz,
                  revoked_by uuid,
                  created_at timestamptz not null default now(),
                  updated_at timestamptz not null default now()
                );

                create table if not exists storybook_visual_references (
                  id uuid primary key,
                  workspace_id uuid not null,
                  asset_reference_id uuid not null,
                  generation_job_id uuid,
                  status varchar(40) not null default 'queued',
                  image_storage_key text,
                  failure_reason text,
                  idempotency_key varchar(120),
                  is_active boolean not null default true,
                  confirmed_at timestamptz,
                  confirmed_by uuid,
                  created_at timestamptz not null default now(),
                  updated_at timestamptz not null default now()
                );

                create index if not exists idx_storybook_assets_workspace_status
                  on storybook_assets (workspace_id, status, updated_at desc);

                create index if not exists idx_storybook_assets_cleanup_candidates
                  on storybook_assets (workspace_id, updated_at)
                  where deleted_at is null and visibility_scope = 'creation_session';

                create index if not exists idx_storybook_asset_refs_session
                  on storybook_asset_references (workspace_id, creation_session_id, updated_at desc);

                create index if not exists idx_storybook_asset_refs_asset
                  on storybook_asset_references (workspace_id, asset_id);

                create unique index if not exists uidx_storybook_asset_refs_idempotency
                  on storybook_asset_references (workspace_id, creation_session_id, idempotency_key)
                  where idempotency_key is not null;

                create index if not exists idx_storybook_visual_refs_asset_ref
                  on storybook_visual_references (workspace_id, asset_reference_id, updated_at desc);

                create unique index if not exists uidx_storybook_visual_refs_active
                  on storybook_visual_references (asset_reference_id)
                  where is_active = true;

                create unique index if not exists uidx_storybook_visual_refs_idempotency
                  on storybook_visual_references (workspace_id, asset_reference_id, idempotency_key)
                  where idempotency_key is not null;

                do $$
                begin
                  if not exists (select 1 from pg_constraint where conname = 'fk_storybook_assets_workspace') then
                    alter table storybook_assets
                      add constraint fk_storybook_assets_workspace
                      foreign key (workspace_id) references workspaces(id) on delete cascade not valid;
                  end if;
                  if not exists (select 1 from pg_constraint where conname = 'fk_storybook_assets_uploaded_by') then
                    alter table storybook_assets
                      add constraint fk_storybook_assets_uploaded_by
                      foreign key (uploaded_by) references users(id) on delete restrict not valid;
                  end if;
                  if not exists (select 1 from pg_constraint where conname = 'fk_storybook_asset_refs_workspace') then
                    alter table storybook_asset_references
                      add constraint fk_storybook_asset_refs_workspace
                      foreign key (workspace_id) references workspaces(id) on delete cascade not valid;
                  end if;
                  if not exists (select 1 from pg_constraint where conname = 'fk_storybook_asset_refs_session') then
                    alter table storybook_asset_references
                      add constraint fk_storybook_asset_refs_session
                      foreign key (creation_session_id) references storybook_creation_sessions(id) on delete cascade not valid;
                  end if;
                  if not exists (select 1 from pg_constraint where conname = 'fk_storybook_asset_refs_asset') then
                    alter table storybook_asset_references
                      add constraint fk_storybook_asset_refs_asset
                      foreign key (asset_id) references storybook_assets(id) on delete restrict not valid;
                  end if;
                  if not exists (select 1 from pg_constraint where conname = 'fk_storybook_asset_refs_revoked_by') then
                    alter table storybook_asset_references
                      add constraint fk_storybook_asset_refs_revoked_by
                      foreign key (revoked_by) references users(id) on delete set null not valid;
                  end if;
                  if not exists (select 1 from pg_constraint where conname = 'fk_storybook_visual_refs_workspace') then
                    alter table storybook_visual_references
                      add constraint fk_storybook_visual_refs_workspace
                      foreign key (workspace_id) references workspaces(id) on delete cascade not valid;
                  end if;
                  if not exists (select 1 from pg_constraint where conname = 'fk_storybook_visual_refs_asset_ref') then
                    alter table storybook_visual_references
                      add constraint fk_storybook_visual_refs_asset_ref
                      foreign key (asset_reference_id) references storybook_asset_references(id) on delete cascade not valid;
                  end if;
                  if not exists (select 1 from pg_constraint where conname = 'fk_storybook_visual_refs_generation_job') then
                    alter table storybook_visual_references
                      add constraint fk_storybook_visual_refs_generation_job
                      foreign key (generation_job_id) references generation_jobs(id) on delete set null not valid;
                  end if;
                  if not exists (select 1 from pg_constraint where conname = 'fk_storybook_visual_refs_confirmed_by') then
                    alter table storybook_visual_references
                      add constraint fk_storybook_visual_refs_confirmed_by
                      foreign key (confirmed_by) references users(id) on delete set null not valid;
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
                alter table storybook_visual_references drop constraint if exists fk_storybook_visual_refs_confirmed_by;
                alter table storybook_visual_references drop constraint if exists fk_storybook_visual_refs_generation_job;
                alter table storybook_visual_references drop constraint if exists fk_storybook_visual_refs_asset_ref;
                alter table storybook_visual_references drop constraint if exists fk_storybook_visual_refs_workspace;
                alter table storybook_asset_references drop constraint if exists fk_storybook_asset_refs_revoked_by;
                alter table storybook_asset_references drop constraint if exists fk_storybook_asset_refs_asset;
                alter table storybook_asset_references drop constraint if exists fk_storybook_asset_refs_session;
                alter table storybook_asset_references drop constraint if exists fk_storybook_asset_refs_workspace;
                alter table storybook_assets drop constraint if exists fk_storybook_assets_uploaded_by;
                alter table storybook_assets drop constraint if exists fk_storybook_assets_workspace;
                drop index if exists uidx_storybook_visual_refs_idempotency;
                drop index if exists uidx_storybook_visual_refs_active;
                drop index if exists idx_storybook_visual_refs_asset_ref;
                drop index if exists uidx_storybook_asset_refs_idempotency;
                drop index if exists idx_storybook_asset_refs_asset;
                drop index if exists idx_storybook_asset_refs_session;
                drop index if exists idx_storybook_assets_cleanup_candidates;
                drop index if exists idx_storybook_assets_workspace_status;
                drop table if exists storybook_visual_references;
                drop table if exists storybook_asset_references;
                drop table if exists storybook_assets;
                "#,
            )
            .await?;
        Ok(())
    }
}
