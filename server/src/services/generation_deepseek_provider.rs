use std::time::Duration;

use serde_json::{Value as JsonValue, json};

use crate::services::{
    generation_mock_provider::MockGenerationProvider,
    generation_output_validator::normalize_provider_output,
    generation_privacy::{provider_input_privacy_audit, sanitize_provider_input},
    generation_provider_config::{env_non_empty, env_u64, first_non_empty_env, truncate},
    generation_provider_contract::{
        AiGenerationProvider, GenerationProviderComponent, GenerationProviderError,
        GenerationRequest, ImageGenerationRequest,
    },
};

const TEXT_JOB_TYPES: &[&str] = &[
    "storybook_plan",
    "storybook_roles",
    "storybook_pages",
    "customization_plan",
];

pub const SUPPORTED_TEXT_JOB_TYPES: &[&str] = TEXT_JOB_TYPES;
pub const DEFAULT_TEXT_SCHEMA_VERSION: &str = "generation.provider.v1";

pub struct DeepSeekTextProvider {
    pub(crate) api_key: Option<String>,
    pub(crate) base_url: String,
    pub(crate) endpoint_path: String,
    pub(crate) model: String,
    pub(crate) timeout_seconds: u64,
    pub(crate) max_tokens: u64,
}

impl DeepSeekTextProvider {
    pub(crate) fn from_env() -> Self {
        Self {
            api_key: env_non_empty("DEEPSEEK_API_KEY"),
            base_url: first_non_empty_env(&["DEEPSEEK_BASE_URL"], "https://api.deepseek.com"),
            endpoint_path: first_non_empty_env(&["DEEPSEEK_ENDPOINT_PATH"], "/chat/completions"),
            model: first_non_empty_env(&["DEEPSEEK_MODEL"], "deepseek-v4-flash"),
            timeout_seconds: env_u64("DEEPSEEK_TIMEOUT_SECONDS", 45),
            max_tokens: env_u64("DEEPSEEK_MAX_TOKENS", 4096),
        }
    }

    pub(crate) fn build_prompt(
        &self,
        request: &GenerationRequest<'_>,
    ) -> Result<JsonValue, GenerationProviderError> {
        if !TEXT_JOB_TYPES.contains(&request.job_type) {
            return Err(GenerationProviderError::new(format!(
                "{} 只支持文本生成任务，收到 {}",
                self.name(),
                request.job_type
            )));
        }
        let sanitized_input = sanitize_provider_input(request.input);

        Ok(json!({
            "provider": self.name(),
            "base_url": self.base_url,
            "model": self.model,
            "job_type": request.job_type,
            "response_schema": response_schema_for(request.job_type),
            "input": sanitized_input,
            "system_prompt": "你是幼儿园教育绘本创作助手。输出必须是 JSON，语言适合 3-6 岁儿童共读，避免记录或编造儿童敏感隐私。",
            "user_prompt": prompt_for(request.job_type)
        }))
    }

    pub(crate) fn build_chat_payload(
        &self,
        request: &GenerationRequest<'_>,
    ) -> Result<JsonValue, GenerationProviderError> {
        let prompt = self.build_prompt(request)?;
        Ok(json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": prompt["system_prompt"]
                },
                {
                    "role": "user",
                    "content": format!(
                        "{}\n\n请只返回一个合法 JSON 对象，不要 Markdown，不要代码块。\n期望 JSON 结构示例：\n{}\n\n输入：\n{}",
                        prompt["user_prompt"].as_str().unwrap_or("请生成结构化绘本内容。"),
                        response_schema_for(request.job_type),
                        prompt["input"]
                    )
                }
            ],
            "response_format": {"type": "json_object"},
            "temperature": 0.7,
            "max_tokens": self.max_tokens,
            "stream": false
        }))
    }

    pub(crate) fn endpoint(&self) -> String {
        format_deepseek_endpoint(&self.base_url, &self.endpoint_path)
    }

    pub(crate) fn summary_component(&self) -> GenerationProviderComponent {
        let configured = self.api_key.is_some();
        GenerationProviderComponent {
            kind: "text".to_string(),
            provider: self.name().to_string(),
            configured,
            ready: configured,
            model: self.model.clone(),
            endpoint: self.endpoint(),
            supports: SUPPORTED_TEXT_JOB_TYPES
                .iter()
                .map(|item| item.to_string())
                .collect(),
            required_configuration: if configured {
                vec![]
            } else {
                vec!["DEEPSEEK_API_KEY".to_string()]
            },
        }
    }
}

impl AiGenerationProvider for DeepSeekTextProvider {
    fn name(&self) -> &'static str {
        "deepseek"
    }

    async fn generate(
        &self,
        request: GenerationRequest<'_>,
    ) -> Result<JsonValue, GenerationProviderError> {
        let Some(api_key) = &self.api_key else {
            return Err(GenerationProviderError::new(
                "KINDLEAF_GENERATION_PROVIDER=deepseek 时必须配置 DEEPSEEK_API_KEY",
            ));
        };

        let privacy_audit = provider_input_privacy_audit(request.input);
        let payload = self.build_chat_payload(&request)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(self.timeout_seconds))
            .build()
            .map_err(|err| {
                GenerationProviderError::new(format!("创建 DeepSeek 客户端失败：{err}"))
            })?;
        let response = client
            .post(self.endpoint())
            .bearer_auth(api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|err| {
                GenerationProviderError::retryable(format!("DeepSeek 请求失败：{err}"))
            })?;

        let status = response.status();
        let body = response.text().await.map_err(|err| {
            GenerationProviderError::new(format!("读取 DeepSeek 响应失败：{err}"))
        })?;

        if !status.is_success() {
            return Err(GenerationProviderError::retryable(format!(
                "DeepSeek 请求返回 {status}：{}",
                truncate(&body, 240)
            )));
        }

        let response_json: JsonValue = serde_json::from_str(&body).map_err(|err| {
            GenerationProviderError::new(format!("DeepSeek 响应不是合法 JSON：{err}"))
        })?;
        let content = response_json["choices"]
            .as_array()
            .and_then(|choices| choices.first())
            .and_then(|choice| choice["message"]["content"].as_str())
            .ok_or_else(|| {
                GenerationProviderError::new("DeepSeek 响应缺少 choices[0].message.content")
            })?;
        let output = serde_json::from_str(content).map_err(|err| {
            GenerationProviderError::new(format!(
                "DeepSeek content 不是合法 JSON：{}；content={}",
                err,
                truncate(content, 240)
            ))
        })?;

        let normalized = normalize_provider_output(
            output,
            self.name(),
            request.job_type,
            response_json.get("usage").cloned(),
            Some(privacy_audit),
        )?;
        validate_output_against_input(&normalized, request.input, request.job_type)?;
        Ok(normalized)
    }

    async fn generate_image(
        &self,
        request: ImageGenerationRequest<'_>,
    ) -> Result<JsonValue, GenerationProviderError> {
        MockGenerationProvider.generate_image(request).await
    }
}

pub(crate) fn validate_output_against_input(
    output: &JsonValue,
    input: &JsonValue,
    job_type: &str,
) -> Result<(), GenerationProviderError> {
    if job_type != "storybook_pages" {
        return Ok(());
    }
    let confirmed_roles = input
        .get("confirmed_roles")
        .and_then(|value| value.as_array())
        .map(|roles| {
            roles
                .iter()
                .filter_map(|role| role.get("name").and_then(|name| name.as_str()))
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if confirmed_roles.is_empty() {
        return Ok(());
    }

    let pages = output
        .get("pages")
        .and_then(|value| value.as_array())
        .ok_or_else(|| {
            GenerationProviderError::new("provider 输出 storybook_pages.pages 必须是 array")
        })?;
    for (index, page) in pages.iter().enumerate() {
        let title = page
            .get("title")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let body = page
            .get("body")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let prompt = page
            .get("illustration_prompt")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let combined = format!("{title} {body} {prompt}");
        if !confirmed_roles.iter().any(|name| combined.contains(name)) {
            return Err(GenerationProviderError::new(format!(
                "provider 输出 storybook_pages.pages[{index}] 未引用已确认角色：{}",
                confirmed_roles.join("、")
            )));
        }
        if !confirmed_roles.iter().any(|name| prompt.contains(name)) {
            return Err(GenerationProviderError::new(format!(
                "provider 输出 storybook_pages.pages[{index}].illustration_prompt 未包含已确认角色姓名"
            )));
        }
        let unexpected_animals = [
            "小象",
            "小兔",
            "小猴",
            "小熊",
            "小猫",
            "小狗",
            "小狐狸",
            "小鹿",
        ];
        let role_text = input
            .get("confirmed_roles")
            .map(JsonValue::to_string)
            .unwrap_or_default();
        if let Some(animal) = unexpected_animals
            .iter()
            .find(|animal| combined.contains(**animal) && !role_text.contains(**animal))
        {
            return Err(GenerationProviderError::new(format!(
                "provider 输出 storybook_pages.pages[{index}] 出现未确认替代角色：{animal}"
            )));
        }
    }
    Ok(())
}

pub(crate) fn format_deepseek_endpoint(base_url: &str, endpoint_path: &str) -> String {
    let trimmed_base = base_url.trim_end_matches('/');
    let trimmed_path = endpoint_path.trim();
    if trimmed_path.is_empty() {
        return format!("{trimmed_base}/chat/completions");
    }
    if trimmed_path.starts_with("http://") || trimmed_path.starts_with("https://") {
        return trimmed_path.to_string();
    }
    format!("{trimmed_base}/{}", trimmed_path.trim_start_matches('/'))
}

pub(crate) fn prompt_for(job_type: &str) -> &'static str {
    match job_type {
        "storybook_plan" => {
            "根据 input.title、input.theme、input.use_scene、input.style 生成普通绘本方案。故事主线必须围绕输入标题和主题展开：如果标题或主题是具体场景，如丛林、海边、厨房、午睡、入园等，summary、outline、role_requirements 必须反复体现该场景和主题，不得沿用无关的玩具轮流、小火车分享等通用示例。先给故事主线，再给分页节奏和老师审核点。"
        }
        "storybook_roles" => {
            "根据 input.plan 中已经确认的故事方案生成主角、同伴、老师形象和关键道具设定，必须紧扣 input.title、input.theme、input.plan.summary 和 input.plan.outline，不得沿用无关示例。role_type 只能使用英文枚举 protagonist、supporting、peer、teacher、prop；不要输出中文类型。appearance 只能写稳定可见的视觉特征，例如物种或身份、颜色、服装或材质、体型轮廓、发型/耳朵/配饰、表情和可跨页重复识别的小标记；禁止把动作、习惯、剧情行为或故事任务写入 appearance，例如“喜欢蹦跳”“离开队伍”“带领探险”“制定规则”。这些内容必须写入 story_function。needs_consistency 只给需要跨页重复出现并保持同一形象的主角、老师、重要同伴或反复出现关键道具设为 true；只出现一次的临时事物、背景动物、一次性道具必须设为 false，不需要参考图。"
        }
        "storybook_pages" => {
            "根据已确认方案和角色生成分页图文，每页包含标题、正文和插图提示词。必须严格沿用 input.confirmed_roles 中的角色姓名、身份、外观和关键道具，不得把人类角色改成动物或新增替代主角；插图提示词也必须复述这些角色一致性线索。"
        }
        "customization_plan" => {
            "基于普通绘本和儿童档案生成定制方案，只输出可审核的改写点和风险检查。"
        }
        _ => "生成结构化绘本内容。",
    }
}

pub(crate) fn response_schema_for(job_type: &str) -> JsonValue {
    match job_type {
        "storybook_plan" => json!({
            "schema_version": "generation.provider.v1",
            "provider": "string",
            "mode": "storybook_plan",
            "message": "string",
            "plan": {
                "title": "string",
                "theme": "string",
                "age_group": "string",
                "summary": "string",
                "page_count": "number",
                "outline": [{"page_range": "string", "goal": "string", "beat": "string"}],
                "role_requirements": ["string"],
                "review_points": ["string"]
            }
        }),
        "storybook_roles" => json!({
            "schema_version": "generation.provider.v1",
            "provider": "string",
            "mode": "storybook_roles",
            "message": "string",
            "roles": [{
                "name": "string",
                "role_type": "protagonist | supporting | peer | teacher | prop",
                "appearance": "string",
                "story_function": "string",
                "needs_consistency": "boolean"
            }],
            "consistency_guide": ["string"]
        }),
        "storybook_pages" => json!({
            "schema_version": "generation.provider.v1",
            "provider": "string",
            "mode": "storybook_pages",
            "message": "string",
            "pages": [{
                "page_number": "number",
                "title": "string",
                "body": "string",
                "illustration_prompt": "string",
                "status": "draft"
            }],
            "editor_notes": ["string"]
        }),
        "customization_plan" => json!({
            "schema_version": "generation.provider.v1",
            "provider": "string",
            "mode": "customization_plan",
            "message": "string",
            "customization": {
                "child_id": "string",
                "intensity": "string",
                "strategy": "string",
                "rewrite_points": [{"scope": "string", "action": "string"}],
                "risk_checks": ["string"]
            }
        }),
        _ => json!({}),
    }
}
