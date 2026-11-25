use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::Utc;
use reqwest::Client;
use tauri::ipc::Channel;
use tokio::sync::{oneshot, watch, Mutex, Notify, RwLock};
use uuid::Uuid;

use crate::compactor::{estimate_messages_tokens, maybe_compact_history, CompactionGuard};
use crate::db::Database;
use crate::llm::{
    build_flipped_messages_with_room, stream_chat_completion, ApiChatMessage,
    StreamCompletionResult, StreamDeltaChunk, TokenUsage,
};
use crate::models::{
    ActivePrivateRoom, BoardApprovalMode, EngineState, ExperimentConfig, InterventionInput,
    NodeConfig, ObserverNote, SpeakerRole, StreamEvent, TimelineMessage,
};
use crate::observer::{parse_director_command, run_observer_analysis};
use crate::sandbox_tools::{
    guardian_review_board, parse_board_tool_call, sandbox_tools_schema, truncate_private_note,
    ApprovalDecision, GuardianVerdict, ParsedBoardTool, MAX_PRIVATE_NOTE_CHARS,
};
use crate::sanitizer::strip_think_tags;

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

/// 张力自适应思考降档（带突发覆盖）：
/// 低张力 (<50) 降档节省过场 Token；若存在未消费低语、近一轮环境事件或处于末 20% 阶段，强制恢复满血推理（None）。
pub fn resolve_reasoning_effort(
    config: &ExperimentConfig,
    latest_tension: Option<u8>,
    has_unconsumed_whisper: bool,
    has_recent_god_system: bool,
    progress_turn: u32,
) -> Option<String> {
    if !config.enable_adaptive_reasoning {
        return None;
    }
    if has_unconsumed_whisper || has_recent_god_system {
        return None;
    }
    if config.max_consecutive_turns > 0
        && (progress_turn as u64) * 100 > (config.max_consecutive_turns as u64) * 80
    {
        return None;
    }
    if config.enable_narrative_arc
        && config.target_total_turns > 0
        && (progress_turn as u64) * 100 > (config.target_total_turns as u64) * 80
    {
        return None;
    }
    let Some(tension) = latest_tension else {
        return None;
    };
    if tension < 50 {
        let tier = match config.adaptive_low_reasoning_effort.as_str() {
            "minimal" => "minimal",
            "medium" => "medium",
            _ => "low",
        };
        Some(tier.to_string())
    } else {
        None
    }
}

/// 3 人及以上智能点名接话路由：检测上一条发言是否明确点名其他角色，且严禁同一角色连续刷屏。
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
    let fallback = config.agents[(current_idx + 1) % config.agents.len()]
        .id
        .clone();

    if !config.enable_smart_selector || config.agents.len() < 3 {
        return fallback;
    }

    let cleaned = strip_think_tags(last_content);
    let mut best_match: Option<(usize, usize, usize, String)> = None;
    for (idx, agent) in config.agents.iter().enumerate() {
        if agent.id == current_speaker_id {
            continue;
        }
        let label = agent.label.trim();
        if label.is_empty() {
            continue;
        }
        if let Some(pos) = cleaned.rfind(label) {
            let rr_dist = (idx + config.agents.len() - current_idx) % config.agents.len();
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

/// 空间隔离最高优先级仲裁：当存在激活的私密房间时，候选池严格锁定在 room_id 成员内部，绝不因提及大厅角色而打破隔离。
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
                .filter(|id| id.as_str() != current_speaker_id)
                .collect();
            if candidates.is_empty() {
                return room.member_ids[0].clone();
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
    pub forced_next_speaker: Arc<Mutex<Option<String>>>,
    pub intervention_queue: Arc<Mutex<VecDeque<(String, InterventionInput)>>>,
    pub pending_approval: Arc<Mutex<Option<(String, oneshot::Sender<ApprovalDecision>)>>>,
    pub compaction_guard: Arc<Mutex<CompactionGuard>>,
    pub channel: Arc<RwLock<Option<Channel<StreamEvent>>>>,
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

        let engine = Arc::new(Self {
            db,
            client,
            active_experiment_id: Arc::new(RwLock::new(initial_experiment_id)),
            state: Arc::new(AtomicU8::new(state_to_u8(EngineState::Idle))),
            consecutive_turns: Arc::new(AtomicU32::new(0)),
            forced_next_speaker: Arc::new(Mutex::new(None)),
            intervention_queue: Arc::new(Mutex::new(VecDeque::new())),
            pending_approval: Arc::new(Mutex::new(None)),
            compaction_guard: Arc::new(Mutex::new(CompactionGuard::default())),
            channel: Arc::new(RwLock::new(None)),
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

    async fn emit(&self, event: StreamEvent) {
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

    pub async fn enqueue_intervention(&self, experiment_id: String, input: InterventionInput) -> usize {
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
            content: format!("【空间变动】{} 与 {} 结束了私聊，各自回到大厅。", label1, label2),
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

    pub async fn execute_director_command(
        &self,
        exp_id: &str,
        instruction: &str,
    ) -> Result<String> {
        let mut detail = self.db.get_experiment_detail(exp_id)?;
        let parsed = parse_director_command(&self.client, &detail.config, instruction).await?;

        let mut config_changed = false;
        if let Some(iv) = parsed.observer_interval {
            detail.config.observer_interval = iv;
            config_changed = true;
        }
        if let Some(ps) = parsed.pause_seconds {
            detail.config.pause_seconds = ps;
            config_changed = true;
        }
        if let Some(tt) = parsed.target_total_turns {
            detail.config.target_total_turns = tt;
            config_changed = true;
        }
        if let Some(arc) = parsed.enable_narrative_arc {
            detail.config.enable_narrative_arc = arc;
            config_changed = true;
        }
        if let Some(ref append) = parsed.global_system_prompt_append {
            if !detail.config.global_system_prompt.trim().is_empty() {
                detail.config.global_system_prompt.push('\n');
            }
            detail.config.global_system_prompt.push_str(append);
            config_changed = true;
        }

        if config_changed {
            self.db
                .update_experiment(exp_id, &detail.title, &detail.config)?;
            self.emit(StreamEvent::ConfigHotUpdated {
                experiment_id: exp_id.to_string(),
                config: detail.config.clone(),
            })
            .await;
        }

        for w in parsed.whispers {
            let matched = detail
                .config
                .agents
                .iter()
                .find(|a| !w.agent_id.is_empty() && a.id == w.agent_id)
                .or_else(|| {
                    detail
                        .config
                        .agents
                        .iter()
                        .find(|a| !w.agent_label.is_empty() && a.label.trim() == w.agent_label)
                });
            if let Some(agent) = matched {
                self.enqueue_intervention(
                    exp_id.to_string(),
                    InterventionInput {
                        kind: SpeakerRole::GodWhisper,
                        agent_id: Some(agent.id.clone()),
                        content: w.whisper,
                    },
                )
                .await;
            }
        }

        if let Some(ev) = parsed.system_event {
            self.enqueue_intervention(
                exp_id.to_string(),
                InterventionInput {
                    kind: SpeakerRole::GodSystem,
                    agent_id: None,
                    content: ev,
                },
            )
            .await;
        }

        let summary = if parsed.summary.is_empty() {
            "导演指令已热更新生效".to_string()
        } else {
            parsed.summary
        };
        self.emit(StreamEvent::EngineNotice {
            experiment_id: exp_id.to_string(),
            message: format!("🎬 导演遥控：{}", summary),
        })
        .await;
        Ok(summary)
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
        self.spawn_observer_task(
            exp_id,
            detail.total_turns,
            detail.config,
            detail.messages,
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
                    let should_perturb = stagnation > 80;
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

                    if should_perturb {
                        let perturbation = InterventionInput {
                            kind: SpeakerRole::GodSystem,
                            agent_id: None,
                            content: "【生态自动扰动 · 打破僵化】观测到当前对话已陷入高度复读或客套循环。请立即推翻刚才的隐含共识，引入一个截然相反的极端反例或具象悖论，继续交锋。".to_string(),
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

                    // 周期直觉反思：将 reflection_whispers 作为一次性 GodWhisper 注入对应角色的下一轮
                    if config.enable_periodic_reflection && !parsed.reflection_whispers.is_empty() {
                        let mut injected_agents: HashSet<String> = HashSet::new();
                        for rw in parsed.reflection_whispers {
                            let matched_agent = config
                                .agents
                                .iter()
                                .find(|a| !rw.agent_id.is_empty() && a.id == rw.agent_id)
                                .or_else(|| {
                                    config
                                        .agents
                                        .iter()
                                        .find(|a| !rw.agent_label.is_empty() && a.label.trim() == rw.agent_label)
                                });
                            let Some(agent) = matched_agent else {
                                continue;
                            };
                            if !injected_agents.insert(agent.id.clone()) {
                                continue;
                            }
                            let whisper_input = InterventionInput {
                                kind: SpeakerRole::GodWhisper,
                                agent_id: Some(agent.id.clone()),
                                content: rw.whisper,
                            };
                            let cur_state = u8_to_state(state.load(Ordering::SeqCst));
                            if cur_state == EngineState::Running || cur_state == EngineState::Stepping {
                                let len = {
                                    let mut q = queue.lock().await;
                                    q.push_back((exp_id.clone(), whisper_input));
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
                                    role: SpeakerRole::GodWhisper,
                                    agent_id: Some(agent.id.clone()),
                                    room_id: "main".to_string(),
                                    reasoning_content: String::new(),
                                    content: whisper_input.content,
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

            // Reload experiment detail
            let mut detail = match self.db.get_experiment_detail(&exp_id) {
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
                self.pause(Some("请至少配置 1 个 AI 智能体".to_string())).await;
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
                    "已达到目标总轮数 {} 轮，故事自然落幕！可生成《实验结案复盘简报》。",
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
                    "已达到最大连续运行 {} 轮熔断上限，自动暂停以防额度耗尽",
                    detail.config.max_consecutive_turns
                )))
                .await;
                continue;
            }

            if detail.config.token_budget > 0 && detail.total_tokens >= detail.config.token_budget {
                self.pause(Some(format!(
                    "已达到单局 Token 预算熔断上限 ({} / {})，自动暂停",
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
                if let Some(last_conv) = detail
                    .messages
                    .iter()
                    .rev()
                    .find(|m| m.room_id == "main" && matches!(m.role, SpeakerRole::Ai | SpeakerRole::GodPublic))
                {
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
                if detail.config.find_agent(&forced_id).is_some() {
                    desired_speaker_id = forced_id;
                }
            }
            let speaker_idx = detail
                .config
                .agents
                .iter()
                .position(|a| a.id == desired_speaker_id)
                .unwrap_or(0);
            let node_cfg = &detail.config.agents[speaker_idx];
            let agent_private_note = detail
                .private_notes
                .get(&node_cfg.id)
                .map(|s| s.as_str())
                .unwrap_or("");

            let core_engine_prompt = self
                .db
                .get_core_engine_prompt()
                .unwrap_or_else(|_| crate::models::DEFAULT_CORE_ENGINE_PROMPT.to_string());
            let mut compacted_summary = detail.compacted_summary.clone();
            let mut compacted_until_seq = detail.compacted_until_seq;
            let mut payload = build_flipped_messages_with_room(
                &node_cfg.id,
                &detail.config,
                &detail.messages,
                &compacted_summary,
                compacted_until_seq,
                &detail.board_content,
                agent_private_note,
                active_room.as_ref(),
                &detail.room_members,
                &core_engine_prompt,
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
                payload = build_flipped_messages_with_room(
                    &node_cfg.id,
                    &detail.config,
                    &detail.messages,
                    &compacted_summary,
                    compacted_until_seq,
                    &detail.board_content,
                    agent_private_note,
                    active_room.as_ref(),
                    &detail.room_members,
                    &core_engine_prompt,
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
                    let tripped = self.compaction_guard.lock().await.record_and_check(next_turn);
                    if tripped {
                        let warn_msg = "记忆压缩防抖动熔断：连续 3 轮内触发 3 次记忆压缩，已自动暂停引擎防止死循环烧钱。请前往「设置 → 调度与安全阀」调大上下文预算窗口。".to_string();
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

            let msg_id = Uuid::new_v4().to_string();
            let context_usage_pct = ((est_prompt_tokens * 100) / ctx_limit).min(999) as u16;
            let enable_board_tools =
                detail.config.enable_board_tools && detail.config.preset_id != "free";
            let enable_private_notes = detail.config.enable_private_notes;
            let tools_schema = sandbox_tools_schema(enable_board_tools, enable_private_notes);

            let latest_tension = detail.observer_notes.first().map(|n| n.tension_score);
            let progress_turn = if cur == EngineState::Running {
                self.consecutive_turns.load(Ordering::SeqCst) + 1
            } else {
                next_turn
            };
            let reasoning_effort = resolve_reasoning_effort(
                &detail.config,
                latest_tension,
                !payload.consumed_whisper_ids.is_empty(),
                payload.has_recent_god_system,
                progress_turn,
            );

            let raw_prompt_json = serde_json::json!({
                "model": node_cfg.model_name,
                "temperature": node_cfg.temperature,
                "reasoning_effort": &reasoning_effort,
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
                    reasoning_effort.as_deref(),
                )
                .await;

            match stream_res {
                Ok(res) => {
                    let _ = self.db.mark_whispers_consumed(&payload.consumed_whisper_ids);

                    let current_room_id = active_room
                        .as_ref()
                        .filter(|r| r.member_ids.iter().any(|id| id == &node_cfg.id))
                        .map(|r| r.room_id.clone())
                        .unwrap_or_else(|| "main".to_string());

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
                        content: res.content,
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

                    // 空间隔离与私密房间轮次递减
                    let mut closed_room_notice: Option<TimelineMessage> = None;
                    let next_speaker_id = if let Some(mut room) = active_room {
                        room.remaining_turns = room.remaining_turns.saturating_sub(1);
                        if room.remaining_turns == 0 {
                            let lobby_speaker = detail
                                .config
                                .agents
                                .iter()
                                .find(|a| !room.member_ids.iter().any(|id| id == &a.id))
                                .map(|a| a.id.clone())
                                .unwrap_or_else(|| {
                                    select_next_speaker(
                                        &detail.config,
                                        &node_cfg.id,
                                        &final_msg.content,
                                    )
                                });
                            let _ = self.db.update_active_room(
                                &exp_id,
                                &None,
                                &detail.room_members,
                            );
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
                                &node_cfg.id,
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
                        select_next_speaker(&detail.config, &node_cfg.id, &final_msg.content)
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
                        let mut updated_msgs = detail.messages;
                        updated_msgs.push(final_msg);
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
        reasoning_effort: Option<&str>,
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
            let available_tools: Vec<serde_json::Value> = if remaining_quota > 0 && !nudged_for_nonempty {
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

            let res = stream_chat_completion(
                &self.client,
                node_cfg,
                &turn_messages,
                active_tools,
                reasoning_effort,
                cancel_rx,
                move |chunk| {
                    let ch_arc = Arc::clone(&channel_ref);
                    let e_id = exp_id_clone.clone();
                    let m_id = msg_id_clone.clone();
                    let ev = match chunk {
                        StreamDeltaChunk::Content(delta) => StreamEvent::StreamDelta {
                            experiment_id: e_id,
                            message_id: m_id,
                            delta: delta.to_string(),
                        },
                        StreamDeltaChunk::Reasoning(delta) => StreamEvent::StreamReasoningDelta {
                            experiment_id: e_id,
                            message_id: m_id,
                            delta: delta.to_string(),
                        },
                        StreamDeltaChunk::RetryNotice(msg) => StreamEvent::EngineNotice {
                            experiment_id: e_id,
                            message: msg,
                        },
                    };
                    if let Ok(guard) = ch_arc.try_read() {
                        if let Some(ch) = guard.as_ref() {
                            let _ = ch.send(ev);
                            return;
                        }
                    }
                    tauri::async_runtime::spawn(async move {
                        let guard = ch_arc.read().await;
                        if let Some(ch) = guard.as_ref() {
                            let _ = ch.send(ev);
                        }
                    });
                },
            )
            .await?;

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
            ParsedBoardTool::UpdateBoard { new_content, reason } => {
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
                            ApprovalDecision::ModifyAndApprove { new_content: edited } => {
                                Some(edited)
                            }
                            ApprovalDecision::Reject { reason: rej } => {
                                return Ok(serde_json::json!({
                                    "ok": false,
                                    "status": "rejected_by_creator",
                                    "message": rej.unwrap_or_else(|| "造物主驳回了本次白板修改，请尊重现状并继续正常发言。".to_string()),
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
    fn test_resolve_reasoning_effort_arbitration_rules() {
        let mut cfg = ExperimentConfig::default();
        cfg.max_consecutive_turns = 100;
        cfg.enable_adaptive_reasoning = false;
        assert_eq!(
            resolve_reasoning_effort(&cfg, Some(20), false, false, 10),
            None
        );

        cfg.enable_adaptive_reasoning = true;
        cfg.adaptive_low_reasoning_effort = "low".to_string();
        // No observer tension score yet -> keep full
        assert_eq!(resolve_reasoning_effort(&cfg, None, false, false, 10), None);
        // Tension < 50 without override -> downgrade
        assert_eq!(
            resolve_reasoning_effort(&cfg, Some(35), false, false, 10),
            Some("low".to_string())
        );
        // Tension >= 50 -> full
        assert_eq!(resolve_reasoning_effort(&cfg, Some(50), false, false, 10), None);
        // Unconsumed GodWhisper overrides downgrade
        assert_eq!(resolve_reasoning_effort(&cfg, Some(20), true, false, 10), None);
        // Recent GodSystem overrides downgrade
        assert_eq!(resolve_reasoning_effort(&cfg, Some(20), false, true, 10), None);
        // Final 20% turns (> 80 of 100) overrides downgrade
        assert_eq!(
            resolve_reasoning_effort(&cfg, Some(20), false, false, 80),
            Some("low".to_string())
        );
        assert_eq!(resolve_reasoning_effort(&cfg, Some(20), false, false, 81), None);
    }

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
        assert_eq!(select_next_speaker(&cfg, "AiB", "外面的雨越下越大了。"), "AiC");
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
}
