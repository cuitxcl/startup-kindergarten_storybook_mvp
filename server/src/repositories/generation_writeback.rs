use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::models::GenerationJob;

pub async fn ensure_image_output_within_storage_quota(
    db: &DatabaseConnection,
    job: &GenerationJob,
    output_json: &JsonValue,
) -> Result<(), DbErr> {
    let Some(image_url) = output_json
        .get("image")
        .and_then(|value| value.get("image_url"))
        .and_then(|value| value.as_str())
    else {
        return Ok(());
    };

    match crate::repositories::storage_quota::ensure_workspace_storage_available_for_url_and_user(
        db,
        job.workspace_id,
        job.created_by,
        image_url,
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = crate::services::storage::delete_file_by_url(image_url);
            Err(err)
        }
    }
}

pub async fn apply_completed_generation(
    db: &DatabaseConnection,
    job: &GenerationJob,
) -> Result<(), DbErr> {
    if job.status != "succeeded" {
        return Ok(());
    }
    let Some(storybook_id) = job.storybook_id else {
        return Ok(());
    };
    let Some(output) = job.output_json.as_ref() else {
        return Ok(());
    };

    match job.job_type.as_str() {
        "storybook_roles" => {
            replace_roles_from_generation(db, storybook_id, output).await?;
            // 角色道具生成成功后流转到「角色待确认」；只从「方案待确认」前进，不回退更后面的状态。
            db.execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "update storybooks set status = 'roles_pending', updated_at = now() where id = $1 and status = 'plan_pending'",
                [storybook_id.into()],
            ))
            .await?;
            Ok(())
        }
        "storybook_pages" => {
            replace_pages_from_generation(db, storybook_id, output).await?;
            // 分页生成成功后流转到「编辑中」；只前进不回退（如已在插图/导出阶段则不动）。
            db.execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "update storybooks set status = 'editing', updated_at = now() where id = $1 and status in ('plan_pending', 'roles_pending')",
                [storybook_id.into()],
            ))
            .await?;
            Ok(())
        }
        "storybook_page_prompt" => apply_page_prompt_rewrite(db, storybook_id, job, output).await,
        "storybook_role_reference_image" => {
            apply_role_reference_image(db, storybook_id, job, output).await
        }
        _ => Ok(()),
    }
}

/// 单页插图描述重写：写回 illustration_prompt，并把已有插图的页面标记为待重新生成。
async fn apply_page_prompt_rewrite(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    job: &GenerationJob,
    output: &JsonValue,
) -> Result<(), DbErr> {
    let page_id = job
        .input_json
        .get("page_id")
        .and_then(|value| value.as_str())
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| DbErr::Custom("插图描述重写任务缺少 page_id，无法写回".to_string()))?;
    let prompt = output
        .get("page")
        .and_then(|value| value.get("illustration_prompt"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| DbErr::Custom("插图描述重写输出缺少 illustration_prompt".to_string()))?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_pages
        set illustration_prompt = $3,
            status = case
                when status = 'ready' then 'needs_regeneration'
                else status
            end
        where storybook_id = $1 and id = $2
        "#,
        [
            storybook_id.into(),
            page_id.into(),
            prompt.to_string().into(),
        ],
    ))
    .await?;

    // 内容发生变化，重置老师审核状态。
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybooks
        set updated_at = now(),
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

async fn apply_role_reference_image(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    job: &GenerationJob,
    output: &JsonValue,
) -> Result<(), DbErr> {
    let role_id = job
        .input_json
        .get("role_id")
        .and_then(|value| value.as_str())
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| DbErr::Custom("角色参考图任务缺少 role_id，无法写回".to_string()))?;
    let image_url = output
        .get("image")
        .and_then(|value| value.get("image_url"))
        .and_then(|value| value.as_str())
        .ok_or_else(|| DbErr::Custom("角色参考图输出缺少 image_url".to_string()))?;
    let prompt = output
        .get("image")
        .and_then(|value| value.get("prompt"))
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let selected_variant = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, is_selected
            from storybook_image_variants
            where generation_job_id = $1
            limit 1
            "#,
            [job.id.into()],
        ))
        .await?;
    let variant_id = selected_variant
        .as_ref()
        .and_then(|row| row.try_get::<Uuid>("", "id").ok());
    let is_selected = selected_variant
        .as_ref()
        .and_then(|row| row.try_get::<bool>("", "is_selected").ok())
        .unwrap_or(false);

    if !is_selected {
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update storybook_roles
            set reference_status = 'ready'
            where storybook_id = $1 and id = $2
            "#,
            [storybook_id.into(), role_id.into()],
        ))
        .await?;
        return touch_storybook(db, storybook_id).await;
    }

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_roles
        set reference_image_url = $3,
            reference_image_prompt = $4,
            reference_status = 'ready',
            selected_image_variant_id = $5
        where storybook_id = $1 and id = $2
        "#,
        [
            storybook_id.into(),
            role_id.into(),
            image_url.to_string().into(),
            prompt.to_string().into(),
            variant_id.into(),
        ],
    ))
    .await?;
    touch_storybook(db, storybook_id).await
}

async fn replace_roles_from_generation(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    output: &JsonValue,
) -> Result<(), DbErr> {
    let Some(roles) = output.get("roles").and_then(|value| value.as_array()) else {
        return Ok(());
    };
    if roles.is_empty() {
        return Ok(());
    }

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "delete from storybook_roles where storybook_id = $1",
        [storybook_id.into()],
    ))
    .await?;

    for role in roles {
        let id = Uuid::new_v4();
        let role_type = normalized_role_type(&json_text(role, "role_type", "supporting"));
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into storybook_roles
              (id, storybook_id, name, role_type, appearance, story_function, needs_consistency,
               reference_image_url, reference_image_prompt, reference_status)
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            [
                id.into(),
                storybook_id.into(),
                json_text(role, "name", "未命名角色").into(),
                role_type.clone().into(),
                clean_visual_appearance(&json_text(role, "appearance", "待确认外观")).into(),
                json_text(role, "story_function", "参与故事推进").into(),
                role.get("needs_consistency")
                    .and_then(|value| value.as_bool())
                    .unwrap_or_else(|| default_needs_consistency(&role_type))
                    .into(),
                json_optional_text(role, "reference_image_url").into(),
                json_optional_text(role, "reference_image_prompt").into(),
                json_text(role, "reference_status", "not_started").into(),
            ],
        ))
        .await?;
    }

    touch_storybook(db, storybook_id).await
}

async fn replace_pages_from_generation(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    output: &JsonValue,
) -> Result<(), DbErr> {
    let Some(pages) = output.get("pages").and_then(|value| value.as_array()) else {
        return Ok(());
    };
    if pages.is_empty() {
        return Ok(());
    }

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "delete from storybook_pages where storybook_id = $1",
        [storybook_id.into()],
    ))
    .await?;

    for (index, page) in pages.iter().enumerate() {
        let id = Uuid::new_v4();
        let page_number = page
            .get("page_number")
            .and_then(|value| value.as_i64())
            .unwrap_or((index + 1) as i64)
            .max(1) as i32;
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into storybook_pages
              (id, storybook_id, page_number, title, body, illustration_prompt, status)
            values ($1, $2, $3, $4, $5, $6, $7)
            "#,
            [
                id.into(),
                storybook_id.into(),
                page_number.into(),
                json_text(page, "title", "未命名分页").into(),
                json_text(page, "body", "待补充分页正文。").into(),
                json_text(page, "illustration_prompt", "待补充插图描述。").into(),
                json_text(page, "status", "draft").into(),
            ],
        ))
        .await?;
    }

    touch_storybook(db, storybook_id).await
}

async fn touch_storybook(db: &DatabaseConnection, storybook_id: Uuid) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "update storybooks set updated_at = now() where id = $1",
        [storybook_id.into()],
    ))
    .await?;
    Ok(())
}

fn json_text(value: &JsonValue, key: &str, fallback: &str) -> String {
    value
        .get(key)
        .and_then(|item| item.as_str())
        .filter(|item| !item.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn json_optional_text(value: &JsonValue, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|item| item.as_str())
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn clean_visual_appearance(value: &str) -> String {
    let behavior_keywords = [
        "喜欢",
        "总喜欢",
        "经常",
        "常常",
        "总是",
        "常和",
        "离开队伍",
        "交流",
        "适合",
        "带领",
        "制定",
        "强调",
        "学习",
        "代表",
        "推动",
        "帮助",
        "引导",
        "鼓励",
        "提醒",
        "跑",
        "跳",
        "蹦",
        "玩",
        "等待",
        "分享",
    ];
    let parts: Vec<&str> = value
        .split(|ch| matches!(ch, '，' | ',' | '。' | '；' | ';' | '、'))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .filter(|part| {
            !behavior_keywords
                .iter()
                .any(|keyword| part.contains(keyword))
        })
        .collect();
    if parts.is_empty() {
        value.trim().to_string()
    } else {
        parts.join("，")
    }
}

fn normalized_role_type(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "protagonist" | "main" | "主角" => "protagonist",
        "teacher" | "guide" | "老师" | "教师" | "引导者" | "向导" => "teacher",
        "peer" | "companion" | "同伴" | "朋友" | "伙伴" => "peer",
        "prop" | "tool" | "object" | "道具" | "关键道具" => "prop",
        "supporting" | "配角" | "背景角色" => "supporting",
        _ => "supporting",
    }
    .to_string()
}

fn default_needs_consistency(role_type: &str) -> bool {
    matches!(role_type, "protagonist" | "teacher")
}
