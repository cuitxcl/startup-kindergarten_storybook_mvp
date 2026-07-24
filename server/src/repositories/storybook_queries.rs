use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use uuid::Uuid;

use crate::models::{PaginationMeta, Storybook, StorybookListQuery, StorybookPage, StorybookRole};
use crate::repositories::storybook_rules::{
    parse_storybook_status, parse_storybook_type, parse_visibility,
};

pub async fn list_by_workspace(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    query: StorybookListQuery,
) -> Result<(Vec<Storybook>, PaginationMeta), DbErr> {
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
            from storybooks s
            where s.workspace_id = $1
              and ($2::text is null or s.storybook_type = $2)
              and ($3::text is null or s.status = $3)
              and ($4::uuid is null or s.target_child_id = $4)
              and (
                $5::text is null
                or s.title ilike '%' || $5 || '%'
                or coalesce(s.teaching_goal, '') ilike '%' || $5 || '%'
              )
            "#,
            [
                workspace_id.into(),
                query.storybook_type.clone().into(),
                query.status.clone().into(),
                query.target_child_id.into(),
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
            select
              s.id, s.workspace_id, s.storybook_type, s.status, s.visibility, s.source,
              s.source_storybook_id, s.target_child_id, s.title, coalesce(s.age_group, '') as age_group,
              coalesce(s.use_scene, '') as use_scene, coalesce(s.teaching_goal, '') as teaching_goal,
              coalesce(s.cover_tone, '') as cover_tone, s.updated_at,
              coalesce(u.display_name, '林老师') as creator_name,
              source.title as source_title
            from storybooks s
            left join users u on u.id = s.creator_id
            left join storybooks source on source.id = s.source_storybook_id
            where s.workspace_id = $1
              and ($2::text is null or s.storybook_type = $2)
              and ($3::text is null or s.status = $3)
              and ($4::uuid is null or s.target_child_id = $4)
              and (
                $5::text is null
                or s.title ilike '%' || $5 || '%'
                or coalesce(s.teaching_goal, '') ilike '%' || $5 || '%'
              )
            order by s.updated_at desc, s.title
            limit $6 offset $7
            "#,
            [
                workspace_id.into(),
                query.storybook_type.into(),
                query.status.into(),
                query.target_child_id.into(),
                q_filter.into(),
                (limit as i64).into(),
                (offset as i64).into(),
            ],
        ))
        .await?;

    let items = storybooks_from_rows(db, rows).await?;
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

pub async fn find(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<Storybook, DbErr> {
    query_storybooks(db, Some(workspace_id))
        .await?
        .into_iter()
        .find(|book| book.id == storybook_id)
        .ok_or_else(|| DbErr::RecordNotFound("storybook".to_string()))
}

pub async fn find_any(db: &DatabaseConnection, storybook_id: Uuid) -> Result<Storybook, DbErr> {
    query_storybooks(db, None)
        .await?
        .into_iter()
        .find(|book| book.id == storybook_id)
        .ok_or_else(|| DbErr::RecordNotFound("storybook".to_string()))
}

async fn query_storybooks(
    db: &DatabaseConnection,
    workspace_id: Option<Uuid>,
) -> Result<Vec<Storybook>, DbErr> {
    let rows = if let Some(workspace_id) = workspace_id {
        db.query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select
              s.id, s.workspace_id, s.storybook_type, s.status, s.visibility, s.source,
              s.source_storybook_id, s.target_child_id, s.title, coalesce(s.age_group, '') as age_group,
              coalesce(s.use_scene, '') as use_scene, coalesce(s.teaching_goal, '') as teaching_goal,
              coalesce(s.cover_tone, '') as cover_tone, s.updated_at,
              coalesce(u.display_name, '林老师') as creator_name,
              source.title as source_title
            from storybooks s
            left join users u on u.id = s.creator_id
            left join storybooks source on source.id = s.source_storybook_id
            where s.workspace_id = $1
            order by s.updated_at desc, s.title
            "#,
            [workspace_id.into()],
        ))
        .await?
    } else {
        db.query_all(Statement::from_string(
            DbBackend::Postgres,
            r#"
            select
              s.id, s.workspace_id, s.storybook_type, s.status, s.visibility, s.source,
              s.source_storybook_id, s.target_child_id, s.title, coalesce(s.age_group, '') as age_group,
              coalesce(s.use_scene, '') as use_scene, coalesce(s.teaching_goal, '') as teaching_goal,
              coalesce(s.cover_tone, '') as cover_tone, s.updated_at,
              coalesce(u.display_name, '林老师') as creator_name,
              source.title as source_title
            from storybooks s
            left join users u on u.id = s.creator_id
            left join storybooks source on source.id = s.source_storybook_id
            order by s.updated_at desc, s.title
            "#
            .to_string(),
        ))
        .await?
    };

    storybooks_from_rows(db, rows).await
}

async fn storybooks_from_rows(
    db: &DatabaseConnection,
    rows: Vec<sea_orm::QueryResult>,
) -> Result<Vec<Storybook>, DbErr> {
    let mut books = Vec::with_capacity(rows.len());
    for row in rows {
        let id = row.try_get("", "id")?;
        books.push(Storybook {
            id,
            workspace_id: row.try_get("", "workspace_id")?,
            title: row.try_get("", "title")?,
            storybook_type: parse_storybook_type(&row.try_get::<String>("", "storybook_type")?),
            status: parse_storybook_status(&row.try_get::<String>("", "status")?),
            visibility: parse_visibility(&row.try_get::<String>("", "visibility")?),
            source: row.try_get("", "source")?,
            source_title: row.try_get("", "source_title")?,
            target_child_id: row.try_get("", "target_child_id")?,
            creator_name: row.try_get("", "creator_name")?,
            updated_at: row
                .try_get::<DateTime<Utc>>("", "updated_at")?
                .format("%Y-%m-%d %H:%M")
                .to_string(),
            age_group: row.try_get("", "age_group")?,
            use_scene: row.try_get("", "use_scene")?,
            teaching_goal: row.try_get("", "teaching_goal")?,
            cover_tone: row.try_get("", "cover_tone")?,
            pages: pages_for(db, id).await?,
            roles: roles_for(db, row.try_get("", "workspace_id")?, id).await?,
        });
    }
    Ok(books)
}

async fn pages_for(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<Vec<StorybookPage>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, page_number, title, body, illustration_prompt, status
            from storybook_pages
            where storybook_id = $1
            order by page_number
            "#,
            [storybook_id.into()],
        ))
        .await?;
    rows.into_iter()
        .map(|row| {
            let page_number: i32 = row.try_get("", "page_number")?;
            Ok(StorybookPage {
                id: row.try_get("", "id")?,
                page_number: page_number.max(0) as u32,
                title: row.try_get("", "title")?,
                body: row.try_get("", "body")?,
                illustration_prompt: row.try_get("", "illustration_prompt")?,
                status: row.try_get("", "status")?,
            })
        })
        .collect()
}

async fn roles_for(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<Vec<StorybookRole>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select
              r.id,
              r.name,
              r.role_type,
              r.appearance,
              coalesce(r.story_function, '') as story_function,
              r.needs_consistency,
              r.reference_image_url,
              r.reference_image_prompt,
              coalesce(r.reference_status, 'not_started') as reference_status,
              ref_job.id as reference_generation_job_id
            from storybook_roles r
            left join lateral (
              select g.id
              from generation_jobs g
              where g.storybook_id = r.storybook_id
                and g.job_type = 'storybook_role_reference_image'
                and g.status = 'succeeded'
                and g.input_json->>'role_id' = r.id::text
                and g.output_json #>> '{image,image_url}' = r.reference_image_url
              order by g.finished_at desc nulls last, g.created_at desc
              limit 1
            ) ref_job on true
            where r.storybook_id = $1
            order by r.role_type, r.name
            "#,
            [storybook_id.into()],
        ))
        .await?;
    rows.into_iter()
        .map(|row| {
            Ok(StorybookRole {
                id: row.try_get("", "id")?,
                name: row.try_get("", "name")?,
                role_type: row.try_get("", "role_type")?,
                appearance: row.try_get("", "appearance")?,
                story_function: row.try_get("", "story_function")?,
                needs_consistency: row.try_get("", "needs_consistency")?,
                reference_image_url: role_reference_image_url(
                    workspace_id,
                    row.try_get("", "reference_image_url")?,
                    row.try_get("", "reference_generation_job_id")?,
                ),
                reference_image_prompt: row.try_get("", "reference_image_prompt")?,
                reference_status: row.try_get("", "reference_status")?,
            })
        })
        .collect()
}

fn role_reference_image_url(
    workspace_id: Uuid,
    stored_url: Option<String>,
    generation_job_id: Option<Uuid>,
) -> Option<String> {
    match (stored_url, generation_job_id) {
        (Some(_), Some(job_id)) => Some(format!(
            "/api/workspaces/{workspace_id}/generation-jobs/{job_id}/image"
        )),
        (stored_url, _) => stored_url,
    }
}
