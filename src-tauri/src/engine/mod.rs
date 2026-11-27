pub mod intervene;
pub mod prompt_builder;
pub mod scheduler;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use reqwest::Client;
use tauri::ipc::Channel;
use tokio::sync::{broadcast, oneshot, watch, Mutex, Notify, RwLock};
use uuid::Uuid;

use crate::compactor::{estimate_messages_tokens, maybe_compact_history, CompactionGuard};
use crate::db::Database;
use crate::models::{
    is_pass_notice, EngineState, ExperimentConfig, InterventionInput, ObserverNote, SpeakerRole,
    StreamEvent, TimelineMessage, PASS_NOTICE_SUFFIX,
};
use crate::observer::run_observer_analysis;
use crate::sandbox_tools::{sandbox_tools_schema, ApprovalDecision};
use crate::sanitizer::{
    extract_leave_marker, extract_micro_cue, strip_leading_speaker_prefix,
};

use prompt_builder::build_flipped_messages_with_room;
pub use scheduler::{
    detect_natural_wrapup_loop, is_pass_reply, select_next_speaker, select_next_speaker_in_room,
};
use scheduler::{maybe_roll_mood_dice, select_next_speaker_with_aux, FAREWELL_RESCUE_MARKER};

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

pub struct TerrariumEngine {
    pub db: Arc<Database>,
    pub client: Client,
    pub active_experiment_id: Arc<RwLock<String>>,
    pub state: Arc<AtomicU8>,
    pub state_version: Arc<AtomicU64>,
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
            state_version: Arc::new(AtomicU64::new(0)),
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

    pub fn current_state_version(&self) -> u64 {
        self.state_version.load(Ordering::SeqCst)
    }

    pub fn bump_state_version(&self) -> u64 {
        self.state_version.fetch_add(1, Ordering::SeqCst) + 1
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

    pub(super) async fn emit(&self, event: StreamEvent) {
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
        self.bump_state_version();
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

        let captured_version = self.current_state_version();

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
        let active_exp_ref = Arc::clone(&self.active_experiment_id);
        let state_version_ref = Arc::clone(&self.state_version);

        tauri::async_runtime::spawn(async move {
            let slice_start = messages.len().saturating_sub(24);
            let recent = &messages[slice_start..];
            let res = run_observer_analysis(
                &client,
                &config,
                recent,
                &board_content,
                &compacted_summary,
            )
            .await;

            // 异步任务状态版本校验：若期间切换了实验、分叉世界线或修改了配置，静默丢弃过期结果
            let is_stale = *active_exp_ref.read().await != exp_id
                || state_version_ref.load(Ordering::SeqCst) != captured_version;
            if is_stale {
                return;
            }

            match res {
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
                                        .find(|c| {
                                            c.kind == "GodSystem" && !c.content.trim().is_empty()
                                        })
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
                                    .update_experiment(
                                        &exp_id,
                                        &latest_detail.title,
                                        &latest_detail.config,
                                    )
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
            let turn_state_version = self.current_state_version();

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
                if *self.active_experiment_id.read().await != exp_id
                    || self.current_state_version() != turn_state_version
                {
                    continue;
                }
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

            let prompt_messages_for_debug = payload.messages.clone();
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

            // 状态版本校验：若推演期间实验被切换、分叉或修改，静默丢弃过期返回
            if *self.active_experiment_id.read().await != exp_id
                || self.current_state_version() != turn_state_version
            {
                continue;
            }

            let aux_model = self.db.get_ui_prefs().ok().and_then(|p| p.aux_model);

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

                    let raw_prompt_json = serde_json::json!({
                        "model": node_cfg.model_name,
                        "temperature": node_cfg.temperature,
                        "reasoning_effort": &thinking_params.reasoning_effort,
                        "enable_thinking": thinking_params.enable_thinking,
                        "thinking_budget": thinking_params.thinking_budget,
                        "thinking": &thinking_params.thinking,
                        "downgraded_params": if res.downgraded_params.is_empty() { None } else { Some(&res.downgraded_params) },
                        "private_note": if enable_private_notes { Some(agent_private_note) } else { None },
                        "messages": &prompt_messages_for_debug,
                        "tools": if !tools_schema.is_empty() { Some(&tools_schema) } else { None },
                    })
                    .to_string();

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
                        select_next_speaker_with_aux(
                            &self.client,
                            aux_model.as_ref(),
                            &detail.config,
                            &speaker_id_owned,
                            &final_msg.content,
                            None,
                        )
                        .await
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
}
