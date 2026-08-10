use serde_json::{Value as JsonValue, json};

use crate::services::generation_provider_contract::{
    AiGenerationProvider, GenerationProviderError, GenerationRequest, ImageGenerationRequest,
};

pub struct MockGenerationProvider;

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
        Ok(json!({
            "schema_version": "generation.mock.v1",
            "provider": self.name(),
            "mode": request.mode,
            "image": {
                "target_id": request.target_id,
                "target_type": request.target_type,
                "image_url": format!("/generated-images/mock-{}.png", request.image_id),
                "alt_text": "mock 生成图片",
                "prompt": request.prompt,
                "image_mode": request.image_mode.as_str(),
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
                "role_type": "main",
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
