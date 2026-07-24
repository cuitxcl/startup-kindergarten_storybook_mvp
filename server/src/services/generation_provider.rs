#![allow(dead_code)]

use serde_json::Value as JsonValue;

pub use crate::services::generation_deepseek_provider::{
    DeepSeekTextProvider, SUPPORTED_TEXT_JOB_TYPES,
};
use crate::services::generation_mock_provider::MockGenerationProvider;
use crate::services::generation_provider_config::env_non_empty;
pub use crate::services::generation_provider_contract::{
    AiGenerationProvider, GenerationProviderError, GenerationProviderSummary, GenerationRequest,
    ImageGenerationMode, ImageGenerationRequest, ImageReference,
};
pub use crate::services::generation_seedream_provider::{
    SUPPORTED_IMAGE_JOB_TYPES, SeedreamImageProvider,
};

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
                if SUPPORTED_TEXT_JOB_TYPES.contains(&job_type) {
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
