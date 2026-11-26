import { invoke } from "@tauri-apps/api/core";
import type {
  CharacterCard,
  EngineState,
  ExperimentDetail,
  ExperimentSummary,
  PresetTemplate,
  UiPrefs,
} from "./types";

/**
 * Kick off initial Tauri IPC data fetches immediately at module evaluation time
 * so Rust/SQLite queries run in parallel with Vue mounting.
 */
export const initialDataPromises = {
  activeExperiment: invoke<ExperimentDetail>("get_active_experiment"),
  experiments: invoke<ExperimentSummary[]>("list_experiments"),
  engineState: invoke<EngineState>("get_engine_state"),
  presets: invoke<PresetTemplate[]>("get_presets"),
  characterCards: invoke<CharacterCard[]>("list_character_cards"),
  uiPrefs: invoke<UiPrefs>("get_ui_prefs"),
};
