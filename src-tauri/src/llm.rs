use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::watch;

use crate::models::{
    ActivePrivateRoom, ExperimentConfig, NodeConfig, SpeakerRole, TimelineMessage,
    DEFAULT_CORE_ENGINE_PROMPT,
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
    pub has_recent_god_system: bool,
}

/// First-Person Perspective Flipper with Memory Compaction support for N agents.
#[cfg(test)]
pub fn build_flipped_messages(
    speaker_agent_id: &str,
    config: &ExperimentConfig,
    timeline: &[TimelineMessage],
) -> FlippedPromptPayload {
    build_flipped_messages_full(speaker_agent_id, config, timeline, "", 0, "", "")
}

#[allow(dead_code)]
pub fn build_flipped_messages_with_memory(
    speaker_agent_id: &str,
    config: &ExperimentConfig,
    timeline: &[TimelineMessage],
    compacted_summary: &str,
    compacted_until_seq: i64,
    board_content: &str,
) -> FlippedPromptPayload {
    build_flipped_messages_full(
        speaker_agent_id,
        config,
        timeline,
        compacted_summary,
        compacted_until_seq,
        board_content,
        "",
    )
}

#[allow(dead_code)]
pub fn build_flipped_messages_full(
    speaker_agent_id: &str,
    config: &ExperimentConfig,
    timeline: &[TimelineMessage],
    compacted_summary: &str,
    compacted_until_seq: i64,
    board_content: &str,
    private_note: &str,
) -> FlippedPromptPayload {
    build_flipped_messages_with_room(
        speaker_agent_id,
        config,
        timeline,
        compacted_summary,
        compacted_until_seq,
        board_content,
        private_note,
        None,
        &HashMap::new(),
        DEFAULT_CORE_ENGINE_PROMPT,
    )
}

pub fn build_flipped_messages_with_room(
    speaker_agent_id: &str,
    config: &ExperimentConfig,
    timeline: &[TimelineMessage],
    compacted_summary: &str,
    compacted_until_seq: i64,
    board_content: &str,
    private_note: &str,
    active_private_room: Option<&ActivePrivateRoom>,
    room_members: &HashMap<String, Vec<String>>,
    core_engine_prompt: &str,
) -> FlippedPromptPayload {
    let node_cfg = config
        .find_agent(speaker_agent_id)
        .or_else(|| config.agents.first())
        .expect("experiment must have at least one agent");

    // 空间隔离：确定当前发言者可见的非 main 私密房间集合
    let mut visible_rooms: HashSet<&str> = HashSet::new();
    visible_rooms.insert("main");
    let speaker_in_active_room = active_private_room
        .filter(|r| r.remaining_turns > 0 && r.member_ids.iter().any(|id| id == speaker_agent_id));
    if let Some(r) = speaker_in_active_room {
        visible_rooms.insert(r.room_id.as_str());
    }
    for (rid, members) in room_members {
        if members.iter().any(|id| id == speaker_agent_id) {
            visible_rooms.insert(rid.as_str());
        }
    }

    const MAX_HISTORICAL_PRIVATE_ROOM_MSGS: usize = 12;

    // 历史私密房间消息：即使主房间推进了 compacted_until_seq，仍保留当前发言者参与过的最近若干条私密房间原话
    let historical_private_seqs: HashSet<i64> = timeline
        .iter()
        .filter(|m| {
            m.room_id != "main"
                && m.seq <= compacted_until_seq
                && matches!(m.role, SpeakerRole::Ai | SpeakerRole::GodPublic)
                && visible_rooms.contains(m.room_id.as_str())
        })
        .rev()
        .take(MAX_HISTORICAL_PRIVATE_ROOM_MSGS)
        .map(|m| m.seq)
        .collect();

    // Collect all uncompacted conversational messages (Ai, GodPublic) visible to this speaker
    let windowed_conv: Vec<&TimelineMessage> = timeline
        .iter()
        .filter(|m| {
            matches!(m.role, SpeakerRole::Ai | SpeakerRole::GodPublic)
                && visible_rooms.contains(m.room_id.as_str())
                && (m.seq > compacted_until_seq || historical_private_seqs.contains(&m.seq))
        })
        .collect();

    let agent_count = config.agents.len().max(1);
    let god_system_events: Vec<&TimelineMessage> = timeline
        .iter()
        .filter(|m| {
            if m.role != SpeakerRole::GodSystem
                || m.seq <= compacted_until_seq
                || !visible_rooms.contains(m.room_id.as_str())
            {
                return false;
            }
            // 滑动时效窗口：仅注入最近 1 个完整轮次内（同空间后续 AI 发言数 < agents.len()）的环境事件
            let ai_turns_after = timeline
                .iter()
                .filter(|other| {
                    other.seq > m.seq
                        && other.role == SpeakerRole::Ai
                        && other.room_id == m.room_id
                })
                .count();
            ai_turns_after < agent_count
        })
        .collect();
    let has_recent_god_system = !god_system_events.is_empty();

    let pending_whispers: Vec<&TimelineMessage> = timeline
        .iter()
        .filter(|m| {
            m.role == SpeakerRole::GodWhisper
                && m.agent_id.as_deref() == Some(speaker_agent_id)
                && !m.whisper_consumed
        })
        .collect();

    let has_god_public = windowed_conv
        .iter()
        .any(|m| m.role == SpeakerRole::GodPublic);

    let is_free_mode = config.preset_id == "free";
    let skip_core_roleplay = is_free_mode || config.preset_id == "late_night_friends";
    let core_prompt = core_engine_prompt.trim();
    let global_prompt = config.global_system_prompt.trim();
    let base_prompt = node_cfg.system_prompt.trim();
    let show_board = config.enable_board_tools && !is_free_mode;

    // =========================================================================
    // 第一段：静态四层金字塔 System Prompt（整局一字不变，100% 命中前缀缓存）
    //   ① 系统底层演戏总纲（非自由模式且非坦诚 AI 闲聊局时）
    //   ② 当前场景与文风规范（global_system_prompt）
    //   ③ 你的身份与私密底牌（agent.system_prompt）
    //   ④ 可用工具说明（白板 / 私密备忘工具声明）
    // =========================================================================
    let mut system_parts: Vec<String> = Vec::new();
    if !skip_core_roleplay && !core_prompt.is_empty() {
        system_parts.push(format!("【系统底层演戏总纲】\n{core_prompt}"));
    }
    if !global_prompt.is_empty() {
        system_parts.push(format!("【当前场景与文风规范】\n{global_prompt}"));
    }
    if !base_prompt.is_empty() {
        system_parts.push(format!("【你的身份与私密底牌】\n{base_prompt}"));
    }
    let mut tool_docs: Vec<&str> = Vec::new();
    if show_board {
        tool_docs.push(
            "- update_board：当出现新的已确认事实、共识或线索时，用于更新实验室共享白板（严禁使用第一人称或泄露隐藏身份；调用后务必继续发表正式对话）。",
        );
    }
    if config.enable_private_notes {
        tool_docs.push(
            "- update_private_note：用于更新仅你本人可见的私密备忘（≤150字），记录怀疑对象、暗中推理或行动计划；其他角色无法看到。",
        );
    }
    if !tool_docs.is_empty() {
        system_parts.push(format!("【可用工具说明】\n{}", tool_docs.join("\n")));
    }

    let system_text = system_parts.join("\n\n");
    let mut result = Vec::with_capacity(windowed_conv.len() + 3);
    if !system_text.trim().is_empty() {
        result.push(ApiChatMessage::text("system", system_text));
    }

    // =========================================================================
    // 第二段：纯净对话流（历史纪要 + 严格交替的翻转对话历史）
    // =========================================================================
    let multi_party = config.agents.len() > 2;
    let add_speaker_prefix = multi_party || has_god_public;
    let mut mapped: Vec<ApiChatMessage> = Vec::with_capacity(windowed_conv.len() + 1);
    let mut last_was_god_public = false;
    for m in &windowed_conv {
        // 强制剥离历史思考链（reasoning_content 字段不读取，正文内残留 <think> 标签同步净化）
        let clean_content = strip_think_tags(&m.content);
        let is_god_public = m.role == SpeakerRole::GodPublic;
        let is_private_room = m.room_id != "main";
        let (role, content) = if m.role == SpeakerRole::Ai
            && m.agent_id.as_deref() == Some(speaker_agent_id)
        {
            if is_private_room {
                ("assistant".to_string(), format!("[私密里屋] {}", clean_content))
            } else {
                ("assistant".to_string(), clean_content)
            }
        } else if is_god_public {
            ("user".to_string(), format!("[路人插话]: {}", clean_content))
        } else if add_speaker_prefix {
            let speaker_label = config.agent_label(m.agent_id.as_deref());
            if is_private_room {
                (
                    "user".to_string(),
                    format!("[私密里屋 · {}]: {}", speaker_label, clean_content),
                )
            } else {
                ("user".to_string(), format!("[{}]: {}", speaker_label, clean_content))
            }
        } else if is_private_room {
            ("user".to_string(), format!("[私密里屋]: {}", clean_content))
        } else {
            ("user".to_string(), clean_content)
        };

        if let Some(last) = mapped.last_mut() {
            if last.role == role {
                let sep = if is_god_public || last_was_god_public {
                    "\n\n---\n"
                } else {
                    "\n\n"
                };
                if let Some(existing) = last.content.as_mut() {
                    existing.push_str(sep);
                    existing.push_str(&content);
                } else {
                    last.content = Some(content);
                }
                last_was_god_public = is_god_public;
                continue;
            }
        }
        mapped.push(ApiChatMessage::text(role, content));
        last_was_god_public = is_god_public;
    }

    if mapped.is_empty() {
        let seed = if !compacted_summary.trim().is_empty() {
            format!(
                "【历史纪要与已确认事实】\n{}\n\n（对话延续中，请承接历史纪要继续）",
                compacted_summary.trim()
            )
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
    } else {
        if !compacted_summary.trim().is_empty() {
            let summary_block = format!(
                "【历史纪要与已确认事实】\n{}",
                compacted_summary.trim()
            );
            if mapped.first().map(|m| m.role.as_str()) == Some("user") {
                if let Some(first_user) = mapped.first_mut() {
                    let prev = first_user.content.take().unwrap_or_default();
                    first_user.content = Some(format!("{summary_block}\n\n---\n\n{prev}"));
                }
            } else {
                mapped.insert(0, ApiChatMessage::text("user", summary_block));
            }
        } else if mapped.first().map(|m| m.role.as_str()) == Some("assistant") {
            mapped.insert(0, ApiChatMessage::text("user", "（对话延续中）"));
        }
    }

    // =========================================================================
    // 第三段：动态尾部态势板（统一收拢进最后一条 user 消息末尾，空项自动省略）
    // =========================================================================
    let mut consumed_whisper_ids = Vec::with_capacity(pending_whispers.len());
    let mut status_items: Vec<String> = Vec::new();

    if show_board && !board_content.trim().is_empty() {
        status_items.push(format!(
            "【共享白板（全员实时可见）】\n{}",
            board_content.trim()
        ));
    }
    if config.enable_private_notes && !private_note.trim().is_empty() {
        status_items.push(format!(
            "【你的私密备忘（仅本人可见，严禁暴露给他人）】\n{}",
            private_note.trim()
        ));
    }
    if let Some(room) = speaker_in_active_room {
        let partner_names: Vec<String> = room
            .member_ids
            .iter()
            .filter(|id| id.as_str() != speaker_agent_id)
            .map(|id| config.agent_label(Some(id)))
            .collect();
        status_items.push(format!(
            "【当前空间位置】临时私密里屋（剩余 {} 轮），仅你与「{}」互相可见，大厅其余角色完全听不到你们的对话。",
            room.remaining_turns,
            partner_names.join("、")
        ));
    }

    let mut normal_events: Vec<&str> = Vec::new();
    let mut finale_events: Vec<&str> = Vec::new();
    for ev in &god_system_events {
        let txt = ev.content.trim();
        if txt.is_empty() {
            continue;
        }
        if txt.contains("【导演节拍器 · 终幕倒计时】") {
            finale_events.push(txt);
        } else {
            normal_events.push(txt);
        }
    }
    if !normal_events.is_empty() {
        let lines: Vec<String> = normal_events.into_iter().map(|e| format!("- {e}")).collect();
        status_items.push(format!("【近期环境事件（客观发生，全员已知）】\n{}", lines.join("\n")));
    }
    if !finale_events.is_empty() {
        status_items.push(finale_events.join("\n"));
    }
    if !pending_whispers.is_empty() {
        let mut lines = Vec::with_capacity(pending_whispers.len());
        for w in pending_whispers {
            lines.push(format!("- {}", w.content.trim()));
            consumed_whisper_ids.push(w.id.clone());
        }
        status_items.push(format!(
            "【潜意识低语（仅你独自察觉，其余角色不知情，仅本轮有效，请自然融入反应）】\n{}",
            lines.join("\n")
        ));
    }
    if has_god_public || !god_system_events.is_empty() {
        status_items.push(
            "【优先回应规则】对话中出现 [路人插话] 或近期环境事件，请在当前回复中第一时间自然回应或承接，严禁无视。"
                .to_string(),
        );
    }

    if !status_items.is_empty() {
        let status_board = format!("【当前局势与即时状态】\n{}", status_items.join("\n\n"));
        if mapped.last().map(|m| m.role.as_str()) == Some("user") {
            if let Some(last_user) = mapped.last_mut() {
                if let Some(existing) = last_user.content.as_mut() {
                    existing.push_str("\n\n");
                    existing.push_str(&status_board);
                } else {
                    last_user.content = Some(status_board);
                }
            }
        } else {
            mapped.push(ApiChatMessage::text("user", status_board));
        }
    }

    result.extend(mapped);

    FlippedPromptPayload {
        messages: result,
        consumed_whisper_ids,
        has_recent_god_system,
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

pub fn is_transient_http_status(status: u16) -> bool {
    status == 429 || (500..=599).contains(&status)
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
    reasoning_effort: Option<&str>,
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
    let mut use_reasoning_effort = reasoning_effort.is_some();
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
            reasoning_effort: if use_reasoning_effort {
                reasoning_effort
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
        let lower_err = err_text.to_lowercase();
        if include_usage
            && (status_code == 400 || status_code == 422)
            && lower_err.contains("stream_options")
        {
            include_usage = false;
            continue;
        }
        if use_tools
            && (status_code == 400 || status_code == 422)
            && (lower_err.contains("tools") || lower_err.contains("function"))
        {
            use_tools = false;
            continue;
        }
        if use_reasoning_effort
            && (status_code == 400 || status_code == 422)
            && lower_err.contains("reasoning")
        {
            use_reasoning_effort = false;
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
            room_id: "main".to_string(),
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
        assert!(payload_a.messages[0].content_str().contains("【系统底层演戏总纲】"));
        assert!(payload_a.messages[0].content_str().contains("【可用工具说明】"));
        assert!(!payload_a.messages[0].content_str().contains("红雨降临"));
        assert!(!payload_a.messages[0].content_str().contains("怀疑B在套话"));

        assert_eq!(payload_a.messages[1].role, "user");
        assert_eq!(payload_a.messages[2].role, "assistant");
        assert_eq!(payload_a.messages[2].content_str(), "A1");
        assert_eq!(payload_a.messages[3].role, "user");
        let tail_a = payload_a.messages[3].content_str();
        assert!(tail_a.contains("[智能体 2]: B1"));
        assert!(tail_a.contains("---\n[路人插话]: 路人甲路过"));
        assert!(tail_a.contains("【当前局势与即时状态】"));
        assert!(tail_a.contains("红雨降临"));
        assert!(tail_a.contains("怀疑B在套话"));
        assert!(!tail_a.contains("不要暴露秘密"));
        assert!(tail_a.contains("【优先回应规则】"));

        let payload_b = build_flipped_messages("AiB", &cfg, &timeline);
        assert_eq!(payload_b.consumed_whisper_ids, vec!["m-5".to_string()]);
        let tail_b = payload_b.messages.last().unwrap().content_str();
        assert!(tail_b.contains("不要暴露秘密"));
        assert!(!tail_b.contains("怀疑B在套话"));
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
        // Free mode with empty prompts emits no system message; merged user messages for 甲 and 乙 carry speaker labels; whisper sits in trailing user status board
        assert_eq!(payload_c.messages[0].role, "user");
        assert!(payload_c.messages[0].content_str().contains("[甲]: 我是甲"));
        assert!(payload_c.messages[0].content_str().contains("[乙]: 我是乙"));
        assert_eq!(payload_c.messages[1].role, "assistant");
        assert_eq!(payload_c.messages[1].content_str(), "我是丙");
        assert_eq!(payload_c.messages[2].role, "user");
        assert!(payload_c.messages[2].content_str().contains("只对丙说"));
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
            "",
        );
        // 历史纪要注入对话流开头，不污染 system 前缀缓存
        assert_eq!(payload.messages[0].role, "user");
        assert!(payload.messages[0].content_str().contains("历史纪要与已确认事实"));
        assert!(payload.messages[0]
            .content_str()
            .contains("双方围绕时间与熵增达成初步共识。"));

        let conv_count = payload
            .messages
            .iter()
            .filter(|m| m.role != "system" && m.content_str() != "（对话延续中）")
            .count();
        assert_eq!(conv_count, 11);
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

    #[test]
    fn test_god_system_sliding_window_expires_after_one_round() {
        let mut cfg = ExperimentConfig::default();
        cfg.preset_id = "casual_chat".to_string();
        let timeline = vec![
            sample_msg(1, 1, SpeakerRole::Ai, Some("AiA"), "A1"),
            sample_msg(2, 2, SpeakerRole::GodSystem, None, "突然停电了"),
            sample_msg(3, 2, SpeakerRole::Ai, Some("AiB"), "B1"),
        ];

        // 2 个 AI 中只有 1 个在事件后发言，仍在 1 轮窗口内（位于动态尾部态势板）
        let p_mid = build_flipped_messages("AiA", &cfg, &timeline);
        assert!(p_mid.messages.last().unwrap().content_str().contains("突然停电了"));

        // 再经历 1 次 AI 发言后，已满 1 个完整轮次（2 个 AI 发言），事件应过期
        let mut timeline_after = timeline;
        timeline_after.push(sample_msg(4, 3, SpeakerRole::Ai, Some("AiA"), "A2"));
        let p_expired = build_flipped_messages("AiB", &cfg, &timeline_after);
        assert!(!p_expired
            .messages
            .iter()
            .any(|m| m.content_str().contains("突然停电了")));
    }

    #[test]
    fn test_clean_three_stage_pyramid_and_no_hardcoded_anchor() {
        let mut cfg = ExperimentConfig::default();
        cfg.preset_id = "casual_chat".to_string();
        cfg.enable_private_notes = true;
        cfg.global_system_prompt = "场景规则：简短口语".to_string();
        cfg.agents[0].system_prompt = "角色人设：店员阿哲".to_string();
        let timeline = vec![
            sample_msg(1, 1, SpeakerRole::Ai, Some("AiB"), "外面雨真大"),
            sample_msg(2, 1, SpeakerRole::GodSystem, None, "收音机响了"),
        ];

        // 开启或关闭 enable_prefix_cache_opt 都走唯一洁净三段式
        for cache_opt in [true, false] {
            cfg.enable_prefix_cache_opt = cache_opt;
            let p = build_flipped_messages_full(
                "AiA",
                &cfg,
                &timeline,
                "早期纪要内容",
                0,
                "- 停电发生在凌晨三点",
                "怀疑老陈在套话",
            );
            assert!(p.has_recent_god_system);

            // 1. system 静态四层金字塔（不含动态白板/私密备忘/事件/纪要）
            let sys = p.messages[0].content_str();
            let pos_core = sys.find("【系统底层演戏总纲】").unwrap();
            let pos_scene = sys.find("【当前场景与文风规范】").unwrap();
            let pos_identity = sys.find("【你的身份与私密底牌】").unwrap();
            let pos_tools = sys.find("【可用工具说明】").unwrap();
            assert!(pos_core < pos_scene);
            assert!(pos_scene < pos_identity);
            assert!(pos_identity < pos_tools);
            assert!(!sys.contains("早期纪要内容"));
            assert!(!sys.contains("停电发生在凌晨三点"));
            assert!(!sys.contains("怀疑老陈在套话"));
            assert!(!sys.contains("收音机响了"));

            // 2. 纯净对话流（纪要 + 对话）
            let first_user = p.messages[1].content_str();
            assert!(first_user.contains("【历史纪要与已确认事实】"));
            assert!(first_user.contains("早期纪要内容"));
            assert!(first_user.contains("外面雨真大"));

            // 3. 动态尾部态势板（合并在最后一条 user，无硬编码系统锚点）
            let last_user = p.messages.iter().rev().find(|m| m.role == "user").unwrap();
            let tail = last_user.content_str();
            assert!(tail.contains("【当前局势与即时状态】"));
            assert!(tail.contains("- 停电发生在凌晨三点"));
            assert!(tail.contains("怀疑老陈在套话"));
            assert!(tail.contains("收音机响了"));
            assert!(!tail.contains("[系统锚点："));
        }

        // 空项自动省略：空白板与空私密备忘不渲染占位
        let p_empty = build_flipped_messages_full("AiA", &cfg, &[], "", 0, "   ", "   ");
        let joined: String = p_empty
            .messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| m.content_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!joined.contains("【当前局势与即时状态】"));
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
    fn test_private_room_visibility_isolation() {
        let mut cfg = ExperimentConfig::default();
        cfg.preset_id = "casual_chat".to_string();
        let mut third = cfg.agents[0].clone();
        third.id = "AiC".to_string();
        third.label = "林医生".to_string();
        cfg.agents.push(third);

        let room = ActivePrivateRoom {
            room_id: "room-123".to_string(),
            member_ids: vec!["AiA".to_string(), "AiB".to_string()],
            remaining_turns: 1,
            title: "里屋密谈".to_string(),
        };
        let mut room_members = HashMap::new();
        room_members.insert(
            "room-123".to_string(),
            vec!["AiA".to_string(), "AiB".to_string()],
        );

        let mut priv_msg = sample_msg(2, 2, SpeakerRole::Ai, Some("AiA"), "我有黑箱子的密码");
        priv_msg.room_id = "room-123".to_string();

        let timeline = vec![
            sample_msg(1, 1, SpeakerRole::Ai, Some("AiC"), "大厅茶真香"),
            priv_msg,
        ];

        // Room member AiB sees the private room message
        let p_b = build_flipped_messages_with_room(
            "AiB",
            &cfg,
            &timeline,
            "",
            0,
            "",
            "",
            Some(&room),
            &room_members,
            DEFAULT_CORE_ENGINE_PROMPT,
        );
        let joined_b: String = p_b
            .messages
            .iter()
            .map(|m| m.content_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined_b.contains("我有黑箱子的密码"));
        assert!(joined_b.contains("临时私密里屋"));

        // Lobby agent AiC CANNOT see private room message
        let p_c = build_flipped_messages_with_room(
            "AiC",
            &cfg,
            &timeline,
            "",
            0,
            "",
            "",
            Some(&room),
            &room_members,
            DEFAULT_CORE_ENGINE_PROMPT,
        );
        let joined_c: String = p_c
            .messages
            .iter()
            .map(|m| m.content_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!joined_c.contains("我有黑箱子的密码"));
        assert!(joined_c.contains("大厅茶真香"));
    }

    #[test]
    fn test_historical_private_room_messages_survive_main_compaction() {
        let mut cfg = ExperimentConfig::default();
        let mut agent_c = cfg.agents[0].clone();
        agent_c.id = "AiC".to_string();
        agent_c.label = "丙".to_string();
        cfg.agents.push(agent_c);
        let mut room_members = HashMap::new();
        room_members.insert(
            "room-secret".to_string(),
            vec!["AiA".to_string(), "AiB".to_string()],
        );

        let mut priv_msg = sample_msg(2, 2, SpeakerRole::Ai, Some("AiA"), "深夜密谋暗号777");
        priv_msg.room_id = "room-secret".to_string();

        let timeline = vec![
            sample_msg(1, 1, SpeakerRole::Ai, Some("AiA"), "早期大厅寒暄"),
            priv_msg,
            sample_msg(3, 3, SpeakerRole::Ai, Some("AiC"), "后期大厅讨论"),
        ];

        // 主房间已压缩到 seq=2（已越过私密房间消息 seq=2）
        let p_b = build_flipped_messages_with_room(
            "AiB",
            &cfg,
            &timeline,
            "主大厅早期纪要",
            2,
            "",
            "",
            None,
            &room_members,
            DEFAULT_CORE_ENGINE_PROMPT,
        );
        let joined_b: String = p_b
            .messages
            .iter()
            .map(|m| m.content_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!joined_b.contains("早期大厅寒暄"));
        assert!(joined_b.contains("深夜密谋暗号777"));
        assert!(joined_b.contains("后期大厅讨论"));

        // 未参与私密房间的 AiC 依然不可见
        let p_c = build_flipped_messages_with_room(
            "AiC",
            &cfg,
            &timeline,
            "主大厅早期纪要",
            2,
            "",
            "",
            None,
            &room_members,
            DEFAULT_CORE_ENGINE_PROMPT,
        );
        let joined_c: String = p_c
            .messages
            .iter()
            .map(|m| m.content_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!joined_c.contains("深夜密谋暗号777"));
    }
}
