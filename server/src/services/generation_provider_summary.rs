use crate::services::{
    generation_deepseek_provider::{
        DEFAULT_TEXT_SCHEMA_VERSION, DeepSeekTextProvider, SUPPORTED_TEXT_JOB_TYPES,
    },
    generation_provider::{ConfiguredGenerationProvider, SUPPORTED_IMAGE_JOB_TYPES},
    generation_provider_contract::{
        AiGenerationProvider, GenerationProviderComponent, GenerationProviderSummary,
    },
    generation_seedream_provider::SeedreamImageProvider,
};

/// 只描述真实 provider 的就绪状态；缺少 API key 时通过
/// missing_configuration / diagnostic 明确暴露，不存在 mock 回退。
pub fn provider_summary(provider: &ConfiguredGenerationProvider) -> GenerationProviderSummary {
    match provider {
        ConfiguredGenerationProvider::DeepSeek(provider) => {
            let text_ready = provider.api_key.is_some();
            GenerationProviderSummary {
                provider: provider.name().to_string(),
                mode: "text".to_string(),
                schema_version: DEFAULT_TEXT_SCHEMA_VERSION.to_string(),
                requires_api_key: true,
                supports_text: supported_text_jobs(),
                supports_image: vec![],
                real_text_ready: text_ready,
                real_image_ready: false,
                production_ready: false,
                missing_configuration: missing_configuration(text_ready, false),
                components: generation_provider_components(),
                diagnostic: if text_ready {
                    "文本生成已接入真实 provider；插图生成未配置（缺少 SEEDREAM_API_KEY 或 ARK_API_KEY）".to_string()
                } else {
                    "缺少 DEEPSEEK_API_KEY，文本生成不可用".to_string()
                },
            }
        }
        ConfiguredGenerationProvider::Seedream(provider) => {
            let image_ready = provider.api_key.is_some();
            GenerationProviderSummary {
                provider: provider.name().to_string(),
                mode: "image".to_string(),
                schema_version: DEFAULT_TEXT_SCHEMA_VERSION.to_string(),
                requires_api_key: true,
                supports_text: vec![],
                supports_image: supported_image_jobs(),
                real_text_ready: false,
                real_image_ready: image_ready,
                production_ready: false,
                missing_configuration: missing_configuration(false, image_ready),
                components: generation_provider_components(),
                diagnostic: if image_ready {
                    "插图生成已接入真实 provider；文本生成未配置（缺少 DEEPSEEK_API_KEY）"
                        .to_string()
                } else {
                    "缺少 SEEDREAM_API_KEY 或 ARK_API_KEY，插图生成不可用".to_string()
                },
            }
        }
        ConfiguredGenerationProvider::Composite { text, image } => {
            let text_ready = text.api_key.is_some();
            let image_ready = image.api_key.is_some();
            GenerationProviderSummary {
                provider: format!("{}+{}", text.name(), image.name()),
                mode: "composite".to_string(),
                schema_version: DEFAULT_TEXT_SCHEMA_VERSION.to_string(),
                requires_api_key: true,
                supports_text: supported_text_jobs(),
                supports_image: supported_image_jobs(),
                real_text_ready: text_ready,
                real_image_ready: image_ready,
                production_ready: text_ready && image_ready,
                missing_configuration: missing_configuration(text_ready, image_ready),
                components: generation_provider_components(),
                diagnostic: match (text_ready, image_ready) {
                    (true, true) => "文本和插图均已接入真实 provider".to_string(),
                    (false, true) => "缺少 DEEPSEEK_API_KEY，文本生成不可用".to_string(),
                    (true, false) => {
                        "缺少 SEEDREAM_API_KEY 或 ARK_API_KEY，插图生成不可用".to_string()
                    }
                    (false, false) => {
                        "缺少 DEEPSEEK_API_KEY 与 SEEDREAM_API_KEY 或 ARK_API_KEY，生成不可用"
                            .to_string()
                    }
                },
            }
        }
    }
}

fn supported_text_jobs() -> Vec<String> {
    SUPPORTED_TEXT_JOB_TYPES
        .iter()
        .map(|item| item.to_string())
        .collect()
}

fn supported_image_jobs() -> Vec<String> {
    SUPPORTED_IMAGE_JOB_TYPES
        .iter()
        .map(|item| item.to_string())
        .collect()
}

fn missing_configuration(text_ready: bool, image_ready: bool) -> Vec<String> {
    let mut missing = Vec::new();
    if !text_ready {
        missing.push("DEEPSEEK_API_KEY".to_string());
    }
    if !image_ready {
        missing.push("SEEDREAM_API_KEY 或 ARK_API_KEY".to_string());
    }
    missing
}

fn generation_provider_components() -> Vec<GenerationProviderComponent> {
    vec![
        DeepSeekTextProvider::from_env().summary_component(),
        SeedreamImageProvider::from_env().summary_component(),
    ]
}
