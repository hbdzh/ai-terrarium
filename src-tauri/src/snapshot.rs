use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use chrono::Utc;
use uuid::Uuid;

use crate::db::Database;
use crate::models::{ExperimentDetail, SpeakerRole, TurnSnapshot};

/// 在每轮 AI 发言结束后捕获该轮全量状态快照，供时间线回放与平行宇宙分叉使用。
pub fn capture_turn_snapshot(
    db: &Database,
    experiment_id: &str,
    turn_number: u32,
) -> Result<TurnSnapshot> {
    let detail = db.get_experiment_detail(experiment_id)?;
    let message_ids: Vec<String> = detail
        .messages
        .iter()
        .filter(|m| m.turn_number <= turn_number)
        .map(|m| m.id.clone())
        .collect();

    let snap = TurnSnapshot {
        id: Uuid::new_v4().to_string(),
        experiment_id: experiment_id.to_string(),
        turn_number,
        message_ids,
        board_content: detail.board_content,
        compacted_summary: detail.compacted_summary,
        compacted_until_seq: detail.compacted_until_seq,
        total_tokens: detail.total_tokens,
        next_speaker: detail.next_speaker,
        created_at: Utc::now().to_rfc3339(),
    };
    db.save_turn_snapshot(&snap)?;
    Ok(snap)
}

/// 从指定历史轮次克隆全新平行宇宙实验分支。
/// 支持可选的 `edit_override = Some((message_id, new_content))` 实现“修改内容并重演”。
pub fn fork_experiment_at_turn(
    db: &Database,
    source_exp_id: &str,
    target_turn: u32,
    edit_override: Option<(String, String)>,
) -> Result<ExperimentDetail> {
    let source = db.get_experiment_detail(source_exp_id)?;
    let target_snap = db.get_turn_snapshot(source_exp_id, target_turn)?;
    let all_source_snaps = db.get_turn_snapshots(source_exp_id)?;

    let mut cloned_msgs = if let Some(ref snap) = target_snap {
        let id_set: HashSet<&str> = snap.message_ids.iter().map(|s| s.as_str()).collect();
        let filtered: Vec<_> = source
            .messages
            .iter()
            .filter(|m| id_set.contains(m.id.as_str()) || m.turn_number <= target_turn)
            .cloned()
            .collect();
        if filtered.is_empty() {
            source
                .messages
                .iter()
                .filter(|m| m.turn_number <= target_turn)
                .cloned()
                .collect()
        } else {
            filtered
        }
    } else {
        source
            .messages
            .iter()
            .filter(|m| m.turn_number <= target_turn)
            .cloned()
            .collect()
    };

    let mut next_speaker = target_snap
        .as_ref()
        .map(|s| s.next_speaker.clone())
        .unwrap_or_else(|| {
            compute_next_speaker_from_messages(&source, &cloned_msgs)
        });

    let is_edit_replay = edit_override.is_some();
    if let Some((edit_msg_id, new_content)) = edit_override {
        let trimmed = new_content.trim().to_string();
        if !trimmed.is_empty() {
            if let Some(target_msg) = cloned_msgs.iter_mut().find(|m| m.id == edit_msg_id) {
                target_msg.content = trimmed;
                target_msg.reasoning_content.clear();
                if target_msg.role == SpeakerRole::Ai {
                    next_speaker =
                        next_agent_after(&source, target_msg.agent_id.as_deref());
                }
            }
        }
    }

    let suffix = if is_edit_replay {
        format!("改写重演 #{target_turn}")
    } else {
        format!("分支 #{target_turn}")
    };
    let base_title = source.title.trim();
    let new_title = format!("{base_title} · {suffix}");

    let board_content = target_snap
        .as_ref()
        .map(|s| s.board_content.clone())
        .unwrap_or_else(|| source.board_content.clone());
    let compacted_summary = target_snap
        .as_ref()
        .map(|s| s.compacted_summary.clone())
        .unwrap_or_else(|| {
            if target_turn >= source.total_turns {
                source.compacted_summary.clone()
            } else {
                String::new()
            }
        });
    let compacted_until_seq = target_snap
        .as_ref()
        .map(|s| s.compacted_until_seq)
        .unwrap_or_else(|| {
            if target_turn >= source.total_turns {
                source.compacted_until_seq
            } else {
                0
            }
        });
    let total_tokens = target_snap
        .as_ref()
        .map(|s| s.total_tokens)
        .unwrap_or_else(|| cloned_msgs.iter().map(|m| m.total_tokens).sum());

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
        .filter(|n| n.trigger_turn <= target_turn)
    {
        note.id = Uuid::new_v4().to_string();
        note.experiment_id = new_exp_id.clone();
        db.append_observer_note(&note)?;
    }

    db.update_experiment_state_for_fork(
        &new_exp_id,
        target_turn,
        total_tokens,
        &next_speaker,
        &board_content,
        &compacted_summary,
        compacted_until_seq,
    )?;

    for mut snap in all_source_snaps
        .into_iter()
        .filter(|s| s.turn_number <= target_turn)
    {
        snap.id = Uuid::new_v4().to_string();
        snap.experiment_id = new_exp_id.clone();
        snap.message_ids = snap
            .message_ids
            .into_iter()
            .filter_map(|old| id_map.get(&old).cloned())
            .collect();
        if snap.turn_number == target_turn {
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

    #[test]
    fn test_snapshot_and_fork_at_turn_with_edit() {
        let db = Database::open_in_memory().unwrap();
        let exp = db.create_experiment("主宇宙", &ExperimentConfig::default()).unwrap();

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
                reasoning_content: "思考".to_string(),
                content: format!("发言 {i}"),
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
}
