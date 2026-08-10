use serde_json::{Map as JsonMap, Value as JsonValue, json};

use crate::services::{
    generation_privacy::validate_provider_output_content_safety,
    generation_provider_contract::GenerationProviderError,
};

pub(crate) fn normalize_provider_output(
    output: JsonValue,
    provider: &str,
    job_type: &str,
    provider_usage: Option<JsonValue>,
    privacy_audit: Option<JsonValue>,
    style: Option<&str>,
) -> Result<JsonValue, GenerationProviderError> {
    let Some(mut object) = output.as_object().cloned() else {
        return Err(GenerationProviderError::new(
            "provider 输出必须是 JSON object",
        ));
    };

    insert_if_missing(
        &mut object,
        "schema_version",
        json!("generation.provider.v1"),
    );
    object.insert("provider".to_string(), json!(provider));
    object.insert("mode".to_string(), json!(job_type));
    if let Some(usage) = provider_usage {
        object.insert("provider_usage".to_string(), usage);
    }
    if let Some(audit) = privacy_audit {
        object.insert("privacy_audit".to_string(), audit);
    }
    insert_if_missing(&mut object, "message", json!("生成任务已完成"));
    normalize_provider_output_values(&mut object, job_type, style)?;
    validate_provider_output_shape(&object, job_type)?;
    validate_provider_output_content_safety(&JsonValue::Object(object.clone()), job_type)?;

    Ok(JsonValue::Object(object))
}

fn normalize_provider_output_values(
    object: &mut JsonMap<String, JsonValue>,
    job_type: &str,
    style: Option<&str>,
) -> Result<(), GenerationProviderError> {
    match job_type {
        "storybook_roles" => normalize_storybook_roles_values(object),
        "storybook_pages" => normalize_storybook_pages_values(object, style)?,
        "storybook_page_prompt" => normalize_storybook_page_prompt_values(object, style)?,
        _ => {}
    }
    Ok(())
}

/// storybook_page_prompt 的插图设定从结构化槽位拼装为 page.illustration_prompt。
fn normalize_storybook_page_prompt_values(
    object: &mut JsonMap<String, JsonValue>,
    style: Option<&str>,
) -> Result<(), GenerationProviderError> {
    let Some(page_object) = object
        .get_mut("page")
        .and_then(|value| value.as_object_mut())
    else {
        return Ok(());
    };
    let Some(assembled) =
        assemble_illustration_prompt(page_object, "storybook_page_prompt.page", style)?
    else {
        // 缺失插图字段的情况交给结构校验统一报错。
        return Ok(());
    };
    page_object.insert("illustration_prompt".to_string(), json!(assembled));
    Ok(())
}

/// 从 page 对象的 illustration 槽位拼装插图提示词；旧格式 illustration_prompt 直接透传。
/// 返回 None 表示两个字段都缺失，交给结构校验统一报错。
fn assemble_illustration_prompt(
    page_object: &JsonMap<String, JsonValue>,
    location: &str,
    style: Option<&str>,
) -> Result<Option<String>, GenerationProviderError> {
    let assembled = if let Some(illustration) = page_object
        .get("illustration")
        .and_then(|value| value.as_object())
    {
        let mut parts = Vec::new();
        for slot in REQUIRED_ILLUSTRATION_SLOTS {
            let text = illustration
                .get(*slot)
                .and_then(|value| value.as_str())
                .map(str::trim)
                .unwrap_or("");
            if text.is_empty() {
                return Err(GenerationProviderError::new(format!(
                    "provider 输出 {location}.illustration.{slot} 必须是非空文本"
                )));
            }
            parts.push(clean_illustration_slot_text(text));
        }
        let prop_detail = illustration
            .get("prop_detail")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .unwrap_or("");
        if !prop_detail.is_empty() {
            parts.push(clean_illustration_slot_text(prop_detail));
        }
        format!(
            "儿童绘本插图，{}。{}",
            parts.join("，"),
            illustration_style_suffix(style)
        )
    } else if let Some(prompt) = page_object
        .get("illustration_prompt")
        .and_then(|value| value.as_str())
    {
        prompt.trim().to_string()
    } else {
        return Ok(None);
    };
    if let Some(word) = FORBIDDEN_ILLUSTRATION_WORDING
        .iter()
        .find(|word| assembled.contains(**word))
    {
        return Err(GenerationProviderError::new(format!(
            "provider 输出 {location} 插图提示词含有禁止写法：{word}（会让画面呆板或丢失叙事信息）"
        )));
    }
    Ok(Some(assembled))
}

fn normalize_storybook_roles_values(object: &mut JsonMap<String, JsonValue>) {
    let Some(roles) = object
        .get_mut("roles")
        .and_then(|value| value.as_array_mut())
    else {
        return;
    };
    for role in roles {
        let Some(role_object) = role.as_object_mut() else {
            continue;
        };
        let normalized = role_object
            .get("role_type")
            .and_then(|value| value.as_str())
            .map(normalize_role_type)
            .unwrap_or_else(|| "supporting".to_string());
        role_object.insert("role_type".to_string(), json!(normalized));
    }
}

/// 插图提示词风格后缀由后端统一拼接，避免模型自由发挥导致风格漂移。
/// 默认水彩风格；用户在需求里选择了画风时，改用用户画风 + 固定质量约束。
const ILLUSTRATION_STYLE_SUFFIX: &str = "柔和水彩绘本风格，圆润饱满造型，大而富有表现力的眼睛，角色身体结构必须严格符合已确认外观，不要凭空添加手、脚、手臂、腿、鞋子或外观未写到的肢体，暖色调，画面充满动感和童趣。画面中不要出现文字。";
/// 与画风无关的质量约束：遵守角色身体结构、无文字。任何画风都必须携带。
const ILLUSTRATION_QUALITY_SUFFIX: &str = "角色身体结构必须严格符合已确认外观，不要凭空添加手、脚、手臂、腿、鞋子或外观未写到的肢体，画面充满动感和童趣。画面中不要出现文字。";

/// 用户在需求里选择的画风（input.style）优先；为空时回退默认水彩风格。
fn illustration_style_suffix(style: Option<&str>) -> String {
    match style.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => format!(
            "{}。{}",
            value.trim_end_matches('。'),
            ILLUSTRATION_QUALITY_SUFFIX
        ),
        None => ILLUSTRATION_STYLE_SUFFIX.to_string(),
    }
}

const REQUIRED_ILLUSTRATION_SLOTS: &[&str] = &[
    "camera",
    "scene_state",
    "contact_chain",
    "crowd",
    "action",
    "expression",
];

/// 这些写法会让画面呆板或把叙事关键信息抹掉，出现在任何插图提示词里都视为不合格输出。
const FORBIDDEN_ILLUSTRATION_WORDING: &[&str] = &[
    "背景虚化",
    "背景模糊",
    "柔焦",
    "略显",
    "并排站",
    "面无表情",
    "人群最前面",
    "证件照",
];

/// storybook_pages 的插图设定从结构化槽位拼装为最终 illustration_prompt。
/// 旧格式（只有 illustration_prompt 字符串）保持兼容，只做禁止写法检查。
fn normalize_storybook_pages_values(
    object: &mut JsonMap<String, JsonValue>,
    style: Option<&str>,
) -> Result<(), GenerationProviderError> {
    let Some(pages) = object
        .get_mut("pages")
        .and_then(|value| value.as_array_mut())
    else {
        return Ok(());
    };
    let mut structured_prompts = Vec::new();
    for (index, page) in pages.iter_mut().enumerate() {
        let Some(page_object) = page.as_object_mut() else {
            continue;
        };
        let has_structured_illustration = page_object
            .get("illustration")
            .and_then(|value| value.as_object())
            .is_some();
        let location = format!("storybook_pages.pages[{index}]");
        let Some(assembled) = assemble_illustration_prompt(page_object, &location, style)? else {
            // 缺失插图字段的情况交给结构校验统一报错。
            continue;
        };
        if has_structured_illustration {
            structured_prompts.push(assembled.clone());
        }
        page_object.insert("illustration_prompt".to_string(), json!(assembled));
    }
    validate_storybook_page_camera_presence(&structured_prompts)?;
    Ok(())
}

fn normalize_camera_shot(prompt: &str) -> Option<&'static str> {
    let candidates = [
        ("局部特写", "局部特写"),
        ("跟随视角", "跟随视角"),
        ("俯视", "俯视"),
        ("中近景", "中近景"),
        ("远景", "远景"),
        ("全景", "全景"),
        ("中景", "中景"),
        ("近景", "近景"),
        ("特写", "特写"),
    ];
    candidates
        .iter()
        .find_map(|(needle, label)| prompt.contains(*needle).then_some(*label))
}

fn validate_storybook_page_camera_presence(
    prompts: &[String],
) -> Result<(), GenerationProviderError> {
    let shots: Vec<Option<&'static str>> = prompts
        .iter()
        .map(|prompt| normalize_camera_shot(prompt))
        .collect();

    if shots.iter().all(Option::is_none) {
        return Ok(());
    }

    if let Some(index) = shots.iter().position(Option::is_none) {
        return Err(GenerationProviderError::new(format!(
            "provider 输出 storybook_pages.pages[{index}] 插图提示词缺少明确镜头景别，请在 camera 中写出远景、全景、中景、中近景、近景、特写、局部特写、俯视或跟随视角"
        )));
    }

    Ok(())
}

fn clean_illustration_slot_text(value: &str) -> String {
    value
        .trim()
        .trim_end_matches(['。', '，', ',', '；', ';', '、'])
        .trim()
        .to_string()
}

fn normalize_role_type(value: &str) -> String {
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

fn insert_if_missing(object: &mut JsonMap<String, JsonValue>, key: &str, value: JsonValue) {
    if !object.contains_key(key) {
        object.insert(key.to_string(), value);
    }
}

fn validate_provider_output_shape(
    object: &JsonMap<String, JsonValue>,
    job_type: &str,
) -> Result<(), GenerationProviderError> {
    match job_type {
        "storybook_plan" => {
            let plan = required_object(object, "plan", job_type)?;
            required_text(plan, "title", job_type)?;
            required_text(plan, "theme", job_type)?;
            required_text(plan, "summary", job_type)?;
            let outline = required_array(plan, "outline", job_type)?;
            for (index, item) in outline.iter().enumerate() {
                let item = item.as_object().ok_or_else(|| {
                    GenerationProviderError::new(format!(
                        "provider 输出 {job_type}.outline[{index}] 必须是 object"
                    ))
                })?;
                required_text_at(item, "page_range", job_type, &format!("outline[{index}]"))?;
                let page_range = item
                    .get("page_range")
                    .and_then(JsonValue::as_str)
                    .unwrap_or_default();
                // 分页节奏必须一页一条，禁止 "1-2"、"第3~4页" 这类跨页区间写法。
                if ["-", "–", "—", "~", "～", "至", "到"]
                    .iter()
                    .any(|mark| page_range.contains(mark))
                {
                    return Err(GenerationProviderError::new(format!(
                        "provider 输出 {job_type}.outline[{index}].page_range 只允许单页页码（如 \"3\"），不允许跨页区间（如 \"1-2\"），请把每一页拆成独立的一条"
                    )));
                }
                required_text_at(item, "goal", job_type, &format!("outline[{index}]"))?;
                required_text_at(item, "beat", job_type, &format!("outline[{index}]"))?;
            }
            // outline 条目数必须与 page_count 一致，保证每条节奏恰好对应一页。
            if let Some(page_count) = plan.get("page_count").and_then(JsonValue::as_u64) {
                if outline.len() as u64 != page_count {
                    return Err(GenerationProviderError::new(format!(
                        "provider 输出 {job_type}.outline 共 {} 条，与 page_count={page_count} 不一致；分页节奏必须一页一条，共 page_count 条",
                        outline.len()
                    )));
                }
            }
            let role_requirements = required_array(plan, "role_requirements", job_type)?;
            for (index, requirement) in role_requirements.iter().enumerate() {
                let has_text = requirement
                    .as_str()
                    .is_some_and(|value| !value.trim().is_empty());
                if !has_text {
                    return Err(GenerationProviderError::new(format!(
                        "provider 输出 {job_type}.role_requirements[{index}] 必须是非空文本"
                    )));
                }
            }
            let review_points = required_array(plan, "review_points", job_type)?;
            for (index, point) in review_points.iter().enumerate() {
                let has_text = point.as_str().is_some_and(|value| !value.trim().is_empty());
                if !has_text {
                    return Err(GenerationProviderError::new(format!(
                        "provider 输出 {job_type}.review_points[{index}] 必须是非空文本"
                    )));
                }
            }
        }
        "storybook_roles" => {
            let roles = required_array(object, "roles", job_type)?;
            for (index, role) in roles.iter().enumerate() {
                let role = role.as_object().ok_or_else(|| {
                    GenerationProviderError::new(format!(
                        "provider 输出 {job_type}.roles[{index}] 必须是 object"
                    ))
                })?;
                required_text_at(role, "name", job_type, &format!("roles[{index}]"))?;
                required_text_at(role, "role_type", job_type, &format!("roles[{index}]"))?;
                required_text_at(role, "appearance", job_type, &format!("roles[{index}]"))?;
                required_text_at(role, "story_function", job_type, &format!("roles[{index}]"))?;
            }
        }
        "storybook_pages" => {
            let pages = required_array(object, "pages", job_type)?;
            for (index, page) in pages.iter().enumerate() {
                let page = page.as_object().ok_or_else(|| {
                    GenerationProviderError::new(format!(
                        "provider 输出 {job_type}.pages[{index}] 必须是 object"
                    ))
                })?;
                required_text_at(page, "title", job_type, &format!("pages[{index}]"))?;
                required_text_at(page, "body", job_type, &format!("pages[{index}]"))?;
                required_text_at(
                    page,
                    "illustration_prompt",
                    job_type,
                    &format!("pages[{index}]"),
                )?;
            }
        }
        "storybook_page_prompt" => {
            let page = required_object(object, "page", job_type)?;
            required_text(page, "illustration_prompt", job_type)?;
        }
        "customization_plan" => {
            let customization = required_object(object, "customization", job_type)?;
            required_text(customization, "strategy", job_type)?;
            let rewrite_points = required_array(customization, "rewrite_points", job_type)?;
            for (index, point) in rewrite_points.iter().enumerate() {
                let point = point.as_object().ok_or_else(|| {
                    GenerationProviderError::new(format!(
                        "provider 输出 {job_type}.rewrite_points[{index}] 必须是 object"
                    ))
                })?;
                required_text_at(
                    point,
                    "scope",
                    job_type,
                    &format!("rewrite_points[{index}]"),
                )?;
                required_text_at(
                    point,
                    "action",
                    job_type,
                    &format!("rewrite_points[{index}]"),
                )?;
            }
            let risk_checks = required_array(customization, "risk_checks", job_type)?;
            for (index, check) in risk_checks.iter().enumerate() {
                let has_text = check.as_str().is_some_and(|value| !value.trim().is_empty());
                if !has_text {
                    return Err(GenerationProviderError::new(format!(
                        "provider 输出 {job_type}.risk_checks[{index}] 必须是非空文本"
                    )));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn required_object<'a>(
    object: &'a JsonMap<String, JsonValue>,
    key: &str,
    job_type: &str,
) -> Result<&'a JsonMap<String, JsonValue>, GenerationProviderError> {
    object
        .get(key)
        .and_then(|value| value.as_object())
        .ok_or_else(|| {
            GenerationProviderError::new(format!("provider 输出 {job_type}.{key} 必须是 object"))
        })
}

fn required_array<'a>(
    object: &'a JsonMap<String, JsonValue>,
    key: &str,
    job_type: &str,
) -> Result<&'a Vec<JsonValue>, GenerationProviderError> {
    let values = object
        .get(key)
        .and_then(|value| value.as_array())
        .ok_or_else(|| {
            GenerationProviderError::new(format!("provider 输出 {job_type}.{key} 必须是 array"))
        })?;
    if values.is_empty() {
        return Err(GenerationProviderError::new(format!(
            "provider 输出 {job_type}.{key} 不能为空"
        )));
    }
    Ok(values)
}

fn required_text(
    object: &JsonMap<String, JsonValue>,
    key: &str,
    job_type: &str,
) -> Result<(), GenerationProviderError> {
    required_text_at(object, key, job_type, "")
}

fn required_text_at(
    object: &JsonMap<String, JsonValue>,
    key: &str,
    job_type: &str,
    path: &str,
) -> Result<(), GenerationProviderError> {
    let has_text = object
        .get(key)
        .and_then(|value| value.as_str())
        .is_some_and(|value| !value.trim().is_empty());
    if !has_text {
        let field = if path.is_empty() {
            key.to_string()
        } else {
            format!("{path}.{key}")
        };
        return Err(GenerationProviderError::new(format!(
            "provider 输出 {job_type}.{field} 必须是非空文本"
        )));
    }
    Ok(())
}
