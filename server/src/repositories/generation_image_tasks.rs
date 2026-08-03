use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

use crate::models::{CreateImageTaskRequest, GenerationJob};
use crate::services::generation_provider::{ImageGenerationMode, ImageReference};

pub struct PageImageRequestInput {
    pub prompt: String,
    pub reference_images: Vec<ImageReference>,
    pub edit_instruction: Option<String>,
    pub image_mode: ImageGenerationMode,
    pub strength: Option<f32>,
}

pub struct ImageJobTarget {
    pub target_id: String,
    pub target_type: &'static str,
}

pub fn is_image_job(job_type: &str) -> bool {
    matches!(
        job_type,
        "storybook_page_image" | "storybook_role_reference_image"
    )
}

pub async fn page_image_job_input(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    page_id: Uuid,
    payload: CreateImageTaskRequest,
) -> Result<JsonValue, DbErr> {
    let page_prompt = page_prompt(db, workspace_id, storybook_id, page_id).await?;
    let prompt = payload
        .prompt
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(page_prompt);
    let mut reference_images = page_image_reference_images(db, storybook_id, &payload).await?;
    // 第三档场景延续：上一页已有插图时，把它作为场景参考图带上；没有则静默降级。
    let scene_reference = previous_page_scene_reference(db, storybook_id, page_id).await?;
    let has_scene_reference = scene_reference.is_some();
    if let Some(scene) = scene_reference {
        if !reference_images.iter().any(|item| item.url == scene.url) {
            reference_images.push(scene);
        }
    }
    let prompt = if has_scene_reference {
        format!("{prompt} 参考上一页画面保持场景布局与在场人群连续，动作与构图以文字描述为准。")
    } else {
        prompt
    };
    let image_mode =
        normalize_image_mode(payload.image_mode.as_deref(), !reference_images.is_empty());
    let edit_instruction = clean_optional_text(payload.edit_instruction);
    // 场景参考图容易把构图拉得过于雷同，带场景参考且未显式指定强度时收敛到 0.5。
    let strength = payload
        .strength
        .map(|value| value.clamp(0.0, 1.0))
        .or(has_scene_reference.then_some(0.5));

    Ok(json!({
        "page_id": page_id,
        "prompt": prompt,
        "mode": "storybook_page_image",
        "image_mode": image_mode.as_str(),
        "reference_role_ids": payload.reference_role_ids,
        "reference_images": reference_images,
        "edit_instruction": edit_instruction,
        "strength": strength
    }))
}

pub async fn role_reference_image_job_input(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    role_id: Uuid,
    payload: CreateImageTaskRequest,
) -> Result<JsonValue, DbErr> {
    let role_prompt = role_reference_prompt(db, workspace_id, storybook_id, role_id).await?;
    let prompt = payload
        .prompt
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(role_prompt);
    let reference_images = clean_reference_image_urls(&payload.reference_image_urls)
        .into_iter()
        .map(|url| ImageReference {
            url,
            source: "direct".to_string(),
            role_id: None,
            label: None,
        })
        .collect::<Vec<_>>();
    let image_mode =
        normalize_image_mode(payload.image_mode.as_deref(), !reference_images.is_empty());

    Ok(json!({
        "role_id": role_id,
        "prompt": prompt,
        "mode": "storybook_role_reference_image",
        "image_mode": image_mode.as_str(),
        "reference_images": reference_images,
        "edit_instruction": clean_optional_text(payload.edit_instruction),
        "strength": payload.strength.map(|value| value.clamp(0.0, 1.0))
    }))
}

pub fn image_target_from_job(job: &GenerationJob) -> Result<ImageJobTarget, DbErr> {
    if job.job_type == "storybook_role_reference_image" {
        let role_id = job
            .input_json
            .get("role_id")
            .and_then(|value| value.as_str())
            .ok_or_else(|| DbErr::Custom("角色参考图任务缺少 role_id，无法执行".to_string()))?;
        Ok(ImageJobTarget {
            target_id: role_id.to_string(),
            target_type: "role",
        })
    } else {
        let page_id = job
            .input_json
            .get("page_id")
            .and_then(|value| value.as_str())
            .ok_or_else(|| DbErr::Custom("插图任务缺少 page_id，无法执行".to_string()))?;
        Ok(ImageJobTarget {
            target_id: page_id.to_string(),
            target_type: "page",
        })
    }
}

pub fn image_request_from_job(job: &GenerationJob) -> Result<PageImageRequestInput, DbErr> {
    let prompt = job
        .input_json
        .get("prompt")
        .and_then(|value| value.as_str())
        .ok_or_else(|| DbErr::Custom("插图任务缺少 prompt，无法执行".to_string()))?
        .to_string();
    let reference_images = job
        .input_json
        .get("reference_images")
        .and_then(|value| serde_json::from_value::<Vec<ImageReference>>(value.clone()).ok())
        .unwrap_or_default();
    let image_mode = normalize_image_mode(
        job.input_json
            .get("image_mode")
            .and_then(|value| value.as_str()),
        !reference_images.is_empty(),
    );
    let edit_instruction = job
        .input_json
        .get("edit_instruction")
        .and_then(|value| value.as_str())
        .and_then(|value| clean_optional_text(Some(value.to_string())));
    let strength = job
        .input_json
        .get("strength")
        .and_then(|value| value.as_f64())
        .map(|value| (value as f32).clamp(0.0, 1.0));

    Ok(PageImageRequestInput {
        prompt,
        reference_images,
        edit_instruction,
        image_mode,
        strength,
    })
}

async fn page_image_reference_images(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    payload: &CreateImageTaskRequest,
) -> Result<Vec<ImageReference>, DbErr> {
    let mut references = Vec::new();
    let selected_roles = payload
        .reference_role_ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();

    if !payload.reference_role_ids.is_empty() {
        let rows = db
            .query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                select id, name, reference_image_url
                from storybook_roles
                where storybook_id = $1
                  and reference_image_url is not null
                "#,
                [storybook_id.into()],
            ))
            .await?;
        for row in rows {
            let role_id: Uuid = row.try_get("", "id")?;
            if !selected_roles.contains(&role_id) {
                continue;
            }
            let Some(url) =
                clean_optional_text(row.try_get::<Option<String>>("", "reference_image_url")?)
            else {
                continue;
            };
            references.push(ImageReference {
                url,
                source: "storybook_role".to_string(),
                role_id: Some(role_id.to_string()),
                label: row.try_get("", "name").ok(),
            });
        }
    }

    for url in clean_reference_image_urls(&payload.reference_image_urls) {
        if references.iter().any(|item| item.url == url) {
            continue;
        }
        references.push(ImageReference {
            url,
            source: "direct".to_string(),
            role_id: None,
            label: None,
        });
    }

    Ok(references)
}

fn normalize_image_mode(value: Option<&str>, has_reference_images: bool) -> ImageGenerationMode {
    match value.map(str::trim) {
        Some("edit_image") => ImageGenerationMode::EditImage,
        Some("reference_image") => ImageGenerationMode::ReferenceImage,
        _ if has_reference_images => ImageGenerationMode::ReferenceImage,
        _ => ImageGenerationMode::TextToImage,
    }
}

fn clean_reference_image_urls(urls: &[String]) -> Vec<String> {
    let mut cleaned = Vec::new();
    for url in urls {
        let Some(url) = clean_optional_text(Some(url.clone())) else {
            continue;
        };
        if cleaned.iter().any(|item| item == &url) {
            continue;
        }
        cleaned.push(url);
    }
    cleaned
}

fn clean_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// 查找上一页最近一次成功插图的图片地址，作为本页的场景参考图。
/// 上一页没有已生成插图时返回 None，调用方静默降级为普通生成。
async fn previous_page_scene_reference(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    page_id: Uuid,
) -> Result<Option<ImageReference>, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select gj.output_json->'image'->>'image_url' as image_url
            from storybook_pages current_page
            join storybook_pages prev_page
              on prev_page.storybook_id = current_page.storybook_id
             and prev_page.page_number = current_page.page_number - 1
            join generation_jobs gj
              on gj.storybook_id = current_page.storybook_id
             and gj.job_type = 'storybook_page_image'
             and gj.status = 'succeeded'
             and gj.input_json->>'page_id' = prev_page.id::text
            where current_page.storybook_id = $1 and current_page.id = $2
            order by gj.created_at desc
            limit 1
            "#,
            [storybook_id.into(), page_id.into()],
        ))
        .await?;

    let Some(row) = row else {
        return Ok(None);
    };
    let Some(url) = clean_optional_text(row.try_get::<Option<String>>("", "image_url")?) else {
        return Ok(None);
    };
    Ok(Some(ImageReference {
        url,
        source: "previous_page".to_string(),
        role_id: None,
        label: Some("上一页画面".to_string()),
    }))
}

async fn page_prompt(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    page_id: Uuid,
) -> Result<String, DbErr> {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        select p.illustration_prompt
        from storybook_pages p
        join storybooks s on s.id = p.storybook_id
        where s.workspace_id = $1 and s.id = $2 and p.id = $3
        limit 1
        "#,
        [workspace_id.into(), storybook_id.into(), page_id.into()],
    ))
    .await?
    .ok_or_else(|| DbErr::RecordNotFound("page".to_string()))?
    .try_get("", "illustration_prompt")
}

async fn role_reference_prompt(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    role_id: Uuid,
) -> Result<String, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select r.name, r.role_type, r.appearance
            from storybook_roles r
            join storybooks s on s.id = r.storybook_id
            where s.workspace_id = $1 and s.id = $2 and r.id = $3
            limit 1
            "#,
            [workspace_id.into(), storybook_id.into(), role_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("role".to_string()))?;
    let name: String = row.try_get("", "name")?;
    let role_type: String = row.try_get("", "role_type")?;
    let appearance: String = row.try_get("", "appearance")?;
    Ok(format!(
        "为绘本生成单一角色标准参考图。角色名：{name}；视觉类型：{role_type}；外观：{appearance}。要求：白底或简洁背景，柔和水彩绘本风格，圆润饱满造型，大而富有表现力的眼睛；角色表情自然生动、富有神采，姿态自然放松，可微微侧身或采用三分之四视角，全身或半身清晰；画面中只有这个角色，无人类，无其他角色，便于后续分页插图保持一致。不要加入故事情节动作或分页场景，不要僵硬对称的证件照式站姿。"
    ))
}
