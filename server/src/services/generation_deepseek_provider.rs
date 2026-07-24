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

        normalize_provider_output(
            output,
            self.name(),
            request.job_type,
            response_json.get("usage").cloned(),
            Some(privacy_audit),
        )
    }

    async fn generate_image(
        &self,
        request: ImageGenerationRequest<'_>,
    ) -> Result<JsonValue, GenerationProviderError> {
        MockGenerationProvider.generate_image(request).await
    }
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
            "根据教学目标生成普通绘本方案。先给故事主线，再给分页节奏和老师审核点。"
        }
        "storybook_roles" => "根据故事方案生成主角、同伴、老师形象和关键道具设定，强调跨页一致性。",
        "storybook_pages" => "根据已确认方案和角色生成分页图文，每页包含标题、正文和插图提示词。",
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
                "role_type": "string",
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
