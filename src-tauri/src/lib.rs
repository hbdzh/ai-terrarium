mod compactor;
mod db;
mod engine;
mod llm;
mod local_api;
mod models;
mod observer;
mod sandbox_tools;
mod sanitizer;
mod snapshot;
#[cfg(target_env = "ohos")]
mod ohos_compat;
#[cfg(target_os = "windows")]
mod win_chrome;

use std::sync::Arc;

use chrono::Utc;
use tauri::ipc::Channel;
use tauri::{Manager, State};
use uuid::Uuid;

use crate::db::Database;
use crate::engine::TerrariumEngine;
use crate::local_api::LocalApiManager;
use crate::models::{
    ActivePrivateRoom, BoardApprovalMode, CharacterCard, DiffLine, EngineState, ExperimentConfig,
    ExperimentDetail, ExperimentSummary, InterventionInput, PresetTemplate, ScenarioPackage,
    StreamEvent, UiPrefs,
};
use crate::observer::GeneratedScenario;
use crate::sandbox_tools::ApprovalDecision;

struct AppState {
    db: Arc<Database>,
    engine: Arc<TerrariumEngine>,
    local_api: Arc<LocalApiManager>,
}

#[tauri::command]
fn get_core_engine_prompt(state: State<'_, AppState>) -> Result<String, String> {
    state.db.get_core_engine_prompt().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_core_engine_prompt(state: State<'_, AppState>, prompt: String) -> Result<String, String> {
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
fn delete_preset(
    state: State<'_, AppState>,
    preset_id: String,
) -> Result<Vec<PresetTemplate>, String> {
    state
        .db
        .delete_preset(&preset_id)
        .map_err(|e| e.to_string())
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
    config.validate_unique_agent_labels()?;
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
    config.validate_unique_agent_labels()?;
    state
        .db
        .update_experiment(&experiment_id, title.trim(), &config)
        .map_err(|e| e.to_string())?;
    state.engine.bump_state_version();
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
fn export_pure_script_markdown(
    state: State<'_, AppState>,
    experiment_id: String,
) -> Result<String, String> {
    state
        .db
        .export_pure_script_markdown(&experiment_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn save_binary_to_file(
    app: tauri::AppHandle,
    path: String,
    data: Vec<u8>,
) -> Result<(), String> {
    #[cfg(target_env = "ohos")]
    {
        let _ = app;
        let normalized = path.strip_prefix("file://").unwrap_or(&path);
        return std::fs::write(normalized, &data).map_err(|e| e.to_string());
    }
    #[cfg(not(target_env = "ohos"))]
    {
        use std::io::Write;
        use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};

        let file_path = path
            .parse::<FilePath>()
            .unwrap_or_else(|_| FilePath::Path(std::path::PathBuf::from(&path)));

        let mut opts = OpenOptions::new();
        opts.write(true).create(true).truncate(true);

        let mut file = match app.fs().open(file_path.clone(), opts) {
            Ok(f) => f,
            Err(_) => {
                let mut fallback_opts = OpenOptions::new();
                fallback_opts.write(true).create(true);
                app.fs()
                    .open(file_path, fallback_opts)
                    .map_err(|e| e.to_string())?
            }
        };

        file.write_all(&data).map_err(|e| e.to_string())?;
        file.flush().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn save_markdown_to_file(
    app: tauri::AppHandle,
    path: String,
    content: String,
) -> Result<(), String> {
    #[cfg(target_env = "ohos")]
    {
        let _ = app;
        let normalized = path.strip_prefix("file://").unwrap_or(&path);
        return std::fs::write(normalized, content.as_bytes()).map_err(|e| e.to_string());
    }
    #[cfg(not(target_env = "ohos"))]
    {
        use std::io::Write;
        use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};

        let file_path = path
            .parse::<FilePath>()
            .unwrap_or_else(|_| FilePath::Path(std::path::PathBuf::from(&path)));

        let mut opts = OpenOptions::new();
        opts.write(true).create(true).truncate(true);

        let mut file = match app.fs().open(file_path.clone(), opts) {
            Ok(f) => f,
            Err(_) => {
                // Some Android ContentProviders reject "wt" mode; retry with "w"
                let mut fallback_opts = OpenOptions::new();
                fallback_opts.write(true).create(true);
                app.fs()
                    .open(file_path, fallback_opts)
                    .map_err(|e| e.to_string())?
            }
        };

        file.write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;
        file.flush().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn read_text_file(app: tauri::AppHandle, path: String) -> Result<String, String> {
    #[cfg(target_env = "ohos")]
    {
        let _ = app;
        let normalized = path.strip_prefix("file://").unwrap_or(&path);
        return std::fs::read_to_string(normalized).map_err(|e| e.to_string());
    }
    #[cfg(not(target_env = "ohos"))]
    {
        use std::io::Read;
        use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};

        let file_path = path
            .parse::<FilePath>()
            .unwrap_or_else(|_| FilePath::Path(std::path::PathBuf::from(&path)));

        let mut opts = OpenOptions::new();
        opts.read(true);
        let mut file = app.fs().open(file_path, opts).map_err(|e| e.to_string())?;
        let mut text = String::new();
        file.read_to_string(&mut text).map_err(|e| e.to_string())?;
        Ok(text)
    }
}

#[tauri::command]
fn compute_line_diff(old_text: String, new_text: String) -> Vec<DiffLine> {
    let a: Vec<&str> = old_text.lines().collect();
    let b: Vec<&str> = new_text.lines().collect();
    let m = a.len();
    let n = b.len();

    if m * n > 400_000 {
        let mut out = Vec::with_capacity(m + n);
        for line in a {
            out.push(DiffLine {
                kind: "removed".to_string(),
                text: line.to_string(),
            });
        }
        for line in b {
            out.push(DiffLine {
                kind: "added".to_string(),
                text: line.to_string(),
            });
        }
        return out;
    }

    let mut dp = vec![0usize; (m + 1) * (n + 1)];
    let cols = n + 1;
    for i in (0..m).rev() {
        for j in (0..n).rev() {
            dp[i * cols + j] = if a[i] == b[j] {
                dp[(i + 1) * cols + (j + 1)] + 1
            } else {
                dp[(i + 1) * cols + j].max(dp[i * cols + (j + 1)])
            };
        }
    }

    let mut out = Vec::with_capacity(m + n);
    let mut i = 0;
    let mut j = 0;
    while i < m && j < n {
        if a[i] == b[j] {
            out.push(DiffLine {
                kind: "same".to_string(),
                text: a[i].to_string(),
            });
            i += 1;
            j += 1;
        } else if dp[(i + 1) * cols + j] >= dp[i * cols + (j + 1)] {
            out.push(DiffLine {
                kind: "removed".to_string(),
                text: a[i].to_string(),
            });
            i += 1;
        } else {
            out.push(DiffLine {
                kind: "added".to_string(),
                text: b[j].to_string(),
            });
            j += 1;
        }
    }
    while i < m {
        out.push(DiffLine {
            kind: "removed".to_string(),
            text: a[i].to_string(),
        });
        i += 1;
    }
    while j < n {
        out.push(DiffLine {
            kind: "added".to_string(),
            text: b[j].to_string(),
        });
        j += 1;
    }
    out
}

#[tauri::command]
fn get_ui_prefs(state: State<'_, AppState>) -> Result<UiPrefs, String> {
    state.db.get_ui_prefs().map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_ui_prefs(state: State<'_, AppState>, prefs: UiPrefs) -> Result<(), String> {
    state.db.save_ui_prefs(&prefs).map_err(|e| e.to_string())?;
    state
        .local_api
        .sync_config(
            prefs.enable_local_api,
            prefs.local_api_port,
            prefs.local_api_token.clone(),
            Arc::clone(&state.db),
            Arc::clone(&state.engine),
        )
        .await?;
    Ok(())
}

#[tauri::command]
fn set_blind_guess_mode(state: State<'_, AppState>, enabled: bool) {
    state.engine.set_blind_guess_mode(enabled);
}

#[tauri::command]
fn get_api_profiles(state: State<'_, AppState>) -> Result<Vec<crate::models::ApiProfile>, String> {
    state.db.get_api_profiles().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_api_profiles(
    state: State<'_, AppState>,
    profiles: Vec<crate::models::ApiProfile>,
) -> Result<Vec<crate::models::ApiProfile>, String> {
    state
        .db
        .save_api_profiles(&profiles)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn fork_experiment(
    state: State<'_, AppState>,
    source_exp_id: String,
    target_turn: u32,
    target_message_id: Option<String>,
    edit_message_id: Option<String>,
    edit_content: Option<String>,
) -> Result<ExperimentDetail, String> {
    let edit_override = match (edit_message_id, edit_content) {
        (Some(id), Some(content)) if !content.trim().is_empty() => Some((id, content)),
        _ => None,
    };
    let forked = crate::snapshot::fork_experiment_at_turn_or_message(
        &state.db,
        &source_exp_id,
        target_turn,
        target_message_id,
        edit_override,
    )
    .map_err(|e| e.to_string())?;
    state.engine.switch_experiment(forked.id.clone()).await;
    Ok(forked)
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
            reason: "主持人手动编辑".to_string(),
            actor_label: "主持人".to_string(),
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
    let cfg = state
        .db
        .set_board_approval_mode(&experiment_id, mode)
        .map_err(|e| e.to_string())?;
    state.engine.bump_state_version();
    Ok(cfg)
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
    // 若导入剧本中未附带 API Key，优先使用全局 API 配置池，其次复用当前激活实验
    let global_profiles = state.db.get_api_profiles().unwrap_or_default();
    let default_profile = global_profiles.first();
    let active_id = state.engine.active_experiment_id.read().await.clone();
    let active_detail = state.db.get_experiment_detail(&active_id).ok();
    let fallback_key = default_profile
        .map(|p| p.api_key.clone())
        .filter(|k| !k.trim().is_empty())
        .or_else(|| {
            active_detail
                .as_ref()
                .and_then(|active| active.config.agents.first().map(|a| a.api_key.clone()))
        })
        .unwrap_or_default();
    for (idx, a) in pkg.config.agents.iter_mut().enumerate() {
        if let Some(pid) = a.api_profile_id.as_deref() {
            if let Some(prof) = global_profiles.iter().find(|p| p.id == pid) {
                a.base_url = prof.base_url.clone();
                a.api_key = prof.api_key.clone();
                a.model_name = prof.model_name.clone();
            }
        }
        if a.api_key.trim().is_empty() {
            a.api_key = active_detail
                .as_ref()
                .and_then(|active| active.config.agents.get(idx).map(|x| x.api_key.clone()))
                .filter(|k| !k.trim().is_empty())
                .unwrap_or_else(|| fallback_key.clone());
        }
    }
    if let Some(pid) = pkg.config.node_c.api_profile_id.as_deref() {
        if let Some(prof) = global_profiles.iter().find(|p| p.id == pid) {
            pkg.config.node_c.base_url = prof.base_url.clone();
            pkg.config.node_c.api_key = prof.api_key.clone();
            pkg.config.node_c.model_name = prof.model_name.clone();
        }
    }
    if pkg.config.node_c.api_key.trim().is_empty() {
        pkg.config.node_c.api_key = active_detail
            .as_ref()
            .map(|active| active.config.node_c.api_key.clone())
            .filter(|k| !k.trim().is_empty())
            .unwrap_or(fallback_key);
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
    label_override: Option<String>,
    prompt_override: Option<String>,
) -> Result<Vec<CharacterCard>, String> {
    let detail = state
        .db
        .get_experiment_detail(&experiment_id)
        .map_err(|e| e.to_string())?;
    let agent = detail.config.find_agent(&agent_id);
    let name = label_override
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| agent.map(|a| a.label.clone()))
        .ok_or_else(|| "未找到指定角色".to_string())?;
    let base_prompt = prompt_override
        .or_else(|| agent.map(|a| a.system_prompt.clone()))
        .unwrap_or_default();
    let traits = crate::observer::extract_agent_traits_from_report(&detail.final_report, &name);
    let card = CharacterCard {
        id: Uuid::new_v4().to_string(),
        name,
        base_prompt,
        behavior_traits: traits,
        memory_imprints: Vec::new(),
        source_experiment_title: detail.title,
        is_builtin: false,
        created_at: Utc::now().to_rfc3339(),
    };
    state
        .db
        .save_character_card(&card)
        .map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DistillImprintsResponse {
    cards: Vec<CharacterCard>,
    distilled: Vec<crate::observer::DistilledImprint>,
}

fn strip_disambiguation_suffix(label: &str) -> &str {
    let trimmed = label.trim();
    if let Some(idx) = trimmed.rfind(" (") {
        if trimmed.ends_with(')') {
            let num = &trimmed[idx + 2..trimmed.len() - 1];
            if !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()) {
                return trimmed[..idx].trim();
            }
        }
    }
    trimmed
}

#[tauri::command]
async fn distill_memory_imprints_to_library(
    state: State<'_, AppState>,
    experiment_id: String,
) -> Result<DistillImprintsResponse, String> {
    let detail = state
        .db
        .get_experiment_detail(&experiment_id)
        .map_err(|e| e.to_string())?;
    let distilled = crate::observer::distill_memory_imprints(&state.engine.client, &detail)
        .await
        .map_err(|e| e.to_string())?;

    let mut existing_cards = state
        .db
        .list_character_cards()
        .map_err(|e| e.to_string())?;

    for item in &distilled {
        let matched_agent = detail
            .config
            .agents
            .iter()
            .find(|a| !item.agent_id.is_empty() && a.id == item.agent_id)
            .or_else(|| {
                detail
                    .config
                    .agents
                    .iter()
                    .find(|a| !item.agent_label.is_empty() && a.label.trim() == item.agent_label)
            });

        let target_label = matched_agent
            .map(|a| a.label.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| item.agent_label.trim().to_string());
        if target_label.is_empty() {
            continue;
        }
        let base_label = strip_disambiguation_suffix(&target_label);

        if let Some(pos) = existing_cards.iter().position(|c| {
            let cn = c.name.trim();
            cn == target_label || cn == base_label
        }) {
            let mut card = existing_cards[pos].clone();
            if !card.memory_imprints.iter().any(|m| m.trim() == item.imprint) {
                card.memory_imprints.push(item.imprint.clone());
                existing_cards = state
                    .db
                    .save_character_card(&card)
                    .map_err(|e| e.to_string())?;
            }
        } else {
            let base_prompt = matched_agent
                .map(|a| a.system_prompt.clone())
                .unwrap_or_default();
            let traits = crate::observer::extract_agent_traits_from_report(
                &detail.final_report,
                &target_label,
            );
            let new_card = CharacterCard {
                id: Uuid::new_v4().to_string(),
                name: target_label,
                base_prompt,
                behavior_traits: traits,
                memory_imprints: vec![item.imprint.clone()],
                source_experiment_title: detail.title.clone(),
                is_builtin: false,
                created_at: Utc::now().to_rfc3339(),
            };
            existing_cards = state
                .db
                .save_character_card(&new_card)
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(DistillImprintsResponse {
        cards: existing_cards,
        distilled,
    })
}

#[tauri::command]
fn update_character_card_imprints(
    state: State<'_, AppState>,
    card_id: String,
    memory_imprints: Vec<String>,
) -> Result<Vec<CharacterCard>, String> {
    state
        .db
        .update_character_card_imprints(&card_id, memory_imprints)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn generate_worldline_comparison_report(
    state: State<'_, AppState>,
    exp_id_a: String,
    exp_id_b: String,
) -> Result<String, String> {
    let exp_a = state
        .db
        .get_experiment_detail(&exp_id_a)
        .map_err(|e| e.to_string())?;
    let exp_b = state
        .db
        .get_experiment_detail(&exp_id_b)
        .map_err(|e| e.to_string())?;
    crate::observer::generate_worldline_comparison_markdown(&state.engine.client, &exp_a, &exp_b)
        .await
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

#[tauri::command]
async fn fetch_provider_models(
    state: State<'_, AppState>,
    base_url: String,
    api_key: String,
) -> Result<Vec<crate::llm::RemoteModelInfo>, String> {
    crate::llm::fetch_provider_models(&state.engine.client, &base_url, &api_key)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_ohos_device_type() -> Option<String> {
    #[cfg(target_env = "ohos")]
    {
        return Some(ohos_compat::get_device_type());
    }
    #[cfg(not(target_env = "ohos"))]
    {
        None
    }
}

#[tauri::command]
async fn ohos_pick_and_save_text(
    default_name: String,
    extensions: Vec<String>,
    content: String,
) -> Result<bool, String> {
    #[cfg(target_env = "ohos")]
    {
        let temp_dir = ohos_compat::sandbox_files_dir();
        let _ = std::fs::create_dir_all(&temp_dir);
        let temp_path = temp_dir.join(format!(".export_{}.tmp", Uuid::new_v4()));
        std::fs::write(&temp_path, content.as_bytes()).map_err(|e| e.to_string())?;
        let res = ohos_compat::call_file_bridge(serde_json::json!({
            "action": "saveFromTemp",
            "defaultName": default_name,
            "extensions": extensions,
            "tempPath": temp_path.to_string_lossy().to_string(),
        }))
        .await;
        let _ = std::fs::remove_file(&temp_path);
        let raw = res?;
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
        return Ok(!parsed.get("cancelled").and_then(|v| v.as_bool()).unwrap_or(false));
    }
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = (default_name, extensions, content);
        Err("ohos_pick_and_save_text is only available on HarmonyOS".to_string())
    }
}

#[tauri::command]
async fn ohos_pick_and_save_binary(
    default_name: String,
    extensions: Vec<String>,
    data: Vec<u8>,
) -> Result<bool, String> {
    #[cfg(target_env = "ohos")]
    {
        let temp_dir = ohos_compat::sandbox_files_dir();
        let _ = std::fs::create_dir_all(&temp_dir);
        let temp_path = temp_dir.join(format!(".export_{}.tmp", Uuid::new_v4()));
        std::fs::write(&temp_path, &data).map_err(|e| e.to_string())?;
        let res = ohos_compat::call_file_bridge(serde_json::json!({
            "action": "saveFromTemp",
            "defaultName": default_name,
            "extensions": extensions,
            "tempPath": temp_path.to_string_lossy().to_string(),
        }))
        .await;
        let _ = std::fs::remove_file(&temp_path);
        let raw = res?;
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
        return Ok(!parsed.get("cancelled").and_then(|v| v.as_bool()).unwrap_or(false));
    }
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = (default_name, extensions, data);
        Err("ohos_pick_and_save_binary is only available on HarmonyOS".to_string())
    }
}

#[tauri::command]
async fn ohos_pick_and_open_text(extensions: Vec<String>) -> Result<Option<String>, String> {
    #[cfg(target_env = "ohos")]
    {
        let raw = ohos_compat::call_file_bridge(serde_json::json!({
            "action": "openText",
            "extensions": extensions,
        }))
        .await?;
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
        if parsed.get("cancelled").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Ok(None);
        }
        return Ok(Some(raw));
    }
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = extensions;
        Err("ohos_pick_and_open_text is only available on HarmonyOS".to_string())
    }
}

#[tauri::command]
fn ohos_write_clipboard(text: String) -> Result<(), String> {
    #[cfg(target_env = "ohos")]
    {
        return ohos_compat::write_clipboard_text(&text);
    }
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = text;
        Err("ohos_write_clipboard is only available on HarmonyOS".to_string())
    }
}

#[tauri::command]
fn ohos_read_clipboard() -> Result<String, String> {
    #[cfg(target_env = "ohos")]
    {
        return ohos_compat::read_clipboard_text();
    }
    #[cfg(not(target_env = "ohos"))]
    {
        Err("ohos_read_clipboard is only available on HarmonyOS".to_string())
    }
}

#[tauri::command]
fn ohos_set_back_intercept(active: bool) {
    #[cfg(target_env = "ohos")]
    {
        ohos_compat::set_back_intercept_active(active);
    }
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = active;
    }
}

#[tauri::command]
fn ohos_get_insets() -> Option<String> {
    #[cfg(target_env = "ohos")]
    {
        return ohos_compat::get_ohos_insets();
    }
    #[cfg(not(target_env = "ohos"))]
    {
        None
    }
}

#[tauri::command]
fn ohos_set_status_bar_light_icons(light_icons: bool) {
    #[cfg(target_env = "ohos")]
    {
        ohos_compat::call_ui_bridge(serde_json::json!({
            "action": "setStatusBar",
            "lightIcons": light_icons,
        }));
    }
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = light_icons;
    }
}

#[tauri::command]
fn ohos_trigger_haptic(kind: String) {
    #[cfg(target_env = "ohos")]
    {
        ohos_compat::call_ui_bridge(serde_json::json!({
            "action": "haptic",
            "kind": kind,
        }));
    }
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = kind;
    }
}

#[cfg(target_env = "ohos")]
static OHOS_STREAM_WS_PORT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);

#[tauri::command]
fn get_ohos_stream_ws_port() -> Option<u16> {
    #[cfg(target_env = "ohos")]
    {
        let port = OHOS_STREAM_WS_PORT.load(std::sync::atomic::Ordering::Relaxed);
        return if port > 0 { Some(port) } else { None };
    }
    #[cfg(not(target_env = "ohos"))]
    {
        None
    }
}

#[tauri::command]
fn show_main_window(window: tauri::Window) {
    if !window.is_visible().unwrap_or(false) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn set_window_theme(app: tauri::AppHandle, theme: String) {
    #[cfg(target_os = "windows")]
    if let Some(win) = app.get_webview_window("main") {
        win_chrome::apply_windows_theme(&win, &theme);
    }
    #[cfg(target_os = "macos")]
    if let Some(win) = app.get_webview_window("main") {
        let t = match theme.as_str() {
            "dark" => Some(tauri::Theme::Dark),
            "light" => Some(tauri::Theme::Light),
            _ => None,
        };
        let _ = win.set_theme(t);
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let _ = (app, theme);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_os::init());
    #[cfg(not(target_env = "ohos"))]
    {
        builder = builder
            .plugin(tauri_plugin_clipboard_manager::init())
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_fs::init());
    }
    builder
        .setup(|app| {
            #[cfg(target_env = "ohos")]
            ohos_compat::set_app_handle(app.handle().clone());

            #[cfg(target_env = "ohos")]
            let app_dir = ohos_compat::sandbox_files_dir();
            #[cfg(not(target_env = "ohos"))]
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            let _ = std::fs::create_dir_all(&app_dir);
            let db_path = app_dir.join("ai_terrarium.db");
            let db = Arc::new(Database::open(&db_path).expect("failed to init sqlite db"));
            let default_exp = db
                .ensure_default_experiment()
                .expect("failed to ensure default experiment");
            let engine = TerrariumEngine::new(Arc::clone(&db), default_exp.id);

            #[cfg(target_env = "ohos")]
            {
                let ws_engine = Arc::clone(&engine);
                tauri::async_runtime::spawn(async move {
                    use axum::extract::ws::{Message, WebSocketUpgrade};
                    use axum::extract::State;
                    use axum::routing::get;
                    use axum::Router;

                    async fn ws_handler(
                        ws: WebSocketUpgrade,
                        State(eng): State<Arc<TerrariumEngine>>,
                    ) -> axum::response::Response {
                        ws.on_upgrade(move |mut socket| async move {
                            let mut rx = eng.subscribe_ws_json();
                            while let Ok(json) = rx.recv().await {
                                if socket.send(Message::Text(json.into())).await.is_err() {
                                    break;
                                }
                            }
                        })
                    }

                    if let Ok(listener) = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await {
                        if let Ok(addr) = listener.local_addr() {
                            OHOS_STREAM_WS_PORT
                                .store(addr.port(), std::sync::atomic::Ordering::Relaxed);
                            let app = Router::new()
                                .route("/ws", get(ws_handler))
                                .with_state(ws_engine);
                            let _ = axum::serve(listener, app).await;
                        }
                    }
                });
            }

            let ui_prefs = db.get_ui_prefs().unwrap_or_default();
            let initial_theme = ui_prefs.theme_preference.clone();
            let local_api = Arc::new(LocalApiManager::new());
            if ui_prefs.enable_local_api {
                let api_mgr = Arc::clone(&local_api);
                let db_clone = Arc::clone(&db);
                let engine_clone = Arc::clone(&engine);
                let port = ui_prefs.local_api_port;
                let token = ui_prefs.local_api_token.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = api_mgr
                        .sync_config(true, port, token, db_clone, engine_clone)
                        .await;
                });
            }
            app.manage(AppState {
                db,
                engine,
                local_api,
            });

            // On Windows, apply frameless Mica backdrop, disable WebView2 browser menu, and enable Win11 Snap Layouts
            #[cfg(target_os = "windows")]
            if let Some(win) = app.get_webview_window("main") {
                win_chrome::setup_windows_chrome(&win, &initial_theme);
            }
            // On macOS, apply flat matte UnderWindowBackground vibrancy (avoids macOS 26 Liquid Glass glossy rim on Effect::Sidebar)
            #[cfg(target_os = "macos")]
            if let Some(win) = app.get_webview_window("main") {
                use tauri::window::{Effect, EffectState, EffectsBuilder};
                let t = match initial_theme.as_str() {
                    "dark" => Some(tauri::Theme::Dark),
                    "light" => Some(tauri::Theme::Light),
                    _ => None,
                };
                let _ = win.set_theme(t);
                let _ = win.set_effects(
                    EffectsBuilder::new()
                        .effect(Effect::UnderWindowBackground)
                        .state(EffectState::FollowsWindowActiveState)
                        .build(),
                );
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos")))]
            let _ = initial_theme;

            // Safety fallback: ensure main window becomes visible even if frontend fails to invoke show_main_window
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                if let Some(win) = handle.get_webview_window("main") {
                    if !win.is_visible().unwrap_or(true) {
                        let _ = win.show();
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_ohos_device_type,
            ohos_pick_and_save_text,
            ohos_pick_and_save_binary,
            ohos_pick_and_open_text,
            ohos_write_clipboard,
            ohos_read_clipboard,
            ohos_set_back_intercept,
            ohos_get_insets,
            ohos_set_status_bar_light_icons,
            ohos_trigger_haptic,
            get_ohos_stream_ws_port,
            show_main_window,
            set_window_theme,
            get_core_engine_prompt,
            save_core_engine_prompt,
            reset_core_engine_prompt,
            get_presets,
            save_preset,
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
            export_pure_script_markdown,
            get_message_raw_prompt,
            save_markdown_to_file,
            save_binary_to_file,
            read_text_file,
            compute_line_diff,
            get_ui_prefs,
            save_ui_prefs,
            set_blind_guess_mode,
            get_api_profiles,
            save_api_profiles,
            fork_experiment,
            update_shared_board,
            set_board_approval_mode,
            resolve_board_approval,
            generate_final_report,
            generate_worldline_comparison_report,
            export_scenario_package,
            import_scenario_package,
            generate_scenario_from_idea,
            start_private_room,
            close_private_room,
            list_character_cards,
            save_agent_to_character_library,
            distill_memory_imprints_to_library,
            update_character_card_imprints,
            delete_character_card,
            fetch_provider_models,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
