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
                alter table storybook_pages
                  add column if not exists review_status text not null default 'unchecked'
                    check (review_status in ('unchecked', 'satisfied', 'needs_changes')),
                  add column if not exists reviewed_by uuid references users(id) on delete set null,
                  add column if not exists reviewed_at timestamptz;

                create index if not exists idx_storybook_pages_review_status
                  on storybook_pages(storybook_id, review_status, page_number);
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
                drop index if exists idx_storybook_pages_review_status;
                alter table storybook_pages
                  drop column if exists reviewed_at,
                  drop column if exists reviewed_by,
                  drop column if exists review_status;
                "#,
            )
            .await?;
        Ok(())
    }
}
