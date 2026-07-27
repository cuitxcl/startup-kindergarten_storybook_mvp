use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use uuid::Uuid;

use crate::{
    models::{
        CreateSubmissionRequest, MarketplaceSubmission, MarketplaceTemplate, PaginationMeta,
        StorybookType,
    },
    repositories::{
        market_submission_helpers::{
            build_template_from_submission, submission_from_row, submission_status_filter,
        },
        market_templates::find_template,
        privacy::storybook_privacy_risks,
        storybooks,
    },
};

const DEMO_USER_ID: &str = "00000000-0000-0000-0000-000000000001";

pub async fn list_submissions(
    db: &DatabaseConnection,
    workspace_id: Uuid,
) -> Result<Vec<MarketplaceSubmission>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select ms.id, ms.workspace_id, ms.title, ms.status, ms.privacy_confirmed, ms.updated_at,
                   coalesce(s.title, ms.title) as source_storybook_title,
                   coalesce(u.display_name, '林老师') as submitted_by
            from marketplace_submissions ms
            left join storybooks s on s.id = ms.source_storybook_id
            left join users u on u.id = ms.submitted_by
            where ms.workspace_id = $1
            order by ms.updated_at desc, ms.title
            "#,
            [workspace_id.into()],
        ))
        .await?;
    rows.into_iter()
        .map(|row| submission_from_row(&row))
        .collect()
}

pub async fn list_submissions_page(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    status: Option<&str>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<(Vec<MarketplaceSubmission>, PaginationMeta), DbErr> {
    let limit = limit.unwrap_or(50).clamp(1, 100);
    let offset = offset.unwrap_or(0);
    let status_filter = submission_status_filter(status)?;
    let total: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "select count(*) as count from marketplace_submissions where workspace_id = $1 {}",
                status_filter.workspace_where_sql()
            ),
            [workspace_id.into()],
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);

    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                r#"
            select ms.id, ms.workspace_id, ms.title, ms.status, ms.privacy_confirmed, ms.updated_at,
                   coalesce(s.title, ms.title) as source_storybook_title,
                   coalesce(u.display_name, '林老师') as submitted_by
            from marketplace_submissions ms
            left join storybooks s on s.id = ms.source_storybook_id
            left join users u on u.id = ms.submitted_by
            where ms.workspace_id = $1
              {status_where}
            order by ms.updated_at desc, ms.title
            limit $2 offset $3
            "#,
                status_where = status_filter.alias_where_sql()
            ),
            [
                workspace_id.into(),
                (limit as i64).into(),
                (offset as i64).into(),
            ],
        ))
        .await?;

    let total = total.max(0) as usize;
    Ok((
        rows.iter()
            .map(submission_from_row)
            .collect::<Result<Vec<_>, _>>()?,
        PaginationMeta {
            total,
            limit,
            offset: offset.min(total),
            has_more: offset.saturating_add(limit) < total,
        },
    ))
}

pub async fn list_operator_submissions_page(
    db: &DatabaseConnection,
    status: Option<&str>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<(Vec<MarketplaceSubmission>, PaginationMeta), DbErr> {
    let limit = limit.unwrap_or(50).clamp(1, 100);
    let offset = offset.unwrap_or(0);
    let status_filter = submission_status_filter(status)?;
    let total: i64 = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            format!(
                r#"
            select count(*) as count
            from marketplace_submissions
            where status in ('submitted', 'approved', 'listed', 'rejected')
              {status_where}
            "#,
                status_where = status_filter.operator_where_sql()
            ),
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);

    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                r#"
            select ms.id, ms.workspace_id, ms.title, ms.status, ms.privacy_confirmed, ms.updated_at,
                   coalesce(s.title, ms.title) as source_storybook_title,
                   coalesce(u.display_name, '林老师') as submitted_by
            from marketplace_submissions ms
            left join storybooks s on s.id = ms.source_storybook_id
            left join users u on u.id = ms.submitted_by
            where ms.status in ('submitted', 'approved', 'listed', 'rejected')
              {status_where}
            order by
              case ms.status when 'submitted' then 0 when 'approved' then 1 when 'listed' then 2 else 3 end,
              ms.updated_at desc
            limit $1 offset $2
            "#,
                status_where = status_filter.alias_where_sql()
            ),
            [(limit as i64).into(), (offset as i64).into()],
        ))
        .await?;

    let total = total.max(0) as usize;
    Ok((
        rows.iter()
            .map(submission_from_row)
            .collect::<Result<Vec<_>, _>>()?,
        PaginationMeta {
            total,
            limit,
            offset: offset.min(total),
            has_more: offset.saturating_add(limit) < total,
        },
    ))
}

pub async fn create_submission(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    payload: CreateSubmissionRequest,
) -> Result<MarketplaceSubmission, DbErr> {
    let book = storybooks::find(db, workspace_id, payload.storybook_id).await?;
    if book.storybook_type != StorybookType::Plain {
        return Err(DbErr::Custom("只有普通绘本可以投稿".to_string()));
    }
    ensure_not_already_submitted(db, workspace_id, payload.storybook_id).await?;

    let id = Uuid::new_v4();
    let submitted_by = Uuid::parse_str(DEMO_USER_ID)
        .map_err(|err| DbErr::Custom(format!("演示用户 ID 无效：{err}")))?;
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into marketplace_submissions
              (id, workspace_id, source_storybook_id, title, submitted_by, status, privacy_confirmed, updated_at)
            values ($1, $2, $3, $4, $5, 'draft', false, now())
            returning id, workspace_id, title, status, privacy_confirmed, updated_at
            "#,
            [
                id.into(),
                workspace_id.into(),
                payload.storybook_id.into(),
                book.title.into(),
                submitted_by.into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("submission".to_string()))?;

    find_submission_with_context(db, workspace_id, row.try_get("", "id")?).await
}

async fn ensure_not_already_submitted(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<(), DbErr> {
    let exists = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id
            from marketplace_submissions
            where workspace_id = $1
              and source_storybook_id = $2
            limit 1
            "#,
            [workspace_id.into(), storybook_id.into()],
        ))
        .await?
        .is_some();

    if exists {
        Err(DbErr::Custom("这本绘本已经创建过市场投稿".to_string()))
    } else {
        Ok(())
    }
}

pub async fn approve_submission(
    db: &DatabaseConnection,
    submission_id: Uuid,
) -> Result<MarketplaceTemplate, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select
              ms.id,
              ms.workspace_id,
              ms.source_storybook_id,
              ms.title,
              ms.privacy_confirmed,
              s.age_group,
              s.use_scene,
              s.teaching_goal,
              coalesce(page_counts.page_count, 0) as page_count
            from marketplace_submissions ms
            join storybooks s on s.id = ms.source_storybook_id and s.workspace_id = ms.workspace_id
            left join (
              select storybook_id, count(*)::int as page_count
              from storybook_pages
              group by storybook_id
            ) page_counts on page_counts.storybook_id = s.id
            where ms.id = $1
              and ms.status = 'submitted'
              and ms.privacy_confirmed = true
              and s.storybook_type = 'plain'
            limit 1
            "#,
            [submission_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("submission".to_string()))?;

    let workspace_id: Uuid = row.try_get("", "workspace_id")?;
    let source_storybook_id: Uuid = row.try_get("", "source_storybook_id")?;
    let title: String = row.try_get("", "title")?;
    let age_group: String = row.try_get("", "age_group")?;
    let use_scene: String = row.try_get("", "use_scene")?;
    let summary: String = row.try_get("", "teaching_goal")?;
    let page_count: i32 = row.try_get("", "page_count")?;
    let template_id = Uuid::new_v4();
    let template = build_template_from_submission(
        template_id,
        workspace_id,
        source_storybook_id,
        title,
        age_group,
        use_scene,
        summary,
        page_count,
    );

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into marketplace_templates
          (id, source_type, source_workspace_id, source_storybook_id, title, summary, age_group, use_scene, page_count, supports_customization, tags, status)
        values ($1, 'school_submission', $2, $3, $4, $5, $6, $7, $8, true, $9, 'listed')
        "#,
        [
            template.id.into(),
            workspace_id.into(),
            source_storybook_id.into(),
            template.title.clone().into(),
            template.summary.clone().into(),
            template.age_group.clone().into(),
            template.use_scene.clone().into(),
            template.page_count.into(),
            serde_json::json!(template.tags).into(),
        ],
    ))
    .await?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update marketplace_submissions
        set status = 'listed',
            updated_at = now()
        where id = $1
        "#,
        [submission_id.into()],
    ))
    .await?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybooks
        set status = 'listed',
            visibility = 'market_listed',
            updated_at = now()
        where id = $1
        "#,
        [source_storybook_id.into()],
    ))
    .await?;

    find_template(db, template.id).await
}

pub async fn reject_submission(
    db: &DatabaseConnection,
    submission_id: Uuid,
) -> Result<MarketplaceSubmission, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update marketplace_submissions
            set status = 'rejected',
                updated_at = now()
            where id = $1
              and status in ('submitted', 'approved')
            returning id, workspace_id, source_storybook_id
            "#,
            [submission_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("submission".to_string()))?;

    let workspace_id: Uuid = row.try_get("", "workspace_id")?;
    let source_storybook_id: Uuid = row.try_get("", "source_storybook_id")?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybooks
        set status = 'exportable',
            visibility = 'private',
            updated_at = now()
        where workspace_id = $1
          and id = $2
          and storybook_type = 'plain'
          and visibility = 'market_submission'
        "#,
        [workspace_id.into(), source_storybook_id.into()],
    ))
    .await?;

    find_submission_with_context(db, workspace_id, submission_id).await
}

pub async fn confirm_submission_privacy(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    submission_id: Uuid,
) -> Result<MarketplaceSubmission, DbErr> {
    ensure_submission_privacy_clear(db, workspace_id, submission_id).await?;

    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update marketplace_submissions
            set privacy_confirmed = true,
                status = 'submitted',
                updated_at = now()
            where workspace_id = $1 and id = $2
            returning id
            "#,
            [workspace_id.into(), submission_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("submission".to_string()))?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybooks
        set status = 'submitted',
            visibility = 'market_submission',
            updated_at = now()
        where workspace_id = $1
          and id = (
            select source_storybook_id
            from marketplace_submissions
            where id = $2 and workspace_id = $1
          )
          and storybook_type = 'plain'
        "#,
        [workspace_id.into(), submission_id.into()],
    ))
    .await?;

    find_submission_with_context(db, workspace_id, row.try_get("", "id")?).await
}

async fn ensure_submission_privacy_clear(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    submission_id: Uuid,
) -> Result<(), DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select concat_ws(
              ' ',
              s.title,
              s.teaching_goal,
              s.use_scene,
              s.cover_tone,
              coalesce(string_agg(distinct concat_ws(' ', sp.title, sp.body, sp.illustration_prompt), ' '), ''),
              coalesce(string_agg(distinct concat_ws(' ', sr.name, sr.appearance, sr.story_function), ' '), '')
            ) as privacy_text
            from marketplace_submissions ms
            join storybooks s on s.id = ms.source_storybook_id and s.workspace_id = ms.workspace_id
            left join storybook_pages sp on sp.storybook_id = s.id
            left join storybook_roles sr on sr.storybook_id = s.id
            where ms.workspace_id = $1 and ms.id = $2
            group by ms.id, s.id
            "#,
            [workspace_id.into(), submission_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("submission".to_string()))?;

    let privacy_text: String = row.try_get("", "privacy_text")?;
    let risks = storybook_privacy_risks(&privacy_text);
    if risks.is_empty() {
        Ok(())
    } else {
        Err(DbErr::Custom(format!(
            "submission_privacy_risk:{}",
            risks.join("、")
        )))
    }
}

async fn find_submission_with_context(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    submission_id: Uuid,
) -> Result<MarketplaceSubmission, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select ms.id, ms.workspace_id, ms.title, ms.status, ms.privacy_confirmed, ms.updated_at,
                   coalesce(s.title, ms.title) as source_storybook_title,
                   coalesce(u.display_name, '林老师') as submitted_by
            from marketplace_submissions ms
            left join storybooks s on s.id = ms.source_storybook_id
            left join users u on u.id = ms.submitted_by
            where ms.workspace_id = $1 and ms.id = $2
            limit 1
            "#,
            [workspace_id.into(), submission_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("submission".to_string()))?;
    submission_from_row(&row)
}
