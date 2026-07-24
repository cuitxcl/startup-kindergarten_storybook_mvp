use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde_json::{Value as JsonValue, json};

use crate::services::{
    generation_provider_contract::{
        AiGenerationProvider, GenerationProviderError, GenerationRequest, ImageGenerationRequest,
    },
    storage,
};

pub struct MockGenerationProvider;

impl AiGenerationProvider for MockGenerationProvider {
    fn name(&self) -> &'static str {
        "mock"
    }

    async fn generate(
        &self,
        request: GenerationRequest<'_>,
    ) -> Result<JsonValue, GenerationProviderError> {
        let output = match request.job_type {
            "storybook_plan" => storybook_plan(request.input),
            "storybook_roles" => storybook_roles(request.input),
            "storybook_pages" => storybook_pages(request.input),
            "customization_plan" => customization_plan(request.input),
            _ => base_output(request.job_type, "生成任务已完成，当前为 mock 结果"),
        };
        Ok(output)
    }

    async fn generate_image(
        &self,
        request: ImageGenerationRequest<'_>,
    ) -> Result<JsonValue, GenerationProviderError> {
        let image_url = write_placeholder_image(request.image_id, "mock")?;
        Ok(json!({
            "schema_version": "generation.mock.v1",
            "provider": self.name(),
            "mode": request.mode,
            "message": "插图任务已完成，当前为 mock 图片结果",
            "image": {
                "target_id": request.target_id,
                "target_type": request.target_type,
                "page_id": if request.target_type == "page" { request.target_id } else { "" },
                "role_id": if request.target_type == "role" { request.target_id } else { "" },
                "image_url": image_url,
                "alt_text": "幼儿园教室里的温暖共读场景",
                "prompt": request.prompt,
                "image_mode": request.image_mode.as_str(),
                "reference_images": request.reference_images,
                "edit_instruction": request.edit_instruction,
                "strength": request.strength,
                "style_notes": ["温暖纸感", "儿童绘本", "角色外观保持一致"]
            }
        }))
    }
}

fn storybook_plan(input: &JsonValue) -> JsonValue {
    let theme = text(input, "theme")
        .or_else(|| text(input, "teaching_goal"))
        .unwrap_or("学习轮流、等待和表达感受");
    let title = text(input, "title").unwrap_or("一起试试看");
    let age_group = text(input, "age_group").unwrap_or("4-5 岁");

    json!({
        "schema_version": "generation.mock.v1",
        "provider": "mock",
        "mode": "storybook_plan",
        "message": "绘本方案已生成，当前为 mock 方案结果",
        "plan": {
            "title": title,
            "theme": theme,
            "age_group": age_group,
            "summary": format!("围绕“{theme}”设计一个适合幼儿园共读的短故事。"),
            "page_count": 6,
            "outline": [
                {"page_range": "1", "goal": "进入场景", "beat": "孩子发现一个和主题有关的小问题"},
                {"page_range": "2", "goal": "出现冲突", "beat": "朋友们有不同想法，需要老师引导"},
                {"page_range": "3", "goal": "提出办法", "beat": "老师把规则变成孩子能理解的小步骤"},
                {"page_range": "4-5", "goal": "尝试练习", "beat": "孩子们轮流尝试，并说出自己的感受"},
                {"page_range": "6", "goal": "收束迁移", "beat": "大家把新办法带回日常生活"}
            ],
            "role_requirements": ["主角儿童", "同伴儿童", "老师引导者", "关键道具"],
            "review_points": ["教学目标是否准确", "故事冲突是否温和", "是否适合班级共读"]
        }
    })
}

fn storybook_roles(input: &JsonValue) -> JsonValue {
    let teacher_name = text(input, "teacher_name").unwrap_or("林老师");

    json!({
        "schema_version": "generation.mock.v1",
        "provider": "mock",
        "mode": "storybook_roles",
        "message": "角色与道具设定已生成，当前为 mock 设定结果",
        "roles": [
            {
                "name": "米米",
                "role_type": "protagonist",
                "appearance": "短发、黄色背带裤、表情好奇",
                "story_function": "代表正在学习规则的孩子",
                "needs_consistency": true
            },
            {
                "name": "乐乐",
                "role_type": "peer",
                "appearance": "蓝色上衣、喜欢提问、动作活泼",
                "story_function": "推动同伴互动和冲突出现",
                "needs_consistency": true
            },
            {
                "name": teacher_name,
                "role_type": "teacher",
                "appearance": "温柔、清楚、穿浅色围裙，适合幼儿园教室场景",
                "story_function": "把规则转化为可执行的小步骤",
                "needs_consistency": true
            },
            {
                "name": "小沙漏",
                "role_type": "prop",
                "appearance": "透明沙漏，红色边框",
                "story_function": "帮助孩子理解等待和轮流",
                "needs_consistency": true
            }
        ],
        "consistency_guide": ["固定服装主色", "老师形象保持稳定", "关键道具每次出现都同色同形"]
    })
}

fn storybook_pages(input: &JsonValue) -> JsonValue {
    let page_count = input
        .get("page_count")
        .and_then(|value| value.as_u64())
        .unwrap_or(6)
        .clamp(4, 8);

    let pages = (1..=page_count)
        .map(|number| {
            json!({
                "page_number": number,
                "title": page_title(number),
                "body": page_body(number),
                "illustration_prompt": page_prompt(number),
                "status": "draft"
            })
        })
        .collect::<Vec<_>>();

    json!({
        "schema_version": "generation.mock.v1",
        "provider": "mock",
        "mode": "storybook_pages",
        "message": "分页图文已生成，当前为 mock 分页结果",
        "pages": pages,
        "editor_notes": ["每页文字控制在幼儿可共读长度", "插图 prompt 保留角色一致性线索"]
    })
}

fn customization_plan(input: &JsonValue) -> JsonValue {
    let child_id = text(input, "child_id").unwrap_or("待选择儿童");
    let intensity = text(input, "intensity").unwrap_or("standard");

    json!({
        "schema_version": "generation.mock.v1",
        "provider": "mock",
        "mode": "customization_plan",
        "message": "定制方案已生成，当前为 mock 定制结果",
        "customization": {
            "child_id": child_id,
            "intensity": intensity,
            "strategy": "保留母本主线，只替换称呼、兴趣道具和少量情节细节。",
            "rewrite_points": [
                {"scope": "title", "action": "加入孩子称呼"},
                {"scope": "pages", "action": "把关键道具替换为孩子感兴趣的元素"},
                {"scope": "illustrations", "action": "仅重绘出现儿童个性化元素的页面"}
            ],
            "risk_checks": ["避免暴露敏感家庭信息", "不改变老师确认过的规则引导目标"]
        }
    })
}

fn base_output(job_type: &str, message: &str) -> JsonValue {
    json!({
        "schema_version": "generation.mock.v1",
        "provider": "mock",
        "mode": job_type,
        "message": message
    })
}

fn text<'a>(input: &'a JsonValue, key: &str) -> Option<&'a str> {
    input
        .get(key)
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
}

fn page_title(number: u64) -> &'static str {
    match number {
        1 => "小问题出现了",
        2 => "朋友也有想法",
        3 => "老师给出小办法",
        4 => "我们试一试",
        5 => "再来一次",
        _ => "把办法带回生活",
    }
}

fn page_body(number: u64) -> &'static str {
    match number {
        1 => "米米带着喜欢的玩具来到教室，大家都想一起玩。",
        2 => "乐乐也伸出手，两个孩子都很着急，不知道该怎么办。",
        3 => "林老师蹲下来，轻声说：我们可以用小沙漏来轮流。",
        4 => "沙子慢慢落下，米米看着沙漏，试着等待自己的下一次机会。",
        5 => "轮到乐乐时，米米发现等待也没有那么难。",
        _ => "收玩具的时候，大家都记住了：先说一说，再轮流玩。",
    }
}

fn page_prompt(number: u64) -> &'static str {
    match number {
        1 => "温暖幼儿园教室，主角孩子拿着玩具，朋友们好奇围过来",
        2 => "两个孩子同时想玩同一个玩具，表情着急但场景温和",
        3 => "老师蹲下与孩子平视，手里拿着红色小沙漏",
        4 => "孩子看着沙漏等待，旁边朋友正在玩玩具",
        5 => "两个孩子轮流玩玩具，表情放松开心",
        _ => "孩子们一起整理玩具，老师微笑鼓励",
    }
}

const TRANSPARENT_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEklEQVR4nGP4cGnfsxNbGCAUAEWMCcWN1afmAAAAAElFTkSuQmCC";

fn write_placeholder_image(
    image_id: &str,
    provider: &str,
) -> Result<String, GenerationProviderError> {
    let file_name = generated_image_file_name(image_id, provider);
    let bytes = BASE64_STANDARD
        .decode(TRANSPARENT_PNG_BASE64)
        .map_err(|err| GenerationProviderError::new(format!("解码占位图片失败：{err}")))?;
    storage::save_generated_image(&file_name, &bytes).map_err(GenerationProviderError::new)
}

fn generated_image_file_name(image_id: &str, provider: &str) -> String {
    let image_id = image_id.replace(['/', '\\'], "_");
    let provider = provider.replace(['/', '\\'], "_");
    format!("{provider}-{image_id}.png")
}
