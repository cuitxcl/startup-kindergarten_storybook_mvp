#![cfg(test)]

use crate::services::generation_deepseek_provider::{
    DeepSeekTextProvider, format_deepseek_endpoint, validate_output_against_input,
};
use crate::services::generation_output_validator::normalize_provider_output;
use crate::services::generation_privacy::{
    provider_input_privacy_audit, sanitize_image_prompt_with_audit,
};
use crate::services::generation_provider::{ConfiguredGenerationProvider, GenerationRequest};
use crate::services::generation_provider_config::first_non_empty_value;
use crate::services::generation_provider_contract::{
    AiGenerationProvider, ImageGenerationMode, ImageGenerationRequest, ImageReference,
};
use crate::services::generation_seedream_provider::{
    SeedreamImageProvider, TRANSPARENT_PNG_BASE64, extract_image_base64, extract_image_url,
    fetch_remote_image, format_seedream_endpoint, generated_image_file_name,
    seedream_reference_image_input, write_generated_image,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde_json::{Value as JsonValue, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration as StdDuration;
use uuid::Uuid;

#[test]
fn provider_config_uses_first_non_empty_value() {
    assert_eq!(
        first_non_empty_value(
            [
                None,
                Some("".trim().to_string()),
                Some("doubao-seedream-5-0-260128".to_string())
            ],
            "fallback-model"
        ),
        "doubao-seedream-5-0-260128"
    );
    assert_eq!(
        first_non_empty_value([None, Some("  ".trim().to_string())], "fallback-model"),
        "fallback-model"
    );
}

#[test]
fn provider_env_defaults_to_mock_until_real_mode_is_explicit() {
    let previous = std::env::var("KINDLEAF_GENERATION_PROVIDER").ok();

    unsafe { std::env::remove_var("KINDLEAF_GENERATION_PROVIDER") };
    assert_eq!(ConfiguredGenerationProvider::from_env().name(), "mock");

    unsafe { std::env::set_var("KINDLEAF_GENERATION_PROVIDER", "mock") };
    assert_eq!(ConfiguredGenerationProvider::from_env().name(), "mock");

    unsafe { std::env::set_var("KINDLEAF_GENERATION_PROVIDER", "unexpected") };
    assert_eq!(ConfiguredGenerationProvider::from_env().name(), "mock");

    unsafe { std::env::set_var("KINDLEAF_GENERATION_PROVIDER", "composite") };
    assert_eq!(
        ConfiguredGenerationProvider::from_env().name(),
        "deepseek+seedream"
    );

    match previous {
        Some(value) => unsafe { std::env::set_var("KINDLEAF_GENERATION_PROVIDER", value) },
        None => unsafe { std::env::remove_var("KINDLEAF_GENERATION_PROVIDER") },
    }
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
            model: "doubao-seedream-5-0-260128".to_string(),
            size: "1920x1920".to_string(),
            output_format: "png".to_string(),
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
        model: "doubao-seedream-5-0-260128".to_string(),
        size: "1920x1920".to_string(),
        output_format: "png".to_string(),
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
    assert!(
        summary
            .supports_image
            .contains(&"storybook_role_reference_image".to_string())
    );
    assert!(
        summary
            .supports_image
            .contains(&"storybook_visual_reference".to_string())
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
            model: "doubao-seedream-5-0-260128".to_string(),
            size: "1920x1920".to_string(),
            output_format: "png".to_string(),
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
            .supports_image
            .contains(&"storybook_role_reference_image".to_string())
    );
    assert!(
        summary
            .supports_image
            .contains(&"storybook_visual_reference".to_string())
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
        r#"{"choices":[{"message":{"content":"{\"plan\":{\"title\":\"排队洗手\",\"theme\":\"排队洗手\",\"age_group\":\"4-5 岁\",\"summary\":\"孩子们学会排队洗手。\",\"page_count\":2,\"outline\":[{\"page_range\":\"1\",\"goal\":\"进入场景\",\"beat\":\"孩子看到洗手台\"},{\"page_range\":\"2\",\"goal\":\"理解规则\",\"beat\":\"大家排队洗手\"}],\"role_requirements\":[\"主角儿童\"],\"review_points\":[\"教学目标是否准确\"]}}"}}]}"#,
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
async fn deepseek_provider_accepts_valid_json_even_if_charset_header_is_wrong() {
    let body = r#"{"choices":[{"message":{"content":"{\"plan\":{\"title\":\"排队喝水\",\"theme\":\"排队喝水\",\"age_group\":\"4-5 岁\",\"summary\":\"孩子们学会轮流接水。\",\"page_count\":2,\"outline\":[{\"page_range\":\"1\",\"goal\":\"进入场景\",\"beat\":\"孩子来到水杯架前\"},{\"page_range\":\"2\",\"goal\":\"理解秩序\",\"beat\":\"大家轮流接水\"}],\"role_requirements\":[\"主角儿童\"],\"review_points\":[\"教学目标是否准确\"]}}"}}]}"#;
    let base_url =
        spawn_http_server_with_content_type(body.as_bytes(), "application/json; charset=gbk");
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
            input: &json!({"theme": "排队喝水"}),
        })
        .await
        .expect("provider should parse JSON from bytes even when charset header is wrong");

    assert_eq!(output["provider"], "deepseek");
    assert_eq!(output["mode"], "storybook_plan");
    assert_eq!(output["plan"]["title"], "排队喝水");
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
        model: "doubao-seedream-5-0-260128".to_string(),
        size: "1920x1920".to_string(),
        output_format: "png".to_string(),
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
            size: None,
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
async fn seedream_provider_sends_reference_image_payload_for_edit_mode() {
    let body = format!(
        r#"{{"data":[{{"b64_json":"{}"}}]}}"#,
        TRANSPARENT_PNG_BASE64
    );
    let (base_url, captured_request) = spawn_capturing_http_server(&body);
    let provider = SeedreamImageProvider {
        api_key: Some("test-key".to_string()),
        base_url,
        endpoint_path: "/api/v3/images/generations".to_string(),
        model: "doubao-seedream-5-0-260128".to_string(),
        size: "1920x1920".to_string(),
        output_format: "png".to_string(),
        timeout_seconds: 45,
    };

    let image_id = Uuid::new_v4().to_string();
    let output = provider
        .generate_image(ImageGenerationRequest {
            image_id: &image_id,
            target_id: "role-1",
            target_type: "role",
            mode: "storybook_role_reference_image",
            prompt: "重绘主角参考图，保持幼儿绘本风格",
            reference_images: vec![ImageReference {
                url: "https://example.test/reference-role.png".to_string(),
                source: "role_reference".to_string(),
                role_id: Some("role-1".to_string()),
                label: Some("主角参考图".to_string()),
                generation_job_id: None,
            }],
            edit_instruction: Some("保持角色身份，改成更清晰的正面半身形象".to_string()),
            image_mode: ImageGenerationMode::EditImage,
            strength: Some(0.45),
            size: None,
        })
        .await
        .expect("seedream edit response should be parsed");

    assert_eq!(output["image"]["target_type"], "role");
    assert_eq!(output["image"]["role_id"], "role-1");
    assert_eq!(output["image"]["image_mode"], "edit_image");

    let request = captured_request
        .lock()
        .expect("captured request lock")
        .clone();
    let json_body = captured_json_body(&request);
    assert_eq!(json_body["image_mode"], "edit_image");
    assert_eq!(
        json_body["image"][0],
        "https://example.test/reference-role.png"
    );
    assert_eq!(json_body["reference_images"][0]["role_id"], "role-1");
    assert_eq!(
        json_body["edit_instruction"],
        "保持角色身份，改成更清晰的正面半身形象"
    );
    let strength = json_body["strength"]
        .as_f64()
        .expect("strength should be numeric");
    assert!((strength - 0.45).abs() < 0.0001);
}

#[test]
fn seedream_reference_image_input_embeds_local_generated_images() {
    let image_id = Uuid::new_v4().to_string();
    let image_url = write_generated_image(&image_id, TRANSPARENT_PNG_BASE64, "seedream")
        .expect("local generated image should be written");
    let reference = ImageReference {
        url: image_url.clone(),
        source: "storybook_role".to_string(),
        role_id: Some(Uuid::new_v4().to_string()),
        label: Some("角色参考图".to_string()),
        generation_job_id: None,
    };

    let input =
        seedream_reference_image_input(&reference).expect("local reference should be embedded");

    assert!(input.starts_with("data:image/png;base64,"));
    assert!(input.contains(TRANSPARENT_PNG_BASE64));
    assert_ne!(input, image_url);

    let file_name = image_url.trim_start_matches("/generated-images/");
    let _ = std::fs::remove_file(
        crate::services::storage::local_generated_image_path(file_name)
            .expect("local image path should be valid"),
    );
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
        model: "doubao-seedream-5-0-260128".to_string(),
        size: "1920x1920".to_string(),
        output_format: "png".to_string(),
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
            size: None,
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
    let prompt = sanitize_image_prompt_with_audit("UI Smoke 普通绘本 1784538853883 明亮教室").0;

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
fn deepseek_plan_prompt_requires_title_and_theme_alignment() {
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
            job_type: "storybook_plan",
            input: &json!({"title": "丛林大探险", "theme": "丛林大探险"}),
        })
        .expect("prompt contract should be built");

    let user_prompt = prompt["user_prompt"]
        .as_str()
        .expect("user prompt should be text");
    assert!(user_prompt.contains("input.title"));
    assert!(user_prompt.contains("input.theme"));
    assert!(user_prompt.contains("不得沿用无关"));
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
    assert!(
        prompt["user_prompt"]
            .as_str()
            .expect("user prompt should be text")
            .contains("confirmed_roles")
    );
}

#[test]
fn deepseek_pages_prompt_carries_personalized_photo_rules() {
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

    let user_prompt = prompt["user_prompt"]
        .as_str()
        .expect("user prompt should be text");
    assert!(user_prompt.contains("asset_references"));
    assert!(user_prompt.contains("confirmed_photo_references"));
    assert!(user_prompt.contains("不要把原始照片当贴图"));
    assert!(user_prompt.contains("人物/角色参考只能约束对应人物的外观"));
    assert!(user_prompt.contains("场景图中的人物不得成为故事角色"));
    assert!(user_prompt.contains("page_evidence"));
}

#[test]
fn deepseek_customization_prompt_matches_product_gates() {
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
            job_type: "customization_plan",
            input: &json!({}),
        })
        .expect("prompt contract should be built");

    let user_prompt = prompt["user_prompt"]
        .as_str()
        .expect("user prompt should be text");
    assert!(user_prompt.contains("来源绘本永远只读"));
    assert!(user_prompt.contains("保留来源书主线、页数和阅读节奏"));
    assert!(user_prompt.contains("confirmed_photo_references"));
    assert!(user_prompt.contains("不要输出儿童 id"));
    assert!(user_prompt.contains("后端负责"));
    assert!(prompt["response_schema"]["customization_plan"]["page_plan"].is_array());
    assert!(prompt["response_schema"]["customization_plan"]["unplaced_materials"].is_array());
    assert_eq!(
        prompt["response_schema"]["customization_plan"]["confirmed_photo_references"][0]["reference_type"],
        "character_reference | prop_reference | scene_reference"
    );
    assert!(
        prompt["response_schema"]["customization_plan"]
            .get("target_child_id")
            .is_none()
    );
    assert!(
        prompt["response_schema"]["customization_plan"]
            .get("source_storybook_id")
            .is_none()
    );
}

#[test]
fn deepseek_pages_output_must_reference_confirmed_roles() {
    let input = json!({
        "confirmed_roles": [
            {
                "name": "乐乐",
                "role_type": "主角",
                "appearance": "红色T恤，蓝色短裤",
                "story_function": "学习轮流等待"
            },
            {
                "name": "小美",
                "role_type": "同伴",
                "appearance": "粉色连衣裙，双马尾",
                "story_function": "展示耐心排队"
            }
        ]
    });
    let output = json!({
        "pages": [{
            "page_number": 1,
            "title": "好玩的套圈",
            "body": "小象、小兔和小猴站成一排。",
            "illustration_prompt": "小象 小兔 小猴 面前摆着彩色套圈"
        }]
    });
    let normalized =
        normalize_provider_output(output, "deepseek", "storybook_pages", None, None, None)
            .expect("shape is valid before role consistency check");

    let err = validate_output_against_input(&normalized, &input, "storybook_pages")
        .expect_err("pages that ignore confirmed roles should fail");

    assert!(!err.retryable);
    assert!(err.safe_message().contains("未引用已确认角色"));
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
            .is_some_and(|content| content.contains("JSON") && content.contains("storybook_plan"))
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
        None,
    )
    .expect("provider output should normalize");

    assert_eq!(output["privacy_audit"]["redacted"], true);
    assert_eq!(output["privacy_audit"]["labels"][0], "sensitive_field");
}

#[test]
fn provider_output_content_safety_allows_normal_review_language() {
    let output = normalize_provider_output(
        json!({
            "customization_plan": {
                "source_snapshot": {
                    "title": "小熊等一等",
                    "status": "exportable",
                    "updated_at": "2026-08-21T00:00:00Z",
                    "page_count": 1,
                    "pages": [{"page_number": 1, "title": "排队", "summary": "大家轮流等候"}]
                },
                "strategy": "保留主线，加入孩子兴趣",
                "page_plan": [{
                    "page_number": 1,
                    "decision": "personalize",
                    "requires_redraw": true,
                    "reason": "替换关键道具",
                    "material_labels": [],
                    "photo_display_names": []
                }],
                "confirmed_photo_references": [],
                "unplaced_materials": [],
                "risk_checks": ["不写入家庭住址", "不暴露敏感健康信息", "不改变老师确认过的规则引导目标"]
            }
        }),
        "deepseek",
        "customization_plan",
        None,
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

    let output = normalize_provider_output(output, "deepseek", "storybook_plan", None, None, None)
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
fn provider_output_fills_missing_plan_page_ranges_from_outline_order() {
    let output = normalize_provider_output(
        json!({
            "plan": {
                "title": "排队洗手",
                "theme": "排队等待",
                "summary": "孩子学习等待洗手。",
                "page_count": 2,
                "outline": [
                    {"goal": "进入场景", "beat": "来到洗手区"},
                    {"page_range": "2", "goal": "理解规则", "beat": "大家排队洗手"}
                ],
                "role_requirements": ["主角儿童", "老师"],
                "review_points": ["教学目标准确"]
            }
        }),
        "deepseek",
        "storybook_plan",
        None,
        None,
        None,
    )
    .expect("outline order should recover a missing page range");

    assert_eq!(output["plan"]["outline"][0]["page_range"], json!("1"));
    assert_eq!(output["plan"]["outline"][1]["page_range"], json!("2"));
}

#[test]
fn provider_output_rejects_multi_page_outline_range() {
    let err = normalize_provider_output(
        json!({
            "plan": {
                "title": "排队洗手",
                "theme": "排队等待",
                "summary": "孩子学习等待洗手。",
                "page_count": 2,
                "outline": [
                    {"page_range": "1-2", "goal": "进入场景", "beat": "来到洗手区并排队"}
                ],
                "role_requirements": ["主角儿童", "老师"],
                "review_points": ["教学目标准确"]
            }
        }),
        "deepseek",
        "storybook_plan",
        None,
        None,
        None,
    )
    .expect_err("multi-page range should fail");

    assert!(!err.retryable);
    assert!(err.safe_message().contains("page_range"));
}

#[test]
fn provider_output_rejects_outline_count_mismatch_with_page_count() {
    let err = normalize_provider_output(
        json!({
            "plan": {
                "title": "排队洗手",
                "theme": "排队等待",
                "summary": "孩子学习等待洗手。",
                "page_count": 4,
                "outline": [
                    {"page_range": "1", "goal": "进入场景", "beat": "来到洗手区"},
                    {"page_range": "2", "goal": "理解规则", "beat": "排队等待"}
                ],
                "role_requirements": ["主角儿童", "老师"],
                "review_points": ["教学目标准确"]
            }
        }),
        "deepseek",
        "storybook_plan",
        None,
        None,
        None,
    )
    .expect_err("outline count mismatch should fail");

    assert!(!err.retryable);
    assert!(err.safe_message().contains("page_count"));
}

#[test]
fn provider_output_accepts_one_outline_item_per_page() {
    let output = normalize_provider_output(
        json!({
            "plan": {
                "title": "排队洗手",
                "theme": "排队等待",
                "summary": "孩子学习等待洗手。",
                "page_count": 2,
                "outline": [
                    {"page_range": "1", "goal": "进入场景", "beat": "来到洗手区"},
                    {"page_range": "2", "goal": "理解规则", "beat": "排队等待"}
                ],
                "role_requirements": ["主角儿童", "老师"],
                "review_points": ["教学目标准确"]
            }
        }),
        "deepseek",
        "storybook_plan",
        None,
        None,
        None,
    )
    .expect("one item per page should pass");

    assert_eq!(output["plan"]["outline"].as_array().map(Vec::len), Some(2));
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
        None,
    )
    .expect_err("missing second page body should fail");

    assert!(!err.retryable);
    assert!(err.safe_message().contains("storybook_pages.pages[1].body"));
}

#[test]
fn provider_output_assembles_illustration_slots_into_prompt() {
    let normalized = normalize_provider_output(
        json!({
            "pages": [{
                "page_number": 1,
                "title": "门口挤成一团",
                "body": "小猫和小兔都想先进去。",
                "illustration": {
                    "camera": "中近景，画面紧凑，一群小动物挤在木门口相互遮挡。",
                    "scene_state": "早晨送园高峰，小动物们身体紧紧挨着、你推我搡卡在门口",
                    "contact_chain": "橘色条纹小猫被夹在人群中间，身后的小熊被推着贴上他的背",
                    "crowd": "门口还有五六只小动物踮脚张望排在后面",
                    "action": "小猫踮起脚尖、肩膀前倾、扒着门把手往门缝里挤",
                    "expression": "小猫眉头紧皱、胡须绷直",
                    "prop_detail": "地上有一只被挤掉的粉色书包。"
                },
                "status": "draft"
            }]
        }),
        "deepseek",
        "storybook_pages",
        None,
        None,
        None,
    )
    .expect("structured illustration slots should assemble");

    let prompt = normalized["pages"][0]["illustration_prompt"]
        .as_str()
        .expect("assembled illustration_prompt should exist");
    assert!(prompt.contains("中近景，画面紧凑"));
    assert!(prompt.contains("身体紧紧挨着"));
    assert!(prompt.contains("贴上他的背"));
    assert!(prompt.contains("踮起脚尖"));
    assert!(prompt.contains("眉头紧皱"));
    assert!(prompt.contains("粉色书包"));
    assert!(prompt.contains("柔和水彩绘本风格"));
    assert!(prompt.contains("不要出现文字"));
    // 槽位首尾标点要被清理，避免重复句号。
    assert!(!prompt.contains("。。"));
}

#[test]
fn provider_output_requires_every_illustration_slot() {
    let err = normalize_provider_output(
        json!({
            "pages": [{
                "page_number": 1,
                "title": "门口挤成一团",
                "body": "小猫和小兔都想先进去。",
                "illustration": {
                    "camera": "中近景，画面紧凑",
                    "scene_state": "早晨送园高峰，小动物们挤在门口",
                    "contact_chain": "小猫被夹在人群中间",
                    "crowd": "后排还有几只小动物排队等待",
                    "action": "",
                    "expression": "小猫眉头紧皱"
                }
            }]
        }),
        "deepseek",
        "storybook_pages",
        None,
        None,
        None,
    )
    .expect_err("empty action slot should fail");

    assert!(!err.retryable);
    assert!(
        err.safe_message()
            .contains("storybook_pages.pages[0].illustration.action")
    );
}

#[test]
fn provider_output_rejects_forbidden_illustration_wording() {
    let err = normalize_provider_output(
        json!({
            "pages": [{
                "page_number": 1,
                "title": "门口挤成一团",
                "body": "小猫和小兔都想先进去。",
                "illustration": {
                    "camera": "中近景，画面紧凑",
                    "scene_state": "早晨送园高峰，小动物们挤在门口，背景虚化",
                    "contact_chain": "小猫被夹在人群中间",
                    "crowd": "后排还有几只小动物排队等待",
                    "action": "小猫踮起脚尖往门缝里挤",
                    "expression": "小猫眉头紧皱"
                }
            }]
        }),
        "deepseek",
        "storybook_pages",
        None,
        None,
        None,
    )
    .expect_err("forbidden wording should fail");

    assert!(!err.retryable);
    assert!(err.safe_message().contains("禁止写法"));
    assert!(err.safe_message().contains("背景虚化"));
}

#[test]
fn provider_output_rejects_forbidden_wording_in_legacy_prompt() {
    let err = normalize_provider_output(
        json!({
            "pages": [{
                "page_number": 1,
                "title": "排队",
                "body": "孩子们排队。",
                "illustration_prompt": "幼儿园门口，孩子们略显拥挤，背景柔焦"
            }]
        }),
        "deepseek",
        "storybook_pages",
        None,
        None,
        None,
    )
    .expect_err("legacy prompt with forbidden wording should fail");

    assert!(err.safe_message().contains("禁止写法"));
}

fn storybook_page_with_camera(page_number: u64, camera: &str) -> JsonValue {
    json!({
        "page_number": page_number,
        "title": format!("第 {page_number} 页"),
        "body": format!("第 {page_number} 页正文。"),
        "illustration": {
            "camera": camera,
            "scene_state": "清晨的森林入口，小动物们按队伍聚在一起准备出发",
            "contact_chain": "小松鼠站在队伍旁边看向前方，小兔靠近小松鼠听他说话",
            "crowd": "后排还有几只小动物在树下等待",
            "action": "小松鼠抬头看路牌，身体微微前倾",
            "expression": "小松鼠眼睛睁圆、嘴角微微上扬",
            "prop_detail": "地上有一片发亮的叶子"
        },
        "status": "draft"
    })
}

#[test]
fn provider_output_allows_repetitive_storybook_camera_rhythm() {
    let normalized = normalize_provider_output(
        json!({
            "pages": [
                storybook_page_with_camera(1, "中近景，画面聚焦角色互动"),
                storybook_page_with_camera(2, "中近景，画面聚焦角色互动"),
                storybook_page_with_camera(3, "中近景，画面聚焦角色互动"),
                storybook_page_with_camera(4, "中近景，画面聚焦角色互动"),
                storybook_page_with_camera(5, "中近景，画面聚焦角色互动")
            ]
        }),
        "deepseek",
        "storybook_pages",
        None,
        None,
        None,
    )
    .expect("repetitive camera rhythm should be accepted");

    let prompt = normalized["pages"][4]["illustration_prompt"]
        .as_str()
        .expect("last assembled prompt should exist");
    assert!(prompt.contains("中近景"));
}

#[test]
fn provider_output_allows_overhead_final_page_camera() {
    let normalized = normalize_provider_output(
        json!({
            "pages": [
                storybook_page_with_camera(1, "远景，画面建立森林入口和排队的小动物"),
                storybook_page_with_camera(2, "中景，画面聚焦小松鼠和小兔讨论路线"),
                storybook_page_with_camera(3, "近景，画面聚焦小松鼠发现发光叶子的表情"),
                storybook_page_with_camera(4, "跟随视角，画面沿着小路跟随队伍前进"),
                storybook_page_with_camera(5, "俯视，画面从上方呈现大家围住地图")
            ]
        }),
        "deepseek",
        "storybook_pages",
        None,
        None,
        None,
    )
    .expect("overhead final page camera should be accepted");

    let prompt = normalized["pages"][4]["illustration_prompt"]
        .as_str()
        .expect("last assembled prompt should exist");
    assert!(prompt.contains("俯视"));
}

#[test]
fn provider_output_accepts_varied_storybook_camera_rhythm() {
    let normalized = normalize_provider_output(
        json!({
            "pages": [
                storybook_page_with_camera(1, "远景，画面建立森林入口和排队的小动物"),
                storybook_page_with_camera(2, "中景，画面聚焦小松鼠和小兔讨论路线"),
                storybook_page_with_camera(3, "近景，画面聚焦小松鼠发现发光叶子的表情"),
                storybook_page_with_camera(4, "跟随视角，画面沿着小路跟随队伍前进"),
                storybook_page_with_camera(5, "全景，画面呈现大家到达终点并围成一圈")
            ]
        }),
        "deepseek",
        "storybook_pages",
        None,
        None,
        None,
    )
    .expect("varied camera rhythm should pass");

    let prompt = normalized["pages"][0]["illustration_prompt"]
        .as_str()
        .expect("first assembled prompt should exist");
    assert!(prompt.contains("远景"));
}

#[test]
fn deepseek_pages_payload_uses_lower_temperature() {
    let provider = DeepSeekTextProvider {
        api_key: Some("test-key".to_string()),
        base_url: "https://api.deepseek.com".to_string(),
        endpoint_path: "/chat/completions".to_string(),
        model: "deepseek-v4-flash".to_string(),
        timeout_seconds: 45,
        max_tokens: 4096,
    };
    let pages_payload = provider
        .build_chat_payload(&GenerationRequest {
            job_type: "storybook_pages",
            input: &json!({"page_count": 6}),
        })
        .expect("pages payload should be built");
    assert_eq!(pages_payload["temperature"], json!(0.35));
    assert!(
        pages_payload["max_tokens"].as_u64().unwrap_or_default() >= 16384,
        "storybook_pages should reserve enough output budget for complete page JSON"
    );

    let plan_payload = provider
        .build_chat_payload(&GenerationRequest {
            job_type: "storybook_plan",
            input: &json!({"theme": "排队洗手"}),
        })
        .expect("plan payload should be built");
    assert_eq!(plan_payload["temperature"], json!(0.7));
    assert_eq!(plan_payload["max_tokens"], json!(4096));
}

#[test]
fn deepseek_payload_appends_retry_feedback() {
    let provider = DeepSeekTextProvider {
        api_key: Some("test-key".to_string()),
        base_url: "https://api.deepseek.com".to_string(),
        endpoint_path: "/chat/completions".to_string(),
        model: "deepseek-v4-flash".to_string(),
        timeout_seconds: 45,
        max_tokens: 4096,
    };
    let payload = provider
        .build_chat_payload_with_feedback(
            &GenerationRequest {
                job_type: "storybook_pages",
                input: &json!({"page_count": 6}),
            },
            Some("pages[0].illustration.action 必须是非空文本"),
        )
        .expect("payload with feedback should be built");

    let content = payload["messages"][1]["content"]
        .as_str()
        .expect("user content should be text");
    assert!(content.contains("上一次输出未通过校验"));
    assert!(content.contains("illustration.action"));
}

#[test]
fn provider_output_requires_customization_strategy() {
    let err = normalize_provider_output(
        json!({"customization_plan": {
            "source_snapshot": {
                "title": "小熊等一等",
                "pages": [{"page_number": 1, "title": "排队", "summary": "大家轮流等候"}]
            },
            "page_plan": [],
            "confirmed_photo_references": [],
            "unplaced_materials": []
        }}),
        "deepseek",
        "customization_plan",
        None,
        None,
        None,
    )
    .expect_err("missing customization strategy should fail");

    assert!(!err.retryable);
    assert!(err.safe_message().contains("customization_plan.strategy"));
}

#[test]
fn provider_output_requires_customization_page_plan() {
    let err = normalize_provider_output(
        json!({
            "customization_plan": {
                "source_snapshot": {
                    "title": "小熊等一等",
                    "pages": [{"page_number": 1, "title": "排队", "summary": "大家轮流等候"}]
                },
                "strategy": "保留主线，加入孩子兴趣",
                "confirmed_photo_references": [],
                "unplaced_materials": [],
                "risk_checks": ["不暴露家庭信息"]
            }
        }),
        "deepseek",
        "customization_plan",
        None,
        None,
        None,
    )
    .expect_err("missing page plan should fail");

    assert!(!err.retryable);
    assert!(err.safe_message().contains("customization_plan.page_plan"));
}

#[test]
fn provider_output_validates_customization_risk_checks() {
    let err = normalize_provider_output(
        json!({
            "customization_plan": {
                "source_snapshot": {
                    "title": "小熊等一等",
                    "status": "exportable",
                    "updated_at": "2026-08-21T00:00:00Z",
                    "page_count": 1,
                    "pages": [{"page_number": 1, "title": "排队", "summary": "大家轮流等候"}]
                },
                "strategy": "保留主线，加入孩子兴趣",
                "page_plan": [{
                    "page_number": 1,
                    "decision": "personalize",
                    "requires_redraw": true,
                    "reason": "替换关键道具",
                    "material_labels": [],
                    "photo_display_names": []
                }],
                "confirmed_photo_references": [],
                "unplaced_materials": [],
                "risk_checks": ["不暴露家庭信息", ""]
            }
        }),
        "deepseek",
        "customization_plan",
        None,
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

fn spawn_http_server_with_content_type(body: &[u8], content_type: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test server");
    let addr = listener.local_addr().expect("local addr");
    let body = body.to_vec();
    let content_type = content_type.to_string();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buffer = [0u8; 4096];
            let _ = stream.read(&mut buffer);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                content_type,
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(&body);
            let _ = stream.flush();
        }
    });

    format!("http://{}", addr)
}

fn spawn_capturing_http_server(body: &str) -> (String, Arc<Mutex<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test server");
    let addr = listener.local_addr().expect("local addr");
    let body = body.to_string();
    let captured_request = Arc::new(Mutex::new(String::new()));
    let captured_for_thread = Arc::clone(&captured_request);

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = stream.set_read_timeout(Some(StdDuration::from_millis(250)));
            let mut request = Vec::new();
            loop {
                let mut buffer = [0u8; 4096];
                match stream.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(size) => {
                        request.extend_from_slice(&buffer[..size]);
                        if http_request_body_complete(&request) {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            if let Ok(mut guard) = captured_for_thread.lock() {
                *guard = String::from_utf8_lossy(&request).to_string();
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    (format!("http://{}", addr), captured_request)
}

fn http_request_body_complete(request: &[u8]) -> bool {
    let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
        return false;
    };
    let headers = String::from_utf8_lossy(&request[..header_end]);
    let Some(content_length) = headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("content-length") {
            value.trim().parse::<usize>().ok()
        } else {
            None
        }
    }) else {
        return true;
    };
    request.len().saturating_sub(header_end + 4) >= content_length
}

fn captured_json_body(request: &str) -> JsonValue {
    let (_, body) = request
        .split_once("\r\n\r\n")
        .expect("captured request should include headers and body");
    serde_json::from_str(body).expect("captured request body should be json")
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

#[test]
fn provider_output_assembles_single_page_prompt_from_slots() {
    let output = normalize_provider_output(
        json!({
            "message": "已重写",
            "page": {
                "page_number": 2,
                "illustration": {
                    "camera": "中近景，画面紧凑，小动物们挤在门口",
                    "scene_state": "早晨送园高峰，大家身体紧紧挨着卡成一团",
                    "contact_chain": "小猫被夹在中间，身后的小熊贴上他的背",
                    "crowd": "门口还有五六只小动物踮脚张望排在后面",
                    "action": "小猫踮起脚尖、肩膀前倾扒着门把手",
                    "expression": "小猫眉头紧皱、眼睛瞪圆",
                    "prop_detail": "地上有一只被挤掉的粉色书包"
                }
            }
        }),
        "deepseek",
        "storybook_page_prompt",
        None,
        None,
        None,
    )
    .expect("single page prompt should assemble");

    let prompt = output["page"]["illustration_prompt"]
        .as_str()
        .expect("page.illustration_prompt should be text");
    assert!(prompt.contains("儿童绘本插图"));
    assert!(prompt.contains("中近景"));
    assert!(prompt.contains("粉色书包"));
    assert!(prompt.contains("柔和水彩绘本风格"));
}

#[test]
fn provider_output_rejects_missing_slot_in_single_page_prompt() {
    let err = normalize_provider_output(
        json!({
            "page": {
                "page_number": 2,
                "illustration": {
                    "camera": "中近景",
                    "scene_state": "大家挤在门口",
                    "contact_chain": "小猫被夹在中间",
                    "crowd": "后排还有几只小动物排队",
                    "action": "小猫踮起脚尖"
                }
            }
        }),
        "deepseek",
        "storybook_page_prompt",
        None,
        None,
        None,
    )
    .expect_err("missing expression slot should fail");

    assert!(err.safe_message().contains("expression"));
    assert!(err.safe_message().contains("storybook_page_prompt.page"));
}

#[test]
fn provider_output_rejects_forbidden_wording_in_single_page_prompt() {
    let err = normalize_provider_output(
        json!({
            "page": {
                "page_number": 1,
                "illustration": {
                    "camera": "中近景",
                    "scene_state": "孩子们略显拥挤，背景柔焦",
                    "contact_chain": "小猫被夹在中间",
                    "crowd": "后排还有几只小动物排队",
                    "action": "小猫踮起脚尖",
                    "expression": "小猫眉头紧皱"
                }
            }
        }),
        "deepseek",
        "storybook_page_prompt",
        None,
        None,
        None,
    )
    .expect_err("forbidden wording should fail");

    assert!(err.safe_message().contains("禁止写法"));
}

#[test]
fn single_page_prompt_output_requires_confirmed_role_reference() {
    let output = normalize_provider_output(
        json!({
            "page": {
                "page_number": 2,
                "illustration": {
                    "camera": "中近景",
                    "scene_state": "小动物们挤在门口",
                    "contact_chain": "一个角色被夹在中间",
                    "crowd": "后排还有几只小动物排队",
                    "action": "一个角色踮起脚尖",
                    "expression": "一个角色眉头紧皱"
                }
            }
        }),
        "deepseek",
        "storybook_page_prompt",
        None,
        None,
        None,
    )
    .expect("normalize should succeed");
    let input = json!({
        "page": {"page_id": "x", "page_number": 2, "title": "入园高峰", "body": "小猫米米去上学。", "illustration_prompt": "旧描述"},
        "confirmed_roles": [{"name": "米米", "role_type": "protagonist", "appearance": "橘色条纹小猫", "story_function": "主角"}]
    });
    let err = validate_output_against_input(&output, &input, "storybook_page_prompt")
        .expect_err("prompt without confirmed role name should fail");

    assert!(err.safe_message().contains("未包含已确认角色姓名"));
}

#[test]
fn single_page_prompt_rejects_extreme_role_repetition() {
    let output = normalize_provider_output(
        json!({
            "page": {
                "page_number": 2,
                "illustration": {
                    "camera": "中近景，画面紧凑",
                    "scene_state": "米米在洗手台前排队",
                    "contact_chain": "米米被队伍夹在中间",
                    "crowd": "后排还有几只小动物排队",
                    "action": "米米踮起脚尖看着水龙头",
                    "expression": "米米看见米米排在米米身后，米米眼睛半眯，米米嘴角上扬，米米耳朵放松"
                }
            }
        }),
        "deepseek",
        "storybook_page_prompt",
        None,
        None,
        None,
    )
    .expect("normalize should succeed");
    let input = json!({
        "page": {"page_id": "x", "page_number": 2, "title": "耐心等一等", "body": "米米排队洗手。", "illustration_prompt": "旧描述"},
        "confirmed_roles": [{"name": "米米", "role_type": "protagonist", "appearance": "橘色条纹小猫", "story_function": "主角"}]
    });
    let err = validate_output_against_input(&output, &input, "storybook_page_prompt")
        .expect_err("extreme role repetition should fail");

    assert!(err.safe_message().contains("被点名 9 次"));
}

#[test]
fn single_page_prompt_allows_moderate_role_repetition() {
    let output = normalize_provider_output(
        json!({
            "page": {
                "page_number": 2,
                "illustration": {
                    "camera": "中近景，画面紧凑",
                    "scene_state": "小动物们在洗手台前排队",
                    "contact_chain": "米米被队伍夹在中间",
                    "crowd": "后排还有几只小动物排队",
                    "action": "米米踮起脚尖看着水龙头",
                    "expression": "米米眼睛半眯"
                }
            }
        }),
        "deepseek",
        "storybook_page_prompt",
        None,
        None,
        None,
    )
    .expect("normalize should succeed");
    let input = json!({
        "page": {"page_id": "x", "page_number": 2, "title": "耐心等一等", "body": "米米排队洗手。", "illustration_prompt": "旧描述"},
        "confirmed_roles": [{"name": "米米", "role_type": "protagonist", "appearance": "橘色条纹小猫", "story_function": "主角"}]
    });
    validate_output_against_input(&output, &input, "storybook_page_prompt")
        .expect("moderate role repetition should be handled by quality suggestions");
}

#[test]
fn single_page_prompt_allows_role_mentioned_twice() {
    let output = normalize_provider_output(
        json!({
            "page": {
                "page_number": 2,
                "illustration": {
                    "camera": "中近景，画面紧凑",
                    "scene_state": "小动物们在洗手台前排队",
                    "contact_chain": "米米被队伍夹在中间",
                    "crowd": "后排还有几只小动物排队",
                    "action": "米米踮起脚尖看着水龙头",
                    "expression": "他眼睛半眯"
                }
            }
        }),
        "deepseek",
        "storybook_page_prompt",
        None,
        None,
        None,
    )
    .expect("normalize should succeed");
    let input = json!({
        "page": {"page_id": "x", "page_number": 2, "title": "耐心等一等", "body": "米米排队洗手。", "illustration_prompt": "旧描述"},
        "confirmed_roles": [{"name": "米米", "role_type": "protagonist", "appearance": "橘色条纹小猫", "story_function": "主角"}]
    });
    validate_output_against_input(&output, &input, "storybook_page_prompt")
        .expect("two mentions should pass");
}

#[test]
fn deepseek_page_prompt_payload_uses_lower_temperature() {
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
            job_type: "storybook_page_prompt",
            input: &json!({"page_id": "p1"}),
        })
        .expect("page prompt payload should be built");
    assert_eq!(payload["temperature"], json!(0.35));
}

#[test]
fn provider_output_requires_crowd_slot() {
    let err = normalize_provider_output(
        json!({
            "page": {
                "page_number": 2,
                "illustration": {
                    "camera": "中近景",
                    "scene_state": "小动物们挤在门口",
                    "contact_chain": "小猫被夹在中间",
                    "action": "小猫踮起脚尖",
                    "expression": "小猫眉头紧皱"
                }
            }
        }),
        "deepseek",
        "storybook_page_prompt",
        None,
        None,
        None,
    )
    .expect_err("missing crowd slot should fail");

    assert!(err.safe_message().contains("crowd"));
}

#[test]
fn single_page_prompt_rejects_crowd_disappearing_between_pages() {
    let input = json!({
        "page": {"page_id": "x", "page_number": 2, "title": "老师来帮忙", "body": "长颈鹿老师来了。", "illustration_prompt": "旧描述"},
        "confirmed_roles": [{"name": "米米", "role_type": "protagonist", "appearance": "橘色条纹小猫", "story_function": "主角"}],
        "neighbor_pages": [
            {"page_number": 1, "title": "门口挤成一团", "illustration_prompt": "儿童绘本插图，一群小动物挤在幼儿园门口，身体紧紧挨着你推我搡"}
        ]
    });
    let vanished = json!({
        "page": {
            "page_number": 2,
            "illustration_prompt": "儿童绘本插图，米米仰头看着长颈鹿老师，老师弯腰示范。"
        }
    });
    let err = validate_output_against_input(&vanished, &input, "storybook_page_prompt")
        .expect_err("crowd vanishing between pages should fail");
    assert!(err.safe_message().contains("未交代在场群体"));

    let kept = json!({
        "page": {
            "page_number": 2,
            "illustration_prompt": "儿童绘本插图，米米仰头看着长颈鹿老师，小动物们在后排排成一列等待。"
        }
    });
    validate_output_against_input(&kept, &input, "storybook_page_prompt")
        .expect("prompt keeping the crowd should pass");
}
