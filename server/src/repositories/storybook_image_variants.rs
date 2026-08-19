use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement, TransactionTrait};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::models::{GenerationJob, ImageVariantListQuery, StorybookImageVariant};

pub const TARGET_ROLE_REFERENCE: &str = "role_reference";
pub const TARGET_PAGE_ILLUSTRATION: &str = "page_illustration";
pub const TARGET_COVER_ILLUSTRATION: &str = "cover_illustration";

pub async fn create_generating_variant_for_job(
    db: &impl ConnectionTrait,
    job: &GenerationJob,
) -> Result<StorybookImageVariant, DbErr> {
    let Some(storybook_id) = job.storybook_id else {
        return Err(DbErr::Custom("图片任务缺少 storybook_id".to_string()));
    };
    let (target_type, target_id) = target_from_job(job)?;
    let prompt = job
        .input_json
        .get("prompt")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());

    let id = Uuid::new_v4();
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into storybook_image_variants
              (id, workspace_id, storybook_id, target_type, target_id, generation_job_id, prompt, status, created_at, updated_at)
            values ($1, $2, $3, $4, $5, $6, $7, 'generating', now(), now())
            returning id, workspace_id, storybook_id, target_type, target_id, generation_job_id,
                      image_url, prompt, provider, status, failure_reason, is_selected, created_at, updated_at
            "#,
            [
                id.into(),
                job.workspace_id.into(),
                storybook_id.into(),
                target_type.into(),
                target_id.into(),
                job.id.into(),
                prompt.into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("storybook_image_variant".to_string()))?;

    variant_from_row(row)
}

pub async fn ensure_variant_for_job(
    db: &impl ConnectionTrait,
    job: &GenerationJob,
) -> Result<StorybookImageVariant, DbErr> {
    if let Some(row) = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, workspace_id, storybook_id, target_type, target_id, generation_job_id,
                   image_url, prompt, provider, status, failure_reason, is_selected, created_at, updated_at
            from storybook_image_variants
            where generation_job_id = $1
            limit 1
            "#,
            [job.id.into()],
        ))
        .await?
    {
        return variant_from_row(row);
    }

    create_generating_variant_for_job(db, job).await
}

pub async fn mark_job_variant_ready(
    db: &impl ConnectionTrait,
    job: &GenerationJob,
) -> Result<(), DbErr> {
    let output = job
        .output_json
        .as_ref()
        .ok_or_else(|| DbErr::Custom("图片任务缺少 output_json".to_string()))?;
    let Some(image_url) = output
        .get("image")
        .and_then(|value| value.get("image_url"))
        .and_then(|value| value.as_str())
    else {
        return Err(DbErr::Custom("图片输出缺少 image_url".to_string()));
    };
    let prompt = output
        .get("image")
        .and_then(|value| value.get("prompt"))
        .and_then(|value| value.as_str())
        .or_else(|| {
            job.input_json
                .get("prompt")
                .and_then(|value| value.as_str())
        })
        .unwrap_or_default()
        .to_string();
    let provider = output
        .get("provider")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();

    ensure_variant_for_job(db, job).await?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_image_variants
        set image_url = $2,
            prompt = $3,
            provider = nullif($4, ''),
            status = 'ready',
            failure_reason = null,
            updated_at = now()
        where generation_job_id = $1
        "#,
        [
            job.id.into(),
            image_url.to_string().into(),
            prompt.into(),
            provider.into(),
        ],
    ))
    .await?;

    // 角色参考图是跨页一致性的当前基准。重新生成后必须立即切换到新图，
    // 否则角色已更新为 ready 但封面和分页仍会继续引用旧图。
    if should_select_completed_variant(&job.job_type, target_has_selected_variant(db, job).await?) {
        select_variant_for_job(db, job).await?;
    }
    Ok(())
}

fn should_select_completed_variant(job_type: &str, target_has_selected_variant: bool) -> bool {
    job_type == "storybook_role_reference_image" || !target_has_selected_variant
}

pub async fn mark_job_variant_failed(
    db: &impl ConnectionTrait,
    job: &GenerationJob,
    failure_reason: &str,
) -> Result<(), DbErr> {
    ensure_variant_for_job(db, job).await?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_image_variants
        set status = 'failed',
            failure_reason = $2,
            updated_at = now()
        where generation_job_id = $1
        "#,
        [job.id.into(), failure_reason.to_string().into()],
    ))
    .await?;
    Ok(())
}

pub async fn list_variants(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    query: ImageVariantListQuery,
) -> Result<Vec<StorybookImageVariant>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, workspace_id, storybook_id, target_type, target_id, generation_job_id,
                   image_url, prompt, provider, status, failure_reason, is_selected, created_at, updated_at
            from storybook_image_variants
            where workspace_id = $1
              and storybook_id = $2
              and ($3::text is null or target_type = $3)
              and ($4::uuid is null or target_id = $4)
            order by created_at desc, id desc
            "#,
            [
                workspace_id.into(),
                storybook_id.into(),
                query.target_type.into(),
                query.target_id.into(),
            ],
        ))
        .await?;
    rows.into_iter().map(variant_from_row).collect()
}

pub async fn select_variant(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    variant_id: Uuid,
) -> Result<StorybookImageVariant, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, workspace_id, storybook_id, target_type, target_id, generation_job_id,
                   image_url, prompt, provider, status, failure_reason, is_selected, created_at, updated_at
            from storybook_image_variants
            where workspace_id = $1 and storybook_id = $2 and id = $3
            limit 1
            "#,
            [workspace_id.into(), storybook_id.into(), variant_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("storybook_image_variant".to_string()))?;
    let variant = variant_from_row(row)?;
    if variant.status != "ready" {
        return Err(DbErr::Custom("只能选择已生成成功的图片".to_string()));
    }
    ensure_variant_target_is_current(db, &variant).await?;

    select_variant_by_id(db, &variant).await?;
    find_variant(db, workspace_id, storybook_id, variant_id).await
}

pub async fn selected_page_image_paths(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<Vec<(Uuid, String)>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select p.id as page_id, v.image_url
            from storybook_pages p
            join storybooks s on s.id = p.storybook_id
            join storybook_image_variants v on v.id = p.selected_image_variant_id
            where p.storybook_id = $1
              and v.status = 'ready'
              and v.image_url is not null
            "#,
            [storybook_id.into()],
        ))
        .await?;
    rows.into_iter()
        .filter_map(|row| {
            let page_id = row.try_get("", "page_id").ok()?;
            let image_url = row.try_get("", "image_url").ok()?;
            Some(Ok((page_id, image_url)))
        })
        .collect()
}

pub async fn selected_cover_image_path(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<Option<String>, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select image_url
            from storybook_image_variants
            where storybook_id = $1
              and target_type = 'cover_illustration'
              and target_id = $1
              and status = 'ready'
              and image_url is not null
              and is_selected
            order by updated_at desc
            limit 1
            "#,
            [storybook_id.into()],
        ))
        .await?;
    row.map(|row| row.try_get("", "image_url")).transpose()
}

fn target_from_job(job: &GenerationJob) -> Result<(&'static str, Uuid), DbErr> {
    if job.job_type == "storybook_role_reference_image" {
        let role_id = input_uuid(&job.input_json, "role_id", "角色参考图任务缺少 role_id")?;
        Ok((TARGET_ROLE_REFERENCE, role_id))
    } else if job.job_type == "storybook_page_image" {
        let page_id = input_uuid(&job.input_json, "page_id", "插图任务缺少 page_id")?;
        Ok((TARGET_PAGE_ILLUSTRATION, page_id))
    } else if job.job_type == "storybook_cover_image" {
        let Some(storybook_id) = job.storybook_id else {
            return Err(DbErr::Custom("封面图任务缺少 storybook_id".to_string()));
        };
        let cover_id = input_uuid(&job.input_json, "cover_id", "封面图任务缺少 cover_id")?;
        if cover_id != storybook_id {
            return Err(DbErr::Custom(
                "封面图任务 cover_id 与 storybook_id 不一致".to_string(),
            ));
        }
        Ok((TARGET_COVER_ILLUSTRATION, cover_id))
    } else {
        Err(DbErr::Custom("不是图片生成任务".to_string()))
    }
}

fn input_uuid(input: &JsonValue, key: &str, message: &str) -> Result<Uuid, DbErr> {
    input
        .get(key)
        .and_then(|value| value.as_str())
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| DbErr::Custom(message.to_string()))
}

async fn target_has_selected_variant(
    db: &impl ConnectionTrait,
    job: &GenerationJob,
) -> Result<bool, DbErr> {
    let Some(storybook_id) = job.storybook_id else {
        return Ok(false);
    };
    let (target_type, target_id) = target_from_job(job)?;
    let exists = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id
            from storybook_image_variants
            where workspace_id = $1
              and storybook_id = $2
              and target_type = $3
              and target_id = $4
              and is_selected
            limit 1
            "#,
            [
                job.workspace_id.into(),
                storybook_id.into(),
                target_type.into(),
                target_id.into(),
            ],
        ))
        .await?
        .is_some();
    Ok(exists)
}

async fn select_variant_for_job(
    db: &impl ConnectionTrait,
    job: &GenerationJob,
) -> Result<(), DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, workspace_id, storybook_id, target_type, target_id, generation_job_id,
                   image_url, prompt, provider, status, failure_reason, is_selected, created_at, updated_at
            from storybook_image_variants
            where generation_job_id = $1
            limit 1
            "#,
            [job.id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("storybook_image_variant".to_string()))?;
    let variant = variant_from_row(row)?;
    select_variant_by_id_on_conn(db, &variant).await
}

async fn ensure_variant_target_is_current(
    db: &impl ConnectionTrait,
    variant: &StorybookImageVariant,
) -> Result<(), DbErr> {
    if variant.target_type == TARGET_COVER_ILLUSTRATION {
        return Ok(());
    }
    let Some(job_id) = variant.generation_job_id else {
        return Err(DbErr::Custom(
            "图片变体缺少生成任务，无法确认是否仍匹配当前内容".to_string(),
        ));
    };
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select input_json
            from generation_jobs
            where id = $1
            limit 1
            "#,
            [job_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("generation_job".to_string()))?;
    let input_json: JsonValue = row.try_get("", "input_json")?;
    let Some(snapshot) = input_json.get("target_snapshot") else {
        return Err(DbErr::Custom(
            "图片变体缺少目标快照，无法安全选择；请重新生成图片".to_string(),
        ));
    };

    if variant.target_type == TARGET_PAGE_ILLUSTRATION {
        let current = db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                select title, body, illustration_prompt
                from storybook_pages
                where storybook_id = $1 and id = $2
                limit 1
                "#,
                [variant.storybook_id.into(), variant.target_id.into()],
            ))
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("page".to_string()))?;
        if snapshot.get("title").and_then(JsonValue::as_str)
            != Some(current.try_get::<String>("", "title")?.as_str())
            || snapshot.get("body").and_then(JsonValue::as_str)
                != Some(current.try_get::<String>("", "body")?.as_str())
            || snapshot
                .get("illustration_prompt")
                .and_then(JsonValue::as_str)
                != Some(
                    current
                        .try_get::<String>("", "illustration_prompt")?
                        .as_str(),
                )
        {
            return Err(DbErr::Custom(
                "图片变体对应的页面内容已变化，请重新生成插图".to_string(),
            ));
        }
    } else if variant.target_type == TARGET_ROLE_REFERENCE {
        let current = db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                select name, role_type, appearance, coalesce(story_function, '') as story_function, needs_consistency
                from storybook_roles
                where storybook_id = $1 and id = $2
                limit 1
                "#,
                [variant.storybook_id.into(), variant.target_id.into()],
            ))
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("role".to_string()))?;
        if snapshot.get("name").and_then(JsonValue::as_str)
            != Some(current.try_get::<String>("", "name")?.as_str())
            || snapshot.get("role_type").and_then(JsonValue::as_str)
                != Some(current.try_get::<String>("", "role_type")?.as_str())
            || snapshot.get("appearance").and_then(JsonValue::as_str)
                != Some(current.try_get::<String>("", "appearance")?.as_str())
            || snapshot.get("story_function").and_then(JsonValue::as_str)
                != Some(current.try_get::<String>("", "story_function")?.as_str())
            || snapshot
                .get("needs_consistency")
                .and_then(JsonValue::as_bool)
                != Some(current.try_get::<bool>("", "needs_consistency")?)
        {
            return Err(DbErr::Custom(
                "图片变体对应的角色设定已变化，请重新生成参考图".to_string(),
            ));
        }
    }
    Ok(())
}

async fn select_variant_by_id(
    db: &DatabaseConnection,
    variant: &StorybookImageVariant,
) -> Result<(), DbErr> {
    let txn = db.begin().await?;
    select_variant_by_id_on_conn(&txn, variant).await?;
    txn.commit().await?;
    Ok(())
}

async fn select_variant_by_id_on_conn(
    txn: &impl ConnectionTrait,
    variant: &StorybookImageVariant,
) -> Result<(), DbErr> {
    txn.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_image_variants
        set is_selected = false,
            updated_at = now()
        where workspace_id = $1
          and storybook_id = $2
          and target_type = $3
          and target_id = $4
          and is_selected
        "#,
        [
            variant.workspace_id.into(),
            variant.storybook_id.into(),
            variant.target_type.clone().into(),
            variant.target_id.into(),
        ],
    ))
    .await?;

    txn.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_image_variants
        set is_selected = true,
            updated_at = now()
        where id = $1
        "#,
        [variant.id.into()],
    ))
    .await?;

    if variant.target_type == TARGET_ROLE_REFERENCE {
        txn.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update storybook_roles
            set selected_image_variant_id = $3,
                reference_image_url = $4,
                reference_image_prompt = coalesce($5, reference_image_prompt),
                reference_status = 'ready'
            where storybook_id = $1 and id = $2
            "#,
            [
                variant.storybook_id.into(),
                variant.target_id.into(),
                variant.id.into(),
                variant.image_url.clone().into(),
                variant.prompt.clone().into(),
            ],
        ))
        .await?;
    } else if variant.target_type == TARGET_PAGE_ILLUSTRATION {
        txn.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update storybook_pages
            set selected_image_variant_id = $3,
                status = 'ready'
            where storybook_id = $1 and id = $2
            "#,
            [
                variant.storybook_id.into(),
                variant.target_id.into(),
                variant.id.into(),
            ],
        ))
        .await?;
    }

    crate::repositories::storybook_lifecycle::mark_storybook_content_changed(
        txn,
        variant.storybook_id,
    )
    .await?;
    Ok(())
}

async fn find_variant(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    variant_id: Uuid,
) -> Result<StorybookImageVariant, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, workspace_id, storybook_id, target_type, target_id, generation_job_id,
                   image_url, prompt, provider, status, failure_reason, is_selected, created_at, updated_at
            from storybook_image_variants
            where workspace_id = $1 and storybook_id = $2 and id = $3
            limit 1
            "#,
            [workspace_id.into(), storybook_id.into(), variant_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("storybook_image_variant".to_string()))?;
    variant_from_row(row)
}

fn variant_from_row(row: sea_orm::QueryResult) -> Result<StorybookImageVariant, DbErr> {
    Ok(StorybookImageVariant {
        id: row.try_get("", "id")?,
        workspace_id: row.try_get("", "workspace_id")?,
        storybook_id: row.try_get("", "storybook_id")?,
        target_type: row.try_get("", "target_type")?,
        target_id: row.try_get("", "target_id")?,
        generation_job_id: row.try_get("", "generation_job_id")?,
        image_url: row.try_get("", "image_url")?,
        prompt: row.try_get("", "prompt")?,
        provider: row.try_get("", "provider")?,
        status: row.try_get("", "status")?,
        failure_reason: row.try_get("", "failure_reason")?,
        is_selected: row.try_get("", "is_selected")?,
        created_at: row.try_get::<DateTime<Utc>>("", "created_at")?,
        updated_at: row.try_get::<DateTime<Utc>>("", "updated_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::should_select_completed_variant;

    #[test]
    fn regenerated_role_reference_becomes_current_variant() {
        assert!(should_select_completed_variant(
            "storybook_role_reference_image",
            true
        ));
        assert!(should_select_completed_variant(
            "storybook_role_reference_image",
            false
        ));
        assert!(!should_select_completed_variant(
            "storybook_page_image",
            true
        ));
    }
}
