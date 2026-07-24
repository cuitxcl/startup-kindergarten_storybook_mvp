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
    validate_provider_output_shape(&object, job_type)?;
    validate_provider_output_content_safety(&JsonValue::Object(object.clone()), job_type)?;

    Ok(JsonValue::Object(object))
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
                required_text_at(item, "goal", job_type, &format!("outline[{index}]"))?;
                required_text_at(item, "beat", job_type, &format!("outline[{index}]"))?;
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
