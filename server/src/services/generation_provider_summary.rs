use crate::services::{
    generation_deepseek_provider::{
        DEFAULT_TEXT_SCHEMA_VERSION, DeepSeekTextProvider, SUPPORTED_TEXT_JOB_TYPES,
    },
    generation_provider::{ConfiguredGenerationProvider, SUPPORTED_IMAGE_JOB_TYPES},
    generation_provider_config::env_non_empty,
    generation_provider_contract::{
        AiGenerationProvider, GenerationProviderComponent, GenerationProviderSummary,
    },
    generation_seedream_provider::SeedreamImageProvider,
};

pub fn provider_summary(provider: &ConfiguredGenerationProvider) -> GenerationProviderSummary {
    let requested_provider = ConfiguredGenerationProvider::raw_provider_mode();
    let deepseek_key_present = env_non_empty("DEEPSEEK_API_KEY").is_some();
    let seedream_key_present = SeedreamImageProvider::api_key_from_env().is_some();

    match provider {
        ConfiguredGenerationProvider::Mock(_) => GenerationProviderSummary {
            provider: "mock".to_string(),
            mode: "demo".to_string(),
            schema_version: "generation.mock.v1".to_string(),
            requires_api_key: false,
            supports_text: supported_text_jobs(),
            supports_image: supported_image_jobs(),
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
        ConfiguredGenerationProvider::DeepSeek(provider) => GenerationProviderSummary {
            provider: provider.name().to_string(),
            mode: "text".to_string(),
            schema_version: DEFAULT_TEXT_SCHEMA_VERSION.to_string(),
            requires_api_key: true,
            supports_text: supported_text_jobs(),
            supports_image: vec![],
            real_text_ready: true,
            real_image_ready: false,
            production_ready: false,
            missing_configuration: vec![],
            components: generation_provider_components(),
            diagnostic: "文本生成已接入真实 provider，插图仍使用 mock".to_string(),
        },
        ConfiguredGenerationProvider::Seedream(provider) => GenerationProviderSummary {
            provider: provider.name().to_string(),
            mode: "image".to_string(),
            schema_version: DEFAULT_TEXT_SCHEMA_VERSION.to_string(),
            requires_api_key: true,
            supports_text: vec![],
            supports_image: supported_image_jobs(),
            real_text_ready: false,
            real_image_ready: true,
            production_ready: false,
            missing_configuration: vec![],
            components: generation_provider_components(),
            diagnostic: "插图生成已接入 Seedream，文本仍使用 mock".to_string(),
        },
        ConfiguredGenerationProvider::Composite { text, image } => GenerationProviderSummary {
            provider: format!("{}+{}", text.name(), image.name()),
            mode: "composite".to_string(),
            schema_version: DEFAULT_TEXT_SCHEMA_VERSION.to_string(),
            requires_api_key: true,
            supports_text: supported_text_jobs(),
            supports_image: supported_image_jobs(),
            real_text_ready: true,
            real_image_ready: true,
            production_ready: true,
            missing_configuration: vec![],
            components: generation_provider_components(),
            diagnostic: "文本和插图均已接入真实 provider".to_string(),
        },
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
