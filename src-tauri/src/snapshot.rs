use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use uuid::Uuid;

use crate::db::Database;
use crate::models::{ExperimentDetail, SpeakerRole, TurnSnapshot};

/// 在每轮 AI 发言结束后捕获该轮全量状态快照，供时间线回放与平行宇宙分叉使用。
pub fn capture_turn_snapshot(
    db: &Database,
    experiment_id: &str,
    turn_number: u32,
) -> Result<TurnSnapshot> {
    let snap = db.get_snapshot_source(experiment_id, turn_number)?;
    db.save_turn_snapshot(&snap)?;
    Ok(snap)
}

/// 从指定历史轮次克隆全新平行宇宙实验分支。
/// 支持可选的 `edit_override = Some((message_id, new_content))` 实现“修改内容并重演”。
#[allow(dead_code)]
pub fn fork_experiment_at_turn(
    db: &Database,
    source_exp_id: &str,
    target_turn: u32,
    edit_override: Option<(String, String)>,
) -> Result<ExperimentDetail> {
    fork_experiment_at_turn_or_message(db, source_exp_id, target_turn, None, edit_override)
}

/// 从指定历史轮次或具体某条消息为止克隆全新平行宇宙实验分支，记录同源谱系 (`parent_experiment_id`, `fork_turn`)。
pub fn fork_experiment_at_turn_or_message(
    db: &Database,
    source_exp_id: &str,
    target_turn: u32,
    target_message_id: Option<String>,
    edit_override: Option<(String, String)>,
) -> Result<ExperimentDetail> {
    let source = db.get_experiment_detail(source_exp_id)?;
    let target_msg_opt = target_message_id
        .as_deref()
        .and_then(|mid| source.messages.iter().find(|m| m.id == mid));

    let effective_turn = target_msg_opt
        .map(|m| m.turn_number)
        .unwrap_or(target_turn);

    let target_snap = db.get_turn_snapshot(source_exp_id, effective_turn)?;
    let all_source_snaps = db.get_turn_snapshots(source_exp_id)?;

    let mut cloned_msgs = if let Some(target_msg) = target_msg_opt {
        source
            .messages
            .iter()
            .filter(|m| m.seq <= target_msg.seq)
            .cloned()
            .collect()
    } else if let Some(ref snap) = target_snap {
        let id_set: HashSet<&str> = snap.message_ids.iter().map(|s| s.as_str()).collect();
        let filtered: Vec<_> = source
            .messages
            .iter()
            .filter(|m| id_set.contains(m.id.as_str()) || m.turn_number <= effective_turn)
            .cloned()
            .collect();
        if filtered.is_empty() {
            source
                .messages
                .iter()
                .filter(|m| m.turn_number <= effective_turn)
                .cloned()
                .collect()
        } else {
            filtered
        }
    } else {
        source
            .messages
            .iter()
            .filter(|m| m.turn_number <= effective_turn)
            .cloned()
            .collect()
    };

    let mut next_speaker = if let Some(target_msg) = target_msg_opt {
        if target_msg.role == SpeakerRole::Ai {
            next_agent_after(&source, target_msg.agent_id.as_deref())
        } else {
            compute_next_speaker_from_messages(&source, &cloned_msgs)
        }
    } else {
        target_snap
            .as_ref()
            .map(|s| s.next_speaker.clone())
            .unwrap_or_else(|| compute_next_speaker_from_messages(&source, &cloned_msgs))
    };

    let is_edit_replay = edit_override.is_some();
    if let Some((edit_msg_id, new_content)) = edit_override {
        let trimmed = new_content.trim().to_string();
        if !trimmed.is_empty() {
            if let Some(target_msg) = cloned_msgs.iter_mut().find(|m| m.id == edit_msg_id) {
                target_msg.content = trimmed;
                target_msg.reasoning_content.clear();
                if target_msg.role == SpeakerRole::Ai {
                    next_speaker = next_agent_after(&source, target_msg.agent_id.as_deref());
                }
            }
        }
    }

    let existing_list = db.list_experiments().unwrap_or_default();
    let existing_titles: HashSet<String> =
        existing_list.iter().map(|e| e.title.clone()).collect();
    let sibling_count = existing_list
        .iter()
        .filter(|e| e.parent_experiment_id.as_deref() == Some(source_exp_id))
        .count();

    let base_title = source.title.trim();
    let (suffix, new_title) = if is_edit_replay {
        let s = format!("改写重演 #{effective_turn}");
        let t = format!("{base_title} · {s}");
        (s, t)
    } else {
        let mut idx = sibling_count;
        loop {
            let letter = ((b'A' + (idx % 26) as u8) as char).to_string();
            let round_num = idx / 26;
            let s = if round_num == 0 {
                format!("分支{letter}")
            } else {
                format!("分支{letter}{}", round_num + 1)
            };
            let candidate = format!("{base_title} · {s}");
            if !existing_titles.contains(&candidate) {
                break (s, candidate);
            }
            idx += 1;
        }
    };
    let _ = suffix;

    let board_content = target_snap
        .as_ref()
        .map(|s| s.board_content.clone())
        .unwrap_or_else(|| source.board_content.clone());
    let private_notes = target_snap
        .as_ref()
        .map(|s| s.private_notes.clone())
        .unwrap_or_default();
    let active_private_room = target_snap
        .as_ref()
        .and_then(|s| s.active_private_room.clone());
    let room_members = target_snap
        .as_ref()
        .map(|s| s.room_members.clone())
        .unwrap_or_else(|| source.room_members.clone());
    let compacted_summary = target_snap
        .as_ref()
        .map(|s| s.compacted_summary.clone())
        .unwrap_or_else(|| {
            if effective_turn >= source.total_turns {
                source.compacted_summary.clone()
            } else {
                String::new()
            }
        });
    let compacted_until_seq = target_snap
        .as_ref()
        .map(|s| s.compacted_until_seq)
        .unwrap_or_else(|| {
            if effective_turn >= source.total_turns {
                source.compacted_until_seq
            } else {
                0
            }
        });
    let total_tokens = cloned_msgs.iter().map(|m| m.total_tokens).sum::<u64>();
    let total_tokens = if total_tokens > 0 {
        total_tokens
    } else {
        target_snap
            .as_ref()
            .map(|s| s.total_tokens)
            .unwrap_or(0)
    };

    let new_exp = db.create_experiment(&new_title, &source.config)?;
    let new_exp_id = new_exp.id.clone();

    // 映射旧消息 ID -> 新消息 ID，保证快照引用完整性
    let mut id_map: HashMap<String, String> = HashMap::with_capacity(cloned_msgs.len());
    for mut m in cloned_msgs {
        let old_id = m.id.clone();
        let new_id = Uuid::new_v4().to_string();
        id_map.insert(old_id, new_id.clone());
        m.id = new_id;
        m.experiment_id = new_exp_id.clone();
        db.append_message(&m).context("克隆分支消息失败")?;
    }

    for mut note in source
        .observer_notes
        .into_iter()
        .filter(|n| n.trigger_turn <= effective_turn)
    {
        note.id = Uuid::new_v4().to_string();
        note.experiment_id = new_exp_id.clone();
        db.append_observer_note(&note)?;
    }

    db.update_experiment_state_for_fork(
        &new_exp_id,
        effective_turn,
        total_tokens,
        &next_speaker,
        &board_content,
        &private_notes,
        &active_private_room,
        &room_members,
        &compacted_summary,
        compacted_until_seq,
    )?;
    db.set_experiment_lineage(&new_exp_id, source_exp_id, effective_turn)?;

    for mut snap in all_source_snaps
        .into_iter()
        .filter(|s| s.turn_number <= effective_turn)
    {
        snap.id = Uuid::new_v4().to_string();
        snap.experiment_id = new_exp_id.clone();
        snap.message_ids = snap
            .message_ids
            .into_iter()
            .filter_map(|old| id_map.get(&old).cloned())
            .collect();
        if snap.turn_number == effective_turn {
            snap.next_speaker = next_speaker.clone();
        }
        db.save_turn_snapshot(&snap)?;
    }

    db.get_experiment_detail(&new_exp_id)
}

fn next_agent_after(source: &ExperimentDetail, last_agent_id: Option<&str>) -> String {
    if source.config.agents.is_empty() {
        return "AiA".to_string();
    }
    if let Some(last_id) = last_agent_id {
        if let Some(pos) = source.config.agents.iter().position(|a| a.id == last_id) {
            let next = (pos + 1) % source.config.agents.len();
            return source.config.agents[next].id.clone();
        }
    }
    source.config.agents[0].id.clone()
}

fn compute_next_speaker_from_messages(
    source: &ExperimentDetail,
    msgs: &[crate::models::TimelineMessage],
) -> String {
    let last_ai = msgs.iter().rev().find(|m| m.role == SpeakerRole::Ai);
    next_agent_after(source, last_ai.and_then(|m| m.agent_id.as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ExperimentConfig, TimelineMessage};
    use chrono::Utc;

    #[test]
    fn test_snapshot_and_fork_at_turn_with_edit() {
        let db = Database::open_in_memory().unwrap();
        let exp = db
            .create_experiment("主宇宙", &ExperimentConfig::default())
            .unwrap();

        for i in 1..=3 {
            let aid = if i % 2 == 1 { "AiA" } else { "AiB" };
            let next_sp = if i % 2 == 1 { "AiB" } else { "AiA" };
            let msg = TimelineMessage {
                id: format!("msg-{i}"),
                experiment_id: exp.id.clone(),
                seq: i,
                turn_number: i as u32,
                role: SpeakerRole::Ai,
                agent_id: Some(aid.to_string()),
                room_id: "main".to_string(),
                reasoning_content: "思考".to_string(),
                content: format!("发言 {i}"),
                micro_cue: None,
                model_name: "test".to_string(),
                duration_ms: 100,
                prompt_tokens: 10,
                completion_tokens: 10,
                total_tokens: 20,
                whisper_consumed: false,
                created_at: Utc::now().to_rfc3339(),
            };
            db.append_message(&msg).unwrap();
            db.update_experiment_progress(&exp.id, i as u32, (i * 20) as u64, next_sp)
                .unwrap();
            capture_turn_snapshot(&db, &exp.id, i as u32).unwrap();
        }

        let forked = fork_experiment_at_turn(
            &db,
            &exp.id,
            2,
            Some(("msg-2".to_string(), "改写后的第2轮发言".to_string())),
        )
        .unwrap();

        assert_eq!(forked.total_turns, 2);
        assert_eq!(forked.messages.len(), 2);
        assert_eq!(forked.messages[1].content, "改写后的第2轮发言");
        assert!(forked.messages[1].reasoning_content.is_empty());
        assert_eq!(forked.next_speaker, "AiA");
        assert_eq!(forked.total_tokens, 40);
        let snaps = db.get_turn_snapshots(&forked.id).unwrap();
        assert_eq!(snaps.len(), 2);
    }

    #[test]
    fn test_fork_without_snapshot_isolates_private_notes_and_room() {
        let db = Database::open_in_memory().unwrap();
        let exp = db
            .create_experiment("无快照测试", &ExperimentConfig::default())
            .unwrap();

        let msg1 = TimelineMessage {
            id: "m1".to_string(),
            experiment_id: exp.id.clone(),
            seq: 1,
            turn_number: 1,
            role: SpeakerRole::Ai,
            agent_id: Some("AiA".to_string()),
            room_id: "main".to_string(),
            reasoning_content: String::new(),
            content: "第1轮".to_string(),
            micro_cue: None,
            model_name: "test".to_string(),
            duration_ms: 50,
            prompt_tokens: 10,
            completion_tokens: 10,
            total_tokens: 20,
            whisper_consumed: false,
            created_at: Utc::now().to_rfc3339(),
        };
        db.append_message(&msg1).unwrap();

        // 模拟后续轮次写入了私密备忘（但第1轮没有快照）
        let mut notes = HashMap::new();
        notes.insert("AiA".to_string(), "未来轮次的秘密".to_string());
        db.update_experiment_state_for_fork(
            &exp.id,
            2,
            40,
            "AiB",
            "",
            &notes,
            &None,
            &HashMap::new(),
            "",
            0,
        )
        .unwrap();

        let forked = fork_experiment_at_turn(&db, &exp.id, 1, None).unwrap();
        assert!(forked.private_notes.is_empty());
        assert!(forked.active_private_room.is_none());
        assert_eq!(forked.parent_experiment_id.as_deref(), Some(exp.id.as_str()));
        assert_eq!(forked.fork_turn, Some(1));
        assert!(forked.title.contains("分支A"));
    }

    #[test]
    fn test_fork_at_specific_message_slices_exact_seq_and_increments_branch_letter() {
        let db = Database::open_in_memory().unwrap();
        let exp = db
            .create_experiment("深夜便利店 #27", &ExperimentConfig::default())
            .unwrap();
        let m1 = TimelineMessage {
            id: "msg-a".to_string(),
            experiment_id: exp.id.clone(),
            seq: 1,
            turn_number: 1,
            role: SpeakerRole::Ai,
            agent_id: Some("AiA".to_string()),
            room_id: "main".to_string(),
            reasoning_content: String::new(),
            content: "第一句".to_string(),
            micro_cue: None,
            model_name: "test".to_string(),
            duration_ms: 50,
            prompt_tokens: 10,
            completion_tokens: 10,
            total_tokens: 20,
            whisper_consumed: false,
            created_at: Utc::now().to_rfc3339(),
        };
        let mut m2 = m1.clone();
        m2.id = "msg-b".to_string();
        m2.seq = 2;
        m2.role = SpeakerRole::GodSystem;
        m2.agent_id = None;
        m2.content = "停电了".to_string();
        let mut m3 = m1.clone();
        m3.id = "msg-c".to_string();
        m3.seq = 3;
        m3.turn_number = 2;
        m3.agent_id = Some("AiB".to_string());
        m3.content = "第二句".to_string();
        db.append_message(&m1).unwrap();
        db.append_message(&m2).unwrap();
        db.append_message(&m3).unwrap();

        let branch_a = fork_experiment_at_turn_or_message(
            &db,
            &exp.id,
            1,
            Some("msg-a".to_string()),
            None,
        )
        .unwrap();
        assert_eq!(branch_a.title, "深夜便利店 #27 · 分支A");
        assert_eq!(branch_a.messages.len(), 1);
        assert_eq!(branch_a.messages[0].content, "第一句");
        assert_eq!(branch_a.parent_experiment_id.as_deref(), Some(exp.id.as_str()));

        let branch_b = fork_experiment_at_turn_or_message(
            &db,
            &exp.id,
            1,
            Some("msg-b".to_string()),
            None,
        )
        .unwrap();
        assert_eq!(branch_b.title, "深夜便利店 #27 · 分支B");
        assert_eq!(branch_b.messages.len(), 2);
    }
}
