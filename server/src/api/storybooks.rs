use super::{ApiError, SharedState, now};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, patch, post},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use std::env;
use std::time::Duration;
use uuid::Uuid;

pub use crate::models::storybooks::{StorybookPageRecord, StorybookRecord, StorybookStore};
use crate::views::storybooks::{
    CaseSummary, ChildSummary, GenerateStorybookResponse, ListResponse, StoryTaskSummary,
    StorybookDetailResponse,
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/storybooks/generate", post(generate_storybook))
        .route("/storybooks", get(list_storybooks))
        .route(
            "/storybooks/{storybook_id}",
            get(get_storybook).patch(update_storybook),
        )
        .route(
            "/storybooks/{storybook_id}/duplicate",
            post(duplicate_storybook),
        )
        .route(
            "/storybooks/{storybook_id}/derive-custom",
            post(derive_custom_storybook),
        )
        .route(
            "/storybooks/{storybook_id}/pages",
            get(list_pages).post(add_page),
        )
        .route(
            "/storybooks/{storybook_id}/pages/{page_id}",
            patch(update_page).delete(delete_page),
        )
        .route(
            "/storybooks/{storybook_id}/pages/{page_id}/rewrite",
            post(rewrite_page),
        )
}

#[derive(Clone, Debug)]
pub enum StoryProviderKind {
    DeepSeek(DeepSeekStoryProvider),
    #[cfg(test)]
    Test(TestStoryProvider),
}

impl StoryProviderKind {
    fn provider_name(&self) -> &'static str {
        match self {
            Self::DeepSeek(provider) => provider.provider_name(),
            #[cfg(test)]
            Self::Test(provider) => provider.provider_name(),
        }
    }

    fn generate(&self, input: StoryGenerationInput) -> StoryGenerationOutput {
        match self {
            Self::DeepSeek(provider) => provider.generate(input),
            #[cfg(test)]
            Self::Test(provider) => provider.generate(input),
        }
    }

    fn rewrite_page(&self, input: PageRewriteInput) -> StoryGeneratedPage {
        match self {
            Self::DeepSeek(provider) => provider.rewrite_page(input),
            #[cfg(test)]
            Self::Test(provider) => provider.rewrite_page(input),
        }
    }
}

pub trait StoryGenerationProvider {
    fn provider_name(&self) -> &'static str;
    fn generate(&self, input: StoryGenerationInput) -> StoryGenerationOutput;
    fn rewrite_page(&self, input: PageRewriteInput) -> StoryGeneratedPage;
}

#[derive(Clone)]
pub struct DeepSeekStoryProvider {
    api_key: Option<String>,
    base_url: String,
    model: String,
    timeout_seconds: u64,
}

impl std::fmt::Debug for DeepSeekStoryProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeepSeekStoryProvider")
            .field("api_key", &self.api_key.as_ref().map(|_| "***"))
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("timeout_seconds", &self.timeout_seconds)
            .finish()
    }
}

impl Default for DeepSeekStoryProvider {
    fn default() -> Self {
        Self::from_env()
    }
}

impl DeepSeekStoryProvider {
    pub fn from_env() -> Self {
        Self {
            api_key: env::var("DEEPSEEK_API_KEY")
                .ok()
                .and_then(normalize_optional_owned),
            base_url: env::var("DEEPSEEK_BASE_URL")
                .ok()
                .and_then(normalize_optional_owned)
                .unwrap_or_else(|| "https://api.deepseek.com".to_string()),
            model: env::var("DEEPSEEK_MODEL")
                .ok()
                .and_then(normalize_optional_owned)
                .unwrap_or_else(|| "deepseek-chat".to_string()),
            timeout_seconds: env::var("DEEPSEEK_TIMEOUT_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(45),
        }
    }

    fn chat_completion(&self, system_prompt: &str, user_prompt: String) -> Option<String> {
        let api_key = self.api_key.as_deref()?;
        let endpoint = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(self.timeout_seconds))
            .build()
            .ok()?;
        let response = client
            .post(endpoint)
            .bearer_auth(api_key)
            .json(&json!({
                "model": self.model,
                "temperature": 0.7,
                "response_format": { "type": "json_object" },
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": user_prompt }
                ]
            }))
            .send()
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let body = response.json::<Value>().ok()?;
        body.get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    }

    fn generate_with_deepseek(&self, input: StoryGenerationInput) -> Option<StoryGenerationOutput> {
        let system_prompt =
            "你是幼儿园老师的绘本故事生成助手。只返回严格 JSON，不要使用 Markdown。";
        let user_prompt = format!(
            r#"请生成一本适合幼儿园孩子阅读的绘本故事。
输出 JSON 格式：
{{
  "title": "故事标题",
  "role_manifest_json": {{"protagonist": {{"role_key": "protagonist", "role_type": "child 或 default_character", "display_name": "角色名"}}}},
  "pages": [
    {{
      "page_role": "cover/story/closing",
      "page_title": "页面标题",
      "body_text": "页面正文，适合孩子朗读，简短温暖",
      "prompt_text": "老师可追问孩子的问题",
      "teacher_tip": "老师引导建议",
      "scene_spec_json": {{"location": "地点", "action": "动作", "composition": "构图", "emotion": "情绪"}}
    }}
  ]
}}
要求：
- 总页数必须是 {page_count} 页。
- 第 1 页 page_role 为 cover，最后 1 页 page_role 为 closing，其余为 story。
- content_type: {content_type}
- 主角: {child_name}
- 主题: {theme}
- 教学目标: {teaching_goal}
- 年龄段: {age_group}
- 每页正文不要超过 80 个中文字符。"#,
            page_count = input.page_count,
            content_type = input.content_type,
            child_name = input.child_name.as_deref().unwrap_or("小朋友"),
            theme = input.theme,
            teaching_goal = input.teaching_goal,
            age_group = input.reading_age_group.as_deref().unwrap_or("5-6")
        );
        let content = self.chat_completion(system_prompt, user_prompt)?;
        parse_deepseek_story_output(&content, input.page_count, input.title)
    }

    fn rewrite_with_deepseek(&self, input: PageRewriteInput) -> Option<StoryGeneratedPage> {
        let system_prompt =
            "你是幼儿园老师的绘本单页改写助手。只返回严格 JSON，不要使用 Markdown。";
        let user_prompt = format!(
            r#"请改写绘本第 {page_number} 页，输出 JSON：
{{
  "page_title": "页面标题",
  "body_text": "改写后的页面正文",
  "prompt_text": "老师可追问孩子的问题",
  "teacher_tip": "老师引导建议",
  "scene_spec_json": {{"location": "地点", "action": "动作", "composition": "构图", "emotion": "情绪"}}
}}
故事标题：{title}
教学目标：{teaching_goal}
原文：{original_body_text}
要求：正文适合幼儿园孩子阅读，不超过 80 个中文字符。"#,
            page_number = input.page_number,
            title = input.title,
            teaching_goal = input.teaching_goal,
            original_body_text = input.original_body_text
        );
        let content = self.chat_completion(system_prompt, user_prompt)?;
        parse_deepseek_rewrite_output(&content)
    }
}

impl StoryGenerationProvider for DeepSeekStoryProvider {
    fn provider_name(&self) -> &'static str {
        "deepseek"
    }

    fn generate(&self, input: StoryGenerationInput) -> StoryGenerationOutput {
        if let Some(output) = self.generate_with_deepseek(input.clone()) {
            return output;
        }
        deterministic_story("deepseek", input)
    }

    fn rewrite_page(&self, input: PageRewriteInput) -> StoryGeneratedPage {
        if let Some(output) = self.rewrite_with_deepseek(input.clone()) {
            return output;
        }
        deterministic_rewrite("deepseek", input)
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Default)]
pub struct TestStoryProvider;

#[cfg(test)]
impl StoryGenerationProvider for TestStoryProvider {
    fn provider_name(&self) -> &'static str {
        "test_story_provider"
    }

    fn generate(&self, input: StoryGenerationInput) -> StoryGenerationOutput {
        deterministic_story("test_story_provider", input)
    }

    fn rewrite_page(&self, input: PageRewriteInput) -> StoryGeneratedPage {
        deterministic_rewrite("test_story_provider", input)
    }
}

#[derive(Clone, Debug)]
pub struct StoryGenerationInput {
    pub title: String,
    pub content_type: String,
    pub child_name: Option<String>,
    pub theme: String,
    pub teaching_goal: String,
    pub reading_age_group: Option<String>,
    pub page_count: i32,
}

#[derive(Clone, Debug)]
pub struct PageRewriteInput {
    pub title: String,
    pub page_number: i32,
    pub teaching_goal: String,
    pub original_body_text: String,
}

#[derive(Clone, Debug)]
pub struct StoryGenerationOutput {
    pub title: String,
    pub pages: Vec<StoryGeneratedPage>,
    pub role_manifest_json: Value,
}

#[derive(Clone, Debug)]
pub struct StoryGeneratedPage {
    pub page_role: String,
    pub page_title: Option<String>,
    pub body_text: String,
    pub prompt_text: Option<String>,
    pub teacher_tip: Option<String>,
    pub scene_spec_json: Value,
}

#[derive(Debug, Deserialize)]
pub struct GenerateStorybookRequest {
    pub content_type: String,
    pub child_id: Option<Uuid>,
    pub case_storybook_id: Option<Uuid>,
    pub source_storybook_id: Option<Uuid>,
    pub title_override: Option<String>,
    pub theme_override: Option<String>,
    pub style_id: Option<String>,
    pub reading_age_group: Option<String>,
    pub teaching_goal: Option<String>,
    pub page_count: Option<i32>,
    #[serde(default)]
    pub generation_options: Value,
}

#[derive(Debug, Deserialize)]
pub struct StorybookListQuery {
    pub status: Option<String>,
    pub child_id: Option<Uuid>,
    pub content_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStorybookRequest {
    pub title: Option<String>,
    pub teaching_goal: Option<String>,
    pub share_scope: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DuplicateStorybookRequest {
    pub title_override: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeriveCustomStorybookRequest {
    pub child_id: Uuid,
    pub title_override: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddPageRequest {
    pub page_number: Option<i32>,
    pub page_role: Option<String>,
    pub page_title: Option<String>,
    pub body_text: String,
    pub prompt_text: Option<String>,
    pub teacher_tip: Option<String>,
    pub scene_spec_json: Option<Value>,
    pub scene_spec_status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePageRequest {
    pub page_title: Option<String>,
    pub body_text: Option<String>,
    pub prompt_text: Option<String>,
    pub teacher_tip: Option<String>,
    pub scene_spec_json: Option<Value>,
    pub scene_spec_status: Option<String>,
    pub is_locked: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct RewritePageRequest {
    pub override_locked: Option<bool>,
}

async fn generate_storybook(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(payload): Json<GenerateStorybookRequest>,
) -> Result<Json<GenerateStorybookResponse>, ApiError> {
    validate_content_type(&payload.content_type)?;
    validate_optional_style_id(payload.style_id.as_deref())?;
    validate_optional_age_group(payload.reading_age_group.as_deref(), "reading_age_group")?;
    if let Some(page_count) = payload.page_count {
        if !(1..=10).contains(&page_count) {
            return Err(ApiError::validation(
                "page_count",
                "页数必须在 1 到 10 之间",
            ));
        }
    }
    let mut state = state.write().expect("state lock poisoned");
    let school_id = state.organization.current_school_id;
    let teacher_id = state.organization.current_teacher_id;
    let idempotency_key = idempotency_key_from_headers(&headers)?;
    let fingerprint = generate_storybook_fingerprint(&payload);
    if let Some(existing) = find_idempotent_storybook(
        &state,
        school_id,
        teacher_id,
        idempotency_key.as_deref(),
        &fingerprint,
    )? {
        return Ok(Json(GenerateStorybookResponse {
            story_task: StoryTaskSummary {
                provider: state.storybooks.story_provider.provider_name().to_string(),
                status: "succeeded".to_string(),
                poll_url: format!("/api/storybooks/{}", existing.id),
            },
            storybook: existing,
        }));
    }
    let source_case = payload
        .case_storybook_id
        .and_then(|case_id| state.content.case_storybooks.get(&case_id).cloned());
    if payload.case_storybook_id.is_some() && source_case.is_none() {
        return Err(ApiError::not_found("case_storybook"));
    }
    if let Some(case_storybook) = source_case.as_ref() {
        if case_storybook.status != "published" {
            return Err(ApiError::not_found("case_storybook"));
        }
        if let Some(template_id) = case_storybook.template_id {
            let template = state
                .content
                .story_templates
                .get(&template_id)
                .ok_or_else(|| ApiError::not_found("story_template"))?;
            if template.status != "active" {
                return Err(ApiError::state_conflict("案例关联模板不是 active"));
            }
        }
    }
    let source_storybook = payload
        .source_storybook_id
        .and_then(|source_id| state.storybooks.storybooks.get(&source_id).cloned());
    if payload.source_storybook_id.is_some() && source_storybook.is_none() {
        return Err(ApiError::not_found("source_storybook"));
    }
    if let Some(source_storybook) = source_storybook.as_ref() {
        if source_storybook.school_id != Some(school_id) {
            return Err(ApiError::forbidden("不能使用其他园所的读本作为来源"));
        }
        if source_storybook.content_type != "plain_storybook" {
            return Err(ApiError::state_conflict(
                "source_storybook_id 必须是普通绘本母本",
            ));
        }
    }
    let child = match payload.child_id {
        Some(child_id) => {
            let child = state
                .children
                .children
                .get(&child_id)
                .cloned()
                .ok_or_else(|| ApiError::not_found("child"))?;
            if child.school_id != Some(school_id) {
                return Err(ApiError::forbidden("不能使用其他园所的儿童档案"));
            }
            Some(child)
        }
        None => None,
    };
    if payload.content_type == "custom_storybook" && child.is_none() {
        return Err(ApiError::validation(
            "child_id",
            "custom_storybook 必须提供 child_id",
        ));
    }

    let title = payload
        .title_override
        .and_then(normalize_optional_owned)
        .or_else(|| source_case.as_ref().map(|case| case.title.clone()))
        .or_else(|| {
            source_storybook
                .as_ref()
                .map(|storybook| storybook.title.clone())
        })
        .unwrap_or_else(|| "新的绘本".to_string());
    let theme = payload
        .theme_override
        .and_then(normalize_optional_owned)
        .or_else(|| source_case.as_ref().map(|case| case.theme.clone()))
        .or_else(|| {
            source_storybook
                .as_ref()
                .map(|storybook| storybook.theme.clone())
        })
        .unwrap_or_else(|| title.clone());
    let has_generation_seed = source_case.is_some()
        || source_storybook.is_some()
        || !theme.trim().is_empty()
        || payload
            .teaching_goal
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
    if !has_generation_seed {
        return Err(ApiError::validation("theme_override", "请先输入故事主题"));
    }
    let teaching_goal = payload
        .teaching_goal
        .and_then(normalize_optional_owned)
        .or_else(|| source_case.as_ref().map(|case| case.teaching_goal.clone()))
        .or_else(|| {
            source_storybook
                .as_ref()
                .and_then(|storybook| storybook.teaching_goal.clone())
        })
        .unwrap_or_else(|| format!("围绕{theme}生成适合幼儿阅读的故事"));
    let page_count = payload
        .page_count
        .or_else(|| source_case.as_ref().map(|case| case.page_count))
        .or_else(|| {
            source_storybook.as_ref().and_then(|storybook| {
                state
                    .storybooks
                    .pages
                    .get(&storybook.id)
                    .map(|pages| pages.len() as i32)
            })
        })
        .unwrap_or(6);
    let page_count = page_count.clamp(1, 10);

    let generated = state
        .storybooks
        .story_provider
        .generate(StoryGenerationInput {
            title: title.clone(),
            content_type: payload.content_type.clone(),
            child_name: child.as_ref().map(|child| child.name.clone()),
            theme: theme.clone(),
            teaching_goal: teaching_goal.clone(),
            reading_age_group: payload.reading_age_group.clone(),
            page_count,
        });

    let created_at = now();
    let storybook_id = Uuid::new_v4();
    let storybook = StorybookRecord {
        id: storybook_id,
        school_id: Some(school_id),
        teacher_id,
        child_id: payload.child_id,
        story_template_id: source_case.as_ref().and_then(|case| case.template_id),
        case_storybook_id: payload.case_storybook_id,
        source_storybook_id: payload.source_storybook_id,
        title: generated.title,
        content_type: payload.content_type,
        theme,
        teaching_goal: Some(teaching_goal),
        style_id: payload.style_id.and_then(normalize_optional_owned),
        reading_age_group: payload.reading_age_group.and_then(normalize_optional_owned),
        generation_config_json: json!({
            "story_provider": state.storybooks.story_provider.provider_name(),
            "options": payload.generation_options,
            "idempotency_key": idempotency_key,
            "idempotency_fingerprint": fingerprint
        }),
        role_manifest_json: generated.role_manifest_json,
        story_status: "story_ready".to_string(),
        illustration_status: "not_started".to_string(),
        status: "ready".to_string(),
        export_status: "not_exported".to_string(),
        share_status: "private".to_string(),
        share_scope: "private".to_string(),
        derivation_type: if payload.source_storybook_id.is_some() {
            "from_plain_storybook".to_string()
        } else if payload.case_storybook_id.is_some() {
            "from_case".to_string()
        } else {
            "from_teacher_brief".to_string()
        },
        created_at,
        updated_at: created_at,
        exported_at: None,
    };
    let pages = generated
        .pages
        .into_iter()
        .enumerate()
        .map(|(index, page)| {
            page_from_generated(storybook_id, (index + 1) as i32, page, created_at)
        })
        .collect::<Vec<_>>();
    state.storybooks.pages.insert(storybook_id, pages);
    state
        .storybooks
        .storybooks
        .insert(storybook_id, storybook.clone());

    Ok(Json(GenerateStorybookResponse {
        story_task: StoryTaskSummary {
            provider: state.storybooks.story_provider.provider_name().to_string(),
            status: "succeeded".to_string(),
            poll_url: format!("/api/storybooks/{storybook_id}"),
        },
        storybook,
    }))
}

async fn list_storybooks(
    State(state): State<SharedState>,
    Query(query): Query<StorybookListQuery>,
) -> Result<Json<ListResponse<StorybookRecord>>, ApiError> {
    validate_optional_status(
        query.status.as_deref(),
        &["draft", "generating", "ready", "exporting", "archived"],
        "status",
    )?;
    validate_optional_content_type(query.content_type.as_deref())?;
    let state = state.read().expect("state lock poisoned");
    if let Some(child_id) = query.child_id {
        let child = state
            .children
            .children
            .get(&child_id)
            .ok_or_else(|| ApiError::not_found("child"))?;
        if child.school_id != Some(state.organization.current_school_id) {
            return Err(ApiError::forbidden("不能访问其他园所的儿童档案"));
        }
    }
    let mut items = state
        .storybooks
        .storybooks
        .values()
        .filter(|storybook| storybook.school_id == Some(state.organization.current_school_id))
        .filter(|storybook| {
            query
                .status
                .as_deref()
                .is_none_or(|status| storybook.status == status)
        })
        .filter(|storybook| {
            query
                .child_id
                .is_none_or(|child_id| storybook.child_id == Some(child_id))
        })
        .filter(|storybook| {
            query
                .content_type
                .as_deref()
                .is_none_or(|content_type| storybook.content_type == content_type)
        })
        .cloned()
        .collect::<Vec<_>>();
    items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(Json(list_response(items)))
}

async fn get_storybook(
    State(state): State<SharedState>,
    Path(storybook_id): Path<Uuid>,
) -> Result<Json<StorybookDetailResponse>, ApiError> {
    let state = state.read().expect("state lock poisoned");
    let storybook = visible_storybook(&state, storybook_id)?.clone();
    Ok(Json(storybook_detail(&state, storybook)))
}

async fn update_storybook(
    State(state): State<SharedState>,
    Path(storybook_id): Path<Uuid>,
    Json(payload): Json<UpdateStorybookRequest>,
) -> Result<Json<StorybookRecord>, ApiError> {
    validate_optional_status(
        payload.status.as_deref(),
        &["draft", "generating", "ready", "exporting", "archived"],
        "status",
    )?;
    validate_optional_status(
        payload.share_scope.as_deref(),
        &[
            "private",
            "family",
            "school",
            "platform_review",
            "platform_public",
        ],
        "share_scope",
    )?;
    let mut state = state.write().expect("state lock poisoned");
    let school_id = state.organization.current_school_id;
    let storybook = state
        .storybooks
        .storybooks
        .get_mut(&storybook_id)
        .ok_or_else(|| ApiError::not_found("storybook"))?;
    if storybook.school_id != Some(school_id) {
        return Err(ApiError::forbidden("不能访问其他园所的读本"));
    }
    if storybook.status == "archived" {
        return Err(ApiError::state_conflict("已归档读本不可编辑"));
    }
    if let Some(title) = payload.title {
        storybook.title = required_trimmed(title, "title")?;
    }
    if let Some(teaching_goal) = payload.teaching_goal {
        storybook.teaching_goal = normalize_optional_owned(teaching_goal);
    }
    if let Some(share_scope) = payload.share_scope {
        validate_manual_share_scope_update(&share_scope)?;
        storybook.share_scope = share_scope;
        if storybook.share_scope == "private" {
            storybook.share_status = "private".to_string();
        }
    }
    if let Some(status) = payload.status {
        storybook.status = status;
    }
    storybook.updated_at = now();
    Ok(Json(storybook.clone()))
}

async fn duplicate_storybook(
    State(state): State<SharedState>,
    Path(storybook_id): Path<Uuid>,
    Json(payload): Json<DuplicateStorybookRequest>,
) -> Result<Json<StorybookRecord>, ApiError> {
    let mut state = state.write().expect("state lock poisoned");
    let source = visible_storybook(&state, storybook_id)?.clone();
    let created_at = now();
    let mut duplicate = source.clone();
    duplicate.id = Uuid::new_v4();
    duplicate.title = payload
        .title_override
        .and_then(normalize_optional_owned)
        .unwrap_or_else(|| format!("{} 副本", source.title));
    duplicate.source_storybook_id = Some(source.id);
    duplicate.derivation_type = "from_custom_storybook".to_string();
    duplicate.share_scope = "private".to_string();
    duplicate.share_status = "private".to_string();
    duplicate.export_status = "not_exported".to_string();
    duplicate.created_at = created_at;
    duplicate.updated_at = created_at;
    duplicate.exported_at = None;
    let pages = state
        .storybooks
        .pages
        .get(&source.id)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|mut page| {
            page.id = Uuid::new_v4();
            page.storybook_id = duplicate.id;
            page.current_image_asset_id = None;
            page.current_image_task_id = None;
            page.created_at = created_at;
            page.updated_at = created_at;
            page
        })
        .collect::<Vec<_>>();
    state.storybooks.pages.insert(duplicate.id, pages);
    state
        .storybooks
        .storybooks
        .insert(duplicate.id, duplicate.clone());
    Ok(Json(duplicate))
}

async fn derive_custom_storybook(
    State(state): State<SharedState>,
    Path(storybook_id): Path<Uuid>,
    Json(payload): Json<DeriveCustomStorybookRequest>,
) -> Result<Json<StorybookRecord>, ApiError> {
    let mut state = state.write().expect("state lock poisoned");
    let school_id = state.organization.current_school_id;
    let source = visible_storybook(&state, storybook_id)?.clone();
    if source.content_type != "plain_storybook" {
        return Err(ApiError::state_conflict("只有普通绘本母本可以派生定制绘本"));
    }
    let child = state
        .children
        .children
        .get(&payload.child_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("child"))?;
    if child.school_id != Some(school_id) {
        return Err(ApiError::forbidden("不能使用其他园所的儿童档案"));
    }
    let created_at = now();
    let mut derived = source.clone();
    derived.id = Uuid::new_v4();
    derived.child_id = Some(child.id);
    derived.content_type = "custom_storybook".to_string();
    derived.title = payload
        .title_override
        .and_then(normalize_optional_owned)
        .unwrap_or_else(|| format!("{}的{}", child.name, source.title));
    derived.source_storybook_id = Some(source.id);
    derived.derivation_type = "from_plain_storybook".to_string();
    derived.role_manifest_json = json!({
        "protagonist": {
            "role_key": "protagonist",
            "role_type": "child",
            "display_name": child.name,
            "child_id": child.id
        }
    });
    derived.illustration_status = "not_started".to_string();
    derived.created_at = created_at;
    derived.updated_at = created_at;
    let pages = state
        .storybooks
        .pages
        .get(&source.id)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|mut page| {
            page.id = Uuid::new_v4();
            page.storybook_id = derived.id;
            page.current_image_asset_id = None;
            page.current_image_task_id = None;
            page.illustration_status = "not_started".to_string();
            page.created_at = created_at;
            page.updated_at = created_at;
            page
        })
        .collect::<Vec<_>>();
    state.storybooks.pages.insert(derived.id, pages);
    state
        .storybooks
        .storybooks
        .insert(derived.id, derived.clone());
    Ok(Json(derived))
}

async fn list_pages(
    State(state): State<SharedState>,
    Path(storybook_id): Path<Uuid>,
) -> Result<Json<ListResponse<StorybookPageRecord>>, ApiError> {
    let state = state.read().expect("state lock poisoned");
    visible_storybook(&state, storybook_id)?;
    let mut pages = state
        .storybooks
        .pages
        .get(&storybook_id)
        .cloned()
        .unwrap_or_default();
    pages.sort_by_key(|page| page.page_number);
    Ok(Json(list_response(pages)))
}

async fn add_page(
    State(state): State<SharedState>,
    Path(storybook_id): Path<Uuid>,
    Json(payload): Json<AddPageRequest>,
) -> Result<Json<StorybookPageRecord>, ApiError> {
    let body_text = required_trimmed_max(payload.body_text, "body_text", 800)?;
    let page_title = normalize_optional_max(payload.page_title, "page_title", 60)?;
    let prompt_text = normalize_optional_max(payload.prompt_text, "prompt_text", 200)?;
    let teacher_tip = normalize_optional_max(payload.teacher_tip, "teacher_tip", 300)?;
    validate_page_role(payload.page_role.as_deref().unwrap_or("story"))?;
    validate_optional_status(
        payload.scene_spec_status.as_deref(),
        &["missing", "draft", "ready"],
        "scene_spec_status",
    )?;
    if payload.scene_spec_status.as_deref() == Some("ready") {
        validate_scene_spec(payload.scene_spec_json.as_ref())?;
    }
    let mut state = state.write().expect("state lock poisoned");
    visible_storybook(&state, storybook_id)?;
    let created_at = now();
    let pages = state.storybooks.pages.entry(storybook_id).or_default();
    let page_number = payload.page_number.unwrap_or((pages.len() + 1) as i32);
    if page_number < 1 {
        return Err(ApiError::validation("page_number", "页码必须大于 0"));
    }
    shift_pages_for_insert(pages, page_number);
    let page = StorybookPageRecord {
        id: Uuid::new_v4(),
        storybook_id,
        page_number,
        page_role: payload.page_role.unwrap_or_else(|| "story".to_string()),
        page_title,
        body_text,
        prompt_text,
        teacher_tip,
        scene_spec_json: payload.scene_spec_json,
        scene_spec_status: payload
            .scene_spec_status
            .unwrap_or_else(|| "missing".to_string()),
        page_visual_subjects_json: None,
        current_image_asset_id: None,
        current_image_task_id: None,
        illustration_status: "not_started".to_string(),
        is_locked: false,
        content_source: "manual_edit".to_string(),
        created_at,
        updated_at: created_at,
    };
    pages.push(page.clone());
    pages.sort_by_key(|page| page.page_number);
    touch_storybook(&mut state, storybook_id);
    Ok(Json(page))
}

async fn update_page(
    State(state): State<SharedState>,
    Path((storybook_id, page_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdatePageRequest>,
) -> Result<Json<StorybookPageRecord>, ApiError> {
    let page_title_provided = payload.page_title.is_some();
    let prompt_text_provided = payload.prompt_text.is_some();
    let teacher_tip_provided = payload.teacher_tip.is_some();
    let page_title = normalize_optional_max(payload.page_title, "page_title", 60)?;
    let body_text = payload
        .body_text
        .map(|body_text| required_trimmed_max(body_text, "body_text", 800))
        .transpose()?;
    let prompt_text = normalize_optional_max(payload.prompt_text, "prompt_text", 200)?;
    let teacher_tip = normalize_optional_max(payload.teacher_tip, "teacher_tip", 300)?;
    validate_optional_status(
        payload.scene_spec_status.as_deref(),
        &["missing", "draft", "ready"],
        "scene_spec_status",
    )?;
    if payload.scene_spec_status.as_deref() == Some("ready") {
        validate_scene_spec(payload.scene_spec_json.as_ref())?;
    }
    let mut state = state.write().expect("state lock poisoned");
    visible_storybook(&state, storybook_id)?;
    let pages = state
        .storybooks
        .pages
        .get_mut(&storybook_id)
        .ok_or_else(|| ApiError::not_found("storybook_page"))?;
    let page = pages
        .iter_mut()
        .find(|page| page.id == page_id)
        .ok_or_else(|| ApiError::not_found("storybook_page"))?;
    let content_affects_image = page_title_provided
        || body_text.is_some()
        || prompt_text_provided
        || teacher_tip_provided
        || payload.scene_spec_json.is_some()
        || payload.scene_spec_status.is_some();
    if page_title_provided {
        page.page_title = page_title;
    }
    if let Some(body_text) = body_text {
        page.body_text = body_text;
    }
    if prompt_text_provided {
        page.prompt_text = prompt_text;
    }
    if teacher_tip_provided {
        page.teacher_tip = teacher_tip;
    }
    if let Some(scene_spec_json) = payload.scene_spec_json {
        page.scene_spec_json = Some(scene_spec_json);
    }
    if let Some(scene_spec_status) = payload.scene_spec_status {
        page.scene_spec_status = scene_spec_status;
    }
    if let Some(is_locked) = payload.is_locked {
        page.is_locked = is_locked;
    }
    if content_affects_image {
        mark_page_image_stale(page);
    }
    page.content_source = "manual_edit".to_string();
    page.updated_at = now();
    let page = page.clone();
    touch_storybook(&mut state, storybook_id);
    Ok(Json(page))
}

async fn delete_page(
    State(state): State<SharedState>,
    Path((storybook_id, page_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ListResponse<StorybookPageRecord>>, ApiError> {
    let mut state = state.write().expect("state lock poisoned");
    visible_storybook(&state, storybook_id)?;
    let pages = state
        .storybooks
        .pages
        .get_mut(&storybook_id)
        .ok_or_else(|| ApiError::not_found("storybook_page"))?;
    if pages.len() <= 1 {
        return Err(ApiError::state_conflict("不能删除最后一页"));
    }
    let original_len = pages.len();
    pages.retain(|page| page.id != page_id);
    if pages.len() == original_len {
        return Err(ApiError::not_found("storybook_page"));
    }
    renumber_pages(pages);
    let items = pages.clone();
    state.visuals.page_visual_subjects.remove(&page_id);
    touch_storybook(&mut state, storybook_id);
    Ok(Json(list_response(items)))
}

async fn rewrite_page(
    State(state): State<SharedState>,
    Path((storybook_id, page_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<RewritePageRequest>,
) -> Result<Json<StorybookPageRecord>, ApiError> {
    let mut state = state.write().expect("state lock poisoned");
    let storybook = visible_storybook(&state, storybook_id)?.clone();
    let page_snapshot = state
        .storybooks
        .pages
        .get(&storybook_id)
        .and_then(|pages| pages.iter().find(|page| page.id == page_id))
        .cloned()
        .ok_or_else(|| ApiError::not_found("storybook_page"))?;
    if page_snapshot.is_locked && payload.override_locked != Some(true) {
        return Err(ApiError::state_conflict("锁定页不能自动重写"));
    }
    let rewritten = state
        .storybooks
        .story_provider
        .rewrite_page(PageRewriteInput {
            title: storybook.title.clone(),
            page_number: page_snapshot.page_number,
            teaching_goal: storybook.teaching_goal.clone().unwrap_or_default(),
            original_body_text: page_snapshot.body_text,
        });
    let pages = state.storybooks.pages.get_mut(&storybook_id).unwrap();
    let page = pages.iter_mut().find(|page| page.id == page_id).unwrap();
    page.page_title = rewritten.page_title;
    page.body_text = rewritten.body_text;
    page.prompt_text = rewritten.prompt_text;
    page.teacher_tip = rewritten.teacher_tip;
    page.scene_spec_json = Some(rewritten.scene_spec_json);
    page.scene_spec_status = "ready".to_string();
    mark_page_image_stale(page);
    page.content_source = "generated".to_string();
    page.updated_at = now();
    let page = page.clone();
    touch_storybook(&mut state, storybook_id);
    Ok(Json(page))
}

fn deterministic_story(provider_name: &str, input: StoryGenerationInput) -> StoryGenerationOutput {
    let protagonist = input.child_name.unwrap_or_else(|| "小朋友".to_string());
    let page_count = input.page_count.max(1);
    let pages = (1..=page_count)
        .map(|page_number| {
            let page_role = if page_number == 1 {
                "cover"
            } else if page_number == page_count {
                "closing"
            } else {
                "story"
            };
            StoryGeneratedPage {
                page_role: page_role.to_string(),
                page_title: Some(if page_number == 1 {
                    input.title.clone()
                } else {
                    format!("{} 第 {} 页", input.theme, page_number)
                }),
                body_text: format!(
                    "{}在{}的故事里练习{}，这是由 {} 生成的第 {} 页。",
                    protagonist, input.theme, input.teaching_goal, provider_name, page_number
                ),
                prompt_text: Some("你愿意说说自己会怎么做吗？".to_string()),
                teacher_tip: Some(format!("围绕{}引导孩子表达想法。", input.teaching_goal)),
                scene_spec_json: json!({
                    "location": "幼儿园教室",
                    "action": format!("{}参与{}", protagonist, input.theme),
                    "composition": "儿童绘本中景构图",
                    "emotion": "温暖、积极"
                }),
            }
        })
        .collect::<Vec<_>>();
    StoryGenerationOutput {
        title: input.title,
        pages,
        role_manifest_json: json!({
            "protagonist": {
                "role_key": "protagonist",
                "role_type": if input.content_type == "custom_storybook" { "child" } else { "default_character" },
                "display_name": protagonist
            },
            "provider": provider_name,
            "reading_age_group": input.reading_age_group
        }),
    }
}

fn deterministic_rewrite(provider_name: &str, input: PageRewriteInput) -> StoryGeneratedPage {
    StoryGeneratedPage {
        page_role: "story".to_string(),
        page_title: Some(format!("{} 第 {} 页新版", input.title, input.page_number)),
        body_text: format!(
            "{}。{} 已根据教学目标“{}”重新生成。",
            input.original_body_text, provider_name, input.teaching_goal
        ),
        prompt_text: Some("你还可以想到什么办法？".to_string()),
        teacher_tip: Some("重写页建议再次确认场景是否适合插图。".to_string()),
        scene_spec_json: json!({
            "location": "幼儿园教室",
            "action": "孩子完成新的故事动作",
            "composition": "简洁稳定构图"
        }),
    }
}

#[derive(Debug, Deserialize)]
struct DeepSeekStoryResponse {
    title: Option<String>,
    pages: Vec<DeepSeekStoryPage>,
    #[serde(default)]
    role_manifest_json: Value,
}

#[derive(Debug, Deserialize)]
struct DeepSeekStoryPage {
    page_role: Option<String>,
    page_title: Option<String>,
    body_text: String,
    prompt_text: Option<String>,
    teacher_tip: Option<String>,
    #[serde(default)]
    scene_spec_json: Value,
}

#[derive(Debug, Deserialize)]
struct DeepSeekRewriteResponse {
    page_title: Option<String>,
    body_text: String,
    prompt_text: Option<String>,
    teacher_tip: Option<String>,
    #[serde(default)]
    scene_spec_json: Value,
}

fn parse_deepseek_story_output(
    content: &str,
    expected_page_count: i32,
    fallback_title: String,
) -> Option<StoryGenerationOutput> {
    let json_content = strip_json_markdown(content);
    let response = serde_json::from_str::<DeepSeekStoryResponse>(&json_content).ok()?;
    if response.pages.len() != expected_page_count.max(1) as usize {
        return None;
    }
    let last_index = response.pages.len().saturating_sub(1);
    let pages = response
        .pages
        .into_iter()
        .enumerate()
        .map(|(index, page)| {
            let fallback_role = if index == 0 {
                "cover"
            } else if index == last_index {
                "closing"
            } else {
                "story"
            };
            StoryGeneratedPage {
                page_role: page
                    .page_role
                    .and_then(normalize_optional_owned)
                    .unwrap_or_else(|| fallback_role.to_string()),
                page_title: page.page_title.and_then(normalize_optional_owned),
                body_text: page.body_text,
                prompt_text: page.prompt_text.and_then(normalize_optional_owned),
                teacher_tip: page.teacher_tip.and_then(normalize_optional_owned),
                scene_spec_json: if page.scene_spec_json.is_null() {
                    json!({})
                } else {
                    page.scene_spec_json
                },
            }
        })
        .collect::<Vec<_>>();
    Some(StoryGenerationOutput {
        title: response
            .title
            .and_then(normalize_optional_owned)
            .unwrap_or(fallback_title),
        pages,
        role_manifest_json: if response.role_manifest_json.is_null() {
            json!({})
        } else {
            response.role_manifest_json
        },
    })
}

fn parse_deepseek_rewrite_output(content: &str) -> Option<StoryGeneratedPage> {
    let json_content = strip_json_markdown(content);
    let response = serde_json::from_str::<DeepSeekRewriteResponse>(&json_content).ok()?;
    Some(StoryGeneratedPage {
        page_role: "story".to_string(),
        page_title: response.page_title.and_then(normalize_optional_owned),
        body_text: response.body_text,
        prompt_text: response.prompt_text.and_then(normalize_optional_owned),
        teacher_tip: response.teacher_tip.and_then(normalize_optional_owned),
        scene_spec_json: if response.scene_spec_json.is_null() {
            json!({})
        } else {
            response.scene_spec_json
        },
    })
}

fn strip_json_markdown(content: &str) -> String {
    let trimmed = content.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }
    let without_start = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .trim();
    without_start
        .strip_suffix("```")
        .unwrap_or(without_start)
        .trim()
        .to_string()
}

fn storybook_detail(
    state: &crate::api::AppState,
    storybook: StorybookRecord,
) -> StorybookDetailResponse {
    let child = storybook.child_id.and_then(|child_id| {
        state
            .children
            .children
            .get(&child_id)
            .map(|child| ChildSummary {
                id: child.id,
                name: child.name.clone(),
                profile_completion_status: child.profile_completion_status.clone(),
            })
    });
    let source_case = storybook.case_storybook_id.and_then(|case_id| {
        state
            .content
            .case_storybooks
            .get(&case_id)
            .map(|case| CaseSummary {
                id: case.id,
                title: case.title.clone(),
                theme: case.theme.clone(),
            })
    });
    let mut pages = state
        .storybooks
        .pages
        .get(&storybook.id)
        .cloned()
        .unwrap_or_default();
    pages.sort_by_key(|page| page.page_number);
    StorybookDetailResponse {
        storybook,
        child,
        source_case,
        pages,
    }
}

fn visible_storybook(
    state: &crate::api::AppState,
    storybook_id: Uuid,
) -> Result<&StorybookRecord, ApiError> {
    let storybook = state
        .storybooks
        .storybooks
        .get(&storybook_id)
        .ok_or_else(|| ApiError::not_found("storybook"))?;
    if storybook.school_id != Some(state.organization.current_school_id) {
        return Err(ApiError::forbidden("不能访问其他园所的读本"));
    }
    Ok(storybook)
}

fn page_from_generated(
    storybook_id: Uuid,
    page_number: i32,
    page: StoryGeneratedPage,
    created_at: DateTime<Utc>,
) -> StorybookPageRecord {
    StorybookPageRecord {
        id: Uuid::new_v4(),
        storybook_id,
        page_number,
        page_role: page.page_role,
        page_title: page.page_title,
        body_text: page.body_text,
        prompt_text: page.prompt_text,
        teacher_tip: page.teacher_tip,
        scene_spec_json: Some(page.scene_spec_json),
        scene_spec_status: "ready".to_string(),
        page_visual_subjects_json: None,
        current_image_asset_id: None,
        current_image_task_id: None,
        illustration_status: "not_started".to_string(),
        is_locked: false,
        content_source: "generated".to_string(),
        created_at,
        updated_at: created_at,
    }
}

fn validate_content_type(content_type: &str) -> Result<(), ApiError> {
    if ["plain_storybook", "custom_storybook"].contains(&content_type) {
        Ok(())
    } else {
        Err(ApiError::validation("content_type", "内容类型枚举不合法"))
    }
}

fn validate_optional_content_type(content_type: Option<&str>) -> Result<(), ApiError> {
    if let Some(content_type) = content_type {
        validate_content_type(content_type)?;
    }
    Ok(())
}

fn validate_optional_style_id(style_id: Option<&str>) -> Result<(), ApiError> {
    if let Some(style_id) = style_id {
        if !["storybook_flat_v1", "watercolor_soft_v1"].contains(&style_id.trim()) {
            return Err(ApiError::validation("style_id", "画风不支持"));
        }
    }
    Ok(())
}

fn validate_optional_age_group(
    age_group: Option<&str>,
    field: &'static str,
) -> Result<(), ApiError> {
    if let Some(age_group) = age_group {
        if !["3-4", "4-5", "5-6", "6-7"].contains(&age_group.trim()) {
            return Err(ApiError::validation(field, "年龄段不支持"));
        }
    }
    Ok(())
}

fn validate_page_role(page_role: &str) -> Result<(), ApiError> {
    if ["cover", "story", "closing"].contains(&page_role) {
        Ok(())
    } else {
        Err(ApiError::validation("page_role", "页面角色枚举不合法"))
    }
}

fn validate_scene_spec(scene_spec_json: Option<&Value>) -> Result<(), ApiError> {
    let scene = scene_spec_json
        .and_then(Value::as_object)
        .ok_or_else(|| ApiError::validation("scene_spec_json", "ready 场景必须是对象"))?;
    let required_count = ["location", "action", "composition"]
        .iter()
        .filter(|key| {
            scene
                .get(**key)
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty())
        })
        .count();
    if required_count < 2 {
        return Err(ApiError::validation(
            "scene_spec_json",
            "ready 场景至少包含 location/action/composition 中两项",
        ));
    }
    Ok(())
}

fn validate_optional_status(
    value: Option<&str>,
    allowed: &[&str],
    field: &'static str,
) -> Result<(), ApiError> {
    if let Some(value) = value {
        if !allowed.contains(&value) {
            return Err(ApiError::validation(field, "状态枚举不合法"));
        }
    }
    Ok(())
}

fn validate_manual_share_scope_update(share_scope: &str) -> Result<(), ApiError> {
    if share_scope == "platform_review" || share_scope == "platform_public" {
        return Err(ApiError::state_conflict(
            "平台审核和公开共享必须通过审核流程变更",
        ));
    }
    if share_scope == "school" || share_scope == "family" {
        return Err(ApiError::state_conflict("分享范围必须通过分享链接接口变更"));
    }
    Ok(())
}

fn find_idempotent_storybook(
    state: &crate::api::AppState,
    school_id: Uuid,
    teacher_id: Uuid,
    idempotency_key: Option<&str>,
    fingerprint: &Value,
) -> Result<Option<StorybookRecord>, ApiError> {
    let Some(idempotency_key) = idempotency_key else {
        return Ok(None);
    };
    let existing = state.storybooks.storybooks.values().find(|storybook| {
        storybook.school_id == Some(school_id)
            && storybook.teacher_id == teacher_id
            && storybook
                .generation_config_json
                .get("idempotency_key")
                .and_then(Value::as_str)
                == Some(idempotency_key)
    });
    if let Some(storybook) = existing {
        if storybook
            .generation_config_json
            .get("idempotency_fingerprint")
            == Some(fingerprint)
        {
            return Ok(Some(storybook.clone()));
        }
        return Err(idempotency_conflict());
    }
    Ok(None)
}

fn generate_storybook_fingerprint(payload: &GenerateStorybookRequest) -> Value {
    json!({
        "content_type": payload.content_type,
        "child_id": payload.child_id,
        "case_storybook_id": payload.case_storybook_id,
        "source_storybook_id": payload.source_storybook_id,
        "title_override": payload.title_override,
        "theme_override": payload.theme_override,
        "style_id": payload.style_id,
        "reading_age_group": payload.reading_age_group,
        "teaching_goal": payload.teaching_goal,
        "page_count": payload.page_count,
        "generation_options": payload.generation_options
    })
}

fn idempotency_key_from_headers(headers: &HeaderMap) -> Result<Option<String>, ApiError> {
    let Some(value) = headers.get("idempotency-key") else {
        return Ok(None);
    };
    let value = value
        .to_str()
        .map_err(|_| ApiError::validation("Idempotency-Key", "幂等键必须是有效字符串"))?
        .trim();
    if value.is_empty() {
        return Err(ApiError::validation("Idempotency-Key", "幂等键不能为空"));
    }
    Ok(Some(value.to_string()))
}

fn idempotency_conflict() -> ApiError {
    ApiError {
        status: axum::http::StatusCode::CONFLICT,
        code: "IDEMPOTENCY_CONFLICT",
        message: "同一个 Idempotency-Key 的请求参数不一致".to_string(),
        details: vec![],
    }
}

fn required_trimmed(value: String, field: &'static str) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ApiError::validation(field, "不能为空"));
    }
    Ok(value.to_string())
}

fn required_trimmed_max(
    value: String,
    field: &'static str,
    max_chars: usize,
) -> Result<String, ApiError> {
    let value = required_trimmed(value, field)?;
    if value.chars().count() > max_chars {
        return Err(ApiError::validation(
            field,
            format!("长度不能超过 {max_chars} 个字符"),
        ));
    }
    Ok(value)
}

fn normalize_optional_owned(value: String) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn normalize_optional_max(
    value: Option<String>,
    field: &'static str,
    max_chars: usize,
) -> Result<Option<String>, ApiError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Some(value) = normalize_optional_owned(value) else {
        return Ok(None);
    };
    if value.chars().count() > max_chars {
        return Err(ApiError::validation(
            field,
            format!("长度不能超过 {max_chars} 个字符"),
        ));
    }
    Ok(Some(value))
}

fn shift_pages_for_insert(pages: &mut [StorybookPageRecord], page_number: i32) {
    for page in pages {
        if page.page_number >= page_number {
            page.page_number += 1;
        }
    }
}

fn renumber_pages(pages: &mut [StorybookPageRecord]) {
    pages.sort_by_key(|page| page.page_number);
    for (index, page) in pages.iter_mut().enumerate() {
        page.page_number = (index + 1) as i32;
    }
}

fn touch_storybook(state: &mut crate::api::AppState, storybook_id: Uuid) {
    if let Some(storybook) = state.storybooks.storybooks.get_mut(&storybook_id) {
        storybook.updated_at = now();
        let pages = state
            .storybooks
            .pages
            .get(&storybook_id)
            .cloned()
            .unwrap_or_default();
        if pages.iter().any(|page| {
            page.illustration_status == "not_started" && page.current_image_asset_id.is_none()
        }) {
            storybook.illustration_status = "not_started".to_string();
        }
    }
}

fn mark_page_image_stale(page: &mut StorybookPageRecord) {
    page.current_image_asset_id = None;
    page.current_image_task_id = None;
    page.illustration_status = "not_started".to_string();
}

fn list_response<T>(items: Vec<T>) -> ListResponse<T> {
    let total = items.len();
    ListResponse {
        items,
        page: 1,
        page_size: total as u32,
        total,
    }
}

#[cfg(test)]
mod tests {
    use super::{TestStoryProvider, StoryProviderKind};
    use crate::api::{AppState, router};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::{Value, json};
    use std::sync::{Arc, RwLock};
    use tower::ServiceExt;
    use uuid::Uuid;

    fn test_state() -> Arc<RwLock<AppState>> {
        let mut state = AppState::test_fixture();
        state.storybooks.story_provider = StoryProviderKind::Test(TestStoryProvider);
        Arc::new(RwLock::new(state))
    }

    fn test_app() -> axum::Router {
        router(test_state())
    }

    fn test_app_with_state(state: AppState) -> axum::Router {
        router(Arc::new(RwLock::new(state)))
    }

    async fn request_json(
        app: axum::Router,
        method: &str,
        uri: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .header(
                "authorization",
                format!("Bearer {}", crate::api::auth::TEST_BEARER_TOKEN),
            )
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    async fn request_json_with_idempotency_key(
        app: axum::Router,
        method: &str,
        uri: &str,
        body: Value,
        idempotency_key: &str,
    ) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .header(
                "authorization",
                format!("Bearer {}", crate::api::auth::TEST_BEARER_TOKEN),
            )
            .header("Idempotency-Key", idempotency_key)
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    async fn get_json(app: axum::Router, uri: &str) -> (StatusCode, Value) {
        let request = Request::builder()
            .method("GET")
            .header(
                "authorization",
                format!("Bearer {}", crate::api::auth::TEST_BEARER_TOKEN),
            )
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    async fn create_plain_storybook(app: axum::Router) -> Value {
        let (_, cases) = get_json(app.clone(), "/api/cases").await;
        let case_id = cases["items"][0]["id"].as_str().unwrap();
        let (status, body) = request_json(
            app,
            "POST",
            "/api/storybooks/generate",
            json!({
                "content_type": "plain_storybook",
                "case_storybook_id": case_id,
                "title_override": "班级分享故事",
                "style_id": "storybook_flat_v1",
                "reading_age_group": "5-6"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        body
    }

    #[tokio::test]
    async fn generates_storybook_with_test_provider_contract() {
        let app = test_app();
        let body = create_plain_storybook(app.clone()).await;
        assert_eq!(body["storybook"]["story_status"], "story_ready");
        assert_eq!(body["story_task"]["provider"], "test_story_provider");
        let storybook_id = body["storybook"]["id"].as_str().unwrap();

        let (status, detail) = get_json(app, &format!("/api/storybooks/{storybook_id}")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(detail["pages"].as_array().unwrap().len(), 6);
        assert_eq!(detail["pages"][0]["scene_spec_status"], "ready");
    }

    #[tokio::test]
    async fn generates_storybook_from_teacher_brief_without_case() {
        let app = test_app();
        let child_id = crate::commons::test_uuid(10);
        let (status, body) = request_json(
            app.clone(),
            "POST",
            "/api/storybooks/generate",
            json!({
                "content_type": "custom_storybook",
                "child_id": child_id,
                "title_override": "乐乐的轮流故事",
                "theme_override": "第一次在幼儿园轮流玩滑梯",
                "teaching_goal": "帮助孩子理解等待、表达和轮流",
                "style_id": "storybook_flat_v1",
                "reading_age_group": "5-6",
                "page_count": 4,
                "generation_options": {
                    "source": "teacher_brief"
                }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["storybook"]["derivation_type"], "from_teacher_brief");
        assert_eq!(body["storybook"]["theme"], "第一次在幼儿园轮流玩滑梯");

        let storybook_id = body["storybook"]["id"].as_str().unwrap();
        let (status, pages) = get_json(app, &format!("/api/storybooks/{storybook_id}/pages")).await;
        assert_eq!(status, StatusCode::OK, "{pages}");
        assert_eq!(pages["total"], 4);
    }

    #[tokio::test]
    async fn generate_storybook_is_idempotent_per_key_and_payload() {
        let app = test_app();
        let (_, cases) = get_json(app.clone(), "/api/cases").await;
        let case_id = cases["items"][0]["id"].as_str().unwrap();
        let payload = json!({
            "content_type": "plain_storybook",
            "case_storybook_id": case_id,
            "title_override": "幂等生成故事",
            "style_id": "storybook_flat_v1",
            "reading_age_group": "5-6"
        });
        let (status, first) = request_json_with_idempotency_key(
            app.clone(),
            "POST",
            "/api/storybooks/generate",
            payload.clone(),
            "story-key-1",
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{first}");
        let (status, second) = request_json_with_idempotency_key(
            app.clone(),
            "POST",
            "/api/storybooks/generate",
            payload,
            "story-key-1",
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{second}");
        assert_eq!(second["storybook"]["id"], first["storybook"]["id"]);
        assert_eq!(
            second["story_task"]["poll_url"],
            format!(
                "/api/storybooks/{}",
                first["storybook"]["id"].as_str().unwrap()
            )
        );

        let (status, conflict) = request_json_with_idempotency_key(
            app,
            "POST",
            "/api/storybooks/generate",
            json!({
                "content_type": "plain_storybook",
                "case_storybook_id": case_id,
                "title_override": "另一个标题",
                "style_id": "storybook_flat_v1",
                "reading_age_group": "5-6"
            }),
            "story-key-1",
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
        assert_eq!(conflict["error"]["code"], "IDEMPOTENCY_CONFLICT");
    }

    #[tokio::test]
    async fn rejects_custom_storybook_without_child() {
        let app = test_app();
        let (_, cases) = get_json(app.clone(), "/api/cases").await;
        let case_id = cases["items"][0]["id"].as_str().unwrap();
        let (status, body) = request_json(
            app,
            "POST",
            "/api/storybooks/generate",
            json!({
                "content_type": "custom_storybook",
                "case_storybook_id": case_id
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["details"][0]["field"], "child_id");
    }

    #[tokio::test]
    async fn rejects_unsupported_generation_style_and_age_group() {
        let app = test_app();
        let (_, cases) = get_json(app.clone(), "/api/cases").await;
        let case_id = cases["items"][0]["id"].as_str().unwrap();
        let (status, body) = request_json(
            app.clone(),
            "POST",
            "/api/storybooks/generate",
            json!({
                "content_type": "plain_storybook",
                "case_storybook_id": case_id,
                "style_id": "unknown_style"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["details"][0]["field"], "style_id");

        let (status, body) = request_json(
            app,
            "POST",
            "/api/storybooks/generate",
            json!({
                "content_type": "plain_storybook",
                "case_storybook_id": case_id,
                "reading_age_group": "9-10"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["details"][0]["field"], "reading_age_group");
    }

    #[tokio::test]
    async fn rejects_generation_from_inactive_case_template() {
        let mut state = AppState::test_fixture();
        state.storybooks.story_provider = StoryProviderKind::Test(TestStoryProvider);
        let template_id = crate::commons::test_uuid(30);
        state
            .content
            .story_templates
            .get_mut(&template_id)
            .unwrap()
            .status = "archived".to_string();
        let app = test_app_with_state(state);
        let case_id = crate::commons::test_uuid(31);
        let (status, body) = request_json(
            app,
            "POST",
            "/api/storybooks/generate",
            json!({
                "content_type": "plain_storybook",
                "case_storybook_id": case_id
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["code"], "STATE_CONFLICT");
    }

    #[tokio::test]
    async fn rejects_custom_storybook_as_generation_source() {
        let app = test_app();
        let (_, cases) = get_json(app.clone(), "/api/cases").await;
        let case_id = cases["items"][0]["id"].as_str().unwrap();
        let (_, children) = get_json(app.clone(), "/api/children").await;
        let child_id = children["items"][0]["id"].as_str().unwrap();
        let (status, created) = request_json(
            app.clone(),
            "POST",
            "/api/storybooks/generate",
            json!({
                "content_type": "custom_storybook",
                "child_id": child_id,
                "case_storybook_id": case_id
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{created}");
        let source_storybook_id = created["storybook"]["id"].as_str().unwrap();
        let (status, body) = request_json(
            app,
            "POST",
            "/api/storybooks/generate",
            json!({
                "content_type": "plain_storybook",
                "source_storybook_id": source_storybook_id
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["code"], "STATE_CONFLICT");
    }

    #[tokio::test]
    async fn edits_pages_and_rejects_invalid_ready_scene() {
        let app = test_app();
        let body = create_plain_storybook(app.clone()).await;
        let storybook_id = body["storybook"]["id"].as_str().unwrap();
        let (status, page) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybooks/{storybook_id}/pages"),
            json!({
                "body_text": "新加的一页",
                "scene_spec_status": "ready",
                "scene_spec_json": {"location": "教室"}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(page["error"]["details"][0]["field"], "scene_spec_json");

        let (status, page) = request_json(
            app,
            "POST",
            &format!("/api/storybooks/{storybook_id}/pages"),
            json!({
                "body_text": "新加的一页",
                "scene_spec_status": "ready",
                "scene_spec_json": {
                    "location": "教室",
                    "action": "一起分享"
                }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(page["content_source"], "manual_edit");
    }

    #[tokio::test]
    async fn validates_page_text_field_lengths() {
        let app = test_app();
        let body = create_plain_storybook(app.clone()).await;
        let storybook_id = body["storybook"]["id"].as_str().unwrap();
        let (_, detail) = get_json(app.clone(), &format!("/api/storybooks/{storybook_id}")).await;
        let page_id = detail["pages"][0]["id"].as_str().unwrap();

        let (status, body) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybooks/{storybook_id}/pages"),
            json!({ "body_text": "字".repeat(801) }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["details"][0]["field"], "body_text");

        for (field, value) in [
            ("page_title", "题".repeat(61)),
            ("prompt_text", "图".repeat(201)),
            ("teacher_tip", "提".repeat(301)),
        ] {
            let (status, body) = request_json(
                app.clone(),
                "PATCH",
                &format!("/api/storybooks/{storybook_id}/pages/{page_id}"),
                json!({ field: value }),
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
            assert_eq!(body["error"]["details"][0]["field"], field);
        }
    }

    #[tokio::test]
    async fn blocks_manual_share_scope_escalation() {
        let app = test_app();
        let body = create_plain_storybook(app.clone()).await;
        let storybook_id = body["storybook"]["id"].as_str().unwrap();
        let (status, body) = request_json(
            app,
            "PATCH",
            &format!("/api/storybooks/{storybook_id}"),
            json!({ "share_scope": "platform_public" }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["code"], "STATE_CONFLICT");
    }

    #[tokio::test]
    async fn page_content_edits_mark_existing_image_stale() {
        let app = test_app();
        let body = create_plain_storybook(app.clone()).await;
        let storybook_id = body["storybook"]["id"].as_str().unwrap();
        let (_, detail) = get_json(app.clone(), &format!("/api/storybooks/{storybook_id}")).await;
        let page_id = detail["pages"][0]["id"].as_str().unwrap();
        let (status, task) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybook-pages/{page_id}/image-tasks"),
            json!({
                "style_id": "storybook_flat_v1",
                "prompt_template_version": "page_image_v1"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{task}");

        let (_, detail) = get_json(app.clone(), &format!("/api/storybooks/{storybook_id}")).await;
        assert!(detail["pages"][0]["current_image_task_id"].is_string());
        assert_eq!(detail["pages"][0]["illustration_status"], "needs_review");

        let (status, page) = request_json(
            app.clone(),
            "PATCH",
            &format!("/api/storybooks/{storybook_id}/pages/{page_id}"),
            json!({ "body_text": "老师手动改过的新正文" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert_eq!(page["current_image_asset_id"], Value::Null);
        assert_eq!(page["current_image_task_id"], Value::Null);
        assert_eq!(page["illustration_status"], "not_started");

        let (_, detail) = get_json(app, &format!("/api/storybooks/{storybook_id}")).await;
        assert_eq!(detail["illustration_status"], "not_started");
    }

    #[tokio::test]
    async fn deleting_page_removes_visual_subject_bindings() {
        let state = test_state();
        let app = router(state.clone());
        let body = create_plain_storybook(app.clone()).await;
        let storybook_id = body["storybook"]["id"].as_str().unwrap();
        let (_, roles) = get_json(
            app.clone(),
            &format!("/api/storybooks/{storybook_id}/roles"),
        )
        .await;
        let role_id = roles["items"][0]["id"].as_str().unwrap();
        let (_, detail) = get_json(app.clone(), &format!("/api/storybooks/{storybook_id}")).await;
        let page_id = detail["pages"][0]["id"].as_str().unwrap();

        let (status, subjects) = request_json(
            app.clone(),
            "PUT",
            &format!("/api/storybook-pages/{page_id}/visual-subjects"),
            json!({
                "subjects": [{
                    "subject_type": "storybook_role",
                    "storybook_role_id": role_id,
                    "importance": "primary"
                }]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{subjects}");
        let page_id = Uuid::parse_str(page_id).unwrap();
        assert!(
            state
                .read()
                .unwrap()
                .visuals
                .page_visual_subjects
                .contains_key(&page_id)
        );

        let (status, pages) = request_json(
            app,
            "DELETE",
            &format!("/api/storybooks/{storybook_id}/pages/{page_id}"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pages}");
        assert!(
            !state
                .read()
                .unwrap()
                .visuals
                .page_visual_subjects
                .contains_key(&page_id)
        );
    }

    #[tokio::test]
    async fn rewrite_respects_locked_page() {
        let app = test_app();
        let body = create_plain_storybook(app.clone()).await;
        let storybook_id = body["storybook"]["id"].as_str().unwrap();
        let (_, detail) = get_json(app.clone(), &format!("/api/storybooks/{storybook_id}")).await;
        let page_id = detail["pages"][0]["id"].as_str().unwrap();
        let (status, _) = request_json(
            app.clone(),
            "PATCH",
            &format!("/api/storybooks/{storybook_id}/pages/{page_id}"),
            json!({ "is_locked": true }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, body) = request_json(
            app.clone(),
            "POST",
            &format!("/api/storybooks/{storybook_id}/pages/{page_id}/rewrite"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"]["code"], "STATE_CONFLICT");

        let (status, body) = request_json(
            app,
            "POST",
            &format!("/api/storybooks/{storybook_id}/pages/{page_id}/rewrite"),
            json!({ "override_locked": true }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["content_source"], "generated");
    }

    #[tokio::test]
    async fn derives_custom_storybook_from_plain_mother_book() {
        let app = test_app();
        let body = create_plain_storybook(app.clone()).await;
        let storybook_id = body["storybook"]["id"].as_str().unwrap();
        let (_, children) = get_json(app.clone(), "/api/children").await;
        let child_id = children["items"][0]["id"].as_str().unwrap();
        let (status, derived) = request_json(
            app,
            "POST",
            &format!("/api/storybooks/{storybook_id}/derive-custom"),
            json!({
                "child_id": child_id,
                "title_override": "乐乐的分享故事"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(derived["content_type"], "custom_storybook");
        assert_eq!(derived["derivation_type"], "from_plain_storybook");
        assert_eq!(derived["title"], "乐乐的分享故事");
    }
}
