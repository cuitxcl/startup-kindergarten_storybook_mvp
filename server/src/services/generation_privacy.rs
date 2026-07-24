use serde_json::{Value as JsonValue, json};

use crate::services::generation_provider_contract::GenerationProviderError;

pub(crate) fn sanitize_provider_input(input: &JsonValue) -> JsonValue {
    sanitize_provider_value(input, None)
}

pub(crate) fn provider_input_privacy_audit(input: &JsonValue) -> JsonValue {
    let sanitized = sanitize_provider_input(input);
    let mut labels = Vec::new();
    collect_provider_redaction_labels(&sanitized, &mut labels);
    labels.sort_unstable();
    labels.dedup();
    json!({
        "redacted": !labels.is_empty(),
        "labels": labels
    })
}

fn collect_provider_redaction_labels(value: &JsonValue, labels: &mut Vec<&'static str>) {
    match value {
        JsonValue::Object(map) => {
            for item in map.values() {
                collect_provider_redaction_labels(item, labels);
            }
        }
        JsonValue::Array(items) => {
            for item in items {
                collect_provider_redaction_labels(item, labels);
            }
        }
        JsonValue::String(value) => match value.as_str() {
            "[redacted]" => labels.push("sensitive_field"),
            "[uuid_redacted]" => labels.push("uuid"),
            "[email_redacted]" => labels.push("email"),
            "[phone_redacted]" => labels.push("phone"),
            _ => {}
        },
        _ => {}
    }
}

pub(crate) fn sanitize_image_prompt_with_audit(prompt: &str) -> (String, Vec<&'static str>) {
    let token_redacted = separate_image_prompt_tokens(prompt)
        .split_whitespace()
        .map(sanitize_image_prompt_token)
        .collect::<Vec<_>>()
        .join(" ");
    let phone_redacted = redact_phone_sequences(&token_redacted);
    let sanitized = redact_sensitive_image_keywords(&phone_redacted);
    let mut labels = Vec::new();
    if sanitized.contains("[uuid_redacted]") {
        labels.push("uuid");
    }
    if sanitized.contains("[email_redacted]") {
        labels.push("email");
    }
    if sanitized.contains("[phone_redacted]") {
        labels.push("phone");
    }
    if sanitized.contains("[private_detail_redacted]") {
        labels.push("private_detail");
    }
    (sanitized, labels)
}

fn separate_image_prompt_tokens(prompt: &str) -> String {
    prompt
        .chars()
        .map(|ch| {
            if matches!(
                ch,
                ',' | ';'
                    | ':'
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '<'
                    | '>'
                    | '"'
                    | '\''
                    | '!'
                    | '?'
                    | '/'
                    | '\\'
                    | '|'
            ) || "，。；、：（）《》【】“”‘’".contains(ch)
            {
                ' '
            } else {
                ch
            }
        })
        .collect()
}

fn sanitize_image_prompt_token(token: &str) -> String {
    let trimmed = token.trim_matches(|ch: char| {
        ch.is_ascii_punctuation() || "，。；、：（）《》【】“”‘’".contains(ch)
    });
    if looks_like_uuid(trimmed) {
        token.replace(trimmed, "[uuid_redacted]")
    } else if looks_like_image_email_token(trimmed) {
        token.replace(trimmed, "[email_redacted]")
    } else if looks_like_image_phone_token(trimmed) {
        token.replace(trimmed, "[phone_redacted]")
    } else {
        token.to_string()
    }
}

fn looks_like_image_email_token(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && domain.contains('.')
        && local
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '%' | '+' | '-'))
        && domain
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-'))
        && !domain.starts_with('.')
        && !domain.ends_with('.')
}

fn looks_like_image_phone_token(value: &str) -> bool {
    value.len() == 11
        && value.chars().all(|ch| ch.is_ascii_digit())
        && value.starts_with('1')
        && value
            .as_bytes()
            .get(1)
            .is_some_and(|second| matches!(*second as char, '3'..='9'))
}

fn redact_phone_sequences(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    let mut output = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '1' && (index == 0 || !chars[index - 1].is_ascii_digit()) {
            let mut digits = String::new();
            let mut cursor = index;
            while cursor < chars.len() && digits.len() < 11 {
                let ch = chars[cursor];
                if ch.is_ascii_digit() {
                    digits.push(ch);
                } else if ch == ' ' || ch == '-' {
                    // Keep scanning common formatted phone numbers.
                } else {
                    break;
                }
                cursor += 1;
            }
            if digits.len() == 11
                && (cursor == chars.len() || !chars[cursor].is_ascii_digit())
                && matches!(digits.as_bytes()[1] as char, '3'..='9')
            {
                output.push_str("[phone_redacted]");
                index = cursor;
                continue;
            }
        }
        output.push(chars[index]);
        index += 1;
    }
    output
}

fn redact_sensitive_image_keywords(value: &str) -> String {
    let mut redacted = value.to_string();
    for keyword in [
        "家庭住址",
        "详细地址",
        "门牌号",
        "身份证",
        "证件号码",
        "病历",
        "诊断",
        "医保",
        "过敏史",
        "家长电话",
        "爸爸",
        "妈妈",
        "父亲",
        "母亲",
    ] {
        redacted = redacted.replace(keyword, "[private_detail_redacted]");
    }
    redacted
}

fn sanitize_provider_value(value: &JsonValue, key: Option<&str>) -> JsonValue {
    if key.is_some_and(is_sensitive_provider_key) {
        return JsonValue::String("[redacted]".to_string());
    }

    match value {
        JsonValue::Object(map) => JsonValue::Object(
            map.iter()
                .map(|(item_key, item_value)| {
                    (
                        item_key.clone(),
                        sanitize_provider_value(item_value, Some(item_key)),
                    )
                })
                .collect(),
        ),
        JsonValue::Array(items) => JsonValue::Array(
            items
                .iter()
                .map(|item| sanitize_provider_value(item, None))
                .collect(),
        ),
        JsonValue::String(value) if looks_like_uuid(value) => {
            JsonValue::String("[uuid_redacted]".to_string())
        }
        JsonValue::String(value) if looks_like_email(value) => {
            JsonValue::String("[email_redacted]".to_string())
        }
        JsonValue::String(value) if looks_like_phone(value) => {
            JsonValue::String("[phone_redacted]".to_string())
        }
        _ => value.clone(),
    }
}

fn is_sensitive_provider_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    let child_named_field = normalized.contains("child")
        && (normalized.contains("id")
            || normalized.contains("name")
            || normalized.contains("nickname"));
    child_named_field
        || normalized == "nickname"
        || normalized == "real_name"
        || normalized.contains("phone")
        || normalized.contains("email")
        || normalized.contains("address")
        || normalized.contains("parent")
        || normalized.contains("guardian")
        || normalized.contains("family")
        || normalized.contains("medical")
        || normalized.contains("diagnosis")
        || normalized.contains("id_card")
        || normalized.contains("identity")
        || normalized.contains("birthday")
}

fn looks_like_uuid(value: &str) -> bool {
    let parts: Vec<_> = value.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    let lengths = [8, 4, 4, 4, 12];
    parts
        .iter()
        .zip(lengths)
        .all(|(part, len)| part.len() == len && part.chars().all(|ch| ch.is_ascii_hexdigit()))
}

fn looks_like_email(value: &str) -> bool {
    let trimmed = value.trim();
    let Some((local, domain)) = trimmed.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.ends_with('.')
}

fn looks_like_phone(value: &str) -> bool {
    let digit_count = value.chars().filter(|ch| ch.is_ascii_digit()).count();
    digit_count >= 8
        && value
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, ' ' | '-' | '+' | '(' | ')'))
}

pub(crate) fn validate_provider_output_content_safety(
    output: &JsonValue,
    job_type: &str,
) -> Result<(), GenerationProviderError> {
    let risks = provider_output_privacy_risks(output);
    if risks.is_empty() {
        return Ok(());
    }
    Err(GenerationProviderError::new(format!(
        "provider 输出 {job_type} 包含敏感信息：{}",
        risks.join("、")
    )))
}

fn provider_output_privacy_risks(value: &JsonValue) -> Vec<&'static str> {
    let mut all_text = String::new();
    collect_provider_output_text(value, &mut all_text);
    let mut content_text = String::new();
    collect_provider_output_content_text(value, None, &mut content_text);
    let mut risks = Vec::new();
    if contains_output_email(&all_text) {
        risks.push("邮箱");
    }
    if contains_output_chinese_mobile(&all_text) {
        risks.push("手机号");
    }
    if contains_output_id_card(&all_text)
        || contains_output_any(&content_text, &["身份证", "身份证号", "证件号码"])
    {
        risks.push("身份信息");
    }
    if contains_output_any(
        &content_text,
        &["家庭住址", "详细地址", "门牌号", "楼栋", "单元号"],
    ) {
        risks.push("住址信息");
    }
    if contains_output_any(
        &content_text,
        &["病历", "诊断证明", "医保卡", "过敏史", "就诊记录"],
    ) {
        risks.push("医疗信息");
    }
    risks
}

fn collect_provider_output_text(value: &JsonValue, text: &mut String) {
    match value {
        JsonValue::String(value) => {
            text.push(' ');
            text.push_str(value);
        }
        JsonValue::Array(items) => {
            for item in items {
                collect_provider_output_text(item, text);
            }
        }
        JsonValue::Object(map) => {
            for item in map.values() {
                collect_provider_output_text(item, text);
            }
        }
        _ => {}
    }
}

fn collect_provider_output_content_text(value: &JsonValue, key: Option<&str>, text: &mut String) {
    if key.is_some_and(is_provider_output_safety_note_key) {
        return;
    }
    match value {
        JsonValue::String(value) => {
            text.push(' ');
            text.push_str(value);
        }
        JsonValue::Array(items) => {
            for item in items {
                collect_provider_output_content_text(item, None, text);
            }
        }
        JsonValue::Object(map) => {
            for (item_key, item_value) in map {
                collect_provider_output_content_text(item_value, Some(item_key), text);
            }
        }
        _ => {}
    }
}

fn is_provider_output_safety_note_key(key: &str) -> bool {
    matches!(
        key,
        "risk_checks"
            | "review_points"
            | "editor_notes"
            | "review_notes"
            | "safety_notes"
            | "privacy_audit"
            | "provider_usage"
    )
}

fn contains_output_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|keyword| text.contains(keyword))
}

fn contains_output_email(text: &str) -> bool {
    text.split(|ch: char| {
        !(ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '%' | '+' | '-' | '@'))
    })
    .any(|candidate| {
        let Some((local, domain)) = candidate.split_once('@') else {
            return false;
        };
        !local.is_empty()
            && !domain.is_empty()
            && domain.contains('.')
            && local
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '%' | '+' | '-'))
            && domain
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-'))
    })
}

fn contains_output_chinese_mobile(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '1' && (index == 0 || !chars[index - 1].is_ascii_digit()) {
            let mut digits = String::new();
            let mut cursor = index;
            while cursor < chars.len() && digits.len() < 11 {
                let ch = chars[cursor];
                if ch.is_ascii_digit() {
                    digits.push(ch);
                } else if ch == ' ' || ch == '-' {
                } else {
                    break;
                }
                cursor += 1;
            }
            if digits.len() == 11
                && (cursor == chars.len() || !chars[cursor].is_ascii_digit())
                && matches!(digits.as_bytes()[1] as char, '3'..='9')
            {
                return true;
            }
        }
        index += 1;
    }
    false
}

fn contains_output_id_card(text: &str) -> bool {
    text.split_whitespace().any(|token| {
        let value = token.trim_matches(|ch: char| {
            ch.is_ascii_punctuation() || "，。；、：（）《》【】“”‘’".contains(ch)
        });
        value.len() == 18
            && value
                .chars()
                .enumerate()
                .all(|(index, ch)| ch.is_ascii_digit() || (index == 17 && matches!(ch, 'x' | 'X')))
    })
}
