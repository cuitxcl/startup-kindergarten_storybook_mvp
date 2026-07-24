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
                alter table generation_jobs
                  add column if not exists created_by uuid references users(id) on delete set null;

                alter table export_jobs
                  add column if not exists created_by uuid references users(id) on delete set null;

                update generation_jobs gj
                set created_by = s.creator_id
                from storybooks s
                where gj.storybook_id = s.id
                  and gj.created_by is null
                  and s.creator_id is not null;

                update export_jobs ej
                set created_by = s.creator_id
                from storybooks s
                where ej.storybook_id = s.id
                  and ej.created_by is null
                  and s.creator_id is not null;

                create index if not exists idx_generation_jobs_created_by_storage
                  on generation_jobs (created_by, status, job_type)
                  where created_by is not null;

                create index if not exists idx_export_jobs_created_by_storage
                  on export_jobs (created_by, status)
                  where created_by is not null;
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
                drop index if exists idx_export_jobs_created_by_storage;
                drop index if exists idx_generation_jobs_created_by_storage;

                alter table export_jobs
                  drop column if exists created_by;

                alter table generation_jobs
                  drop column if exists created_by;
                "#,
            )
            .await
            .map(|_| ())
    }
}
