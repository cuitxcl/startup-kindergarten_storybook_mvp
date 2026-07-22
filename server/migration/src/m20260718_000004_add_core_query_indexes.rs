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
                create unique index if not exists uidx_classrooms_workspace_name
                  on classrooms (workspace_id, name);

                create index if not exists idx_children_workspace_classroom
                  on children (workspace_id, classroom_id);

                create index if not exists idx_storybooks_workspace_updated
                  on storybooks (workspace_id, updated_at desc);

                create unique index if not exists uidx_storybook_pages_book_number
                  on storybook_pages (storybook_id, page_number);

                create index if not exists idx_marketplace_templates_status_source
                  on marketplace_templates (status, source_type);

                create index if not exists idx_marketplace_submissions_workspace_status
                  on marketplace_submissions (workspace_id, status);

                create index if not exists idx_generation_jobs_workspace_status
                  on generation_jobs (workspace_id, status, created_at desc);

                create index if not exists idx_export_jobs_storybook_status
                  on export_jobs (storybook_id, status);
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
                drop index if exists idx_export_jobs_storybook_status;
                drop index if exists idx_generation_jobs_workspace_status;
                drop index if exists idx_marketplace_submissions_workspace_status;
                drop index if exists idx_marketplace_templates_status_source;
                drop index if exists uidx_storybook_pages_book_number;
                drop index if exists idx_storybooks_workspace_updated;
                drop index if exists idx_children_workspace_classroom;
                drop index if exists uidx_classrooms_workspace_name;
                "#,
            )
            .await?;
        Ok(())
    }
}
