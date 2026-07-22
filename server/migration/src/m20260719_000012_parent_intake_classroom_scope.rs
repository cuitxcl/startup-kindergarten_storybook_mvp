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
                alter table parent_intake_links
                  add column if not exists classroom_id uuid references classrooms(id) on delete set null;

                alter table parent_intakes
                  add column if not exists classroom_id uuid references classrooms(id) on delete set null;

                create index if not exists idx_parent_intake_links_workspace_classroom
                  on parent_intake_links (workspace_id, classroom_id, created_at desc);

                create index if not exists idx_parent_intakes_workspace_classroom
                  on parent_intakes (workspace_id, classroom_id, created_at desc);
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
                drop index if exists idx_parent_intake_links_workspace_classroom;
                drop index if exists idx_parent_intakes_workspace_classroom;

                alter table parent_intakes
                  drop column if exists classroom_id;

                alter table parent_intake_links
                  drop column if exists classroom_id;
                "#,
            )
            .await?;
        Ok(())
    }
}
