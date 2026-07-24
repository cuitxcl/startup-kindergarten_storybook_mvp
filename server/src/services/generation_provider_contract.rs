use serde_json::Value as JsonValue;

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
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: false,
        }
    }

    pub(crate) fn retryable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
        }
    }

    pub fn safe_message(&self) -> String {
        truncate_message(&self.message, 240)
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

fn truncate_message(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        let mut truncated = value.chars().take(max_chars).collect::<String>();
        truncated.push('…');
        truncated
    }
}
