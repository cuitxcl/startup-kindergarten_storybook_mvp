use std::{borrow::Cow, time::Duration};

use serde_json::{Value as JsonValue, json};

use crate::models::UNEXPECTED_ANIMAL_NAMES;
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
    "storybook_page_prompt",
    "customization_plan",
    "creation_understanding",
    "creation_directions",
    "creation_outline",
];

pub const SUPPORTED_TEXT_JOB_TYPES: &[&str] = TEXT_JOB_TYPES;
pub const DEFAULT_TEXT_SCHEMA_VERSION: &str = "generation.provider.v1";
const DEFAULT_VALIDATION_MAX_ATTEMPTS: u64 = 2;

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
            timeout_seconds: env_u64("DEEPSEEK_TIMEOUT_SECONDS", 300),
            max_tokens: env_u64("DEEPSEEK_MAX_TOKENS", 16384),
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

    #[cfg(test)]
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
            prompt["user_prompt"]
                .as_str()
                .unwrap_or("请生成结构化绘本内容。"),
            response_schema_for(request.job_type),
            prompt["input"]
        );
        let user_content = match retry_feedback
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(feedback) => format!(
                "{base_user_content}\n\n上一次输出未通过校验：{feedback}\n请针对上述问题修正后，重新输出完整 JSON 对象。"
            ),
            None => base_user_content,
        };
        // 分页图文需要严格按槽位模板输出，降低采样温度提升格式遵循度；方案/角色保留创造力空间。
        let temperature = if matches!(
            request.job_type,
            "storybook_pages" | "storybook_page_prompt"
        ) {
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
            "max_tokens": self.max_tokens_for_job(request.job_type),
            "stream": false
        }))
    }

    fn max_tokens_for_job(&self, job_type: &str) -> u64 {
        if matches!(job_type, "storybook_pages") {
            env_u64("DEEPSEEK_PAGES_MAX_TOKENS", self.max_tokens.max(16384))
        } else if matches!(job_type, "storybook_page_prompt") {
            env_u64(
                "DEEPSEEK_PAGE_PROMPT_MAX_TOKENS",
                self.max_tokens.max(16384),
            )
        } else {
            self.max_tokens
        }
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
        let body = response.bytes().await.map_err(|err| {
            GenerationProviderError::retryable(format!(
                "读取 DeepSeek 响应失败（可能响应超时或连接中断）：{err}"
            ))
        })?;

        if !status.is_success() {
            return Err(GenerationProviderError::retryable(format!(
                "DeepSeek 请求返回 {status}：{}",
                truncate(&lossy_body(&body), 240)
            )));
        }

        let response_json: JsonValue = serde_json::from_slice(&body).map_err(|err| {
            GenerationProviderError::new(format!(
                "DeepSeek 响应不是合法 JSON：{}；body={}",
                err,
                truncate(&lossy_body(&body), 240)
            ))
        })?;
        let content = response_json["choices"]
            .as_array()
            .and_then(|choices| choices.first())
            .and_then(|choice| choice["message"]["content"].as_str())
            .ok_or_else(|| {
                GenerationProviderError::new("DeepSeek 响应缺少 choices[0].message.content")
            })?
            .to_string();
        // 推理型模型（返回 reasoning_content）可能把 max_tokens 全部耗在思考上，content 为空。
        // 空内容直接给出可行动的报错，避免下游解析时报出难懂的 "EOF while parsing"。
        if content.trim().is_empty() {
            let finish_reason = response_json["choices"]
                .as_array()
                .and_then(|choices| choices.first())
                .and_then(|choice| choice["finish_reason"].as_str())
                .unwrap_or("unknown");
            return Err(GenerationProviderError::new(format!(
                "DeepSeek 返回空内容（finish_reason={finish_reason}）：推理型模型可能把 max_tokens 全部耗在思考上，请调大 DEEPSEEK_MAX_TOKENS"
            )));
        }
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

fn lossy_body(body: &[u8]) -> Cow<'_, str> {
    String::from_utf8_lossy(body)
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

        // 结构校验失败通常是模型遗漏字段而非用户输入错误。默认追加一次带反馈的修复请求，
        // 避免用户为可恢复的格式偏差手动重新发起整轮生成。
        let validation_max_attempts = env_u64(
            "DEEPSEEK_VALIDATION_MAX_ATTEMPTS",
            DEFAULT_VALIDATION_MAX_ATTEMPTS,
        )
        .clamp(1, 2);
        let mut retry_feedback: Option<String> = None;
        let mut last_validation_error: Option<GenerationProviderError> = None;
        for attempt in 0..validation_max_attempts {
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
                    // 用户选择的画风（input.style）透传给插图提示词拼接，让生图按所选画风执行。
                    let style = request.input.get("style").and_then(|value| value.as_str());
                    let normalized = normalize_provider_output(
                        output,
                        self.name(),
                        request.job_type,
                        usage,
                        Some(privacy_audit.clone()),
                        style,
                    )?;
                    validate_output_against_input(&normalized, request.input, request.job_type)?;
                    Ok(normalized)
                });
            match result {
                Ok(normalized) => return Ok(normalized),
                Err(err) => {
                    if attempt + 1 < validation_max_attempts {
                        retry_feedback = Some(err.safe_message());
                        last_validation_error = Some(err);
                        continue;
                    }
                    return Err(err);
                }
            }
        }

        Err(last_validation_error
            .unwrap_or_else(|| GenerationProviderError::new("DeepSeek 输出校验失败，请重试")))
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
    if !matches!(job_type, "storybook_pages" | "storybook_page_prompt") {
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

    // 收集待检查的 (定位标签, 标题, 正文, 插图提示词)：分页任务逐页检查，单页重写只检查重写页。
    let mut targets: Vec<(String, String, String, String)> = Vec::new();
    if job_type == "storybook_pages" {
        let pages = output
            .get("pages")
            .and_then(|value| value.as_array())
            .ok_or_else(|| {
                GenerationProviderError::new("provider 输出 storybook_pages.pages 必须是 array")
            })?;
        for (index, page) in pages.iter().enumerate() {
            targets.push((
                format!("{job_type}.pages[{index}]"),
                page.get("title")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string(),
                page.get("body")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string(),
                page.get("illustration_prompt")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string(),
            ));
        }
    } else {
        let page = output.get("page").and_then(|value| value.as_object());
        let prompt = page
            .and_then(|page| page.get("illustration_prompt"))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        let input_page = input.get("page");
        targets.push((
            format!("{job_type}.page"),
            input_page
                .and_then(|page| page.get("title"))
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string(),
            input_page
                .and_then(|page| page.get("body"))
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string(),
            prompt,
        ));
    }

    for (location, title, body, prompt) in &targets {
        let combined = format!("{title} {body} {prompt}");
        if !confirmed_roles.iter().any(|name| combined.contains(name)) {
            return Err(GenerationProviderError::new(format!(
                "provider 输出 {location} 未引用已确认角色：{}",
                confirmed_roles.join("、")
            )));
        }
        if !confirmed_roles.iter().any(|name| prompt.contains(name)) {
            return Err(GenerationProviderError::new(format!(
                "provider 输出 {location} 插图提示词未包含已确认角色姓名"
            )));
        }
        let unexpected_animals = UNEXPECTED_ANIMAL_NAMES;
        let role_text = input
            .get("confirmed_roles")
            .map(JsonValue::to_string)
            .unwrap_or_default();
        if let Some(animal) = unexpected_animals
            .iter()
            .find(|animal| combined.contains(**animal) && !role_text.contains(**animal))
        {
            return Err(GenerationProviderError::new(format!(
                "provider 输出 {location} 出现未确认替代角色：{animal}"
            )));
        }
        // 同一角色在插图提示词里被反复点名时，文生图模型可能会把它画成多个。
        // 这里仅拦截极端重复，普通超限交给质量检查做建议，避免真实生成白白失败。
        for name in &confirmed_roles {
            let mention_count = prompt.matches(name).count();
            if mention_count > 8 {
                return Err(GenerationProviderError::new(format!(
                    "provider 输出 {location} 插图提示词中「{name}」被点名 {mention_count} 次，重复过多会让模型把角色画成多个；请把它的动作、表情合并进同一组描述"
                )));
            }
        }
    }

    // 单页重写：相邻页存在群体场景时，新描述必须交代在场群体，防止人群跨页凭空消失。
    if job_type == "storybook_page_prompt" {
        let crowd_markers = [
            "一群",
            "挤",
            "人群",
            "大家",
            "小动物们",
            "孩子们",
            "排队",
            "簇拥",
            "涌",
        ];
        let neighbor_text = input
            .get("neighbor_pages")
            .and_then(|value| value.as_array())
            .map(|pages| {
                pages
                    .iter()
                    .filter_map(|page| {
                        page.get("illustration_prompt")
                            .and_then(|value| value.as_str())
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        let neighbor_has_crowd = !neighbor_text.is_empty()
            && crowd_markers
                .iter()
                .any(|marker| neighbor_text.contains(marker));
        if neighbor_has_crowd {
            let new_prompt = targets
                .first()
                .map(|(_, _, _, prompt)| prompt.as_str())
                .unwrap_or("");
            if !crowd_markers
                .iter()
                .any(|marker| new_prompt.contains(marker))
            {
                return Err(GenerationProviderError::new(format!(
                    "provider 输出 {job_type}.page 插图提示词未交代在场群体：相邻页存在人群场景，本页人群可以退到后排但不能凭空消失，请在 crowd 槽位写明人在哪里（或说明人群为何散去）"
                )));
            }
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

/// 分页图文与单页重写的插图槽位写法指引，保持同一套标准。
const COMPACT_ILLUSTRATION_SLOT_GUIDE: &str = r#"每页 illustration 必须输出 7 个短槽位：camera、scene_state、contact_chain、crowd、action、expression、prop_detail。

输出长度：title 不超过 14 个中文字符；body 不超过 90 个中文字符；每个插图槽位不超过 45 个中文字符，避免长句和重复点名。

镜头规则：camera 必须以景别开头（远景、全景、中景、中近景、近景、特写、局部特写、俯视、跟随视角）。远景/全景必须写“角色全身较小、环境占主画面”，不能同时要求看清微小细节；发现小虫、叶片纹理、表情、手部动作或两人交流时，优先近景、中近景、特写或局部特写。整本要有主体大小、视角、前后景层次变化，禁止连续多页实际都是中景。

角色规则：只使用 input.confirmed_roles.name 中的完整名称，不要简称、改名或创造昵称。每页有名角色最多 3 个；同一个有名角色在 illustration 全部槽位合计最多点名 2 次，把动作和表情集中写。背景群体只写数量和位置，不逐一命名。

动作规则：尊重角色外观结构。有手、前爪、脚或翅膀才写对应动作；无手脚、蛇形、球形或道具形态角色，只写头部朝向、身体弯曲、尾部、整体移动、光泽或位置变化，不得新增手脚、手臂、腿、鞋子或手指。

连续性：同一事件内地点、时间、群体规模不能突变；相邻页出现的人群可以换位置或退后，但不能无原因消失。

禁止写法：不要写“背景虚化”“背景模糊”“柔焦”“略显”“并排站”“面无表情”“证件照”；不要写风格词，风格由系统统一拼接；画面中不要出现文字。"#;

#[allow(dead_code)]
const ILLUSTRATION_SLOT_GUIDE: &str = "每页的插图设定必须输出为 illustration 对象，按以下 7 个槽位分别填写，槽位之间不要重复内容：\n- camera：镜头与构图，一句话以景别开头（远景、全景、中景、中近景、近景、特写、局部特写、俯视或跟随视角），再写构图重点和主体大小关系（例如远景=角色全身较小、环境占主画面，不能要求看清微小物件细节；近景或特写=适合发现小虫、叶片纹理、表情和手部动作；俯视=从上方看清空间关系）；根据本页剧情功能选择镜头，尽量让整本有自然变化，禁止连续几页实际都写成中景。\n- scene_state：场景此刻正在发生什么，必须写状态而不是场景名词；有群体时必须写出密度（身体紧紧挨着、你推我搡、卡成一团）。\n- contact_chain：主角与周围角色之间的接触和遮挡关系，例如“身后的小熊被人群推着贴上小猫的背”；主角必须被写进关系里，不能孤立在人群之外。\n- crowd：在场群体交代，必须写明画面里除主要角色外还有哪些人、在画面什么位置（例如“后排还有五六只小动物踮脚张望，门口小路上仍有人赶来”）；如果故事此刻处于人群场景中（拥挤、排队、一群），本页人群必须仍在场，可以退到后排或让出画面中心，但不能凭空消失；确实只剩少量角色时，必须写明原因（例如“其他小动物已经进教室了，门口只剩小猫和小兔”）；背景群体只用数量词和位置概括（例如“后排还有几只小动物”），不要给背景角色逐一分配动作、表情或外观细节。\n- action：主要角色的动作，必须写成身体语言，且必须尊重 input.confirmed_roles 中的外观结构；有手、前爪、脚或翅膀的角色可以写扒着、挥动、扶着、抱住、捂住、指向等具体动作；如果角色外观写明无手、无脚、蛇形、球形或道具形态，禁止硬写手脚动作，改用头部朝向、身体弯曲、尾部、整体移动、光芒变化或位置变化表达动作。\n- expression：主要角色的表情，必须写成具体五官细节（眉头紧皱、眼睛瞪圆、嘴巴张成 O 形）；如果角色没有五官，只写可见材质、光泽、姿态或位置变化；禁止只写“慌张”“着急”“开心”这类总结词。\n- prop_detail：一个增强现场感的小道具细节，例如“地上有一只被挤掉的小书包”；没有合适的就填空字符串。\n\n跨页连续性：同一事件内，地点、时间和在场群体规模不得突变；前一页出现的人群在后一页必须仍在场（可以换位置、退到后排），次要角色可以换位但不能整群蒸发。\n\n镜头节奏建议：按故事需要安排景别，不要为了凑规则牺牲画面表达。开篇只有在需要建立大地点或群体关系时才用远景或全景，并必须写出环境占比和角色较小；如果本页核心是发现小虫、观察叶片、看清表情、手部动作或两个角色交流，优先使用近景、中近景、特写或局部特写，不要硬写远景；中段必须让主体大小、视角或前后景层次有明显差异；收尾可以用远景、全景、中景或其他能表达结果与情绪的视角。拥挤、排队、争抢、密集互动场面可优先中近景；安静交流、发现线索、俯看地图等场景可以使用更贴切的镜头。\n\n分镜节奏参考：可以参考“开篇建立场景、中段推进动作与情绪、结尾收束结果”的节奏，但不强制套用固定模板；如果连续几页确实是同一类互动，可以保持相近景别，只要每页构图重点清楚。\n\n角色预算：每页有名角色（input.confirmed_roles 里的角色）最多 3 个，主角加 1~2 个互动对象，其余角色退为背景群体；同一个有名角色在全部槽位合计最多出现 2 次，把它的动作、表情集中写进同一组描述，不要在多个槽位反复点名，文生图模型会把反复点名的角色画成多个。\n\n情绪翻译词库（必须先把情绪翻译成身体语言再写）：着急=身体前倾+眉头紧锁；慌张=眼睛瞪圆+后仰失衡；开心=眯起笑眼+嘴角上扬；拥挤=身体紧挨+相互遮挡+你推我搡。对于无手脚、蛇形或道具角色，只能使用符合其结构的身体语言，不得新增手、脚、手臂、腿或鞋子。\n\n禁止写法（出现即视为不合格）：“背景虚化”“背景模糊”“柔焦”“略显”“并排站”“面无表情”“人群最前面”“证件照”；禁止只写场景名词而不写场景状态；禁止给无手脚角色写手、脚、胳膊、腿、鞋子或手指。风格不用你写，由系统统一拼接，不要在任何槽位里写风格描述或“绘本风格”字样。\n\n合格示例（仅作写法示范，实际输出必须使用 input.confirmed_roles 里的角色）：\ncamera：全景，画面从幼儿园门口和小路展开，一群小动物挤在木门前相互遮挡\nscene_state：早晨送园高峰，小动物们身体紧紧挨着、你推我搡卡在门口，小路上还有人赶来\ncontact_chain：橘色条纹小猫被夹在人群中间，身后的小熊被推着贴上他的背；白色小兔紧挨着小猫，长耳朵被旁边的小动物压歪\ncrowd：门口还有五六只小动物踮脚张望排在后面，小路上仍有两三只小动物赶来\naction：小猫踮起脚尖、肩膀前倾、扒着门把手往门缝里挤；小兔被挤得踉跄前倾、前爪慌乱挥舞\nexpression：小猫眉头紧皱、胡须绷直；小兔眼睛瞪圆、嘴巴张成 O 形\nprop_detail：地上有一只被挤掉的粉色书包\n\n无手脚角色示例（仅作写法示范）：\ncamera：近景，画面聚焦蛇形角色抬头和前方发光果实\nscene_state：蛇形角色停在草地边缘，身体微微盘起，前方有一颗发光果实\ncontact_chain：蛇形角色靠近果实，尾部留在草叶旁作为支撑\ncrowd：周围没有其他角色\naction：蛇形角色抬起头，身体弯成柔和 S 形，尾端轻轻贴着草地向前滑动\nexpression：蛇形角色眼睛睁圆、嘴角微微上扬\nprop_detail：果实旁有几片被光照亮的叶子\n\n安静场景对照示例（非拥挤剧情用这种简洁写法，有名角色少、构图单一焦点）：\ncamera：中景，画面聚焦窗边桌面和两个角色的安静互动\nscene_state：午后安静的活动室，小猫坐在窗边拼图，阳光落在桌面上\ncontact_chain：小兔侧身挨着小猫坐下，一只前爪轻轻搭在小猫背上\ncrowd：其他小动物已经去午睡区了，教室里只剩小猫和小兔\naction：小猫低头捏起一块拼图，指尖对准缺口轻轻放下\nexpression：小猫眯起笑眼、嘴角上扬；小兔耳朵放松垂下、眼睛弯成弧线\nprop_detail：桌角放着一杯冒着热气的温水";

pub(crate) fn prompt_for(job_type: &str) -> String {
    match job_type {
        "storybook_plan" => {
            "根据 input.title、input.theme、input.use_scene、input.style 生成普通儿童绘本方案。input.style 是画面风格（插画视觉效果），input.story_style 是故事风格（情节基调与叙事类型，如温情治愈、冒险奇幻、幽默搞笑）；如果 input.story_style 非空，summary、outline 的情节走向、冲突设计和情绪基调必须符合该故事风格。故事主线必须围绕输入标题和主题展开：如果标题或主题是具体场景，如丛林、海边、厨房、午睡、入园等，summary、outline、role_requirements 必须反复体现该场景和主题，不得沿用无关的玩具轮流、小火车分享等通用示例。如果 input.story_framework 非空，那是用户提供的故事框架：summary 和 outline 必须严格按框架的起因、经过、结果展开分页，不得另起主线或更换结局，只允许在框架内补充细节、对话和情绪描写；story_framework 为空时由你自由创作主线。先给故事主线，再给分页节奏和创作者确认点。分页节奏（outline）必须一页一条，共 page_count 条：每条都必须包含 page_range、goal、beat 三个非空字段。page_range 是必填字段，只写单个页码数字（如 \"3\"），禁止写 \"1-2\"、\"3-4\" 这类跨页区间；每条的 goal 和 beat 只描述这一页的画面与剧情，不要把两页内容合并进一条。"
                .to_string()
        }
        "storybook_roles" => {
            "根据 input.plan 中已经确认的故事方案生成主角、同伴、老师形象和关键道具设定，必须紧扣 input.title、input.theme、input.plan.summary 和 input.plan.outline，不得沿用无关示例。role_type 只能使用英文枚举 protagonist、supporting、peer、teacher、prop；不要输出中文类型。appearance 只能写稳定可见的视觉特征，例如物种或身份、颜色、服装或材质、体型轮廓、发型/耳朵/配饰、表情和可跨页重复识别的小标记；禁止把动作、习惯、剧情行为或故事任务写入 appearance，例如“喜欢蹦跳”“离开队伍”“带领探险”“制定规则”。这些内容必须写入 story_function。needs_consistency 只给需要跨页重复出现并保持同一形象的主角、老师、重要同伴或反复出现关键道具设为 true；只出现一次的临时事物、背景动物、一次性道具必须设为 false，不需要参考图。needs_consistency=true 时可以输出 reference_image_prompt，它必须只基于角色名称、role_type、appearance 和 input.style 生成标准参考图提示；必须包含 input.style 或“画面风格必须与整本绘本一致”，禁止写与 input.style 冲突的水彩、卡通、手绘等其他画风词。"
                .to_string()
        }
        "storybook_pages" => {
            format!("根据已确认方案和角色生成分页图文，每页包含标题、正文和插图设定。如果 input.creation_context 存在，说明这是“专属故事共创”的最终成品生成：必须优先服务用户的被理解、参与感和私人定制感；正文要承接 input.creation_context.quick_idea、understanding、selected_direction、materials、visual_preferences，不能只泛泛扩写 outline。input.creation_context.materials 中 locked=true 的素材，其 label 必须至少一次原样出现在某一页 title、body 或最终插图描述中；如果素材较难自然进入正文，也要用原 label 放入画面道具、场景细节或角色称呼里，避免只做语义暗示。\n\n必须严格沿用 input.confirmed_roles 中的完整角色姓名、身份、外观和关键道具；正文、标题和插图槽位里只要点名角色，就必须使用 confirmed_roles.name 的完整名称，不要把“兔老师”简称为“小兔”、把“小狐狸图图”改成“小狐狸”或创造任何昵称；不得把人类角色改成动物或新增替代主角。每页正文应像正式儿童绘本文案：简短、有画面、有情绪推进，避免说教，避免把大纲句原样复制成正文。\n\n{COMPACT_ILLUSTRATION_SLOT_GUIDE}")
        }
        "storybook_page_prompt" => {
            format!("根据 input.page 的标题（title）和正文（body），为这一页重新创作插图设定，替换现有插图描述。正文必须忠于 input.page.body，不要改动剧情。必须严格沿用 input.confirmed_roles 中的完整角色姓名、身份、外观和关键道具；正文、标题和插图槽位里只要点名角色，就必须使用 confirmed_roles.name 的完整名称，不要把“兔老师”简称为“小兔”、把“小狐狸图图”改成“小狐狸”或创造任何昵称；不得把人类角色改成动物或新增替代主角。\n\ninput.neighbor_pages 包含前后相邻页的插图描述（可能为空）：本页必须与相邻页保持场景连续，相邻页出现的人群在本页必须仍在场（可以退到后排或让出画面中心，但不能消失）；如果剧情确实让人群散去了，必须在 crowd 槽位写明人群去了哪里。\n\n{COMPACT_ILLUSTRATION_SLOT_GUIDE}")
        }
        "customization_plan" => {
            "基于普通绘本和儿童档案生成定制方案，只输出可审核的改写点和风险检查。"
                .to_string()
        }
        "creation_understanding" => {
            "你是儿童故事共创助手，目标不是帮用户填表，而是让用户感觉“AI 真的听懂了我为什么要做这本绘本”。请基于 input.quick_idea、input.use_scene、input.age_group 和 input.preserved_user_materials 输出 understanding 与 materials。\n\n理解规则：summary 要用用户语言复述真实动机，必须包含对象、冲突/事件、想传达的目标；不要只摘关键词，不要写技术解释。target_user 根据语境判断为 parent、teacher、creator 或 organization，老师只是可能场景之一，不要默认锁定老师。goal 写成孩子或共读场景中的成长目标；tone 写成用户能理解的故事语气。\n\n素材规则：materials 必须提取能带来私人定制感的真实姓名、地点、物品、事件、主题、情绪、关系；input.preserved_user_materials 必须保留，不得改名、删掉或降低 locked。用户明确提到的人名、地点、物品、真实事件默认 locked=true；泛泛主题可以 locked=false。材料 type 只能是 character/object/place/event/theme/emotion/custom，source 只能是 ai_extracted/user_added/system。\n\n输出只包含 schema 要求字段；除 schema 固定字段外，不要输出额外技术解释；message、summary、quality_flags 等用户可见或业务字段中不要提 provider、job、prompt、model、队列。".to_string()
        }
        "creation_directions" => {
            "你正在把用户的一句话想法变成 3 个可选择的故事方向。方向选择是用户参与感的来源，所以 3 个方向必须是真正不同的叙事策略，而不是替换形容词或换标题。\n\n输入包含 input.quick_idea、input.understanding、input.materials。请优先使用 locked=true 的素材；每个方向的 material_ids 至少包含一个真实素材，能自然使用多个时优先多用。每个方向必须包含：title、summary、fit_reason、personal_hook、material_ids、tone。\n\n差异规则：3 个方向应分别体现不同创作角度，例如成长练习、趣味任务、特别回忆、关系修复、课堂导入、礼物纪念等，但不要机械套模板，要根据用户动机选择。summary 写一句用户看得懂的剧情走向；fit_reason 说明适合什么共读/使用场景；personal_hook 必须明确说私人素材会在故事哪个关键时刻发挥作用。\n\n输出只包含 schema 要求字段；除 schema 固定字段外，不要输出额外技术解释；message、summary、fit_reason、personal_hook、quality_flags 等用户可见或业务字段中不要提 provider、job、prompt、model。".to_string()
        }
        "creation_outline" => {
            "你正在生成正式绘本前的大纲。大纲不是正文编辑器，而是帮助用户放心点击生成的心理缓冲。请基于 input.quick_idea、input.understanding、input.selected_direction、input.materials、input.visual_preferences 和 input.page_count 输出 outline。\n\n大纲规则：pages 数量必须等于 page_count，page_number 从 1 连续递增；每页只写一句 summary，短而具体，不要写成完整正文；每页 material_ids 至少引用一个素材。locked=true 的素材必须尽量进入不同页面的具体情节，而不是只出现在标题或 review_points。summary 要让用户看见“我的名字/地点/物品/真实事件如何被用到”。\n\n节奏规则：开头建立真实场景，中段让冲突或任务推进，结尾给孩子可理解的温柔结果；不要说教，不要把所有问题一页解决。visual_preferences 只影响画面复杂度和画面想象，不要暴露模型参数。\n\nreview_points 用用户语言列出 2-4 个确认点，例如“是否保留某个真实素材”“语气是否足够温柔”“结尾是否适合实际共读”。输出只包含 schema 要求字段；除 schema 固定字段外，不要输出额外技术解释；message、summary、review_points、quality_flags 等用户可见或业务字段中不要提 provider、job、prompt、model。".to_string()
        }
        _ => "生成结构化绘本内容。".to_string(),
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
                "outline": [{"page_range": "string（单页页码，如 \"3\"，禁止 \"1-2\" 跨页区间）", "goal": "string", "beat": "string"}],
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
                "reference_image_prompt": "string（可选，needs_consistency=true 时使用；只描述标准参考图，必须包含 input.style 或整本绘本画风一致要求）",
                "needs_consistency": "boolean"
            }],
            "consistency_guide": ["string"]
        }),
        "storybook_pages" => json!({
            "schema_version": "generation.provider.v1",
            "provider": "string",
            "mode": "storybook_pages",
            "message": "string",
            "quality_notice": "string（可选；仅当专属素材无法自然进入成品或需要用户确认时，用用户能理解的话说明，不要写技术原因）",
            "pages": [{
                "page_number": "number",
                "title": "string",
                "body": "string",
                "illustration": {
                    "camera": "string（镜头与构图，必须以远景、全景、中景、中近景、近景、特写、局部特写、俯视或跟随视角开头）",
                    "scene_state": "string（场景正在发生什么，群体必须写出密度）",
                    "contact_chain": "string（主角与周围角色的接触和遮挡关系）",
                    "crowd": "string（在场群体交代，人群场景必须写人在哪，人少必须写原因）",
                    "action": "string（动作的身体语言，不是动作名称）",
                    "expression": "string（表情的具体五官细节，不是情绪总结词）",
                    "prop_detail": "string（一个氛围道具细节，可为空字符串）"
                },
                "status": "draft"
            }],
            "editor_notes": ["string（给创作者的用户语言检查点，不要写技术细节）"]
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
        "storybook_page_prompt" => json!({
            "schema_version": "generation.provider.v1",
            "provider": "string",
            "mode": "storybook_page_prompt",
            "message": "string",
            "page": {
                "page_number": "number",
                "illustration": {
                    "camera": "string（镜头与构图，必须以远景、全景、中景、中近景、近景、特写、局部特写、俯视或跟随视角开头）",
                    "scene_state": "string（场景正在发生什么，群体必须写出密度）",
                    "contact_chain": "string（主角与周围角色的接触和遮挡关系）",
                    "crowd": "string（在场群体交代，人群场景必须写人在哪，人少必须写原因）",
                    "action": "string（动作的身体语言，不是动作名称）",
                    "expression": "string（表情的具体五官细节，不是情绪总结词）",
                    "prop_detail": "string（一个氛围道具细节，可为空字符串）"
                }
            }
        }),
        "creation_understanding" => json!({
            "schema_version": "creation.provider.v1",
            "provider": "string",
            "mode": "creation_understanding",
            "message": "string",
            "understanding": {
                "summary": "string",
                "target_user": "parent | teacher | creator | organization",
                "goal": "string",
                "tone": "string",
                "scene": "string",
                "age_group": "string"
            },
            "materials": [{
                "id": "mat_1",
                "label": "string",
                "type": "character | object | place | event | theme | emotion | custom",
                "source": "ai_extracted | user_added | system",
                "confidence": "number or null",
                "locked": "boolean"
            }],
            "quality_flags": ["string"]
        }),
        "creation_directions" => json!({
            "schema_version": "creation.provider.v1",
            "provider": "string",
            "mode": "creation_directions",
            "message": "string",
            "directions": [{
                "id": "dir_1",
                "title": "string",
                "summary": "string",
                "fit_reason": "string",
                "personal_hook": "string",
                "material_ids": ["mat_1"],
                "tone": "gentle | playful | warm | clear | encouraging | custom"
            }],
            "quality_flags": ["string"]
        }),
        "creation_outline" => json!({
            "schema_version": "creation.provider.v1",
            "provider": "string",
            "mode": "creation_outline",
            "message": "string",
            "outline": {
                "summary": "string",
                "pages": [{
                    "page_number": "number",
                    "summary": "string",
                    "material_ids": ["mat_1"]
                }],
                "review_points": ["string"]
            },
            "quality_flags": ["string"]
        }),
        _ => json!({}),
    }
}
