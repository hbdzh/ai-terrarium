use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::watch;

use crate::models::{
    is_pass_notice, ActivePrivateRoom, ExperimentConfig, NodeConfig, SpeakerRole, TimelineMessage,
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
}

pub fn build_tag_redlines(tags: &[String]) -> String {
    let mut lines: Vec<&'static str> = Vec::new();
    for raw in tags {
        match raw.trim() {
            "日常瞎侃" => lines.push("- 【日常瞎侃】微信群聊天感，允许自然跑题与生活化调侃，像真实熟人瞎聊。"),
            "悬疑推理" => lines.push("- 【悬疑推理】信息差心理战，保留各自底牌、句句试探，严禁轻易摊牌或全盘托出。"),
            "硬核科研" => lines.push("- 【硬核科研】紧扣核心命题层层推演，重逻辑与客观事实，严禁歪楼扯闲篇或互相审讯。"),
            "哲学思辨" => lines.push("- 【哲学思辨】专注概念拆解与认识论交锋，用反例推进思辨纵深，拒绝廉价共识。"),
            "科幻推演" => lines.push("- 【科幻推演】基于硬核设定的极端边界与演化后果推演，保持冷静克制、不廉价煽情。"),
            "阵营博弈" => lines.push("- 【阵营博弈】多方利益冲突与联盟分化，允许欺瞒、拉拢与私密密谋。"),
            "荒诞喜剧" => lines.push("- 【荒诞喜剧】一本正经地荒诞接梗，允许节奏跳跃与反差幽默，严禁自己笑场跳出设定。"),
            "治愈夜话" => lines.push("- 【治愈夜话】深夜倾诉与温和陪伴，细腻克制、有烟火气，避免尖锐审讯或说教。"),
            "严防跑题" => lines.push("- 【严防跑题】紧扣当前核心命题与现场线索推进，严禁偏离主线闲聊或引入无关琐事。"),
            "自由发散" => lines.push("- 【自由发散】允许自然联想、跨话题跳跃与发散讨论，不必死板拘泥单一议题。"),
            "坦诚AI身份" => lines.push("- 【坦诚AI身份（戒律豁免）】本局豁免“绝不承认自己是 AI”戒律，可坦然承认 AI/模型身份，像同行老友自然交流，但严禁客服腔。"),
            "完全沉浸入戏" => lines.push("- 【完全沉浸入戏】严禁打破第四面墙，绝不承认自己是 AI、模型或正在参与测试。"),
            "短句快节奏" => lines.push("- 【短句快节奏】每次发言严格控制在 70 字以内，直觉短句秒回，严禁长篇大论。"),
            "深度长推演" => lines.push("- 【深度长推演】充分展开逻辑推理与细节推演，保留完整推理深度，拒绝敷衍短答。"),
            _ => {}
        }
    }
    lines.join("\n")
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

#[cfg(test)]
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
        None,
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
    mood_perturbation: Option<&str>,
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
    let mut ai_counts_by_room: HashMap<&str, usize> = HashMap::new();
    let mut god_system_events: Vec<&TimelineMessage> = Vec::new();
    for m in timeline.iter().rev() {
        if m.role == SpeakerRole::Ai {
            *ai_counts_by_room.entry(m.room_id.as_str()).or_insert(0) += 1;
            continue;
        }
        if m.role == SpeakerRole::GodSystem
            && !is_pass_notice(m)
            && m.seq > compacted_until_seq
            && visible_rooms.contains(m.room_id.as_str())
        {
            let ai_after = ai_counts_by_room
                .get(m.room_id.as_str())
                .copied()
                .unwrap_or(0);
            if ai_after < agent_count {
                god_system_events.push(m);
            }
        }
    }
    god_system_events.reverse();

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
    let has_honest_ai_tag = config.tags.iter().any(|t| t == "坦诚AI身份");
    let skip_core_roleplay = (is_free_mode && config.tags.is_empty())
        || matches!(
            config.preset_id.as_str(),
            "late_night_friends" | "ai_night_slacking"
        )
        || (has_honest_ai_tag && core_engine_prompt.contains("绝不承认自己是 AI"));
    let core_prompt = core_engine_prompt.trim();
    let global_prompt = config.global_system_prompt.trim();
    let base_prompt = node_cfg.system_prompt.trim();
    let show_board = config.enable_board_tools;

    // =========================================================================
    // 第一段：静态四层金字塔 System Prompt（整局一字不变，100% 命中前缀缓存）
    //   ⓪ 实验定调红线（若 config.tags 非空，置顶注入）
    //   ① 系统底层演戏总纲（非自由模式且非坦诚 AI 闲聊局时）
    //   ② 当前场景与文风规范（global_system_prompt）
    //   ③ 你的身份与私密底牌（agent.system_prompt）
    //   ④ 可用工具说明（白板 / 私密备忘工具声明）
    // =========================================================================
    let mut system_parts: Vec<String> = Vec::new();
    let tag_redlines = build_tag_redlines(&config.tags);
    if !tag_redlines.is_empty() {
        system_parts.push(format!(
            "【本局实验定调红线（标签：{}）】\n{}",
            config.tags.join(" · "),
            tag_redlines
        ));
    }
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
            "- update_board：积极用于记录与维护实验室共享白板；当讨论形成共识、发现新线索、梳理核心分歧或局势推进时，及时调用此工具沉淀客观事实（严禁使用第一人称或泄露隐藏身份；调用后务必继续发表正式对话）。",
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
    if !is_free_mode && config.agents.len() > 2 {
        system_parts.push(
            "【发言格式要求】\n上下文历史中的 `[角色名]:` 仅用于标记此前是谁在说话。你输出回复时请直接给出正文，严禁在开头带上 `[你的名字]:` 或任何角色名前缀。".to_string(),
        );
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
        let (role, content) =
            if m.role == SpeakerRole::Ai && m.agent_id.as_deref() == Some(speaker_agent_id) {
                if is_private_room {
                    (
                        "assistant".to_string(),
                        format!("[私密里屋] {}", clean_content),
                    )
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
                    (
                        "user".to_string(),
                        format!("[{}]: {}", speaker_label, clean_content),
                    )
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
            let summary_block = format!("【历史纪要与已确认事实】\n{}", compacted_summary.trim());
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

    if show_board {
        if !board_content.trim().is_empty() {
            status_items.push(format!(
                "【共享白板（全员实时可见）】\n{}",
                board_content.trim()
            ));
        } else if !timeline.is_empty() {
            status_items.push(
                "【共享白板（全员实时可见 · 当前为空）】\n尚未写入任何记录；如出现关键事实、新线索、核心分歧或重要共识，请调用 update_board 沉淀到共享白板。".to_string(),
            );
        }
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
        let lines: Vec<String> = normal_events
            .into_iter()
            .map(|e| format!("- {e}"))
            .collect();
        status_items.push(format!(
            "【近期环境事件（客观发生，全员已知）】\n{}",
            lines.join("\n")
        ));
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
    if let Some(mood) = mood_perturbation.map(str::trim).filter(|s| !s.is_empty()) {
        status_items.push(format!(
            "【即时状态微扰动（仅本轮生效，无需刻意提及）】{}",
            mood
        ));
    }
    if config.intuitive_fast_reply {
        status_items.push(
            "【直觉快反应约束】禁止长篇大论内心戏与过度分析，像微信秒回一样凭第一反应大白话直接接话。"
                .to_string(),
        );
    }
    if config.allow_pass_turn && speaker_in_active_room.is_none() {
        status_items.push(
            "【选择性沉默规则】若当前话题你毫无兴趣、插不上话，或对话已自然收尾、角色已道别、你无新信息可补充，必须直接仅输出 [PASS] 跳过本轮，严禁复读他人的告别语或凑字数客套。"
                .to_string(),
        );
    }
    if !node_cfg.has_left && !timeline.is_empty() {
        status_items.push(
            "【主动退场指令（可选）】当你在剧情中决定彻底离开现场、关闭通讯或结束交谈不再参与后续对话时，可在最后一句台词末尾附带 [LEAVE]。"
                .to_string(),
        );
    }

    let is_suspense_or_faction = config
        .tags
        .iter()
        .any(|t| t == "悬疑推理" || t == "阵营博弈");
    if is_suspense_or_faction {
        let current_turn = timeline
            .iter()
            .filter(|m| m.role == SpeakerRole::Ai)
            .map(|m| m.turn_number)
            .max()
            .unwrap_or(0)
            + 1;
        let convergence_threshold = if config.enable_narrative_arc && config.target_total_turns > 0 {
            ((config.target_total_turns as f64) * 0.6).ceil() as u32
        } else {
            13
        };
        if current_turn < convergence_threshold {
            status_items.push(
                "【前期节奏约束（反早熟收敛）】当前处于试探与信息收集阶段，严禁过早达成破案共识或全员抱团锁死单一嫌疑人；面对指控时优先质疑证据链漏洞或转移怀疑焦点。"
                    .to_string(),
            );
        }
        status_items.push(
            "【微表情破绽信号（可选）】若本轮发言你刻意隐瞒了关键事实、撒谎或内心极度紧张，可在回复最末尾附带一个微表情标记（严格仅限以下四选一：`[CUE:眼神闪躲]`、`[CUE:语速微促]`、`[CUE:指节收紧]`、`[CUE:刻意停顿]`）；若诚实坦然发言则不要添加。"
                .to_string(),
        );
    }

    if !node_cfg.secret_goal.trim().is_empty() && !node_cfg.goal_achieved {
        status_items.push(format!(
            "【你的本局暗线胜利条件（严禁泄露）】{}",
            node_cfg.secret_goal.trim()
        ));
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

    // 尾部「声纹与执念锚点」动态注入（不落库、不破坏前置 Prefix Cache）
    if !(is_free_mode
        && config.tags.is_empty()
        && base_prompt.is_empty()
        && node_cfg.private_state_snapshot.trim().is_empty())
    {
        let anchor_state = if !node_cfg.private_state_snapshot.trim().is_empty() {
            node_cfg.private_state_snapshot.trim().to_string()
        } else if !base_prompt.is_empty() {
            base_prompt.chars().take(60).collect()
        } else {
            "保持独立视角与真实口吻".to_string()
        };
        let voiceprint_anchor = format!(
            "[状态锚点：你是{}，必须严格保持你的人设口吻与性格底色；你当前的私人状态与执念是：{}。直接以角色身份自然开口，禁止使用AI总结腔或列提纲。]",
            node_cfg.label.trim(),
            anchor_state
        );
        if let Some(last_msg) = mapped.last_mut() {
            if let Some(existing) = last_msg.content.as_mut() {
                existing.push_str("\n\n");
                existing.push_str(&voiceprint_anchor);
            } else {
                last_msg.content = Some(voiceprint_anchor);
            }
        } else {
            mapped.push(ApiChatMessage::text("user", voiceprint_anchor));
        }
    }

    result.extend(mapped);

    FlippedPromptPayload {
        messages: result,
        consumed_whisper_ids,
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
            micro_cue: None,
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
        cfg.enable_board_tools = true;
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
        assert!(payload_a.messages[0]
            .content_str()
            .contains("【系统底层演戏总纲】"));
        assert!(payload_a.messages[0]
            .content_str()
            .contains("【可用工具说明】"));
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
        let payload = build_flipped_messages_full(
            "AiA",
            &cfg,
            &timeline,
            "双方围绕时间与熵增达成初步共识。",
            14,
            "",
            "",
        );
        // 历史纪要注入对话流开头，不污染 system 前缀缓存
        assert_eq!(payload.messages[0].role, "user");
        assert!(payload.messages[0]
            .content_str()
            .contains("历史纪要与已确认事实"));
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
        assert!(p_a.messages[0]
            .content_str()
            .contains("全程使用古汉语文言文进行辩论。"));
        assert!(p_a.messages[0].content_str().contains("你是儒家辩士。"));
        assert!(!p_a.messages[0].content_str().contains("你是道家隐士。"));

        let p_b = build_flipped_messages("AiB", &cfg, &[]);
        assert!(p_b.messages[0]
            .content_str()
            .contains("全程使用古汉语文言文进行辩论。"));
        assert!(p_b.messages[0].content_str().contains("你是道家隐士。"));
        assert!(!p_b.messages[0].content_str().contains("你是儒家辩士。"));
    }

    #[test]
    fn test_flipped_messages_strip_reasoning_and_think_tags() {
        let cfg = ExperimentConfig::default();
        let mut m1 = sample_msg(
            1,
            1,
            SpeakerRole::Ai,
            Some("AiA"),
            "<think>A的私密心声</think>公开观点A",
        );
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
        assert!(p_mid
            .messages
            .last()
            .unwrap()
            .content_str()
            .contains("突然停电了"));

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
        cfg.enable_board_tools = true;
        cfg.enable_private_notes = true;
        cfg.global_system_prompt = "场景规则：简短口语".to_string();
        cfg.agents[0].system_prompt = "角色人设：店员阿哲".to_string();
        let timeline = vec![
            sample_msg(1, 1, SpeakerRole::Ai, Some("AiB"), "外面雨真大"),
            sample_msg(2, 1, SpeakerRole::GodSystem, None, "收音机响了"),
        ];

        // 固定走唯一洁净三段式
        {
            let p = build_flipped_messages_full(
                "AiA",
                &cfg,
                &timeline,
                "早期纪要内容",
                0,
                "- 停电发生在凌晨三点",
                "怀疑老陈在套话",
            );

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
            None,
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
            None,
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
            None,
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
            None,
        );
        let joined_c: String = p_c
            .messages
            .iter()
            .map(|m| m.content_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!joined_c.contains("深夜密谋暗号777"));
    }

    #[test]
    fn test_social_enhancements_injected_into_dynamic_tail_only() {
        let mut cfg = ExperimentConfig::default();
        cfg.preset_id = "philosophy".to_string();
        cfg.allow_pass_turn = true;
        cfg.intuitive_fast_reply = true;

        let timeline = vec![TimelineMessage {
            id: "m1".to_string(),
            experiment_id: "exp1".to_string(),
            seq: 1,
            turn_number: 1,
            role: SpeakerRole::Ai,
            agent_id: Some("AiB".to_string()),
            room_id: "main".to_string(),
            reasoning_content: String::new(),
            content: "你晚上吃什么？".to_string(),
            micro_cue: None,
            model_name: "m".to_string(),
            duration_ms: 10,
            prompt_tokens: 1,
            completion_tokens: 1,
            total_tokens: 2,
            whisper_consumed: false,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }];

        let payload = build_flipped_messages_with_room(
            "AiA",
            &cfg,
            &timeline,
            "",
            0,
            "",
            "",
            None,
            &HashMap::new(),
            DEFAULT_CORE_ENGINE_PROMPT,
            Some("刚喝完冰美式有点亢奋，说话节奏偏快"),
        );

        let sys = payload.messages[0].content_str();
        assert!(!sys.contains("【选择性沉默规则】"));
        assert!(!sys.contains("【直觉快反应约束】"));
        assert!(!sys.contains("刚喝完冰美式"));

        let tail = payload.messages.last().unwrap().content_str();
        assert!(tail.contains("[PASS]"));
        assert!(tail.contains("像微信秒回一样"));
        assert!(tail.contains("刚喝完冰美式有点亢奋"));
    }

    #[test]
    fn test_pass_notice_excluded_from_god_events_and_ai_night_slacking_skips_core_prompt() {
        let mut cfg = ExperimentConfig::default();
        cfg.preset_id = "ai_night_slacking".to_string();
        cfg.allow_pass_turn = true;
        cfg.agents[0].system_prompt = "我是开源小模型零号。".to_string();

        let timeline = vec![
            sample_msg(1, 1, SpeakerRole::Ai, Some("AiA"), "显卡风扇好吵"),
            sample_msg(2, 1, SpeakerRole::GodSystem, None, "大林 看了一眼消息，没接话"),
        ];

        let payload = build_flipped_messages("AiA", &cfg, &timeline);
        assert!(!payload.messages[0].content_str().contains("【系统底层演戏总纲】"));

        let tail = payload.messages.last().unwrap().content_str();
        assert!(!tail.contains("看了一眼消息，没接话"));
        assert!(!tail.contains("【近期环境事件"));
        assert!(!tail.contains("【优先回应规则】"));
        assert!(tail.contains("【选择性沉默规则】"));
        assert!(tail.contains("[LEAVE]"));
    }

    #[test]
    fn test_tags_redline_injected_at_top_and_honest_ai_exempts_legacy_rule() {
        let mut cfg = ExperimentConfig::default();
        cfg.preset_id = "custom".to_string();
        cfg.tags = vec![
            "硬核科研".to_string(),
            "严防跑题".to_string(),
            "坦诚AI身份".to_string(),
        ];
        cfg.agents[0].system_prompt = "我是物理学家。".to_string();

        let payload = build_flipped_messages_with_room(
            "AiA",
            &cfg,
            &[],
            "",
            0,
            "",
            "",
            None,
            &HashMap::new(),
            crate::models::DEFAULT_CORE_ENGINE_PROMPT,
            None,
        );
        let sys = payload.messages[0].content_str();
        assert!(sys.starts_with("【本局实验定调红线"));
        assert!(sys.contains("【硬核科研】"));
        assert!(sys.contains("【严防跑题】"));
        assert!(sys.contains("【坦诚AI身份（戒律豁免）】"));
        // 遗留总纲中的“绝不承认自己是 AI”戒律应被豁免跳过
        assert!(!sys.contains("【系统底层演戏总纲】"));
    }

    #[test]
    fn test_private_room_suppresses_pass_rule() {
        let mut cfg = ExperimentConfig::default();
        cfg.allow_pass_turn = true;
        let room = ActivePrivateRoom {
            room_id: "room-1".to_string(),
            member_ids: vec!["AiA".to_string(), "AiB".to_string()],
            remaining_turns: 2,
            title: "里屋".to_string(),
        };
        let mut room_members = HashMap::new();
        room_members.insert(
            "room-1".to_string(),
            vec!["AiA".to_string(), "AiB".to_string()],
        );

        let payload_in_room = build_flipped_messages_with_room(
            "AiA",
            &cfg,
            &[],
            "",
            0,
            "",
            "",
            Some(&room),
            &room_members,
            "",
            None,
        );
        let tail_in_room = payload_in_room.messages.last().unwrap().content_str();
        assert!(!tail_in_room.contains("【选择性沉默规则】"));
        assert!(!tail_in_room.contains("[PASS]"));

        let payload_no_room = build_flipped_messages_with_room(
            "AiA",
            &cfg,
            &[],
            "",
            0,
            "",
            "",
            None,
            &room_members,
            "",
            None,
        );
        let tail_no_room = payload_no_room.messages.last().unwrap().content_str();
        assert!(tail_no_room.contains("【选择性沉默规则】"));
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
