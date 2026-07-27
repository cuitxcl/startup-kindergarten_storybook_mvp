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
                  add column if not exists teacher_review_status text not null default 'pending',
                  add column if not exists teacher_reviewed_by uuid references users(id) on delete set null,
                  add column if not exists teacher_reviewed_at timestamptz;

                create index if not exists idx_storybooks_teacher_review
                  on storybooks (workspace_id, teacher_review_status, updated_at desc);
                "#,
            )
            .await
            .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                drop index if exists idx_storybooks_teacher_review;

                alter table storybooks
                  drop column if exists teacher_reviewed_at,
                  drop column if exists teacher_reviewed_by,
                  drop column if exists teacher_review_status;
                "#,
            )
            .await
            .map(|_| ())
    }
}
