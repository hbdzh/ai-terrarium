use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::models::{
    builtin_presets, BoardApprovalMode, CrisisCard, ExperimentConfig, ExperimentDetail,
    ExperimentSummary, ObserverNote, PresetTemplate, SpeakerRole, TimelineMessage, TurnSnapshot,
};

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
            ",
        )?;

        // Seed built-in presets if not yet persisted
        let now = Utc::now().to_rfc3339();
        for (idx, preset) in builtin_presets().into_iter().enumerate() {
            let data_json = serde_json::to_string(&preset)?;
            conn.execute(
                "INSERT OR IGNORE INTO preset_templates (id, sort_order, data_json, updated_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![preset.id, idx as i64, data_json, now],
            )?;
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
        Ok(())
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
                    let max_o: Option<i64> = conn.query_row(
                        "SELECT MAX(sort_order) FROM preset_templates",
                        [],
                        |r| r.get(0),
                    )?;
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

    pub fn create_experiment(&self, title: &str, config: &ExperimentConfig) -> Result<ExperimentDetail> {
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
            "SELECT id, title, total_turns, total_tokens, next_speaker, created_at, updated_at
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
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
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
                        final_report, created_at, updated_at
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
                    ))
                },
            )
            .optional()?
            .context("experiment not found")?;

        let mut config: ExperimentConfig = serde_json::from_str(&row.2).unwrap_or_default();
        config.normalize();

        let mut msg_stmt = conn.prepare(
            "SELECT id, experiment_id, seq, turn_number, role, agent_id, reasoning_content, content, model_name, duration_ms,
                    prompt_tokens, completion_tokens, total_tokens, whisper_consumed, created_at
             FROM messages
             WHERE experiment_id = ?1
             ORDER BY seq ASC",
        )?;
        let msg_rows = msg_stmt.query_map(params![id], |r| {
            let role_str: String = r.get(4)?;
            let agent_id_col: String = r.get(5)?;
            let (role, agent_id) = SpeakerRole::from_db(&role_str, &agent_id_col);
            Ok(TimelineMessage {
                id: r.get(0)?,
                experiment_id: r.get(1)?,
                seq: r.get(2)?,
                turn_number: r.get(3)?,
                role,
                agent_id,
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
                    crisis_cards_json, created_at
             FROM observer_notes
             WHERE experiment_id = ?1
             ORDER BY trigger_turn DESC, created_at DESC",
        )?;
        let obs_rows = obs_stmt.query_map(params![id], |r| {
            let tags_str: String = r.get(3)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            let cards_str: String = r.get(10)?;
            let crisis_cards: Vec<CrisisCard> = serde_json::from_str(&cards_str).unwrap_or_default();
            Ok(ObserverNote {
                id: r.get(0)?,
                experiment_id: r.get(1)?,
                trigger_turn: r.get(2)?,
                tags,
                depth_stage: r.get(4)?,
                note: r.get(5)?,
                tension_score: r.get::<_, i64>(6)?.clamp(0, 100) as u8,
                abstraction_score: r.get::<_, i64>(7)?.clamp(0, 100) as u8,
                stagnation_index: r.get::<_, i64>(8)?.clamp(0, 100) as u8,
                perturbation_injected: r.get::<_, i64>(9)? != 0,
                crisis_cards,
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
            board_content: row.6,
            board_prev_content: row.7,
            compacted_summary: row.8,
            compacted_until_seq: row.9,
            final_report: row.10,
            messages,
            observer_notes,
            created_at: row.11,
            updated_at: row.12,
        })
    }

    pub fn update_experiment(&self, id: &str, title: &str, config: &ExperimentConfig) -> Result<()> {
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
        conn.execute("DELETE FROM experiments WHERE id = ?1", params![id])?;
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
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO messages (
                id, experiment_id, seq, turn_number, role, agent_id, reasoning_content, content, model_name, duration_ms,
                prompt_tokens, completion_tokens, total_tokens, whisper_consumed, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                msg.id,
                msg.experiment_id,
                msg.seq,
                msg.turn_number,
                msg.role.as_str(),
                msg.agent_id.as_deref().unwrap_or(""),
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

    pub fn update_experiment_state_for_fork(
        &self,
        experiment_id: &str,
        total_turns: u32,
        total_tokens: u64,
        next_speaker: &str,
        board_content: &str,
        compacted_summary: &str,
        compacted_until_seq: i64,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE experiments
             SET total_turns = ?1, total_tokens = ?2, next_speaker = ?3,
                 board_content = ?4, board_prev_content = ?4,
                 compacted_summary = ?5, compacted_until_seq = ?6, updated_at = ?7
             WHERE id = ?8",
            params![
                total_turns,
                total_tokens,
                next_speaker,
                board_content,
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
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO turn_snapshots (
                id, experiment_id, turn_number, message_ids_json, board_content,
                compacted_summary, compacted_until_seq, total_tokens, next_speaker, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(experiment_id, turn_number) DO UPDATE SET
                id = excluded.id,
                message_ids_json = excluded.message_ids_json,
                board_content = excluded.board_content,
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
            "SELECT id, experiment_id, turn_number, message_ids_json, board_content,
                    compacted_summary, compacted_until_seq, total_tokens, next_speaker, created_at
             FROM turn_snapshots
             WHERE experiment_id = ?1
             ORDER BY turn_number ASC",
        )?;
        let rows = stmt.query_map(params![experiment_id], |r| {
            let ids_str: String = r.get(3)?;
            let message_ids: Vec<String> = serde_json::from_str(&ids_str).unwrap_or_default();
            Ok(TurnSnapshot {
                id: r.get(0)?,
                experiment_id: r.get(1)?,
                turn_number: r.get(2)?,
                message_ids,
                board_content: r.get(4)?,
                compacted_summary: r.get(5)?,
                compacted_until_seq: r.get(6)?,
                total_tokens: r.get(7)?,
                next_speaker: r.get(8)?,
                created_at: r.get(9)?,
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
                "SELECT id, experiment_id, turn_number, message_ids_json, board_content,
                        compacted_summary, compacted_until_seq, total_tokens, next_speaker, created_at
                 FROM turn_snapshots
                 WHERE experiment_id = ?1 AND turn_number = ?2",
                params![experiment_id, turn_number],
                |r| {
                    let ids_str: String = r.get(3)?;
                    let message_ids: Vec<String> =
                        serde_json::from_str(&ids_str).unwrap_or_default();
                    Ok(TurnSnapshot {
                        id: r.get(0)?,
                        experiment_id: r.get(1)?,
                        turn_number: r.get(2)?,
                        message_ids,
                        board_content: r.get(4)?,
                        compacted_summary: r.get(5)?,
                        compacted_until_seq: r.get(6)?,
                        total_tokens: r.get(7)?,
                        next_speaker: r.get(8)?,
                        created_at: r.get(9)?,
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
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO observer_notes (
                id, experiment_id, trigger_turn, tags_json, depth_stage, note,
                tension_score, abstraction_score, stagnation_index, perturbation_injected,
                crisis_cards_json, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                note.id,
                note.experiment_id,
                note.trigger_turn,
                tags_json,
                note.depth_stage,
                note.note,
                note.tension_score as i64,
                note.abstraction_score as i64,
                note.stagnation_index as i64,
                if note.perturbation_injected { 1 } else { 0 },
                cards_json,
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
                    out.push_str(&format!("> 🌐 **[造物主 · 环境旁白]**: {}\n\n", msg.content));
                }
                SpeakerRole::GodPublic => {
                    out.push_str(&format!("> 👤 **[造物主 · 路人降临]**: {}\n\n", msg.content));
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
        db.update_experiment_progress(&exp.id, 1, 120, "AiB").unwrap();

        let note = ObserverNote {
            id: "n1".to_string(),
            experiment_id: exp.id.clone(),
            trigger_turn: 1,
            tags: vec!["时间".to_string(), "隐喻".to_string()],
            depth_stage: "破冰试探".to_string(),
            note: "开局直接抛出了强意象。".to_string(),
            tension_score: 45,
            abstraction_score: 70,
            stagnation_index: 15,
            perturbation_injected: false,
            crisis_cards: vec![],
            created_at: Utc::now().to_rfc3339(),
        };
        db.append_observer_note(&note).unwrap();

        let md = db.export_markdown(&exp.id).unwrap();
        assert!(md.contains("时间是一条不回头的河。"));
        assert!(md.contains("观察员手记"));
        assert!(md.contains("破冰试探"));

        let exp2 = db
            .create_experiment("测试实验2", &ExperimentConfig::default())
            .unwrap();
        assert_eq!(db.list_experiments().unwrap().len(), 2);
        db.delete_experiment(&exp2.id).unwrap();
        assert_eq!(db.list_experiments().unwrap().len(), 1);
    }
}
