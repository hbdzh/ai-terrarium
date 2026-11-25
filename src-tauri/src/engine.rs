use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::Utc;
use reqwest::Client;
use tauri::ipc::Channel;
use tokio::sync::{oneshot, watch, Mutex, Notify, RwLock};
use uuid::Uuid;

use crate::compactor::{estimate_messages_tokens, maybe_compact_history};
use crate::db::Database;
use crate::llm::{
    build_flipped_messages_with_memory, stream_chat_completion, ApiChatMessage,
    StreamCompletionResult, StreamDeltaChunk, TokenUsage,
};
use crate::models::{
    BoardApprovalMode, EngineState, ExperimentConfig, InterventionInput, NodeConfig, ObserverNote,
    SpeakerRole, StreamEvent, TimelineMessage,
};
use crate::observer::run_observer_analysis;
use crate::sandbox_tools::{
    board_tools_schema, parse_board_tool_call, ApprovalDecision, ParsedBoardTool,
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

pub struct TerrariumEngine {
    pub db: Arc<Database>,
    pub client: Client,
    pub active_experiment_id: Arc<RwLock<String>>,
    pub state: Arc<AtomicU8>,
    pub consecutive_turns: Arc<AtomicU32>,
    pub intervention_queue: Arc<Mutex<VecDeque<(String, InterventionInput)>>>,
    pub pending_approval: Arc<Mutex<Option<(String, oneshot::Sender<ApprovalDecision>)>>>,
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
            intervention_queue: Arc::new(Mutex::new(VecDeque::new())),
            pending_approval: Arc::new(Mutex::new(None)),
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
        self.consecutive_turns.store(0, Ordering::SeqCst);
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

    pub async fn step_once(&self) {
        let cur = self.current_state();
        if cur == EngineState::Running || cur == EngineState::Stepping {
            return;
        }
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

        let current_turn = self
            .db
            .get_experiment_detail(exp_id)
            .map(|d| d.total_turns)
            .unwrap_or(0);

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
            match run_observer_analysis(&client, &config, recent, &board_content).await {
                Ok(parsed) => {
                    let stagnation = parsed.stagnation_index.clamp(0, 100) as u8;
                    let should_perturb = stagnation > 80;
                    let note = ObserverNote {
                        id: Uuid::new_v4().to_string(),
                        experiment_id: exp_id.clone(),
                        trigger_turn,
                        tags: parsed.tags,
                        depth_stage: parsed.depth_stage,
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
                                    experiment_id: exp_id,
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
                                        experiment_id: exp_id,
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

            // Reload experiment detail
            let detail = match self.db.get_experiment_detail(&exp_id) {
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

            // 2. Check max consecutive turns & token budget circuit breakers
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

            // 3. Round-robin speaker selection across N agents
            let speaker_idx = detail
                .config
                .agents
                .iter()
                .position(|a| a.id == detail.next_speaker)
                .unwrap_or(0);
            let node_cfg = &detail.config.agents[speaker_idx];
            let next_idx = (speaker_idx + 1) % detail.config.agents.len();
            let next_speaker_id = detail.config.agents[next_idx].id.clone();

            let next_turn = detail.total_turns + 1;
            let mut compacted_summary = detail.compacted_summary.clone();
            let mut compacted_until_seq = detail.compacted_until_seq;
            let mut payload = build_flipped_messages_with_memory(
                &node_cfg.id,
                &detail.config,
                &detail.messages,
                &compacted_summary,
                compacted_until_seq,
            );

            // 3.5 Auto context compaction if estimated prompt tokens exceed threshold (e.g. 80%)
            let est_prompt_tokens = estimate_messages_tokens(&payload.messages);
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
                payload = build_flipped_messages_with_memory(
                    &node_cfg.id,
                    &detail.config,
                    &detail.messages,
                    &compacted_summary,
                    compacted_until_seq,
                );
                self.emit(StreamEvent::MemoryCompacted {
                    experiment_id: exp_id.clone(),
                    compacted_until_seq,
                    compacted_summary,
                    tokens_before: comp.tokens_before,
                    tokens_after: comp.tokens_after,
                })
                .await;
            }

            let msg_id = Uuid::new_v4().to_string();

            self.emit(StreamEvent::StreamStart {
                experiment_id: exp_id.clone(),
                message_id: msg_id.clone(),
                role: SpeakerRole::Ai,
                agent_id: node_cfg.id.clone(),
                agent_label: node_cfg.label.clone(),
                turn_number: next_turn,
                model_name: node_cfg.model_name.clone(),
            })
            .await;

            let stream_res = self
                .execute_turn_with_tools(
                    &exp_id,
                    &msg_id,
                    node_cfg,
                    payload.messages,
                    detail.config.enable_board_tools,
                )
                .await;

            match stream_res {
                Ok(res) => {
                    let _ = self.db.mark_whispers_consumed(&payload.consumed_whisper_ids);

                    let seq = self.db.next_seq(&exp_id).unwrap_or(1);
                    let final_msg = TimelineMessage {
                        id: msg_id,
                        experiment_id: exp_id.clone(),
                        seq,
                        turn_number: next_turn,
                        role: SpeakerRole::Ai,
                        agent_id: Some(node_cfg.id.clone()),
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
                    }
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

                    if detail.config.observer_interval > 0
                        && next_turn % detail.config.observer_interval == 0
                    {
                        let mut updated_msgs = detail.messages;
                        updated_msgs.push(final_msg);
                        let latest_board = self
                            .db
                            .get_experiment_detail(&exp_id)
                            .map(|d| d.board_content)
                            .unwrap_or(detail.board_content);
                        self.spawn_observer_task(
                            exp_id.clone(),
                            next_turn,
                            detail.config.clone(),
                            updated_msgs,
                            latest_board,
                        )
                        .await;
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

    /// 执行单个智能体发言回合（铁律 2：工具调用回合内闭环，单回合最多 2 次工具调用；铁律 3：异步审批防死锁）
    async fn execute_turn_with_tools(
        &self,
        exp_id: &str,
        msg_id: &str,
        node_cfg: &NodeConfig,
        mut turn_messages: Vec<ApiChatMessage>,
        enable_tools: bool,
    ) -> Result<StreamCompletionResult> {
        let tools_schema = if enable_tools {
            Some(board_tools_schema())
        } else {
            None
        };

        let mut total_reasoning = String::new();
        let mut total_usage = TokenUsage::default();
        let mut total_duration_ms = 0u64;
        let mut tool_calls_used = 0usize;
        let mut nudged_for_nonempty = false;

        loop {
            let allow_tools_this_call = tool_calls_used < 2;
            let active_tools = if allow_tools_this_call {
                tools_schema.as_deref()
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

            if !res.tool_calls.is_empty() && tool_calls_used < 2 {
                let remaining_quota = 2 - tool_calls_used;
                let calls_to_run: Vec<_> = res.tool_calls.into_iter().take(remaining_quota).collect();
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

                for tc in calls_to_run {
                    tool_calls_used += 1;
                    self.emit(StreamEvent::ToolCallStarted {
                        experiment_id: exp_id.to_string(),
                        message_id: msg_id.to_string(),
                        agent_label: node_cfg.label.clone(),
                        tool_name: tc.function.name.clone(),
                    })
                    .await;

                    let tool_result = self
                        .execute_board_tool(exp_id, node_cfg, &tc.function.name, &tc)
                        .await?;
                    turn_messages.push(ApiChatMessage::tool_result(&tc.id, tool_result));
                }

                if tool_calls_used >= 2 {
                    turn_messages.push(ApiChatMessage::text(
                        "user",
                        "工具调用已达本回合上限（2次），请立即结合刚才获取的白板信息发表你的正式对话内容。",
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
                tool_calls_used = 2; // 强制下一轮禁用工具
                continue;
            }

            let non_empty_content = if final_content.is_empty() {
                "（已完成共享白板检索与同步，请各位继续推进讨论。）".to_string()
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
                let board = self
                    .db
                    .get_experiment_detail(exp_id)
                    .map(|d| d.board_content)
                    .unwrap_or_default();
                let resp = serde_json::json!({
                    "ok": true,
                    "board_content": if board.trim().is_empty() { "（当前白板为空）".to_string() } else { board }
                });
                Ok(resp.to_string())
            }
            ParsedBoardTool::UpdateBoard { new_content, reason } => {
                let detail = self.db.get_experiment_detail(exp_id)?;
                let mode = detail.config.board_approval_mode;
                let current_board = detail.board_content;

                let approved_content: Option<String> = match mode {
                    BoardApprovalMode::Auto => Some(new_content),
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
            ParsedBoardTool::Unknown { name } => Ok(serde_json::json!({
                "ok": false,
                "error": format!("未知工具: {name}")
            })
            .to_string()),
        }
    }
}
