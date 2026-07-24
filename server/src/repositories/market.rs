pub use crate::repositories::market_submissions::{
    approve_submission, confirm_submission_privacy, create_submission,
    list_operator_submissions_page, list_submissions, list_submissions_page, reject_submission,
};
pub use crate::repositories::market_templates::{find_template, list_templates, update_template};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};

pub async fn seed_demo_marketplace(db: &DatabaseConnection) -> Result<(), DbErr> {
    execute(
        db,
        r#"
        insert into marketplace_templates
          (id, source_type, source_workspace_id, title, summary, age_group, use_scene, page_count, supports_customization, tags, status)
        values
          ('50000000-0000-0000-0000-000000000001', 'platform', null, '一起玩小汽车', '围绕分享、轮流和表达感受的 6 页生活化绘本。', '4-5 岁', '规则引导', 6, true, '["分享", "轮流", "课堂共读"]'::jsonb, 'listed'),
          ('50000000-0000-0000-0000-000000000002', 'school_submission', '20000000-0000-0000-0000-000000000001', '安静午睡的一天', '帮助小班孩子理解午睡前准备、安静入睡和醒后整理。', '4-5 岁', '午睡习惯', 6, true, '["午睡", "生活习惯", "园所共创"]'::jsonb, 'listed')
        on conflict (id) do update
          set source_type = excluded.source_type,
              source_workspace_id = excluded.source_workspace_id,
              title = excluded.title,
              summary = excluded.summary,
              age_group = excluded.age_group,
              use_scene = excluded.use_scene,
              page_count = excluded.page_count,
              supports_customization = excluded.supports_customization,
              tags = excluded.tags,
              status = excluded.status;
        "#,
    )
    .await?;

    execute(
        db,
        r#"
        insert into marketplace_submissions
          (id, workspace_id, source_storybook_id, title, submitted_by, status, privacy_confirmed, updated_at)
        values
          ('60000000-0000-0000-0000-000000000001', '20000000-0000-0000-0000-000000000001', '40000000-0000-0000-0000-000000000003', '午睡小小约定', '00000000-0000-0000-0000-000000000001', 'submitted', true, now())
        on conflict (id) do update
          set title = excluded.title,
              status = excluded.status,
              privacy_confirmed = excluded.privacy_confirmed,
              updated_at = now();
        "#,
    )
    .await?;

    Ok(())
}

async fn execute(db: &DatabaseConnection, sql: &str) -> Result<(), DbErr> {
    db.execute(Statement::from_string(DbBackend::Postgres, sql.to_string()))
        .await?;
    Ok(())
}
