use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

use crate::db::Database;
use crate::engine::TerrariumEngine;
use crate::models::{
    CrisisCard, InterventionInput, ObserverNote, SpeakerRole, StreamEvent,
};
use crate::observer::run_observer_analysis;

#[derive(Clone)]
pub struct LocalApiState {
    pub db: Arc<Database>,
    pub engine: Arc<TerrariumEngine>,
    pub token: String,
}

struct RunningServer {
    port: u16,
    token: String,
    shutdown_tx: oneshot::Sender<()>,
}

pub struct LocalApiManager {
    inner: Mutex<Option<RunningServer>>,
}

impl LocalApiManager {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    pub async fn sync_config(
        &self,
        enabled: bool,
        port: u16,
        token: String,
        db: Arc<Database>,
        engine: Arc<TerrariumEngine>,
    ) -> Result<(), String> {
        let mut guard = self.inner.lock().await;
        if !enabled {
            if let Some(srv) = guard.take() {
                let _ = srv.shutdown_tx.send(());
            }
            return Ok(());
        }

        let effective_port = if port < 1024 { 17890 } else { port };
        let clean_token = token.trim().to_string();

        if let Some(existing) = guard.as_ref() {
            if existing.port == effective_port && existing.token == clean_token {
                return Ok(());
            }
        }

        if let Some(srv) = guard.take() {
            let _ = srv.shutdown_tx.send(());
        }

        let state = LocalApiState {
            db,
            engine,
            token: clean_token.clone(),
        };
        let app = build_router(state);
        let listener = TcpListener::bind(("127.0.0.1", effective_port))
            .await
            .map_err(|e| format!("绑定 127.0.0.1:{} 失败: {}", effective_port, e))?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await;
        });

        *guard = Some(RunningServer {
            port: effective_port,
            token: clean_token,
            shutdown_tx,
        });
        Ok(())
    }
}

pub fn build_router(state: LocalApiState) -> Router {
    Router::new()
        .route("/api/status", get(handle_status))
        .route("/api/messages", get(handle_messages))
        .route("/api/board", get(handle_board))
        .route("/api/control/start", post(handle_start))
        .route("/api/control/pause", post(handle_pause))
        .route("/api/control/step", post(handle_step))
        .route("/api/intervene/event", post(handle_intervene_event))
        .route("/api/intervene/whisper", post(handle_intervene_whisper))
        .route("/api/intervene/speak", post(handle_intervene_speak))
        .route("/api/intervene/private_room", post(handle_private_room))
        .route("/api/cards/draw", post(handle_draw_cards))
        .route("/ws", get(handle_ws))
        .with_state(state)
}

fn ok_json<T: Serialize>(data: T) -> Response {
    (
        StatusCode::OK,
        Json(json!({ "ok": true, "data": data })),
    )
        .into_response()
}

fn err_json(status: StatusCode, msg: impl Into<String>) -> Response {
    (
        status,
        Json(json!({ "ok": false, "error": msg.into() })),
    )
        .into_response()
}

fn check_auth(
    state: &LocalApiState,
    headers: &HeaderMap,
    query_token: Option<&str>,
) -> Result<(), Response> {
    if state.token.is_empty() {
        return Ok(());
    }
    if let Some(qt) = query_token {
        if qt == state.token {
            return Ok(());
        }
    }
    if let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(val) = auth.to_str() {
            if let Some(bearer) = val.strip_prefix("Bearer ").or_else(|| val.strip_prefix("bearer ")) {
                if bearer.trim() == state.token {
                    return Ok(());
                }
            }
        }
    }
    Err(err_json(StatusCode::UNAUTHORIZED, "Unauthorized: invalid or missing token"))
}

#[derive(Deserialize, Default)]
struct TokenQuery {
    token: Option<String>,
}

#[derive(Deserialize, Default)]
struct MessagesQuery {
    token: Option<String>,
    limit: Option<usize>,
    include_hidden: Option<bool>,
}

#[derive(Deserialize, Default)]
struct StepBody {
    agent_id: Option<String>,
}

#[derive(Deserialize)]
struct EventBody {
    content: String,
}

#[derive(Deserialize)]
struct WhisperBody {
    agent_id: String,
    content: String,
}

#[derive(Deserialize)]
struct SpeakBody {
    #[serde(default)]
    sender_name: String,
    content: String,
}

#[derive(Deserialize)]
struct PrivateRoomBody {
    agent_ids: Vec<String>,
    #[serde(default = "default_room_turns")]
    turns: u32,
}

fn default_room_turns() -> u32 {
    3
}

#[derive(Deserialize, Default)]
struct DrawCardsBody {
    inject_index: Option<usize>,
    content: Option<String>,
    kind: Option<String>,
}

async fn handle_status(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(q): Query<TokenQuery>,
) -> Response {
    if let Err(resp) = check_auth(&state, &headers, q.token.as_deref()) {
        return resp;
    }
    let exp_id = state.engine.active_experiment_id.read().await.clone();
    let (id, title, config, total_turns, _) = match state.db.get_experiment_status_light(&exp_id) {
        Ok(h) => h,
        Err(e) => return err_json(StatusCode::NOT_FOUND, e.to_string()),
    };
    let blind = state.engine.is_blind_guess_mode();
    let agents: Vec<Value> = config
        .agents
        .iter()
        .map(|a| {
            let mut obj = json!({
                "id": a.id,
                "label": a.label,
                "modelName": a.model_name,
            });
            if !blind {
                obj["goalAchieved"] = json!(a.goal_achieved);
                obj["secretGoal"] = json!(a.secret_goal);
            }
            obj
        })
        .collect();

    ok_json(json!({
        "experimentId": id,
        "title": title,
        "state": state.engine.current_state(),
        "totalTurns": total_turns,
        "targetTotalTurns": if config.enable_narrative_arc { Some(config.target_total_turns) } else { None },
        "blindGuessMode": blind,
        "agents": agents,
    }))
}

async fn handle_messages(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(q): Query<MessagesQuery>,
) -> Response {
    if let Err(resp) = check_auth(&state, &headers, q.token.as_deref()) {
        return resp;
    }
    let exp_id = state.engine.active_experiment_id.read().await.clone();
    let (_, _, config, _, _) = match state.db.get_experiment_status_light(&exp_id) {
        Ok(h) => h,
        Err(e) => return err_json(StatusCode::NOT_FOUND, e.to_string()),
    };
    let blind = state.engine.is_blind_guess_mode();
    let include_hidden = q.include_hidden.unwrap_or(false);
    let hide_secrets = blind && !include_hidden;
    let limit = q.limit.unwrap_or(50).clamp(1, 200);

    let messages = match state.db.get_recent_messages(&exp_id, limit, hide_secrets) {
        Ok(m) => m,
        Err(e) => return err_json(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    let filtered: Vec<Value> = messages
        .iter()
        .map(|m| {
            let speaker = m
                .agent_id
                .as_deref()
                .map(|id| config.agent_label(Some(id)));
            json!({
                "id": m.id,
                "seq": m.seq,
                "turnNumber": m.turn_number,
                "role": m.role,
                "agentId": m.agent_id,
                "speaker": speaker,
                "roomId": m.room_id,
                "content": m.content,
                "reasoningContent": if hide_secrets { "" } else { m.reasoning_content.as_str() },
                "microCue": m.micro_cue,
                "createdAt": m.created_at,
            })
        })
        .collect();

    ok_json(filtered)
}

async fn handle_board(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(q): Query<TokenQuery>,
) -> Response {
    if let Err(resp) = check_auth(&state, &headers, q.token.as_deref()) {
        return resp;
    }
    let exp_id = state.engine.active_experiment_id.read().await.clone();
    match state.db.get_experiment_status_light(&exp_id) {
        Ok((_, _, _, _, board_content)) => ok_json(json!({ "boardContent": board_content })),
        Err(e) => err_json(StatusCode::NOT_FOUND, e.to_string()),
    }
}

async fn handle_start(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(q): Query<TokenQuery>,
) -> Response {
    if let Err(resp) = check_auth(&state, &headers, q.token.as_deref()) {
        return resp;
    }
    state.engine.start().await;
    ok_json(json!({ "state": "running" }))
}

async fn handle_pause(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(q): Query<TokenQuery>,
) -> Response {
    if let Err(resp) = check_auth(&state, &headers, q.token.as_deref()) {
        return resp;
    }
    state.engine.pause(None).await;
    ok_json(json!({ "state": "paused" }))
}

async fn handle_step(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(q): Query<TokenQuery>,
    body: Option<Json<StepBody>>,
) -> Response {
    if let Err(resp) = check_auth(&state, &headers, q.token.as_deref()) {
        return resp;
    }
    let target = body.and_then(|b| b.0.agent_id).filter(|s| !s.trim().is_empty());
    state.engine.step_once(target).await;
    ok_json(json!({ "state": "stepping" }))
}

async fn handle_intervene_event(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(q): Query<TokenQuery>,
    Json(body): Json<EventBody>,
) -> Response {
    if let Err(resp) = check_auth(&state, &headers, q.token.as_deref()) {
        return resp;
    }
    if body.content.trim().is_empty() {
        return err_json(StatusCode::BAD_REQUEST, "content cannot be empty");
    }
    let exp_id = state.engine.active_experiment_id.read().await.clone();
    state
        .engine
        .enqueue_intervention(
            exp_id,
            InterventionInput {
                kind: SpeakerRole::GodSystem,
                agent_id: None,
                content: body.content,
            },
        )
        .await;
    ok_json(json!({ "queued": true }))
}

async fn handle_intervene_whisper(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(q): Query<TokenQuery>,
    Json(body): Json<WhisperBody>,
) -> Response {
    if let Err(resp) = check_auth(&state, &headers, q.token.as_deref()) {
        return resp;
    }
    if body.agent_id.trim().is_empty() || body.content.trim().is_empty() {
        return err_json(StatusCode::BAD_REQUEST, "agent_id and content are required");
    }
    let exp_id = state.engine.active_experiment_id.read().await.clone();
    state
        .engine
        .enqueue_intervention(
            exp_id,
            InterventionInput {
                kind: SpeakerRole::GodWhisper,
                agent_id: Some(body.agent_id),
                content: body.content,
            },
        )
        .await;
    ok_json(json!({ "queued": true }))
}

async fn handle_intervene_speak(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(q): Query<TokenQuery>,
    Json(body): Json<SpeakBody>,
) -> Response {
    if let Err(resp) = check_auth(&state, &headers, q.token.as_deref()) {
        return resp;
    }
    if body.content.trim().is_empty() {
        return err_json(StatusCode::BAD_REQUEST, "content cannot be empty");
    }
    let exp_id = state.engine.active_experiment_id.read().await.clone();
    let text = if body.sender_name.trim().is_empty() {
        body.content.trim().to_string()
    } else {
        format!("【{}】{}", body.sender_name.trim(), body.content.trim())
    };
    state
        .engine
        .enqueue_intervention(
            exp_id,
            InterventionInput {
                kind: SpeakerRole::GodPublic,
                agent_id: None,
                content: text,
            },
        )
        .await;
    ok_json(json!({ "queued": true }))
}

async fn handle_private_room(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(q): Query<TokenQuery>,
    Json(body): Json<PrivateRoomBody>,
) -> Response {
    if let Err(resp) = check_auth(&state, &headers, q.token.as_deref()) {
        return resp;
    }
    let exp_id = state.engine.active_experiment_id.read().await.clone();
    match state
        .engine
        .start_private_room(&exp_id, body.agent_ids, Some(body.turns))
        .await
    {
        Ok(room) => ok_json(room),
        Err(e) => err_json(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

async fn handle_draw_cards(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(q): Query<TokenQuery>,
    body: Option<Json<DrawCardsBody>>,
) -> Response {
    if let Err(resp) = check_auth(&state, &headers, q.token.as_deref()) {
        return resp;
    }
    let exp_id = state.engine.active_experiment_id.read().await.clone();
    let req = body.map(|b| b.0).unwrap_or_default();

    if let Some(custom_content) = req.content.filter(|s| !s.trim().is_empty()) {
        let kind = if req.kind.as_deref() == Some("GodPublic") {
            SpeakerRole::GodPublic
        } else {
            SpeakerRole::GodSystem
        };
        state
            .engine
            .enqueue_intervention(
                exp_id,
                InterventionInput {
                    kind,
                    agent_id: None,
                    content: custom_content,
                },
            )
            .await;
        return ok_json(json!({ "injected": true }));
    }

    let detail = match state.db.get_experiment_detail(&exp_id) {
        Ok(d) => d,
        Err(e) => return err_json(StatusCode::NOT_FOUND, e.to_string()),
    };

    let slice_start = detail.messages.len().saturating_sub(24);
    let recent = &detail.messages[slice_start..];
    match run_observer_analysis(
        &state.engine.client,
        &detail.config,
        recent,
        &detail.board_content,
        &detail.compacted_summary,
    )
    .await
    {
        Ok(parsed) => {
            let cards: Vec<CrisisCard> = parsed.crisis_cards.clone();
            let note = ObserverNote {
                id: Uuid::new_v4().to_string(),
                experiment_id: exp_id.clone(),
                trigger_turn: detail.total_turns,
                tags: parsed.tags,
                depth_stage: parsed.depth_stage,
                narrative_act: parsed.narrative_act,
                note: parsed.note,
                tension_score: parsed.tension_score.clamp(0, 100) as u8,
                abstraction_score: parsed.abstraction_score.clamp(0, 100) as u8,
                stagnation_index: parsed.stagnation_index.clamp(0, 100) as u8,
                perturbation_injected: false,
                crisis_cards: cards.clone(),
                relationship_matrix: parsed
                    .relationship_matrix
                    .into_iter()
                    .map(|mut edge| {
                        if let Some(ag) = detail.config.find_agent(&edge.from) {
                            edge.from = ag.label.clone();
                        }
                        if let Some(ag) = detail.config.find_agent(&edge.to) {
                            edge.to = ag.label.clone();
                        }
                        edge
                    })
                    .collect(),
                created_at: Utc::now().to_rfc3339(),
            };
            if state.db.append_observer_note(&note).is_ok() {
                state
                    .engine
                    .emit_event(StreamEvent::ObserverCompleted {
                        experiment_id: exp_id.clone(),
                        note,
                    })
                    .await;
            }
            if let Some(idx) = req.inject_index {
                if let Some(card) = cards.get(idx) {
                    let kind = if card.kind == "GodPublic" {
                        SpeakerRole::GodPublic
                    } else {
                        SpeakerRole::GodSystem
                    };
                    state
                        .engine
                        .enqueue_intervention(
                            exp_id,
                            InterventionInput {
                                kind,
                                agent_id: None,
                                content: card.content.clone(),
                            },
                        )
                        .await;
                }
            }
            ok_json(json!({ "cards": cards }))
        }
        Err(e) => err_json(StatusCode::BAD_GATEWAY, e.to_string()),
    }
}

async fn handle_ws(
    ws: WebSocketUpgrade,
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(q): Query<TokenQuery>,
) -> Response {
    if let Err(resp) = check_auth(&state, &headers, q.token.as_deref()) {
        return resp;
    }
    ws.on_upgrade(|socket| ws_loop(socket, state))
}

async fn resolve_ws_speaker(
    state: &LocalApiState,
    cache: &Mutex<HashMap<(String, String), String>>,
    exp_id: &str,
    agent_id: Option<&str>,
) -> String {
    let Some(aid) = agent_id else {
        return "系统".to_string();
    };
    let mut guard = cache.lock().await;
    if let Some(lbl) = guard.get(&(exp_id.to_string(), aid.to_string())) {
        return lbl.clone();
    }
    if let Ok((_, _, cfg, _, _)) = state.db.get_experiment_status_light(exp_id) {
        for a in &cfg.agents {
            guard.insert((exp_id.to_string(), a.id.clone()), a.label.clone());
        }
        return cfg.agent_label(Some(aid));
    }
    aid.to_string()
}

async fn ws_loop(mut socket: WebSocket, state: LocalApiState) {
    let mut rx = state.engine.subscribe_events();
    let label_cache: Mutex<HashMap<(String, String), String>> = Mutex::new(HashMap::new());

    loop {
        tokio::select! {
            recv = rx.recv() => {
                let Ok(ev) = recv else { continue; };
                let blind = state.engine.is_blind_guess_mode();
                let payload = match ev {
                    StreamEvent::StreamEnd { message, .. } => {
                        if blind && (message.room_id != "main" || message.role == SpeakerRole::GodWhisper) {
                            None
                        } else {
                            let speaker = resolve_ws_speaker(
                                &state,
                                &label_cache,
                                &message.experiment_id,
                                message.agent_id.as_deref(),
                            )
                            .await;
                            Some(json!({
                                "type": "message_appended",
                                "data": {
                                    "speaker": speaker,
                                    "content": message.content,
                                    "microCue": message.micro_cue,
                                    "isPrivateRoom": message.room_id != "main",
                                    "turnNumber": message.turn_number,
                                }
                            }))
                        }
                    }
                    StreamEvent::MessageAdded { message, .. } => {
                        if blind && (message.room_id != "main" || message.role == SpeakerRole::GodWhisper) {
                            None
                        } else {
                            let speaker = resolve_ws_speaker(
                                &state,
                                &label_cache,
                                &message.experiment_id,
                                message.agent_id.as_deref(),
                            )
                            .await;
                            Some(json!({
                                "type": "message_appended",
                                "data": {
                                    "speaker": speaker,
                                    "content": message.content,
                                    "microCue": message.micro_cue,
                                    "isPrivateRoom": message.room_id != "main",
                                    "turnNumber": message.turn_number,
                                }
                            }))
                        }
                    }
                    StreamEvent::BoardUpdated { board_content, actor_label, reason, .. } => {
                        Some(json!({
                            "type": "board_updated",
                            "data": {
                                "boardContent": board_content,
                                "actorLabel": actor_label,
                                "reason": reason,
                            }
                        }))
                    }
                    StreamEvent::ObserverCompleted { note, .. } => {
                        Some(json!({
                            "type": "observer_report",
                            "data": note,
                        }))
                    }
                    StreamEvent::StateChanged { state: engine_state, reason, consecutive_turns, .. } => {
                        Some(json!({
                            "type": "status_changed",
                            "data": {
                                "state": engine_state,
                                "reason": reason,
                                "consecutiveTurns": consecutive_turns,
                            }
                        }))
                    }
                    StreamEvent::SecretGoalAchieved { agent_id, agent_label, secret_goal, .. } => {
                        Some(json!({
                            "type": "status_changed",
                            "data": {
                                "event": "secret_goal_achieved",
                                "agentId": agent_id,
                                "agentLabel": agent_label,
                                "secretGoal": if blind { "" } else { secret_goal.as_str() },
                            }
                        }))
                    }
                    StreamEvent::NarrativeFinaleReached { total_turns, target_turns, .. } => {
                        Some(json!({
                            "type": "status_changed",
                            "data": {
                                "event": "narrative_finale_reached",
                                "totalTurns": total_turns,
                                "targetTurns": target_turns,
                            }
                        }))
                    }
                    _ => None,
                };
                if let Some(p) = payload {
                    if socket.send(Message::Text(p.to_string())).await.is_err() {
                        break;
                    }
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_local_api_auth_and_status_board() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let exp = db.ensure_default_experiment().unwrap();
        let engine = TerrariumEngine::new(Arc::clone(&db), exp.id.clone());

        let state = LocalApiState {
            db,
            engine,
            token: "secret-123".to_string(),
        };
        let app = build_router(state);
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let client = reqwest::Client::new();
        // Unauthorized without token
        let r_unauth = client
            .get(format!("http://{}/api/status", addr))
            .send()
            .await
            .unwrap();
        assert_eq!(r_unauth.status(), reqwest::StatusCode::UNAUTHORIZED);

        // Authorized with Bearer token
        let r_ok = client
            .get(format!("http://{}/api/status", addr))
            .bearer_auth("secret-123")
            .send()
            .await
            .unwrap();
        assert_eq!(r_ok.status(), reqwest::StatusCode::OK);
        let body: Value = r_ok.json().await.unwrap();
        assert_eq!(body["ok"], true);
        assert_eq!(body["data"]["experimentId"], exp.id);

        // Board endpoint via query token
        let r_board = client
            .get(format!("http://{}/api/board?token=secret-123", addr))
            .send()
            .await
            .unwrap();
        assert_eq!(r_board.status(), reqwest::StatusCode::OK);
        let b_json: Value = r_board.json().await.unwrap();
        assert_eq!(b_json["ok"], true);
    }
}
