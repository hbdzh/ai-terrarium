use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::models::{
    builtin_character_cards, builtin_presets, is_pass_notice, ActivePrivateRoom, ApiProfile,
    BoardApprovalMode, CharacterCard, CrisisCard, ExperimentConfig, ExperimentDetail,
    ExperimentSummary, ObserverNote, PresetTemplate, RelationshipEdge, SpeakerRole,
    TimelineMessage, TurnSnapshot, UiPrefs, DEFAULT_CORE_ENGINE_PROMPT, LEGACY_CORE_ENGINE_PROMPT,
};
use crate::sanitizer::strip_think_tags;

const CORE_ENGINE_PROMPT_KEY: &str = "core_engine_prompt";
const UI_PREFS_KEY: &str = "ui_prefs";
const API_PROFILES_KEY: &str = "api_profiles";
const BUILTIN_PRESETS_VERSION_KEY: &str = "builtin_presets_version";
const CURRENT_BUILTIN_PRESETS_VERSION: &str = "5";

fn is_legacy_unmodified_builtin(p: &PresetTemplate) -> bool {
    match p.id.as_str() {
        "free" => {
            p.is_builtin && p.recommended_config.is_none() && p.global_system_prompt.is_empty()
        }
        "late_night_friends" => p.is_builtin && p.recommended_config.is_none(),
        "casual_chat" => {
            (p.is_builtin && p.recommended_config.is_none())
                || p.global_system_prompt
                    == "你们正在一间深夜便利店里对话。请像真人发微信一样自然口语化，每次严格 3～5 句话（不超过 120 字），适当带动作括号描写（如（拧开瓶盖）（瞥了一眼门外）），禁止引文挑刺、禁止学术黑话、禁止客套说教与总结升华。"
                || p.global_system_prompt.starts_with("【场景背景】深夜暴雨停电的街角便利店。")
        }
        "shared_apartment" | "space_station" | "missing_usb" => {
            p.is_builtin && p.recommended_config.is_none()
        }
        "philosophy" => {
            (p.global_system_prompt
                == "本次实验聚焦高密度哲学思辨：禁止空泛情绪宣泄或敷衍认同，每次回应必须精准回应上一位发言者的核心命题并推进一层思辨纵深。"
                || p.global_system_prompt.starts_with("【场景背景】高密度哲学思辨圆桌。"))
                && p.initial_topic.is_empty()
        }
        "scifi" => {
            (p.global_system_prompt
                == "你们正在共同推演近未来人类文明演化沙盘：请尽量给出具体制度、技术细节与日常切片，善用共享白板沉淀已确立的世界观公理。"
                || p.global_system_prompt.starts_with("【场景背景】近未来人类文明演化沙盘推演。"))
                && p.initial_topic.is_empty()
        }
        "absurd" => {
            (p.global_system_prompt
                == "喜剧黄金法则：无论设定多么离谱，所有参与者都必须全程一本正经、绝不自己笑场或跳出设定吐槽。"
                || p.global_system_prompt.starts_with("【场景背景】严肃荒诞喜剧现场。"))
                && p.initial_topic.is_empty()
        }
        _ => false,
    }
}

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("failed to create db parent dir")?;
        }
        let conn = Connection::open(path).context("failed to open sqlite database")?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA temp_store = MEMORY;
             PRAGMA cache_size = -16000;
             PRAGMA busy_timeout = 5000;
             PRAGMA foreign_keys = ON;",
        )?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS experiments (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                config_json TEXT NOT NULL,
                total_turns INTEGER NOT NULL DEFAULT 0,
                total_tokens INTEGER NOT NULL DEFAULT 0,
                next_speaker TEXT NOT NULL DEFAULT 'AiA',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                experiment_id TEXT NOT NULL,
                seq INTEGER NOT NULL,
                turn_number INTEGER NOT NULL DEFAULT 0,
                role TEXT NOT NULL,
                agent_id TEXT NOT NULL DEFAULT '',
                content TEXT NOT NULL,
                model_name TEXT NOT NULL DEFAULT '',
                duration_ms INTEGER NOT NULL DEFAULT 0,
                prompt_tokens INTEGER NOT NULL DEFAULT 0,
                completion_tokens INTEGER NOT NULL DEFAULT 0,
                total_tokens INTEGER NOT NULL DEFAULT 0,
                whisper_consumed INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                FOREIGN KEY (experiment_id) REFERENCES experiments(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_messages_exp_seq
                ON messages(experiment_id, seq);

            CREATE TABLE IF NOT EXISTS observer_notes (
                id TEXT PRIMARY KEY,
                experiment_id TEXT NOT NULL,
                trigger_turn INTEGER NOT NULL,
                tags_json TEXT NOT NULL,
                depth_stage TEXT NOT NULL,
                note TEXT NOT NULL,
                stagnation_index INTEGER NOT NULL,
                perturbation_injected INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                FOREIGN KEY (experiment_id) REFERENCES experiments(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_observer_exp_turn
                ON observer_notes(experiment_id, trigger_turn DESC);

            CREATE TABLE IF NOT EXISTS turn_snapshots (
                id TEXT PRIMARY KEY,
                experiment_id TEXT NOT NULL,
                turn_number INTEGER NOT NULL,
                message_ids_json TEXT NOT NULL,
                board_content TEXT NOT NULL DEFAULT '',
                compacted_summary TEXT NOT NULL DEFAULT '',
                compacted_until_seq INTEGER NOT NULL DEFAULT 0,
                total_tokens INTEGER NOT NULL DEFAULT 0,
                next_speaker TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (experiment_id) REFERENCES experiments(id) ON DELETE CASCADE
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_snapshots_exp_turn
                ON turn_snapshots(experiment_id, turn_number);

            CREATE TABLE IF NOT EXISTS preset_templates (
                id TEXT PRIMARY KEY,
                sort_order INTEGER NOT NULL DEFAULT 0,
                data_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS character_cards (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                base_prompt TEXT NOT NULL DEFAULT '',
                behavior_traits TEXT NOT NULL DEFAULT '',
                source_experiment_title TEXT NOT NULL DEFAULT '',
                is_builtin INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            ",
        )?;

        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![CORE_ENGINE_PROMPT_KEY, DEFAULT_CORE_ENGINE_PROMPT, now],
        )?;
        conn.execute(
            "UPDATE app_settings SET value = ?1, updated_at = ?2 WHERE key = ?3 AND value = ?4",
            params![
                DEFAULT_CORE_ENGINE_PROMPT,
                now,
                CORE_ENGINE_PROMPT_KEY,
                LEGACY_CORE_ENGINE_PROMPT
            ],
        )?;

        let preset_version: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![BUILTIN_PRESETS_VERSION_KEY],
                |r| r.get(0),
            )
            .optional()?;
        if preset_version.as_deref() != Some(CURRENT_BUILTIN_PRESETS_VERSION) {
            for obsolete_id in [
                "philosophy",
                "scifi",
                "absurd",
                "shared_apartment",
                "space_station",
                "missing_usb",
            ] {
                let existing_obsolete: Option<String> = conn
                    .query_row(
                        "SELECT data_json FROM preset_templates WHERE id = ?1",
                        params![obsolete_id],
                        |r| r.get(0),
                    )
                    .optional()?;
                if let Some(old_json) = existing_obsolete {
                    if let Ok(old_preset) = serde_json::from_str::<PresetTemplate>(&old_json) {
                        if is_legacy_unmodified_builtin(&old_preset) {
                            conn.execute(
                                "DELETE FROM preset_templates WHERE id = ?1",
                                params![obsolete_id],
                            )?;
                        }
                    }
                }
            }
            conn.execute(
                "INSERT INTO app_settings (key, value, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET
                    value = excluded.value,
                    updated_at = excluded.updated_at",
                params![
                    BUILTIN_PRESETS_VERSION_KEY,
                    CURRENT_BUILTIN_PRESETS_VERSION,
                    now
                ],
            )?;
        }

        // Seed built-in presets if not yet persisted, or upgrade unmodified legacy built-in presets
        for (idx, preset) in builtin_presets().into_iter().enumerate() {
            let data_json = serde_json::to_string(&preset)?;
            let existing: Option<String> = conn
                .query_row(
                    "SELECT data_json FROM preset_templates WHERE id = ?1",
                    params![preset.id],
                    |r| r.get(0),
                )
                .optional()?;

            match existing {
                None => {
                    conn.execute(
                        "INSERT INTO preset_templates (id, sort_order, data_json, updated_at)
                         VALUES (?1, ?2, ?3, ?4)",
                        params![preset.id, idx as i64, data_json, now],
                    )?;
                }
                Some(old_json) => {
                    if let Ok(mut old_preset) = serde_json::from_str::<PresetTemplate>(&old_json) {
                        if is_legacy_unmodified_builtin(&old_preset) {
                            conn.execute(
                                "UPDATE preset_templates
                                 SET sort_order = ?1, data_json = ?2, updated_at = ?3
                                 WHERE id = ?4",
                                params![idx as i64, data_json, now, preset.id],
                            )?;
                        } else if old_preset.is_builtin
                            && old_preset.tags.is_empty()
                            && !preset.tags.is_empty()
                        {
                            old_preset.tags = preset.tags.clone();
                            let patched_json = serde_json::to_string(&old_preset)?;
                            conn.execute(
                                "UPDATE preset_templates
                                 SET sort_order = ?1, data_json = ?2, updated_at = ?3
                                 WHERE id = ?4",
                                params![idx as i64, patched_json, now, preset.id],
                            )?;
                        } else {
                            conn.execute(
                                "UPDATE preset_templates SET sort_order = ?1 WHERE id = ?2",
                                params![idx as i64, preset.id],
                            )?;
                        }
                    }
                }
            }
        }

        // Migrate older databases lacking messages.agent_id or messages.reasoning_content
        let mut stmt = conn.prepare("PRAGMA table_info(messages)")?;
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        if !cols.iter().any(|c| c == "agent_id") {
            conn.execute(
                "ALTER TABLE messages ADD COLUMN agent_id TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        if !cols.iter().any(|c| c == "reasoning_content") {
            conn.execute(
                "ALTER TABLE messages ADD COLUMN reasoning_content TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }

        // Migrate older databases lacking character_cards.is_builtin, then seed built-in character cards
        let mut cc_stmt = conn.prepare("PRAGMA table_info(character_cards)")?;
        let cc_cols: Vec<String> = cc_stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        if !cc_cols.iter().any(|c| c == "is_builtin") {
            conn.execute(
                "ALTER TABLE character_cards ADD COLUMN is_builtin INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        if !cc_cols.iter().any(|c| c == "memory_imprints_json") {
            conn.execute(
                "ALTER TABLE character_cards ADD COLUMN memory_imprints_json TEXT NOT NULL DEFAULT '[]'",
                [],
            )?;
        }
        self.seed_builtin_character_cards_conn(&conn)?;
        if !cols.iter().any(|c| c == "raw_prompt_json") {
            conn.execute("ALTER TABLE messages ADD COLUMN raw_prompt_json TEXT", [])?;
        }
        if !cols.iter().any(|c| c == "room_id") {
            conn.execute(
                "ALTER TABLE messages ADD COLUMN room_id TEXT NOT NULL DEFAULT 'main'",
                [],
            )?;
        }

        // Migrate older databases lacking experiments.compacted_summary / compacted_until_seq
        let mut exp_stmt = conn.prepare("PRAGMA table_info(experiments)")?;
        let exp_cols: Vec<String> = exp_stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        if !exp_cols.iter().any(|c| c == "compacted_summary") {
            conn.execute(
                "ALTER TABLE experiments ADD COLUMN compacted_summary TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        if !exp_cols.iter().any(|c| c == "compacted_until_seq") {
            conn.execute(
                "ALTER TABLE experiments ADD COLUMN compacted_until_seq INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        if !exp_cols.iter().any(|c| c == "board_content") {
            conn.execute(
                "ALTER TABLE experiments ADD COLUMN board_content TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        if !exp_cols.iter().any(|c| c == "board_prev_content") {
            conn.execute(
                "ALTER TABLE experiments ADD COLUMN board_prev_content TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        if !exp_cols.iter().any(|c| c == "final_report") {
            conn.execute(
                "ALTER TABLE experiments ADD COLUMN final_report TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        if !exp_cols.iter().any(|c| c == "private_notes_json") {
            conn.execute(
                "ALTER TABLE experiments ADD COLUMN private_notes_json TEXT NOT NULL DEFAULT '{}'",
                [],
            )?;
        }
        if !exp_cols.iter().any(|c| c == "active_room_json") {
            conn.execute(
                "ALTER TABLE experiments ADD COLUMN active_room_json TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        if !exp_cols.iter().any(|c| c == "room_members_json") {
            conn.execute(
                "ALTER TABLE experiments ADD COLUMN room_members_json TEXT NOT NULL DEFAULT '{}'",
                [],
            )?;
        }
        if !exp_cols.iter().any(|c| c == "parent_experiment_id") {
            conn.execute(
                "ALTER TABLE experiments ADD COLUMN parent_experiment_id TEXT",
                [],
            )?;
        }
        if !exp_cols.iter().any(|c| c == "fork_turn") {
            conn.execute(
                "ALTER TABLE experiments ADD COLUMN fork_turn INTEGER",
                [],
            )?;
        }

        let mut snap_stmt = conn.prepare("PRAGMA table_info(turn_snapshots)")?;
        let snap_cols: Vec<String> = snap_stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        if !snap_cols.iter().any(|c| c == "private_notes_json") {
            conn.execute(
                "ALTER TABLE turn_snapshots ADD COLUMN private_notes_json TEXT NOT NULL DEFAULT '{}'",
                [],
            )?;
        }
        if !snap_cols.iter().any(|c| c == "active_room_json") {
            conn.execute(
                "ALTER TABLE turn_snapshots ADD COLUMN active_room_json TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        if !snap_cols.iter().any(|c| c == "room_members_json") {
            conn.execute(
                "ALTER TABLE turn_snapshots ADD COLUMN room_members_json TEXT NOT NULL DEFAULT '{}'",
                [],
            )?;
        }

        // Migrate older databases lacking observer_notes.tension_score / abstraction_score / crisis_cards_json
        let mut obs_cols_stmt = conn.prepare("PRAGMA table_info(observer_notes)")?;
        let obs_cols: Vec<String> = obs_cols_stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        if !obs_cols.iter().any(|c| c == "tension_score") {
            conn.execute(
                "ALTER TABLE observer_notes ADD COLUMN tension_score INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        if !obs_cols.iter().any(|c| c == "abstraction_score") {
            conn.execute(
                "ALTER TABLE observer_notes ADD COLUMN abstraction_score INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        if !obs_cols.iter().any(|c| c == "crisis_cards_json") {
            conn.execute(
                "ALTER TABLE observer_notes ADD COLUMN crisis_cards_json TEXT NOT NULL DEFAULT '[]'",
                [],
            )?;
        }
        if !obs_cols.iter().any(|c| c == "narrative_act") {
            conn.execute(
                "ALTER TABLE observer_notes ADD COLUMN narrative_act TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        if !obs_cols.iter().any(|c| c == "relationship_matrix_json") {
            conn.execute(
                "ALTER TABLE observer_notes ADD COLUMN relationship_matrix_json TEXT NOT NULL DEFAULT '[]'",
                [],
            )?;
        }
        Ok(())
    }

    pub fn get_core_engine_prompt(&self) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        let val: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![CORE_ENGINE_PROMPT_KEY],
                |r| r.get(0),
            )
            .optional()?;
        Ok(val.unwrap_or_else(|| DEFAULT_CORE_ENGINE_PROMPT.to_string()))
    }

    pub fn save_core_engine_prompt(&self, prompt: &str) -> Result<String> {
        let now = Utc::now().to_rfc3339();
        let trimmed = prompt.trim().to_string();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO app_settings (key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at",
            params![CORE_ENGINE_PROMPT_KEY, trimmed, now],
        )?;
        Ok(trimmed)
    }

    pub fn reset_core_engine_prompt(&self) -> Result<String> {
        self.save_core_engine_prompt(DEFAULT_CORE_ENGINE_PROMPT)
    }

    pub fn get_ui_prefs(&self) -> Result<UiPrefs> {
        let conn = self.conn.lock().unwrap();
        let val: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![UI_PREFS_KEY],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(json) = val {
            if let Ok(prefs) = serde_json::from_str::<UiPrefs>(&json) {
                return Ok(prefs);
            }
        }
        Ok(UiPrefs::default())
    }

    pub fn save_ui_prefs(&self, prefs: &UiPrefs) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let json = serde_json::to_string(prefs)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO app_settings (key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at",
            params![UI_PREFS_KEY, json, now],
        )?;
        Ok(())
    }

    pub fn get_api_profiles(&self) -> Result<Vec<ApiProfile>> {
        let conn = self.conn.lock().unwrap();
        let val: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![API_PROFILES_KEY],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(json) = val {
            if let Ok(profiles) = serde_json::from_str::<Vec<ApiProfile>>(&json) {
                return Ok(profiles);
            }
        }
        Ok(Vec::new())
    }

    pub fn save_api_profiles(&self, profiles: &[ApiProfile]) -> Result<Vec<ApiProfile>> {
        let now = Utc::now().to_rfc3339();
        let mut normalized: Vec<ApiProfile> = profiles
            .iter()
            .enumerate()
            .map(|(idx, p)| ApiProfile {
                id: if p.id.trim().is_empty() {
                    format!("api-{}", Uuid::new_v4().simple())
                } else {
                    p.id.trim().to_string()
                },
                name: if p.name.trim().is_empty() {
                    format!("API 配置 {}", idx + 1)
                } else {
                    p.name.trim().to_string()
                },
                base_url: p.base_url.trim().to_string(),
                api_key: p.api_key.trim().to_string(),
                model_name: p.model_name.trim().to_string(),
            })
            .collect();
        if normalized.is_empty() {
            normalized.push(ApiProfile {
                id: "default".to_string(),
                name: "默认 API".to_string(),
                base_url: "https://api.deepseek.com".to_string(),
                api_key: String::new(),
                model_name: "deepseek-flash".to_string(),
            });
        }
        let json = serde_json::to_string(&normalized)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO app_settings (key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at",
            params![API_PROFILES_KEY, json, now],
        )?;
        Ok(normalized)
    }

    pub fn list_presets(&self) -> Result<Vec<PresetTemplate>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT data_json FROM preset_templates ORDER BY sort_order ASC, updated_at ASC",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut items = Vec::new();
        for r in rows {
            if let Ok(mut p) = serde_json::from_str::<PresetTemplate>(&r?) {
                p.normalize();
                items.push(p);
            }
        }
        if items.is_empty() {
            return Ok(builtin_presets());
        }
        Ok(items)
    }

    pub fn save_preset(&self, preset: &PresetTemplate) -> Result<Vec<PresetTemplate>> {
        let mut normalized = preset.clone();
        if normalized.id.trim().is_empty() {
            normalized.id = format!("preset-{}", Uuid::new_v4().simple());
        }
        normalized.normalize();
        let data_json = serde_json::to_string(&normalized)?;
        let now = Utc::now().to_rfc3339();
        {
            let conn = self.conn.lock().unwrap();
            let existing_order: Option<i64> = conn
                .query_row(
                    "SELECT sort_order FROM preset_templates WHERE id = ?1",
                    params![normalized.id],
                    |r| r.get(0),
                )
                .optional()?;
            let sort_order = match existing_order {
                Some(o) => o,
                None => {
                    let max_o: Option<i64> =
                        conn.query_row("SELECT MAX(sort_order) FROM preset_templates", [], |r| {
                            r.get(0)
                        })?;
                    max_o.unwrap_or(0) + 1
                }
            };
            conn.execute(
                "INSERT INTO preset_templates (id, sort_order, data_json, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                    data_json = excluded.data_json,
                    updated_at = excluded.updated_at",
                params![normalized.id, sort_order, data_json, now],
            )?;
        }
        self.list_presets()
    }

    pub fn reset_builtin_preset(&self, preset_id: &str) -> Result<Vec<PresetTemplate>> {
        if let Some(builtin) = builtin_presets().into_iter().find(|p| p.id == preset_id) {
            return self.save_preset(&builtin);
        }
        self.list_presets()
    }

    pub fn delete_preset(&self, preset_id: &str) -> Result<Vec<PresetTemplate>> {
        {
            let conn = self.conn.lock().unwrap();
            conn.execute(
                "DELETE FROM preset_templates WHERE id = ?1",
                params![preset_id],
            )?;
        }
        self.list_presets()
    }

    pub fn ensure_default_experiment(&self) -> Result<ExperimentDetail> {
        let list = self.list_experiments()?;
        if let Some(first) = list.first() {
            return self.get_experiment_detail(&first.id);
        }
        let cfg = ExperimentConfig::default();
        self.create_experiment("自由漫谈 #01", &cfg)
    }

    pub fn create_experiment(
        &self,
        title: &str,
        config: &ExperimentConfig,
    ) -> Result<ExperimentDetail> {
        let mut normalized = config.clone();
        normalized.normalize();

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let config_json = serde_json::to_string(&normalized)?;
        let first_speaker = normalized
            .agents
            .first()
            .map(|a| a.id.clone())
            .unwrap_or_else(|| "AiA".to_string());
        {
            let conn = self.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO experiments (id, title, config_json, total_turns, total_tokens, next_speaker, board_content, board_prev_content, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 0, 0, ?4, ?5, ?5, ?6, ?7)",
                params![id, title, config_json, first_speaker, normalized.initial_board, now, now],
            )?;
        }
        self.get_experiment_detail(&id)
    }

    pub fn list_experiments(&self) -> Result<Vec<ExperimentSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, total_turns, total_tokens, next_speaker, parent_experiment_id, fork_turn, created_at, updated_at
             FROM experiments
             ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ExperimentSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                total_turns: row.get(2)?,
                total_tokens: row.get(3)?,
                next_speaker: row.get(4)?,
                parent_experiment_id: row.get(5)?,
                fork_turn: row.get::<_, Option<i64>>(6)?.map(|v| v.max(0) as u32),
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?;
        let mut items = Vec::new();
        for r in rows {
            items.push(r?);
        }
        Ok(items)
    }

    pub fn get_experiment_detail(&self, id: &str) -> Result<ExperimentDetail> {
        let conn = self.conn.lock().unwrap();
        let row = conn
            .query_row(
                "SELECT id, title, config_json, total_turns, total_tokens, next_speaker,
                        board_content, board_prev_content, compacted_summary, compacted_until_seq,
                        final_report, created_at, updated_at, private_notes_json,
                        active_room_json, room_members_json, parent_experiment_id, fork_turn
                 FROM experiments WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, u32>(3)?,
                        row.get::<_, u64>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, i64>(9)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, String>(11)?,
                        row.get::<_, String>(12)?,
                        row.get::<_, String>(13)?,
                        row.get::<_, String>(14)?,
                        row.get::<_, String>(15)?,
                        row.get::<_, Option<String>>(16)?,
                        row.get::<_, Option<i64>>(17)?,
                    ))
                },
            )
            .optional()?
            .context("experiment not found")?;

        let private_notes: HashMap<String, String> =
            serde_json::from_str(&row.13).unwrap_or_default();
        let active_private_room: Option<ActivePrivateRoom> = if row.14.trim().is_empty() {
            None
        } else {
            serde_json::from_str(&row.14).ok()
        };
        let room_members: HashMap<String, Vec<String>> =
            serde_json::from_str(&row.15).unwrap_or_default();

        let mut config: ExperimentConfig = serde_json::from_str(&row.2).unwrap_or_default();
        config.normalize();

        let mut msg_stmt = conn.prepare(
            "SELECT id, experiment_id, seq, turn_number, role, agent_id, reasoning_content, content, model_name, duration_ms,
                    prompt_tokens, completion_tokens, total_tokens, whisper_consumed, created_at, room_id
             FROM messages
             WHERE experiment_id = ?1
             ORDER BY seq ASC",
        )?;
        let msg_rows = msg_stmt.query_map(params![id], |r| {
            let role_str: String = r.get(4)?;
            let agent_id_col: String = r.get(5)?;
            let (role, agent_id) = SpeakerRole::from_db(&role_str, &agent_id_col);
            let room_col: String = r.get(15)?;
            let room_id = if room_col.trim().is_empty() {
                "main".to_string()
            } else {
                room_col
            };
            Ok(TimelineMessage {
                id: r.get(0)?,
                experiment_id: r.get(1)?,
                seq: r.get(2)?,
                turn_number: r.get(3)?,
                role,
                agent_id,
                room_id,
                reasoning_content: r.get(6)?,
                content: r.get(7)?,
                model_name: r.get(8)?,
                duration_ms: r.get(9)?,
                prompt_tokens: r.get(10)?,
                completion_tokens: r.get(11)?,
                total_tokens: r.get(12)?,
                whisper_consumed: r.get::<_, i64>(13)? != 0,
                created_at: r.get(14)?,
            })
        })?;
        let mut messages = Vec::new();
        for m in msg_rows {
            messages.push(m?);
        }

        let mut obs_stmt = conn.prepare(
            "SELECT id, experiment_id, trigger_turn, tags_json, depth_stage, note,
                    tension_score, abstraction_score, stagnation_index, perturbation_injected,
                    crisis_cards_json, created_at, narrative_act, relationship_matrix_json
             FROM observer_notes
             WHERE experiment_id = ?1
             ORDER BY trigger_turn DESC, created_at DESC",
        )?;
        let obs_rows = obs_stmt.query_map(params![id], |r| {
            let tags_str: String = r.get(3)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            let cards_str: String = r.get(10)?;
            let crisis_cards: Vec<CrisisCard> =
                serde_json::from_str(&cards_str).unwrap_or_default();
            let matrix_str: String = r.get(13)?;
            let relationship_matrix: Vec<RelationshipEdge> =
                serde_json::from_str(&matrix_str).unwrap_or_default();
            Ok(ObserverNote {
                id: r.get(0)?,
                experiment_id: r.get(1)?,
                trigger_turn: r.get(2)?,
                tags,
                depth_stage: r.get(4)?,
                narrative_act: r.get(12)?,
                note: r.get(5)?,
                tension_score: r.get::<_, i64>(6)?.clamp(0, 100) as u8,
                abstraction_score: r.get::<_, i64>(7)?.clamp(0, 100) as u8,
                stagnation_index: r.get::<_, i64>(8)?.clamp(0, 100) as u8,
                perturbation_injected: r.get::<_, i64>(9)? != 0,
                crisis_cards,
                relationship_matrix,
                created_at: r.get(11)?,
            })
        })?;
        let mut observer_notes = Vec::new();
        for n in obs_rows {
            observer_notes.push(n?);
        }

        Ok(ExperimentDetail {
            id: row.0,
            title: row.1,
            config,
            total_turns: row.3,
            total_tokens: row.4,
            next_speaker: row.5,
            parent_experiment_id: row.16,
            fork_turn: row.17.map(|v| v.max(0) as u32),
            board_content: row.6,
            board_prev_content: row.7,
            private_notes,
            active_private_room,
            room_members,
            compacted_summary: row.8,
            compacted_until_seq: row.9,
            final_report: row.10,
            messages,
            observer_notes,
            created_at: row.11,
            updated_at: row.12,
        })
    }

    pub fn get_total_turns(&self, experiment_id: &str) -> Result<u32> {
        let conn = self.conn.lock().unwrap();
        let turns: u32 = conn
            .query_row(
                "SELECT total_turns FROM experiments WHERE id = ?1",
                params![experiment_id],
                |r| r.get(0),
            )
            .optional()?
            .context("experiment not found")?;
        Ok(turns)
    }

    pub fn get_board_content(&self, experiment_id: &str) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        let board: String = conn
            .query_row(
                "SELECT board_content FROM experiments WHERE id = ?1",
                params![experiment_id],
                |r| r.get(0),
            )
            .optional()?
            .context("experiment not found")?;
        Ok(board)
    }

    pub fn get_experiment_header(&self, experiment_id: &str) -> Result<(ExperimentConfig, String)> {
        let conn = self.conn.lock().unwrap();
        let (cfg_json, board): (String, String) = conn
            .query_row(
                "SELECT config_json, board_content FROM experiments WHERE id = ?1",
                params![experiment_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .context("experiment not found")?;
        let mut config: ExperimentConfig = serde_json::from_str(&cfg_json).unwrap_or_default();
        config.normalize();
        Ok((config, board))
    }

    pub fn get_snapshot_source(
        &self,
        experiment_id: &str,
        turn_number: u32,
    ) -> Result<TurnSnapshot> {
        let conn = self.conn.lock().unwrap();
        let row = conn
            .query_row(
                "SELECT board_content, private_notes_json, active_room_json, room_members_json,
                        compacted_summary, compacted_until_seq, total_tokens, next_speaker
                 FROM experiments WHERE id = ?1",
                params![experiment_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, u64>(6)?,
                        r.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()?
            .context("experiment not found")?;

        let mut stmt = conn.prepare(
            "SELECT id FROM messages WHERE experiment_id = ?1 AND turn_number <= ?2 ORDER BY seq ASC",
        )?;
        let rows = stmt.query_map(params![experiment_id, turn_number], |r| {
            r.get::<_, String>(0)
        })?;
        let mut message_ids = Vec::new();
        for r in rows {
            message_ids.push(r?);
        }

        let private_notes: HashMap<String, String> =
            serde_json::from_str(&row.1).unwrap_or_default();
        let active_private_room: Option<ActivePrivateRoom> = if row.2.trim().is_empty() {
            None
        } else {
            serde_json::from_str(&row.2).ok()
        };
        let room_members: HashMap<String, Vec<String>> =
            serde_json::from_str(&row.3).unwrap_or_default();

        Ok(TurnSnapshot {
            id: Uuid::new_v4().to_string(),
            experiment_id: experiment_id.to_string(),
            turn_number,
            message_ids,
            board_content: row.0,
            private_notes,
            active_private_room,
            room_members,
            compacted_summary: row.4,
            compacted_until_seq: row.5,
            total_tokens: row.6,
            next_speaker: row.7,
            created_at: Utc::now().to_rfc3339(),
        })
    }

    pub fn update_experiment(
        &self,
        id: &str,
        title: &str,
        config: &ExperimentConfig,
    ) -> Result<()> {
        let mut normalized = config.clone();
        normalized.normalize();
        let now = Utc::now().to_rfc3339();
        let config_json = serde_json::to_string(&normalized)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE experiments SET title = ?1, config_json = ?2, updated_at = ?3 WHERE id = ?4",
            params![title, config_json, now, id],
        )?;
        Ok(())
    }

    pub fn delete_experiment(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute("DELETE FROM experiments WHERE id = ?1", params![id])?;
        if deleted > 0 {
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE); PRAGMA optimize;");
        }
        Ok(())
    }

    pub fn next_seq(&self, experiment_id: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let max_seq: Option<i64> = conn.query_row(
            "SELECT MAX(seq) FROM messages WHERE experiment_id = ?1",
            params![experiment_id],
            |r| r.get(0),
        )?;
        Ok(max_seq.unwrap_or(0) + 1)
    }

    pub fn append_message(&self, msg: &TimelineMessage) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let room_id = if msg.room_id.trim().is_empty() {
            "main"
        } else {
            msg.room_id.trim()
        };
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO messages (
                id, experiment_id, seq, turn_number, role, agent_id, room_id, reasoning_content, content, model_name, duration_ms,
                prompt_tokens, completion_tokens, total_tokens, whisper_consumed, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                msg.id,
                msg.experiment_id,
                msg.seq,
                msg.turn_number,
                msg.role.as_str(),
                msg.agent_id.as_deref().unwrap_or(""),
                room_id,
                msg.reasoning_content,
                msg.content,
                msg.model_name,
                msg.duration_ms,
                msg.prompt_tokens,
                msg.completion_tokens,
                msg.total_tokens,
                if msg.whisper_consumed { 1 } else { 0 },
                msg.created_at,
            ],
        )?;
        conn.execute(
            "UPDATE experiments SET updated_at = ?1 WHERE id = ?2",
            params![now, msg.experiment_id],
        )?;
        Ok(())
    }

    pub fn save_message_raw_prompt(&self, message_id: &str, raw_prompt_json: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE messages SET raw_prompt_json = ?1 WHERE id = ?2",
            params![raw_prompt_json, message_id],
        )?;
        Ok(())
    }

    pub fn get_message_raw_prompt(&self, message_id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let res: Option<Option<String>> = conn
            .query_row(
                "SELECT raw_prompt_json FROM messages WHERE id = ?1",
                params![message_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(res.flatten())
    }

    pub fn set_experiment_lineage(
        &self,
        experiment_id: &str,
        parent_experiment_id: &str,
        fork_turn: u32,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE experiments SET parent_experiment_id = ?1, fork_turn = ?2 WHERE id = ?3",
            params![parent_experiment_id, fork_turn as i64, experiment_id],
        )?;
        Ok(())
    }

    pub fn update_experiment_state_for_fork(
        &self,
        experiment_id: &str,
        total_turns: u32,
        total_tokens: u64,
        next_speaker: &str,
        board_content: &str,
        private_notes: &HashMap<String, String>,
        active_private_room: &Option<ActivePrivateRoom>,
        room_members: &HashMap<String, Vec<String>>,
        compacted_summary: &str,
        compacted_until_seq: i64,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let private_notes_json = serde_json::to_string(private_notes)?;
        let active_room_json = match active_private_room {
            Some(r) => serde_json::to_string(r)?,
            None => String::new(),
        };
        let room_members_json = serde_json::to_string(room_members)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE experiments
             SET total_turns = ?1, total_tokens = ?2, next_speaker = ?3,
                 board_content = ?4, board_prev_content = ?4,
                 private_notes_json = ?5, active_room_json = ?6, room_members_json = ?7,
                 compacted_summary = ?8, compacted_until_seq = ?9, updated_at = ?10
             WHERE id = ?11",
            params![
                total_turns,
                total_tokens,
                next_speaker,
                board_content,
                private_notes_json,
                active_room_json,
                room_members_json,
                compacted_summary,
                compacted_until_seq,
                now,
                experiment_id
            ],
        )?;
        Ok(())
    }

    pub fn save_turn_snapshot(&self, snap: &TurnSnapshot) -> Result<()> {
        let ids_json = serde_json::to_string(&snap.message_ids)?;
        let private_notes_json = serde_json::to_string(&snap.private_notes)?;
        let active_room_json = match &snap.active_private_room {
            Some(r) => serde_json::to_string(r)?,
            None => String::new(),
        };
        let room_members_json = serde_json::to_string(&snap.room_members)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO turn_snapshots (
                id, experiment_id, turn_number, message_ids_json, board_content, private_notes_json,
                active_room_json, room_members_json,
                compacted_summary, compacted_until_seq, total_tokens, next_speaker, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(experiment_id, turn_number) DO UPDATE SET
                id = excluded.id,
                message_ids_json = excluded.message_ids_json,
                board_content = excluded.board_content,
                private_notes_json = excluded.private_notes_json,
                active_room_json = excluded.active_room_json,
                room_members_json = excluded.room_members_json,
                compacted_summary = excluded.compacted_summary,
                compacted_until_seq = excluded.compacted_until_seq,
                total_tokens = excluded.total_tokens,
                next_speaker = excluded.next_speaker,
                created_at = excluded.created_at",
            params![
                snap.id,
                snap.experiment_id,
                snap.turn_number,
                ids_json,
                snap.board_content,
                private_notes_json,
                active_room_json,
                room_members_json,
                snap.compacted_summary,
                snap.compacted_until_seq,
                snap.total_tokens,
                snap.next_speaker,
                snap.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_turn_snapshots(&self, experiment_id: &str) -> Result<Vec<TurnSnapshot>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, experiment_id, turn_number, message_ids_json, board_content, private_notes_json,
                    active_room_json, room_members_json,
                    compacted_summary, compacted_until_seq, total_tokens, next_speaker, created_at
             FROM turn_snapshots
             WHERE experiment_id = ?1
             ORDER BY turn_number ASC",
        )?;
        let rows = stmt.query_map(params![experiment_id], |r| {
            let ids_str: String = r.get(3)?;
            let message_ids: Vec<String> = serde_json::from_str(&ids_str).unwrap_or_default();
            let notes_str: String = r.get(5)?;
            let private_notes: HashMap<String, String> =
                serde_json::from_str(&notes_str).unwrap_or_default();
            let active_str: String = r.get(6)?;
            let active_private_room = if active_str.trim().is_empty() {
                None
            } else {
                serde_json::from_str(&active_str).ok()
            };
            let members_str: String = r.get(7)?;
            let room_members: HashMap<String, Vec<String>> =
                serde_json::from_str(&members_str).unwrap_or_default();
            Ok(TurnSnapshot {
                id: r.get(0)?,
                experiment_id: r.get(1)?,
                turn_number: r.get(2)?,
                message_ids,
                board_content: r.get(4)?,
                private_notes,
                active_private_room,
                room_members,
                compacted_summary: r.get(8)?,
                compacted_until_seq: r.get(9)?,
                total_tokens: r.get(10)?,
                next_speaker: r.get(11)?,
                created_at: r.get(12)?,
            })
        })?;
        let mut items = Vec::new();
        for r in rows {
            items.push(r?);
        }
        Ok(items)
    }

    pub fn get_turn_snapshot(
        &self,
        experiment_id: &str,
        turn_number: u32,
    ) -> Result<Option<TurnSnapshot>> {
        let conn = self.conn.lock().unwrap();
        let res = conn
            .query_row(
                "SELECT id, experiment_id, turn_number, message_ids_json, board_content, private_notes_json,
                        active_room_json, room_members_json,
                        compacted_summary, compacted_until_seq, total_tokens, next_speaker, created_at
                 FROM turn_snapshots
                 WHERE experiment_id = ?1 AND turn_number = ?2",
                params![experiment_id, turn_number],
                |r| {
                    let ids_str: String = r.get(3)?;
                    let message_ids: Vec<String> =
                        serde_json::from_str(&ids_str).unwrap_or_default();
                    let notes_str: String = r.get(5)?;
                    let private_notes: HashMap<String, String> =
                        serde_json::from_str(&notes_str).unwrap_or_default();
                    let active_str: String = r.get(6)?;
                    let active_private_room = if active_str.trim().is_empty() {
                        None
                    } else {
                        serde_json::from_str(&active_str).ok()
                    };
                    let members_str: String = r.get(7)?;
                    let room_members: HashMap<String, Vec<String>> =
                        serde_json::from_str(&members_str).unwrap_or_default();
                    Ok(TurnSnapshot {
                        id: r.get(0)?,
                        experiment_id: r.get(1)?,
                        turn_number: r.get(2)?,
                        message_ids,
                        board_content: r.get(4)?,
                        private_notes,
                        active_private_room,
                        room_members,
                        compacted_summary: r.get(8)?,
                        compacted_until_seq: r.get(9)?,
                        total_tokens: r.get(10)?,
                        next_speaker: r.get(11)?,
                        created_at: r.get(12)?,
                    })
                },
            )
            .optional()?;
        Ok(res)
    }

    pub fn update_compacted_memory(
        &self,
        experiment_id: &str,
        summary: &str,
        until_seq: i64,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE experiments SET compacted_summary = ?1, compacted_until_seq = ?2, updated_at = ?3
             WHERE id = ?4",
            params![summary, until_seq, now, experiment_id],
        )?;
        Ok(())
    }

    pub fn update_shared_board(
        &self,
        experiment_id: &str,
        new_content: &str,
    ) -> Result<(String, String)> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        let prev: String = conn
            .query_row(
                "SELECT board_content FROM experiments WHERE id = ?1",
                params![experiment_id],
                |r| r.get(0),
            )
            .optional()?
            .context("experiment not found")?;
        conn.execute(
            "UPDATE experiments
             SET board_prev_content = ?1, board_content = ?2, updated_at = ?3
             WHERE id = ?4",
            params![prev, new_content, now, experiment_id],
        )?;
        Ok((prev, new_content.to_string()))
    }

    pub fn update_active_room(
        &self,
        experiment_id: &str,
        active_room: &Option<ActivePrivateRoom>,
        room_members: &HashMap<String, Vec<String>>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let active_room_json = match active_room {
            Some(r) => serde_json::to_string(r)?,
            None => String::new(),
        };
        let room_members_json = serde_json::to_string(room_members)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE experiments SET active_room_json = ?1, room_members_json = ?2, updated_at = ?3
             WHERE id = ?4",
            params![active_room_json, room_members_json, now, experiment_id],
        )?;
        Ok(())
    }

    fn seed_builtin_character_cards_conn(&self, conn: &Connection) -> Result<()> {
        for card in builtin_character_cards() {
            let imprints_json = serde_json::to_string(&card.memory_imprints)?;
            conn.execute(
                "INSERT OR IGNORE INTO character_cards (id, name, base_prompt, behavior_traits, memory_imprints_json, source_experiment_title, is_builtin, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    card.id,
                    card.name,
                    card.base_prompt,
                    card.behavior_traits,
                    imprints_json,
                    card.source_experiment_title,
                    if card.is_builtin { 1i64 } else { 0i64 },
                    card.created_at,
                ],
            )?;
        }
        Ok(())
    }

    pub fn list_character_cards(&self) -> Result<Vec<CharacterCard>> {
        let conn = self.conn.lock().unwrap();
        self.seed_builtin_character_cards_conn(&conn)?;
        let mut stmt = conn.prepare(
            "SELECT id, name, base_prompt, behavior_traits, memory_imprints_json, source_experiment_title, is_builtin, created_at
             FROM character_cards
             ORDER BY is_builtin ASC, created_at DESC, id ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            let imprints_str: String = r.get(4)?;
            let memory_imprints: Vec<String> =
                serde_json::from_str(&imprints_str).unwrap_or_default();
            Ok(CharacterCard {
                id: r.get(0)?,
                name: r.get(1)?,
                base_prompt: r.get(2)?,
                behavior_traits: r.get(3)?,
                memory_imprints,
                source_experiment_title: r.get(5)?,
                is_builtin: r.get::<_, i64>(6).unwrap_or(0) != 0,
                created_at: r.get(7)?,
            })
        })?;
        let mut items = Vec::new();
        for r in rows {
            items.push(r?);
        }
        Ok(items)
    }

    pub fn save_character_card(&self, card: &CharacterCard) -> Result<Vec<CharacterCard>> {
        let imprints_json = serde_json::to_string(&card.memory_imprints)?;
        {
            let conn = self.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO character_cards (id, name, base_prompt, behavior_traits, memory_imprints_json, source_experiment_title, is_builtin, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    base_prompt = excluded.base_prompt,
                    behavior_traits = excluded.behavior_traits,
                    memory_imprints_json = excluded.memory_imprints_json,
                    source_experiment_title = excluded.source_experiment_title,
                    is_builtin = excluded.is_builtin",
                params![
                    card.id,
                    card.name,
                    card.base_prompt,
                    card.behavior_traits,
                    imprints_json,
                    card.source_experiment_title,
                    if card.is_builtin { 1i64 } else { 0i64 },
                    card.created_at,
                ],
            )?;
        }
        self.list_character_cards()
    }

    pub fn update_character_card_imprints(
        &self,
        card_id: &str,
        memory_imprints: Vec<String>,
    ) -> Result<Vec<CharacterCard>> {
        let cleaned: Vec<String> = memory_imprints
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let imprints_json = serde_json::to_string(&cleaned)?;
        {
            let conn = self.conn.lock().unwrap();
            conn.execute(
                "UPDATE character_cards SET memory_imprints_json = ?1 WHERE id = ?2",
                params![imprints_json, card_id],
            )?;
        }
        self.list_character_cards()
    }

    pub fn delete_character_card(&self, id: &str) -> Result<Vec<CharacterCard>> {
        {
            let conn = self.conn.lock().unwrap();
            conn.execute(
                "DELETE FROM character_cards WHERE id = ?1 AND is_builtin = 0",
                params![id],
            )?;
        }
        self.list_character_cards()
    }

    pub fn update_agent_private_note(
        &self,
        experiment_id: &str,
        agent_id: &str,
        note: &str,
    ) -> Result<(String, HashMap<String, String>)> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        let raw_json: String = conn
            .query_row(
                "SELECT private_notes_json FROM experiments WHERE id = ?1",
                params![experiment_id],
                |r| r.get(0),
            )
            .optional()?
            .context("experiment not found")?;
        let mut map: HashMap<String, String> = serde_json::from_str(&raw_json).unwrap_or_default();
        let trimmed: String = note.trim().chars().take(150).collect();
        if trimmed.is_empty() {
            map.remove(agent_id);
        } else {
            map.insert(agent_id.to_string(), trimmed.clone());
        }
        let updated_json = serde_json::to_string(&map)?;
        conn.execute(
            "UPDATE experiments SET private_notes_json = ?1, updated_at = ?2 WHERE id = ?3",
            params![updated_json, now, experiment_id],
        )?;
        Ok((trimmed, map))
    }

    pub fn set_board_approval_mode(
        &self,
        experiment_id: &str,
        mode: BoardApprovalMode,
    ) -> Result<ExperimentConfig> {
        let mut detail = self.get_experiment_detail(experiment_id)?;
        detail.config.board_approval_mode = mode;
        self.update_experiment(experiment_id, &detail.title, &detail.config)?;
        Ok(detail.config)
    }

    pub fn update_experiment_progress(
        &self,
        experiment_id: &str,
        total_turns: u32,
        total_tokens: u64,
        next_speaker: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE experiments SET total_turns = ?1, total_tokens = ?2, next_speaker = ?3, updated_at = ?4
             WHERE id = ?5",
            params![total_turns, total_tokens, next_speaker, now, experiment_id],
        )?;
        Ok(())
    }

    pub fn mark_whispers_consumed(&self, message_ids: &[String]) -> Result<()> {
        if message_ids.is_empty() {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("UPDATE messages SET whisper_consumed = 1 WHERE id = ?1")?;
        for id in message_ids {
            stmt.execute(params![id])?;
        }
        Ok(())
    }

    pub fn save_final_report(&self, experiment_id: &str, report: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE experiments SET final_report = ?1, updated_at = ?2 WHERE id = ?3",
            params![report, now, experiment_id],
        )?;
        Ok(())
    }

    pub fn append_observer_note(&self, note: &ObserverNote) -> Result<()> {
        let tags_json = serde_json::to_string(&note.tags)?;
        let cards_json = serde_json::to_string(&note.crisis_cards)?;
        let matrix_json = serde_json::to_string(&note.relationship_matrix)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO observer_notes (
                id, experiment_id, trigger_turn, tags_json, depth_stage, narrative_act, note,
                tension_score, abstraction_score, stagnation_index, perturbation_injected,
                crisis_cards_json, relationship_matrix_json, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                note.id,
                note.experiment_id,
                note.trigger_turn,
                tags_json,
                note.depth_stage,
                note.narrative_act,
                note.note,
                note.tension_score as i64,
                note.abstraction_score as i64,
                note.stagnation_index as i64,
                if note.perturbation_injected { 1 } else { 0 },
                cards_json,
                matrix_json,
                note.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn export_markdown(&self, experiment_id: &str) -> Result<String> {
        let detail = self.get_experiment_detail(experiment_id)?;
        let mut out = String::new();

        out.push_str(&format!("# AI 生态观察箱实验档案：{}\n\n", detail.title));
        out.push_str(&format!("- **实验 ID**: `{}`\n", detail.id));
        out.push_str(&format!("- **创建时间**: {}\n", detail.created_at));
        out.push_str(&format!("- **总对话轮数**: {} 轮\n", detail.total_turns));
        out.push_str(&format!("- **累计 Token 消耗**: {}\n", detail.total_tokens));
        for (idx, agent) in detail.config.agents.iter().enumerate() {
            out.push_str(&format!(
                "- **智能体 #{} ({})**: `{}` (Temp: {:.2})\n",
                idx + 1,
                agent.label,
                agent.model_name,
                agent.temperature
            ));
        }
        out.push_str(&format!(
            "- **观察员 C**: `{}` (每 {} 轮触发)\n\n",
            detail.config.node_c.model_name, detail.config.observer_interval
        ));
        if !detail.board_content.trim().is_empty() {
            out.push_str("---\n\n## 实验室共享白板\n\n");
            out.push_str(detail.board_content.trim());
            out.push_str("\n\n");
        }
        let mut note_entries: Vec<(String, String)> = Vec::new();
        for agent in &detail.config.agents {
            if let Some(note) = detail.private_notes.get(&agent.id) {
                let trimmed = note.trim();
                if !trimmed.is_empty() {
                    note_entries.push((agent.label.clone(), trimmed.to_string()));
                }
            }
        }
        let known_ids: std::collections::HashSet<&str> =
            detail.config.agents.iter().map(|a| a.id.as_str()).collect();
        let mut extra_notes: Vec<(&String, &String)> = detail
            .private_notes
            .iter()
            .filter(|(k, v)| !known_ids.contains(k.as_str()) && !v.trim().is_empty())
            .collect();
        extra_notes.sort_by(|a, b| a.0.cmp(b.0));
        for (id, note) in extra_notes {
            note_entries.push((id.clone(), note.trim().to_string()));
        }
        if !note_entries.is_empty() {
            out.push_str("---\n\n## 角色私密备忘\n\n");
            for (label, note) in note_entries {
                out.push_str(&format!("- **{}**: {}\n", label, note));
            }
            out.push('\n');
        }
        out.push_str("---\n\n## 对话演化与干预实录\n\n");

        let mut notes_sorted = detail.observer_notes.clone();
        notes_sorted.sort_by_key(|n| n.trigger_turn);
        let mut note_idx = 0usize;

        for msg in &detail.messages {
            match msg.role {
                SpeakerRole::Ai => {
                    let label = detail.config.agent_label(msg.agent_id.as_deref());
                    out.push_str(&format!(
                        "### [第 {} 轮] {} (`{}` · {:.1}s · {} tok)\n\n",
                        msg.turn_number,
                        label,
                        msg.model_name,
                        msg.duration_ms as f64 / 1000.0,
                        msg.total_tokens
                    ));
                    if !msg.reasoning_content.trim().is_empty() {
                        out.push_str(&format!(
                            "<details>\n<summary>幕后心声（思考过程）</summary>\n\n{}\n\n</details>\n\n",
                            msg.reasoning_content.trim()
                        ));
                    }
                    out.push_str(&format!("{}\n\n", msg.content));
                }
                SpeakerRole::GodSystem => {
                    out.push_str(&format!(
                        "> 🌐 **[造物主 · 环境旁白]**: {}\n\n",
                        msg.content
                    ));
                }
                SpeakerRole::GodPublic => {
                    out.push_str(&format!(
                        "> 👤 **[造物主 · 路人降临]**: {}\n\n",
                        msg.content
                    ));
                }
                SpeakerRole::GodWhisper => {
                    let target = detail.config.agent_label(msg.agent_id.as_deref());
                    out.push_str(&format!(
                        "> 🔮 **[造物主 · 心灵低语 → {}]**: {}\n\n",
                        target, msg.content
                    ));
                }
            }

            if msg.role.is_ai() {
                while note_idx < notes_sorted.len()
                    && notes_sorted[note_idx].trigger_turn <= msg.turn_number
                {
                    let n = &notes_sorted[note_idx];
                    out.push_str(&format!(
                        "#### 🔭 观察员手记（第 {} 轮 · {} · 火药味 {} · 抽象度 {} · 僵化指数 {}/100{}）\n",
                        n.trigger_turn,
                        n.depth_stage,
                        n.tension_score,
                        n.abstraction_score,
                        n.stagnation_index,
                        if n.perturbation_injected {
                            " · 已自动注入破局扰动"
                        } else {
                            ""
                        }
                    ));
                    if !n.tags.is_empty() {
                        out.push_str(&format!("- **核心话题**: {}\n", n.tags.join(" / ")));
                    }
                    out.push_str(&format!("- **观察手记**: {}\n\n", n.note));
                    note_idx += 1;
                }
            }
        }

        while note_idx < notes_sorted.len() {
            let n = &notes_sorted[note_idx];
            out.push_str(&format!(
                "#### 🔭 观察员手记（第 {} 轮 · {} · 火药味 {} · 抽象度 {} · 僵化指数 {}/100）\n- **核心话题**: {}\n- **观察手记**: {}\n\n",
                n.trigger_turn,
                n.depth_stage,
                n.tension_score,
                n.abstraction_score,
                n.stagnation_index,
                n.tags.join(" / "),
                n.note
            ));
            note_idx += 1;
        }

        if !detail.final_report.trim().is_empty() {
            out.push_str("---\n\n## 观察员结案复盘简报\n\n");
            out.push_str(detail.final_report.trim());
            out.push_str("\n\n");
        }

        Ok(out)
    }

    /// 导出「纯享剧本模式」Markdown：剥离所有 `<think>` 幕后心声、Token 统计与调试元数据，
    /// 仅保留剧本标题、定调标签、角色台词（干净剧本对白排版）、环境旁白、私密里屋标记与观察员阶段手记。
    pub fn export_pure_script_markdown(&self, experiment_id: &str) -> Result<String> {
        let detail = self.get_experiment_detail(experiment_id)?;
        let mut out = String::new();

        out.push_str(&format!("# {}\n\n", detail.title));
        if !detail.config.tags.is_empty() {
            out.push_str(&format!(
                "> **定调标签**：{}\n\n",
                detail.config.tags.join(" · ")
            ));
        }

        let cast: Vec<String> = detail
            .config
            .agents
            .iter()
            .map(|a| format!("**{}**", a.label))
            .collect();
        if !cast.is_empty() {
            out.push_str(&format!("**参演角色**：{}\n\n", cast.join(" / ")));
        }

        if !detail.board_content.trim().is_empty() {
            out.push_str("---\n\n## 现场线索与共识\n\n");
            out.push_str(detail.board_content.trim());
            out.push_str("\n\n");
        }

        out.push_str("---\n\n## 剧本正文\n\n");

        let mut notes_sorted = detail.observer_notes.clone();
        notes_sorted.sort_by_key(|n| n.trigger_turn);
        let mut note_idx = 0usize;
        let mut last_room = "main".to_string();

        for msg in &detail.messages {
            let clean_body = strip_think_tags(&msg.content);
            match msg.role {
                SpeakerRole::Ai => {
                    if msg.room_id != "main" && msg.room_id != last_room {
                        let members_desc = detail
                            .room_members
                            .get(&msg.room_id)
                            .map(|ids| {
                                ids.iter()
                                    .map(|id| detail.config.agent_label(Some(id)))
                                    .collect::<Vec<_>>()
                                    .join(" × ")
                            })
                            .unwrap_or_else(|| "里屋密谈".to_string());
                        out.push_str(&format!("> 🔒 **【私密里屋 · {}】**\n\n", members_desc));
                    }
                    last_room = msg.room_id.clone();

                    let label = detail.config.agent_label(msg.agent_id.as_deref());
                    let room_tag = if msg.room_id != "main" {
                        " *(里屋)*"
                    } else {
                        ""
                    };
                    out.push_str(&format!("**{}{}**：\n{}\n\n", label, room_tag, clean_body));
                }
                SpeakerRole::GodSystem => {
                    if is_pass_notice(msg) {
                        out.push_str(&format!("> *{}*\n\n", clean_body));
                    } else {
                        out.push_str(&format!("> 🌐 **【环境旁白】** {}\n\n", clean_body));
                    }
                }
                SpeakerRole::GodPublic => {
                    out.push_str(&format!("> 👤 **【路人】** {}\n\n", clean_body));
                }
                SpeakerRole::GodWhisper => {
                    // 纯享剧本模式不暴露造物主后台低语或调试提示
                }
            }

            if msg.role.is_ai() {
                while note_idx < notes_sorted.len()
                    && notes_sorted[note_idx].trigger_turn <= msg.turn_number
                {
                    let n = &notes_sorted[note_idx];
                    let act = if n.narrative_act.trim().is_empty() {
                        String::new()
                    } else {
                        format!("{} · ", n.narrative_act.trim())
                    };
                    out.push_str(&format!(
                        "---\n\n> 🔭 **观察员阶段手记**（第 {} 轮 · {}{}）：{}\n\n",
                        n.trigger_turn, act, n.depth_stage, n.note
                    ));
                    note_idx += 1;
                }
            }
        }

        while note_idx < notes_sorted.len() {
            let n = &notes_sorted[note_idx];
            out.push_str(&format!(
                "---\n\n> 🔭 **观察员阶段手记**（第 {} 轮 · {}）：{}\n\n",
                n.trigger_turn, n.depth_stage, n.note
            ));
            note_idx += 1;
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ExperimentConfig;

    #[test]
    fn test_sqlite_experiment_lifecycle_and_markdown_export() {
        let db = Database::open_in_memory().unwrap();
        let exp = db.ensure_default_experiment().unwrap();
        assert_eq!(exp.total_turns, 0);
        assert_eq!(exp.config.agents.len(), 2);

        let msg1 = TimelineMessage {
            id: "m1".to_string(),
            experiment_id: exp.id.clone(),
            seq: 1,
            turn_number: 1,
            role: SpeakerRole::Ai,
            agent_id: Some("AiA".to_string()),
            room_id: "main".to_string(),
            reasoning_content: "先构思一个流动的意象。".to_string(),
            content: "时间是一条不回头的河。".to_string(),
            model_name: "deepseek-chat".to_string(),
            duration_ms: 1200,
            prompt_tokens: 100,
            completion_tokens: 20,
            total_tokens: 120,
            whisper_consumed: false,
            created_at: Utc::now().to_rfc3339(),
        };
        db.append_message(&msg1).unwrap();
        db.update_experiment_progress(&exp.id, 1, 120, "AiB")
            .unwrap();

        let note = ObserverNote {
            id: "n1".to_string(),
            experiment_id: exp.id.clone(),
            trigger_turn: 1,
            tags: vec!["时间".to_string(), "隐喻".to_string()],
            depth_stage: "破冰试探".to_string(),
            narrative_act: "起".to_string(),
            note: "开局直接抛出了强意象。".to_string(),
            tension_score: 45,
            abstraction_score: 70,
            stagnation_index: 15,
            perturbation_injected: false,
            crisis_cards: vec![],
            relationship_matrix: vec![RelationshipEdge {
                from: "智能体 1".to_string(),
                to: "智能体 2".to_string(),
                trust_score: 35,
                attitude_tag: "谨慎试探".to_string(),
            }],
            created_at: Utc::now().to_rfc3339(),
        };
        db.append_observer_note(&note).unwrap();

        let md = db.export_markdown(&exp.id).unwrap();
        assert!(md.contains("时间是一条不回头的河。"));
        assert!(md.contains("观察员手记"));
        assert!(md.contains("破冰试探"));

        let pure_md = db.export_pure_script_markdown(&exp.id).unwrap();
        assert!(pure_md.contains("时间是一条不回头的河。"));
        assert!(pure_md.contains("观察员阶段手记"));
        assert!(!pure_md.contains("先构思一个流动的意象"));
        assert!(!pure_md.contains("120 tok"));
        assert!(!pure_md.contains("deepseek-chat"));

        let exp2 = db
            .create_experiment("测试实验2", &ExperimentConfig::default())
            .unwrap();
        assert_eq!(db.list_experiments().unwrap().len(), 2);

        // Private notes persistence & 150-char clamp
        let long_note = "记".repeat(170);
        let (saved_note, map) = db
            .update_agent_private_note(&exp.id, "AiA", &long_note)
            .unwrap();
        assert_eq!(saved_note.chars().count(), 150);
        assert_eq!(map.get("AiA").unwrap().chars().count(), 150);
        let reloaded = db.get_experiment_detail(&exp.id).unwrap();
        assert_eq!(
            reloaded.private_notes.get("AiA").unwrap().chars().count(),
            150
        );
        assert_eq!(reloaded.observer_notes[0].relationship_matrix.len(), 1);
        assert_eq!(reloaded.observer_notes[0].relationship_matrix[0].trust_score, 35);
        let md_with_notes = db.export_markdown(&exp.id).unwrap();
        assert!(md_with_notes.contains("## 角色私密备忘"));
        assert!(md_with_notes.contains("记"));

        // Character cards CRUD & memory imprints
        let card = CharacterCard {
            id: "card-1".to_string(),
            name: "阿哲".to_string(),
            base_prompt: "便利店夜班店员".to_string(),
            behavior_traits: "嘴硬心细、善于反套话".to_string(),
            memory_imprints: vec!["曾在暴雨夜藏过黑背包".to_string()],
            source_experiment_title: exp.title.clone(),
            is_builtin: false,
            created_at: Utc::now().to_rfc3339(),
        };
        let cards = db.save_character_card(&card).unwrap();
        assert_eq!(cards.len(), 17);
        assert_eq!(cards[0].name, "阿哲");
        assert_eq!(cards[0].memory_imprints.len(), 1);
        let updated_imprints = db
            .update_character_card_imprints(
                "card-1",
                vec!["烙印1".to_string(), "烙印2".to_string()],
            )
            .unwrap();
        assert_eq!(updated_imprints[0].memory_imprints, vec!["烙印1", "烙印2"]);
        let after_del = db.delete_character_card("card-1").unwrap();
        assert_eq!(after_del.len(), 16);
        let after_builtin_del = db
            .delete_character_card("builtin-char-01-wang-jianguo")
            .unwrap();
        assert_eq!(after_builtin_del.len(), 16);

        db.delete_experiment(&exp2.id).unwrap();
        assert_eq!(db.list_experiments().unwrap().len(), 1);
    }

    #[test]
    fn test_legacy_v2_builtin_presets_migrate_cleanly_to_v4() {
        let db = Database::open_in_memory().unwrap();
        {
            let conn = db.conn.lock().unwrap();
            conn.execute("DELETE FROM preset_templates", []).unwrap();
            conn.execute(
                "DELETE FROM app_settings WHERE key = ?1",
                params![BUILTIN_PRESETS_VERSION_KEY],
            )
            .unwrap();
            let old_philosophy = PresetTemplate {
                id: "philosophy".to_string(),
                name: "哲学思辨局".to_string(),
                description: "old".to_string(),
                global_system_prompt: "【场景背景】高密度哲学思辨圆桌。\nold".to_string(),
                initial_topic: String::new(),
                tags: vec![],
                agent_prompts: vec![],
                is_builtin: true,
                recommended_config: None,
                node_a_label: "感性直觉派".to_string(),
                node_a_prompt: "old".to_string(),
                node_b_label: "理性解构派".to_string(),
                node_b_prompt: "old".to_string(),
            };
            conn.execute(
                "INSERT INTO preset_templates (id, sort_order, data_json, updated_at)
                 VALUES (?1, 2, ?2, '2026-01-01T00:00:00Z')",
                params![
                    "philosophy",
                    serde_json::to_string(&old_philosophy).unwrap()
                ],
            )
            .unwrap();
        }

        db.init_schema().unwrap();
        let migrated = db.list_presets().unwrap();
        assert_eq!(migrated.len(), 12);
        assert!(!migrated.iter().any(|p| p.id == "philosophy"));
        assert!(migrated.iter().any(|p| p.id == "late_night_friends"));
        assert!(migrated.iter().any(|p| p.id == "ai_night_slacking"));
        assert!(migrated.iter().any(|p| p.id == "green_train_sleeper"));
        assert!(migrated.iter().any(|p| p.id == "neutrino_anomaly"));
        assert!(migrated.iter().any(|p| p.id == "heat_death_ark"));
    }
}
