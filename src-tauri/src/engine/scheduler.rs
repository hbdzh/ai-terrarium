use std::collections::HashSet;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::llm::{resolve_chat_completions_url, ApiChatMessage};
use crate::models::{
    is_pass_notice, ActivePrivateRoom, AuxModelConfig, ExperimentConfig, SpeakerRole,
    TimelineMessage,
};
use crate::sanitizer::strip_think_tags;

/// 内置 15 条轻量生活状态微扰动池（随机微情绪骰子使用）
pub const MOOD_DICE_POOL: [&str; 15] = [
    "刚喝完冰美式有点亢奋，说话节奏偏快",
    "有点犯困走神，想换个轻松话题",
    "突然想起一件生活琐事，注意力有点飘",
    "心情不错，想调侃对方两句",
    "刚刷到一条离谱热搜，脑子里还留着槽点",
    "有点饿了，聊着聊着容易联想到吃的",
    "莫名有点小烦躁，不太想听长篇说教",
    "刚伸完懒腰，整个人松弛下来，语气更随性",
    "对刚才某个细节突然有点较真，想追问一句",
    "窗外好像起风了，情绪稍微带点感性",
    "手机一直在震但懒得看，只想顺嘴接两句",
    "突然觉得这个话题有点好笑，忍着笑意接话",
    "脑子稍微有点卡壳，更想用简短大白话回应",
    "刚喝了口热茶，心态平和但有点爱抬杠",
    "忽然涌起一点表达欲，想直接抛出个人直觉判断",
];

pub fn maybe_roll_mood_dice(config: &ExperimentConfig) -> Option<&'static str> {
    if !config.random_mood_dice {
        return None;
    }
    let bytes = *Uuid::new_v4().as_bytes();
    if (bytes[0] % 100) < 30 {
        Some(MOOD_DICE_POOL[(bytes[1] as usize) % MOOD_DICE_POOL.len()])
    } else {
        None
    }
}

/// 去除 `<think>...</think>` 思考块及首尾空白后，判断是否为 `[PASS]` 跳过指令
pub fn is_pass_reply(content: &str) -> bool {
    let mut cleaned = content.to_string();
    loop {
        let lower = cleaned.to_ascii_lowercase();
        let Some(start) = lower.find("<think>") else {
            break;
        };
        if let Some(rel_end) = lower[start + 7..].find("</think>") {
            let end = start + 7 + rel_end + 8;
            cleaned.replace_range(start..end, "");
        } else {
            cleaned.truncate(start);
            break;
        }
    }
    cleaned.trim().eq_ignore_ascii_case("[PASS]")
}

pub const FAREWELL_RESCUE_MARKER: &str = "【叙事导演 · 挽留变故】";

fn is_farewell_wrapup_text(content: &str) -> bool {
    let cleaned = strip_think_tags(content);
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return false;
    }
    let continuation_markers = [
        "等等", "慢着", "先别走", "别急着走", "话还没说完", "等一下", "先等等", "慢点走", "突然",
    ];
    if continuation_markers.iter().any(|k| trimmed.contains(k)) {
        return false;
    }
    let farewell_keywords = [
        "走吧",
        "再见",
        "散了吧",
        "散了",
        "各自回去",
        "关门了",
        "晚安",
        "告辞",
        "没什么可说的了",
        "就这样吧",
        "先走了",
        "回见",
        "拜拜",
        "保重",
        "后会有期",
        "收工",
        "休息吧",
        "回去了",
        "散会",
        "打烊",
        "各自散去",
        "不必再说",
        "到此为止",
        "各自保重",
        "就此别过",
        "各自休息",
        "明早见",
        "明天见",
    ];
    farewell_keywords.iter().any(|k| trimmed.contains(k))
}

fn normalized_chars(text: &str) -> Vec<char> {
    strip_think_tags(text)
        .chars()
        .filter(|c| {
            !c.is_whitespace()
                && !matches!(
                    c,
                    '，' | '。'
                        | '！'
                        | '？'
                        | '、'
                        | '；'
                        | '：'
                        | '“'
                        | '”'
                        | '‘'
                        | '’'
                        | ','
                        | '.'
                        | '!'
                        | '?'
                        | ';'
                        | ':'
                        | '"'
                        | '\''
                        | '（'
                        | '）'
                        | '('
                        | ')'
                        | '~'
                        | '…'
                        | '—'
                        | '-'
                )
        })
        .collect()
}

fn jaccard_bigram_similarity(a: &str, b: &str) -> f64 {
    let ca = normalized_chars(a);
    let cb = normalized_chars(b);
    if ca.len() < 8 || cb.len() < 8 {
        return 0.0;
    }
    let sa: HashSet<(char, char)> = ca.windows(2).map(|w| (w[0], w[1])).collect();
    let sb: HashSet<(char, char)> = cb.windows(2).map(|w| (w[0], w[1])).collect();
    if sa.is_empty() || sb.is_empty() {
        return 0.0;
    }
    let inter = sa.intersection(&sb).count() as f64;
    let union = sa.union(&sb).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

/// 检测最近连续 3 条非沉默 AI 发言是否均属于道别收尾，或连续 3 轮高度语义重复。
pub fn detect_natural_wrapup_loop(messages: &[TimelineMessage]) -> Option<&'static str> {
    let mut recent_ai: Vec<&TimelineMessage> = Vec::with_capacity(3);
    for msg in messages.iter().rev() {
        if is_pass_notice(msg) {
            continue;
        }
        match msg.role {
            SpeakerRole::Ai => {
                recent_ai.push(msg);
                if recent_ai.len() == 3 {
                    break;
                }
            }
            SpeakerRole::GodPublic | SpeakerRole::GodSystem => {
                break;
            }
            SpeakerRole::GodWhisper => continue,
        }
    }
    if recent_ai.len() < 3 {
        return None;
    }
    if recent_ai
        .iter()
        .all(|m| is_farewell_wrapup_text(&m.content))
    {
        return Some("farewell");
    }
    let s01 = jaccard_bigram_similarity(&recent_ai[0].content, &recent_ai[1].content);
    let s12 = jaccard_bigram_similarity(&recent_ai[1].content, &recent_ai[2].content);
    let s02 = jaccard_bigram_similarity(&recent_ai[0].content, &recent_ai[2].content);
    if s01 >= 0.48 && s12 >= 0.48 && s02 >= 0.42 {
        return Some("repetition");
    }
    None
}

/// 3 人及以上智能点名接话路由：检测上一条发言是否明确点名其他角色，且严禁同一角色连续刷屏（自动跳过已退场角色）。
pub fn select_next_speaker(
    config: &ExperimentConfig,
    current_speaker_id: &str,
    last_content: &str,
) -> String {
    if config.agents.is_empty() {
        return current_speaker_id.to_string();
    }
    let current_idx = config
        .agents
        .iter()
        .position(|a| a.id == current_speaker_id)
        .unwrap_or(0);
    let len = config.agents.len();
    let fallback = (1..=len)
        .map(|offset| &config.agents[(current_idx + offset) % len])
        .find(|a| !a.has_left && a.id != current_speaker_id)
        .or_else(|| config.agents.iter().find(|a| !a.has_left))
        .unwrap_or(&config.agents[(current_idx + 1) % len])
        .id
        .clone();

    let active_count = config.agents.iter().filter(|a| !a.has_left).count();
    if !config.enable_smart_selector || active_count < 3 {
        return fallback;
    }

    let cleaned = strip_think_tags(last_content);
    let mut best_match: Option<(usize, usize, usize, String)> = None;
    for (idx, agent) in config.agents.iter().enumerate() {
        if agent.id == current_speaker_id || agent.has_left {
            continue;
        }
        let label = agent.label.trim();
        if label.is_empty() {
            continue;
        }
        if let Some(pos) = cleaned.rfind(label) {
            let rr_dist = (idx + len - current_idx) % len;
            // 优先匹配文末最近一次点名，其次较长角色名，其次顺位最近
            let score = (pos, label.len(), usize::MAX - rr_dist);
            if best_match
                .as_ref()
                .map(|(bp, bl, bd, _)| score > (*bp, *bl, *bd))
                .unwrap_or(true)
            {
                best_match = Some((score.0, score.1, score.2, agent.id.clone()));
            }
        }
    }

    best_match.map(|(_, _, _, id)| id).unwrap_or(fallback)
}

/// 空间隔离最高优先级仲裁：当存在激活的私密房间时，候选池严格锁定在 room_id 成员内部，绝不因提及大厅角色而打破隔离（自动跳过已退场角色）。
pub fn select_next_speaker_in_room(
    config: &ExperimentConfig,
    current_speaker_id: &str,
    last_content: &str,
    active_room: Option<&ActivePrivateRoom>,
) -> String {
    if let Some(room) = active_room {
        if room.remaining_turns > 0 && room.member_ids.len() >= 2 {
            let candidates: Vec<&String> = room
                .member_ids
                .iter()
                .filter(|id| {
                    id.as_str() != current_speaker_id
                        && config.find_agent(id.as_str()).is_some_and(|a| !a.has_left)
                })
                .collect();
            if candidates.is_empty() {
                return room
                    .member_ids
                    .iter()
                    .find(|id| config.find_agent(id.as_str()).is_some_and(|a| !a.has_left))
                    .cloned()
                    .unwrap_or_else(|| room.member_ids[0].clone());
            }
            if config.enable_smart_selector && candidates.len() >= 2 {
                let cleaned = strip_think_tags(last_content);
                let mut best: Option<(usize, String)> = None;
                for cid in &candidates {
                    let label = config.agent_label(Some(cid.as_str()));
                    if let Some(pos) = cleaned.rfind(label.trim()) {
                        if best.as_ref().map(|(bp, _)| pos > *bp).unwrap_or(true) {
                            best = Some((pos, (*cid).clone()));
                        }
                    }
                }
                if let Some((_, id)) = best {
                    return id;
                }
            }
            return candidates[0].clone();
        }
    }
    select_next_speaker(config, current_speaker_id, last_content)
}

#[derive(Serialize)]
struct AuxSelectorReq<'a> {
    model: &'a str,
    messages: &'a [ApiChatMessage],
    temperature: f32,
    stream: bool,
}

#[derive(Deserialize)]
struct AuxSelectorResp {
    choices: Vec<AuxSelectorChoice>,
}

#[derive(Deserialize)]
struct AuxSelectorChoice {
    message: AuxSelectorMsg,
}

#[derive(Deserialize)]
struct AuxSelectorMsg {
    #[serde(default)]
    content: String,
}

/// 若配置了独立的轻量辅助调度模型且未在上一句中检测到显式点名，使用辅助模型判断最自然的接话角色；失败或超时自动回退到规则轮转。
pub async fn select_next_speaker_with_aux(
    client: &Client,
    aux_model: Option<&AuxModelConfig>,
    config: &ExperimentConfig,
    current_speaker_id: &str,
    last_content: &str,
    active_room: Option<&ActivePrivateRoom>,
) -> String {
    let heuristic_pick =
        select_next_speaker_in_room(config, current_speaker_id, last_content, active_room);
    if active_room.is_some() || !config.enable_smart_selector {
        return heuristic_pick;
    }
    let Some(aux) = aux_model.filter(|m| m.is_configured()) else {
        return heuristic_pick;
    };

    let active_candidates: Vec<_> = config
        .agents
        .iter()
        .filter(|a| !a.has_left && a.id != current_speaker_id)
        .collect();
    if active_candidates.len() < 2 {
        return heuristic_pick;
    }

    // 若已存在显式点名，无需耗费额外辅助模型请求
    let cleaned = strip_think_tags(last_content);
    let has_explicit_mention = active_candidates
        .iter()
        .any(|a| !a.label.trim().is_empty() && cleaned.contains(a.label.trim()));
    if has_explicit_mention || cleaned.trim().is_empty() {
        return heuristic_pick;
    }

    let candidate_desc: Vec<String> = active_candidates
        .iter()
        .map(|a| format!("- id: \"{}\" | 角色名: {}", a.id, a.label))
        .collect();
    let current_label = config.agent_label(Some(current_speaker_id));
    let preview: String = cleaned.chars().take(400).collect();
    let msgs = vec![
        ApiChatMessage::text(
            "system",
            "你是多智能体对话的轻量发言调度器。根据上一位角色的发言内容，从候选角色中选择最适合紧接着回应的一位。只输出候选角色的 id 字符串，不要输出任何解释或标点。",
        ),
        ApiChatMessage::text(
            "user",
            format!(
                "【上一位发言者】{}\n【发言内容】\n{}\n\n【候选接话角色】\n{}\n\n默认顺位：{}\n请只输出最适合接话的角色 id：",
                current_label,
                preview,
                candidate_desc.join("\n"),
                heuristic_pick
            ),
        ),
    ];

    let url = resolve_chat_completions_url(&aux.base_url);
    let body = AuxSelectorReq {
        model: aux.model_name.trim(),
        messages: &msgs,
        temperature: 0.1,
        stream: false,
    };

    let Ok(resp) = client
        .post(&url)
        .bearer_auth(aux.api_key.trim())
        .timeout(Duration::from_secs(6))
        .json(&body)
        .send()
        .await
    else {
        return heuristic_pick;
    };
    if !resp.status().is_success() {
        return heuristic_pick;
    }
    let Ok(parsed) = resp.json::<AuxSelectorResp>().await else {
        return heuristic_pick;
    };
    let raw = parsed
        .choices
        .first()
        .map(|c| strip_think_tags(&c.message.content))
        .unwrap_or_default();
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'');
    if let Some(matched) = active_candidates
        .iter()
        .find(|a| a.id == trimmed || a.label == trimmed || trimmed.contains(&a.id))
    {
        return matched.id.clone();
    }
    heuristic_pick
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smart_selector_three_agents_and_no_consecutive_post() {
        let mut cfg = ExperimentConfig::default();
        cfg.enable_smart_selector = true;
        cfg.agents[0].label = "阿哲".to_string();
        cfg.agents[1].label = "老陈".to_string();
        let mut third = cfg.agents[0].clone();
        third.id = "AiC".to_string();
        third.label = "林医生".to_string();
        cfg.agents.push(third);

        assert_eq!(
            select_next_speaker(&cfg, "AiA", "我觉得老陈刚才的说法不对，林医生你怎么看？"),
            "AiC"
        );
        assert_eq!(
            select_next_speaker(&cfg, "AiA", "阿哲我自己其实一直很犹豫……"),
            "AiB"
        );
        assert_eq!(
            select_next_speaker(&cfg, "AiB", "外面的雨越下越大了。"),
            "AiC"
        );
        cfg.enable_smart_selector = false;
        assert_eq!(select_next_speaker(&cfg, "AiA", "林医生你来说"), "AiB");
    }

    #[test]
    fn test_spatial_isolation_smart_selector_locked_inside_private_room() {
        let mut cfg = ExperimentConfig::default();
        cfg.enable_smart_selector = true;
        cfg.agents[0].label = "阿哲".to_string();
        cfg.agents[1].label = "老陈".to_string();
        let mut third = cfg.agents[0].clone();
        third.id = "AiC".to_string();
        third.label = "林医生".to_string();
        cfg.agents.push(third);

        let room = ActivePrivateRoom {
            room_id: "room-secret".to_string(),
            member_ids: vec!["AiA".to_string(), "AiB".to_string()],
            remaining_turns: 2,
            title: "里屋".to_string(),
        };
        assert_eq!(
            select_next_speaker_in_room(
                &cfg,
                "AiA",
                "待会我们出去问问林医生到底知不知道黑箱子的事。",
                Some(&room),
            ),
            "AiB"
        );
    }

    #[test]
    fn test_social_enhancements_pass_mood_and_fast_reply() {
        assert_eq!(MOOD_DICE_POOL.len(), 15);
        assert!(is_pass_reply("[PASS]"));
        assert!(is_pass_reply("  [pass] \n"));
        assert!(is_pass_reply("<think>没啥好说的</think>\n  [PASS] "));
        assert!(!is_pass_reply("<think>思考</think> 我觉得还行 [PASS]"));
    }

    #[test]
    fn test_skip_left_agent_and_detect_farewell_or_repetition() {
        let mut cfg = ExperimentConfig::default();
        cfg.agents[0].has_left = true;
        assert_eq!(select_next_speaker(&cfg, "AiB", "AiA 你怎么看？"), "AiB");

        let mk_ai = |seq: i64, agent: &str, text: &str| TimelineMessage {
            id: format!("m{seq}"),
            experiment_id: "e1".to_string(),
            seq,
            turn_number: seq as u32,
            role: SpeakerRole::Ai,
            agent_id: Some(agent.to_string()),
            room_id: "main".to_string(),
            reasoning_content: String::new(),
            content: text.to_string(),
            micro_cue: None,
            model_name: "m".to_string(),
            duration_ms: 10,
            prompt_tokens: 1,
            completion_tokens: 1,
            total_tokens: 2,
            whisper_consumed: false,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let farewell_msgs = vec![
            mk_ai(1, "AiA", "时候不早了，我先走了，大家晚安。"),
            mk_ai(2, "AiB", "好，店里也关门了，各自回去吧。"),
            mk_ai(3, "AiC", "嗯，没什么可说的了，散了吧，再见。"),
        ];
        assert_eq!(detect_natural_wrapup_loop(&farewell_msgs), Some("farewell"));

        let interrupted_msgs = vec![
            mk_ai(1, "AiA", "时候不早了，我先走了，大家晚安。"),
            mk_ai(2, "AiB", "等等！先别走，门外有脚步声！"),
            mk_ai(3, "AiC", "好，各自回去吧。"),
        ];
        assert_eq!(detect_natural_wrapup_loop(&interrupted_msgs), None);

        let repeat_msgs = vec![
            mk_ai(1, "AiA", "这件事确实很蹊跷，我们必须把监控录像彻底查清楚。"),
            mk_ai(2, "AiB", "没错，这件事确实很蹊跷，必须把监控录像彻底查清楚。"),
            mk_ai(3, "AiC", "对，这件事确实很蹊跷，得把监控录像彻底查清楚才行。"),
        ];
        assert_eq!(detect_natural_wrapup_loop(&repeat_msgs), Some("repetition"));
    }
}
