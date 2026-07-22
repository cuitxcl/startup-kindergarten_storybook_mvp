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
                alter table parent_intakes
                  add column if not exists workspace_id uuid,
                  add column if not exists confirmed_child_id uuid,
                  add column if not exists updated_at timestamp with time zone;

                update parent_intakes
                set workspace_id = '20000000-0000-0000-0000-000000000001'
                where workspace_id is null;

                update parent_intakes
                set updated_at = created_at
                where updated_at is null;

                alter table parent_intakes
                  alter column workspace_id set not null,
                  alter column updated_at set not null;

                create index if not exists idx_parent_intakes_workspace_status
                  on parent_intakes (workspace_id, status, created_at desc);
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
                drop index if exists idx_parent_intakes_workspace_status;
                alter table parent_intakes
                  drop column if exists confirmed_child_id,
                  drop column if exists workspace_id,
                  drop column if exists updated_at;
                "#,
            )
            .await?;
        Ok(())
    }
}
