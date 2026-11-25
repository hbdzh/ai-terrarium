mod compactor;
mod db;
mod engine;
mod llm;
mod models;
mod observer;
mod sandbox_tools;
mod sanitizer;
mod snapshot;

use std::sync::Arc;

use chrono::Utc;
use tauri::ipc::Channel;
use tauri::{Manager, State};
use uuid::Uuid;

use crate::db::Database;
use crate::engine::TerrariumEngine;
use crate::models::{
    ActivePrivateRoom, BoardApprovalMode, CharacterCard, EngineState, ExperimentConfig,
    ExperimentDetail, ExperimentSummary, InterventionInput, PresetTemplate, ScenarioPackage,
    StreamEvent, TurnSnapshot,
};
use crate::observer::GeneratedScenario;
use crate::sandbox_tools::ApprovalDecision;

struct AppState {
    db: Arc<Database>,
    engine: Arc<TerrariumEngine>,
}

#[tauri::command]
fn get_core_engine_prompt(state: State<'_, AppState>) -> Result<String, String> {
    state.db.get_core_engine_prompt().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_core_engine_prompt(
    state: State<'_, AppState>,
    prompt: String,
) -> Result<String, String> {
    state
        .db
        .save_core_engine_prompt(&prompt)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn reset_core_engine_prompt(state: State<'_, AppState>) -> Result<String, String> {
    state
        .db
        .reset_core_engine_prompt()
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_presets(state: State<'_, AppState>) -> Result<Vec<PresetTemplate>, String> {
    state.db.list_presets().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_preset(
    state: State<'_, AppState>,
    preset: PresetTemplate,
) -> Result<Vec<PresetTemplate>, String> {
    state.db.save_preset(&preset).map_err(|e| e.to_string())
}

#[tauri::command]
fn reset_builtin_preset(
    state: State<'_, AppState>,
    preset_id: String,
) -> Result<Vec<PresetTemplate>, String> {
    state
        .db
        .reset_builtin_preset(&preset_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_preset(
    state: State<'_, AppState>,
    preset_id: String,
) -> Result<Vec<PresetTemplate>, String> {
    state.db.delete_preset(&preset_id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn register_stream_channel(
    state: State<'_, AppState>,
    channel: Channel<StreamEvent>,
) -> Result<(), String> {
    state.engine.set_channel(channel).await;
    Ok(())
}

#[tauri::command]
fn list_experiments(state: State<'_, AppState>) -> Result<Vec<ExperimentSummary>, String> {
    state.db.list_experiments().map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_active_experiment(state: State<'_, AppState>) -> Result<ExperimentDetail, String> {
    let active_id = state.engine.active_experiment_id.read().await.clone();
    state
        .db
        .get_experiment_detail(&active_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_experiment(
    state: State<'_, AppState>,
    title: String,
    config: ExperimentConfig,
) -> Result<ExperimentDetail, String> {
    let detail = state
        .db
        .create_experiment(title.trim(), &config)
        .map_err(|e| e.to_string())?;
    state.engine.switch_experiment(detail.id.clone()).await;
    Ok(detail)
}

#[tauri::command]
async fn switch_experiment(
    state: State<'_, AppState>,
    experiment_id: String,
) -> Result<ExperimentDetail, String> {
    let detail = state
        .db
        .get_experiment_detail(&experiment_id)
        .map_err(|e| e.to_string())?;
    state.engine.switch_experiment(experiment_id).await;
    Ok(detail)
}

#[tauri::command]
fn update_experiment(
    state: State<'_, AppState>,
    experiment_id: String,
    title: String,
    config: ExperimentConfig,
) -> Result<ExperimentDetail, String> {
    state
        .db
        .update_experiment(&experiment_id, title.trim(), &config)
        .map_err(|e| e.to_string())?;
    state
        .db
        .get_experiment_detail(&experiment_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_experiment(
    state: State<'_, AppState>,
    experiment_id: String,
) -> Result<ExperimentDetail, String> {
    let active_id = state.engine.active_experiment_id.read().await.clone();
    if active_id == experiment_id {
        state.engine.pause(None).await;
    }
    state
        .db
        .delete_experiment(&experiment_id)
        .map_err(|e| e.to_string())?;
    let next = state
        .db
        .ensure_default_experiment()
        .map_err(|e| e.to_string())?;
    if active_id == experiment_id {
        state.engine.switch_experiment(next.id.clone()).await;
    }
    Ok(next)
}

#[tauri::command]
async fn start_engine(state: State<'_, AppState>) -> Result<(), String> {
    state.engine.start().await;
    Ok(())
}

#[tauri::command]
async fn pause_engine(state: State<'_, AppState>) -> Result<(), String> {
    state.engine.pause(None).await;
    Ok(())
}

#[tauri::command]
async fn step_engine(
    state: State<'_, AppState>,
    target_agent_id: Option<String>,
) -> Result<(), String> {
    state.engine.step_once(target_agent_id).await;
    Ok(())
}

#[tauri::command]
fn get_engine_state(state: State<'_, AppState>) -> EngineState {
    state.engine.current_state()
}

#[tauri::command]
async fn enqueue_intervention(
    state: State<'_, AppState>,
    experiment_id: String,
    input: InterventionInput,
) -> Result<usize, String> {
    let len = state
        .engine
        .enqueue_intervention(experiment_id, input)
        .await;
    Ok(len)
}

#[tauri::command]
async fn trigger_observer_now(
    state: State<'_, AppState>,
    experiment_id: String,
) -> Result<(), String> {
    state.engine.trigger_observer_now(experiment_id).await;
    Ok(())
}

#[tauri::command]
fn get_message_raw_prompt(
    state: State<'_, AppState>,
    message_id: String,
) -> Result<Option<String>, String> {
    state
        .db
        .get_message_raw_prompt(&message_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn export_experiment_markdown(
    state: State<'_, AppState>,
    experiment_id: String,
) -> Result<String, String> {
    state
        .db
        .export_markdown(&experiment_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn save_markdown_to_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
async fn fork_experiment(
    state: State<'_, AppState>,
    source_exp_id: String,
    target_turn: u32,
    edit_message_id: Option<String>,
    edit_content: Option<String>,
) -> Result<ExperimentDetail, String> {
    let edit_override = match (edit_message_id, edit_content) {
        (Some(id), Some(content)) if !content.trim().is_empty() => Some((id, content)),
        _ => None,
    };
    let forked = crate::snapshot::fork_experiment_at_turn(
        &state.db,
        &source_exp_id,
        target_turn,
        edit_override,
    )
    .map_err(|e| e.to_string())?;
    state.engine.switch_experiment(forked.id.clone()).await;
    Ok(forked)
}

#[tauri::command]
fn get_experiment_snapshots(
    state: State<'_, AppState>,
    experiment_id: String,
) -> Result<Vec<TurnSnapshot>, String> {
    state
        .db
        .get_turn_snapshots(&experiment_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_shared_board(
    state: State<'_, AppState>,
    experiment_id: String,
    content: String,
) -> Result<(String, String), String> {
    let (prev, saved) = state
        .db
        .update_shared_board(&experiment_id, &content)
        .map_err(|e| e.to_string())?;
    state
        .engine
        .emit_event(StreamEvent::BoardUpdated {
            experiment_id,
            board_content: saved.clone(),
            board_prev_content: prev.clone(),
            reason: "造物主手动编辑".to_string(),
            actor_label: "造物主".to_string(),
        })
        .await;
    Ok((prev, saved))
}

#[tauri::command]
fn set_board_approval_mode(
    state: State<'_, AppState>,
    experiment_id: String,
    mode: BoardApprovalMode,
) -> Result<ExperimentConfig, String> {
    state
        .db
        .set_board_approval_mode(&experiment_id, mode)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn resolve_board_approval(
    state: State<'_, AppState>,
    request_id: String,
    approved: bool,
    modified_content: Option<String>,
    reject_reason: Option<String>,
) -> Result<bool, String> {
    let decision = if !approved {
        ApprovalDecision::Reject {
            reason: reject_reason,
        }
    } else if let Some(edited) = modified_content {
        ApprovalDecision::ModifyAndApprove {
            new_content: edited,
        }
    } else {
        ApprovalDecision::Approve
    };
    Ok(state
        .engine
        .resolve_board_approval(&request_id, decision)
        .await)
}

#[tauri::command]
async fn generate_final_report(
    state: State<'_, AppState>,
    experiment_id: String,
) -> Result<String, String> {
    let detail = state
        .db
        .get_experiment_detail(&experiment_id)
        .map_err(|e| e.to_string())?;
    let report = crate::observer::generate_final_report_markdown(&state.engine.client, &detail)
        .await
        .map_err(|e| e.to_string())?;
    state
        .db
        .save_final_report(&experiment_id, &report)
        .map_err(|e| e.to_string())?;
    Ok(report)
}

#[tauri::command]
fn export_scenario_package(
    state: State<'_, AppState>,
    experiment_id: String,
    include_keys: bool,
) -> Result<ScenarioPackage, String> {
    let detail = state
        .db
        .get_experiment_detail(&experiment_id)
        .map_err(|e| e.to_string())?;
    let mut cfg = detail.config;
    if !include_keys {
        for a in &mut cfg.agents {
            a.api_key.clear();
        }
        cfg.node_c.api_key.clear();
    }
    let board = if !detail.board_content.trim().is_empty() {
        detail.board_content
    } else {
        cfg.initial_board.clone()
    };
    cfg.initial_board = board.clone();
    Ok(ScenarioPackage {
        version: "2.0".to_string(),
        title: detail.title,
        config: cfg,
        initial_board: board,
    })
}

#[tauri::command]
async fn import_scenario_package(
    state: State<'_, AppState>,
    mut pkg: ScenarioPackage,
) -> Result<ExperimentDetail, String> {
    if !pkg.initial_board.trim().is_empty() && pkg.config.initial_board.trim().is_empty() {
        pkg.config.initial_board = pkg.initial_board;
    }
    // 若导入剧本中未附带 API Key，自动复用当前激活实验的同顺序节点或首节点 Key 方便直接开局
    let active_id = state.engine.active_experiment_id.read().await.clone();
    if let Ok(active) = state.db.get_experiment_detail(&active_id) {
        let fallback_key = active
            .config
            .agents
            .first()
            .map(|a| a.api_key.clone())
            .unwrap_or_default();
        for (idx, a) in pkg.config.agents.iter_mut().enumerate() {
            if a.api_key.trim().is_empty() {
                a.api_key = active
                    .config
                    .agents
                    .get(idx)
                    .map(|x| x.api_key.clone())
                    .filter(|k| !k.trim().is_empty())
                    .unwrap_or_else(|| fallback_key.clone());
            }
        }
        if pkg.config.node_c.api_key.trim().is_empty() {
            pkg.config.node_c.api_key = if !active.config.node_c.api_key.trim().is_empty() {
                active.config.node_c.api_key
            } else {
                fallback_key
            };
        }
    }
    let title = if pkg.title.trim().is_empty() {
        "导入剧本实验".to_string()
    } else {
        pkg.title.trim().to_string()
    };
    let created = state
        .db
        .create_experiment(&title, &pkg.config)
        .map_err(|e| e.to_string())?;
    state.engine.switch_experiment(created.id.clone()).await;
    Ok(created)
}

#[tauri::command]
async fn generate_scenario_from_idea(
    state: State<'_, AppState>,
    idea: String,
    base_config: Option<ExperimentConfig>,
) -> Result<GeneratedScenario, String> {
    if idea.trim().is_empty() {
        return Err("请输入一句话脑洞或场景构想".to_string());
    }
    let cfg = if let Some(c) = base_config {
        c
    } else {
        let active_id = state.engine.active_experiment_id.read().await.clone();
        state
            .db
            .get_experiment_detail(&active_id)
            .map(|d| d.config)
            .map_err(|e| e.to_string())?
    };
    crate::observer::generate_scenario_from_idea(&state.engine.client, &cfg, &idea)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn execute_director_command(
    state: State<'_, AppState>,
    experiment_id: String,
    instruction: String,
) -> Result<String, String> {
    if instruction.trim().is_empty() {
        return Err("请输入导演遥控指令".to_string());
    }
    state
        .engine
        .execute_director_command(&experiment_id, &instruction)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_private_room(
    state: State<'_, AppState>,
    experiment_id: String,
    member_ids: Vec<String>,
    turns: Option<u32>,
) -> Result<ActivePrivateRoom, String> {
    state
        .engine
        .start_private_room(&experiment_id, member_ids, turns)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn close_private_room(
    state: State<'_, AppState>,
    experiment_id: String,
) -> Result<(), String> {
    state
        .engine
        .close_private_room(&experiment_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_character_cards(state: State<'_, AppState>) -> Result<Vec<CharacterCard>, String> {
    state.db.list_character_cards().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_agent_to_character_library(
    state: State<'_, AppState>,
    experiment_id: String,
    agent_id: String,
) -> Result<Vec<CharacterCard>, String> {
    let detail = state
        .db
        .get_experiment_detail(&experiment_id)
        .map_err(|e| e.to_string())?;
    let agent = detail
        .config
        .find_agent(&agent_id)
        .ok_or_else(|| "未找到指定智能体".to_string())?;
    let traits = crate::observer::extract_agent_traits_from_report(&detail.final_report, &agent.label);
    let card = CharacterCard {
        id: Uuid::new_v4().to_string(),
        name: agent.label.clone(),
        base_prompt: agent.system_prompt.clone(),
        behavior_traits: traits,
        source_experiment_title: detail.title,
        created_at: Utc::now().to_rfc3339(),
    };
    state
        .db
        .save_character_card(&card)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_character_card(
    state: State<'_, AppState>,
    card_id: String,
) -> Result<Vec<CharacterCard>, String> {
    state
        .db
        .delete_character_card(&card_id)
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            let db_path = app_dir.join("ai_terrarium.db");
            let db = Arc::new(Database::open(&db_path).expect("failed to init sqlite db"));
            let default_exp = db
                .ensure_default_experiment()
                .expect("failed to ensure default experiment");
            let engine = TerrariumEngine::new(Arc::clone(&db), default_exp.id);

            app.manage(AppState { db, engine });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_core_engine_prompt,
            save_core_engine_prompt,
            reset_core_engine_prompt,
            get_presets,
            save_preset,
            reset_builtin_preset,
            delete_preset,
            register_stream_channel,
            list_experiments,
            get_active_experiment,
            create_experiment,
            switch_experiment,
            update_experiment,
            delete_experiment,
            start_engine,
            pause_engine,
            step_engine,
            get_engine_state,
            enqueue_intervention,
            trigger_observer_now,
            export_experiment_markdown,
            get_message_raw_prompt,
            save_markdown_to_file,
            fork_experiment,
            get_experiment_snapshots,
            update_shared_board,
            set_board_approval_mode,
            resolve_board_approval,
            generate_final_report,
            export_scenario_package,
            import_scenario_package,
            generate_scenario_from_idea,
            execute_director_command,
            start_private_room,
            close_private_room,
            list_character_cards,
            save_agent_to_character_library,
            delete_character_card,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
