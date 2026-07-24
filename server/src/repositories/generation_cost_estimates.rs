use serde_json::Value as JsonValue;

use crate::models::GenerationJob;

#[cfg(test)]
use chrono::Utc;
#[cfg(test)]
use serde_json::json;
#[cfg(test)]
use uuid::Uuid;

#[derive(Debug, PartialEq)]
pub(crate) struct GenerationCostEstimate {
    pub(crate) provider: String,
    pub(crate) estimated_input_units: i32,
    pub(crate) estimated_output_units: i32,
    pub(crate) image_count: i32,
    pub(crate) estimated_cost_micros: i64,
    pub(crate) currency: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GenerationCostPricing {
    deepseek_input_unit_micros: i64,
    deepseek_output_unit_micros: i64,
    seedream_image_micros: i64,
    currency: String,
}

impl Default for GenerationCostPricing {
    fn default() -> Self {
        Self {
            deepseek_input_unit_micros: 1,
            deepseek_output_unit_micros: 4,
            seedream_image_micros: 40_000,
            currency: "USD".to_string(),
        }
    }
}

impl GenerationCostPricing {
    pub(crate) fn from_env() -> Self {
        let defaults = Self::default();
        Self {
            deepseek_input_unit_micros: env_i64(
                "KINDLEAF_COST_DEEPSEEK_INPUT_UNIT_MICROS",
                defaults.deepseek_input_unit_micros,
            ),
            deepseek_output_unit_micros: env_i64(
                "KINDLEAF_COST_DEEPSEEK_OUTPUT_UNIT_MICROS",
                defaults.deepseek_output_unit_micros,
            ),
            seedream_image_micros: env_i64(
                "KINDLEAF_COST_SEEDREAM_IMAGE_MICROS",
                defaults.seedream_image_micros,
            ),
            currency: std::env::var("KINDLEAF_COST_CURRENCY")
                .ok()
                .map(|value| value.trim().to_ascii_uppercase())
                .filter(|value| !value.is_empty())
                .unwrap_or(defaults.currency),
        }
    }
}

pub(crate) fn estimate_generation_cost(job: &GenerationJob) -> GenerationCostEstimate {
    estimate_generation_cost_with_pricing(job, &GenerationCostPricing::from_env())
}

pub(crate) fn estimate_generation_cost_with_pricing(
    job: &GenerationJob,
    pricing: &GenerationCostPricing,
) -> GenerationCostEstimate {
    let output = job.output_json.as_ref();
    let provider = output
        .and_then(|value| value.get("provider"))
        .and_then(|value| value.as_str())
        .unwrap_or("unknown")
        .to_string();
    let status = job.status.as_str();
    let (estimated_input_units, estimated_output_units) =
        output.and_then(provider_usage_units).unwrap_or_else(|| {
            (
                estimate_json_units(&job.input_json),
                output.map(estimate_json_units).unwrap_or_default(),
            )
        });
    let image_count = if is_image_job(&job.job_type) && status == "succeeded" {
        1
    } else {
        0
    };
    let estimated_cost_micros = if status != "succeeded" || provider == "mock" {
        0
    } else if provider == "seedream" && is_image_job(&job.job_type) {
        pricing.seedream_image_micros * i64::from(image_count)
    } else if provider == "deepseek" {
        i64::from(estimated_input_units) * pricing.deepseek_input_unit_micros
            + i64::from(estimated_output_units) * pricing.deepseek_output_unit_micros
    } else {
        0
    };

    GenerationCostEstimate {
        provider,
        estimated_input_units,
        estimated_output_units,
        image_count,
        estimated_cost_micros,
        currency: pricing.currency.clone(),
    }
}

fn env_i64(key: &str, fallback: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value >= 0)
        .unwrap_or(fallback)
}

pub(crate) fn estimate_json_units(value: &JsonValue) -> i32 {
    let text = serde_json::to_string(value).unwrap_or_default();
    ((text.chars().count() as f64) / 4.0).ceil().max(0.0) as i32
}

pub(crate) fn provider_usage_units(output: &JsonValue) -> Option<(i32, i32)> {
    let usage = output.get("provider_usage")?;
    let input = usage
        .get("prompt_tokens")
        .or_else(|| usage.get("input_tokens"))
        .and_then(|value| value.as_i64())
        .unwrap_or(0)
        .max(0) as i32;
    let output = usage
        .get("completion_tokens")
        .or_else(|| usage.get("output_tokens"))
        .and_then(|value| value.as_i64())
        .unwrap_or(0)
        .max(0) as i32;
    if input == 0 && output == 0 {
        None
    } else {
        Some((input, output))
    }
}

pub(crate) fn is_image_job(job_type: &str) -> bool {
    matches!(
        job_type,
        "storybook_page_image" | "storybook_role_reference_image"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(
        job_type: &str,
        status: &str,
        input_json: JsonValue,
        output_json: JsonValue,
    ) -> GenerationJob {
        GenerationJob {
            id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            storybook_id: Some(Uuid::new_v4()),
            job_type: job_type.to_string(),
            status: status.to_string(),
            input_json,
            output_json: Some(output_json),
            attempt_count: 1,
            last_error: None,
            next_run_at: None,
            locked_by: None,
            locked_at: None,
            created_at: Utc::now(),
            finished_at: Some(Utc::now()),
        }
    }

    #[test]
    fn mock_generation_cost_is_zero() {
        let estimate = estimate_generation_cost(&job(
            "storybook_plan",
            "succeeded",
            json!({"theme": "排队洗手"}),
            json!({"provider": "mock", "mode": "storybook_plan"}),
        ));

        assert_eq!(estimate.provider, "mock");
        assert_eq!(estimate.estimated_cost_micros, 0);
    }

    #[test]
    fn deepseek_text_generation_cost_uses_input_and_output_units() {
        let estimate = estimate_generation_cost(&job(
            "storybook_plan",
            "succeeded",
            json!({"theme": "排队洗手", "age_group": "4-5 岁"}),
            json!({"provider": "deepseek", "plan": {"title": "一起洗手", "summary": "孩子学会排队洗手"}}),
        ));

        assert_eq!(estimate.provider, "deepseek");
        assert!(estimate.estimated_input_units > 0);
        assert!(estimate.estimated_output_units > 0);
        assert!(estimate.estimated_cost_micros > 0);
        assert_eq!(estimate.image_count, 0);
    }

    #[test]
    fn deepseek_text_generation_cost_prefers_provider_usage() {
        let estimate = estimate_generation_cost(&job(
            "storybook_plan",
            "succeeded",
            json!({"theme": "排队洗手", "age_group": "4-5 岁"}),
            json!({
                "provider": "deepseek",
                "provider_usage": {
                    "prompt_tokens": 120,
                    "completion_tokens": 80,
                    "total_tokens": 200
                },
                "plan": {"title": "一起洗手"}
            }),
        ));

        assert_eq!(estimate.estimated_input_units, 120);
        assert_eq!(estimate.estimated_output_units, 80);
        assert_eq!(estimate.estimated_cost_micros, 440);
    }

    #[test]
    fn deepseek_text_generation_cost_uses_configured_pricing() {
        let estimate = estimate_generation_cost_with_pricing(
            &job(
                "storybook_plan",
                "succeeded",
                json!({"theme": "排队洗手"}),
                json!({
                    "provider": "deepseek",
                    "provider_usage": {
                        "prompt_tokens": 10,
                        "completion_tokens": 20
                    }
                }),
            ),
            &GenerationCostPricing {
                deepseek_input_unit_micros: 2,
                deepseek_output_unit_micros: 5,
                seedream_image_micros: 40_000,
                currency: "CNY".to_string(),
            },
        );

        assert_eq!(estimate.estimated_input_units, 10);
        assert_eq!(estimate.estimated_output_units, 20);
        assert_eq!(estimate.estimated_cost_micros, 120);
        assert_eq!(estimate.currency, "CNY");
    }

    #[test]
    fn seedream_image_generation_cost_counts_one_image() {
        let estimate = estimate_generation_cost(&job(
            "storybook_page_image",
            "succeeded",
            json!({"prompt": "温暖幼儿园教室"}),
            json!({"provider": "seedream", "image": {"image_url": "/api/image.png"}}),
        ));

        assert_eq!(estimate.provider, "seedream");
        assert_eq!(estimate.image_count, 1);
        assert_eq!(estimate.estimated_cost_micros, 40_000);
    }

    #[test]
    fn seedream_image_generation_cost_uses_configured_pricing() {
        let estimate = estimate_generation_cost_with_pricing(
            &job(
                "storybook_page_image",
                "succeeded",
                json!({"prompt": "温暖幼儿园教室"}),
                json!({"provider": "seedream", "image": {"image_url": "/api/image.png"}}),
            ),
            &GenerationCostPricing {
                deepseek_input_unit_micros: 1,
                deepseek_output_unit_micros: 4,
                seedream_image_micros: 88_000,
                currency: "USD".to_string(),
            },
        );

        assert_eq!(estimate.image_count, 1);
        assert_eq!(estimate.estimated_cost_micros, 88_000);
    }

    #[test]
    fn failed_generation_cost_is_zero() {
        let estimate = estimate_generation_cost(&job(
            "storybook_plan",
            "failed",
            json!({"theme": "排队洗手"}),
            json!({"provider": "deepseek", "error": {"retryable": true}}),
        ));

        assert_eq!(estimate.provider, "deepseek");
        assert_eq!(estimate.estimated_cost_micros, 0);
    }
}
