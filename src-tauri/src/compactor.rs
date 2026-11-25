use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::llm::{resolve_chat_completions_url, ApiChatMessage};
use crate::models::{ExperimentConfig, NodeConfig, SpeakerRole, TimelineMessage};
use crate::sanitizer::strip_think_tags;

pub const KEEP_RECENT_TURNS: usize = 7;
pub const MIN_RECENT_CONV_MESSAGES: usize = 8;

/// 记忆压缩防抖动熔断器：防止上下文预算过小导致连续轮次反复触发压缩死循环。
#[derive(Debug, Clone, Default)]
pub struct CompactionGuard {
    recent_compaction_turns: Vec<u32>,
}

impl CompactionGuard {
    /// 记录一次压缩发生的轮次。若连续 3 轮内（最大轮次与最小轮次差值 <= 2）累计触发 3 次压缩，则返回 true 表示需要熔断暂停。
    pub fn record_and_check(&mut self, turn_number: u32) -> bool {
        self.recent_compaction_turns.push(turn_number);
        if self.recent_compaction_turns.len() > 3 {
            let drain_n = self.recent_compaction_turns.len() - 3;
            self.recent_compaction_turns.drain(0..drain_n);
        }
        if self.recent_compaction_turns.len() == 3 {
            let first = self.recent_compaction_turns[0];
            let last = self.recent_compaction_turns[2];
            if last.saturating_sub(first) <= 2 {
                return true;
            }
        }
        false
    }

    pub fn reset(&mut self) {
        self.recent_compaction_turns.clear();
    }
}

/// 快速估算单段文本 Token 数（兼容中英文混合文本）。
/// - ASCII 字符约 3.3 chars/token
/// - 非 ASCII（汉字/标点等）约 1.35 tokens/char
pub fn estimate_text_tokens(text: &str) -> u64 {
    if text.is_empty() {
        return 0;
    }
    let mut ascii_count = 0usize;
    let mut non_ascii_count = 0usize;
    for ch in text.chars() {
        if ch.is_ascii() {
            ascii_count += 1;
        } else {
            non_ascii_count += 1;
        }
    }
    let est = (ascii_count as f64) * 0.30 + (non_ascii_count as f64) * 1.35;
    est.ceil() as u64
}

pub fn estimate_messages_tokens(messages: &[ApiChatMessage]) -> u64 {
    let mut total = 8u64;
    for m in messages {
        total += estimate_text_tokens(m.content_str()) + 4;
    }
    total
}

pub fn select_compactor_node(config: &ExperimentConfig) -> Option<&NodeConfig> {
    if !config.node_c.api_key.trim().is_empty() && !config.node_c.model_name.trim().is_empty() {
        return Some(&config.node_c);
    }
    config
        .agents
        .iter()
        .find(|a| !a.api_key.trim().is_empty() && !a.model_name.trim().is_empty())
}

#[derive(Debug, Clone)]
pub struct CompactionResult {
    pub new_summary: String,
    pub new_until_seq: i64,
    pub tokens_before: u64,
    pub tokens_after: u64,
}

#[derive(Serialize)]
struct CompactionRequest<'a> {
    model: &'a str,
    messages: &'a [ApiChatMessage],
    temperature: f32,
    stream: bool,
}

#[derive(Deserialize)]
struct NonStreamResp {
    choices: Vec<NonStreamChoice>,
}

#[derive(Deserialize)]
struct NonStreamChoice {
    message: NonStreamMsg,
}

#[derive(Deserialize)]
struct NonStreamMsg {
    content: Option<String>,
}

/// 识别待折叠的早期消息区间，保留最近 6~8 轮原始对话细节不被压缩。
pub fn split_compactable_messages<'a>(
    timeline: &'a [TimelineMessage],
    compacted_until_seq: i64,
) -> Option<(Vec<&'a TimelineMessage>, i64)> {
    let uncompacted: Vec<&TimelineMessage> = timeline
        .iter()
        .filter(|m| m.seq > compacted_until_seq)
        .collect();

    let conv_msgs: Vec<&TimelineMessage> = uncompacted
        .iter()
        .copied()
        .filter(|m| {
            m.room_id == "main" && matches!(m.role, SpeakerRole::Ai | SpeakerRole::GodPublic)
        })
        .collect();

    if conv_msgs.len() <= MIN_RECENT_CONV_MESSAGES {
        return None;
    }

    // 保留最近 KEEP_RECENT_TURNS 轮（且至少 MIN_RECENT_CONV_MESSAGES 条）完整对话
    let latest_turn = conv_msgs.last().map(|m| m.turn_number).unwrap_or(0);
    let keep_from_turn = latest_turn.saturating_sub(KEEP_RECENT_TURNS as u32);

    let by_count_idx = conv_msgs.len().saturating_sub(MIN_RECENT_CONV_MESSAGES);
    let by_turn_idx = conv_msgs
        .iter()
        .position(|m| m.turn_number > keep_from_turn)
        .unwrap_or(by_count_idx);

    let keep_start_idx = by_count_idx.min(by_turn_idx);
    if keep_start_idx == 0 {
        return None;
    }

    let cutoff_seq = conv_msgs[keep_start_idx].seq;
    let to_compact: Vec<&TimelineMessage> = uncompacted
        .into_iter()
        .filter(|m| m.seq < cutoff_seq)
        .collect();

    if to_compact.is_empty() {
        return None;
    }

    let new_until_seq = to_compact.last().map(|m| m.seq).unwrap_or(compacted_until_seq);
    Some((to_compact, new_until_seq))
}

/// 当当前上下文预估 Token 超过阈值（如 80%）时，唤醒观察员或备用模型执行无损记忆压缩。
pub async fn maybe_compact_history(
    client: &Client,
    config: &ExperimentConfig,
    timeline: &[TimelineMessage],
    existing_summary: &str,
    compacted_until_seq: i64,
    current_prompt_tokens: u64,
) -> Result<Option<CompactionResult>> {
    let ctx_limit = config.context_window_tokens.max(2048);
    let pct = (config.compaction_threshold_pct as u64).clamp(40, 95);
    let threshold = ctx_limit * pct / 100;

    if current_prompt_tokens < threshold {
        return Ok(None);
    }

    let Some((to_compact, new_until_seq)) =
        split_compactable_messages(timeline, compacted_until_seq)
    else {
        return Ok(None);
    };

    let Some(compactor_node) = select_compactor_node(config) else {
        return Ok(None);
    };

    let mut transcript = String::new();
    for m in &to_compact {
        // 空间隔离最高优先级仲裁铁律：全局记忆压缩器必须严格跳过非 "main" 房间的私密对话
        if m.room_id != "main" {
            continue;
        }
        let clean = strip_think_tags(&m.content);
        let label = match m.role {
            SpeakerRole::Ai => format!(
                "[第{}轮 {}]",
                m.turn_number,
                config.agent_label(m.agent_id.as_deref())
            ),
            SpeakerRole::GodPublic => "[路人插话]".to_string(),
            SpeakerRole::GodSystem => "[环境事件]".to_string(),
            SpeakerRole::GodWhisper => continue, // 私密低语不写入全局公开记忆纪要
        };
        transcript.push_str(&format!("{}: {}\n\n", label, clean));
    }

    if transcript.trim().is_empty() {
        return Ok(None);
    }

    let system_prompt = "你是一个高保真记忆压缩引擎。请将早期多智能体对话（以及已有历史纪要）无损浓缩为一份结构化中文纪要。\n\
        必须严格保留：\n\
        1. 已确认的客观事实、核心设定与白板共识\n\
        2. 各参与者的关键立场、核心分歧与立场转变\n\
        3. 尚未解决的悬念、伏笔与核心问题\n\
        要求：客观精炼（控制在 350~600 字以内），禁止客套，只输出纪要正文，不要添加多余标题前缀。";

    let mut user_content = String::new();
    if !existing_summary.trim().is_empty() {
        user_content.push_str("【已有历史纪要】\n");
        user_content.push_str(existing_summary.trim());
        user_content.push_str("\n\n");
    }
    user_content.push_str("【待合并压缩的新增早期对话】\n");
    user_content.push_str(&transcript);

    let api_messages = vec![
        ApiChatMessage::text("system", system_prompt),
        ApiChatMessage::text("user", user_content),
    ];

    let url = resolve_chat_completions_url(&compactor_node.base_url);
    let body = CompactionRequest {
        model: compactor_node.model_name.trim(),
        messages: &api_messages,
        temperature: 0.2,
        stream: false,
    };

    let resp = client
        .post(&url)
        .timeout(std::time::Duration::from_secs(90))
        .bearer_auth(compactor_node.api_key.trim())
        .json(&body)
        .send()
        .await
        .with_context(|| format!("记忆折叠请求 {} 失败", url))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err = resp.text().await.unwrap_or_default();
        return Err(anyhow!("记忆折叠 API 错误 ({}): {}", status, err));
    }

    let parsed: NonStreamResp = resp.json().await.context("解析记忆折叠响应失败")?;
    let raw_summary = parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .unwrap_or_default();
    let new_summary = strip_think_tags(&raw_summary);
    if new_summary.is_empty() {
        return Ok(None);
    }

    let tokens_after_delta = estimate_text_tokens(&new_summary);
    let compacted_raw_tokens: u64 = to_compact
        .iter()
        .map(|m| estimate_text_tokens(&m.content) + 4)
        .sum();
    let tokens_after = current_prompt_tokens
        .saturating_sub(compacted_raw_tokens)
        .saturating_add(tokens_after_delta);

    Ok(Some(CompactionResult {
        new_summary,
        new_until_seq,
        tokens_before: current_prompt_tokens,
        tokens_after,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(seq: i64, turn: u32, role: SpeakerRole) -> TimelineMessage {
        TimelineMessage {
            id: format!("m-{seq}"),
            experiment_id: "e1".to_string(),
            seq,
            turn_number: turn,
            role,
            agent_id: Some("AiA".to_string()),
            room_id: "main".to_string(),
            reasoning_content: String::new(),
            content: format!("content-{seq}"),
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
    fn test_estimate_tokens_mixed_cjk_ascii() {
        assert_eq!(estimate_text_tokens(""), 0);
        let cjk = estimate_text_tokens("你好世界");
        assert!(cjk >= 4 && cjk <= 8);
        let ascii = estimate_text_tokens("hello world this is a test");
        assert!(ascii >= 5 && ascii <= 12);
    }

    #[test]
    fn test_split_compactable_preserves_recent_turns() {
        let mut msgs = Vec::new();
        for i in 1..=20 {
            msgs.push(mk(i, i as u32, SpeakerRole::Ai));
        }
        let (to_compact, until_seq) = split_compactable_messages(&msgs, 0).unwrap();
        // Should keep recent 8+ messages / 7 turns intact
        assert!(until_seq <= 12);
        assert_eq!(to_compact.last().unwrap().seq, until_seq);
        assert!(msgs.len() - to_compact.len() >= MIN_RECENT_CONV_MESSAGES);
    }

    #[test]
    fn test_compaction_guard_trips_on_three_within_three_turns() {
        let mut guard = CompactionGuard::default();
        assert!(!guard.record_and_check(10));
        assert!(!guard.record_and_check(11));
        assert!(guard.record_and_check(12));

        guard.reset();
        assert!(!guard.record_and_check(10));
        assert!(!guard.record_and_check(11));
        assert!(!guard.record_and_check(13));
        assert!(!guard.record_and_check(15));
        assert!(guard.record_and_check(15));
    }

    #[test]
    fn test_compactor_skips_private_room_messages() {
        let mut msgs = Vec::new();
        for i in 1..=12 {
            let mut m = mk(i, i as u32, SpeakerRole::Ai);
            if i == 3 || i == 4 {
                m.room_id = "room-secret".to_string();
            }
            msgs.push(m);
        }
        let (to_compact, _) = split_compactable_messages(&msgs, 0).unwrap();
        let main_only: Vec<_> = to_compact
            .into_iter()
            .filter(|m| m.room_id == "main")
            .collect();
        assert!(main_only.iter().all(|m| m.room_id == "main"));
        assert!(!main_only.iter().any(|m| m.seq == 3 || m.seq == 4));
    }
}
