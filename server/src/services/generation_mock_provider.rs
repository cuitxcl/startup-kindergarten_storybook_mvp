use serde_json::{Value as JsonValue, json};

use crate::services::generation_provider_contract::{
    AiGenerationProvider, GenerationProviderError, GenerationRequest, ImageGenerationRequest,
};
use crate::services::generation_seedream_provider::write_generated_image;

pub struct MockGenerationProvider;
const MOCK_IMAGE_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEklEQVR4nGP4cGnfsxNbGCAUAEWMCcWN1afmAAAAAElFTkSuQmCC";

impl MockGenerationProvider {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl AiGenerationProvider for MockGenerationProvider {
    fn name(&self) -> &'static str {
        "mock"
    }

    async fn generate(
        &self,
        request: GenerationRequest<'_>,
    ) -> Result<JsonValue, GenerationProviderError> {
        Ok(match request.job_type {
            "storybook_plan" => mock_storybook_plan(request.input),
            "storybook_roles" => mock_storybook_roles(request.input),
            "storybook_pages" => mock_storybook_pages(request.input),
            "storybook_page_prompt" => mock_storybook_page_prompt(request.input),
            "customization_plan" => mock_customization_plan(request.input),
            "creation_understanding" => mock_creation_understanding(request.input),
            "creation_directions" => mock_creation_directions(request.input),
            "creation_outline" => mock_creation_outline(request.input),
            "creation_storybook_generate" => mock_creation_storybook_generate(request.input),
            other => {
                return Err(GenerationProviderError::new(format!(
                    "mock provider 不支持文本任务：{other}"
                )));
            }
        })
    }

    async fn generate_image(
        &self,
        request: ImageGenerationRequest<'_>,
    ) -> Result<JsonValue, GenerationProviderError> {
        let image_url = write_generated_image(
            &request.image_id.to_string(),
            MOCK_IMAGE_PNG_BASE64,
            self.name(),
        )?;
        Ok(json!({
            "schema_version": "generation.mock.v1",
            "provider": self.name(),
            "mode": request.mode,
            "image": {
                "target_id": request.target_id,
                "target_type": request.target_type,
                "image_url": image_url,
                "alt_text": "mock 生成图片",
                "prompt": request.prompt,
                "image_mode": request.image_mode.as_str(),
                "size": request.size,
                "reference_images": request.reference_images,
                "edit_instruction": request.edit_instruction,
                "strength": request.strength,
                "style_notes": ["本地 mock 结果", "不会调用外部生图服务"]
            },
            "message": "生成任务已完成，当前为 mock 图片结果"
        }))
    }
}

fn text(input: &JsonValue, key: &str, fallback: &str) -> String {
    input
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn mock_storybook_plan(input: &JsonValue) -> JsonValue {
    let title = text(input, "title", "动物世界");
    let theme = text(input, "theme", "动物迁徙");
    json!({
        "schema_version": "generation.mock.v1",
        "provider": "mock",
        "mode": "storybook_plan",
        "plan": {
            "title": title,
            "theme": theme,
            "summary": "孩子跟随老师观察动物移动和生活环境，在比较中发现自然规律。",
            "page_beats": [
                "第 1 页：孩子提出问题，故事开始。",
                "第 2 页：老师陪孩子观察第一个线索。",
                "第 3 页：孩子发现新的对比。",
                "第 4 页：大家一起验证想法。",
                "第 5 页：孩子记录观察结果。",
                "第 6 页：故事温和收束。"
            ],
            "role_requirements": "主角、老师和关键动物角色。",
            "teacher_focus": "适合课堂共读，鼓励观察、比较和表达。"
        },
        "message": "生成任务已完成，当前为 mock 方案结果"
    })
}

fn mock_storybook_roles(_input: &JsonValue) -> JsonValue {
    json!({
        "schema_version": "generation.mock.v1",
        "provider": "mock",
        "mode": "storybook_roles",
        "roles": [
            {
                "name": "小朋友",
                "role_type": "protagonist",
                "appearance": "4-5 岁小朋友，圆润可爱，蓝色上衣，黄色背带裤，表情好奇",
                "story_function": "提出问题并参与观察",
                "needs_consistency": true,
                "reference_status": "not_started"
            },
            {
                "name": "老师",
                "role_type": "teacher",
                "appearance": "温和老师，绿色衬衫，棕色长裤，戴眼镜，笑容亲切",
                "story_function": "陪伴并引导孩子观察",
                "needs_consistency": true,
                "reference_status": "not_started"
            },
            {
                "name": "大象",
                "role_type": "prop",
                "appearance": "圆润粘土大象，灰色身体，大耳朵，表情温和",
                "story_function": "作为观察对象出现",
                "needs_consistency": false,
                "reference_status": "not_started"
            }
        ],
        "message": "生成任务已完成，当前为 mock 角色结果"
    })
}

fn mock_storybook_pages(input: &JsonValue) -> JsonValue {
    let title = text(input, "title", "动物世界");
    json!({
        "schema_version": "generation.mock.v1",
        "provider": "mock",
        "mode": "storybook_pages",
        "pages": [
            mock_page(1, &title, "孩子在图画书里看到大象排队，产生了好奇。", "中景"),
            mock_page(2, "我们去看看", "老师带孩子一起观察动物为什么移动。", "全景"),
            mock_page(3, "找到线索", "孩子发现动物会为了水和食物寻找合适的地方。", "近景"),
            mock_page(4, "一起记录", "大家把观察到的发现画在纸上。", "俯视"),
            mock_page(5, "新的发现", "孩子比较不同动物的移动方式。", "跟随视角"),
            mock_page(6, "温柔的约定", "孩子和老师约定继续保护自然。", "中近景")
        ],
        "message": "生成任务已完成，当前为 mock 分页结果"
    })
}

fn mock_page(page_number: i32, title: &str, text: &str, camera: &str) -> JsonValue {
    json!({
        "page_number": page_number,
        "title": title,
        "body": text,
        "illustration_prompt": format!(
            "儿童绘本插图，{camera}，温暖教室和自然观察角，孩子与老师正在互动，画面有明确动作和表情，不出现文字。"
        )
    })
}

fn mock_storybook_page_prompt(input: &JsonValue) -> JsonValue {
    let page = input.get("page").unwrap_or(input);
    json!({
        "schema_version": "generation.mock.v1",
        "provider": "mock",
        "mode": "storybook_page_prompt",
        "page": {
            "title": text(page, "title", "新的页面"),
            "illustration_prompt": "儿童绘本插图，中景，孩子和老师围坐观察图片，动作自然，表情好奇，画面温暖，不出现文字。"
        },
        "message": "生成任务已完成，当前为 mock 插图描述结果"
    })
}

fn mock_customization_plan(input: &JsonValue) -> JsonValue {
    json!({
        "schema_version": "generation.mock.v1",
        "provider": "mock",
        "mode": "customization_plan",
        "plan": {
            "title": text(input, "title", "定制绘本"),
            "summary": "根据孩子特点生成的本地 mock 定制方案。",
            "teacher_notes": "当前为本地 mock，不会调用 DeepSeek。"
        },
        "message": "生成任务已完成，当前为 mock 定制结果"
    })
}

fn mock_creation_storybook_generate(input: &JsonValue) -> JsonValue {
    json!({
        "schema_version": "creation.provider.v1",
        "provider": "mock",
        "mode": "creation_storybook_generate",
        "creation_session_id": input.get("creation_session_id").cloned().unwrap_or(JsonValue::Null),
        "storybook_id": input.get("storybook_id").cloned().unwrap_or(JsonValue::Null),
        "materials": input.get("materials").cloned().unwrap_or_else(|| json!([])),
        "selected_direction": input.get("selected_direction").cloned().unwrap_or_else(|| json!({})),
        "outline": input.get("outline").cloned().unwrap_or_else(|| json!({})),
        "visual_preferences": input.get("visual_preferences").cloned().unwrap_or_else(|| json!({})),
        "message": "共创绘本草稿已生成，当前为 mock 聚合任务结果"
    })
}

fn mock_creation_understanding(input: &JsonValue) -> JsonValue {
    let quick_idea = text(input, "quick_idea", "给孩子做一本温柔成长故事");
    let scene = text(input, "use_scene", "家庭共读");
    let age_group = text(input, "age_group", "4-5 岁");
    let materials = mock_creation_materials(&quick_idea);
    json!({
        "schema_version": "creation.provider.v1",
        "provider": "mock",
        "mode": "creation_understanding",
        "understanding": {
            "summary": format!("我理解你想把“{}”变成一本有真实细节的儿童绘本。", quick_idea.chars().take(28).collect::<String>()),
            "target_user": if quick_idea.contains("老师") || quick_idea.contains("班") { "teacher" } else { "parent" },
            "goal": if quick_idea.contains("分享") { "帮助孩子理解分享和轮流" } else { "把真实生活里的小问题变成适合共读的成长故事" },
            "tone": if quick_idea.contains("温柔") { "温柔、鼓励、不说教" } else { "清楚、轻松、有陪伴感" },
            "scene": scene,
            "age_group": age_group
        },
        "materials": materials,
        "quality_flags": ["mock_provider"]
    })
}

fn mock_creation_materials(quick_idea: &str) -> Vec<JsonValue> {
    let mut labels = Vec::new();
    for token in [
        "乐乐",
        "红色小汽车",
        "星星班",
        "分享",
        "轮流",
        "妈妈",
        "爸爸",
        "老师",
    ] {
        if quick_idea.contains(token) {
            labels.push(token);
        }
    }
    if labels.is_empty() {
        labels.extend(["主角", "真实小事件"]);
    }
    labels
        .into_iter()
        .enumerate()
        .map(|(index, label)| {
            let material_type = if matches!(label, "乐乐" | "妈妈" | "爸爸" | "老师" | "主角")
            {
                "character"
            } else if matches!(label, "星星班") {
                "place"
            } else if matches!(label, "分享" | "轮流") {
                "theme"
            } else {
                "object"
            };
            json!({
                "id": format!("mat_{}", index + 1),
                "label": label,
                "type": material_type,
                "source": "ai_extracted",
                "confidence": 0.78,
                "locked": material_type != "theme"
            })
        })
        .collect()
}

fn mock_creation_directions(input: &JsonValue) -> JsonValue {
    let materials = input.get("materials").cloned().unwrap_or_else(|| json!([]));
    let material_ids = materials
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("id").and_then(|value| value.as_str()))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| vec!["mat_1".to_string()]);
    json!({
        "schema_version": "creation.provider.v1",
        "provider": "mock",
        "mode": "creation_directions",
        "directions": [
            mock_direction(1, "温柔练习", "从一个被理解的小情绪开始，让孩子慢慢尝试改变。", "适合情绪安抚和家庭共读", "把真实素材放在第一次愿意尝试的关键时刻。", "gentle", &material_ids),
            mock_direction(2, "有趣任务", "把成长目标变成轻松任务，让孩子在游戏感中完成选择。", "适合活泼课堂或亲子互动", "让专属物品成为推动任务的小线索。", "playful", &material_ids),
            mock_direction(3, "特别回忆", "把真实瞬间做成有纪念感的故事，让孩子看见自己被认真记住。", "适合作为礼物或阶段成长记录", "把地点和关系放在结尾的温柔约定里。", "warm", &material_ids)
        ],
        "quality_flags": ["mock_provider"]
    })
}

fn mock_direction(
    index: usize,
    title: &str,
    summary: &str,
    fit_reason: &str,
    personal_hook: &str,
    tone: &str,
    material_ids: &[String],
) -> JsonValue {
    json!({
        "id": format!("dir_{index}"),
        "title": title,
        "summary": summary,
        "fit_reason": fit_reason,
        "personal_hook": personal_hook,
        "material_ids": material_ids,
        "tone": tone
    })
}

fn mock_creation_outline(input: &JsonValue) -> JsonValue {
    let page_count = input
        .get("page_count")
        .and_then(|value| value.as_u64())
        .unwrap_or(6)
        .clamp(4, 12);
    let material_ids = input
        .get("selected_direction")
        .and_then(|value| value.get("material_ids"))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| vec!["mat_1".to_string()]);
    let pages = (1..=page_count)
        .map(|page_number| {
            json!({
                "page_number": page_number,
                "summary": format!("第 {page_number} 页让一个专属素材进入具体情节，推动故事温柔展开。"),
                "material_ids": material_ids
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": "creation.provider.v1",
        "provider": "mock",
        "mode": "creation_outline",
        "outline": {
            "summary": "一本围绕真实素材展开的专属成长绘本。",
            "pages": pages,
            "review_points": ["是否保留真实素材", "语气是否足够温柔", "页数节奏是否适合共读"]
        },
        "quality_flags": ["mock_provider"]
    })
}
