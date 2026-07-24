use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::models::{
    MarketplaceQuery, MarketplaceTemplate, PaginationMeta, UpdateMarketplaceTemplateRequest,
};

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

pub(crate) fn source_label(source_type: &str) -> &'static str {
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
