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
                create index if not exists idx_export_jobs_storybook_created
                  on export_jobs (storybook_id, created_at desc);

                create index if not exists idx_share_links_storybook_active_created
                  on share_links (storybook_id, status, created_at desc)
                  where status = 'active';
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
                drop index if exists idx_share_links_storybook_active_created;
                drop index if exists idx_export_jobs_storybook_created;
                "#,
            )
            .await?;
        Ok(())
    }
}
