use sea_orm::{ConnectionTrait, DbBackend, DbErr, Statement};
use uuid::Uuid;

pub async fn mark_storybook_content_changed(
    db: &impl ConnectionTrait,
    storybook_id: Uuid,
) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybooks
        set status = case
                when status in ('exportable', 'listed') then 'image_pending'
                else status
            end,
            visibility = case
                when status = 'listed' then 'workspace'
                else visibility
            end,
            updated_at = now(),
            teacher_review_status = 'pending',
            teacher_reviewed_by = null,
            teacher_reviewed_at = null
        where id = $1
        "#,
        [storybook_id.into()],
    ))
    .await?;
    Ok(())
}
