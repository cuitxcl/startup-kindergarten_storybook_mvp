#![allow(dead_code)]

use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde_json::{Map as JsonMap, Value as JsonValue, json};

use crate::services::storage;

const TEXT_JOB_TYPES: &[&str] = &[
    "storybook_plan",
    "storybook_roles",
    "storybook_pages",
    "customization_plan",
];

pub const SUPPORTED_TEXT_JOB_TYPES: &[&str] = TEXT_JOB_TYPES;
pub const DEFAULT_TEXT_SCHEMA_VERSION: &str = "generation.provider.v1";
const SUPPORTED_IMAGE_JOB_TYPES: &[&str] =
    &["storybook_page_image", "storybook_role_reference_image"];

pub struct GenerationRequest<'a> {
    pub job_type: &'a str,
    pub input: &'a JsonValue,
}

pub struct ImageGenerationRequest<'a> {
    pub image_id: &'a str,
    pub target_id: &'a str,
    pub target_type: &'a str,
    pub mode: &'a str,
    pub prompt: &'a str,
    pub reference_images: Vec<ImageReference>,
    pub edit_instruction: Option<String>,
    pub image_mode: ImageGenerationMode,
    pub strength: Option<f32>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ImageReference {
    pub url: String,
    pub source: String,
    pub role_id: Option<String>,
    pub label: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageGenerationMode {
    TextToImage,
    ReferenceImage,
    EditImage,
}

impl ImageGenerationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TextToImage => "text_to_image",
            Self::ReferenceImage => "reference_image",
            Self::EditImage => "edit_image",
        }
    }
}

#[derive(Debug)]
pub struct GenerationProviderError {
    pub message: String,
    pub retryable: bool,
}

impl GenerationProviderError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: false,
        }
    }

    fn retryable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
        }
    }

    pub fn safe_message(&self) -> String {
        truncate(&self.message, 240)
    }
}

pub trait AiGenerationProvider {
    fn name(&self) -> &'static str;
    async fn generate(
        &self,
        request: GenerationRequest<'_>,
    ) -> Result<JsonValue, GenerationProviderError>;
    async fn generate_image(
        &self,
        request: ImageGenerationRequest<'_>,
    ) -> Result<JsonValue, GenerationProviderError>;
}

pub enum ConfiguredGenerationProvider {
    Mock(MockGenerationProvider),
    DeepSeek(DeepSeekTextProvider),
    Seedream(SeedreamImageProvider),
    Composite {
        text: DeepSeekTextProvider,
        image: SeedreamImageProvider,
    },
}

impl ConfiguredGenerationProvider {
    pub fn from_env() -> Self {
        let provider = std::env::var("KINDLEAF_GENERATION_PROVIDER")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        match provider.as_str() {
            "mock" => Self::Mock(MockGenerationProvider),
            "deepseek" => {
                let text = DeepSeekTextProvider::from_env();
                if text.api_key.is_some() {
                    Self::DeepSeek(text)
                } else {
                    Self::Mock(MockGenerationProvider)
                }
            }
            "seedream" => {
                let image = SeedreamImageProvider::from_env();
                if image.api_key.is_some() {
                    Self::Seedream(image)
                } else {
                    Self::Mock(MockGenerationProvider)
                }
            }
            "" => {
                let text = DeepSeekTextProvider::from_env();
                let image = SeedreamImageProvider::from_env();
                match (text.api_key.is_some(), image.api_key.is_some()) {
                    (true, true) => Self::Composite { text, image },
                    (true, false) => Self::DeepSeek(text),
                    (false, true) => Self::Seedream(image),
                    (false, false) => Self::Mock(MockGenerationProvider),
                }
            }
            _ => {
                let text = DeepSeekTextProvider::from_env();
                let image = SeedreamImageProvider::from_env();
                match (text.api_key.is_some(), image.api_key.is_some()) {
                    (true, true) => Self::Composite { text, image },
                    (true, false) => Self::DeepSeek(text),
                    (false, true) => Self::Seedream(image),
                    (false, false) => Self::Mock(MockGenerationProvider),
                }
            }
        }
    }

    pub fn raw_provider_mode() -> String {
        std::env::var("KINDLEAF_GENERATION_PROVIDER")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
    }

    pub fn ready_for_text() -> bool {
        matches!(Self::raw_provider_mode().as_str(), "deepseek" | "")
            && env_non_empty("DEEPSEEK_API_KEY").is_some()
    }

    pub async fn generate(
        &self,
        request: GenerationRequest<'_>,
    ) -> Result<JsonValue, GenerationProviderError> {
        match self {
            Self::Mock(provider) => provider.generate(request).await,
            Self::DeepSeek(provider) => provider.generate(request).await,
            Self::Seedream(provider) => provider.generate(request).await,
            Self::Composite { text, .. } => text.generate(request).await,
        }
    }

    pub async fn generate_image(
        &self,
        request: ImageGenerationRequest<'_>,
    ) -> Result<JsonValue, GenerationProviderError> {
        match self {
            Self::Mock(provider) => provider.generate_image(request).await,
            Self::DeepSeek(provider) => provider.generate_image(request).await,
            Self::Seedream(provider) => provider.generate_image(request).await,
            Self::Composite { image, .. } => image.generate_image(request).await,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Mock(provider) => provider.name(),
            Self::DeepSeek(provider) => provider.name(),
            Self::Seedream(provider) => provider.name(),
            Self::Composite { .. } => "deepseek+seedream",
        }
    }

    pub fn name_for_job_type(&self, job_type: &str) -> &'static str {
        match self {
            Self::Mock(provider) => provider.name(),
            Self::DeepSeek(provider) => {
                if TEXT_JOB_TYPES.contains(&job_type) {
                    provider.name()
                } else {
                    "mock"
                }
            }
            Self::Seedream(provider) => {
                if SUPPORTED_IMAGE_JOB_TYPES.contains(&job_type) {
                    provider.name()
                } else {
                    "mock"
                }
            }
            Self::Composite { text, image } => {
                if SUPPORTED_IMAGE_JOB_TYPES.contains(&job_type) {
                    image.name()
                } else if TEXT_JOB_TYPES.contains(&job_type) {
                    text.name()
                } else {
                    self.name()
                }
            }
        }
    }

    pub fn summary(&self) -> GenerationProviderSummary {
        let requested_provider = std::env::var("KINDLEAF_GENERATION_PROVIDER")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let deepseek_key_present = env_non_empty("DEEPSEEK_API_KEY").is_some();
        let seedream_key_present = SeedreamImageProvider::api_key_from_env().is_some();

        match self {
            Self::Mock(_) => GenerationProviderSummary {
                provider: "mock".to_string(),
                mode: "demo".to_string(),
                schema_version: "generation.mock.v1".to_string(),
                requires_api_key: false,
                supports_text: SUPPORTED_TEXT_JOB_TYPES
                    .iter()
                    .map(|item| item.to_string())
                    .collect(),
                supports_image: SUPPORTED_IMAGE_JOB_TYPES
                    .iter()
                    .map(|item| item.to_string())
                    .collect(),
                real_text_ready: false,
                real_image_ready: false,
                production_ready: false,
                missing_configuration: missing_generation_configuration(
                    &requested_provider,
                    deepseek_key_present,
                    seedream_key_present,
                ),
                components: generation_provider_components(),
                diagnostic: mock_diagnostic(
                    &requested_provider,
                    deepseek_key_present,
                    seedream_key_present,
                ),
            },
            Self::DeepSeek(provider) => GenerationProviderSummary {
                provider: provider.name().to_string(),
                mode: "text".to_string(),
                schema_version: DEFAULT_TEXT_SCHEMA_VERSION.to_string(),
                requires_api_key: true,
                supports_text: SUPPORTED_TEXT_JOB_TYPES
                    .iter()
                    .map(|item| item.to_string())
                    .collect(),
                supports_image: vec![],
                real_text_ready: true,
                real_image_ready: false,
                production_ready: false,
                missing_configuration: vec![],
                components: generation_provider_components(),
                diagnostic: "文本生成已接入真实 provider，插图仍使用 mock".to_string(),
            },
            Self::Seedream(provider) => GenerationProviderSummary {
                provider: provider.name().to_string(),
                mode: "image".to_string(),
                schema_version: DEFAULT_TEXT_SCHEMA_VERSION.to_string(),
                requires_api_key: true,
                supports_text: vec![],
                supports_image: SUPPORTED_IMAGE_JOB_TYPES
                    .iter()
                    .map(|item| item.to_string())
                    .collect(),
                real_text_ready: false,
                real_image_ready: true,
                production_ready: false,
                missing_configuration: vec![],
                components: generation_provider_components(),
                diagnostic: "插图生成已接入 Seedream，文本仍使用 mock".to_string(),
            },
            Self::Composite { text, image } => GenerationProviderSummary {
                provider: format!("{}+{}", text.name(), image.name()),
                mode: "composite".to_string(),
                schema_version: DEFAULT_TEXT_SCHEMA_VERSION.to_string(),
                requires_api_key: true,
                supports_text: SUPPORTED_TEXT_JOB_TYPES
                    .iter()
                    .map(|item| item.to_string())
                    .collect(),
                supports_image: SUPPORTED_IMAGE_JOB_TYPES
                    .iter()
                    .map(|item| item.to_string())
                    .collect(),
                real_text_ready: true,
                real_image_ready: true,
                production_ready: true,
                missing_configuration: vec![],
                components: generation_provider_components(),
                diagnostic: "文本和插图均已接入真实 provider".to_string(),
            },
        }
    }
}

fn missing_generation_configuration(
    requested_provider: &str,
    deepseek_key_present: bool,
    seedream_key_present: bool,
) -> Vec<String> {
    match requested_provider {
        "deepseek" if !deepseek_key_present => vec!["DEEPSEEK_API_KEY".to_string()],
        "seedream" if !seedream_key_present => {
            vec!["SEEDREAM_API_KEY 或 ARK_API_KEY".to_string()]
        }
        "" if !deepseek_key_present && !seedream_key_present => {
            vec![
                "DEEPSEEK_API_KEY".to_string(),
                "SEEDREAM_API_KEY 或 ARK_API_KEY".to_string(),
            ]
        }
        _ => Vec::new(),
    }
}

fn mock_diagnostic(
    requested_provider: &str,
    deepseek_key_present: bool,
    seedream_key_present: bool,
) -> String {
    match requested_provider {
        "mock" => "当前使用 demo mock，未接入真实 AI provider".to_string(),
        "deepseek" if !deepseek_key_present => {
            "已请求 deepseek，但缺少 DEEPSEEK_API_KEY，已回退到 mock".to_string()
        }
        "seedream" if !seedream_key_present => {
            "已请求 seedream，但缺少 SEEDREAM_API_KEY 或 ARK_API_KEY，已回退到 mock".to_string()
        }
        "" if !deepseek_key_present && !seedream_key_present => {
            "未配置 DeepSeek / Seedream，已回退到 mock".to_string()
        }
        _ => "当前使用 mock 作为兜底执行器".to_string(),
    }
}

fn generation_provider_components() -> Vec<GenerationProviderComponent> {
    vec![
        DeepSeekTextProvider::from_env().summary_component(),
        SeedreamImageProvider::from_env().summary_component(),
    ]
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GenerationProviderSummary {
    pub provider: String,
    pub mode: String,
    pub schema_version: String,
    pub requires_api_key: bool,
    pub supports_text: Vec<String>,
    pub supports_image: Vec<String>,
    pub real_text_ready: bool,
    pub real_image_ready: bool,
    pub production_ready: bool,
    pub missing_configuration: Vec<String>,
    pub components: Vec<GenerationProviderComponent>,
    pub diagnostic: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GenerationProviderComponent {
    pub kind: String,
    pub provider: String,
    pub configured: bool,
    pub ready: bool,
    pub model: String,
    pub endpoint: String,
    pub supports: Vec<String>,
    pub required_configuration: Vec<String>,
}

pub struct SeedreamImageProvider {
    api_key: Option<String>,
    base_url: String,
    endpoint_path: String,
    model: String,
    size: String,
    timeout_seconds: u64,
}

impl SeedreamImageProvider {
    fn from_env() -> Self {
        Self {
            api_key: Self::api_key_from_env(),
            base_url: first_non_empty_env(
                &["SEEDREAM_BASE_URL", "ARK_BASE_URL"],
                "https://ark.cn-beijing.volces.com",
            ),
            endpoint_path: first_non_empty_env(
                &["SEEDREAM_ENDPOINT_PATH", "ARK_IMAGE_ENDPOINT_PATH"],
                "/api/v3/images/generations",
            ),
            model: first_non_empty_env(
                &["SEEDREAM_IMAGE_MODEL", "ARK_IMAGE_MODEL"],
                "doubao-seedream-5-0-lite",
            ),
            size: first_non_empty_env(&["SEEDREAM_IMAGE_SIZE"], "1024x1024"),
            timeout_seconds: env_u64("SEEDREAM_TIMEOUT_SECONDS", 120),
        }
    }

    fn api_key_from_env() -> Option<String> {
        env_non_empty("SEEDREAM_API_KEY").or_else(|| env_non_empty("ARK_API_KEY"))
    }

    fn endpoint(&self) -> String {
        format_seedream_endpoint(&self.base_url, &self.endpoint_path)
    }

    fn summary_component(&self) -> GenerationProviderComponent {
        let configured = self.api_key.is_some();
        GenerationProviderComponent {
            kind: "image".to_string(),
            provider: self.name().to_string(),
            configured,
            ready: configured,
            model: self.model.clone(),
            endpoint: self.endpoint(),
            supports: SUPPORTED_IMAGE_JOB_TYPES
                .iter()
                .map(|item| item.to_string())
                .collect(),
            required_configuration: if configured {
                vec![]
            } else {
                vec!["SEEDREAM_API_KEY 或 ARK_API_KEY".to_string()]
            },
        }
    }
}

impl AiGenerationProvider for SeedreamImageProvider {
    fn name(&self) -> &'static str {
        "seedream"
    }

    async fn generate(
        &self,
        request: GenerationRequest<'_>,
    ) -> Result<JsonValue, GenerationProviderError> {
        MockGenerationProvider.generate(request).await
    }

    async fn generate_image(
        &self,
        request: ImageGenerationRequest<'_>,
    ) -> Result<JsonValue, GenerationProviderError> {
        let Some(api_key) = &self.api_key else {
            return Err(GenerationProviderError::new(
                "KINDLEAF_GENERATION_PROVIDER=seedream 时必须配置 SEEDREAM_API_KEY 或 ARK_API_KEY",
            ));
        };

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(self.timeout_seconds))
            .build()
            .map_err(|err| {
                GenerationProviderError::new(format!("创建 Seedream 客户端失败：{err}"))
            })?;
        let (sanitized_prompt, redaction_labels) = sanitize_image_prompt_with_audit(request.prompt);
        let mut payload = json!({
            "model": self.model,
            "prompt": sanitized_prompt,
            "size": self.size,
            "response_format": "b64_json",
            "watermark": false,
            "image_mode": request.image_mode.as_str(),
        });
        if !request.reference_images.is_empty() {
            payload["image"] = json!(
                request
                    .reference_images
                    .iter()
                    .map(|item| item.url.clone())
                    .collect::<Vec<_>>()
            );
            payload["reference_images"] = json!(request.reference_images);
        }
        if let Some(edit_instruction) = request
            .edit_instruction
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            payload["edit_instruction"] = json!(edit_instruction);
        }
        if let Some(strength) = request.strength {
            payload["strength"] = json!(strength.clamp(0.0, 1.0));
        }
        let response = client
            .post(self.endpoint())
            .bearer_auth(api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|err| {
                GenerationProviderError::retryable(format!("Seedream 图片请求失败：{err}"))
            })?;

        let status = response.status();
        let body = response.text().await.map_err(|err| {
            GenerationProviderError::new(format!("读取 Seedream 图片响应失败：{err}"))
        })?;

        if !status.is_success() {
            return Err(GenerationProviderError::retryable(format!(
                "Seedream 图片请求返回 {status}：{}",
                truncate(&body, 240)
            )));
        }

        let response_json: JsonValue = serde_json::from_str(&body).map_err(|err| {
            GenerationProviderError::new(format!("Seedream 图片响应不是合法 JSON：{err}"))
        })?;
        let image_url =
            image_response_to_image_url(&client, request.image_id, response_json, self.name())
                .await?;

        Ok(json!({
            "schema_version": "generation.provider.v1",
            "provider": self.name(),
            "mode": request.mode,
            "message": "插图任务已完成",
            "image": {
                "target_id": request.target_id,
                "target_type": request.target_type,
                "page_id": if request.target_type == "page" { request.target_id } else { "" },
                "role_id": if request.target_type == "role" { request.target_id } else { "" },
                "image_url": image_url,
                "alt_text": "AI 生成的幼儿园绘本插图",
                "prompt": sanitized_prompt,
                "image_mode": request.image_mode.as_str(),
                "reference_images": request.reference_images,
                "edit_instruction": request.edit_instruction,
                "strength": request.strength,
                "privacy_audit": {
                    "redacted": !redaction_labels.is_empty(),
                    "labels": redaction_labels
                },
                "style_notes": ["Seedream 生成", "儿童绘本", "角色一致"]
            }
        }))
    }
}

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

pub struct DeepSeekTextProvider {
    api_key: Option<String>,
    base_url: String,
    endpoint_path: String,
    model: String,
    timeout_seconds: u64,
    max_tokens: u64,
}

impl DeepSeekTextProvider {
    fn from_env() -> Self {
        Self {
            api_key: env_non_empty("DEEPSEEK_API_KEY"),
            base_url: first_non_empty_env(&["DEEPSEEK_BASE_URL"], "https://api.deepseek.com"),
            endpoint_path: first_non_empty_env(&["DEEPSEEK_ENDPOINT_PATH"], "/chat/completions"),
            model: first_non_empty_env(&["DEEPSEEK_MODEL"], "deepseek-v4-flash"),
            timeout_seconds: env_u64("DEEPSEEK_TIMEOUT_SECONDS", 45),
            max_tokens: env_u64("DEEPSEEK_MAX_TOKENS", 4096),
        }
    }

    fn build_prompt(
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

    fn build_chat_payload(
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

    fn endpoint(&self) -> String {
        format_deepseek_endpoint(&self.base_url, &self.endpoint_path)
    }

    fn summary_component(&self) -> GenerationProviderComponent {
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

fn sanitize_provider_input(input: &JsonValue) -> JsonValue {
    sanitize_provider_value(input, None)
}

fn provider_input_privacy_audit(input: &JsonValue) -> JsonValue {
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

fn sanitize_image_prompt(prompt: &str) -> String {
    sanitize_image_prompt_with_audit(prompt).0
}

fn sanitize_image_prompt_with_audit(prompt: &str) -> (String, Vec<&'static str>) {
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

fn env_non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn first_non_empty_env(keys: &[&str], fallback: &str) -> String {
    first_non_empty_value(keys.iter().map(|key| env_non_empty(key)), fallback)
}

fn first_non_empty_value<I>(values: I, fallback: &str) -> String
where
    I: IntoIterator<Item = Option<String>>,
{
    values
        .into_iter()
        .flatten()
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn env_u64(key: &str, fallback: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut truncated = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        truncated.push_str("...");
    }
    truncated
}

const TRANSPARENT_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEklEQVR4nGP4cGnfsxNbGCAUAEWMCcWN1afmAAAAAElFTkSuQmCC";

fn write_generated_image(
    image_id: &str,
    image_b64: &str,
    provider: &str,
) -> Result<String, GenerationProviderError> {
    let bytes = BASE64_STANDARD
        .decode(image_b64.trim())
        .map_err(|err| GenerationProviderError::new(format!("解码图片内容失败：{err}")))?;
    validate_png_image_bytes(&bytes)?;
    let file_name = generated_image_file_name(image_id, provider);
    storage::save_generated_image(&file_name, &bytes).map_err(GenerationProviderError::new)
}

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

async fn image_response_to_image_url(
    client: &reqwest::Client,
    image_id: &str,
    response_json: JsonValue,
    provider: &str,
) -> Result<String, GenerationProviderError> {
    let image_item = response_json["data"]
        .as_array()
        .and_then(|items| items.first());

    if let Some(image_b64) = image_item.and_then(extract_image_base64) {
        return write_generated_image(image_id, image_b64, provider);
    }

    if let Some(image_url) = image_item.and_then(extract_image_url) {
        return fetch_remote_image(client, image_id, image_url, provider).await;
    }

    Err(GenerationProviderError::new(
        "Seedream 图片响应缺少 data[0].b64_json/image_base64 或 data[0].url/image_url",
    ))
}

fn extract_image_base64(item: &JsonValue) -> Option<&str> {
    item["b64_json"]
        .as_str()
        .or_else(|| item["image_base64"].as_str())
}

fn extract_image_url(item: &JsonValue) -> Option<&str> {
    item["url"].as_str().or_else(|| item["image_url"].as_str())
}

fn format_deepseek_endpoint(base_url: &str, endpoint_path: &str) -> String {
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

fn format_seedream_endpoint(base_url: &str, endpoint_path: &str) -> String {
    let trimmed_base = base_url.trim_end_matches('/');
    let trimmed_path = endpoint_path.trim();
    if trimmed_path.is_empty() {
        return format!("{trimmed_base}/api/v3/images/generations");
    }
    if trimmed_path.starts_with("http://") || trimmed_path.starts_with("https://") {
        return trimmed_path.to_string();
    }
    format!("{trimmed_base}/{}", trimmed_path.trim_start_matches('/'))
}

async fn fetch_remote_image(
    client: &reqwest::Client,
    image_id: &str,
    image_url: &str,
    provider: &str,
) -> Result<String, GenerationProviderError> {
    let response = client.get(image_url).send().await.map_err(|err| {
        GenerationProviderError::retryable(format!("下载 Seedream 图片失败：{err}"))
    })?;

    let status = response.status();
    let bytes = response.bytes().await.map_err(|err| {
        GenerationProviderError::new(format!("读取 Seedream 图片字节失败：{err}"))
    })?;

    if !status.is_success() {
        return Err(GenerationProviderError::retryable(format!(
            "下载 Seedream 图片返回 {status}"
        )));
    }
    validate_png_image_bytes(&bytes)?;

    let file_name = generated_image_file_name(image_id, provider);
    storage::save_generated_image(&file_name, &bytes).map_err(GenerationProviderError::new)
}

fn validate_png_image_bytes(bytes: &[u8]) -> Result<(), GenerationProviderError> {
    const PNG_SIGNATURE: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if bytes.starts_with(PNG_SIGNATURE) {
        return Ok(());
    }
    Err(GenerationProviderError::new(
        "Seedream 图片内容不是合法 PNG 文件",
    ))
}

fn generated_image_file_name(image_id: &str, provider: &str) -> String {
    let image_id = image_id.replace(['/', '\\'], "_");
    let provider = provider.replace(['/', '\\'], "_");
    format!("{provider}-{image_id}.png")
}

fn normalize_provider_output(
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

fn validate_provider_output_content_safety(
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

fn prompt_for(job_type: &str) -> &'static str {
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

fn response_schema_for(job_type: &str) -> JsonValue {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use uuid::Uuid;

    #[tokio::test]
    async fn mock_provider_generates_structured_storybook_plan() {
        let provider = ConfiguredGenerationProvider::Mock(MockGenerationProvider);
        let output = provider
            .generate(GenerationRequest {
                job_type: "storybook_plan",
                input: &json!({"theme": "排队洗手"}),
            })
            .await
            .expect("mock plan should be generated");

        assert_eq!(output["schema_version"], "generation.mock.v1");
        assert_eq!(output["provider"], "mock");
        assert!(
            output["plan"]["outline"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        );
    }

    #[tokio::test]
    async fn mock_provider_generates_structured_image_result() {
        let provider = ConfiguredGenerationProvider::Mock(MockGenerationProvider);
        let image_id = Uuid::new_v4().to_string();
        let output = provider
            .generate_image(ImageGenerationRequest {
                image_id: &image_id,
                target_id: "page-1",
                target_type: "page",
                mode: "storybook_page_image",
                prompt: "明亮教室",
                reference_images: vec![],
                edit_instruction: None,
                image_mode: ImageGenerationMode::TextToImage,
                strength: None,
            })
            .await
            .expect("mock image should be generated");

        assert_eq!(
            output["image"]["image_url"],
            format!("/generated-images/mock-{image_id}.png")
        );
        assert_eq!(output["image"]["page_id"], "page-1");
        assert_eq!(output["image"]["prompt"], "明亮教室");
    }

    #[test]
    fn mock_summary_reports_not_production_ready() {
        let provider = ConfiguredGenerationProvider::Mock(MockGenerationProvider);
        let summary = provider.summary();

        assert_eq!(summary.provider, "mock");
        assert!(!summary.real_text_ready);
        assert!(!summary.real_image_ready);
        assert!(!summary.production_ready);
        assert!(!summary.diagnostic.is_empty());
    }

    #[test]
    fn provider_config_uses_first_non_empty_value() {
        assert_eq!(
            first_non_empty_value(
                [
                    None,
                    Some("".trim().to_string()),
                    Some("doubao-seedream-5-0-lite".to_string())
                ],
                "fallback-model"
            ),
            "doubao-seedream-5-0-lite"
        );
        assert_eq!(
            first_non_empty_value([None, Some("  ".trim().to_string())], "fallback-model"),
            "fallback-model"
        );
    }

    #[test]
    fn composite_provider_names_match_job_type() {
        let provider = ConfiguredGenerationProvider::Composite {
            text: DeepSeekTextProvider {
                api_key: Some("test-key".to_string()),
                base_url: "https://api.deepseek.com".to_string(),
                endpoint_path: "/chat/completions".to_string(),
                model: "deepseek-v4-flash".to_string(),
                timeout_seconds: 45,
                max_tokens: 4096,
            },
            image: SeedreamImageProvider {
                api_key: Some("test-key".to_string()),
                base_url: "https://ark.cn-beijing.volces.com".to_string(),
                endpoint_path: "/api/v3/images/generations".to_string(),
                model: "doubao-seedream-5-0-lite".to_string(),
                size: "1024x1024".to_string(),
                timeout_seconds: 45,
            },
        };

        assert_eq!(provider.name(), "deepseek+seedream");
        assert_eq!(provider.name_for_job_type("storybook_plan"), "deepseek");
        assert_eq!(provider.name_for_job_type("customization_plan"), "deepseek");
        assert_eq!(
            provider.name_for_job_type("storybook_page_image"),
            "seedream"
        );
    }

    #[test]
    fn deepseek_summary_reports_text_ready_only() {
        let provider = ConfiguredGenerationProvider::DeepSeek(DeepSeekTextProvider {
            api_key: Some("test-key".to_string()),
            base_url: "https://api.deepseek.com".to_string(),
            endpoint_path: "/chat/completions".to_string(),
            model: "deepseek-v4-flash".to_string(),
            timeout_seconds: 45,
            max_tokens: 4096,
        });
        let summary = provider.summary();

        assert_eq!(summary.provider, "deepseek");
        assert_eq!(summary.mode, "text");
        assert!(summary.real_text_ready);
        assert!(!summary.real_image_ready);
        assert!(!summary.production_ready);
        assert!(
            summary
                .supports_text
                .contains(&"storybook_plan".to_string())
        );
        assert!(summary.supports_image.is_empty());
    }

    #[test]
    fn seedream_summary_reports_image_ready_only() {
        let provider = ConfiguredGenerationProvider::Seedream(SeedreamImageProvider {
            api_key: Some("test-key".to_string()),
            base_url: "https://ark.cn-beijing.volces.com".to_string(),
            endpoint_path: "/api/v3/images/generations".to_string(),
            model: "doubao-seedream-5-0-lite".to_string(),
            size: "1024x1024".to_string(),
            timeout_seconds: 45,
        });
        let summary = provider.summary();

        assert_eq!(summary.provider, "seedream");
        assert_eq!(summary.mode, "image");
        assert!(!summary.real_text_ready);
        assert!(summary.real_image_ready);
        assert!(!summary.production_ready);
        assert!(summary.supports_text.is_empty());
        assert!(
            summary
                .supports_image
                .contains(&"storybook_page_image".to_string())
        );
        let image = summary
            .components
            .iter()
            .find(|item| item.kind == "image")
            .expect("image component should be present");
        assert_eq!(image.provider, "seedream");
        assert!(!image.model.is_empty());
    }

    #[test]
    fn composite_summary_reports_text_and_image_ready() {
        let provider = ConfiguredGenerationProvider::Composite {
            text: DeepSeekTextProvider {
                api_key: Some("test-key".to_string()),
                base_url: "https://api.deepseek.com".to_string(),
                endpoint_path: "/chat/completions".to_string(),
                model: "deepseek-v4-flash".to_string(),
                timeout_seconds: 45,
                max_tokens: 4096,
            },
            image: SeedreamImageProvider {
                api_key: Some("test-key".to_string()),
                base_url: "https://ark.cn-beijing.volces.com".to_string(),
                endpoint_path: "/api/v3/images/generations".to_string(),
                model: "doubao-seedream-5-0-lite".to_string(),
                size: "1024x1024".to_string(),
                timeout_seconds: 45,
            },
        };
        let summary = provider.summary();

        assert_eq!(summary.provider, "deepseek+seedream");
        assert_eq!(summary.mode, "composite");
        assert!(summary.real_text_ready);
        assert!(summary.real_image_ready);
        assert!(summary.production_ready);
        assert!(
            summary
                .supports_text
                .contains(&"storybook_plan".to_string())
        );
        assert!(
            summary
                .supports_image
                .contains(&"storybook_page_image".to_string())
        );
        assert!(
            summary
                .components
                .iter()
                .any(|item| item.kind == "text" && item.provider == "deepseek")
        );
        assert!(
            summary
                .components
                .iter()
                .any(|item| item.kind == "image" && item.provider == "seedream")
        );
    }

    #[tokio::test]
    async fn deepseek_provider_parses_real_http_response() {
        let base_url = spawn_http_server(
            r#"{"choices":[{"message":{"content":"{\"plan\":{\"title\":\"排队洗手\",\"theme\":\"排队洗手\",\"age_group\":\"4-5 岁\",\"summary\":\"孩子们学会排队洗手。\",\"page_count\":6,\"outline\":[{\"page_range\":\"1\",\"goal\":\"进入场景\",\"beat\":\"孩子看到洗手台\"}],\"role_requirements\":[\"主角儿童\"],\"review_points\":[\"教学目标是否准确\"]}}"}}]}"#,
        );
        let provider = DeepSeekTextProvider {
            api_key: Some("test-key".to_string()),
            base_url,
            endpoint_path: "/chat/completions".to_string(),
            model: "deepseek-v4-flash".to_string(),
            timeout_seconds: 45,
            max_tokens: 4096,
        };

        let output = provider
            .generate(GenerationRequest {
                job_type: "storybook_plan",
                input: &json!({"theme": "排队洗手"}),
            })
            .await
            .expect("deepseek response should be parsed");

        assert_eq!(output["provider"], "deepseek");
        assert_eq!(output["mode"], "storybook_plan");
        assert_eq!(output["plan"]["title"], "排队洗手");
    }

    #[tokio::test]
    async fn seedream_provider_parses_real_http_image_response() {
        let body = format!(
            r#"{{"data":[{{"b64_json":"{}"}}]}}"#,
            TRANSPARENT_PNG_BASE64
        );
        let base_url = spawn_http_server(&body);
        let provider = SeedreamImageProvider {
            api_key: Some("test-key".to_string()),
            base_url,
            endpoint_path: "/api/v3/images/generations".to_string(),
            model: "doubao-seedream-5-0-lite".to_string(),
            size: "1024x1024".to_string(),
            timeout_seconds: 45,
        };

        let image_id = Uuid::new_v4().to_string();
        let output = provider
            .generate_image(ImageGenerationRequest {
                image_id: &image_id,
                target_id: "page-1",
                target_type: "page",
                mode: "storybook_page_image",
                prompt: "明亮教室",
                reference_images: vec![],
                edit_instruction: None,
                image_mode: ImageGenerationMode::TextToImage,
                strength: None,
            })
            .await
            .expect("seedream image response should be parsed");

        assert_eq!(output["provider"], "seedream");
        assert_eq!(output["mode"], "storybook_page_image");
        assert_eq!(output["image"]["page_id"], "page-1");
        assert_eq!(output["image"]["prompt"], "明亮教室");
        assert_eq!(output["image"]["privacy_audit"]["redacted"], false);
    }

    #[tokio::test]
    async fn seedream_provider_redacts_private_image_prompt_output() {
        let body = format!(
            r#"{{"data":[{{"b64_json":"{}"}}]}}"#,
            TRANSPARENT_PNG_BASE64
        );
        let base_url = spawn_http_server(&body);
        let provider = SeedreamImageProvider {
            api_key: Some("test-key".to_string()),
            base_url,
            endpoint_path: "/api/v3/images/generations".to_string(),
            model: "doubao-seedream-5-0-lite".to_string(),
            size: "1024x1024".to_string(),
            timeout_seconds: 45,
        };

        let image_id = Uuid::new_v4().to_string();
        let output = provider
            .generate_image(ImageGenerationRequest {
                image_id: &image_id,
                target_id: "page-1",
                target_type: "page",
                mode: "storybook_page_image",
                prompt: "明亮教室，家长电话 138 0013 8000，爸爸近期出差，parent@example.com",
                reference_images: vec![],
                edit_instruction: None,
                image_mode: ImageGenerationMode::TextToImage,
                strength: None,
            })
            .await
            .expect("seedream image response should be parsed");
        let prompt = output["image"]["prompt"]
            .as_str()
            .expect("prompt should be string");

        assert!(prompt.contains("明亮教室"));
        assert!(prompt.contains("[phone_redacted]"));
        assert!(prompt.contains("[email_redacted]"));
        assert!(prompt.contains("[private_detail_redacted]"));
        assert!(!prompt.contains("138 0013 8000"));
        assert!(!prompt.contains("parent@example.com"));
        assert!(!prompt.contains("爸爸"));
        assert_eq!(output["image"]["privacy_audit"]["redacted"], true);
        let labels = output["image"]["privacy_audit"]["labels"]
            .as_array()
            .expect("privacy labels should be array");
        assert!(labels.iter().any(|label| label.as_str() == Some("phone")));
        assert!(labels.iter().any(|label| label.as_str() == Some("email")));
        assert!(
            labels
                .iter()
                .any(|label| label.as_str() == Some("private_detail"))
        );
    }

    #[test]
    fn image_prompt_sanitizer_does_not_treat_long_ids_as_phone_numbers() {
        let prompt = sanitize_image_prompt("UI Smoke 普通绘本 1784538853883 明亮教室");

        assert!(prompt.contains("1784538853883"));
        assert!(!prompt.contains("[phone_redacted]"));
    }

    #[test]
    fn seedream_base64_image_rejects_non_png_bytes() {
        let err = write_generated_image(
            "bad-image-job",
            &BASE64_STANDARD.encode(b"not-a-png"),
            "seedream",
        )
        .expect_err("non-png base64 should fail");

        assert!(!err.retryable);
        assert!(err.safe_message().contains("不是合法 PNG"));
    }

    #[tokio::test]
    async fn seedream_remote_image_rejects_non_png_bytes() {
        let base_url = spawn_http_server("not-a-png");
        let client = reqwest::Client::new();
        let err = fetch_remote_image(&client, "bad-remote-image-job", &base_url, "seedream")
            .await
            .expect_err("non-png remote image should fail");

        assert!(!err.retryable);
        assert!(err.safe_message().contains("不是合法 PNG"));
    }

    #[test]
    fn seedream_image_response_accepts_base64_alias() {
        let item = json!({"image_base64": "abc"});

        assert_eq!(extract_image_base64(&item), Some("abc"));
        assert_eq!(extract_image_url(&item), None);
    }

    #[test]
    fn seedream_image_response_accepts_url_alias() {
        let item = json!({"image_url": "https://example.com/image.png"});

        assert_eq!(
            extract_image_url(&item),
            Some("https://example.com/image.png")
        );
        assert_eq!(extract_image_base64(&item), None);
    }

    #[test]
    fn seedream_endpoint_path_is_configurable() {
        assert_eq!(
            format_seedream_endpoint(
                "https://ark.cn-beijing.volces.com/",
                "/api/v3/images/generations"
            ),
            "https://ark.cn-beijing.volces.com/api/v3/images/generations"
        );
        assert_eq!(
            format_seedream_endpoint(
                "https://ark.cn-beijing.volces.com",
                "api/v1/online/images/generations"
            ),
            "https://ark.cn-beijing.volces.com/api/v1/online/images/generations"
        );
        assert_eq!(
            format_seedream_endpoint("https://ark.cn-beijing.volces.com", ""),
            "https://ark.cn-beijing.volces.com/api/v3/images/generations"
        );
        assert_eq!(
            format_seedream_endpoint(
                "https://ignored.example.com",
                "https://custom.example.com/images"
            ),
            "https://custom.example.com/images"
        );
    }

    #[test]
    fn deepseek_endpoint_path_is_configurable() {
        assert_eq!(
            format_deepseek_endpoint("https://api.deepseek.com/", "/chat/completions"),
            "https://api.deepseek.com/chat/completions"
        );
        assert_eq!(
            format_deepseek_endpoint("https://api.deepseek.com", "v1/chat/completions"),
            "https://api.deepseek.com/v1/chat/completions"
        );
        assert_eq!(
            format_deepseek_endpoint("https://api.deepseek.com", ""),
            "https://api.deepseek.com/chat/completions"
        );
        assert_eq!(
            format_deepseek_endpoint(
                "https://ignored.example.com",
                "https://custom.example.com/chat/completions"
            ),
            "https://custom.example.com/chat/completions"
        );
    }

    #[test]
    fn generated_image_file_name_sanitizes_path_segments() {
        assert_eq!(
            generated_image_file_name("image/1\\a", "seedream"),
            "seedream-image_1_a.png"
        );
    }

    #[test]
    fn deepseek_prompt_contract_names_schema_and_job_type() {
        let provider = DeepSeekTextProvider {
            api_key: Some("test-key".to_string()),
            base_url: "https://api.deepseek.com".to_string(),
            endpoint_path: "/chat/completions".to_string(),
            model: "deepseek-v4-flash".to_string(),
            timeout_seconds: 45,
            max_tokens: 4096,
        };
        let prompt = provider
            .build_prompt(&GenerationRequest {
                job_type: "storybook_pages",
                input: &json!({"page_count": 6}),
            })
            .expect("prompt contract should be built");

        assert_eq!(prompt["provider"], "deepseek");
        assert_eq!(prompt["job_type"], "storybook_pages");
        assert_eq!(prompt["response_schema"]["mode"], "storybook_pages");
    }

    #[test]
    fn deepseek_chat_payload_enables_json_mode() {
        let provider = DeepSeekTextProvider {
            api_key: Some("test-key".to_string()),
            base_url: "https://api.deepseek.com".to_string(),
            endpoint_path: "/chat/completions".to_string(),
            model: "deepseek-v4-flash".to_string(),
            timeout_seconds: 45,
            max_tokens: 4096,
        };
        let payload = provider
            .build_chat_payload(&GenerationRequest {
                job_type: "storybook_plan",
                input: &json!({"theme": "排队洗手"}),
            })
            .expect("payload should be built");

        assert_eq!(payload["model"], "deepseek-v4-flash");
        assert_eq!(payload["response_format"]["type"], "json_object");
        assert!(
            payload["messages"][1]["content"]
                .as_str()
                .is_some_and(
                    |content| content.contains("JSON") && content.contains("storybook_plan")
                )
        );
    }

    #[test]
    fn deepseek_chat_payload_redacts_child_private_fields() {
        let provider = DeepSeekTextProvider {
            api_key: Some("test-key".to_string()),
            base_url: "https://api.deepseek.com".to_string(),
            endpoint_path: "/chat/completions".to_string(),
            model: "deepseek-v4-flash".to_string(),
            timeout_seconds: 45,
            max_tokens: 4096,
        };
        let payload = provider
            .build_chat_payload(&GenerationRequest {
                job_type: "customization_plan",
                input: &json!({
                    "child_id": "10000000-0000-0000-0000-000000000123",
                    "child_nickname": "小雨",
                    "interests": ["贴纸", "小兔"],
                    "focus": "入园适应",
                    "parent_email": "parent@example.com",
                    "guardian_phone": "+86 138 0013 8000",
                    "family_note": "爸爸近期出差"
                }),
            })
            .expect("payload should be built");
        let content = payload["messages"][1]["content"]
            .as_str()
            .expect("user prompt should be text");

        assert!(!content.contains("10000000-0000-0000-0000-000000000123"));
        assert!(!content.contains("小雨"));
        assert!(!content.contains("parent@example.com"));
        assert!(!content.contains("138 0013 8000"));
        assert!(!content.contains("爸爸近期出差"));
        assert!(content.contains("贴纸"));
        assert!(content.contains("入园适应"));
    }

    #[test]
    fn deepseek_input_privacy_audit_reports_redaction_labels_without_raw_values() {
        let audit = provider_input_privacy_audit(&json!({
            "child_id": "10000000-0000-0000-0000-000000000123",
            "child_nickname": "小雨",
            "interests": ["贴纸"],
            "parent_email": "parent@example.com",
            "guardian_phone": "+86 138 0013 8000",
            "family_note": "爸爸近期出差"
        }));

        assert_eq!(audit["redacted"], true);
        let labels = audit["labels"].as_array().expect("labels should be array");
        assert!(
            labels
                .iter()
                .any(|label| label.as_str() == Some("sensitive_field"))
        );
        assert!(!audit.to_string().contains("小雨"));
        assert!(!audit.to_string().contains("parent@example.com"));
        assert!(!audit.to_string().contains("138 0013 8000"));
    }

    #[test]
    fn normalizes_provider_output_metadata() {
        let output = normalize_provider_output(
            valid_plan_output(),
            "deepseek",
            "storybook_plan",
            None,
            None,
        )
        .expect("provider output should normalize");

        assert_eq!(output["schema_version"], "generation.provider.v1");
        assert_eq!(output["provider"], "deepseek");
        assert_eq!(output["mode"], "storybook_plan");
        assert_eq!(output["message"], "生成任务已完成");
    }

    #[test]
    fn normalizes_provider_output_keeps_provider_usage() {
        let output = normalize_provider_output(
            valid_plan_output(),
            "deepseek",
            "storybook_plan",
            Some(json!({
                "prompt_tokens": 120,
                "completion_tokens": 80,
                "total_tokens": 200
            })),
            None,
        )
        .expect("provider output should normalize");

        assert_eq!(output["provider_usage"]["prompt_tokens"], 120);
        assert_eq!(output["provider_usage"]["completion_tokens"], 80);
        assert_eq!(output["provider_usage"]["total_tokens"], 200);
    }

    #[test]
    fn normalizes_provider_output_keeps_privacy_audit() {
        let output = normalize_provider_output(
            valid_plan_output(),
            "deepseek",
            "storybook_plan",
            None,
            Some(json!({
                "redacted": true,
                "labels": ["sensitive_field", "email"]
            })),
        )
        .expect("provider output should normalize");

        assert_eq!(output["privacy_audit"]["redacted"], true);
        assert_eq!(output["privacy_audit"]["labels"][0], "sensitive_field");
    }

    #[test]
    fn provider_output_content_safety_allows_normal_review_language() {
        let output = normalize_provider_output(
            json!({
                "customization": {
                    "strategy": "保留主线，加入孩子兴趣",
                    "rewrite_points": [{"scope": "pages", "action": "替换关键道具"}],
                    "risk_checks": ["不写入家庭住址", "不暴露敏感健康信息", "不改变老师确认过的规则引导目标"]
                }
            }),
            "deepseek",
            "customization_plan",
            None,
            None,
        )
        .expect("normal risk check wording should be allowed");

        assert_eq!(output["mode"], "customization_plan");
    }

    #[test]
    fn provider_output_content_safety_blocks_address_keywords_in_story_content() {
        let err = normalize_provider_output(
            json!({
                "pages": [
                    {
                        "page_number": 1,
                        "title": "放学路上",
                        "body": "老师把家庭住址写进了故事正文。",
                        "illustration_prompt": "幼儿园门口"
                    }
                ]
            }),
            "deepseek",
            "storybook_pages",
            None,
            None,
        )
        .expect_err("address keywords in story content should fail");

        assert!(err.safe_message().contains("住址信息"));
    }

    #[test]
    fn provider_output_content_safety_blocks_contact_details_before_writeback() {
        let err = normalize_provider_output(
            json!({
                "pages": [
                    {
                        "page_number": 1,
                        "title": "老师联系家长",
                        "body": "老师说，家长手机号 138 0013 8000 不应该进入绘本。",
                        "illustration_prompt": "教室里老师和孩子读绘本"
                    }
                ]
            }),
            "deepseek",
            "storybook_pages",
            None,
            None,
        )
        .expect_err("provider output with phone number should fail");

        assert!(!err.retryable);
        assert!(err.safe_message().contains("包含敏感信息"));
        assert!(err.safe_message().contains("手机号"));
    }

    #[test]
    fn provider_output_content_safety_does_not_treat_long_ids_as_phone_numbers() {
        let mut output = valid_plan_output();
        output["plan"]["summary"] = json!("UI Smoke 普通绘本 1784538853883 会学习排队等待。");

        let output = normalize_provider_output(output, "deepseek", "storybook_plan", None, None)
            .expect("long ids should not be treated as phone numbers");

        assert_eq!(output["provider"], "deepseek");
    }

    #[test]
    fn provider_output_requires_plan_shape() {
        let err = normalize_provider_output(
            json!({"message": "缺少 plan"}),
            "deepseek",
            "storybook_plan",
            None,
            None,
        )
        .expect_err("missing plan should fail");

        assert!(!err.retryable);
        assert!(err.safe_message().contains("storybook_plan.plan"));
    }

    #[test]
    fn provider_output_validates_every_plan_outline_item() {
        let err = normalize_provider_output(
            json!({
                "plan": {
                    "title": "排队洗手",
                    "theme": "排队等待",
                    "summary": "孩子学习等待洗手。",
                    "outline": [
                        {"page_range": "1", "goal": "进入场景", "beat": "来到洗手区"},
                        {"page_range": "2", "goal": "理解规则"}
                    ],
                    "role_requirements": ["主角儿童", "老师"],
                    "review_points": ["教学目标准确"]
                }
            }),
            "deepseek",
            "storybook_plan",
            None,
            None,
        )
        .expect_err("missing outline beat should fail");

        assert!(!err.retryable);
        assert!(
            err.safe_message()
                .contains("storybook_plan.outline[1].beat")
        );
    }

    #[test]
    fn provider_output_validates_plan_review_points() {
        let err = normalize_provider_output(
            json!({
                "plan": {
                    "title": "排队洗手",
                    "theme": "排队等待",
                    "summary": "孩子学习等待洗手。",
                    "outline": [{"page_range": "1", "goal": "进入场景", "beat": "来到洗手区"}],
                    "role_requirements": ["主角儿童", "老师"],
                    "review_points": ["教学目标准确", ""]
                }
            }),
            "deepseek",
            "storybook_plan",
            None,
            None,
        )
        .expect_err("empty review point should fail");

        assert!(!err.retryable);
        assert!(
            err.safe_message()
                .contains("storybook_plan.review_points[1]")
        );
    }

    #[test]
    fn provider_output_requires_non_empty_roles() {
        let err = normalize_provider_output(
            json!({"roles": []}),
            "deepseek",
            "storybook_roles",
            None,
            None,
        )
        .expect_err("empty roles should fail");

        assert!(!err.retryable);
        assert!(err.safe_message().contains("storybook_roles.roles"));
    }

    #[test]
    fn provider_output_validates_every_role() {
        let err = normalize_provider_output(
            json!({
                "roles": [
                    {
                        "name": "真真",
                        "role_type": "protagonist",
                        "appearance": "蓝色外套",
                        "story_function": "学习规则"
                    },
                    {
                        "name": "林老师",
                        "role_type": "teacher",
                        "story_function": "给出引导"
                    }
                ]
            }),
            "deepseek",
            "storybook_roles",
            None,
            None,
        )
        .expect_err("missing role appearance should fail");

        assert!(!err.retryable);
        assert!(
            err.safe_message()
                .contains("storybook_roles.roles[1].appearance")
        );
    }

    #[test]
    fn provider_output_requires_page_fields() {
        let err = normalize_provider_output(
            json!({"pages": [{"title": "第 1 页", "body": "缺少插图提示"}]}),
            "deepseek",
            "storybook_pages",
            None,
            None,
        )
        .expect_err("missing illustration prompt should fail");

        assert!(!err.retryable);
        assert!(
            err.safe_message()
                .contains("storybook_pages.pages[0].illustration_prompt")
        );
    }

    #[test]
    fn provider_output_validates_every_page() {
        let err = normalize_provider_output(
            json!({
                "pages": [
                    {
                        "page_number": 1,
                        "title": "排队开始",
                        "body": "孩子们来到洗手区。",
                        "illustration_prompt": "幼儿园洗手区"
                    },
                    {
                        "page_number": 2,
                        "title": "轮到我",
                        "illustration_prompt": "孩子等待洗手"
                    }
                ]
            }),
            "deepseek",
            "storybook_pages",
            None,
            None,
        )
        .expect_err("missing second page body should fail");

        assert!(!err.retryable);
        assert!(err.safe_message().contains("storybook_pages.pages[1].body"));
    }

    #[test]
    fn provider_output_requires_customization_strategy() {
        let err = normalize_provider_output(
            json!({"customization": {"rewrite_points": []}}),
            "deepseek",
            "customization_plan",
            None,
            None,
        )
        .expect_err("missing customization strategy should fail");

        assert!(!err.retryable);
        assert!(err.safe_message().contains("customization_plan.strategy"));
    }

    #[test]
    fn provider_output_requires_customization_rewrite_points() {
        let err = normalize_provider_output(
            json!({
                "customization": {
                    "strategy": "保留主线，加入孩子兴趣",
                    "risk_checks": ["不暴露家庭信息"]
                }
            }),
            "deepseek",
            "customization_plan",
            None,
            None,
        )
        .expect_err("missing rewrite points should fail");

        assert!(!err.retryable);
        assert!(
            err.safe_message()
                .contains("customization_plan.rewrite_points")
        );
    }

    #[test]
    fn provider_output_validates_customization_risk_checks() {
        let err = normalize_provider_output(
            json!({
                "customization": {
                    "strategy": "保留主线，加入孩子兴趣",
                    "rewrite_points": [{"scope": "pages", "action": "替换关键道具"}],
                    "risk_checks": ["不暴露家庭信息", ""]
                }
            }),
            "deepseek",
            "customization_plan",
            None,
            None,
        )
        .expect_err("empty risk check should fail");

        assert!(!err.retryable);
        assert!(
            err.safe_message()
                .contains("customization_plan.risk_checks[1]")
        );
    }

    #[tokio::test]
    async fn deepseek_without_api_key_returns_configuration_error() {
        let provider = DeepSeekTextProvider {
            api_key: None,
            base_url: "https://api.deepseek.com".to_string(),
            endpoint_path: "/chat/completions".to_string(),
            model: "deepseek-v4-flash".to_string(),
            timeout_seconds: 45,
            max_tokens: 4096,
        };
        let err = provider
            .generate(GenerationRequest {
                job_type: "storybook_plan",
                input: &json!({"theme": "排队洗手"}),
            })
            .await
            .expect_err("missing key should fail");

        assert!(!err.retryable);
        assert!(err.safe_message().contains("DEEPSEEK_API_KEY"));
    }

    fn spawn_http_server(body: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test server");
        let addr = listener.local_addr().expect("local addr");
        let body = body.to_string();

        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 4096];
                let _ = stream.read(&mut buffer);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });

        format!("http://{}", addr)
    }

    fn valid_plan_output() -> JsonValue {
        json!({
            "plan": {
                "title": "排队洗手",
                "theme": "排队等待",
                "summary": "孩子们在老师引导下学习排队等待和洗手步骤。",
                "outline": [
                    {
                        "page_range": "1",
                        "goal": "进入场景",
                        "beat": "来到洗手区"
                    }
                ],
                "role_requirements": ["主角儿童", "老师引导者"],
                "review_points": ["教学目标是否准确"]
            }
        })
    }
}
