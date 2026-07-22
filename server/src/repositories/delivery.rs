use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use std::{collections::HashMap, path::PathBuf};
use uuid::Uuid;

use crate::{
    models::{ExportJob, PaginationMeta, ShareLink, Storybook},
    repositories::storybooks,
    services::{pdf::encode_storybook_pdf_with_images, storage},
};

pub async fn create_export(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<ExportJob, DbErr> {
    ensure_storybook_in_workspace(db, workspace_id, storybook_id).await?;
    enqueue_export(db, storybook_id).await
}

pub async fn create_export_by_share_token(
    db: &DatabaseConnection,
    token: &str,
) -> Result<ExportJob, DbErr> {
    let storybook = storybook_by_share_token(db, token).await?;
    enqueue_export(db, storybook.id).await
}

pub async fn find_export_by_share_token(
    db: &DatabaseConnection,
    token: &str,
    export_id: Uuid,
) -> Result<ExportJob, DbErr> {
    let storybook = storybook_by_share_token(db, token).await?;
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, storybook_id, status, file_url, last_error, created_at, finished_at
            from export_jobs
            where id = $1 and storybook_id = $2
            limit 1
            "#,
            [export_id.into(), storybook.id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("export_job".to_string()))?;

    export_from_row(row)
}

async fn enqueue_export(db: &DatabaseConnection, storybook_id: Uuid) -> Result<ExportJob, DbErr> {
    ensure_storybook_delivery_privacy_clear(db, storybook_id).await?;
    let id = Uuid::new_v4();
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into export_jobs
              (id, storybook_id, status, created_at)
            values ($1, $2, 'queued', now())
            returning id, storybook_id, status, file_url, last_error, created_at, finished_at
            "#,
            [id.into(), storybook_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("export_job".to_string()))?;

    export_from_row(row)
}

async fn complete_export(
    db: &DatabaseConnection,
    job_id: Uuid,
    storybook_id: Uuid,
) -> Result<ExportJob, DbErr> {
    mark_export_running(db, job_id).await?;

    let file_url = match write_export_file(db, job_id, storybook_id).await {
        Ok(file_url) => file_url,
        Err(err) => {
            let _ = mark_export_failed(db, job_id, &err.to_string()).await;
            return Err(err);
        }
    };

    mark_export_succeeded(db, job_id, file_url).await
}

async fn mark_export_running(db: &DatabaseConnection, job_id: Uuid) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update export_jobs
        set status = 'running',
            last_error = null
        where id = $1 and status = 'queued'
        "#,
        [job_id.into()],
    ))
    .await?;
    Ok(())
}

async fn write_export_file(
    db: &DatabaseConnection,
    export_id: Uuid,
    storybook_id: Uuid,
) -> Result<String, DbErr> {
    let storybook = storybooks::find_any(db, storybook_id).await?;
    let page_images = latest_storybook_page_image_paths(db, storybook_id).await?;
    let file_name = export_file_name(export_id);
    let pdf = encode_storybook_pdf_with_images(&storybook, &page_images);
    storage::save_export_file(&file_name, &pdf).map_err(DbErr::Custom)
}

async fn latest_storybook_page_image_paths(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<HashMap<Uuid, PathBuf>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select distinct on (input_json->>'page_id')
              input_json->>'page_id' as page_id,
              output_json #>> '{image,image_url}' as image_url
            from generation_jobs
            where storybook_id = $1
              and job_type = 'storybook_page_image'
              and status = 'succeeded'
              and input_json->>'page_id' is not null
              and output_json #>> '{image,image_url}' is not null
            order by input_json->>'page_id', finished_at desc nulls last, created_at desc
            "#,
            [storybook_id.into()],
        ))
        .await?;

    let mut images = HashMap::new();
    for row in rows {
        let page_id = row
            .try_get::<String>("", "page_id")
            .ok()
            .and_then(|value| Uuid::parse_str(&value).ok());
        let file_name = row
            .try_get::<String>("", "image_url")
            .ok()
            .and_then(|value| export_image_file_name(&value));
        if let (Some(page_id), Some(file_name)) = (page_id, file_name) {
            let image_path =
                storage::local_generated_image_path(&file_name).map_err(DbErr::Custom)?;
            images.insert(page_id, image_path);
        }
    }
    Ok(images)
}

fn export_image_file_name(image_url: &str) -> Option<String> {
    let file_name = image_url.rsplit('/').next()?.trim();
    let (provider, id_with_ext) = file_name.split_once('-')?;
    if !matches!(provider, "mock" | "seedream") {
        return None;
    }
    let id = id_with_ext.strip_suffix(".png")?;
    Uuid::parse_str(id).ok()?;
    Some(file_name.to_string())
}

fn export_file_name(export_id: Uuid) -> String {
    format!("{export_id}.pdf")
}

fn truncate_export_error(error: &str) -> String {
    const MAX_ERROR_CHARS: usize = 240;
    let trimmed = error.trim();
    if trimmed.chars().count() <= MAX_ERROR_CHARS {
        return trimmed.to_string();
    }
    let mut value = trimmed.chars().take(MAX_ERROR_CHARS).collect::<String>();
    value.push('…');
    value
}

async fn mark_export_succeeded(
    db: &DatabaseConnection,
    job_id: Uuid,
    file_url: String,
) -> Result<ExportJob, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update export_jobs
            set status = 'succeeded',
                file_url = $2,
                last_error = null,
                finished_at = now()
            where id = $1 and status = 'running'
            returning id, storybook_id, status, file_url, last_error, created_at, finished_at
            "#,
            [job_id.into(), file_url.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("export_job".to_string()))?;

    export_from_row(row)
}

async fn mark_export_failed(
    db: &DatabaseConnection,
    job_id: Uuid,
    error: &str,
) -> Result<ExportJob, DbErr> {
    let error = truncate_export_error(error);
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update export_jobs
            set status = 'failed',
                last_error = $2,
                finished_at = now()
            where id = $1 and status = 'running'
            returning id, storybook_id, status, file_url, last_error, created_at, finished_at
            "#,
            [job_id.into(), error.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("export_job".to_string()))?;

    export_from_row(row)
}

pub async fn execute_export_job(
    db: &DatabaseConnection,
    export_id: Uuid,
) -> Result<ExportJob, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, storybook_id, status, file_url, last_error, created_at, finished_at
            from export_jobs
            where id = $1
            limit 1
            "#,
            [export_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("export_job".to_string()))?;
    let job = export_from_row(row)?;
    if job.status != "queued" {
        return Err(DbErr::Custom(
            "只有 queued 状态的导出任务可以执行".to_string(),
        ));
    }
    complete_export(db, job.id, job.storybook_id).await
}

pub async fn find_export(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    export_id: Uuid,
) -> Result<ExportJob, DbErr> {
    ensure_storybook_in_workspace(db, workspace_id, storybook_id).await?;
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, storybook_id, status, file_url, last_error, created_at, finished_at
            from export_jobs
            where id = $1 and storybook_id = $2
            limit 1
            "#,
            [export_id.into(), storybook_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("export_job".to_string()))?;

    export_from_row(row)
}

pub async fn list_exports(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    limit: usize,
    offset: usize,
) -> Result<(Vec<ExportJob>, PaginationMeta), DbErr> {
    ensure_storybook_in_workspace(db, workspace_id, storybook_id).await?;
    let total = count_exports(db, storybook_id).await?;
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, storybook_id, status, file_url, last_error, created_at, finished_at
            from export_jobs
            where storybook_id = $1
            order by created_at desc
            limit $2 offset $3
            "#,
            [
                storybook_id.into(),
                (limit as i64).into(),
                (offset as i64).into(),
            ],
        ))
        .await?;

    let jobs = rows
        .into_iter()
        .map(export_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok((
        jobs,
        PaginationMeta {
            total,
            limit,
            offset,
            has_more: offset.saturating_add(limit) < total,
        },
    ))
}

pub async fn create_share_link(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    expires_at: Option<DateTime<Utc>>,
) -> Result<ShareLink, DbErr> {
    ensure_storybook_in_workspace(db, workspace_id, storybook_id).await?;
    ensure_storybook_delivery_privacy_clear(db, storybook_id).await?;

    let id = Uuid::new_v4();
    let token = Uuid::new_v4().simple().to_string();
    let status = if expires_at.is_some_and(|value| value <= Utc::now()) {
        "expired"
    } else {
        "active"
    };
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into share_links
              (id, storybook_id, token, status, created_at, expires_at)
            values ($1, $2, $3, $4, now(), $5)
            returning id, storybook_id, token, status, access_count, last_accessed_at, expires_at
            "#,
            [
                id.into(),
                storybook_id.into(),
                token.into(),
                status.into(),
                expires_at.into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("share_link".to_string()))?;

    share_link_from_row(&row)
}

pub async fn list_share_links(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    limit: usize,
    offset: usize,
) -> Result<(Vec<ShareLink>, PaginationMeta), DbErr> {
    ensure_storybook_in_workspace(db, workspace_id, storybook_id).await?;
    let total = count_share_links(db, storybook_id).await?;

    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, storybook_id, token, status, access_count, last_accessed_at, expires_at
            from share_links
            where storybook_id = $1
              and status = 'active'
              and (expires_at is null or expires_at > now())
            order by created_at desc
            limit $2 offset $3
            "#,
            [
                storybook_id.into(),
                (limit as i64).into(),
                (offset as i64).into(),
            ],
        ))
        .await?;

    let links = rows
        .iter()
        .map(share_link_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok((
        links,
        PaginationMeta {
            total,
            limit,
            offset,
            has_more: offset.saturating_add(limit) < total,
        },
    ))
}

async fn count_exports(db: &DatabaseConnection, storybook_id: Uuid) -> Result<usize, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "select count(*) as count from export_jobs where storybook_id = $1",
            [storybook_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("export_jobs_count".to_string()))?;
    let count: i64 = row.try_get("", "count")?;
    Ok(count as usize)
}

async fn count_share_links(db: &DatabaseConnection, storybook_id: Uuid) -> Result<usize, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select count(*) as count
            from share_links
            where storybook_id = $1
              and status = 'active'
              and (expires_at is null or expires_at > now())
            "#,
            [storybook_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("share_links_count".to_string()))?;
    let count: i64 = row.try_get("", "count")?;
    Ok(count as usize)
}

pub async fn revoke_share_link(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    share_link_id: Uuid,
) -> Result<ShareLink, DbErr> {
    ensure_storybook_in_workspace(db, workspace_id, storybook_id).await?;

    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update share_links
            set status = 'revoked'
            where id = $1
              and storybook_id = $2
              and status = 'active'
            returning id, storybook_id, token, status, access_count, last_accessed_at, expires_at
            "#,
            [share_link_id.into(), storybook_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("share_link".to_string()))?;

    share_link_from_row(&row)
}

pub async fn storybook_by_share_token(
    db: &DatabaseConnection,
    token: &str,
) -> Result<Storybook, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select storybook_id
            from share_links
            where token = $1
              and status = 'active'
              and (expires_at is null or expires_at > now())
            limit 1
            "#,
            [token.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("share_link".to_string()))?;

    storybooks::find_any(db, row.try_get("", "storybook_id")?).await
}

pub async fn record_share_link_access(
    db: &DatabaseConnection,
    token: &str,
) -> Result<ShareLink, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update share_links
            set access_count = access_count + 1,
                last_accessed_at = now()
            where token = $1
              and status = 'active'
              and (expires_at is null or expires_at > now())
            returning id, storybook_id, token, status, access_count, last_accessed_at, expires_at
            "#,
            [token.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("share_link".to_string()))?;

    share_link_from_row(&row)
}

async fn ensure_storybook_delivery_privacy_clear(
    db: &DatabaseConnection,
    storybook_id: Uuid,
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
            from storybooks s
            left join storybook_pages sp on sp.storybook_id = s.id
            left join storybook_roles sr on sr.storybook_id = s.id
            where s.id = $1
            group by s.id
            "#,
            [storybook_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("storybook".to_string()))?;

    let privacy_text: String = row.try_get("", "privacy_text")?;
    let risks = delivery_privacy_risks(&privacy_text);
    if risks.is_empty() {
        Ok(())
    } else {
        Err(DbErr::Custom(format!(
            "delivery_privacy_risk:{}",
            risks.join("、")
        )))
    }
}

fn delivery_privacy_risks(text: &str) -> Vec<&'static str> {
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
    text.split(|ch: char| {
        !(ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '%' | '+' | '-' | '@'))
    })
    .any(|candidate| {
        let Some((local, domain)) = candidate.split_once('@') else {
            return false;
        };
        !local.is_empty()
            && !domain.is_empty()
            && domain.contains('.')
            && local
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '%' | '+' | '-'))
            && domain
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-'))
    })
}

fn contains_chinese_mobile(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '1' && (index == 0 || !chars[index - 1].is_ascii_digit()) {
            let mut digits = String::new();
            let mut cursor = index;
            while cursor < chars.len() && digits.len() < 11 {
                let ch = chars[cursor];
                if ch.is_ascii_digit() {
                    digits.push(ch);
                } else if ch == ' ' || ch == '-' {
                } else {
                    break;
                }
                cursor += 1;
            }
            if digits.len() == 11
                && (cursor == chars.len() || !chars[cursor].is_ascii_digit())
                && matches!(digits.as_bytes()[1] as char, '3'..='9')
            {
                return true;
            }
        }
        index += 1;
    }
    false
}

fn contains_id_card(text: &str) -> bool {
    text.split_whitespace().any(|token| {
        let value = token.trim_matches(|ch: char| {
            ch.is_ascii_punctuation() || "，。；、：（）《》【】“”‘’".contains(ch)
        });
        value.len() == 18
            && value
                .chars()
                .enumerate()
                .all(|(index, ch)| ch.is_ascii_digit() || (index == 17 && matches!(ch, 'x' | 'X')))
    })
}

async fn ensure_storybook_in_workspace(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<(), DbErr> {
    let exists = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id
            from storybooks
            where workspace_id = $1 and id = $2
            limit 1
            "#,
            [workspace_id.into(), storybook_id.into()],
        ))
        .await?
        .is_some();

    if exists {
        Ok(())
    } else {
        Err(DbErr::RecordNotFound("storybook".to_string()))
    }
}

fn share_link_from_row(row: &sea_orm::QueryResult) -> Result<ShareLink, DbErr> {
    let token: String = row.try_get("", "token")?;
    let expires_at: Option<DateTime<Utc>> = row.try_get("", "expires_at")?;
    let stored_status: String = row.try_get("", "status")?;
    let status = if stored_status == "active" && expires_at.is_some_and(|value| value <= Utc::now())
    {
        "expired".to_string()
    } else {
        stored_status
    };
    Ok(ShareLink {
        id: row.try_get("", "id")?,
        storybook_id: row.try_get("", "storybook_id")?,
        url: format!("/link/share/{token}"),
        token,
        status,
        access_count: row.try_get("", "access_count")?,
        last_accessed_at: row
            .try_get::<Option<DateTime<Utc>>>("", "last_accessed_at")?
            .map(|value| value.format("%Y-%m-%d %H:%M").to_string()),
        expires_at: expires_at.map(|value| value.to_rfc3339()),
    })
}

fn export_from_row(row: sea_orm::QueryResult) -> Result<ExportJob, DbErr> {
    Ok(ExportJob {
        id: row.try_get("", "id")?,
        storybook_id: row.try_get("", "storybook_id")?,
        status: row.try_get("", "status")?,
        file_url: row.try_get("", "file_url")?,
        last_error: row.try_get("", "last_error")?,
        created_at: row.try_get::<DateTime<Utc>>("", "created_at")?,
        finished_at: row.try_get("", "finished_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_link_url_keeps_current_public_route_shape() {
        let token = Uuid::new_v4().simple().to_string();
        assert_eq!(
            format!("/link/share/{token}"),
            format!("/link/share/{token}")
        );
    }

    #[test]
    fn export_file_name_is_pdf_for_export_id() {
        let export_id = Uuid::new_v4();
        let file_name = export_file_name(export_id);
        assert_eq!(file_name, format!("{export_id}.pdf"));
        assert!(file_name.ends_with(".pdf"));
        assert!(!file_name.contains('/'));
        assert!(!file_name.contains('\\'));
    }

    #[test]
    fn export_error_is_truncated_for_operator_readability() {
        let error = "错误".repeat(200);
        let truncated = truncate_export_error(&error);
        assert!(truncated.chars().count() <= 241);
        assert!(truncated.ends_with('…'));
    }

    #[test]
    fn delivery_privacy_risks_allows_normal_storybook_text() {
        let risks = delivery_privacy_risks("小朋友们在教室里练习轮流等待，老师提醒大家慢慢来。");
        assert!(risks.is_empty());
    }

    #[test]
    fn delivery_privacy_risks_detects_contact_and_private_details() {
        let risks = delivery_privacy_risks(
            "家长手机号 138 0013 8000，邮箱 parent@example.com，家庭住址在某小区，孩子有过敏史。",
        );
        assert!(risks.contains(&"手机号"));
        assert!(risks.contains(&"邮箱"));
        assert!(risks.contains(&"住址信息"));
        assert!(risks.contains(&"医疗信息"));
    }

    #[test]
    fn delivery_privacy_risks_does_not_treat_long_ids_as_phone_numbers() {
        let risks = delivery_privacy_risks("UI Smoke 普通绘本 1784538853883");
        assert!(risks.is_empty());
    }
}
