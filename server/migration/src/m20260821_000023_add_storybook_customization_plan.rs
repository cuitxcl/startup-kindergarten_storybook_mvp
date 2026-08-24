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
                alter table storybooks
                  add column if not exists customization_plan jsonb null;

                alter table storybook_creation_sessions
                  add column if not exists entry_type varchar(40) not null default 'direct_create',
                  add column if not exists source_storybook_id uuid null;

                create index if not exists idx_storybooks_customization_plan_gin
                  on storybooks using gin (customization_plan)
                  where customization_plan is not null;

                create index if not exists idx_creation_sessions_source_assets
                  on storybook_creation_sessions (workspace_id, created_by, source_storybook_id, updated_at desc)
                  where entry_type = 'from_storybook_assets' and status not in ('storybook_ready', 'abandoned');

                do $$
                begin
                  if not exists (select 1 from pg_constraint where conname = 'fk_creation_sessions_source_storybook') then
                    alter table storybook_creation_sessions
                      add constraint fk_creation_sessions_source_storybook
                      foreign key (source_storybook_id) references storybooks(id) on delete set null not valid;
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
                drop index if exists idx_storybooks_customization_plan_gin;
                drop index if exists idx_creation_sessions_source_assets;

                alter table storybooks
                  drop column if exists customization_plan;

                alter table storybook_creation_sessions
                  drop constraint if exists fk_creation_sessions_source_storybook,
                  drop column if exists source_storybook_id,
                  drop column if exists entry_type;
                "#,
            )
            .await?;
        Ok(())
    }
}
