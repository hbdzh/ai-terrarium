use std::collections::{BTreeMap, HashSet};
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::watch;

use crate::models::NodeConfig;
use crate::sandbox_tools::{ApiToolCall, ApiToolFunctionCall};
use crate::sanitizer::SanitizedSegment;
use crate::sanitizer::ThinkTagStreamParser;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ApiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ApiChatMessage {
    pub fn text(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".to_string(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }

    pub fn content_str(&self) -> &str {
        self.content.as_deref().unwrap_or("")
    }
}

#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

pub fn resolve_chat_completions_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/chat/completions")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkingVendor {
    DashScopeQwen,
    DeepSeekVolces,
    OpenAiStandard,
}

pub fn detect_thinking_vendor(base_url: &str, model_name: &str) -> ThinkingVendor {
    let url = base_url.to_ascii_lowercase();
    let model = model_name.to_ascii_lowercase();
    if url.contains("dashscope") || model.contains("qwen") {
        ThinkingVendor::DashScopeQwen
    } else if url.contains("deepseek")
        || url.contains("ark")
        || url.contains("volces")
        || model.contains("deepseek")
        || model.contains("doubao")
    {
        ThinkingVendor::DeepSeekVolces
    } else {
        ThinkingVendor::OpenAiStandard
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ThinkingConfigTag {
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedThinkingParams {
    pub reasoning_effort: Option<String>,
    pub enable_thinking: Option<bool>,
    pub thinking_budget: Option<u32>,
    pub thinking: Option<ThinkingConfigTag>,
}

impl ResolvedThinkingParams {
    pub fn is_empty(&self) -> bool {
        self.reasoning_effort.is_none()
            && self.enable_thinking.is_none()
            && self.thinking_budget.is_none()
            && self.thinking.is_none()
    }
}

/// 思考控制参数优先级裁决（本局直觉快反应 > 单智能体 thinking_level）
pub fn resolve_thinking_params(
    config: &ExperimentConfig,
    node: &NodeConfig,
) -> ResolvedThinkingParams {
    let vendor = detect_thinking_vendor(&node.base_url, &node.model_name);

    // 第一优先级：全局「直觉快反应模式」开启时，全员一律压低思考预算直出
    if config.intuitive_fast_reply {
        return match vendor {
            ThinkingVendor::DashScopeQwen => ResolvedThinkingParams {
                enable_thinking: Some(false),
                ..Default::default()
            },
            ThinkingVendor::DeepSeekVolces => ResolvedThinkingParams {
                thinking: Some(ThinkingConfigTag {
                    kind: "disabled".to_string(),
                }),
                ..Default::default()
            },
            ThinkingVendor::OpenAiStandard => ResolvedThinkingParams {
                reasoning_effort: Some("minimal".to_string()),
                ..Default::default()
            },
        };
    }

    // 第二优先级：读取该智能体独立的 thinking_level
    let level = crate::models::normalize_thinking_level(&node.thinking_level);
    match level.as_str() {
        "off" => match vendor {
            ThinkingVendor::DashScopeQwen => ResolvedThinkingParams {
                enable_thinking: Some(false),
                ..Default::default()
            },
            ThinkingVendor::DeepSeekVolces => ResolvedThinkingParams {
                thinking: Some(ThinkingConfigTag {
                    kind: "disabled".to_string(),
                }),
                ..Default::default()
            },
            ThinkingVendor::OpenAiStandard => ResolvedThinkingParams {
                reasoning_effort: Some("none".to_string()),
                ..Default::default()
            },
        },
        "low" | "medium" | "high" => match vendor {
            ThinkingVendor::DashScopeQwen => {
                let budget = match level.as_str() {
                    "low" => 1024,
                    "medium" => 4096,
                    _ => 16384,
                };
                ResolvedThinkingParams {
                    enable_thinking: Some(true),
                    thinking_budget: Some(budget),
                    ..Default::default()
                }
            }
            ThinkingVendor::DeepSeekVolces => ResolvedThinkingParams {
                thinking: Some(ThinkingConfigTag {
                    kind: "enabled".to_string(),
                }),
                reasoning_effort: Some(level),
                ..Default::default()
            },
            ThinkingVendor::OpenAiStandard => ResolvedThinkingParams {
                reasoning_effort: Some(level),
                ..Default::default()
            },
        },
        _ => ResolvedThinkingParams::default(),
    }
}

#[derive(Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: &'a [ApiChatMessage],
    temperature: f32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<StreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [Value]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enable_thinking: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking_budget: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<&'a ThinkingConfigTag>,
}

#[derive(Serialize)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Serialize)]
struct ResponseFormat<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
}

#[derive(Deserialize)]
struct SseChunk {
    #[serde(default)]
    choices: Vec<SseChoice>,
    usage: Option<SseUsage>,
}

#[derive(Deserialize)]
struct SseChoice {
    delta: Option<SseDelta>,
}

#[derive(Deserialize)]
struct SseToolCallDelta {
    #[serde(default)]
    index: usize,
    id: Option<String>,
    function: Option<SseToolFunctionDelta>,
}

#[derive(Deserialize)]
struct SseToolFunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Deserialize)]
struct SseDelta {
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    reasoning: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<SseToolCallDelta>>,
}

#[derive(Debug, Clone)]
pub enum StreamDeltaChunk<'a> {
    Content(&'a str),
    Reasoning(&'a str),
    RetryNotice(String),
}

use crate::models::ExperimentConfig;

pub fn is_transient_http_status(status: u16) -> bool {
    status == 429 || (500..=599).contains(&status)
}

pub fn is_context_length_exceeded_error(status: u16, lower_err: &str) -> bool {
    if !matches!(status, 400 | 413 | 422) {
        return false;
    }
    lower_err.contains("context_length_exceeded")
        || lower_err.contains("maximum context length")
        || lower_err.contains("context window")
        || lower_err.contains("too many tokens")
        || lower_err.contains("token limit")
        || lower_err.contains("exceeds the maximum")
        || lower_err.contains("prompt is too long")
        || lower_err.contains("reduce the length of the messages")
}

pub fn classify_fatal_http_error(status: u16, err_text: &str) -> Option<String> {
    let lower = err_text.to_lowercase();
    if status == 401 {
        return Some(format!(
            "API 认证失败 (401)：请检查 API Key 是否有效或已过期。原始信息: {}",
            err_text.trim()
        ));
    }
    if status == 403 {
        return Some(format!(
            "API 拒绝访问或额度不足 (403)：请检查账号权限或余额。原始信息: {}",
            err_text.trim()
        ));
    }
    if is_context_length_exceeded_error(status, &lower) {
        return Some(format!(
            "上下文长度超出模型上限 ({status})：请适当调小「上下文窗口」或手动折叠历史记录。原始信息: {}",
            err_text.trim()
        ));
    }
    None
}

pub fn retry_backoff_secs(attempt_zero_based: u32) -> u64 {
    1u64 << attempt_zero_based.min(4)
}

pub fn is_transient_reqwest_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request() || err.is_body()
}

#[derive(Debug, Clone, Default)]
pub struct StreamCompletionResult {
    pub content: String,
    pub reasoning_content: String,
    pub tool_calls: Vec<ApiToolCall>,
    pub usage: TokenUsage,
    pub duration_ms: u64,
    pub downgraded_params: Vec<String>,
}

#[derive(Deserialize)]
struct SseUsage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
}

pub async fn stream_chat_completion<F>(
    client: &Client,
    node: &NodeConfig,
    messages: &[ApiChatMessage],
    tools: Option<&[Value]>,
    thinking_params: &ResolvedThinkingParams,
    mut cancel_rx: watch::Receiver<u64>,
    mut on_delta: F,
) -> Result<StreamCompletionResult>
where
    F: FnMut(StreamDeltaChunk<'_>),
{
    if node.api_key.trim().is_empty() {
        return Err(anyhow!("节点「{}」尚未配置 API Key", node.label));
    }
    if node.model_name.trim().is_empty() {
        return Err(anyhow!("节点「{}」尚未配置 Model Name", node.label));
    }

    let url = resolve_chat_completions_url(&node.base_url);
    let initial_gen = *cancel_rx.borrow();
    let started = Instant::now();

    let mut include_usage = true;
    let mut use_tools = tools.is_some();
    let mut use_thinking_params = !thinking_params.is_empty();
    let mut downgraded_params: Vec<String> = Vec::new();
    let mut transient_retries: u32 = 0;
    const MAX_TRANSIENT_RETRIES: u32 = 3;

    let response = loop {
        let body = ChatCompletionRequest {
            model: node.model_name.trim(),
            messages,
            temperature: node.temperature,
            stream: true,
            stream_options: if include_usage {
                Some(StreamOptions {
                    include_usage: true,
                })
            } else {
                None
            },
            response_format: None,
            tools: if use_tools { tools } else { None },
            reasoning_effort: if use_thinking_params {
                thinking_params.reasoning_effort.as_deref()
            } else {
                None
            },
            enable_thinking: if use_thinking_params {
                thinking_params.enable_thinking
            } else {
                None
            },
            thinking_budget: if use_thinking_params {
                thinking_params.thinking_budget
            } else {
                None
            },
            thinking: if use_thinking_params {
                thinking_params.thinking.as_ref()
            } else {
                None
            },
        };

        let req_fut = client
            .post(&url)
            .timeout(std::time::Duration::from_secs(120))
            .bearer_auth(node.api_key.trim())
            .json(&body)
            .send();
        tokio::pin!(req_fut);

        let send_res = loop {
            tokio::select! {
                res = &mut req_fut => {
                    break res;
                }
                _ = cancel_rx.changed() => {
                    if *cancel_rx.borrow() != initial_gen {
                        return Err(anyhow!("__CANCELLED__"));
                    }
                }
            }
        };

        let resp = match send_res {
            Ok(r) => r,
            Err(err) => {
                if is_transient_reqwest_error(&err) && transient_retries < MAX_TRANSIENT_RETRIES {
                    let wait_secs = retry_backoff_secs(transient_retries);
                    transient_retries += 1;
                    on_delta(StreamDeltaChunk::RetryNotice(format!(
                        "网络请求波动，{wait_secs}秒后自动重试 ({transient_retries}/{MAX_TRANSIENT_RETRIES})..."
                    )));
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_secs(wait_secs)) => {}
                        _ = cancel_rx.changed() => {
                            if *cancel_rx.borrow() != initial_gen {
                                return Err(anyhow!("__CANCELLED__"));
                            }
                        }
                    }
                    continue;
                }
                return Err(err).with_context(|| format!("请求 {} 失败", url));
            }
        };

        if resp.status().is_success() {
            break resp;
        }

        let status = resp.status();
        let status_code = status.as_u16();
        let err_text = resp.text().await.unwrap_or_default();
        if let Some(fatal_msg) = classify_fatal_http_error(status_code, &err_text) {
            return Err(anyhow!(fatal_msg));
        }
        let lower_err = err_text.to_lowercase();
        if include_usage
            && (status_code == 400 || status_code == 422)
            && lower_err.contains("stream_options")
        {
            include_usage = false;
            downgraded_params.push("stream_options_stripped".to_string());
            continue;
        }
        if use_tools
            && (status_code == 400 || status_code == 422)
            && (lower_err.contains("tools") || lower_err.contains("function"))
        {
            use_tools = false;
            downgraded_params.push("tools_stripped".to_string());
            continue;
        }
        if use_thinking_params
            && (status_code == 400 || status_code == 422)
            && (lower_err.contains("reasoning")
                || lower_err.contains("thinking")
                || lower_err.contains("budget")
                || lower_err.contains("effort")
                || lower_err.contains("extra_forbidden")
                || lower_err.contains("unrecognized")
                || lower_err.contains("unsupported"))
        {
            use_thinking_params = false;
            downgraded_params.push("thinking_params_stripped".to_string());
            continue;
        }
        if is_transient_http_status(status_code) && transient_retries < MAX_TRANSIENT_RETRIES {
            let wait_secs = retry_backoff_secs(transient_retries);
            transient_retries += 1;
            let label = if status_code == 429 {
                "接口限流 (429)".to_string()
            } else {
                format!("服务瞬态异常 ({status_code})")
            };
            on_delta(StreamDeltaChunk::RetryNotice(format!(
                "{label}，{wait_secs}秒后自动重试 ({transient_retries}/{MAX_TRANSIENT_RETRIES})..."
            )));
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(wait_secs)) => {}
                _ = cancel_rx.changed() => {
                    if *cancel_rx.borrow() != initial_gen {
                        return Err(anyhow!("__CANCELLED__"));
                    }
                }
            }
            continue;
        }
        return Err(anyhow!("API 返回错误 ({}): {}", status, err_text));
    };

    let mut stream = response.bytes_stream();
    let mut line_buf: Vec<u8> = Vec::new();
    let mut full_content = String::new();
    let mut full_reasoning = String::new();
    let mut tool_acc: BTreeMap<usize, ApiToolCall> = BTreeMap::new();
    let mut think_parser = ThinkTagStreamParser::new();
    let mut usage = TokenUsage::default();
    let mut saw_usage = false;

    let process_sse_data = |data: &str,
                            full_content: &mut String,
                            full_reasoning: &mut String,
                            tool_acc: &mut BTreeMap<usize, ApiToolCall>,
                            think_parser: &mut ThinkTagStreamParser,
                            usage: &mut TokenUsage,
                            saw_usage: &mut bool,
                            on_delta: &mut F| {
        if let Ok(parsed) = serde_json::from_str::<SseChunk>(data) {
            if let Some(u) = parsed.usage {
                if u.total_tokens > 0 || u.completion_tokens > 0 || u.prompt_tokens > 0 {
                    *usage = TokenUsage {
                        prompt_tokens: u.prompt_tokens,
                        completion_tokens: u.completion_tokens,
                        total_tokens: if u.total_tokens > 0 {
                            u.total_tokens
                        } else {
                            u.prompt_tokens + u.completion_tokens
                        },
                    };
                    *saw_usage = true;
                }
            }
            if let Some(choice) = parsed.choices.into_iter().next() {
                if let Some(delta) = choice.delta {
                    if let Some(r) = delta.reasoning_content.or(delta.reasoning) {
                        if !r.is_empty() {
                            full_reasoning.push_str(&r);
                            on_delta(StreamDeltaChunk::Reasoning(&r));
                        }
                    }
                    if let Some(tcs) = delta.tool_calls {
                        for tc in tcs {
                            let entry = tool_acc.entry(tc.index).or_insert_with(|| ApiToolCall {
                                id: format!("call_{}", tc.index),
                                kind: "function".to_string(),
                                function: ApiToolFunctionCall {
                                    name: String::new(),
                                    arguments: String::new(),
                                },
                            });
                            if let Some(id) = tc.id {
                                if !id.is_empty() {
                                    entry.id = id;
                                }
                            }
                            if let Some(f) = tc.function {
                                if let Some(name) = f.name {
                                    entry.function.name.push_str(&name);
                                }
                                if let Some(args) = f.arguments {
                                    entry.function.arguments.push_str(&args);
                                }
                            }
                        }
                    }
                    if let Some(text) = delta.content {
                        if !text.is_empty() {
                            for seg in think_parser.push(&text) {
                                match seg {
                                    SanitizedSegment::Reasoning(r) => {
                                        full_reasoning.push_str(&r);
                                        on_delta(StreamDeltaChunk::Reasoning(&r));
                                    }
                                    SanitizedSegment::Content(c) => {
                                        full_content.push_str(&c);
                                        on_delta(StreamDeltaChunk::Content(&c));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    loop {
        let next_chunk = tokio::select! {
            _ = cancel_rx.changed() => {
                if *cancel_rx.borrow() != initial_gen {
                    return Err(anyhow!("__CANCELLED__"));
                }
                continue;
            }
            item = stream.next() => item,
        };

        let Some(chunk_res) = next_chunk else {
            break;
        };
        let bytes = chunk_res.context("读取 SSE 数据流失败")?;
        line_buf.extend_from_slice(&bytes);

        let mut line_start = 0usize;
        while let Some(rel_pos) = line_buf[line_start..].iter().position(|&b| b == b'\n') {
            let line_end = line_start + rel_pos;
            let raw_slice = &line_buf[line_start..line_end];
            let raw_slice = raw_slice.strip_suffix(b"\r").unwrap_or(raw_slice);
            line_start = line_end + 1;

            let line = String::from_utf8_lossy(raw_slice);
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(':') {
                continue;
            }
            if let Some(data_payload) = trimmed.strip_prefix("data:") {
                let data = data_payload.trim();
                if data == "[DONE]" {
                    line_buf.drain(..line_start);
                    for seg in think_parser.finish() {
                        match seg {
                            SanitizedSegment::Reasoning(r) => {
                                full_reasoning.push_str(&r);
                                on_delta(StreamDeltaChunk::Reasoning(&r));
                            }
                            SanitizedSegment::Content(c) => {
                                full_content.push_str(&c);
                                on_delta(StreamDeltaChunk::Content(&c));
                            }
                        }
                    }
                    let elapsed = started.elapsed().as_millis() as u64;
                    if !saw_usage {
                        usage = estimate_usage(messages, &full_content, &full_reasoning);
                    }
                    let tool_calls: Vec<ApiToolCall> = tool_acc
                        .into_values()
                        .filter(|tc| !tc.function.name.trim().is_empty())
                        .collect();
                    return Ok(StreamCompletionResult {
                        content: full_content,
                        reasoning_content: full_reasoning,
                        tool_calls,
                        usage,
                        duration_ms: elapsed,
                        downgraded_params: downgraded_params.clone(),
                    });
                }
                process_sse_data(
                    data,
                    &mut full_content,
                    &mut full_reasoning,
                    &mut tool_acc,
                    &mut think_parser,
                    &mut usage,
                    &mut saw_usage,
                    &mut on_delta,
                );
            }
        }
        if line_start > 0 {
            line_buf.drain(..line_start);
        }
    }

    if !line_buf.is_empty() {
        let line = String::from_utf8_lossy(&line_buf);
        let trimmed = line.trim();
        if let Some(data_payload) = trimmed.strip_prefix("data:") {
            let data = data_payload.trim();
            if data != "[DONE]" {
                process_sse_data(
                    data,
                    &mut full_content,
                    &mut full_reasoning,
                    &mut tool_acc,
                    &mut think_parser,
                    &mut usage,
                    &mut saw_usage,
                    &mut on_delta,
                );
            }
        }
    }

    for seg in think_parser.finish() {
        match seg {
            SanitizedSegment::Reasoning(r) => {
                full_reasoning.push_str(&r);
                on_delta(StreamDeltaChunk::Reasoning(&r));
            }
            SanitizedSegment::Content(c) => {
                full_content.push_str(&c);
                on_delta(StreamDeltaChunk::Content(&c));
            }
        }
    }

    let elapsed = started.elapsed().as_millis() as u64;
    if !saw_usage {
        usage = estimate_usage(messages, &full_content, &full_reasoning);
    }
    let tool_calls: Vec<ApiToolCall> = tool_acc
        .into_values()
        .filter(|tc| !tc.function.name.trim().is_empty())
        .collect();
    Ok(StreamCompletionResult {
        content: full_content,
        reasoning_content: full_reasoning,
        tool_calls,
        usage,
        duration_ms: elapsed,
        downgraded_params,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteModelInfo {
    pub id: String,
    #[serde(default)]
    pub owned_by: String,
}

pub fn resolve_models_url(base_url: &str) -> String {
    let mut trimmed = base_url.trim().trim_end_matches('/');
    if let Some(prefix) = trimmed.strip_suffix("/chat/completions") {
        trimmed = prefix.trim_end_matches('/');
    }
    if trimmed.ends_with("/models") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/models")
    }
}

pub fn parse_models_response(val: &Value, strip_google_prefix: bool) -> Vec<RemoteModelInfo> {
    let items: Option<&Vec<Value>> = val
        .get("data")
        .and_then(|v| v.as_array())
        .or_else(|| val.get("models").and_then(|v| v.as_array()))
        .or_else(|| val.as_array());

    let Some(arr) = items else {
        return Vec::new();
    };

    let mut seen = HashSet::new();
    let mut collected: Vec<(i64, RemoteModelInfo)> = Vec::new();

    for item in arr {
        let raw_id = if let Some(s) = item.as_str() {
            s.trim()
        } else {
            item.get("id")
                .or_else(|| item.get("name"))
                .and_then(|v| v.as_str())
                .map(str::trim)
                .unwrap_or("")
        };
        if raw_id.is_empty() {
            continue;
        }
        let id = if strip_google_prefix {
            raw_id.strip_prefix("models/").unwrap_or(raw_id).to_string()
        } else {
            raw_id.to_string()
        };
        if !seen.insert(id.clone()) {
            continue;
        }
        let owned_by = item
            .get("owned_by")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let created = item.get("created").and_then(|v| v.as_i64()).unwrap_or(0);
        collected.push((created, RemoteModelInfo { id, owned_by }));
    }

    // Float newly released models (higher `created` timestamp) to the top when available.
    if collected.iter().any(|(c, _)| *c > 0) {
        collected.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    }

    collected.into_iter().map(|(_, info)| info).collect()
}

pub async fn fetch_provider_models(
    client: &Client,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<RemoteModelInfo>> {
    let trimmed_base = base_url.trim();
    if trimmed_base.is_empty() {
        return Err(anyhow!("请先填写 Base URL"));
    }
    if !trimmed_base.starts_with("http://") && !trimmed_base.starts_with("https://") {
        return Err(anyhow!("Base URL 必须以 http:// 或 https:// 开头"));
    }

    let url = resolve_models_url(trimmed_base);
    let lower_url = url.to_ascii_lowercase();
    let mut req = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(15));

    let key = api_key.trim();
    if !key.is_empty() {
        req = req.bearer_auth(key);
        if lower_url.contains("anthropic.com") {
            req = req
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01");
        }
    }

    let resp = req
        .send()
        .await
        .with_context(|| format!("请求模型列表接口失败: {url}"))?;

    let status = resp.status();
    let body_text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        let preview: String = body_text.chars().take(240).collect();
        let hint = if status.as_u16() == 401 || status.as_u16() == 403 {
            "（请检查 API Key 是否正确或已填写）"
        } else {
            ""
        };
        return Err(anyhow!(
            "拉取模型列表失败 (HTTP {}){}: {}",
            status.as_u16(),
            hint,
            preview.trim()
        ));
    }

    let json_val: Value = serde_json::from_str(&body_text)
        .with_context(|| "接口返回的不是有效的 JSON 格式".to_string())?;
    let strip_google_prefix = lower_url.contains("generativelanguage.googleapis.com");
    let models = parse_models_response(&json_val, strip_google_prefix);
    if models.is_empty() {
        return Err(anyhow!("接口返回成功，但未解析到可用模型列表"));
    }
    Ok(models)
}

fn estimate_usage(messages: &[ApiChatMessage], completion: &str, reasoning: &str) -> TokenUsage {
    let prompt_chars: usize = messages
        .iter()
        .map(|m| m.content_str().chars().count())
        .sum();
    let comp_chars = completion.chars().count() + reasoning.chars().count();
    let prompt_tokens = ((prompt_chars as f64) * 0.75).ceil() as u64;
    let completion_tokens = ((comp_chars as f64) * 0.75).ceil() as u64;
    TokenUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens + completion_tokens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_fatal_http_error() {
        assert!(classify_fatal_http_error(401, "invalid api key").is_some());
        assert!(classify_fatal_http_error(403, "insufficient quota").is_some());
        assert!(classify_fatal_http_error(
            400,
            "This model's maximum context length is 8192 tokens (context_length_exceeded)"
        )
        .is_some());
        assert!(classify_fatal_http_error(400, "unknown parameter: reasoning_effort").is_none());
        assert!(classify_fatal_http_error(429, "rate limit exceeded").is_none());
    }

    #[test]
    fn test_transient_status_and_exponential_backoff() {
        assert!(is_transient_http_status(429));
        assert!(is_transient_http_status(500));
        assert!(is_transient_http_status(502));
        assert!(is_transient_http_status(503));
        assert!(is_transient_http_status(504));
        assert!(is_transient_http_status(524));
        assert!(!is_transient_http_status(400));
        assert!(!is_transient_http_status(401));
        assert_eq!(retry_backoff_secs(0), 1);
        assert_eq!(retry_backoff_secs(1), 2);
        assert_eq!(retry_backoff_secs(2), 4);
    }

    #[test]
    fn test_thinking_params_priority_and_vendor_mapping() {
        let mut cfg = ExperimentConfig::default();
        cfg.intuitive_fast_reply = false;

        // 1. "default" injects nothing
        let mut openai_node = cfg.agents[0].clone();
        openai_node.base_url = "https://api.openai.com/v1".to_string();
        openai_node.model_name = "o3-mini".to_string();
        openai_node.thinking_level = "default".to_string();
        assert!(resolve_thinking_params(&cfg, &openai_node).is_empty());

        // 2. OpenAI standard mapping: off -> "none", low/medium/high -> matching effort
        openai_node.thinking_level = "off".to_string();
        assert_eq!(
            resolve_thinking_params(&cfg, &openai_node).reasoning_effort.as_deref(),
            Some("none")
        );
        openai_node.thinking_level = "high".to_string();
        assert_eq!(
            resolve_thinking_params(&cfg, &openai_node).reasoning_effort.as_deref(),
            Some("high")
        );

        // 3. DashScope / Qwen mapping: off -> enable_thinking:false, low/medium/high -> budget
        let mut qwen_node = openai_node.clone();
        qwen_node.base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string();
        qwen_node.model_name = "qwen-plus".to_string();
        qwen_node.thinking_level = "off".to_string();
        let qwen_off = resolve_thinking_params(&cfg, &qwen_node);
        assert_eq!(qwen_off.enable_thinking, Some(false));
        assert_eq!(qwen_off.thinking_budget, None);

        qwen_node.thinking_level = "medium".to_string();
        let qwen_med = resolve_thinking_params(&cfg, &qwen_node);
        assert_eq!(qwen_med.enable_thinking, Some(true));
        assert_eq!(qwen_med.thinking_budget, Some(4096));

        qwen_node.thinking_level = "high".to_string();
        let qwen_high = resolve_thinking_params(&cfg, &qwen_node);
        assert_eq!(qwen_high.enable_thinking, Some(true));
        assert_eq!(qwen_high.thinking_budget, Some(16384));

        // 4. DeepSeek / Volces mapping
        let mut ds_node = openai_node.clone();
        ds_node.base_url = "https://ark.cn-beijing.volces.com/api/v3".to_string();
        ds_node.model_name = "doubao-seed".to_string();
        ds_node.thinking_level = "off".to_string();
        let ds_off = resolve_thinking_params(&cfg, &ds_node);
        assert_eq!(ds_off.thinking.as_ref().map(|t| t.kind.as_str()), Some("disabled"));
        assert_eq!(ds_off.reasoning_effort, None);

        ds_node.thinking_level = "low".to_string();
        let ds_low = resolve_thinking_params(&cfg, &ds_node);
        assert_eq!(ds_low.thinking.as_ref().map(|t| t.kind.as_str()), Some("enabled"));
        assert_eq!(ds_low.reasoning_effort.as_deref(), Some("low"));

        // 5. Priority 1: intuitive_fast_reply overrides any agent thinking_level ("high")
        cfg.intuitive_fast_reply = true;
        let fast_qwen = resolve_thinking_params(&cfg, &qwen_node);
        assert_eq!(fast_qwen.enable_thinking, Some(false));
        assert_eq!(fast_qwen.thinking_budget, None);

        let fast_ds = resolve_thinking_params(&cfg, &ds_node);
        assert_eq!(fast_ds.thinking.as_ref().map(|t| t.kind.as_str()), Some("disabled"));

        let fast_openai = resolve_thinking_params(&cfg, &openai_node);
        assert_eq!(fast_openai.reasoning_effort.as_deref(), Some("minimal"));
    }

    #[test]
    fn test_resolve_models_url_and_parse_response() {
        assert_eq!(
            resolve_models_url("https://api.deepseek.com"),
            "https://api.deepseek.com/models"
        );
        assert_eq!(
            resolve_models_url("https://api.deepseek.com/v1/"),
            "https://api.deepseek.com/v1/models"
        );
        assert_eq!(
            resolve_models_url("https://api.openai.com/v1/chat/completions"),
            "https://api.openai.com/v1/models"
        );
        assert_eq!(
            resolve_models_url("https://open.bigmodel.cn/api/paas/v4/models"),
            "https://open.bigmodel.cn/api/paas/v4/models"
        );

        let payload = serde_json::json!({
            "object": "list",
            "data": [
                { "id": "old-model", "created": 1700000000, "owned_by": "vendor" },
                { "id": "new-model", "created": 1790000000, "owned_by": "vendor" },
                { "id": "new-model", "created": 1790000000, "owned_by": "vendor" }
            ]
        });
        let parsed = parse_models_response(&payload, false);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].id, "new-model");
        assert_eq!(parsed[1].id, "old-model");

        let gemini_payload = serde_json::json!({
            "data": [
                { "id": "models/gemini-4-hydrogen", "owned_by": "google" }
            ]
        });
        let parsed_gemini = parse_models_response(&gemini_payload, true);
        assert_eq!(parsed_gemini[0].id, "gemini-4-hydrogen");
    }
}
