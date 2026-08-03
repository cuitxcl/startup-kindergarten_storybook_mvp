#![allow(dead_code)]

use serde_json::Value as JsonValue;

pub use crate::services::generation_deepseek_provider::{
    DeepSeekTextProvider, SUPPORTED_TEXT_JOB_TYPES,
};
use crate::services::generation_provider_config::env_non_empty;
pub use crate::services::generation_provider_contract::{
    AiGenerationProvider, GenerationProviderError, GenerationProviderSummary, GenerationRequest,
    ImageGenerationMode, ImageGenerationRequest, ImageReference,
};
pub use crate::services::generation_seedream_provider::{
    SUPPORTED_IMAGE_JOB_TYPES, SeedreamImageProvider,
};

pub enum ConfiguredGenerationProvider {
    DeepSeek(DeepSeekTextProvider),
    Seedream(SeedreamImageProvider),
    Composite {
        text: DeepSeekTextProvider,
        image: SeedreamImageProvider,
    },
}

impl ConfiguredGenerationProvider {
    /// 只接入真实 provider，没有 mock 回退。
    /// 缺 key 时不做静默降级：生成任务会以明确的配置错误失败，
    /// 并通过 provider summary 的 missing_configuration 暴露给前端。
    pub fn from_env() -> Self {
        let provider = std::env::var("KINDLEAF_GENERATION_PROVIDER")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        match provider.as_str() {
            "deepseek" => Self::DeepSeek(DeepSeekTextProvider::from_env()),
            "seedream" => Self::Seedream(SeedreamImageProvider::from_env()),
            _ => Self::Composite {
                text: DeepSeekTextProvider::from_env(),
                image: SeedreamImageProvider::from_env(),
            },
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
            Self::DeepSeek(provider) => provider.generate_image(request).await,
            Self::Seedream(provider) => provider.generate_image(request).await,
            Self::Composite { image, .. } => image.generate_image(request).await,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::DeepSeek(provider) => provider.name(),
            Self::Seedream(provider) => provider.name(),
            Self::Composite { .. } => "deepseek+seedream",
        }
    }

    pub fn name_for_job_type(&self, job_type: &str) -> &'static str {
        match self {
            Self::DeepSeek(provider) => provider.name(),
            Self::Seedream(provider) => provider.name(),
            Self::Composite { text, image } => {
                if SUPPORTED_IMAGE_JOB_TYPES.contains(&job_type) {
                    image.name()
                } else if SUPPORTED_TEXT_JOB_TYPES.contains(&job_type) {
                    text.name()
                } else {
                    self.name()
                }
            }
        }
    }

    pub fn summary(&self) -> GenerationProviderSummary {
        crate::services::generation_provider_summary::provider_summary(self)
    }
}
