use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde_json::{Value as JsonValue, json};

use crate::services::{
    generation_mock_provider::MockGenerationProvider,
    generation_privacy::sanitize_image_prompt_with_audit,
    generation_provider_config::{env_non_empty, env_u64, first_non_empty_env, truncate},
    generation_provider_contract::{
        AiGenerationProvider, GenerationProviderComponent, GenerationProviderError,
        GenerationRequest, ImageGenerationRequest,
    },
    storage,
};

pub const SUPPORTED_IMAGE_JOB_TYPES: &[&str] =
    &["storybook_page_image", "storybook_role_reference_image"];

pub struct SeedreamImageProvider {
    pub(crate) api_key: Option<String>,
    pub(crate) base_url: String,
    pub(crate) endpoint_path: String,
    pub(crate) model: String,
    pub(crate) size: String,
    pub(crate) timeout_seconds: u64,
}

impl SeedreamImageProvider {
    pub(crate) fn from_env() -> Self {
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

    pub(crate) fn api_key_from_env() -> Option<String> {
        env_non_empty("SEEDREAM_API_KEY").or_else(|| env_non_empty("ARK_API_KEY"))
    }

    fn endpoint(&self) -> String {
        format_seedream_endpoint(&self.base_url, &self.endpoint_path)
    }

    pub(crate) fn summary_component(&self) -> GenerationProviderComponent {
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

#[cfg(test)]
pub(crate) const TRANSPARENT_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEklEQVR4nGP4cGnfsxNbGCAUAEWMCcWN1afmAAAAAElFTkSuQmCC";

pub(crate) fn write_generated_image(
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

pub(crate) async fn image_response_to_image_url(
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

pub(crate) fn extract_image_base64(item: &JsonValue) -> Option<&str> {
    item["b64_json"]
        .as_str()
        .or_else(|| item["image_base64"].as_str())
}

pub(crate) fn extract_image_url(item: &JsonValue) -> Option<&str> {
    item["url"].as_str().or_else(|| item["image_url"].as_str())
}

pub(crate) fn format_seedream_endpoint(base_url: &str, endpoint_path: &str) -> String {
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

pub(crate) async fn fetch_remote_image(
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

pub(crate) fn generated_image_file_name(image_id: &str, provider: &str) -> String {
    let image_id = image_id.replace(['/', '\\'], "_");
    let provider = provider.replace(['/', '\\'], "_");
    format!("{provider}-{image_id}.png")
}
