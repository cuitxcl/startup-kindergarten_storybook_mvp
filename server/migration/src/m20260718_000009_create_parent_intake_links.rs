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
                create table if not exists parent_intake_links (
                  id uuid primary key,
                  workspace_id uuid not null references workspaces(id) on delete cascade,
                  token text not null unique,
                  label text not null,
                  status text not null default 'active',
                  expires_at timestamp with time zone,
                  created_by uuid references users(id) on delete set null,
                  created_at timestamp with time zone not null,
                  updated_at timestamp with time zone not null
                );

                create index if not exists idx_parent_intake_links_workspace_status
                  on parent_intake_links (workspace_id, status, created_at desc);
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
                drop index if exists idx_parent_intake_links_workspace_status;
                drop table if exists parent_intake_links;
                "#,
            )
            .await?;
        Ok(())
    }
}
