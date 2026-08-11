use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

use crate::models::{CreateImageTaskRequest, GenerationJob};
use crate::page_aspect::{image_size_for_aspect_with_fallback, page_aspect_spec};
use crate::services::generation_provider::{ImageGenerationMode, ImageReference};

pub struct PageImageRequestInput {
    pub prompt: String,
    pub reference_images: Vec<ImageReference>,
    pub edit_instruction: Option<String>,
    pub image_mode: ImageGenerationMode,
    pub strength: Option<f32>,
    pub size: Option<String>,
}

pub struct ImageJobTarget {
    pub target_id: String,
    pub target_type: &'static str,
}

pub fn is_image_job(job_type: &str) -> bool {
    matches!(
        job_type,
        "storybook_page_image" | "storybook_role_reference_image" | "storybook_cover_image"
    )
}

pub async fn cover_image_job_input(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    payload: CreateImageTaskRequest,
) -> Result<JsonValue, DbErr> {
    let default_prompt = cover_prompt(db, workspace_id, storybook_id).await?;
    let aspect_ratio = storybook_page_aspect_ratio(db, workspace_id, storybook_id).await?;
    let aspect = page_aspect_spec(&aspect_ratio);
    let prompt = payload
        .prompt
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(default_prompt);
    let reference_images = storybook_role_reference_images(db, storybook_id).await?;
    let image_mode =
        normalize_image_mode(payload.image_mode.as_deref(), !reference_images.is_empty());

    Ok(json!({
        "cover_id": storybook_id,
        "prompt": prompt,
        "mode": "storybook_cover_image",
        "image_mode": image_mode.as_str(),
        "aspect_ratio": aspect.key,
        "size": aspect.image_size,
        "reference_images": reference_images,
        "edit_instruction": clean_optional_text(payload.edit_instruction),
        "strength": payload.strength.map(|value| value.clamp(0.0, 1.0))
    }))
}

pub async fn page_image_job_input(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    page_id: Uuid,
    payload: CreateImageTaskRequest,
) -> Result<JsonValue, DbErr> {
    let page_prompt = page_prompt(db, workspace_id, storybook_id, page_id).await?;
    let cover_tone = storybook_cover_tone(db, workspace_id, storybook_id).await?;
    let aspect_ratio = storybook_page_aspect_ratio(db, workspace_id, storybook_id).await?;
    let aspect = page_aspect_spec(&aspect_ratio);
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
    // 把本页点名角色的外观关键词注入提示词：参考图未就绪或模型没吃参考图时，形象也不漂。
    let named_roles = page_named_role_appearances(db, storybook_id, &prompt).await?;
    let prompt = if named_roles.is_empty() {
        prompt
    } else {
        let roster = named_roles
            .iter()
            .map(|(name, _, appearance)| format!("{name}={appearance}"))
            .collect::<Vec<_>>()
            .join("；");
        format!("{prompt} 角色外观：{roster}。")
    };
    let anatomy_rules = named_roles
        .iter()
        .filter_map(|(name, role_type, appearance)| {
            let clause = role_anatomy_clause(name, role_type, appearance);
            is_limb_free_character(name, role_type, appearance).then_some(clause)
        })
        .collect::<Vec<_>>();
    let prompt = if anatomy_rules.is_empty() {
        prompt
    } else {
        format!("{prompt} 角色结构约束：{}。", anatomy_rules.join("；"))
    };
    let shot_instruction = page_camera_shot_instruction(&prompt);
    let prompt = format!(
        "{shot_instruction} 原始插图描述：{prompt} {} {}",
        storybook_style_guard(&cover_tone),
        aspect.prompt_clause
    );
    // 高频或多主体描述容易让文生图模型把同一角色画成多个、拆成拼接场景；
    // 去重约束点名道姓（命中角色时），比泛指"每个角色"更有效。
    let dedup_subject = if named_roles.is_empty() {
        "每个角色".to_string()
    } else {
        named_roles
            .iter()
            .map(|(name, _, _)| name.as_str())
            .collect::<Vec<_>>()
            .join("、")
    };
    let prompt = format!(
        "{prompt} {dedup_subject}在画面中各自只出现一次，不要重复绘制同一角色。单幅连续场景，不要分格、不要上下拼接两个画面。"
    );
    let prompt = if has_scene_reference {
        format!(
            "{prompt} 参考上一页画面保持角色、场景元素和光线连续；但本页镜头距离、主体大小、视角和构图必须以本页镜头要求为准，不要复制上一页景别。"
        )
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
        "aspect_ratio": aspect.key,
        "size": aspect.image_size,
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
    // 角色参考图必须由后端根据最新角色外观和整本画风统一组装。
    // 不采信前端传入的历史 prompt，避免旧画风或"四肢完整"类默认词覆盖老师刚修改的外观设定。
    let prompt = role_reference_prompt(db, workspace_id, storybook_id, role_id).await?;
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
    } else if job.job_type == "storybook_cover_image" {
        let cover_id = job
            .input_json
            .get("cover_id")
            .and_then(|value| value.as_str())
            .ok_or_else(|| DbErr::Custom("封面图任务缺少 cover_id，无法执行".to_string()))?;
        Ok(ImageJobTarget {
            target_id: cover_id.to_string(),
            target_type: "cover",
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

async fn cover_prompt(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<String, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select title, age_group, use_scene, teaching_goal, cover_tone, coalesce(page_aspect_ratio, 'portrait_4_5') as page_aspect_ratio
            from storybooks
            where workspace_id = $1 and id = $2
            limit 1
            "#,
            [workspace_id.into(), storybook_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("storybook".to_string()))?;
    let title: String = row.try_get("", "title")?;
    let age_group: String = row.try_get("", "age_group")?;
    let use_scene: String = row.try_get("", "use_scene")?;
    let teaching_goal: String = row.try_get("", "teaching_goal")?;
    let cover_tone: String = row.try_get("", "cover_tone")?;
    let page_aspect_ratio: String = row.try_get("", "page_aspect_ratio")?;
    let roles = storybook_cover_roles_for_prompt(db, storybook_id).await?;
    let story_beats = storybook_cover_story_beats(db, storybook_id).await?;
    let style_guard = storybook_style_guard(&cover_tone);
    let aspect = page_aspect_spec(&page_aspect_ratio);
    Ok(format!(
        "为幼儿园绘本《{title}》生成封面插图。年龄段：{age_group}；使用场景：{use_scene}；教学目标：{teaching_goal}。故事线索：{}。角色关系：{}。画面要求：{style_guard} {} 角色外观由参考图决定，文字提示词只负责镜头、场景、关系和情绪，不要重复设计角色外观。封面应像真实绘本封面，选择一个最能概括故事主题的完整故事瞬间，不要做成白底角色设定图、角色排排站或单纯人物合照。镜头采用中景或中远景，关键道具作为视觉焦点；角色之间要有明确关系，例如共同注视、靠近、守护、分享、发现或解决问题。背景要交代故事发生地点和情绪，有前景、中景、背景层次，光线自然温暖。画面上方或左上方使用自然简洁的低细节背景区域，便于后期排标题；不要绘制白色矩形、文本框、纸片、牌匾或任何人为留白块。不要出现任何文字、标题、logo、水印或页码，文字由系统排版叠加。",
        if story_beats.is_empty() {
            "围绕标题、教学目标和故事主题设计一个有情境的封面瞬间".to_string()
        } else {
            story_beats.join("；")
        },
        if roles.is_empty() {
            "根据主题自然设计".to_string()
        } else {
            roles.join("；")
        },
        aspect.prompt_clause
    ))
}

async fn storybook_cover_story_beats(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<Vec<String>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            with numbered_pages as (
              select page_number, title, body, illustration_prompt
              from storybook_pages
              where storybook_id = $1
            ),
            bounds as (
              select min(page_number) as first_page, max(page_number) as last_page
              from numbered_pages
            )
            select page_number, title, body, illustration_prompt
            from numbered_pages, bounds
            where page_number = first_page or page_number = last_page
            order by page_number asc
            "#,
            [storybook_id.into()],
        ))
        .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let page_number = row.try_get::<i32>("", "page_number").ok()?;
            let title = row.try_get::<String>("", "title").ok()?;
            let body = row.try_get::<String>("", "body").ok()?;
            let illustration_prompt = row.try_get::<String>("", "illustration_prompt").ok()?;
            Some(format!(
                "第{}页《{}》：{}；画面：{}",
                page_number,
                clip_prompt_text(&title, 24),
                clip_prompt_text(&body, 70),
                clip_prompt_text(&illustration_prompt, 90)
            ))
        })
        .collect())
}

async fn storybook_cover_roles_for_prompt(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<Vec<String>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select name, role_type, story_function
            from storybook_roles
            where storybook_id = $1
            order by
              case role_type when 'protagonist' then 0 when 'teacher' then 1 else 2 end,
              name asc
            limit 6
            "#,
            [storybook_id.into()],
        ))
        .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let name = row.try_get::<String>("", "name").ok()?;
            let role_type = row.try_get::<String>("", "role_type").ok()?;
            let story_function = row.try_get::<String>("", "story_function").ok()?;
            Some(format!(
                "{name}（{role_type}）：{}",
                clip_prompt_text(&story_function, 40)
            ))
        })
        .collect::<Vec<_>>())
}

fn clip_prompt_text(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut clipped = trimmed.chars().take(max_chars).collect::<String>();
    clipped.push('…');
    clipped
}

async fn storybook_role_reference_images(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<Vec<ImageReference>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select r.id, r.name, coalesce(v.image_url, r.reference_image_url) as image_url
            from storybook_roles r
            left join storybook_image_variants v
              on v.id = r.selected_image_variant_id
             and v.status = 'ready'
             and v.image_url is not null
            where r.storybook_id = $1
              and r.needs_consistency
              and coalesce(v.image_url, r.reference_image_url) is not null
            order by
              case r.role_type when 'protagonist' then 0 when 'teacher' then 1 else 2 end,
              r.name asc
            limit 4
            "#,
            [storybook_id.into()],
        ))
        .await?;
    let mut references = Vec::new();
    for row in rows {
        let role_id: Uuid = row.try_get("", "id")?;
        let name: String = row.try_get("", "name")?;
        let image_url: String = row.try_get("", "image_url")?;
        if let Some(url) = resolve_stored_image_url(db, &image_url).await? {
            references.push(ImageReference {
                url,
                source: "storybook_role".to_string(),
                role_id: Some(role_id.to_string()),
                label: Some(name),
            });
        }
    }
    Ok(references)
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
    let aspect_ratio = job
        .input_json
        .get("aspect_ratio")
        .and_then(|value| value.as_str());
    let requested_size = job
        .input_json
        .get("size")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let size = image_size_for_aspect_with_fallback(aspect_ratio, requested_size);

    Ok(PageImageRequestInput {
        prompt,
        reference_images,
        edit_instruction,
        image_mode,
        strength,
        size,
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
            let Some(url) = resolve_stored_image_url(db, &url).await? else {
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
        let Some(url) = resolve_stored_image_url(db, &url).await? else {
            continue;
        };
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

/// 归一化库里的参考图地址，保证最终能映射到本地文件或合法远程 URL：
/// - /generated-images/... 本地文件路径：直接用（生图请求时会转 base64）
/// - 带域名的本地路径（http://host/generated-images/...）：剥离域名
/// - 旧数据里的任务图片 API 路径（/api/workspaces/{ws}/generation-jobs/{job}/image）：
///   追溯到该任务输出的真实图片地址
/// 解析不出的返回 None，调用方跳过该参考图，避免无效 URL 发给生图服务导致整单 400。
async fn resolve_stored_image_url(
    db: &DatabaseConnection,
    url: &str,
) -> Result<Option<String>, DbErr> {
    let trimmed = url.trim();
    if trimmed.starts_with("/generated-images/") {
        return Ok(Some(trimmed.to_string()));
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        // 带域名的本地生成图：剥离 scheme+host，按本地文件处理。
        if let Some(scheme_end) = trimmed.find("://") {
            if let Some(path_start) = trimmed[scheme_end + 3..].find('/') {
                let path = &trimmed[scheme_end + 3 + path_start..];
                if path.starts_with("/generated-images/") {
                    return Ok(Some(path.to_string()));
                }
            }
        }
        return Ok(Some(trimmed.to_string()));
    }
    if let Some(rest) = trimmed.strip_prefix("/api/workspaces/") {
        let segments: Vec<&str> = rest.split('/').collect();
        if segments.len() == 4 && segments[1] == "generation-jobs" && segments[3] == "image" {
            if let Ok(job_id) = Uuid::parse_str(segments[2]) {
                let row = db
                    .query_one(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        "select output_json->'image'->>'image_url' as image_url from generation_jobs where id = $1 limit 1",
                        [job_id.into()],
                    ))
                    .await?;
                if let Some(row) = row {
                    if let Some(inner) =
                        clean_optional_text(row.try_get::<Option<String>>("", "image_url")?)
                    {
                        if inner != trimmed {
                            return Box::pin(resolve_stored_image_url(db, &inner)).await;
                        }
                    }
                }
            }
            // API 路径但追溯不到真实文件：跳过，不发无效 URL 给生图服务。
            return Ok(None);
        }
    }
    Ok(Some(trimmed.to_string()))
}

async fn storybook_cover_tone(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<String, DbErr> {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        select coalesce(cover_tone, '') as cover_tone
        from storybooks
        where workspace_id = $1 and id = $2
        limit 1
        "#,
        [workspace_id.into(), storybook_id.into()],
    ))
    .await?
    .ok_or_else(|| DbErr::RecordNotFound("storybook".to_string()))?
    .try_get("", "cover_tone")
}

async fn storybook_page_aspect_ratio(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
) -> Result<String, DbErr> {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        select coalesce(page_aspect_ratio, 'portrait_4_5') as page_aspect_ratio
        from storybooks
        where workspace_id = $1 and id = $2
        limit 1
        "#,
        [workspace_id.into(), storybook_id.into()],
    ))
    .await?
    .ok_or_else(|| DbErr::RecordNotFound("storybook".to_string()))?
    .try_get("", "page_aspect_ratio")
}

fn storybook_style_clause(cover_tone: &str) -> (String, bool) {
    let trimmed = cover_tone.trim().trim_end_matches('。');
    let uses_default_style = trimmed.is_empty() || trimmed == "温暖、清楚";
    if uses_default_style {
        (
            "柔和水彩绘本风格，圆润饱满造型，大而富有表现力的眼睛".to_string(),
            true,
        )
    } else if trimmed.contains("皮克斯") || trimmed.contains("3D") {
        (
            format!(
                "{trimmed}，高质量3D动画电影质感，立体圆润角色，柔和棚拍光，细腻材质，真实体积感"
            ),
            false,
        )
    } else {
        (trimmed.to_string(), false)
    }
}

fn storybook_style_guard(cover_tone: &str) -> String {
    let (style_clause, uses_default_style) = storybook_style_clause(cover_tone);
    if uses_default_style {
        format!("画面风格：{style_clause}。")
    } else {
        format!(
            "画面风格必须严格采用整本绘本选择的风格：{style_clause}；禁止改成水彩、平面卡通、手绘素描或其他未选择的画风。"
        )
    }
}

fn page_camera_shot_instruction(prompt: &str) -> &'static str {
    if prompt.contains("局部特写") {
        return "镜头执行：必须是局部特写，画面聚焦一个局部细节或关键物件，局部可占画面 70% 以上，允许合理裁切身体或环境；不要改成完整人物中景。";
    }
    if prompt.contains("特写") {
        return "镜头执行：必须是特写，脸部、表情、手部动作或关键物件占主画面，背景只作少量氛围；不要改成半身中景。";
    }
    if prompt.contains("俯视") {
        return "镜头执行：必须是俯视或鸟瞰视角，从上方向下看清地面、桌面或空间布局，人物和物件按平面关系分布；不要改成平视中景。";
    }
    if prompt.contains("跟随视角") {
        return "镜头执行：必须是跟随视角，镜头像跟在角色身后或侧后方移动，前景、主体和前进方向有层次；不要改成静态正面中景。";
    }
    if prompt.contains("远景") {
        return "最高优先级镜头执行：本图必须按远景生成。镜头明显拉远，环境占画面 65%-80%，角色全身可见且相对较小，用环境和位置关系讲故事；如果原始描述里出现眼睛、触角、咬痕、手部、表情等近距离细节，这些只作为故事信息，不要求清晰可见，不要为了看清它们而推近镜头；微小物件可以只呈现为小轮廓或色块；不要裁切身体，不要改成人物半身、近景或中景。";
    }
    if prompt.contains("全景") {
        return "镜头执行：必须是全景，完整交代地点、角色全身和彼此位置关系，环境面积大于角色面积；不要改成近距离人物中景。";
    }
    if prompt.contains("中近景") {
        return "镜头执行：必须是中近景，角色上半身和手部互动清楚，仍保留少量环境线索；不要拉成远景，也不要推成脸部特写。";
    }
    if prompt.contains("近景") {
        return "镜头执行：必须是近景，重点表现角色动作和表情，主体较大但保留必要动作空间；不要退成远景或普通中景。";
    }
    if prompt.contains("中景") {
        return "镜头执行：必须是中景，角色半身到全身上部与周围环境平衡呈现，动作和关系清楚；不要自动变成脸部特写。";
    }
    "镜头执行：严格按照插图描述里的镜头、视角和构图重点执行，主体大小要匹配该镜头；不要把所有页面统一生成为中景。"
}

fn is_limb_free_character(name: &str, role_type: &str, appearance: &str) -> bool {
    let text = format!("{name} {role_type} {appearance}");
    [
        "无手",
        "没有手",
        "无脚",
        "没有脚",
        "无手和脚",
        "没有手和脚",
        "无四肢",
        "没有四肢",
        "蛇",
        "小蛇",
        "蚯蚓",
        "毛毛虫",
        "蜗牛",
        "球形",
    ]
    .iter()
    .any(|keyword| text.contains(keyword))
}

fn role_anatomy_clause(name: &str, role_type: &str, appearance: &str) -> String {
    if is_limb_free_character(name, role_type, appearance) {
        format!(
            "{name}必须严格符合外观设定：没有手、没有脚、没有手臂和腿，不要生成手指、鞋子、胳膊或人形四肢；用头部、眼睛、身体弯曲、尾部和整体姿态表达动作"
        )
    } else {
        format!(
            "{name}的身体结构必须严格符合外观设定；有手、脚、爪或翅膀时可以清晰表现，但不要凭空添加外观没有写到的肢体"
        )
    }
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
            select coalesce(
              selected_variant.image_url,
              latest_job.output_json->'image'->>'image_url'
            ) as image_url
            from storybook_pages current_page
            join storybook_pages prev_page
              on prev_page.storybook_id = current_page.storybook_id
             and prev_page.page_number = current_page.page_number - 1
            left join storybook_image_variants selected_variant
              on selected_variant.id = prev_page.selected_image_variant_id
             and selected_variant.status = 'ready'
             and selected_variant.image_url is not null
            left join lateral (
              select gj.output_json
              from generation_jobs gj
              where gj.storybook_id = current_page.storybook_id
                and gj.job_type = 'storybook_page_image'
                and gj.status = 'succeeded'
                and gj.input_json->>'page_id' = prev_page.id::text
              order by gj.created_at desc
              limit 1
            ) latest_job on true
            where current_page.storybook_id = $1 and current_page.id = $2
              and coalesce(selected_variant.image_url, latest_job.output_json->'image'->>'image_url') is not null
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
    let Some(url) = resolve_stored_image_url(db, &url).await? else {
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

/// 查询本页提示词里点名角色的（名字, 类型, 外观设定）清单；没有命中则返回空 vec。
/// 最多取 4 个，避免清单过长稀释画面描述。
async fn page_named_role_appearances(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    prompt: &str,
) -> Result<Vec<(String, String, String)>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select name, role_type, appearance
            from storybook_roles
            where storybook_id = $1
            order by name asc, id asc
            "#,
            [storybook_id.into()],
        ))
        .await?;
    let pairs = rows
        .into_iter()
        .filter_map(|row| {
            let name = row.try_get::<String>("", "name").ok()?;
            let role_type = row.try_get::<String>("", "role_type").ok()?;
            let appearance = row.try_get::<String>("", "appearance").ok()?;
            let name = name.trim();
            let role_type = role_type.trim();
            let appearance = appearance.trim().trim_end_matches(['。', '；', ';']);
            (!name.is_empty() && !appearance.is_empty() && prompt.contains(name)).then(|| {
                (
                    name.to_string(),
                    role_type.to_string(),
                    appearance.to_string(),
                )
            })
        })
        .take(4)
        .collect::<Vec<_>>();
    Ok(pairs)
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
            select r.name, r.role_type, r.appearance, s.cover_tone
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
    let cover_tone: String = row.try_get("", "cover_tone")?;
    let style_guard = storybook_style_guard(&cover_tone);
    let anatomy_clause = role_anatomy_clause(&name, &role_type, &appearance);

    Ok(format!(
        "为绘本生成单一角色标准参考图。角色名：{name}；视觉类型：{role_type}；稳定外观：{appearance}。要求：白底或简洁背景，{style_guard}；{anatomy_clause}；表情自然生动、富有神采，姿态自然放松，可微微侧身或采用三分之四视角，清晰展示完整轮廓或半身；画面中只有这个角色，无人类，无其他角色，便于后续分页插图保持一致。不要加入故事情节动作或分页场景，不要僵硬对称的证件照式站姿。"
    ))
}
