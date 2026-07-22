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
                alter table marketplace_templates
                  add column if not exists source_storybook_id uuid null;

                create index if not exists idx_marketplace_templates_source_storybook
                  on marketplace_templates (source_storybook_id)
                  where source_storybook_id is not null;
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
                drop index if exists idx_marketplace_templates_source_storybook;

                alter table marketplace_templates
                  drop column if exists source_storybook_id;
                "#,
            )
            .await?;
        Ok(())
    }
}
