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
    let page_count = input
        .get("page_count")
        .and_then(JsonValue::as_u64)
        .unwrap_or(6)
        .clamp(1, 12);
    let outline = (1..=page_count)
        .map(|page_number| {
            json!({
                "page_range": page_number.to_string(),
                "goal": format!("第 {page_number} 页推进一个清楚的观察目标"),
                "beat": match page_number {
                    1 => "孩子提出问题，故事从真实场景开始。".to_string(),
                    n if n == page_count => "孩子整理发现，留下温柔约定。".to_string(),
                    _ => "老师陪孩子观察线索，情绪和行动继续推进。".to_string(),
                }
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": "generation.mock.v1",
        "provider": "mock",
        "mode": "storybook_plan",
        "plan": {
            "title": title,
            "theme": theme,
            "age_group": text(input, "age_group", "4-5 岁"),
            "summary": "孩子跟随老师观察动物移动和生活环境，在比较中发现自然规律。",
            "page_count": page_count,
            "outline": outline,
            "role_requirements": ["主角孩子", "陪伴老师", "关键观察对象"],
            "review_points": ["故事是否围绕主题", "页数节奏是否适合共读"]
        },
        "message": "生成任务已完成，当前为 mock 方案结果"
    })
}

fn mock_storybook_roles(input: &JsonValue) -> JsonValue {
    let teacher_name = text(input, "teacher_name", "老师");
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
                "name": teacher_name,
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
    let teacher_name = text(input, "teacher_name", "老师");
    json!({
        "schema_version": "generation.mock.v1",
        "provider": "mock",
        "mode": "storybook_pages",
        "pages": [
            mock_page(1, &title, "孩子在图画书里看到大象排队，产生了好奇。", "中景", &teacher_name),
            mock_page(2, "我们去看看", &format!("{teacher_name}带孩子一起观察动物为什么移动。"), "全景", &teacher_name),
            mock_page(3, "找到线索", "孩子发现动物会为了水和食物寻找合适的地方。", "近景", &teacher_name),
            mock_page(4, "一起记录", "大家把观察到的发现画在纸上。", "俯视", &teacher_name),
            mock_page(5, "新的发现", "孩子比较不同动物的移动方式。", "跟随视角", &teacher_name),
            mock_page(6, "温柔的约定", &format!("孩子和{teacher_name}约定继续保护自然。"), "中近景", &teacher_name)
        ],
        "message": "生成任务已完成，当前为 mock 分页结果"
    })
}

fn mock_page(
    page_number: i32,
    title: &str,
    body: &str,
    camera: &str,
    teacher_name: &str,
) -> JsonValue {
    json!({
        "page_number": page_number,
        "title": title,
        "body": body,
        "illustration_prompt": format!(
            "儿童绘本插图，{camera}，温暖教室和自然观察角，孩子与{teacher_name}正在互动，画面有明确动作和表情，不出现文字。"
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
    let source_snapshot = input.get("source_snapshot").cloned().unwrap_or_else(|| {
        json!({
            "title": text(input, "title", "来源绘本"),
            "status": "exportable",
            "updated_at": "mock",
            "page_count": 6,
            "pages": [
                {"page_number": 1, "title": "开场", "summary": "建立原书场景"},
                {"page_number": 2, "title": "尝试", "summary": "目标对象进入情节"},
                {"page_number": 3, "title": "变化", "summary": "关键素材推动冲突"},
                {"page_number": 4, "title": "解决", "summary": "角色一起尝试办法"},
                {"page_number": 5, "title": "发现", "summary": "目标对象获得新理解"},
                {"page_number": 6, "title": "约定", "summary": "保持原书温柔收束"}
            ]
        })
    });
    let pages = source_snapshot
        .get("pages")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_else(|| {
            (1..=6)
                .map(|page_number| json!({"page_number": page_number, "title": "分页", "summary": "来源页摘要"}))
                .collect()
        });
    let confirmed_photo_references = input
        .get("confirmed_photo_references")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();
    let photo_names = confirmed_photo_references
        .iter()
        .filter_map(|item| item.get("display_name").and_then(JsonValue::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let page_plan = pages
        .iter()
        .map(|page| {
            let page_number = page
                .get("page_number")
                .and_then(JsonValue::as_u64)
                .unwrap_or(1);
            let decision = if page_number == 1 {
                "keep"
            } else if page_number % 3 == 0 {
                "redraw_required"
            } else {
                "personalize"
            };
            json!({
                "page_number": page_number,
                "decision": decision,
                "requires_redraw": decision == "redraw_required",
                "reason": if decision == "keep" { "保留来源书开场节奏" } else { "把目标对象和专属素材自然放入这一页" },
                "material_labels": input.get("material_labels").cloned().unwrap_or_else(|| json!([])),
                "photo_display_names": photo_names,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": "generation.mock.v1",
        "provider": "mock",
        "mode": "customization_plan",
        "customization_plan": {
            "source_snapshot": source_snapshot,
            "strategy": "保留来源书主线、页数和阅读节奏，只替换目标对象相关页面。",
            "page_plan": page_plan,
            "confirmed_photo_references": confirmed_photo_references,
            "unplaced_materials": [],
            "risk_checks": ["mock_provider：请用真实 provider 复核最终文案和重绘范围"]
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

#[cfg(test)]
mod tests {
    use super::{mock_customization_plan, mock_storybook_plan};
    use serde_json::json;

    #[test]
    fn mock_storybook_plan_matches_real_plan_contract() {
        let output = mock_storybook_plan(&json!({
            "title": "排队洗手",
            "theme": "轮流等待",
            "age_group": "中班",
            "page_count": 6
        }));

        assert_eq!(output["mode"], "storybook_plan");
        assert_eq!(output["plan"]["title"], "排队洗手");
        assert_eq!(output["plan"]["outline"].as_array().unwrap().len(), 6);
        assert!(output["plan"]["outline"][0]["page_range"].is_string());
        assert!(output["plan"]["role_requirements"].is_array());
        assert!(output["plan"]["review_points"].is_array());
    }

    #[test]
    fn mock_customization_plan_matches_prompt_contract_without_internal_ids() {
        let output = mock_customization_plan(&json!({
            "source_snapshot": {
                "title": "小熊等一等",
                "status": "exportable",
                "updated_at": "2026-08-21T00:00:00Z",
                "page_count": 2,
                "pages": [
                    {"page_number": 1, "title": "门口", "summary": "大家排队"},
                    {"page_number": 2, "title": "轮到我", "summary": "小熊尝试等待"}
                ]
            },
            "confirmed_photo_references": [{
                "display_name": "小汽车",
                "usage": "story_object",
                "reference_type": "道具参考",
                "planned_pages": [{"page_number": 2, "reason": "推动任务"}]
            }]
        }));

        let plan = &output["customization_plan"];
        assert!(plan["source_snapshot"].is_object());
        assert_eq!(plan["page_plan"].as_array().unwrap().len(), 2);
        assert!(plan["confirmed_photo_references"].is_array());
        assert!(plan["unplaced_materials"].is_array());
        assert!(plan.get("target_child_id").is_none());
        assert!(plan.get("source_storybook_id").is_none());
        assert!(plan.get("asset_reference_id").is_none());
    }
}
