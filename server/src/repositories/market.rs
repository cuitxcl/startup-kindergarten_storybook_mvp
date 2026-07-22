use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::{
    models::{
        CreateSubmissionRequest, MarketplaceQuery, MarketplaceSubmission, MarketplaceTemplate,
        PaginationMeta, StorybookType, UpdateMarketplaceTemplateRequest,
    },
    repositories::storybooks,
};

const DEMO_USER_ID: &str = "00000000-0000-0000-0000-000000000001";

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

pub async fn list_templates(
    db: &DatabaseConnection,
    query: MarketplaceQuery,
) -> Result<(Vec<MarketplaceTemplate>, PaginationMeta), DbErr> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0);
    let q_filter = query
        .q
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let total: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select count(*) as count
            from marketplace_templates
            where status = 'listed'
              and ($1::text is null or source_type = $1)
              and ($2::boolean is null or supports_customization = $2)
              and (
                $3::text is null
                or title ilike '%' || $3 || '%'
                or summary ilike '%' || $3 || '%'
              )
            "#,
            [
                query.source.clone().into(),
                query.supports_customization.into(),
                q_filter.clone().into(),
            ],
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);

    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, source_type, source_storybook_id, title, summary, coalesce(age_group, '') as age_group,
                   coalesce(use_scene, '') as use_scene, page_count, supports_customization, tags
            from marketplace_templates
            where status = 'listed'
              and ($1::text is null or source_type = $1)
              and ($2::boolean is null or supports_customization = $2)
              and (
                $3::text is null
                or title ilike '%' || $3 || '%'
                or summary ilike '%' || $3 || '%'
              )
            order by source_type, title
            limit $4 offset $5
            "#,
            [
                query.source.into(),
                query.supports_customization.into(),
                q_filter.into(),
                (limit as i64).into(),
                (offset as i64).into(),
            ],
        ))
        .await?;

    let items = rows
        .into_iter()
        .map(|row| template_from_row(&row))
        .collect::<Result<Vec<_>, _>>()?;
    let total = total.max(0) as usize;
    Ok((
        items,
        PaginationMeta {
            total,
            limit,
            offset: offset.min(total),
            has_more: offset.saturating_add(limit) < total,
        },
    ))
}

pub async fn find_template(
    db: &DatabaseConnection,
    template_id: Uuid,
) -> Result<MarketplaceTemplate, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, source_type, source_storybook_id, title, summary, coalesce(age_group, '') as age_group,
                   coalesce(use_scene, '') as use_scene, page_count, supports_customization, tags
            from marketplace_templates
            where id = $1 and status = 'listed'
            limit 1
            "#,
            [template_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("template".to_string()))?;
    template_from_row(&row)
}

pub async fn update_template(
    db: &DatabaseConnection,
    template_id: Uuid,
    payload: UpdateMarketplaceTemplateRequest,
) -> Result<MarketplaceTemplate, DbErr> {
    let current = find_template(db, template_id).await?;
    let title = clean_required(payload.title.as_deref(), &current.title, "模板标题")?;
    let summary = clean_required(payload.summary.as_deref(), &current.summary, "模板摘要")?;
    let age_group = clean_required(payload.age_group.as_deref(), &current.age_group, "年龄段")?;
    let use_scene = clean_required(payload.use_scene.as_deref(), &current.use_scene, "使用场景")?;
    let supports_customization = payload
        .supports_customization
        .unwrap_or(current.supports_customization);
    let tags = payload
        .tags
        .map(clean_tags)
        .unwrap_or(current.tags)
        .into_iter()
        .take(12)
        .collect::<Vec<_>>();

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update marketplace_templates
        set title = $2,
            summary = $3,
            age_group = $4,
            use_scene = $5,
            supports_customization = $6,
            tags = $7
        where id = $1
          and status = 'listed'
        "#,
        [
            template_id.into(),
            title.into(),
            summary.into(),
            age_group.into(),
            use_scene.into(),
            supports_customization.into(),
            serde_json::json!(tags).into(),
        ],
    ))
    .await?;

    find_template(db, template_id).await
}

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

pub(crate) fn build_template_from_submission(
    submission_id: Uuid,
    workspace_id: Uuid,
    source_storybook_id: Uuid,
    title: String,
    age_group: String,
    use_scene: String,
    summary: String,
    page_count: i32,
) -> MarketplaceTemplate {
    let source_label = source_label("school_submission").to_string();
    let tags_scene = use_scene.clone();
    MarketplaceTemplate {
        id: submission_id,
        title,
        summary,
        source_type: "school_submission".to_string(),
        source_label,
        source_storybook_id: Some(source_storybook_id),
        age_group,
        use_scene,
        page_count: page_count.max(0) as u32,
        supports_customization: true,
        tags: vec!["园所共创".to_string(), tags_scene, workspace_id.to_string()],
    }
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
    let risks = submission_privacy_risks(&privacy_text);
    if risks.is_empty() {
        Ok(())
    } else {
        Err(DbErr::Custom(format!(
            "submission_privacy_risk:{}",
            risks.join("、")
        )))
    }
}

fn submission_privacy_risks(text: &str) -> Vec<&'static str> {
    let mut risks = Vec::new();
    if contains_email(text) {
        risks.push("邮箱");
    }
    if contains_chinese_mobile(text) {
        risks.push("手机号");
    }
    if contains_id_card(text) || contains_any(text, &["身份证", "身份证号", "证件号码"])
    {
        risks.push("身份信息");
    }
    if contains_any(text, &["家庭住址", "详细地址", "门牌号", "楼栋", "单元号"]) {
        risks.push("住址信息");
    }
    if contains_any(text, &["病历", "诊断证明", "医保卡", "过敏史", "就诊记录"]) {
        risks.push("医疗信息");
    }
    risks
}

fn contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|keyword| text.contains(keyword))
}

fn contains_email(text: &str) -> bool {
    text.split(|ch: char| ch.is_whitespace() || "，。；、（）()<>《》[]【】".contains(ch))
        .any(|part| {
            let Some(at) = part.find('@') else {
                return false;
            };
            at > 0 && part[at + 1..].contains('.') && part[at + 1..].len() >= 3
        })
}

fn contains_chinese_mobile(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    for start in 0..chars.len() {
        if chars[start] != '1' {
            continue;
        }
        if start > 0 && chars[start - 1].is_ascii_digit() {
            continue;
        }
        let mut digits = String::new();
        let mut index = start;
        while index < chars.len() && digits.len() < 11 {
            let ch = chars[index];
            if ch.is_ascii_digit() {
                digits.push(ch);
            } else if ch == ' ' || ch == '-' {
                // Allow common formatting such as 138 0013 8000 or 138-0013-8000.
            } else {
                break;
            }
            index += 1;
        }
        if digits.len() == 11 {
            if index < chars.len() && chars[index].is_ascii_digit() {
                continue;
            }
            let second = digits.as_bytes()[1] as char;
            if matches!(second, '3'..='9') {
                return true;
            }
        }
    }
    false
}

fn contains_id_card(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    for start in 0..chars.len() {
        let mut digits = 0;
        let mut index = start;
        while index < chars.len() && chars[index].is_ascii_digit() {
            digits += 1;
            index += 1;
        }
        if digits == 18
            || (digits == 17 && index < chars.len() && matches!(chars[index], 'x' | 'X'))
        {
            return true;
        }
    }
    false
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

#[derive(Clone, Copy)]
enum SubmissionStatusFilter {
    Any,
    Draft,
    Submitted,
    Approved,
    Listed,
    Rejected,
}

impl SubmissionStatusFilter {
    fn workspace_where_sql(self) -> &'static str {
        match self {
            SubmissionStatusFilter::Any => "",
            SubmissionStatusFilter::Draft => "and status = 'draft'",
            SubmissionStatusFilter::Submitted => "and status = 'submitted'",
            SubmissionStatusFilter::Approved => "and status = 'approved'",
            SubmissionStatusFilter::Listed => "and status = 'listed'",
            SubmissionStatusFilter::Rejected => "and status = 'rejected'",
        }
    }

    fn alias_where_sql(self) -> &'static str {
        match self {
            SubmissionStatusFilter::Any => "",
            SubmissionStatusFilter::Draft => "and ms.status = 'draft'",
            SubmissionStatusFilter::Submitted => "and ms.status = 'submitted'",
            SubmissionStatusFilter::Approved => "and ms.status = 'approved'",
            SubmissionStatusFilter::Listed => "and ms.status = 'listed'",
            SubmissionStatusFilter::Rejected => "and ms.status = 'rejected'",
        }
    }

    fn operator_where_sql(self) -> &'static str {
        match self {
            SubmissionStatusFilter::Draft => "and status = 'draft'",
            _ => self.workspace_where_sql(),
        }
    }
}

fn submission_status_filter(status: Option<&str>) -> Result<SubmissionStatusFilter, DbErr> {
    match status.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(SubmissionStatusFilter::Any),
        Some("draft") => Ok(SubmissionStatusFilter::Draft),
        Some("submitted") => Ok(SubmissionStatusFilter::Submitted),
        Some("approved") => Ok(SubmissionStatusFilter::Approved),
        Some("listed") => Ok(SubmissionStatusFilter::Listed),
        Some("rejected") => Ok(SubmissionStatusFilter::Rejected),
        Some(other) => Err(DbErr::Custom(format!("不支持的市场投稿状态：{other}"))),
    }
}

fn template_from_row(row: &sea_orm::QueryResult) -> Result<MarketplaceTemplate, DbErr> {
    let source_type: String = row.try_get("", "source_type")?;
    let tags: JsonValue = row.try_get("", "tags")?;
    let page_count: i32 = row.try_get("", "page_count")?;
    Ok(MarketplaceTemplate {
        id: row.try_get("", "id")?,
        title: row.try_get("", "title")?,
        summary: row.try_get("", "summary")?,
        source_label: source_label(&source_type).to_string(),
        source_type,
        source_storybook_id: row.try_get("", "source_storybook_id")?,
        age_group: row.try_get("", "age_group")?,
        use_scene: row.try_get("", "use_scene")?,
        page_count: page_count.max(0) as u32,
        supports_customization: row.try_get("", "supports_customization")?,
        tags: tags
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn submission_from_row(row: &sea_orm::QueryResult) -> Result<MarketplaceSubmission, DbErr> {
    Ok(MarketplaceSubmission {
        id: row.try_get("", "id")?,
        workspace_id: row.try_get("", "workspace_id")?,
        title: row.try_get("", "title")?,
        source_storybook_title: row.try_get("", "source_storybook_title")?,
        submitted_by: row.try_get("", "submitted_by")?,
        status: row.try_get("", "status")?,
        privacy_confirmed: row.try_get("", "privacy_confirmed")?,
        updated_at: row
            .try_get::<DateTime<Utc>>("", "updated_at")?
            .format("%Y-%m-%d %H:%M")
            .to_string(),
    })
}

fn source_label(source_type: &str) -> &'static str {
    match source_type {
        "school_submission" => "园所投稿",
        _ => "平台精选",
    }
}

fn clean_required(input: Option<&str>, fallback: &str, label: &str) -> Result<String, DbErr> {
    let value = input.unwrap_or(fallback).trim();
    if value.is_empty() {
        Err(DbErr::Custom(format!("{label}不能为空")))
    } else {
        Ok(value.to_string())
    }
}

fn clean_tags(tags: Vec<String>) -> Vec<String> {
    let mut cleaned = Vec::new();
    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty() || cleaned.iter().any(|item| item == tag) {
            continue;
        }
        cleaned.push(tag.to_string());
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_template_from_submission_marks_school_submission() {
        let template = build_template_from_submission(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "午睡小小约定".to_string(),
            "4-5 岁".to_string(),
            "午睡习惯".to_string(),
            "建立睡前整理和安静入睡流程".to_string(),
            6,
        );

        assert_eq!(template.source_type, "school_submission");
        assert_eq!(template.source_label, "园所投稿");
        assert!(template.supports_customization);
        assert_eq!(template.page_count, 6);
    }

    #[test]
    fn build_template_from_submission_clamps_page_count() {
        let template = build_template_from_submission(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "午睡小小约定".to_string(),
            "4-5 岁".to_string(),
            "午睡习惯".to_string(),
            "建立睡前整理和安静入睡流程".to_string(),
            -1,
        );

        assert_eq!(template.page_count, 0);
    }

    #[test]
    fn submission_privacy_risks_allows_normal_storybook_text() {
        let risks = submission_privacy_risks(
            "午睡小小约定 建立睡前整理和安静入睡流程 第 1 页 孩子们把鞋子摆整齐",
        );

        assert!(risks.is_empty());
    }

    #[test]
    fn submission_privacy_risks_does_not_treat_long_ids_as_phone_numbers() {
        let risks = submission_privacy_risks("UI Smoke 普通绘本 1784538853883");

        assert!(risks.is_empty());
    }

    #[test]
    fn submission_privacy_risks_detects_contact_and_identity() {
        let risks = submission_privacy_risks(
            "请联系家长 parent@example.com 或 138 0013 8000，身份证号 11010119900307123X",
        );

        assert!(risks.contains(&"邮箱"));
        assert!(risks.contains(&"手机号"));
        assert!(risks.contains(&"身份信息"));
    }

    #[test]
    fn submission_privacy_risks_detects_address_and_medical_text() {
        let risks = submission_privacy_risks("家庭住址在某小区 3 号楼，孩子有过敏史。");

        assert!(risks.contains(&"住址信息"));
        assert!(risks.contains(&"医疗信息"));
    }
}

async fn execute(db: &DatabaseConnection, sql: &str) -> Result<(), DbErr> {
    db.execute(Statement::from_string(DbBackend::Postgres, sql.to_string()))
        .await?;
    Ok(())
}
