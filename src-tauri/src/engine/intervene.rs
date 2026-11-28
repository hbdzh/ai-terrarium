use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::Utc;
use tauri::ipc::Channel;
use tokio::sync::{oneshot, RwLock};
use uuid::Uuid;

use crate::llm::{
    stream_chat_completion, ApiChatMessage, ResolvedThinkingParams, StreamCompletionResult,
    StreamDeltaChunk, TokenUsage,
};
use crate::models::{
    ActivePrivateRoom, BoardApprovalMode, EngineState, InterventionInput, NodeConfig, SpeakerRole,
    StreamEvent, TimelineMessage,
};
use crate::sandbox_tools::{
    guardian_review_board, parse_board_tool_call, sandbox_tools_schema, truncate_private_note,
    ApiToolCall, ApprovalDecision, GuardianVerdict, ParsedBoardTool, MAX_PRIVATE_NOTE_CHARS,
};

use super::TerrariumEngine;

impl TerrariumEngine {
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

    pub(super) async fn drain_interventions(&self, exp_id: &str) {
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

    /// 执行单个智能体发言回合（单回合最多 2 次工具调用且同一工具最多 1 次；守门员驳回可重试；异步审批防死锁）
    pub(super) async fn execute_turn_with_tools(
        &self,
        exp_id: &str,
        msg_id: &str,
        node_cfg: &NodeConfig,
        mut turn_messages: Vec<ApiChatMessage>,
        enable_board_tools: bool,
        enable_private_notes: bool,
        thinking_params: &ResolvedThinkingParams,
    ) -> Result<StreamCompletionResult> {
        let all_tools_schema = sandbox_tools_schema(enable_board_tools, enable_private_notes);

        let mut total_reasoning = String::new();
        let mut total_usage = TokenUsage::default();
        let mut total_duration_ms = 0u64;
        let mut total_downgraded: Vec<String> = Vec::new();
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
            let ws_tx_ref = self.ws_tx.clone();
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
            let emit_event = |ch_arc: &Arc<RwLock<Option<Channel<StreamEvent>>>>,
                              ws_tx: &tokio::sync::broadcast::Sender<String>,
                              ev: StreamEvent| {
                if ws_tx.receiver_count() > 0 {
                    if let Ok(json) = serde_json::to_string(&ev) {
                        if ws_tx.send(json).is_ok() {
                            return;
                        }
                    }
                }
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
                               ws_tx: &tokio::sync::broadcast::Sender<String>,
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
                        ws_tx,
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
                        ws_tx,
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
            let ws_for_cb = ws_tx_ref.clone();
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
                        flush_batch(&batch_for_cb, &ch_for_cb, &ws_for_cb, &e_for_cb, &m_for_cb, false);
                    }
                    StreamDeltaChunk::Reasoning(delta) => {
                        {
                            let mut b = batch_for_cb.lock().unwrap();
                            b.reasoning.push_str(delta);
                        }
                        flush_batch(&batch_for_cb, &ch_for_cb, &ws_for_cb, &e_for_cb, &m_for_cb, false);
                    }
                    StreamDeltaChunk::RetryNotice(msg) => {
                        flush_batch(&batch_for_cb, &ch_for_cb, &ws_for_cb, &e_for_cb, &m_for_cb, true);
                        emit_event(
                            &ch_for_cb,
                            &ws_for_cb,
                            StreamEvent::EngineNotice {
                                experiment_id: e_for_cb.clone(),
                                message: msg,
                            },
                        );
                    }
                },
            )
            .await;
            flush_batch(&delta_batch, &channel_ref, &ws_tx_ref, &exp_id_clone, &msg_id_clone, true);
            let res = res?;

            for d in res.downgraded_params {
                if !total_downgraded.contains(&d) {
                    total_downgraded.push(d);
                }
            }
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
                downgraded_params: total_downgraded,
            });
        }
    }

    async fn execute_board_tool(
        &self,
        exp_id: &str,
        node_cfg: &NodeConfig,
        _tool_name: &str,
        call: &ApiToolCall,
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
                let aux_model = self.db.get_ui_prefs().ok().and_then(|p| p.aux_model);

                let approved_content: Option<String> = match mode {
                    BoardApprovalMode::Auto => Some(new_content),
                    BoardApprovalMode::Guardian => {
                        match guardian_review_board(
                            &self.client,
                            &cfg,
                            aux_model.as_ref(),
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
