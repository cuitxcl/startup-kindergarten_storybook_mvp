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
                  add column if not exists access_count integer not null default 0,
                  add column if not exists last_accessed_at timestamp with time zone;
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
                alter table parent_intake_links
                  drop column if exists last_accessed_at,
                  drop column if exists access_count;
                "#,
            )
            .await?;
        Ok(())
    }
}
