use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::Utc;
use reqwest::Client;
use tauri::ipc::Channel;
use tokio::sync::{broadcast, oneshot, watch, Mutex, Notify, RwLock};
use uuid::Uuid;

use crate::compactor::{estimate_messages_tokens, maybe_compact_history, CompactionGuard};
use crate::db::Database;
use crate::llm::{
    build_flipped_messages_with_room, stream_chat_completion, ApiChatMessage,
    StreamCompletionResult, StreamDeltaChunk, TokenUsage,
};
use crate::models::{
    is_pass_notice, ActivePrivateRoom, BoardApprovalMode, EngineState, ExperimentConfig,
    InterventionInput, NodeConfig, ObserverNote, SpeakerRole, StreamEvent, TimelineMessage,
    PASS_NOTICE_SUFFIX,
};
use crate::observer::run_observer_analysis;
use crate::sandbox_tools::{
    guardian_review_board, parse_board_tool_call, sandbox_tools_schema, truncate_private_note,
    ApprovalDecision, GuardianVerdict, ParsedBoardTool, MAX_PRIVATE_NOTE_CHARS,
};
use crate::sanitizer::{
    extract_leave_marker, extract_micro_cue, strip_leading_speaker_prefix, strip_think_tags,
};

fn state_to_u8(s: EngineState) -> u8 {
    match s {
        EngineState::Idle => 0,
        EngineState::Running => 1,
        EngineState::Paused => 2,
        EngineState::Stepping => 3,
    }
}

fn u8_to_state(v: u8) -> EngineState {
    match v {
        1 => EngineState::Running,
        2 => EngineState::Paused,
        3 => EngineState::Stepping,
        _ => EngineState::Idle,
    }
}

/// 内置 15 条轻量生活状态微扰动池（随机微情绪骰子使用）
const MOOD_DICE_POOL: [&str; 15] = [
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

fn maybe_roll_mood_dice(config: &ExperimentConfig) -> Option<&'static str> {
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

const FAREWELL_RESCUE_MARKER: &str = "【叙事导演 · 挽留变故】";

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
        .filter(|c| !c.is_whitespace() && !matches!(c, '，' | '。' | '！' | '？' | '、' | '；' | '：' | '“' | '”' | '‘' | '’' | ',' | '.' | '!' | '?' | ';' | ':' | '"' | '\'' | '（' | '）' | '(' | ')' | '~' | '…' | '—' | '-'))
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

pub struct TerrariumEngine {
    pub db: Arc<Database>,
    pub client: Client,
    pub active_experiment_id: Arc<RwLock<String>>,
    pub state: Arc<AtomicU8>,
    pub consecutive_turns: Arc<AtomicU32>,
    pub consecutive_passes: Arc<AtomicU32>,
    pub forced_next_speaker: Arc<Mutex<Option<String>>>,
    pub intervention_queue: Arc<Mutex<VecDeque<(String, InterventionInput)>>>,
    pub pending_approval: Arc<Mutex<Option<(String, oneshot::Sender<ApprovalDecision>)>>>,
    pub compaction_guard: Arc<Mutex<CompactionGuard>>,
    pub channel: Arc<RwLock<Option<Channel<StreamEvent>>>>,
    pub event_tx: broadcast::Sender<StreamEvent>,
    pub blind_guess_mode: Arc<AtomicBool>,
    pub notify: Arc<Notify>,
    pub cancel_tx: watch::Sender<u64>,
    pub loop_started: AtomicBool,
}

impl TerrariumEngine {
    pub fn new(db: Arc<Database>, initial_experiment_id: String) -> Arc<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .build()
            .expect("failed to build reqwest client");
        let (cancel_tx, _) = watch::channel(0u64);
        let (event_tx, _) = broadcast::channel(256);

        let engine = Arc::new(Self {
            db,
            client,
            active_experiment_id: Arc::new(RwLock::new(initial_experiment_id)),
            state: Arc::new(AtomicU8::new(state_to_u8(EngineState::Idle))),
            consecutive_turns: Arc::new(AtomicU32::new(0)),
            consecutive_passes: Arc::new(AtomicU32::new(0)),
            forced_next_speaker: Arc::new(Mutex::new(None)),
            intervention_queue: Arc::new(Mutex::new(VecDeque::new())),
            pending_approval: Arc::new(Mutex::new(None)),
            compaction_guard: Arc::new(Mutex::new(CompactionGuard::default())),
            channel: Arc::new(RwLock::new(None)),
            event_tx,
            blind_guess_mode: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
            cancel_tx,
            loop_started: AtomicBool::new(false),
        });

        engine.ensure_loop_spawned();
        engine
    }

    pub fn ensure_loop_spawned(self: &Arc<Self>) {
        if self
            .loop_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            let engine = Arc::clone(self);
            tauri::async_runtime::spawn(async move {
                engine.run_coordinator_loop().await;
            });
        }
    }

    pub fn current_state(&self) -> EngineState {
        u8_to_state(self.state.load(Ordering::SeqCst))
    }

    pub async fn set_channel(&self, ch: Channel<StreamEvent>) {
        let mut guard = self.channel.write().await;
        *guard = Some(ch);
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<StreamEvent> {
        self.event_tx.subscribe()
    }

    pub fn set_blind_guess_mode(&self, enabled: bool) {
        self.blind_guess_mode.store(enabled, Ordering::SeqCst);
    }

    pub fn is_blind_guess_mode(&self) -> bool {
        self.blind_guess_mode.load(Ordering::SeqCst)
    }

    async fn emit(&self, event: StreamEvent) {
        if !matches!(
            event,
            StreamEvent::StreamDelta { .. }
                | StreamEvent::StreamReasoningDelta { .. }
                | StreamEvent::StreamStart { .. }
        ) {
            let _ = self.event_tx.send(event.clone());
        }
        let guard = self.channel.read().await;
        if let Some(ch) = guard.as_ref() {
            let _ = ch.send(event);
        }
    }

    pub async fn emit_event(&self, event: StreamEvent) {
        self.emit(event).await;
    }

    pub async fn resolve_board_approval(
        &self,
        request_id: &str,
        decision: ApprovalDecision,
    ) -> bool {
        let mut guard = self.pending_approval.lock().await;
        if let Some((pending_id, _)) = guard.as_ref() {
            if pending_id == request_id {
                if let Some((_, tx)) = guard.take() {
                    let _ = tx.send(decision);
                    return true;
                }
            }
        }
        false
    }

    fn bump_cancel(&self) {
        let next = self.cancel_tx.borrow().wrapping_add(1);
        let _ = self.cancel_tx.send(next);
    }

    pub async fn start(&self) {
        *self.forced_next_speaker.lock().await = None;
        self.consecutive_turns.store(0, Ordering::SeqCst);
        self.consecutive_passes.store(0, Ordering::SeqCst);
        self.compaction_guard.lock().await.reset();
        self.state
            .store(state_to_u8(EngineState::Running), Ordering::SeqCst);
        let exp_id = self.active_experiment_id.read().await.clone();
        self.emit(StreamEvent::StateChanged {
            experiment_id: exp_id,
            state: EngineState::Running,
            reason: None,
            consecutive_turns: 0,
        })
        .await;
        self.notify.notify_one();
    }

    pub async fn pause(&self, reason: Option<String>) {
        let prev = self.current_state();
        self.state
            .store(state_to_u8(EngineState::Paused), Ordering::SeqCst);
        *self.forced_next_speaker.lock().await = None;
        if prev == EngineState::Running || prev == EngineState::Stepping {
            self.bump_cancel();
        }
        {
            let mut guard = self.pending_approval.lock().await;
            *guard = None;
        }
        let exp_id = self.active_experiment_id.read().await.clone();
        self.emit(StreamEvent::StateChanged {
            experiment_id: exp_id,
            state: EngineState::Paused,
            reason,
            consecutive_turns: self.consecutive_turns.load(Ordering::SeqCst),
        })
        .await;
        self.notify.notify_one();
    }

    pub async fn step_once(&self, target_agent_id: Option<String>) {
        let cur = self.current_state();
        if cur == EngineState::Running || cur == EngineState::Stepping {
            return;
        }
        *self.forced_next_speaker.lock().await = target_agent_id;
        self.state
            .store(state_to_u8(EngineState::Stepping), Ordering::SeqCst);
        let exp_id = self.active_experiment_id.read().await.clone();
        self.emit(StreamEvent::StateChanged {
            experiment_id: exp_id,
            state: EngineState::Stepping,
            reason: None,
            consecutive_turns: self.consecutive_turns.load(Ordering::SeqCst),
        })
        .await;
        self.notify.notify_one();
    }

    pub async fn switch_experiment(&self, experiment_id: String) {
        self.pause(None).await;
        {
            let mut guard = self.pending_approval.lock().await;
            *guard = None;
        }
        {
            let mut q = self.intervention_queue.lock().await;
            q.clear();
        }
        self.compaction_guard.lock().await.reset();
        {
            let mut active = self.active_experiment_id.write().await;
            *active = experiment_id.clone();
        }
        self.consecutive_turns.store(0, Ordering::SeqCst);
        self.consecutive_passes.store(0, Ordering::SeqCst);
        self.state
            .store(state_to_u8(EngineState::Idle), Ordering::SeqCst);
        self.emit(StreamEvent::StateChanged {
            experiment_id,
            state: EngineState::Idle,
            reason: None,
            consecutive_turns: 0,
        })
        .await;
    }

    pub async fn enqueue_intervention(
        &self,
        experiment_id: String,
        input: InterventionInput,
    ) -> usize {
        let len = {
            let mut q = self.intervention_queue.lock().await;
            q.push_back((experiment_id.clone(), input));
            q.len()
        };
        let cur = self.current_state();
        if cur == EngineState::Idle || cur == EngineState::Paused {
            self.drain_interventions(&experiment_id).await;
            return 0;
        }
        self.emit(StreamEvent::InterventionQueued {
            experiment_id,
            queue_len: len,
        })
        .await;
        len
    }

    async fn drain_interventions(&self, exp_id: &str) {
        let items: Vec<InterventionInput> = {
            let mut q = self.intervention_queue.lock().await;
            let mut matched = Vec::new();
            let mut remaining = VecDeque::new();
            while let Some((id, item)) = q.pop_front() {
                if id == exp_id {
                    matched.push(item);
                } else {
                    remaining.push_back((id, item));
                }
            }
            *q = remaining;
            matched
        };

        if items.is_empty() {
            return;
        }

        let current_turn = self.db.get_total_turns(exp_id).unwrap_or(0);

        for item in items {
            let content = item.content.trim().to_string();
            if content.is_empty() {
                continue;
            }
            let seq = self.db.next_seq(exp_id).unwrap_or(1);
            let msg = TimelineMessage {
                id: Uuid::new_v4().to_string(),
                experiment_id: exp_id.to_string(),
                seq,
                turn_number: current_turn,
                role: item.kind,
                agent_id: item.agent_id,
                room_id: "main".to_string(),
                reasoning_content: String::new(),
                content,
                micro_cue: None,
                model_name: String::new(),
                duration_ms: 0,
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                whisper_consumed: false,
                created_at: Utc::now().to_rfc3339(),
            };
            if self.db.append_message(&msg).is_ok() {
                self.emit(StreamEvent::MessageAdded {
                    experiment_id: exp_id.to_string(),
                    message: msg,
                })
                .await;
            }
        }

        self.emit(StreamEvent::InterventionQueued {
            experiment_id: exp_id.to_string(),
            queue_len: 0,
        })
        .await;
    }

    pub async fn start_private_room(
        &self,
        exp_id: &str,
        member_ids: Vec<String>,
        turns: Option<u32>,
    ) -> Result<ActivePrivateRoom> {
        let mut detail = self.db.get_experiment_detail(exp_id)?;
        if detail.config.agents.len() < 3 {
            return Err(anyhow!("临时私密房间需要至少 3 位 AI 参与者"));
        }
        if member_ids.len() != 2 || member_ids[0] == member_ids[1] {
            return Err(anyhow!("请指定 2 位不同的角色进入临时私密房间"));
        }
        for mid in &member_ids {
            if detail.config.find_agent(mid).is_none() {
                return Err(anyhow!("未找到角色 ID: {mid}"));
            }
        }

        let duration_turns = turns.unwrap_or(2).clamp(1, 5);
        let label1 = detail.config.agent_label(Some(&member_ids[0]));
        let label2 = detail.config.agent_label(Some(&member_ids[1]));
        let room = ActivePrivateRoom {
            room_id: format!("room-{}", &Uuid::new_v4().simple().to_string()[..8]),
            member_ids: member_ids.clone(),
            remaining_turns: duration_turns,
            title: format!("{} × {} 的里屋", label1, label2),
        };
        detail
            .room_members
            .insert(room.room_id.clone(), room.member_ids.clone());
        self.db
            .update_active_room(exp_id, &Some(room.clone()), &detail.room_members)?;
        let _ = self.db.update_experiment_progress(
            exp_id,
            detail.total_turns,
            detail.total_tokens,
            &member_ids[0],
        );

        let seq = self.db.next_seq(exp_id).unwrap_or(1);
        let notice_msg = TimelineMessage {
            id: Uuid::new_v4().to_string(),
            experiment_id: exp_id.to_string(),
            seq,
            turn_number: detail.total_turns,
            role: SpeakerRole::GodSystem,
            agent_id: None,
            room_id: "main".to_string(),
            reasoning_content: String::new(),
            content: format!(
                "【空间变动】{} 与 {} 低声走入里屋私聊（大厅其余角色无法听见里屋对话）。",
                label1, label2
            ),
            micro_cue: None,
            model_name: String::new(),
            duration_ms: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            whisper_consumed: false,
            created_at: Utc::now().to_rfc3339(),
        };
        if self.db.append_message(&notice_msg).is_ok() {
            self.emit(StreamEvent::MessageAdded {
                experiment_id: exp_id.to_string(),
                message: notice_msg,
            })
            .await;
        }
        self.emit(StreamEvent::PrivateRoomUpdated {
            experiment_id: exp_id.to_string(),
            active_room: Some(room.clone()),
            room_members: detail.room_members,
        })
        .await;
        Ok(room)
    }

    pub async fn close_private_room(&self, exp_id: &str) -> Result<()> {
        let detail = self.db.get_experiment_detail(exp_id)?;
        let Some(prev_room) = detail.active_private_room else {
            return Ok(());
        };
        self.db
            .update_active_room(exp_id, &None, &detail.room_members)?;
        let label1 = prev_room
            .member_ids
            .first()
            .map(|id| detail.config.agent_label(Some(id)))
            .unwrap_or_default();
        let label2 = prev_room
            .member_ids
            .get(1)
            .map(|id| detail.config.agent_label(Some(id)))
            .unwrap_or_default();

        let seq = self.db.next_seq(exp_id).unwrap_or(1);
        let notice_msg = TimelineMessage {
            id: Uuid::new_v4().to_string(),
            experiment_id: exp_id.to_string(),
            seq,
            turn_number: detail.total_turns,
            role: SpeakerRole::GodSystem,
            agent_id: None,
            room_id: "main".to_string(),
            reasoning_content: String::new(),
            content: format!(
                "【空间变动】{} 与 {} 结束了私聊，各自回到大厅。",
                label1, label2
            ),
            micro_cue: None,
            model_name: String::new(),
            duration_ms: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            whisper_consumed: false,
            created_at: Utc::now().to_rfc3339(),
        };
        if self.db.append_message(&notice_msg).is_ok() {
            self.emit(StreamEvent::MessageAdded {
                experiment_id: exp_id.to_string(),
                message: notice_msg,
            })
            .await;
        }
        self.emit(StreamEvent::PrivateRoomUpdated {
            experiment_id: exp_id.to_string(),
            active_room: None,
            room_members: detail.room_members,
        })
        .await;
        Ok(())
    }

    pub async fn trigger_observer_now(&self, exp_id: String) {
        let detail = match self.db.get_experiment_detail(&exp_id) {
            Ok(d) => d,
            Err(e) => {
                self.emit(StreamEvent::ObserverError {
                    experiment_id: exp_id,
                    error: e.to_string(),
                })
                .await;
                return;
            }
        };
        if detail.messages.is_empty() {
            return;
        }
        let tail_start = detail.messages.len().saturating_sub(24);
        let recent_msgs = detail.messages[tail_start..].to_vec();
        self.spawn_observer_task(
            exp_id,
            detail.total_turns,
            detail.config,
            recent_msgs,
            detail.board_content,
            detail.compacted_summary,
        )
        .await;
    }

    async fn spawn_observer_task(
        &self,
        exp_id: String,
        trigger_turn: u32,
        config: ExperimentConfig,
        messages: Vec<TimelineMessage>,
        board_content: String,
        compacted_summary: String,
    ) {
        if config.node_c.api_key.trim().is_empty() || config.node_c.model_name.trim().is_empty() {
            return;
        }

        self.emit(StreamEvent::ObserverStarted {
            experiment_id: exp_id.clone(),
            trigger_turn,
        })
        .await;

        let db = Arc::clone(&self.db);
        let client = self.client.clone();
        let channel = Arc::clone(&self.channel);
        let queue = Arc::clone(&self.intervention_queue);
        let state = Arc::clone(&self.state);

        tauri::async_runtime::spawn(async move {
            let slice_start = messages.len().saturating_sub(24);
            let recent = &messages[slice_start..];
            match run_observer_analysis(
                &client,
                &config,
                recent,
                &board_content,
                &compacted_summary,
            )
            .await
            {
                Ok(parsed) => {
                    let stagnation = parsed.stagnation_index.clamp(0, 100) as u8;
                    let director_event_content = if config.enable_auto_director {
                        parsed
                            .director_event
                            .clone()
                            .filter(|s| !s.trim().is_empty())
                            .or_else(|| {
                                if stagnation > 80 {
                                    parsed
                                        .crisis_cards
                                        .iter()
                                        .find(|c| c.kind == "GodSystem" && !c.content.trim().is_empty())
                                        .or_else(|| {
                                            parsed
                                                .crisis_cards
                                                .iter()
                                                .find(|c| !c.content.trim().is_empty())
                                        })
                                        .map(|c| c.content.trim().to_string())
                                } else {
                                    None
                                }
                            })
                    } else {
                        None
                    };
                    let should_perturb = if config.enable_auto_director {
                        director_event_content.is_some()
                    } else {
                        stagnation > 80
                    };
                    let note = ObserverNote {
                        id: Uuid::new_v4().to_string(),
                        experiment_id: exp_id.clone(),
                        trigger_turn,
                        tags: parsed.tags,
                        depth_stage: parsed.depth_stage,
                        narrative_act: parsed.narrative_act,
                        note: parsed.note,
                        tension_score: parsed.tension_score.clamp(0, 100) as u8,
                        abstraction_score: parsed.abstraction_score.clamp(0, 100) as u8,
                        stagnation_index: stagnation,
                        perturbation_injected: should_perturb,
                        crisis_cards: parsed.crisis_cards,
                        relationship_matrix: parsed
                            .relationship_matrix
                            .into_iter()
                            .map(|mut edge| {
                                if let Some(ag) = config.find_agent(&edge.from) {
                                    edge.from = ag.label.clone();
                                }
                                if let Some(ag) = config.find_agent(&edge.to) {
                                    edge.to = ag.label.clone();
                                }
                                edge
                            })
                            .collect(),
                        created_at: Utc::now().to_rfc3339(),
                    };

                    if db.append_observer_note(&note).is_ok() {
                        let guard = channel.read().await;
                        if let Some(ch) = guard.as_ref() {
                            let _ = ch.send(StreamEvent::ObserverCompleted {
                                experiment_id: exp_id.clone(),
                                note,
                            });
                        }
                    }

                    if !parsed.achieved_agent_ids.is_empty() {
                        if let Ok(mut latest_detail) = db.get_experiment_detail(&exp_id) {
                            let mut newly_achieved: Vec<(String, String, String)> = Vec::new();
                            for ag in &mut latest_detail.config.agents {
                                if ag.secret_goal.trim().is_empty() || ag.goal_achieved {
                                    continue;
                                }
                                let matched = parsed.achieved_agent_ids.iter().any(|aid| {
                                    let t = aid.trim();
                                    t == ag.id || t == ag.label
                                });
                                if matched {
                                    ag.goal_achieved = true;
                                    newly_achieved.push((
                                        ag.id.clone(),
                                        ag.label.clone(),
                                        ag.secret_goal.clone(),
                                    ));
                                }
                            }
                            if !newly_achieved.is_empty()
                                && db
                                    .update_experiment(&exp_id, &latest_detail.title, &latest_detail.config)
                                    .is_ok()
                            {
                                let guard = channel.read().await;
                                if let Some(ch) = guard.as_ref() {
                                    let _ = ch.send(StreamEvent::ConfigHotUpdated {
                                        experiment_id: exp_id.clone(),
                                        config: latest_detail.config.clone(),
                                    });
                                    for (agent_id, agent_label, secret_goal) in newly_achieved {
                                        let _ = ch.send(StreamEvent::SecretGoalAchieved {
                                            experiment_id: exp_id.clone(),
                                            agent_id,
                                            agent_label,
                                            secret_goal,
                                        });
                                    }
                                }
                            }
                        }
                    }

                    if should_perturb {
                        let perturbation_text = if let Some(ev_text) = director_event_content {
                            format!("【叙事导演 · 突发环境事件】{}", ev_text.trim())
                        } else {
                            "【生态自动扰动 · 打破僵化】观测到当前对话已陷入高度复读或客套循环。请立即推翻刚才的隐含共识，引入一个截然相反的极端反例或具象悖论，继续交锋。".to_string()
                        };
                        let perturbation = InterventionInput {
                            kind: SpeakerRole::GodSystem,
                            agent_id: None,
                            content: perturbation_text,
                        };
                        let cur_state = u8_to_state(state.load(Ordering::SeqCst));
                        if cur_state == EngineState::Running || cur_state == EngineState::Stepping {
                            let len = {
                                let mut q = queue.lock().await;
                                q.push_back((exp_id.clone(), perturbation));
                                q.len()
                            };
                            let guard = channel.read().await;
                            if let Some(ch) = guard.as_ref() {
                                let _ = ch.send(StreamEvent::InterventionQueued {
                                    experiment_id: exp_id.clone(),
                                    queue_len: len,
                                });
                            }
                        } else {
                            let seq = db.next_seq(&exp_id).unwrap_or(1);
                            let msg = TimelineMessage {
                                id: Uuid::new_v4().to_string(),
                                experiment_id: exp_id.clone(),
                                seq,
                                turn_number: trigger_turn,
                                role: perturbation.kind,
                                agent_id: None,
                                room_id: "main".to_string(),
                                reasoning_content: String::new(),
                                content: perturbation.content,
                                micro_cue: None,
                                model_name: String::new(),
                                duration_ms: 0,
                                prompt_tokens: 0,
                                completion_tokens: 0,
                                total_tokens: 0,
                                whisper_consumed: false,
                                created_at: Utc::now().to_rfc3339(),
                            };
                            if db.append_message(&msg).is_ok() {
                                let guard = channel.read().await;
                                if let Some(ch) = guard.as_ref() {
                                    let _ = ch.send(StreamEvent::MessageAdded {
                                        experiment_id: exp_id.clone(),
                                        message: msg,
                                    });
                                }
                            }
                        }
                    }
                }
                Err(_e) => {
                    // 铁律 4：解析或观察员阶段失败时静默跳过本轮分析，绝不中断主对话流
                    let guard = channel.read().await;
                    if let Some(ch) = guard.as_ref() {
                        let _ = ch.send(StreamEvent::ObserverSkipped {
                            experiment_id: exp_id,
                            trigger_turn,
                        });
                    }
                }
            }
        });
    }

    async fn run_coordinator_loop(self: Arc<Self>) {
        loop {
            let cur = self.current_state();
            if cur != EngineState::Running && cur != EngineState::Stepping {
                self.notify.notified().await;
                continue;
            }

            let exp_id = self.active_experiment_id.read().await.clone();

            // 1. Drain Creator intervention queue before starting the turn
            self.drain_interventions(&exp_id).await;

            // Reload experiment detail (windowed for O(window) scalability on long runs)
            let mut detail = match self.db.get_experiment_detail_windowed(&exp_id) {
                Ok(d) => d,
                Err(e) => {
                    self.emit(StreamEvent::EngineError {
                        experiment_id: exp_id,
                        message: format!("读取实验数据失败: {e}"),
                    })
                    .await;
                    self.pause(Some("实验加载失败".to_string())).await;
                    continue;
                }
            };

            if detail.config.agents.is_empty() {
                self.pause(Some("请至少配置 1 个角色".to_string()))
                    .await;
                continue;
            }

            let active_agent_count = detail.config.agents.iter().filter(|a| !a.has_left).count();
            if (detail.config.agents.len() >= 2 && active_agent_count < 2)
                || active_agent_count == 0
            {
                let msg = "🎬 角色已悉数离场，本场推演自然落幕".to_string();
                let already_ended = detail
                    .messages
                    .last()
                    .is_some_and(|m| m.role == SpeakerRole::GodSystem && m.content == msg);
                if !already_ended {
                    let seq = self.db.next_seq(&exp_id).unwrap_or(1);
                    let end_msg = TimelineMessage {
                        id: Uuid::new_v4().to_string(),
                        experiment_id: exp_id.clone(),
                        seq,
                        turn_number: detail.total_turns,
                        role: SpeakerRole::GodSystem,
                        agent_id: None,
                        room_id: "main".to_string(),
                        reasoning_content: String::new(),
                        content: msg.clone(),
                        micro_cue: None,
                        model_name: String::new(),
                        duration_ms: 0,
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        total_tokens: 0,
                        whisper_consumed: false,
                        created_at: Utc::now().to_rfc3339(),
                    };
                    if self.db.append_message(&end_msg).is_ok() {
                        self.emit(StreamEvent::MessageAdded {
                            experiment_id: exp_id.clone(),
                            message: end_msg,
                        })
                        .await;
                    }
                }
                self.pause(Some(msg)).await;
                continue;
            }

            // 2. Check max consecutive turns, token budget & narrative arc finale
            if detail.config.enable_narrative_arc
                && detail.config.target_total_turns > 0
                && detail.total_turns >= detail.config.target_total_turns
            {
                self.emit(StreamEvent::NarrativeFinaleReached {
                    experiment_id: exp_id.clone(),
                    total_turns: detail.total_turns,
                    target_turns: detail.config.target_total_turns,
                })
                .await;
                self.pause(Some(format!(
                    "已达到目标总轮数 {} 轮，本局已收尾，可生成《复盘报告》。",
                    detail.config.target_total_turns
                )))
                .await;
                continue;
            }

            if cur == EngineState::Running
                && detail.config.max_consecutive_turns > 0
                && self.consecutive_turns.load(Ordering::SeqCst)
                    >= detail.config.max_consecutive_turns
            {
                self.pause(Some(format!(
                    "连续运行已达 {} 轮上限，已自动暂停",
                    detail.config.max_consecutive_turns
                )))
                .await;
                continue;
            }

            if detail.config.token_budget > 0 && detail.total_tokens >= detail.config.token_budget {
                self.pause(Some(format!(
                    "已达到单局 Token 上限 ({} / {})，自动暂停",
                    detail.total_tokens, detail.config.token_budget
                )))
                .await;
                continue;
            }

            let next_turn = detail.total_turns + 1;

            // 2.5 Narrative Arc 80% countdown injection
            if detail.config.enable_narrative_arc && detail.config.target_total_turns >= 5 {
                let countdown_turn =
                    ((detail.config.target_total_turns as f64) * 0.8).ceil() as u32;
                let already_countdown = detail.messages.iter().any(|m| {
                    m.role == SpeakerRole::GodSystem
                        && m.content.contains("【导演节拍器 · 终幕倒计时】")
                });
                if next_turn >= countdown_turn && !already_countdown {
                    let seq = self.db.next_seq(&exp_id).unwrap_or(1);
                    let cd_msg = TimelineMessage {
                        id: Uuid::new_v4().to_string(),
                        experiment_id: exp_id.clone(),
                        seq,
                        turn_number: detail.total_turns,
                        role: SpeakerRole::GodSystem,
                        agent_id: None,
                        room_id: "main".to_string(),
                        reasoning_content: String::new(),
                        content: format!(
                            "【导演节拍器 · 终幕倒计时】当前已进入全剧最后 20% 收束阶段（第 {}/{} 轮），请所有角色逐步收束既有伏笔、直面核心矛盾并推动冲突走向终局。",
                            next_turn, detail.config.target_total_turns
                        ),
                        micro_cue: None,
                        model_name: String::new(),
                        duration_ms: 0,
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        total_tokens: 0,
                        whisper_consumed: false,
                        created_at: Utc::now().to_rfc3339(),
                    };
                    if self.db.append_message(&cd_msg).is_ok() {
                        detail.messages.push(cd_msg.clone());
                        self.emit(StreamEvent::MessageAdded {
                            experiment_id: exp_id.clone(),
                            message: cd_msg,
                        })
                        .await;
                    }
                }
            }

            // 3. Speaker selection with Spatial Isolation highest priority
            let active_room = detail
                .active_private_room
                .clone()
                .filter(|r| r.remaining_turns > 0 && r.member_ids.len() >= 2);
            let mut desired_speaker_id = detail.next_speaker.clone();
            if let Some(ref room) = active_room {
                if !room.member_ids.iter().any(|id| id == &desired_speaker_id) {
                    desired_speaker_id = room.member_ids[0].clone();
                }
                let last_room_ai = detail
                    .messages
                    .iter()
                    .rev()
                    .find(|m| m.room_id == room.room_id && m.role == SpeakerRole::Ai)
                    .and_then(|m| m.agent_id.as_deref());
                if last_room_ai == Some(desired_speaker_id.as_str()) {
                    if let Some(other) = room
                        .member_ids
                        .iter()
                        .find(|id| id.as_str() != desired_speaker_id)
                    {
                        desired_speaker_id = other.clone();
                    }
                }
            } else if detail.config.enable_smart_selector && detail.config.agents.len() >= 3 {
                if let Some(last_conv) = detail.messages.iter().rev().find(|m| {
                    m.room_id == "main"
                        && (matches!(m.role, SpeakerRole::Ai | SpeakerRole::GodPublic)
                            || is_pass_notice(m))
                }) {
                    if last_conv.role == SpeakerRole::GodPublic {
                        let last_ai_id = detail
                            .messages
                            .iter()
                            .rev()
                            .find(|m| m.room_id == "main" && m.role == SpeakerRole::Ai)
                            .and_then(|m| m.agent_id.as_deref())
                            .unwrap_or("");
                        desired_speaker_id =
                            select_next_speaker(&detail.config, last_ai_id, &last_conv.content);
                    }
                }
            }
            if let Some(forced_id) = self.forced_next_speaker.lock().await.take() {
                if detail
                    .config
                    .find_agent(&forced_id)
                    .is_some_and(|a| !a.has_left)
                {
                    desired_speaker_id = forced_id;
                }
            }
            if detail
                .config
                .find_agent(&desired_speaker_id)
                .is_none_or(|a| a.has_left)
            {
                if let Some(first_active) = detail.config.agents.iter().find(|a| !a.has_left) {
                    desired_speaker_id = first_active.id.clone();
                }
            }

            // 3.1 检查定时线索（TimedClues）是否到达触发轮次
            let upcoming_turn = detail.total_turns + 1;
            let mut due_clue_indices: Vec<usize> = Vec::new();
            for (idx, clue) in detail.config.timed_clues.iter().enumerate() {
                if !clue.triggered
                    && upcoming_turn >= clue.trigger_turn
                    && !clue.content.trim().is_empty()
                {
                    due_clue_indices.push(idx);
                }
            }
            if !due_clue_indices.is_empty() {
                for idx in due_clue_indices {
                    detail.config.timed_clues[idx].triggered = true;
                    let clue_text = detail.config.timed_clues[idx].content.trim().to_string();
                    let clue_seq = self.db.next_seq(&exp_id).unwrap_or(1);
                    let clue_msg = TimelineMessage {
                        id: Uuid::new_v4().to_string(),
                        experiment_id: exp_id.clone(),
                        seq: clue_seq,
                        turn_number: upcoming_turn,
                        role: SpeakerRole::GodSystem,
                        agent_id: None,
                        room_id: "main".to_string(),
                        reasoning_content: String::new(),
                        content: format!("【定时线索 · 现场新发现】{}", clue_text),
                        micro_cue: None,
                        model_name: String::new(),
                        duration_ms: 0,
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        total_tokens: 0,
                        whisper_consumed: false,
                        created_at: Utc::now().to_rfc3339(),
                    };
                    if self.db.append_message(&clue_msg).is_ok() {
                        detail.messages.push(clue_msg.clone());
                        self.emit(StreamEvent::MessageAdded {
                            experiment_id: exp_id.clone(),
                            message: clue_msg,
                        })
                        .await;
                    }
                }
                let _ = self
                    .db
                    .update_experiment(&exp_id, &detail.title, &detail.config);
                self.emit(StreamEvent::ConfigHotUpdated {
                    experiment_id: exp_id.clone(),
                    config: detail.config.clone(),
                })
                .await;
            }

            let speaker_idx = detail
                .config
                .agents
                .iter()
                .position(|a| a.id == desired_speaker_id)
                .unwrap_or(0);
            let speaker_id = detail.config.agents[speaker_idx].id.clone();
            let agent_private_note = detail
                .private_notes
                .get(&speaker_id)
                .map(|s| s.as_str())
                .unwrap_or("");

            let core_engine_prompt = self
                .db
                .get_core_engine_prompt()
                .unwrap_or_else(|_| crate::models::DEFAULT_CORE_ENGINE_PROMPT.to_string());
            let mut compacted_summary = detail.compacted_summary.clone();
            let mut compacted_until_seq = detail.compacted_until_seq;
            let rolled_mood = maybe_roll_mood_dice(&detail.config);
            let mut payload = build_flipped_messages_with_room(
                &speaker_id,
                &detail.config,
                &detail.messages,
                &compacted_summary,
                compacted_until_seq,
                &detail.board_content,
                agent_private_note,
                active_room.as_ref(),
                &detail.room_members,
                &core_engine_prompt,
                rolled_mood,
            );

            // 3.5 Auto context compaction if estimated prompt tokens exceed threshold (e.g. 80%)
            let mut est_prompt_tokens = estimate_messages_tokens(&payload.messages);
            let ctx_limit = detail.config.context_window_tokens.max(2048);
            let mut compact_cancel_rx = self.cancel_tx.subscribe();
            let compact_init_gen = *compact_cancel_rx.borrow();
            let compact_res = tokio::select! {
                res = maybe_compact_history(
                    &self.client,
                    &detail.config,
                    &detail.messages,
                    &compacted_summary,
                    compacted_until_seq,
                    est_prompt_tokens,
                ) => Some(res),
                _ = compact_cancel_rx.changed() => {
                    if *compact_cancel_rx.borrow() != compact_init_gen {
                        None
                    } else {
                        Some(Ok(None))
                    }
                }
            };

            let Some(compact_res) = compact_res else {
                continue;
            };

            if let Ok(Some(comp)) = compact_res {
                compacted_summary = comp.new_summary.clone();
                compacted_until_seq = comp.new_until_seq;
                let _ = self.db.update_compacted_memory(
                    &exp_id,
                    &compacted_summary,
                    compacted_until_seq,
                );
                if !comp.private_state_snapshots.is_empty() {
                    let mut snapshot_changed = false;
                    for ag in &mut detail.config.agents {
                        if let Some(snap) = comp.private_state_snapshots.get(&ag.id) {
                            if ag.private_state_snapshot != *snap {
                                ag.private_state_snapshot = snap.clone();
                                snapshot_changed = true;
                            }
                        }
                    }
                    if snapshot_changed {
                        let _ = self
                            .db
                            .update_experiment(&exp_id, &detail.title, &detail.config);
                    }
                }
                payload = build_flipped_messages_with_room(
                    &speaker_id,
                    &detail.config,
                    &detail.messages,
                    &compacted_summary,
                    compacted_until_seq,
                    &detail.board_content,
                    agent_private_note,
                    active_room.as_ref(),
                    &detail.room_members,
                    &core_engine_prompt,
                    rolled_mood,
                );
                est_prompt_tokens = estimate_messages_tokens(&payload.messages);
                let after_pct = ((est_prompt_tokens * 100) / ctx_limit).min(999) as u16;
                self.emit(StreamEvent::MemoryCompacted {
                    experiment_id: exp_id.clone(),
                    compacted_until_seq,
                    compacted_summary: compacted_summary.clone(),
                    tokens_before: comp.tokens_before,
                    tokens_after: comp.tokens_after,
                    context_usage_pct: after_pct,
                })
                .await;

                if detail.config.enable_compaction_guard {
                    let tripped = self
                        .compaction_guard
                        .lock()
                        .await
                        .record_and_check(next_turn);
                    if tripped {
                        let warn_msg = "防循环自动暂停：连续 3 轮触发 3 次记忆压缩，已自动暂停。请前往「设置 → 外观与调度」调大上下文窗口。".to_string();
                        self.emit(StreamEvent::EngineError {
                            experiment_id: exp_id.clone(),
                            message: warn_msg.clone(),
                        })
                        .await;
                        self.pause(Some(warn_msg)).await;
                        continue;
                    }
                }
            }

            let node_cfg = &detail.config.agents[speaker_idx];

            let msg_id = Uuid::new_v4().to_string();
            let context_usage_pct = ((est_prompt_tokens * 100) / ctx_limit).min(999) as u16;
            let enable_board_tools = detail.config.enable_board_tools;
            let enable_private_notes = detail.config.enable_private_notes;
            let tools_schema = sandbox_tools_schema(enable_board_tools, enable_private_notes);

            let thinking_params = crate::llm::resolve_thinking_params(&detail.config, node_cfg);

            let raw_prompt_json = serde_json::json!({
                "model": node_cfg.model_name,
                "temperature": node_cfg.temperature,
                "reasoning_effort": &thinking_params.reasoning_effort,
                "enable_thinking": thinking_params.enable_thinking,
                "thinking_budget": thinking_params.thinking_budget,
                "thinking": &thinking_params.thinking,
                "private_note": if enable_private_notes { Some(agent_private_note) } else { None },
                "messages": &payload.messages,
                "tools": if !tools_schema.is_empty() { Some(&tools_schema) } else { None },
            })
            .to_string();

            self.emit(StreamEvent::StreamStart {
                experiment_id: exp_id.clone(),
                message_id: msg_id.clone(),
                role: SpeakerRole::Ai,
                agent_id: node_cfg.id.clone(),
                agent_label: node_cfg.label.clone(),
                turn_number: next_turn,
                model_name: node_cfg.model_name.clone(),
                active_tokens: est_prompt_tokens,
                context_usage_pct,
            })
            .await;

            let stream_res = self
                .execute_turn_with_tools(
                    &exp_id,
                    &msg_id,
                    node_cfg,
                    payload.messages,
                    enable_board_tools,
                    enable_private_notes,
                    &thinking_params,
                )
                .await;

            match stream_res {
                Ok(res) => {
                    let _ = self
                        .db
                        .mark_whispers_consumed(&payload.consumed_whisper_ids);

                    if active_room.is_none()
                        && detail.config.allow_pass_turn
                        && is_pass_reply(&res.content)
                    {
                        let consec_passes =
                            self.consecutive_passes.fetch_add(1, Ordering::SeqCst) + 1;
                        let active_in_scope = if let Some(ref room) = active_room {
                            room.member_ids
                                .iter()
                                .filter(|id| {
                                    detail
                                        .config
                                        .find_agent(id.as_str())
                                        .is_some_and(|a| !a.has_left)
                                })
                                .count()
                        } else {
                            detail.config.agents.iter().filter(|a| !a.has_left).count()
                        }
                        .max(1) as u32;

                        let seq = self.db.next_seq(&exp_id).unwrap_or(1);
                        let pass_msg = TimelineMessage {
                            id: msg_id,
                            experiment_id: exp_id.clone(),
                            seq,
                            turn_number: detail.total_turns,
                            role: SpeakerRole::GodSystem,
                            agent_id: None,
                            room_id: active_room
                                .as_ref()
                                .filter(|r| r.member_ids.iter().any(|id| id == &node_cfg.id))
                                .map(|r| r.room_id.clone())
                                .unwrap_or_else(|| "main".to_string()),
                            reasoning_content: String::new(),
                            content: format!("{} {}", node_cfg.label, PASS_NOTICE_SUFFIX),
                            micro_cue: None,
                            model_name: String::new(),
                            duration_ms: res.duration_ms,
                            prompt_tokens: res.usage.prompt_tokens,
                            completion_tokens: res.usage.completion_tokens,
                            total_tokens: res.usage.total_tokens,
                            whisper_consumed: false,
                            created_at: Utc::now().to_rfc3339(),
                        };
                        let new_total_tokens = detail.total_tokens + res.usage.total_tokens;
                        let _ = self.db.append_message(&pass_msg);
                        let next_speaker_id = select_next_speaker_in_room(
                            &detail.config,
                            &node_cfg.id,
                            "",
                            active_room.as_ref(),
                        );
                        let _ = self.db.update_experiment_progress(
                            &exp_id,
                            detail.total_turns,
                            new_total_tokens,
                            &next_speaker_id,
                        );
                        self.emit(StreamEvent::StreamEnd {
                            experiment_id: exp_id.clone(),
                            message: pass_msg,
                            total_turns: detail.total_turns,
                            total_tokens: new_total_tokens,
                        })
                        .await;

                        if consec_passes >= active_in_scope {
                            self.consecutive_passes.store(0, Ordering::SeqCst);
                            let end_text = "💬 全员已无补充，对话自然结束".to_string();
                            let end_seq = self.db.next_seq(&exp_id).unwrap_or(seq + 1);
                            let end_msg = TimelineMessage {
                                id: Uuid::new_v4().to_string(),
                                experiment_id: exp_id.clone(),
                                seq: end_seq,
                                turn_number: detail.total_turns,
                                role: SpeakerRole::GodSystem,
                                agent_id: None,
                                room_id: "main".to_string(),
                                reasoning_content: String::new(),
                                content: end_text.clone(),
                                micro_cue: None,
                                model_name: String::new(),
                                duration_ms: 0,
                                prompt_tokens: 0,
                                completion_tokens: 0,
                                total_tokens: 0,
                                whisper_consumed: false,
                                created_at: Utc::now().to_rfc3339(),
                            };
                            if self.db.append_message(&end_msg).is_ok() {
                                self.emit(StreamEvent::MessageAdded {
                                    experiment_id: exp_id.clone(),
                                    message: end_msg,
                                })
                                .await;
                            }
                            self.pause(Some(end_text)).await;
                            continue;
                        }

                        if self.current_state() == EngineState::Stepping {
                            self.state
                                .store(state_to_u8(EngineState::Paused), Ordering::SeqCst);
                            let consec = self.consecutive_turns.load(Ordering::SeqCst);
                            self.emit(StreamEvent::StateChanged {
                                experiment_id: exp_id,
                                state: EngineState::Paused,
                                reason: None,
                                consecutive_turns: consec,
                            })
                            .await;
                        }
                        continue;
                    }

                    self.consecutive_passes.store(0, Ordering::SeqCst);

                    let current_room_id = active_room
                        .as_ref()
                        .filter(|r| r.member_ids.iter().any(|id| id == &node_cfg.id))
                        .map(|r| r.room_id.clone())
                        .unwrap_or_else(|| "main".to_string());

                    let all_agent_labels: Vec<&str> = detail
                        .config
                        .agents
                        .iter()
                        .map(|a| a.label.as_str())
                        .collect();
                    let prefix_stripped = strip_leading_speaker_prefix(
                        &res.content,
                        &node_cfg.label,
                        &all_agent_labels,
                    );
                    let (leave_stripped, agent_has_left) = extract_leave_marker(&prefix_stripped);
                    let (mut sanitized_content, micro_cue) = extract_micro_cue(&leave_stripped);
                    if agent_has_left && sanitized_content.trim().is_empty() {
                        sanitized_content = "（转身离开）".to_string();
                    }
                    let speaker_id_owned = node_cfg.id.clone();

                    let seq = self.db.next_seq(&exp_id).unwrap_or(1);
                    let final_msg = TimelineMessage {
                        id: msg_id,
                        experiment_id: exp_id.clone(),
                        seq,
                        turn_number: next_turn,
                        role: SpeakerRole::Ai,
                        agent_id: Some(node_cfg.id.clone()),
                        room_id: current_room_id,
                        reasoning_content: res.reasoning_content,
                        content: sanitized_content,
                        micro_cue,
                        model_name: node_cfg.model_name.clone(),
                        duration_ms: res.duration_ms,
                        prompt_tokens: res.usage.prompt_tokens,
                        completion_tokens: res.usage.completion_tokens,
                        total_tokens: res.usage.total_tokens,
                        whisper_consumed: false,
                        created_at: Utc::now().to_rfc3339(),
                    };

                    let new_total_tokens = detail.total_tokens + res.usage.total_tokens;

                    if let Err(e) = self.db.append_message(&final_msg) {
                        self.emit(StreamEvent::EngineError {
                            experiment_id: exp_id.clone(),
                            message: format!("保存消息失败: {e}"),
                        })
                        .await;
                    } else {
                        let _ = self
                            .db
                            .save_message_raw_prompt(&final_msg.id, &raw_prompt_json);
                    }

                    if agent_has_left {
                        detail.config.agents[speaker_idx].has_left = true;
                        let _ = self
                            .db
                            .update_experiment(&exp_id, &detail.title, &detail.config);
                        self.emit(StreamEvent::ConfigHotUpdated {
                            experiment_id: exp_id.clone(),
                            config: detail.config.clone(),
                        })
                        .await;
                    }

                    // 空间隔离与私密房间轮次递减
                    let mut closed_room_notice: Option<TimelineMessage> = None;
                    let next_speaker_id = if let Some(mut room) = active_room {
                        if agent_has_left {
                            room.member_ids.retain(|id| id != &speaker_id_owned);
                        }
                        room.remaining_turns = room.remaining_turns.saturating_sub(1);
                        if room.remaining_turns == 0 || room.member_ids.len() < 2 {
                            let lobby_speaker = detail
                                .config
                                .agents
                                .iter()
                                .find(|a| !a.has_left && !room.member_ids.iter().any(|id| id == &a.id))
                                .map(|a| a.id.clone())
                                .unwrap_or_else(|| {
                                    select_next_speaker(
                                        &detail.config,
                                        &speaker_id_owned,
                                        &final_msg.content,
                                    )
                                });
                            let _ =
                                self.db
                                    .update_active_room(&exp_id, &None, &detail.room_members);
                            self.emit(StreamEvent::PrivateRoomUpdated {
                                experiment_id: exp_id.clone(),
                                active_room: None,
                                room_members: detail.room_members.clone(),
                            })
                            .await;
                            let l1 = room
                                .member_ids
                                .first()
                                .map(|id| detail.config.agent_label(Some(id)))
                                .unwrap_or_default();
                            let l2 = room
                                .member_ids
                                .get(1)
                                .map(|id| detail.config.agent_label(Some(id)))
                                .unwrap_or_default();
                            let nseq = self.db.next_seq(&exp_id).unwrap_or(seq + 1);
                            let nmsg = TimelineMessage {
                                id: Uuid::new_v4().to_string(),
                                experiment_id: exp_id.clone(),
                                seq: nseq,
                                turn_number: next_turn,
                                role: SpeakerRole::GodSystem,
                                agent_id: None,
                                room_id: "main".to_string(),
                                reasoning_content: String::new(),
                                content: format!(
                                    "【空间变动】{} 与 {} 结束了私聊，各自回到大厅。",
                                    l1, l2
                                ),
                                micro_cue: None,
                                model_name: String::new(),
                                duration_ms: 0,
                                prompt_tokens: 0,
                                completion_tokens: 0,
                                total_tokens: 0,
                                whisper_consumed: false,
                                created_at: Utc::now().to_rfc3339(),
                            };
                            if self.db.append_message(&nmsg).is_ok() {
                                closed_room_notice = Some(nmsg);
                            }
                            lobby_speaker
                        } else {
                            let sp = select_next_speaker_in_room(
                                &detail.config,
                                &speaker_id_owned,
                                &final_msg.content,
                                Some(&room),
                            );
                            let _ = self.db.update_active_room(
                                &exp_id,
                                &Some(room.clone()),
                                &detail.room_members,
                            );
                            self.emit(StreamEvent::PrivateRoomUpdated {
                                experiment_id: exp_id.clone(),
                                active_room: Some(room),
                                room_members: detail.room_members.clone(),
                            })
                            .await;
                            sp
                        }
                    } else {
                        select_next_speaker(&detail.config, &speaker_id_owned, &final_msg.content)
                    };

                    let _ = self.db.update_experiment_progress(
                        &exp_id,
                        next_turn,
                        new_total_tokens,
                        &next_speaker_id,
                    );
                    let _ = crate::snapshot::capture_turn_snapshot(&self.db, &exp_id, next_turn);

                    let consec = self.consecutive_turns.fetch_add(1, Ordering::SeqCst) + 1;

                    self.emit(StreamEvent::StreamEnd {
                        experiment_id: exp_id.clone(),
                        message: final_msg.clone(),
                        total_turns: next_turn,
                        total_tokens: new_total_tokens,
                    })
                    .await;

                    if let Some(nmsg) = closed_room_notice {
                        detail.messages.push(nmsg.clone());
                        self.emit(StreamEvent::MessageAdded {
                            experiment_id: exp_id.clone(),
                            message: nmsg,
                        })
                        .await;
                    }

                    if detail.config.observer_interval > 0
                        && next_turn % detail.config.observer_interval == 0
                    {
                        let keep_prev = 23usize;
                        let tail_start = detail.messages.len().saturating_sub(keep_prev);
                        let mut updated_msgs = Vec::with_capacity(
                            (detail.messages.len() - tail_start) + 1,
                        );
                        updated_msgs.extend_from_slice(&detail.messages[tail_start..]);
                        updated_msgs.push(final_msg.clone());
                        let latest_board = self
                            .db
                            .get_board_content(&exp_id)
                            .unwrap_or(detail.board_content);
                        self.spawn_observer_task(
                            exp_id.clone(),
                            next_turn,
                            detail.config.clone(),
                            updated_msgs,
                            latest_board,
                            compacted_summary,
                        )
                        .await;
                    }

                    detail.messages.push(final_msg);

                    // 第二层终局检查：退场后在场活跃角色少于 2 人
                    let remaining_active =
                        detail.config.agents.iter().filter(|a| !a.has_left).count();
                    if (detail.config.agents.len() >= 2 && remaining_active < 2)
                        || remaining_active == 0
                    {
                        let end_text = "🎬 角色已悉数离场，本场推演自然落幕".to_string();
                        let end_seq = self.db.next_seq(&exp_id).unwrap_or(seq + 1);
                        let end_msg = TimelineMessage {
                            id: Uuid::new_v4().to_string(),
                            experiment_id: exp_id.clone(),
                            seq: end_seq,
                            turn_number: next_turn,
                            role: SpeakerRole::GodSystem,
                            agent_id: None,
                            room_id: "main".to_string(),
                            reasoning_content: String::new(),
                            content: end_text.clone(),
                            micro_cue: None,
                            model_name: String::new(),
                            duration_ms: 0,
                            prompt_tokens: 0,
                            completion_tokens: 0,
                            total_tokens: 0,
                            whisper_consumed: false,
                            created_at: Utc::now().to_rfc3339(),
                        };
                        if self.db.append_message(&end_msg).is_ok() {
                            self.emit(StreamEvent::MessageAdded {
                                experiment_id: exp_id.clone(),
                                message: end_msg,
                            })
                            .await;
                        }
                        self.pause(Some(end_text)).await;
                        continue;
                    }

                    // 第三层终局检查：连续 3 条非沉默发言属于道别收尾语义，或连续 3 轮高度语义重复
                    if detect_natural_wrapup_loop(&detail.messages).is_some() {
                        let progress_lt_50 = if detail.config.enable_narrative_arc
                            && detail.config.target_total_turns > 0
                        {
                            (next_turn as u64) * 100
                                < (detail.config.target_total_turns as u64) * 50
                        } else if detail.config.max_consecutive_turns > 0 {
                            (consec as u64) * 100
                                < (detail.config.max_consecutive_turns as u64) * 50
                        } else {
                            next_turn < 10
                        };
                        let already_rescued = detail.messages.iter().any(|m| {
                            m.role == SpeakerRole::GodSystem
                                && m.content.contains(FAREWELL_RESCUE_MARKER)
                        });
                        if detail.config.enable_auto_director
                            && progress_lt_50
                            && !already_rescued
                        {
                            let rescue_seq = self.db.next_seq(&exp_id).unwrap_or(seq + 1);
                            let rescue_msg = TimelineMessage {
                                id: Uuid::new_v4().to_string(),
                                experiment_id: exp_id.clone(),
                                seq: rescue_seq,
                                turn_number: next_turn,
                                role: SpeakerRole::GodSystem,
                                agent_id: None,
                                room_id: "main".to_string(),
                                reasoning_content: String::new(),
                                content: format!(
                                    "{} 众人正准备散场之际，现场忽然发生意外变故——原本以为已经尘埃落定的线索突然出现异常回响，迫使所有人重新停下脚步。",
                                    FAREWELL_RESCUE_MARKER
                                ),
                                micro_cue: None,
                                model_name: String::new(),
                                duration_ms: 0,
                                prompt_tokens: 0,
                                completion_tokens: 0,
                                total_tokens: 0,
                                whisper_consumed: false,
                                created_at: Utc::now().to_rfc3339(),
                            };
                            if self.db.append_message(&rescue_msg).is_ok() {
                                self.emit(StreamEvent::MessageAdded {
                                    experiment_id: exp_id.clone(),
                                    message: rescue_msg,
                                })
                                .await;
                            }
                        } else {
                            let end_text = "🎬 角色已互相道别，本场推演自然落幕".to_string();
                            let end_seq = self.db.next_seq(&exp_id).unwrap_or(seq + 1);
                            let end_msg = TimelineMessage {
                                id: Uuid::new_v4().to_string(),
                                experiment_id: exp_id.clone(),
                                seq: end_seq,
                                turn_number: next_turn,
                                role: SpeakerRole::GodSystem,
                                agent_id: None,
                                room_id: "main".to_string(),
                                reasoning_content: String::new(),
                                content: end_text.clone(),
                                micro_cue: None,
                                model_name: String::new(),
                                duration_ms: 0,
                                prompt_tokens: 0,
                                completion_tokens: 0,
                                total_tokens: 0,
                                whisper_consumed: false,
                                created_at: Utc::now().to_rfc3339(),
                            };
                            if self.db.append_message(&end_msg).is_ok() {
                                self.emit(StreamEvent::MessageAdded {
                                    experiment_id: exp_id.clone(),
                                    message: end_msg,
                                })
                                .await;
                            }
                            self.pause(Some(end_text)).await;
                            continue;
                        }
                    }

                    if detail.config.enable_narrative_arc
                        && detail.config.target_total_turns > 0
                        && next_turn >= detail.config.target_total_turns
                    {
                        self.emit(StreamEvent::NarrativeFinaleReached {
                            experiment_id: exp_id.clone(),
                            total_turns: next_turn,
                            target_turns: detail.config.target_total_turns,
                        })
                        .await;
                        self.pause(Some(format!(
                            "已推进至第 {} 轮目标结局，故事自然落幕！",
                            next_turn
                        )))
                        .await;
                        continue;
                    }

                    if self.current_state() == EngineState::Stepping {
                        self.state
                            .store(state_to_u8(EngineState::Paused), Ordering::SeqCst);
                        self.emit(StreamEvent::StateChanged {
                            experiment_id: exp_id,
                            state: EngineState::Paused,
                            reason: None,
                            consecutive_turns: consec,
                        })
                        .await;
                        continue;
                    }

                    let pause_ms = (detail.config.pause_seconds.clamp(0.2, 30.0) * 1000.0) as u64;
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(pause_ms)) => {}
                        _ = self.notify.notified() => {}
                    }
                }
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("__CANCELLED__") {
                        self.emit(StreamEvent::StateChanged {
                            experiment_id: exp_id,
                            state: self.current_state(),
                            reason: None,
                            consecutive_turns: self.consecutive_turns.load(Ordering::SeqCst),
                        })
                        .await;
                    } else {
                        self.emit(StreamEvent::EngineError {
                            experiment_id: exp_id,
                            message: msg.clone(),
                        })
                        .await;
                        self.pause(Some(format!("请求异常: {msg}"))).await;
                    }
                }
            }
        }
    }

    /// 执行单个智能体发言回合（单回合最多 2 次工具调用且同一工具最多 1 次；守门员驳回可重试；异步审批防死锁）
    async fn execute_turn_with_tools(
        &self,
        exp_id: &str,
        msg_id: &str,
        node_cfg: &NodeConfig,
        mut turn_messages: Vec<ApiChatMessage>,
        enable_board_tools: bool,
        enable_private_notes: bool,
        thinking_params: &crate::llm::ResolvedThinkingParams,
    ) -> Result<StreamCompletionResult> {
        let all_tools_schema = sandbox_tools_schema(enable_board_tools, enable_private_notes);

        let mut total_reasoning = String::new();
        let mut total_usage = TokenUsage::default();
        let mut total_duration_ms = 0u64;
        let mut total_calls_used = 0usize;
        let mut used_tools: HashSet<String> = HashSet::new();
        let mut guardian_retries_used = 0usize;
        let mut nudged_for_nonempty = false;

        loop {
            let remaining_quota = 2usize.saturating_sub(total_calls_used);
            let available_tools: Vec<serde_json::Value> =
                if remaining_quota > 0 && !nudged_for_nonempty {
                    all_tools_schema
                        .iter()
                        .filter(|t| {
                            let name = t["function"]["name"].as_str().unwrap_or("");
                            !used_tools.contains(name)
                        })
                        .cloned()
                        .collect()
                } else {
                    Vec::new()
                };
            let allow_tools_this_call = !available_tools.is_empty();
            let active_tools = if allow_tools_this_call {
                Some(available_tools.as_slice())
            } else {
                None
            };

            let cancel_rx = self.cancel_tx.subscribe();
            let channel_ref = Arc::clone(&self.channel);
            let exp_id_clone = exp_id.to_string();
            let msg_id_clone = msg_id.to_string();

            struct DeltaBatch {
                reasoning: String,
                content: String,
                last_flush: std::time::Instant,
            }
            let delta_batch = Arc::new(std::sync::Mutex::new(DeltaBatch {
                reasoning: String::new(),
                content: String::new(),
                last_flush: std::time::Instant::now(),
            }));
            let emit_event = |ch_arc: &Arc<RwLock<Option<Channel<StreamEvent>>>>, ev: StreamEvent| {
                if let Ok(guard) = ch_arc.try_read() {
                    if let Some(ch) = guard.as_ref() {
                        let _ = ch.send(ev);
                        return;
                    }
                }
                let ch_clone = Arc::clone(ch_arc);
                tauri::async_runtime::spawn(async move {
                    let guard = ch_clone.read().await;
                    if let Some(ch) = guard.as_ref() {
                        let _ = ch.send(ev);
                    }
                });
            };
            let flush_batch = |batch_arc: &Arc<std::sync::Mutex<DeltaBatch>>,
                               ch_arc: &Arc<RwLock<Option<Channel<StreamEvent>>>>,
                               e_id: &str,
                               m_id: &str,
                               force: bool| {
                let (r_out, c_out) = {
                    let mut b = batch_arc.lock().unwrap();
                    if !force
                        && b.last_flush.elapsed() < Duration::from_millis(24)
                        && (b.reasoning.len() + b.content.len()) < 256
                    {
                        return;
                    }
                    b.last_flush = std::time::Instant::now();
                    (
                        std::mem::take(&mut b.reasoning),
                        std::mem::take(&mut b.content),
                    )
                };
                if !r_out.is_empty() {
                    emit_event(
                        ch_arc,
                        StreamEvent::StreamReasoningDelta {
                            experiment_id: e_id.to_string(),
                            message_id: m_id.to_string(),
                            delta: r_out,
                        },
                    );
                }
                if !c_out.is_empty() {
                    emit_event(
                        ch_arc,
                        StreamEvent::StreamDelta {
                            experiment_id: e_id.to_string(),
                            message_id: m_id.to_string(),
                            delta: c_out,
                        },
                    );
                }
            };

            let batch_for_cb = Arc::clone(&delta_batch);
            let ch_for_cb = Arc::clone(&channel_ref);
            let e_for_cb = exp_id_clone.clone();
            let m_for_cb = msg_id_clone.clone();

            let res = stream_chat_completion(
                &self.client,
                node_cfg,
                &turn_messages,
                active_tools,
                thinking_params,
                cancel_rx,
                move |chunk| match chunk {
                    StreamDeltaChunk::Content(delta) => {
                        {
                            let mut b = batch_for_cb.lock().unwrap();
                            b.content.push_str(delta);
                        }
                        flush_batch(&batch_for_cb, &ch_for_cb, &e_for_cb, &m_for_cb, false);
                    }
                    StreamDeltaChunk::Reasoning(delta) => {
                        {
                            let mut b = batch_for_cb.lock().unwrap();
                            b.reasoning.push_str(delta);
                        }
                        flush_batch(&batch_for_cb, &ch_for_cb, &e_for_cb, &m_for_cb, false);
                    }
                    StreamDeltaChunk::RetryNotice(msg) => {
                        flush_batch(&batch_for_cb, &ch_for_cb, &e_for_cb, &m_for_cb, true);
                        emit_event(
                            &ch_for_cb,
                            StreamEvent::EngineNotice {
                                experiment_id: e_for_cb.clone(),
                                message: msg,
                            },
                        );
                    }
                },
            )
            .await;
            flush_batch(&delta_batch, &channel_ref, &exp_id_clone, &msg_id_clone, true);
            let res = res?;

            if !res.reasoning_content.is_empty() {
                if !total_reasoning.is_empty() {
                    total_reasoning.push('\n');
                }
                total_reasoning.push_str(&res.reasoning_content);
            }
            total_usage.prompt_tokens += res.usage.prompt_tokens;
            total_usage.completion_tokens += res.usage.completion_tokens;
            total_usage.total_tokens += res.usage.total_tokens;
            total_duration_ms += res.duration_ms;

            if !res.tool_calls.is_empty() && allow_tools_this_call {
                let calls_to_run: Vec<_> = res.tool_calls.into_iter().take(2).collect();
                turn_messages.push(ApiChatMessage {
                    role: "assistant".to_string(),
                    content: if res.content.trim().is_empty() {
                        None
                    } else {
                        Some(res.content)
                    },
                    tool_calls: Some(calls_to_run.clone()),
                    tool_call_id: None,
                });

                let mut guardian_rejected = false;
                for tc in calls_to_run {
                    let tool_name = tc.function.name.trim().to_string();
                    if total_calls_used >= 2 || used_tools.contains(&tool_name) {
                        let reject_msg = serde_json::json!({
                            "ok": false,
                            "error": format!("工具「{}」本回合已调用过或已达单回合最多 2 次工具调用上限，请直接继续正式对话。", tool_name),
                        })
                        .to_string();
                        turn_messages.push(ApiChatMessage::tool_result(&tc.id, reject_msg));
                        continue;
                    }

                    self.emit(StreamEvent::ToolCallStarted {
                        experiment_id: exp_id.to_string(),
                        message_id: msg_id.to_string(),
                        agent_label: node_cfg.label.clone(),
                        tool_name: tool_name.clone(),
                    })
                    .await;

                    let tool_result = self
                        .execute_board_tool(exp_id, node_cfg, &tool_name, &tc)
                        .await?;
                    if tool_result.contains("\"rejected_by_guardian\"") {
                        guardian_rejected = true;
                        guardian_retries_used += 1;
                        if guardian_retries_used > 1 {
                            used_tools.insert(tool_name);
                        }
                    } else {
                        total_calls_used += 1;
                        used_tools.insert(tool_name);
                    }
                    turn_messages.push(ApiChatMessage::tool_result(&tc.id, tool_result));
                }

                if guardian_rejected && guardian_retries_used <= 1 {
                    let nudge = if enable_private_notes {
                        "守门员审核未通过（涉及主观推断或私密设定）。请改写入 update_private_note，并用客观第三人称重写一次 update_board，或放弃更新直接发表正式对话内容。"
                    } else {
                        "守门员审核未通过，请立即用客观第三人称（不含私密身份或第一人称）重写一次 update_board，或放弃更新并直接发表你的正式对话内容。"
                    };
                    turn_messages.push(ApiChatMessage::text("user", nudge));
                } else if total_calls_used >= 2 || used_tools.len() >= all_tools_schema.len() {
                    turn_messages.push(ApiChatMessage::text(
                        "user",
                        "工具调用已达本回合上限，请立即结合当前状态发表你的正式对话内容。",
                    ));
                } else {
                    turn_messages.push(ApiChatMessage::text(
                        "user",
                        "工具已执行完毕，请继续调用另一可用工具或直接发表你的正式对话内容。",
                    ));
                }
                continue;
            }

            let final_content = res.content.trim().to_string();
            if final_content.is_empty() && !nudged_for_nonempty {
                nudged_for_nonempty = true;
                turn_messages.push(ApiChatMessage::text(
                    "user",
                    "请不要只返回空内容，请结合你的角色与当前白板状态直接输出正式对话。",
                ));
                continue;
            }

            let non_empty_content = if final_content.is_empty() {
                "（已完成状态同步，请各位继续推进讨论。）".to_string()
            } else {
                res.content
            };

            return Ok(StreamCompletionResult {
                content: non_empty_content,
                reasoning_content: total_reasoning,
                tool_calls: Vec::new(),
                usage: total_usage,
                duration_ms: total_duration_ms,
            });
        }
    }

    async fn execute_board_tool(
        &self,
        exp_id: &str,
        node_cfg: &NodeConfig,
        _tool_name: &str,
        call: &crate::sandbox_tools::ApiToolCall,
    ) -> Result<String> {
        match parse_board_tool_call(call) {
            ParsedBoardTool::ReadBoard => {
                let board = self.db.get_board_content(exp_id).unwrap_or_default();
                let resp = serde_json::json!({
                    "ok": true,
                    "board_content": if board.trim().is_empty() { "（当前白板为空）".to_string() } else { board }
                });
                Ok(resp.to_string())
            }
            ParsedBoardTool::UpdateBoard {
                new_content,
                reason,
            } => {
                let (cfg, current_board) = self.db.get_experiment_header(exp_id)?;
                let mode = cfg.board_approval_mode;

                let approved_content: Option<String> = match mode {
                    BoardApprovalMode::Auto => Some(new_content),
                    BoardApprovalMode::Guardian => {
                        match guardian_review_board(
                            &self.client,
                            &cfg,
                            node_cfg,
                            &current_board,
                            &new_content,
                            &reason,
                        )
                        .await
                        {
                            GuardianVerdict::Approved => Some(new_content),
                            GuardianVerdict::Rejected { reason: rej_reason } => {
                                self.emit(StreamEvent::EngineNotice {
                                    experiment_id: exp_id.to_string(),
                                    message: format!(
                                        "守门员已拦截「{}」的白板写入：{}",
                                        node_cfg.label, rej_reason
                                    ),
                                })
                                .await;
                                let reject_hint = if cfg.enable_private_notes {
                                    format!(
                                        "守门员审核驳回：{}。涉及主观推断或私密设定，请改写入 update_private_note 并重写客观白板。",
                                        rej_reason
                                    )
                                } else {
                                    format!(
                                        "守门员审核驳回：{}。涉及主观推断或私密设定，请严格使用客观第三人称重写客观白板（绝不泄露隐藏人设）。",
                                        rej_reason
                                    )
                                };
                                return Ok(serde_json::json!({
                                    "ok": false,
                                    "status": "rejected_by_guardian",
                                    "message": reject_hint,
                                    "current_board": current_board,
                                })
                                .to_string());
                            }
                        }
                    }
                    BoardApprovalMode::Approval => {
                        let request_id = Uuid::new_v4().to_string();
                        let (tx, mut rx) = oneshot::channel::<ApprovalDecision>();
                        {
                            let mut guard = self.pending_approval.lock().await;
                            *guard = Some((request_id.clone(), tx));
                        }

                        self.emit(StreamEvent::BoardApprovalRequested {
                            experiment_id: exp_id.to_string(),
                            request_id: request_id.clone(),
                            agent_id: node_cfg.id.clone(),
                            agent_label: node_cfg.label.clone(),
                            current_content: current_board.clone(),
                            proposed_content: new_content.clone(),
                            reason: reason.clone(),
                        })
                        .await;

                        let mut cancel_rx = self.cancel_tx.subscribe();
                        let initial_gen = *cancel_rx.borrow();

                        let decision = loop {
                            tokio::select! {
                                res = &mut rx => {
                                    break match res {
                                        Ok(d) => d,
                                        Err(_) => ApprovalDecision::Reject {
                                            reason: Some("审批请求已取消".to_string()),
                                        },
                                    };
                                }
                                _ = cancel_rx.changed() => {
                                    if *cancel_rx.borrow() != initial_gen {
                                        let mut guard = self.pending_approval.lock().await;
                                        if guard.as_ref().map(|(id, _)| id == &request_id).unwrap_or(false) {
                                            *guard = None;
                                        }
                                        drop(guard);
                                        self.emit(StreamEvent::BoardApprovalResolved {
                                            experiment_id: exp_id.to_string(),
                                            request_id: request_id.clone(),
                                            approved: false,
                                        })
                                        .await;
                                        return Err(anyhow!("__CANCELLED__"));
                                    }
                                }
                            }
                        };

                        let is_approved = !matches!(decision, ApprovalDecision::Reject { .. });
                        self.emit(StreamEvent::BoardApprovalResolved {
                            experiment_id: exp_id.to_string(),
                            request_id,
                            approved: is_approved,
                        })
                        .await;

                        match decision {
                            ApprovalDecision::Approve => Some(new_content),
                            ApprovalDecision::ModifyAndApprove {
                                new_content: edited,
                            } => Some(edited),
                            ApprovalDecision::Reject { reason: rej } => {
                                return Ok(serde_json::json!({
                                    "ok": false,
                                    "status": "rejected_by_creator",
                                    "message": rej.unwrap_or_else(|| "主持人驳回了本次白板修改，请按当前白板继续发言。".to_string()),
                                    "current_board": current_board,
                                })
                                .to_string());
                            }
                        }
                    }
                };

                if let Some(content_to_save) = approved_content {
                    let (prev, saved) = self.db.update_shared_board(exp_id, &content_to_save)?;
                    self.emit(StreamEvent::BoardUpdated {
                        experiment_id: exp_id.to_string(),
                        board_content: saved.clone(),
                        board_prev_content: prev,
                        reason,
                        actor_label: node_cfg.label.clone(),
                    })
                    .await;
                    Ok(serde_json::json!({
                        "ok": true,
                        "status": "updated",
                        "board_content": saved,
                    })
                    .to_string())
                } else {
                    Ok(serde_json::json!({ "ok": false }).to_string())
                }
            }
            ParsedBoardTool::UpdatePrivateNote { note } => {
                let (cfg, _) = self.db.get_experiment_header(exp_id)?;
                if !cfg.enable_private_notes {
                    return Ok(serde_json::json!({
                        "ok": false,
                        "error": "当前实验未启用角色私有便签功能"
                    })
                    .to_string());
                }
                let (clamped, truncated) = truncate_private_note(&note, MAX_PRIVATE_NOTE_CHARS);
                let (saved, _) =
                    self.db
                        .update_agent_private_note(exp_id, &node_cfg.id, &clamped)?;
                self.emit(StreamEvent::PrivateNoteUpdated {
                    experiment_id: exp_id.to_string(),
                    agent_id: node_cfg.id.clone(),
                    agent_label: node_cfg.label.clone(),
                    note: saved.clone(),
                })
                .await;
                Ok(serde_json::json!({
                    "ok": true,
                    "status": "updated",
                    "note": saved,
                    "truncated": truncated,
                })
                .to_string())
            }
            ParsedBoardTool::Unknown { name } => Ok(serde_json::json!({
                "ok": false,
                "error": format!("未知工具: {name}")
            })
            .to_string()),
        }
    }
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

        // Explicit callout picks林医生 even though round-robin from AiA would be AiB
        assert_eq!(
            select_next_speaker(&cfg, "AiA", "我觉得老陈刚才的说法不对，林医生你怎么看？"),
            "AiC"
        );
        // Calling oneself never causes consecutive posting; falls back to round-robin
        assert_eq!(
            select_next_speaker(&cfg, "AiA", "阿哲我自己其实一直很犹豫……"),
            "AiB"
        );
        // No mention falls back to round-robin
        assert_eq!(
            select_next_speaker(&cfg, "AiB", "外面的雨越下越大了。"),
            "AiC"
        );
        // Disabled falls back to round-robin
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
        // 即使私聊中明确点名大厅角色“林医生”，候选池也必须严格锁定在 room_id 内部（返回 AiB）
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
        // 智能点名与轮转均应跳过已离场的 AiA
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
