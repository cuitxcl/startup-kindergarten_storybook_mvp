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
                  add column if not exists page_aspect_ratio varchar(32) not null default 'portrait_4_5';

                create index if not exists idx_storybooks_page_aspect_ratio
                  on storybooks (workspace_id, page_aspect_ratio);
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
                drop index if exists idx_storybooks_page_aspect_ratio;

                alter table storybooks
                  drop column if exists page_aspect_ratio;
                "#,
            )
            .await?;
        Ok(())
    }
}
