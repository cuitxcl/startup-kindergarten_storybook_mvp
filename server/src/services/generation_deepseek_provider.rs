use std::time::Duration;

use serde_json::{Value as JsonValue, json};

use crate::services::{
    generation_output_validator::normalize_provider_output,
    generation_privacy::{provider_input_privacy_audit, sanitize_provider_input},
    generation_provider_config::{env_non_empty, env_u64, first_non_empty_env, truncate},
    generation_provider_contract::{
        AiGenerationProvider, GenerationProviderComponent, GenerationProviderError,
        GenerationRequest, ImageGenerationRequest,
    },
};

const TEXT_JOB_TYPES: &[&str] = &[
    "storybook_plan",
    "storybook_roles",
    "storybook_pages",
    "customization_plan",
];

pub const SUPPORTED_TEXT_JOB_TYPES: &[&str] = TEXT_JOB_TYPES;
pub const DEFAULT_TEXT_SCHEMA_VERSION: &str = "generation.provider.v1";

pub struct DeepSeekTextProvider {
    pub(crate) api_key: Option<String>,
    pub(crate) base_url: String,
    pub(crate) endpoint_path: String,
    pub(crate) model: String,
    pub(crate) timeout_seconds: u64,
    pub(crate) max_tokens: u64,
}

impl DeepSeekTextProvider {
    pub(crate) fn from_env() -> Self {
        Self {
            api_key: env_non_empty("DEEPSEEK_API_KEY"),
            base_url: first_non_empty_env(&["DEEPSEEK_BASE_URL"], "https://api.deepseek.com"),
            endpoint_path: first_non_empty_env(&["DEEPSEEK_ENDPOINT_PATH"], "/chat/completions"),
            model: first_non_empty_env(&["DEEPSEEK_MODEL"], "deepseek-v4-flash"),
            timeout_seconds: env_u64("DEEPSEEK_TIMEOUT_SECONDS", 180),
            max_tokens: env_u64("DEEPSEEK_MAX_TOKENS", 4096),
        }
    }

    pub(crate) fn build_prompt(
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

    pub(crate) fn build_chat_payload(
        &self,
        request: &GenerationRequest<'_>,
    ) -> Result<JsonValue, GenerationProviderError> {
        self.build_chat_payload_with_feedback(request, None)
    }

    /// retry_feedback 用于校验未通过后的自动重试：把校验失败原因反馈给模型，引导其修正输出。
    pub(crate) fn build_chat_payload_with_feedback(
        &self,
        request: &GenerationRequest<'_>,
        retry_feedback: Option<&str>,
    ) -> Result<JsonValue, GenerationProviderError> {
        let prompt = self.build_prompt(request)?;
        let base_user_content = format!(
            "{}\n\n请只返回一个合法 JSON 对象，不要 Markdown，不要代码块。\n期望 JSON 结构示例：\n{}\n\n输入：\n{}",
            prompt["user_prompt"].as_str().unwrap_or("请生成结构化绘本内容。"),
            response_schema_for(request.job_type),
            prompt["input"]
        );
        let user_content = match retry_feedback.map(str::trim).filter(|value| !value.is_empty()) {
            Some(feedback) => format!(
                "{base_user_content}\n\n上一次输出未通过校验：{feedback}\n请针对上述问题修正后，重新输出完整 JSON 对象。"
            ),
            None => base_user_content,
        };
        // 分页图文需要严格按槽位模板输出，降低采样温度提升格式遵循度；方案/角色保留创造力空间。
        let temperature = if request.job_type == "storybook_pages" {
            0.35
        } else {
            0.7
        };
        Ok(json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": prompt["system_prompt"]
                },
                {
                    "role": "user",
                    "content": user_content
                }
            ],
            "response_format": {"type": "json_object"},
            "temperature": temperature,
            "max_tokens": self.max_tokens,
            "stream": false
        }))
    }

    pub(crate) fn endpoint(&self) -> String {
        format_deepseek_endpoint(&self.base_url, &self.endpoint_path)
    }

    /// 单次 DeepSeek 调用，返回 (content, usage)。传输层与协议层错误直接返回，不进入校验重试。
    async fn request_content(
        &self,
        client: &reqwest::Client,
        api_key: &str,
        payload: &JsonValue,
    ) -> Result<(String, Option<JsonValue>), GenerationProviderError> {
        let response = client
            .post(self.endpoint())
            .bearer_auth(api_key)
            .json(payload)
            .send()
            .await
            .map_err(|err| {
                GenerationProviderError::retryable(format!("DeepSeek 请求失败：{err}"))
            })?;

        let status = response.status();
        let body = response.text().await.map_err(|err| {
            GenerationProviderError::retryable(format!(
                "读取 DeepSeek 响应失败（可能响应超时或连接中断）：{err}"
            ))
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
            })?
            .to_string();
        let usage = response_json.get("usage").cloned();
        Ok((content, usage))
    }

    pub(crate) fn summary_component(&self) -> GenerationProviderComponent {
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
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(self.timeout_seconds))
            .build()
            .map_err(|err| {
                GenerationProviderError::new(format!("创建 DeepSeek 客户端失败：{err}"))
            })?;

        // 输出校验未通过时，把失败原因反馈给模型自动重试一次；传输层错误仍交给任务队列重试。
        let mut retry_feedback: Option<String> = None;
        let mut last_validation_error: Option<GenerationProviderError> = None;
        for attempt in 0..2 {
            let payload =
                self.build_chat_payload_with_feedback(&request, retry_feedback.as_deref())?;
            let (content, usage) = self.request_content(&client, api_key, &payload).await?;
            let result = serde_json::from_str::<JsonValue>(&content)
                .map_err(|err| {
                    GenerationProviderError::new(format!(
                        "DeepSeek content 不是合法 JSON：{}；content={}",
                        err,
                        truncate(&content, 240)
                    ))
                })
                .and_then(|output| {
                    let normalized = normalize_provider_output(
                        output,
                        self.name(),
                        request.job_type,
                        usage,
                        Some(privacy_audit.clone()),
                    )?;
                    validate_output_against_input(&normalized, request.input, request.job_type)?;
                    Ok(normalized)
                });
            match result {
                Ok(normalized) => return Ok(normalized),
                Err(err) => {
                    if attempt == 0 {
                        retry_feedback = Some(err.safe_message());
                        last_validation_error = Some(err);
                        continue;
                    }
                    return Err(err);
                }
            }
        }

        Err(last_validation_error.unwrap_or_else(|| {
            GenerationProviderError::new("DeepSeek 输出校验失败，请重试")
        }))
    }

    async fn generate_image(
        &self,
        _request: ImageGenerationRequest<'_>,
    ) -> Result<JsonValue, GenerationProviderError> {
        Err(GenerationProviderError::new(
            "当前仅配置了文本 provider（deepseek），插图生成需要配置 SEEDREAM_API_KEY 或 ARK_API_KEY",
        ))
    }
}

pub(crate) fn validate_output_against_input(
    output: &JsonValue,
    input: &JsonValue,
    job_type: &str,
) -> Result<(), GenerationProviderError> {
    if job_type != "storybook_pages" {
        return Ok(());
    }
    let confirmed_roles = input
        .get("confirmed_roles")
        .and_then(|value| value.as_array())
        .map(|roles| {
            roles
                .iter()
                .filter_map(|role| role.get("name").and_then(|name| name.as_str()))
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if confirmed_roles.is_empty() {
        return Ok(());
    }

    let pages = output
        .get("pages")
        .and_then(|value| value.as_array())
        .ok_or_else(|| {
            GenerationProviderError::new("provider 输出 storybook_pages.pages 必须是 array")
        })?;
    for (index, page) in pages.iter().enumerate() {
        let title = page
            .get("title")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let body = page
            .get("body")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let prompt = page
            .get("illustration_prompt")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let combined = format!("{title} {body} {prompt}");
        if !confirmed_roles.iter().any(|name| combined.contains(name)) {
            return Err(GenerationProviderError::new(format!(
                "provider 输出 storybook_pages.pages[{index}] 未引用已确认角色：{}",
                confirmed_roles.join("、")
            )));
        }
        if !confirmed_roles.iter().any(|name| prompt.contains(name)) {
            return Err(GenerationProviderError::new(format!(
                "provider 输出 storybook_pages.pages[{index}].illustration_prompt 未包含已确认角色姓名"
            )));
        }
        let unexpected_animals = [
            "小象",
            "小兔",
            "小猴",
            "小熊",
            "小猫",
            "小狗",
            "小狐狸",
            "小鹿",
        ];
        let role_text = input
            .get("confirmed_roles")
            .map(JsonValue::to_string)
            .unwrap_or_default();
        if let Some(animal) = unexpected_animals
            .iter()
            .find(|animal| combined.contains(**animal) && !role_text.contains(**animal))
        {
            return Err(GenerationProviderError::new(format!(
                "provider 输出 storybook_pages.pages[{index}] 出现未确认替代角色：{animal}"
            )));
        }
    }
    Ok(())
}

pub(crate) fn format_deepseek_endpoint(base_url: &str, endpoint_path: &str) -> String {
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

pub(crate) fn prompt_for(job_type: &str) -> &'static str {
    match job_type {
        "storybook_plan" => {
            "根据 input.title、input.theme、input.use_scene、input.style 生成普通绘本方案。故事主线必须围绕输入标题和主题展开：如果标题或主题是具体场景，如丛林、海边、厨房、午睡、入园等，summary、outline、role_requirements 必须反复体现该场景和主题，不得沿用无关的玩具轮流、小火车分享等通用示例。先给故事主线，再给分页节奏和老师审核点。"
        }
        "storybook_roles" => {
            "根据 input.plan 中已经确认的故事方案生成主角、同伴、老师形象和关键道具设定，必须紧扣 input.title、input.theme、input.plan.summary 和 input.plan.outline，不得沿用无关示例。role_type 只能使用英文枚举 protagonist、supporting、peer、teacher、prop；不要输出中文类型。appearance 只能写稳定可见的视觉特征，例如物种或身份、颜色、服装或材质、体型轮廓、发型/耳朵/配饰、表情和可跨页重复识别的小标记；禁止把动作、习惯、剧情行为或故事任务写入 appearance，例如“喜欢蹦跳”“离开队伍”“带领探险”“制定规则”。这些内容必须写入 story_function。needs_consistency 只给需要跨页重复出现并保持同一形象的主角、老师、重要同伴或反复出现关键道具设为 true；只出现一次的临时事物、背景动物、一次性道具必须设为 false，不需要参考图。"
        }
        "storybook_pages" => {
            "根据已确认方案和角色生成分页图文，每页包含标题、正文和插图设定。必须严格沿用 input.confirmed_roles 中的角色姓名、身份、外观和关键道具，不得把人类角色改成动物或新增替代主角。\n\n每页的插图设定必须输出为 illustration 对象，按以下 6 个槽位分别填写，槽位之间不要重复内容：\n- camera：镜头与构图，一句话，例如“中近景，画面紧凑，角色相互遮挡”；角色多或画面拥挤时优先中近景。\n- scene_state：场景此刻正在发生什么，必须写状态而不是场景名词；有群体时必须写出密度（身体紧紧挨着、你推我搡、卡成一团）。\n- contact_chain：主角与周围角色之间的接触和遮挡关系，例如“身后的小熊被人群推着贴上小猫的背”；主角必须被写进关系里，不能孤立在人群之外。\n- action：主要角色的动作，必须写成身体语言（踮起脚尖、肩膀前倾、扒着门框、踉跄后仰），不能只写动作名称。\n- expression：主要角色的表情，必须写成具体五官细节（眉头紧皱、眼睛瞪圆、嘴巴张成 O 形），禁止只写“慌张”“着急”“开心”这类总结词。\n- prop_detail：一个增强现场感的小道具细节，例如“地上有一只被挤掉的小书包”；没有合适的就填空字符串。\n\n情绪翻译词库（必须先把情绪翻译成身体语言再写）：着急=踮脚+身体前倾+眉头紧锁；慌张=眼睛瞪圆+耳朵后压+后仰失衡+手脚乱挥；开心=眯起笑眼+嘴角上扬+蹦跳离地；拥挤=身体紧挨+相互遮挡+你推我搡。\n\n禁止写法（出现即视为不合格）：“背景虚化”“背景模糊”“柔焦”“略显”“并排站”“面无表情”“人群最前面”“证件照”；禁止只写场景名词而不写场景状态。风格不用你写，由系统统一拼接，不要在任何槽位里写风格描述或“绘本风格”字样。\n\n合格示例（仅作写法示范，实际输出必须使用 input.confirmed_roles 里的角色）：\ncamera：中近景，画面紧凑，一群小动物挤在木门口相互遮挡\nscene_state：早晨送园高峰，小动物们身体紧紧挨着、你推我搡卡在门口，小路上还有人赶来\ncontact_chain：橘色条纹小猫被夹在人群中间，身后的小熊被推着贴上他的背；白色小兔紧挨着小猫，长耳朵被旁边的小动物压歪\naction：小猫踮起脚尖、肩膀前倾、扒着门把手往门缝里挤；小兔被挤得踉跄前倾、前爪慌乱挥舞\nexpression：小猫眉头紧皱、胡须绷直；小兔眼睛瞪圆、嘴巴张成 O 形\nprop_detail：地上有一只被挤掉的粉色书包"
        }
        "customization_plan" => {
            "基于普通绘本和儿童档案生成定制方案，只输出可审核的改写点和风险检查。"
        }
        _ => "生成结构化绘本内容。",
    }
}

pub(crate) fn response_schema_for(job_type: &str) -> JsonValue {
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
                "role_type": "protagonist | supporting | peer | teacher | prop",
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
                "illustration": {
                    "camera": "string（镜头与构图，角色多时优先中近景）",
                    "scene_state": "string（场景正在发生什么，群体必须写出密度）",
                    "contact_chain": "string（主角与周围角色的接触和遮挡关系）",
                    "action": "string（动作的身体语言，不是动作名称）",
                    "expression": "string（表情的具体五官细节，不是情绪总结词）",
                    "prop_detail": "string（一个氛围道具细节，可为空字符串）"
                },
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
