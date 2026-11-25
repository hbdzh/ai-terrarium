use std::collections::BTreeMap;
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::watch;

use crate::models::{
    ExperimentConfig, NodeConfig, SpeakerRole, TimelineMessage, ANTI_PLEASANTRY_CONSTRAINT,
};
use crate::sandbox_tools::{ApiToolCall, ApiToolFunctionCall};
use crate::sanitizer::{strip_think_tags, SanitizedSegment, ThinkTagStreamParser};

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

pub struct FlippedPromptPayload {
    pub messages: Vec<ApiChatMessage>,
    pub consumed_whisper_ids: Vec<String>,
}

/// First-Person Perspective Flipper with Memory Compaction support for N agents.
#[cfg(test)]
pub fn build_flipped_messages(
    speaker_agent_id: &str,
    config: &ExperimentConfig,
    timeline: &[TimelineMessage],
) -> FlippedPromptPayload {
    build_flipped_messages_with_memory(speaker_agent_id, config, timeline, "", 0)
}

pub fn build_flipped_messages_with_memory(
    speaker_agent_id: &str,
    config: &ExperimentConfig,
    timeline: &[TimelineMessage],
    compacted_summary: &str,
    compacted_until_seq: i64,
) -> FlippedPromptPayload {
    let node_cfg = config
        .find_agent(speaker_agent_id)
        .or_else(|| config.agents.first())
        .expect("experiment must have at least one agent");

    // Collect all uncompacted conversational messages (Ai, GodPublic) after compacted_until_seq
    let windowed_conv: Vec<&TimelineMessage> = timeline
        .iter()
        .filter(|m| {
            m.seq > compacted_until_seq
                && matches!(m.role, SpeakerRole::Ai | SpeakerRole::GodPublic)
        })
        .collect();

    let god_system_events: Vec<&TimelineMessage> = timeline
        .iter()
        .filter(|m| m.role == SpeakerRole::GodSystem && m.seq > compacted_until_seq)
        .collect();

    let pending_whispers: Vec<&TimelineMessage> = timeline
        .iter()
        .filter(|m| {
            m.role == SpeakerRole::GodWhisper
                && m.agent_id.as_deref() == Some(speaker_agent_id)
                && !m.whisper_consumed
        })
        .collect();

    let is_free_mode = config.preset_id == "free";
    let global_prompt = config.global_system_prompt.trim();
    let base_prompt = node_cfg.system_prompt.trim();

    let mut prompt_parts: Vec<String> = Vec::new();
    if !global_prompt.is_empty() {
        prompt_parts.push(format!("【全局通用设定】\n{global_prompt}"));
    }
    if !base_prompt.is_empty() {
        prompt_parts.push(base_prompt.to_string());
    }

    let mut system_text = prompt_parts.join("\n\n");
    if !is_free_mode {
        if system_text.is_empty() {
            system_text = ANTI_PLEASANTRY_CONSTRAINT.trim_start().to_string();
        } else {
            system_text.push_str(ANTI_PLEASANTRY_CONSTRAINT);
        }
    }

    if !compacted_summary.trim().is_empty() {
        if !system_text.is_empty() {
            system_text.push_str("\n\n");
        }
        system_text.push_str("【历史纪要与已确认事实】\n");
        system_text.push_str(compacted_summary.trim());
    }

    if config.enable_board_tools && !is_free_mode {
        if !system_text.is_empty() {
            system_text.push_str("\n\n");
        }
        system_text.push_str("【实验室共享白板工具可用】你可以使用 read_board 查看共享白板，或用 update_board 记录已确认的共识/设定/线索（调用工具后务必继续发表正式对话内容）。");
    }

    if !god_system_events.is_empty() {
        if !system_text.is_empty() {
            system_text.push_str("\n\n");
        }
        system_text.push_str("【当前环境事件（客观发生，所有参与者均已知晓）】");
        for ev in god_system_events {
            system_text.push_str(&format!("\n- {}", ev.content.trim()));
        }
    }

    let mut consumed_whisper_ids = Vec::with_capacity(pending_whispers.len());
    if !pending_whispers.is_empty() {
        if !system_text.is_empty() {
            system_text.push_str("\n\n");
        }
        system_text.push_str("【潜意识低语（仅你独自察觉，其余参与者完全不知情，仅本轮有效，请自然融入你的反应中）】");
        for w in pending_whispers {
            system_text.push_str(&format!("\n- {}", w.content.trim()));
            consumed_whisper_ids.push(w.id.clone());
        }
    }

    let mut result = Vec::with_capacity(windowed_conv.len() + 2);
    if !system_text.trim().is_empty() {
        result.push(ApiChatMessage::text("system", system_text));
    }

    let multi_party = config.agents.len() > 2;
    let mut mapped: Vec<ApiChatMessage> = Vec::with_capacity(windowed_conv.len());
    for m in &windowed_conv {
        // 强制剥离历史思考链（reasoning_content 字段不读取，正文内残留 <think> 标签同步净化）
        let clean_content = strip_think_tags(&m.content);
        let (role, content) = if m.role == SpeakerRole::Ai
            && m.agent_id.as_deref() == Some(speaker_agent_id)
        {
            ("assistant".to_string(), clean_content)
        } else if m.role == SpeakerRole::GodPublic {
            ("user".to_string(), format!("[路人插话]: {}", clean_content))
        } else if multi_party {
            let speaker_label = config.agent_label(m.agent_id.as_deref());
            ("user".to_string(), format!("[{}]: {}", speaker_label, clean_content))
        } else {
            ("user".to_string(), clean_content)
        };

        if let Some(last) = mapped.last_mut() {
            if last.role == role {
                if let Some(existing) = last.content.as_mut() {
                    existing.push_str("\n\n");
                    existing.push_str(&content);
                } else {
                    last.content = Some(content);
                }
                continue;
            }
        }
        mapped.push(ApiChatMessage::text(role, content));
    }

    if mapped.is_empty() {
        let seed = if !compacted_summary.trim().is_empty() {
            "（对话延续中，请承接历史纪要继续）".to_string()
        } else if config.initial_topic.trim().is_empty() {
            if is_free_mode {
                "你好。".to_string()
            } else {
                "你好，请直接开口。".to_string()
            }
        } else {
            format!(
                "请围绕以下引子直接开口表达你的看法（不要客套）：\n{}",
                config.initial_topic.trim()
            )
        };
        mapped.push(ApiChatMessage::text("user", seed));
    } else if mapped.first().map(|m| m.role.as_str()) == Some("assistant") {
        mapped.insert(0, ApiChatMessage::text("user", "（对话延续中）"));
    }

    result.extend(mapped);

    FlippedPromptPayload {
        messages: result,
        consumed_whisper_ids,
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

#[derive(Debug, Clone, Copy)]
pub enum StreamDeltaChunk<'a> {
    Content(&'a str),
    Reasoning(&'a str),
}

#[derive(Debug, Clone, Default)]
pub struct StreamCompletionResult {
    pub content: String,
    pub reasoning_content: String,
    pub tool_calls: Vec<ApiToolCall>,
    pub usage: TokenUsage,
    pub duration_ms: u64,
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
        };

        let req_fut = client
            .post(&url)
            .bearer_auth(node.api_key.trim())
            .json(&body)
            .send();
        tokio::pin!(req_fut);

        let resp = loop {
            tokio::select! {
                res = &mut req_fut => {
                    break res.with_context(|| format!("请求 {} 失败", url))?;
                }
                _ = cancel_rx.changed() => {
                    if *cancel_rx.borrow() != initial_gen {
                        return Err(anyhow!("__CANCELLED__"));
                    }
                }
            }
        };

        if resp.status().is_success() {
            break resp;
        }

        let status = resp.status();
        let err_text = resp.text().await.unwrap_or_default();
        let lower_err = err_text.to_lowercase();
        if include_usage
            && (status.as_u16() == 400 || status.as_u16() == 422)
            && lower_err.contains("stream_options")
        {
            include_usage = false;
            continue;
        }
        if use_tools
            && (status.as_u16() == 400 || status.as_u16() == 422)
            && (lower_err.contains("tools") || lower_err.contains("function"))
        {
            use_tools = false;
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

        while let Some(pos) = line_buf.iter().position(|&b| b == b'\n') {
            let mut raw_line: Vec<u8> = line_buf.drain(..=pos).collect();
            if raw_line.last() == Some(&b'\n') {
                raw_line.pop();
            }
            if raw_line.last() == Some(&b'\r') {
                raw_line.pop();
            }
            let line = String::from_utf8_lossy(&raw_line);
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(':') {
                continue;
            }
            if let Some(data_payload) = trimmed.strip_prefix("data:") {
                let data = data_payload.trim();
                if data == "[DONE]" {
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
    })
}

fn estimate_usage(messages: &[ApiChatMessage], completion: &str, reasoning: &str) -> TokenUsage {
    let prompt_chars: usize = messages.iter().map(|m| m.content_str().chars().count()).sum();
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

    fn sample_msg(
        seq: i64,
        turn: u32,
        role: SpeakerRole,
        agent_id: Option<&str>,
        content: &str,
    ) -> TimelineMessage {
        TimelineMessage {
            id: format!("m-{seq}"),
            experiment_id: "exp-1".to_string(),
            seq,
            turn_number: turn,
            role,
            agent_id: agent_id.map(|s| s.to_string()),
            reasoning_content: String::new(),
            content: content.to_string(),
            model_name: "test".to_string(),
            duration_ms: 100,
            prompt_tokens: 10,
            completion_tokens: 10,
            total_tokens: 20,
            whisper_consumed: false,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_role_flipping_and_whisper_isolation() {
        let mut cfg = ExperimentConfig::default();
        cfg.preset_id = "philosophy".to_string();
        let timeline = vec![
            sample_msg(1, 1, SpeakerRole::Ai, Some("AiA"), "A1"),
            sample_msg(2, 2, SpeakerRole::Ai, Some("AiB"), "B1"),
            sample_msg(3, 2, SpeakerRole::GodSystem, None, "红雨降临"),
            sample_msg(4, 2, SpeakerRole::GodWhisper, Some("AiA"), "怀疑B在套话"),
            sample_msg(5, 2, SpeakerRole::GodWhisper, Some("AiB"), "不要暴露秘密"),
            sample_msg(6, 2, SpeakerRole::GodPublic, None, "路人甲路过"),
        ];

        let payload_a = build_flipped_messages("AiA", &cfg, &timeline);
        assert_eq!(payload_a.consumed_whisper_ids, vec!["m-4".to_string()]);
        assert!(payload_a.messages[0].content_str().contains("红雨降临"));
        assert!(payload_a.messages[0].content_str().contains("怀疑B在套话"));
        assert!(!payload_a.messages[0].content_str().contains("不要暴露秘密"));
        assert!(payload_a.messages[0].content_str().contains("禁止使用“感谢分享”"));
        assert_eq!(payload_a.messages[1].role, "user");
        assert_eq!(payload_a.messages[2].role, "assistant");
        assert_eq!(payload_a.messages[2].content_str(), "A1");
        assert_eq!(payload_a.messages[3].role, "user");
        assert!(payload_a.messages[3].content_str().contains("B1"));
        assert!(payload_a.messages[3].content_str().contains("[路人插话]: 路人甲路过"));

        let payload_b = build_flipped_messages("AiB", &cfg, &timeline);
        assert_eq!(payload_b.consumed_whisper_ids, vec!["m-5".to_string()]);
        assert!(payload_b.messages[0].content_str().contains("不要暴露秘密"));
        assert!(!payload_b.messages[0].content_str().contains("怀疑B在套话"));
        assert_eq!(payload_b.messages[1].role, "user");
        assert_eq!(payload_b.messages[2].role, "assistant");
        assert_eq!(payload_b.messages[3].role, "user");
    }

    #[test]
    fn test_multi_agent_three_or_more_speaker_labels() {
        let mut cfg = ExperimentConfig::default();
        cfg.agents[0].label = "甲".to_string();
        cfg.agents[1].label = "乙".to_string();
        let mut c_node = cfg.agents[0].clone();
        c_node.id = "ai-3".to_string();
        c_node.label = "丙".to_string();
        cfg.agents.push(c_node);

        let timeline = vec![
            sample_msg(1, 1, SpeakerRole::Ai, Some("AiA"), "我是甲"),
            sample_msg(2, 2, SpeakerRole::Ai, Some("AiB"), "我是乙"),
            sample_msg(3, 3, SpeakerRole::Ai, Some("ai-3"), "我是丙"),
            sample_msg(4, 3, SpeakerRole::GodWhisper, Some("ai-3"), "只对丙说"),
        ];

        let payload_c = build_flipped_messages("ai-3", &cfg, &timeline);
        assert_eq!(payload_c.consumed_whisper_ids, vec!["m-4".to_string()]);
        // System message contains whisper for ai-3
        assert_eq!(payload_c.messages[0].role, "system");
        assert!(payload_c.messages[0].content_str().contains("只对丙说"));
        // Merged user messages for 甲 and 乙 carry speaker labels
        assert_eq!(payload_c.messages[1].role, "user");
        assert!(payload_c.messages[1].content_str().contains("[甲]: 我是甲"));
        assert!(payload_c.messages[1].content_str().contains("[乙]: 我是乙"));
        assert_eq!(payload_c.messages[2].role, "assistant");
        assert_eq!(payload_c.messages[2].content_str(), "我是丙");
    }

    #[test]
    fn test_memory_compaction_cutoff_and_summary_injection() {
        let cfg = ExperimentConfig::default();
        let mut timeline = Vec::new();
        for i in 1..=24 {
            let aid = if i % 2 == 1 { "AiA" } else { "AiB" };
            timeline.push(sample_msg(
                i,
                i as u32,
                SpeakerRole::Ai,
                Some(aid),
                &format!("msg-{i}"),
            ));
        }
        let payload = build_flipped_messages_with_memory(
            "AiA",
            &cfg,
            &timeline,
            "双方围绕时间与熵增达成初步共识。",
            14,
        );
        assert_eq!(payload.messages[0].role, "system");
        assert!(payload.messages[0].content_str().contains("历史纪要与已确认事实"));
        assert!(payload.messages[0]
            .content_str()
            .contains("双方围绕时间与熵增达成初步共识。"));

        let conv_count = payload
            .messages
            .iter()
            .filter(|m| m.role != "system" && m.content_str() != "（对话延续中）")
            .count();
        assert_eq!(conv_count, 10);
        assert!(!payload.messages.iter().any(|m| m.content_str() == "msg-14"));
        assert!(payload.messages.iter().any(|m| m.content_str() == "msg-15"));
    }

    #[test]
    fn test_free_mode_has_no_system_prompt_or_topic() {
        let cfg = ExperimentConfig::default();
        assert_eq!(cfg.preset_id, "free");
        assert!(cfg.initial_topic.is_empty());
        assert!(cfg.global_system_prompt.is_empty());
        assert!(cfg.agents.iter().all(|a| a.system_prompt.is_empty()));

        let payload = build_flipped_messages("AiA", &cfg, &[]);
        assert_eq!(payload.messages.len(), 1);
        assert_eq!(payload.messages[0].role, "user");
        assert_eq!(payload.messages[0].content_str(), "你好。");
    }

    #[test]
    fn test_global_system_prompt_applies_to_all_agents() {
        let mut cfg = ExperimentConfig::default();
        cfg.preset_id = "philosophy".to_string();
        cfg.global_system_prompt = "全程使用古汉语文言文进行辩论。".to_string();
        cfg.agents[0].system_prompt = "你是儒家辩士。".to_string();
        cfg.agents[1].system_prompt = "你是道家隐士。".to_string();

        let p_a = build_flipped_messages("AiA", &cfg, &[]);
        assert!(p_a.messages[0].content_str().contains("全程使用古汉语文言文进行辩论。"));
        assert!(p_a.messages[0].content_str().contains("你是儒家辩士。"));
        assert!(!p_a.messages[0].content_str().contains("你是道家隐士。"));

        let p_b = build_flipped_messages("AiB", &cfg, &[]);
        assert!(p_b.messages[0].content_str().contains("全程使用古汉语文言文进行辩论。"));
        assert!(p_b.messages[0].content_str().contains("你是道家隐士。"));
        assert!(!p_b.messages[0].content_str().contains("你是儒家辩士。"));
    }

    #[test]
    fn test_flipped_messages_strip_reasoning_and_think_tags() {
        let cfg = ExperimentConfig::default();
        let mut m1 = sample_msg(1, 1, SpeakerRole::Ai, Some("AiA"), "<think>A的私密心声</think>公开观点A");
        m1.reasoning_content = "不应该泄露出给B的内心戏".to_string();
        let timeline = vec![m1];

        let payload = build_flipped_messages("AiB", &cfg, &timeline);
        let joined: String = payload
            .messages
            .iter()
            .map(|m| m.content_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!joined.contains("不应该泄露出给B的内心戏"));
        assert!(!joined.contains("A的私密心声"));
        assert!(joined.contains("公开观点A"));
    }
}
