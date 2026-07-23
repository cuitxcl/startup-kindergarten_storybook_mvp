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
                alter table storybook_roles
                  add column if not exists reference_image_url text,
                  add column if not exists reference_image_prompt text,
                  add column if not exists reference_status varchar(32) not null default 'not_started';

                create index if not exists idx_storybook_roles_reference_status
                  on storybook_roles (storybook_id, reference_status);
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
                drop index if exists idx_storybook_roles_reference_status;

                alter table storybook_roles
                  drop column if exists reference_status,
                  drop column if exists reference_image_prompt,
                  drop column if exists reference_image_url;
                "#,
            )
            .await?;
        Ok(())
    }
}
