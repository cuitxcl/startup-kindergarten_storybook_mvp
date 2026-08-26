use std::collections::HashSet;

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

use crate::models::{CreateImageTaskRequest, GenerationJob};
use crate::page_aspect::{image_size_for_aspect_with_fallback, page_aspect_spec};
use crate::services::generation_provider::{ImageGenerationMode, ImageReference};

fn reference_evidence(references: &[ImageReference], style_version: Option<i32>) -> Vec<JsonValue> {
    references
        .iter()
        .map(|reference| {
            json!({
                "kind": reference.source,
                "reference_id": reference.role_id,
                "label": reference.label,
                "image_url": reference.url,
                "generation_job_id": reference.generation_job_id,
                "style_version": style_version,
            })
        })
        .collect()
}

#[derive(Clone, Debug)]
struct PageNamedRole {
    name: String,
    role_type: String,
    appearance: String,
    story_function: String,
}

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
        "storybook_page_image"
            | "storybook_role_reference_image"
            | "storybook_cover_image"
            | "storybook_visual_reference"
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
    let style_version = storybook_visual_style_version(db, storybook_id).await?;
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
        "reference_evidence": reference_evidence(&reference_images, style_version),
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
    let style_version = storybook_visual_style_version(db, storybook_id).await?;
    let aspect = page_aspect_spec(&aspect_ratio);
    let prompt = payload
        .prompt
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(page_prompt);
    let mut reference_images = page_image_reference_images(db, storybook_id, &payload).await?;
    let (photo_reference_images, photo_reference_clause) =
        page_visual_reference_images(db, workspace_id, storybook_id, page_id).await?;
    for reference in photo_reference_images {
        if !reference_images
            .iter()
            .any(|item| item.url == reference.url)
        {
            reference_images.push(reference);
        }
    }
    // 场景延续只能在没有角色参考图时启用。上一页若已有人物漂移，作为图片输入会
    // 覆盖当前角色图的帽子、发型和服装，反而把错误传给后续分页。
    let has_role_reference = reference_images
        .iter()
        .any(|reference| reference.source == "storybook_role");
    let scene_reference = if has_role_reference {
        None
    } else {
        previous_page_scene_reference(db, storybook_id, page_id).await?
    };
    let has_scene_reference = scene_reference.is_some();
    if let Some(scene) = scene_reference {
        if !reference_images.iter().any(|item| item.url == scene.url) {
            reference_images.push(scene);
        }
    }
    // 把本页点名角色的外观关键词注入提示词：参考图未就绪或模型没吃参考图时，形象也不漂。
    let named_roles = page_named_roles(db, storybook_id, &prompt).await?;
    let page_story_context = page_story_context(db, workspace_id, storybook_id, page_id).await?;
    let prompt = if named_roles.is_empty() {
        prompt
    } else {
        let roster = named_roles
            .iter()
            .map(|role| format!("{}={}", role.name, role.appearance))
            .collect::<Vec<_>>()
            .join("；");
        format!("{prompt} 角色外观：{roster}。")
    };
    let prompt = if page_story_context.is_empty() {
        prompt
    } else {
        format!("{prompt} 本页故事关系：{page_story_context}。")
    };
    let prompt = if photo_reference_clause.is_empty() {
        prompt
    } else {
        format!("{prompt} {photo_reference_clause}")
    };
    let relation_clause = page_role_relation_clause(&named_roles);
    let prompt = if relation_clause.is_empty() {
        prompt
    } else {
        format!("{prompt} 角色关系与站位：{relation_clause}。")
    };
    let anatomy_rules = named_roles
        .iter()
        .filter_map(|role| {
            let clause = role_anatomy_clause(&role.name, &role.role_type, &role.appearance);
            is_limb_free_character(&role.name, &role.role_type, &role.appearance).then_some(clause)
        })
        .collect::<Vec<_>>();
    let prompt = if anatomy_rules.is_empty() {
        prompt
    } else {
        format!("{prompt} 角色结构约束：{}。", anatomy_rules.join("；"))
    };
    let shot_instruction = page_camera_shot_instruction(&prompt);
    let shot_priority = page_camera_priority_guard(&prompt);
    let prompt = format!(
        "{shot_instruction} {shot_priority} 原始插图描述：{prompt} {} {}",
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
            .map(|role| role.name.as_str())
            .collect::<Vec<_>>()
            .join("、")
    };
    let prompt = format!(
        "{prompt} {dedup_subject}在画面中各自只出现一次，不要重复绘制同一角色。单幅连续场景，不要分格、不要上下拼接两个画面。"
    );
    let prompt = if has_scene_reference {
        format!(
            "{prompt} 参考上一页画面保持角色、场景元素和光线连续；但本页镜头距离、主体大小、视角和构图必须以本页镜头要求为准，不要复制上一页景别。最终检查：如果镜头要求和人物关系、动作、道具细节冲突，一律保镜头，不要改景别。"
        )
    } else {
        format!(
            "{prompt} 最终检查：如果镜头要求和人物关系、动作、道具细节冲突，一律保镜头，不要改景别。"
        )
    };
    let image_mode =
        normalize_image_mode(payload.image_mode.as_deref(), !reference_images.is_empty());
    let prompt = if reference_images.is_empty() {
        prompt
    } else {
        let referenced_names = reference_images
            .iter()
            .filter_map(|reference| reference.label.as_deref())
            .collect::<Vec<_>>();
        format!(
            "{prompt} 角色身份必须严格以随附角色参考图为准：{}。必须保留参考图中的脸型、发型、帽子或头饰、服装配色与年龄感；不得改成另一位孩子，不得省略显著头饰或替换服装。",
            if referenced_names.is_empty() {
                "全部已提供角色".to_string()
            } else {
                referenced_names.join("、")
            }
        )
    };
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
        "target_snapshot": page_content_snapshot(db, storybook_id, page_id).await?,
        "image_mode": image_mode.as_str(),
        "aspect_ratio": aspect.key,
        "size": aspect.image_size,
        "reference_role_ids": payload.reference_role_ids,
        "reference_images": reference_images,
        "reference_evidence": reference_evidence(&reference_images, style_version),
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
            generation_job_id: None,
        })
        .collect::<Vec<_>>();
    let image_mode =
        normalize_image_mode(payload.image_mode.as_deref(), !reference_images.is_empty());

    Ok(json!({
        "role_id": role_id,
        "prompt": prompt,
        "mode": "storybook_role_reference_image",
        "target_snapshot": role_content_snapshot(db, storybook_id, role_id).await?,
        "image_mode": image_mode.as_str(),
        "reference_images": reference_images,
        "edit_instruction": clean_optional_text(payload.edit_instruction),
        "strength": payload.strength.map(|value| value.clamp(0.0, 1.0))
    }))
}

async fn page_content_snapshot(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    page_id: Uuid,
) -> Result<JsonValue, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select title, body, illustration_prompt
            from storybook_pages
            where storybook_id = $1 and id = $2
            limit 1
            "#,
            [storybook_id.into(), page_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("page".to_string()))?;
    Ok(json!({
        "title": row.try_get::<String>("", "title")?,
        "body": row.try_get::<String>("", "body")?,
        "illustration_prompt": row.try_get::<String>("", "illustration_prompt")?,
    }))
}

async fn role_content_snapshot(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    role_id: Uuid,
) -> Result<JsonValue, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select name, role_type, appearance, coalesce(story_function, '') as story_function, needs_consistency
            from storybook_roles
            where storybook_id = $1 and id = $2
            limit 1
            "#,
            [storybook_id.into(), role_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("role".to_string()))?;
    Ok(json!({
        "name": row.try_get::<String>("", "name")?,
        "role_type": row.try_get::<String>("", "role_type")?,
        "appearance": row.try_get::<String>("", "appearance")?,
        "story_function": row.try_get::<String>("", "story_function")?,
        "needs_consistency": row.try_get::<bool>("", "needs_consistency")?,
    }))
}

pub fn image_target_from_job(job: &GenerationJob) -> Result<ImageJobTarget, DbErr> {
    if job.job_type == "storybook_visual_reference" {
        let target_id = job
            .input_json
            .get("target_id")
            .or_else(|| job.input_json.get("asset_reference_id"))
            .and_then(|value| value.as_str())
            .ok_or_else(|| {
                DbErr::Custom("同画风参考任务缺少 asset_reference_id，无法执行".to_string())
            })?;
        Ok(ImageJobTarget {
            target_id: target_id.to_string(),
            target_type: "asset_reference",
        })
    } else if job.job_type == "storybook_role_reference_image" {
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
        "为幼儿园绘本《{title}》生成封面插图。年龄段：{age_group}；使用场景：{use_scene}；教学目标：{teaching_goal}。故事线索：{}。角色关系：{}。画面要求：{style_guard} {} 角色外观必须严格遵循角色关系中的外观描述，并与输入的角色参考图保持同一角色身份、毛色、体型、耳朵/鼻子等关键特征；只允许出现上述角色，不得替换、增添或重新设计角色。封面应像真实绘本封面，选择一个最能概括故事主题的完整故事瞬间，不要做成白底角色设定图、角色排排站或单纯人物合照。镜头采用中景或中远景，关键道具作为视觉焦点；角色之间要有明确关系，例如共同注视、靠近、守护、分享、发现或解决问题。背景要交代故事发生地点和情绪，有前景、中景、背景层次，光线自然温暖。画面上方或左上方使用自然简洁的低细节背景区域，便于后期排标题；不要绘制白色矩形、文本框、纸片、牌匾或任何人为留白块。不要出现任何文字、标题、logo、水印或页码，文字由系统排版叠加。",
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
            select r.name, r.role_type, r.appearance, r.story_function
            from storybook_roles r
            left join storybook_image_variants v
              on v.id = r.selected_image_variant_id
             and v.status = 'ready'
             and v.image_url is not null
            where r.storybook_id = $1
              and r.needs_consistency
              and r.reference_status = 'ready'
              and coalesce(v.image_url, r.reference_image_url) is not null
            order by
              case r.role_type when 'protagonist' then 0 when 'teacher' then 1 else 2 end,
              r.name asc
            limit 4
            "#,
            [storybook_id.into()],
        ))
        .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let name = row.try_get::<String>("", "name").ok()?;
            let role_type = row.try_get::<String>("", "role_type").ok()?;
            let appearance = row.try_get::<String>("", "appearance").ok()?;
            let story_function = row.try_get::<String>("", "story_function").ok()?;
            Some(format!(
                "{name}（{role_type}）：外观={}; 故事作用={}",
                clip_prompt_text(&appearance, 80),
                clip_prompt_text(&story_function, 40),
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
            select r.id, r.name, coalesce(v.image_url, r.reference_image_url) as image_url,
                   v.generation_job_id
            from storybook_roles r
            left join storybook_image_variants v
              on v.id = r.selected_image_variant_id
             and v.status = 'ready'
             and v.image_url is not null
            where r.storybook_id = $1
              and r.needs_consistency
              and r.reference_status = 'ready'
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
                generation_job_id: row
                    .try_get::<Option<Uuid>>("", "generation_job_id")?
                    .map(|id| id.to_string()),
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
                select r.id, r.name, r.reference_image_url, v.generation_job_id
                from storybook_roles r
                left join storybook_image_variants v
                  on v.id = r.selected_image_variant_id
                where r.storybook_id = $1
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
                generation_job_id: row
                    .try_get::<Option<Uuid>>("", "generation_job_id")?
                    .map(|id| id.to_string()),
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
            generation_job_id: None,
        });
    }

    Ok(references)
}

async fn page_visual_reference_images(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    page_id: Uuid,
) -> Result<(Vec<ImageReference>, String), DbErr> {
    let Some(plan) = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select s.customization_plan,
                   creation_job.input_json as creation_input_json,
                   p.page_number
            from storybook_pages p
            join storybooks s on s.id = p.storybook_id
            left join storybook_creation_sessions creation_session
              on creation_session.workspace_id = s.workspace_id
             and creation_session.storybook_id = s.id
            left join generation_jobs creation_job
              on creation_job.id = creation_session.last_job_id
             and creation_job.workspace_id = s.workspace_id
             and creation_job.job_type = 'creation_storybook_generate'
            where s.workspace_id = $1 and s.id = $2 and p.id = $3
            limit 1
            "#,
            [workspace_id.into(), storybook_id.into(), page_id.into()],
        ))
        .await?
    else {
        return Ok((Vec::new(), String::new()));
    };
    let customization_plan: Option<JsonValue> = plan.try_get("", "customization_plan")?;
    let creation_input: Option<JsonValue> = plan.try_get("", "creation_input_json")?;
    let visual_reference_plan = page_visual_reference_plan(
        customization_plan.unwrap_or(JsonValue::Null),
        creation_input,
    );
    let page_number: i32 = plan.try_get("", "page_number")?;
    let photo_reference_clause = page_photo_reference_clause(&visual_reference_plan, page_number);
    let mut preview_urls = Vec::new();
    let mut visual_reference_ids = Vec::new();

    if let Some(page_evidence) = visual_reference_plan
        .get("page_evidence")
        .and_then(JsonValue::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
                item.get("page_number")
                    .and_then(JsonValue::as_i64)
                    .is_some_and(|value| value == i64::from(page_number))
            })
        })
    {
        collect_visual_reference_inputs(
            page_evidence,
            &mut preview_urls,
            &mut visual_reference_ids,
        );
    }

    let page_plan = visual_reference_plan
        .get("page_plan")
        .and_then(JsonValue::as_array)
        .and_then(|pages| {
            pages.iter().find(|page| {
                page.get("page_number")
                    .and_then(JsonValue::as_i64)
                    .is_some_and(|value| value == i64::from(page_number))
            })
        });
    let page_reference_ids = page_plan.map(page_reference_ids).unwrap_or_default();
    let has_typed_page_reference_ids = page_plan.is_some_and(has_typed_page_reference_ids);

    if let Some(references) = visual_reference_plan
        .get("confirmed_photo_references")
        .and_then(JsonValue::as_array)
    {
        for reference in references {
            let is_assigned_to_page = reference
                .get("asset_reference_id")
                .and_then(JsonValue::as_str)
                .is_some_and(|id| {
                    page_reference_ids
                        .iter()
                        .any(|assigned_id| assigned_id == id)
                });
            let is_assigned_by_legacy_plan = !has_typed_page_reference_ids
                && reference
                    .get("planned_pages")
                    .and_then(JsonValue::as_array)
                    .is_some_and(|pages| {
                        pages.iter().any(|page| {
                            page.get("page_number")
                                .and_then(JsonValue::as_i64)
                                .is_some_and(|value| value == i64::from(page_number))
                        })
                    });
            if is_assigned_to_page || is_assigned_by_legacy_plan {
                collect_visual_reference_inputs(
                    reference,
                    &mut preview_urls,
                    &mut visual_reference_ids,
                );
            }
        }
    }

    let mut references = Vec::new();
    for url in preview_urls {
        let Some(url) = resolve_stored_image_url(db, &url).await? else {
            continue;
        };
        if references
            .iter()
            .any(|item: &ImageReference| item.url == url)
        {
            continue;
        }
        references.push(ImageReference {
            url,
            source: "photo_visual_reference".to_string(),
            role_id: None,
            label: Some("已确认同画风参考".to_string()),
            generation_job_id: None,
        });
    }

    for visual_reference_id in visual_reference_ids {
        let Some(row) = db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                select v.generation_job_id, r.display_name
                from storybook_visual_references v
                join storybook_asset_references r
                  on r.id = v.asset_reference_id and r.workspace_id = v.workspace_id
                where v.workspace_id = $1
                  and v.id = $2
                  and v.status = 'confirmed'
                  and v.generation_job_id is not null
                limit 1
                "#,
                [workspace_id.into(), visual_reference_id.into()],
            ))
            .await?
        else {
            continue;
        };
        let generation_job_id: Uuid = row.try_get("", "generation_job_id")?;
        let url =
            format!("/api/workspaces/{workspace_id}/generation-jobs/{generation_job_id}/image");
        let Some(url) = resolve_stored_image_url(db, &url).await? else {
            continue;
        };
        if references
            .iter()
            .any(|item: &ImageReference| item.url == url)
        {
            continue;
        }
        references.push(ImageReference {
            url,
            source: "photo_visual_reference".to_string(),
            role_id: None,
            label: row.try_get("", "display_name").ok(),
            generation_job_id: Some(generation_job_id.to_string()),
        });
    }

    Ok((references, photo_reference_clause))
}

fn page_photo_reference_clause(visual_reference_plan: &JsonValue, page_number: i32) -> String {
    let mut references = Vec::<(String, String)>::new();
    let mut seen_ids = HashSet::new();
    let page_evidence = visual_reference_plan
        .get("page_evidence")
        .and_then(JsonValue::as_array)
        .and_then(|pages| {
            pages.iter().find(|page| {
                page.get("page_number")
                    .and_then(JsonValue::as_i64)
                    .is_some_and(|value| value == i64::from(page_number))
            })
        });
    if let Some(evidence) = page_evidence {
        if let Some(asset_references) = evidence
            .get("asset_references")
            .and_then(JsonValue::as_array)
        {
            for reference in asset_references {
                collect_photo_reference_clause_item(&mut references, &mut seen_ids, reference);
            }
        }
    }

    let page_plan = visual_reference_plan
        .get("page_plan")
        .and_then(JsonValue::as_array)
        .and_then(|pages| {
            pages.iter().find(|page| {
                page.get("page_number")
                    .and_then(JsonValue::as_i64)
                    .is_some_and(|value| value == i64::from(page_number))
            })
        });
    let assigned_ids = page_plan.map(page_reference_ids).unwrap_or_default();
    if !assigned_ids.is_empty() {
        if let Some(confirmed_references) = visual_reference_plan
            .get("confirmed_photo_references")
            .and_then(JsonValue::as_array)
        {
            for reference in confirmed_references {
                if reference
                    .get("asset_reference_id")
                    .and_then(JsonValue::as_str)
                    .is_some_and(|id| assigned_ids.iter().any(|assigned_id| assigned_id == id))
                {
                    collect_photo_reference_clause_item(&mut references, &mut seen_ids, reference);
                }
            }
        }
    }

    let names_for = |reference_type: &str| {
        references
            .iter()
            .filter(|(kind, _)| kind == reference_type)
            .map(|(_, name)| name.as_str())
            .collect::<Vec<_>>()
            .join("、")
    };
    let mut rules = Vec::new();
    let character_names = names_for("character_reference");
    if !character_names.is_empty() {
        rules.push(format!(
            "角色参考（{character_names}）仅约束对应人物的外观；参考图或原照片中的背景不得作为本页故事场景"
        ));
    }
    let prop_names = names_for("prop_reference");
    if !prop_names.is_empty() {
        rules.push(format!(
            "道具参考（{prop_names}）仅约束对应物品或宠物的颜色、轮廓和材质，不得画成角色"
        ));
    }
    let scene_names = names_for("scene_reference");
    if !scene_names.is_empty() {
        rules.push(format!(
            "场景参考（{scene_names}）仅约束本页地点、环境和光线；场景图中的人物不得成为故事角色"
        ));
    }
    if rules.is_empty() {
        String::new()
    } else {
        format!("照片参考使用规则：{}。", rules.join("；"))
    }
}

fn collect_photo_reference_clause_item(
    references: &mut Vec<(String, String)>,
    seen_ids: &mut HashSet<String>,
    reference: &JsonValue,
) {
    let Some(reference_type) = reference
        .get("reference_type")
        .and_then(JsonValue::as_str)
        .or_else(|| match reference.get("kind").and_then(JsonValue::as_str) {
            Some("person") => Some("character_reference"),
            Some("scene") => Some("scene_reference"),
            Some("object") => Some("prop_reference"),
            _ => None,
        })
    else {
        return;
    };
    let Some(id) = reference
        .get("asset_reference_id")
        .and_then(JsonValue::as_str)
    else {
        return;
    };
    if !seen_ids.insert(id.to_string()) {
        return;
    }
    let name = reference
        .get("display_name")
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("已确认照片参考");
    references.push((reference_type.to_string(), name.to_string()));
}

fn page_visual_reference_plan(
    customization_plan: JsonValue,
    creation_input: Option<JsonValue>,
) -> JsonValue {
    if ["page_evidence", "page_plan", "confirmed_photo_references"]
        .into_iter()
        .any(|field| customization_plan.get(field).is_some())
    {
        return customization_plan;
    }
    creation_input.unwrap_or(JsonValue::Null)
}

fn page_reference_ids(page_plan: &JsonValue) -> Vec<String> {
    [
        "character_reference_ids",
        "prop_reference_ids",
        "scene_reference_ids",
    ]
    .into_iter()
    .flat_map(|field| {
        page_plan
            .get(field)
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
            .filter_map(JsonValue::as_str)
            .map(str::to_string)
    })
    .collect()
}

fn has_typed_page_reference_ids(page_plan: &JsonValue) -> bool {
    [
        "character_reference_ids",
        "prop_reference_ids",
        "scene_reference_ids",
    ]
    .into_iter()
    .any(|field| page_plan.get(field).and_then(JsonValue::as_array).is_some())
}

fn collect_visual_reference_inputs(
    value: &JsonValue,
    preview_urls: &mut Vec<String>,
    visual_reference_ids: &mut Vec<Uuid>,
) {
    if let Some(url) = value
        .get("visual_reference_preview_url")
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if !preview_urls.iter().any(|item| item == url) {
            preview_urls.push(url.to_string());
        }
    }
    if let Some(id) = value
        .get("visual_reference_id")
        .and_then(JsonValue::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
    {
        if !visual_reference_ids.contains(&id) {
            visual_reference_ids.push(id);
        }
    }
    if let Some(references) = value.get("asset_references").and_then(JsonValue::as_array) {
        for reference in references {
            collect_visual_reference_inputs(reference, preview_urls, visual_reference_ids);
        }
    }
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

async fn storybook_visual_style_version(
    db: &DatabaseConnection,
    storybook_id: Uuid,
) -> Result<Option<i32>, DbErr> {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "select visual_style_version from storybooks where id = $1 limit 1",
        [storybook_id.into()],
    ))
    .await?
    .ok_or_else(|| DbErr::RecordNotFound("storybook".to_string()))?
    .try_get("", "visual_style_version")
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

fn page_camera_priority_guard(prompt: &str) -> &'static str {
    if prompt.contains("远景") || prompt.contains("全景") {
        return "镜头优先级最高：必须先满足本页景别要求，再处理人物关系、动作和道具细节。若正文或细节描述与景别冲突，允许弱化眼睛、手部、表情、咬痕、触角等局部信息，也不能把镜头推近。";
    }
    if prompt.contains("俯视") || prompt.contains("跟随视角") {
        return "镜头优先级最高：必须先满足本页视角要求，再处理人物关系和细节。若正文里出现不适合该视角的局部描写，可保留为故事信息，但不能改成平视或普通中景。";
    }
    if prompt.contains("特写") || prompt.contains("近景") || prompt.contains("中近景") {
        return "镜头优先级最高：必须先满足本页近距离景别要求，主体应足够大。若正文里有环境、队伍或背景信息，只保留少量必要线索，不能为了交代全环境把镜头拉远。";
    }
    if prompt.contains("中景") {
        return "镜头优先级最高：必须保持中景，兼顾人物关系和必要环境。不要为了脸部细节推成特写，也不要为了交代全场退成全景。";
    }
    "镜头优先级最高：先满足景别、视角和构图，再安排关系、动作和细节；不要让任何局部细节覆盖镜头要求。"
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
        generation_job_id: None,
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
async fn page_named_roles(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    prompt: &str,
) -> Result<Vec<PageNamedRole>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select name, role_type, appearance, story_function
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
            let story_function = row.try_get::<String>("", "story_function").ok()?;
            let name = name.trim();
            let role_type = role_type.trim();
            let appearance = appearance.trim().trim_end_matches(['。', '；', ';']);
            let story_function = story_function.trim().trim_end_matches(['。', '；', ';']);
            (!name.is_empty() && !appearance.is_empty() && prompt.contains(name)).then(|| {
                PageNamedRole {
                    name: name.to_string(),
                    role_type: role_type.to_string(),
                    appearance: appearance.to_string(),
                    story_function: story_function.to_string(),
                }
            })
        })
        .take(4)
        .collect::<Vec<_>>();
    Ok(pairs)
}

async fn page_story_context(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    storybook_id: Uuid,
    page_id: Uuid,
) -> Result<String, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            with current_page as (
              select p.page_number, p.title, p.body
              from storybook_pages p
              join storybooks s on s.id = p.storybook_id
              where s.workspace_id = $1 and s.id = $2 and p.id = $3
              limit 1
            ),
            previous_page as (
              select title, body
              from storybook_pages
              where storybook_id = $2
                and page_number = (select page_number - 1 from current_page)
              limit 1
            ),
            next_page as (
              select title, body
              from storybook_pages
              where storybook_id = $2
                and page_number = (select page_number + 1 from current_page)
              limit 1
            )
            select
              (select title from current_page) as current_title,
              (select body from current_page) as current_body,
              (select title from previous_page) as prev_title,
              (select body from previous_page) as prev_body,
              (select title from next_page) as next_title,
              (select body from next_page) as next_body
            "#,
            [workspace_id.into(), storybook_id.into(), page_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("page".to_string()))?;

    let current_title = row
        .try_get::<String>("", "current_title")
        .unwrap_or_default();
    let current_body = row
        .try_get::<String>("", "current_body")
        .unwrap_or_default();
    let prev_title = row
        .try_get::<Option<String>>("", "prev_title")?
        .unwrap_or_default();
    let prev_body = row
        .try_get::<Option<String>>("", "prev_body")?
        .unwrap_or_default();
    let next_title = row
        .try_get::<Option<String>>("", "next_title")?
        .unwrap_or_default();
    let next_body = row
        .try_get::<Option<String>>("", "next_body")?
        .unwrap_or_default();

    let mut parts = vec![format!(
        "本页标题《{}》，核心情节：{}",
        clip_prompt_text(&current_title, 18),
        clip_prompt_text(&current_body, 52)
    )];
    if !prev_body.trim().is_empty() {
        parts.push(format!(
            "承接上一页《{}》：{}",
            clip_prompt_text(&prev_title, 18),
            clip_prompt_text(&prev_body, 40)
        ));
    }
    if !next_body.trim().is_empty() {
        parts.push(format!(
            "为下一页《{}》做铺垫：{}",
            clip_prompt_text(&next_title, 18),
            clip_prompt_text(&next_body, 40)
        ));
    }
    Ok(parts.join("；"))
}

fn page_role_relation_clause(named_roles: &[PageNamedRole]) -> String {
    if named_roles.is_empty() {
        return String::new();
    }
    let protagonist = named_roles
        .iter()
        .find(|role| role.role_type == "protagonist")
        .map(|role| role.name.as_str());
    let teacher = named_roles
        .iter()
        .find(|role| role.role_type == "teacher")
        .map(|role| role.name.as_str());
    let supporting = named_roles
        .iter()
        .filter(|role| role.role_type == "supporting" || role.role_type == "peer")
        .map(|role| role.name.as_str())
        .collect::<Vec<_>>();
    let props = named_roles
        .iter()
        .filter(|role| role.role_type == "prop")
        .map(|role| role.name.as_str())
        .collect::<Vec<_>>();

    let mut parts: Vec<String> = named_roles
        .iter()
        .map(|role| {
            format!(
                "{}：{}",
                role.name,
                clip_prompt_text(&role.story_function, 36)
            )
        })
        .collect();

    if let Some(name) = protagonist {
        parts.push(format!(
            "{name}是本页视觉主角，构图中心优先围绕{name}展开，其余角色围绕{name}形成明确互动，不要平均分散站位"
        ));
    }
    if let (Some(protagonist), Some(teacher)) = (protagonist, teacher) {
        parts.push(format!(
            "{teacher}与{protagonist}之间要有清晰的引导或回应关系，可用注视、靠近、弯腰、陪伴、示范等动作表现"
        ));
    }
    if let Some(protagonist) = protagonist.filter(|_| !supporting.is_empty()) {
        parts.push(format!(
            "{}与{}之间必须表现具体关系，例如一起观察、请求回应、鼓励帮助、对话或协作，不要只是并排站立",
            protagonist,
            supporting.join("、")
        ));
    }
    if let Some(protagonist) = protagonist.filter(|_| !props.is_empty()) {
        parts.push(format!(
            "{}与关键道具{}要有明确物理关系，如拿着、指向、背着、递给、围着观察或共同注视，避免道具漂浮在背景里",
            protagonist,
            props.join("、")
        ));
    }
    parts.push(
        "角色之间要有远近、朝向和遮挡层次，明确谁靠前、谁靠后、谁在看谁、谁在回应谁".to_string(),
    );
    parts.join("；")
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        PageNamedRole, has_typed_page_reference_ids, image_request_from_job, image_target_from_job,
        is_image_job, page_camera_priority_guard, page_photo_reference_clause, page_reference_ids,
        page_role_relation_clause, page_visual_reference_plan, reference_evidence,
    };
    use crate::models::GenerationJob;
    use crate::services::generation_provider::ImageReference;

    #[test]
    fn page_role_relation_clause_emphasizes_relationships_and_staging() {
        let clause = page_role_relation_clause(&[
            PageNamedRole {
                name: "图图".to_string(),
                role_type: "protagonist".to_string(),
                appearance: "小狐狸".to_string(),
                story_function: "面对灰灰的不合理要求，从不敢拒绝到学会勇敢说不".to_string(),
            },
            PageNamedRole {
                name: "灰灰".to_string(),
                role_type: "supporting".to_string(),
                appearance: "小灰狼".to_string(),
                story_function: "向图图提出要求，推动冲突".to_string(),
            },
            PageNamedRole {
                name: "兔老师".to_string(),
                role_type: "teacher".to_string(),
                appearance: "兔子老师".to_string(),
                story_function: "引导图图表达内心，帮助他建立规则意识".to_string(),
            },
            PageNamedRole {
                name: "布书包".to_string(),
                role_type: "prop".to_string(),
                appearance: "浅蓝色布书包".to_string(),
                story_function: "象征不合理请求".to_string(),
            },
        ]);

        assert!(clause.contains("图图是本页视觉主角"));
        assert!(clause.contains("兔老师与图图之间要有清晰的引导或回应关系"));
        assert!(clause.contains("图图与灰灰之间必须表现具体关系"));
        assert!(clause.contains("图图与关键道具布书包要有明确物理关系"));
        assert!(clause.contains("谁靠前、谁靠后、谁在看谁、谁在回应谁"));
    }

    #[test]
    fn reference_evidence_freezes_the_actual_attached_reference() {
        let evidence = reference_evidence(
            &[ImageReference {
                url: "/api/workspaces/demo/generation-jobs/ref/image".to_string(),
                source: "storybook_role".to_string(),
                role_id: Some("role-1".to_string()),
                label: Some("淅淅".to_string()),
                generation_job_id: Some("job-1".to_string()),
            }],
            Some(3),
        );

        assert_eq!(evidence[0]["kind"], "storybook_role");
        assert_eq!(evidence[0]["reference_id"], "role-1");
        assert_eq!(evidence[0]["label"], "淅淅");
        assert_eq!(
            evidence[0]["image_url"],
            "/api/workspaces/demo/generation-jobs/ref/image"
        );
        assert_eq!(evidence[0]["generation_job_id"], "job-1");
        assert_eq!(evidence[0]["style_version"], 3);
    }

    #[test]
    fn page_camera_priority_guard_keeps_wide_shots_above_local_details() {
        let guard =
            page_camera_priority_guard("儿童绘本插图，全景，老师弯腰看着孩子，孩子眼睛亮晶晶。");
        assert!(guard.contains("镜头优先级最高"));
        assert!(guard.contains("也不能把镜头推近"));
    }

    #[test]
    fn visual_reference_jobs_are_executable_image_jobs() {
        let asset_reference_id = Uuid::new_v4();
        let job = GenerationJob {
            id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            storybook_id: None,
            created_by: None,
            job_type: "storybook_visual_reference".to_string(),
            status: "queued".to_string(),
            input_json: json!({
                "asset_reference_id": asset_reference_id,
                "prompt": "生成同画风参考",
                "image_mode": "reference_image",
                "reference_images": [{
                    "url": "/storybook-assets/source.png",
                    "source": "storybook_asset",
                    "role_id": null,
                    "label": "爸爸"
                }]
            }),
            output_json: None,
            attempt_count: 0,
            last_error: None,
            next_run_at: None,
            locked_by: None,
            locked_at: None,
            created_at: chrono::Utc::now(),
            finished_at: None,
        };

        assert!(is_image_job("storybook_visual_reference"));
        let target = image_target_from_job(&job).expect("target should parse");
        assert_eq!(target.target_type, "asset_reference");
        assert_eq!(target.target_id, asset_reference_id.to_string());
        let request = image_request_from_job(&job).expect("request should parse");
        assert_eq!(request.prompt, "生成同画风参考");
        assert_eq!(request.reference_images.len(), 1);
        assert_eq!(request.reference_images[0].source, "storybook_asset");
    }

    #[test]
    fn page_reference_ids_keep_photo_types_separate_from_legacy_input() {
        let page_plan = json!({
            "character_reference_ids": ["character-ref"],
            "prop_reference_ids": ["prop-ref"],
            "scene_reference_ids": ["scene-ref"],
            "asset_reference_ids": ["legacy-ref"]
        });

        let ids = page_reference_ids(&page_plan);

        assert_eq!(ids, vec!["character-ref", "prop-ref", "scene-ref"]);
        assert!(!ids.contains(&"legacy-ref".to_string()));
        assert!(has_typed_page_reference_ids(&page_plan));
        assert!(!has_typed_page_reference_ids(&json!({
            "asset_reference_ids": ["legacy-ref"]
        })));
    }

    #[test]
    fn page_photo_reference_clause_keeps_each_reference_type_in_its_scope() {
        let plan = json!({
            "page_evidence": [{
                "page_number": 2,
                "asset_references": [
                    { "asset_reference_id": "person-ref", "kind": "person", "display_name": "嘟嘟" },
                    { "asset_reference_id": "prop-ref", "kind": "object", "display_name": "小火车" },
                    { "asset_reference_id": "scene-ref", "kind": "scene", "display_name": "彩虹教室" }
                ]
            }]
        });

        let clause = page_photo_reference_clause(&plan, 2);

        assert!(clause.contains("角色参考（嘟嘟）仅约束对应人物的外观"));
        assert!(clause.contains("道具参考（小火车）仅约束对应物品或宠物"));
        assert!(clause.contains("场景参考（彩虹教室）仅约束本页地点"));
        assert!(clause.contains("不得作为本页故事场景"));
    }

    #[test]
    fn page_photo_reference_clause_only_describes_references_assigned_to_the_page() {
        let plan = json!({
            "page_plan": [{
                "page_number": 2,
                "character_reference_ids": ["person-ref"],
                "prop_reference_ids": [],
                "scene_reference_ids": []
            }],
            "confirmed_photo_references": [
                { "asset_reference_id": "person-ref", "reference_type": "character_reference", "display_name": "嘟嘟" },
                { "asset_reference_id": "scene-ref", "reference_type": "scene_reference", "display_name": "彩虹教室" }
            ]
        });

        let clause = page_photo_reference_clause(&plan, 2);

        assert!(clause.contains("角色参考（嘟嘟）"));
        assert!(!clause.contains("彩虹教室"));
        assert!(!clause.contains("场景参考"));
    }

    #[test]
    fn direct_creation_input_supplies_page_visual_reference_plan() {
        let direct_creation_input = json!({
            "page_evidence": [{
                "page_number": 2,
                "prop_reference_ids": ["prop-ref"],
                "scene_reference_ids": ["scene-ref"]
            }]
        });

        let plan = page_visual_reference_plan(json!({}), Some(direct_creation_input));

        assert_eq!(plan["page_evidence"][0]["page_number"], 2);
        assert_eq!(
            page_reference_ids(&plan["page_evidence"][0]),
            vec!["prop-ref", "scene-ref"]
        );
    }

    #[test]
    fn customization_plan_remains_authoritative_over_creation_input() {
        let customization_plan = json!({
            "page_evidence": [{ "page_number": 1, "prop_reference_ids": ["custom-ref"] }]
        });
        let direct_creation_input = json!({
            "page_evidence": [{ "page_number": 1, "prop_reference_ids": ["creation-ref"] }]
        });

        let plan = page_visual_reference_plan(customization_plan, Some(direct_creation_input));

        assert_eq!(
            page_reference_ids(&plan["page_evidence"][0]),
            vec!["custom-ref"]
        );
    }
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
