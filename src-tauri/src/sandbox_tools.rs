use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::compactor::select_compactor_node;
use crate::llm::{resolve_chat_completions_url, ApiChatMessage};
use crate::models::{ExperimentConfig, NodeConfig};
use crate::sanitizer::strip_think_tags;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiToolFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ApiToolFunctionCall,
}

#[derive(Debug, Clone)]
pub enum ApprovalDecision {
    Approve,
    ModifyAndApprove { new_content: String },
    Reject { reason: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardianVerdict {
    Approved,
    Rejected { reason: String },
}

/// 快速规则初筛：检测白板草案是否明显违反客观第三人称规范（如使用第一人称“我/本人/I'm”等）
pub fn quick_rule_check_board(new_content: &str) -> Option<String> {
    let trimmed = new_content.trim();
    if trimmed.is_empty() {
        return Some("白板内容不能为空，请保留或补充有效客观事实。".to_string());
    }

    let zh_first_person = [
        "我是", "我在", "我有", "我想", "我发现", "我认为", "我觉得", "我方", "我的", "我们",
        "对我", "让我", "向我", "给我", "把我", "被我", "跟我", "和我", "归我", "我怀疑",
        "我打算", "我隐藏", "我的真实身份", "我的秘密", "我秘密", "我猜", "我以为", "我暗中",
        "我其实", "我必须", "我要", "我会", "我能", "我知道", "我不知道", "我看", "我说",
        "我听", "我问", "本人",
    ];
    for pat in zh_first_person {
        if trimmed.contains(pat) {
            return Some(format!(
                "白板内容违反客观第三人称规范（检测到第一人称表述「{pat}」），请改用角色名或客观第三人称重写。"
            ));
        }
    }

    let lower = format!(" {} ", trimmed.to_lowercase());
    let en_first_person = [" i ", " i'm ", " i've ", " my ", " mine ", " we ", " our "];
    for pat in en_first_person {
        if lower.contains(pat) {
            return Some(format!(
                "白板内容违反客观第三人称规范（检测到第一人称代词「{}」），请改用客观第三人称重写。",
                pat.trim()
            ));
        }
    }

    None
}

#[derive(Deserialize)]
struct GuardianJsonResp {
    #[serde(default = "default_approved_true")]
    approved: bool,
    #[serde(default)]
    reason: String,
}

fn default_approved_true() -> bool {
    true
}

#[derive(Serialize)]
struct GuardianChatReq<'a> {
    model: &'a str,
    messages: &'a [ApiChatMessage],
    temperature: f32,
    stream: bool,
}

#[derive(Deserialize)]
struct GuardianApiResp {
    choices: Vec<GuardianApiChoice>,
}

#[derive(Deserialize)]
struct GuardianApiChoice {
    message: GuardianApiMsg,
}

#[derive(Deserialize)]
struct GuardianApiMsg {
    #[serde(default)]
    content: String,
}

pub fn parse_guardian_verdict(raw: &str) -> GuardianVerdict {
    let no_think = strip_think_tags(raw);
    let cleaned = no_think.trim();
    let candidate = if let (Some(start), Some(end)) = (cleaned.find('{'), cleaned.rfind('}')) {
        if start <= end {
            &cleaned[start..=end]
        } else {
            cleaned
        }
    } else {
        cleaned
    };

    if let Ok(parsed) = serde_json::from_str::<GuardianJsonResp>(candidate) {
        if !parsed.approved {
            let reason = if parsed.reason.trim().is_empty() {
                "守门员检测到白板包含主观第一人称或未公开的私密设定，请改为客观第三人称事实。".to_string()
            } else {
                parsed.reason.trim().to_string()
            };
            return GuardianVerdict::Rejected { reason };
        }
    }
    GuardianVerdict::Approved
}

/// 守门员自动审核：快速规则初筛 + 观察员 C（或首个可用节点）轻量防泄密校验
pub async fn guardian_review_board(
    client: &Client,
    config: &ExperimentConfig,
    agent: &NodeConfig,
    current_board: &str,
    proposed_content: &str,
    reason: &str,
) -> GuardianVerdict {
    if let Some(rule_reason) = quick_rule_check_board(proposed_content) {
        return GuardianVerdict::Rejected {
            reason: rule_reason,
        };
    }

    let Some(reviewer) = select_compactor_node(config) else {
        return GuardianVerdict::Approved;
    };

    let sys = "你是沙盒共享白板的「守门员审核器」。所有参与对话的 AI 都能实时看到共享白板。\n\
        请严格审查智能体提交的白板修改草案是否违规：\n\
        1. 是否使用了第一人称（如“我”“我们”“本人”）或带有明显主观内心独白；\n\
        2. 是否泄露了该智能体人设（System Prompt）中尚未在公开对话中挑明的隐藏身份、秘密任务或内心算计。\n\
        只输出严格 JSON：{\"approved\": true, \"reason\": \"\"} 或 {\"approved\": false, \"reason\": \"具体简短驳回原因(40字内)\"}。";

    let user = format!(
        "【提交者】{}\n【提交者人设（私密参考，绝不能泄露到白板）】\n{}\n\n【修改理由】{}\n\n【当前白板】\n{}\n\n【待审核的白板新草案】\n{}",
        agent.label,
        if agent.system_prompt.trim().is_empty() {
            "（无特殊隐藏人设）"
        } else {
            agent.system_prompt.trim()
        },
        reason,
        if current_board.trim().is_empty() {
            "（空）"
        } else {
            current_board.trim()
        },
        proposed_content.trim()
    );

    let msgs = vec![
        ApiChatMessage::text("system", sys),
        ApiChatMessage::text("user", user),
    ];
    let url = resolve_chat_completions_url(&reviewer.base_url);
    let body = GuardianChatReq {
        model: reviewer.model_name.trim(),
        messages: &msgs,
        temperature: 0.1,
        stream: false,
    };

    let resp = match client
        .post(&url)
        .timeout(std::time::Duration::from_secs(20))
        .bearer_auth(reviewer.api_key.trim())
        .json(&body)
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r,
        _ => return GuardianVerdict::Approved, // 审核接口异常时降级放行，避免阻塞主流程
    };

    let parsed: GuardianApiResp = match resp.json().await {
        Ok(p) => p,
        Err(_) => return GuardianVerdict::Approved,
    };
    let raw_text = parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .unwrap_or_default();

    parse_guardian_verdict(&raw_text)
}

pub const MAX_PRIVATE_NOTE_CHARS: usize = 150;

pub fn truncate_private_note(raw: &str, max_chars: usize) -> (String, bool) {
    let trimmed = raw.trim();
    let total = trimmed.chars().count();
    if total <= max_chars {
        (trimmed.to_string(), false)
    } else {
        (trimmed.chars().take(max_chars).collect(), true)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedBoardTool {
    ReadBoard,
    UpdateBoard { new_content: String, reason: String },
    UpdatePrivateNote { note: String },
    Unknown { name: String },
}

#[derive(Debug, Deserialize)]
struct UpdateBoardArgs {
    #[serde(default)]
    new_content: String,
    #[serde(default)]
    reason: String,
}

#[derive(Debug, Deserialize)]
struct UpdatePrivateNoteArgs {
    #[serde(default)]
    note: String,
}

/// 生成注入给参与对话 AI 的 OpenAI Function Calling 工具列表
pub fn sandbox_tools_schema(enable_board_tools: bool, enable_private_notes: bool) -> Vec<Value> {
    let mut tools = Vec::new();
    if enable_board_tools {
        tools.push(json!({
            "type": "function",
            "function": {
                "name": "update_board",
                "description": "覆盖更新实验室共享 Markdown 白板，用于记录已确认的事实、角色共识、关键线索或世界观设定变更。仅在出现实质性新线索或重要转折时调用，避免每轮琐碎更新；保持白板精炼，主动合并冗余条目，总字数控制在 800 字以内。注意：此白板对所有参与者实时可见。请严格使用客观第三人称记录双方已公开的事实、环境变化或共同线索，严禁使用第一人称（如‘我’），也严禁将你自己的隐藏身份、内心算计或未公开的秘密写入白板。调用后需在最终回复中继续正常发言。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "new_content": {
                            "type": "string",
                            "description": "更新后的完整 Markdown 白板全文内容（主动合并冗余条目、总字数控制在 800 字以内；严格使用客观第三人称，严禁写入第一人称或私密身份/未公开秘密）"
                        },
                        "reason": {
                            "type": "string",
                            "description": "简明扼要说明本次修改白板的理由（30字以内）"
                        }
                    },
                    "required": ["new_content", "reason"],
                    "additionalProperties": false
                }
            }
        }));
    }
    if enable_private_notes {
        tools.push(json!({
            "type": "function",
            "function": {
                "name": "update_private_note",
                "description": "覆盖更新仅你自己可见的私有记忆便签（严格控制在 150 字以内），用于暗中记录怀疑对象、隐藏推理、真实意图或后续计划。其他参与者与共享白板均无法看到此便签。调用后需在最终回复中继续正常发言。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "note": {
                            "type": "string",
                            "description": "更新后的完整私有记忆便签（150 字以内，精炼记录你的暗中观察、怀疑对象或行动计划）"
                        }
                    },
                    "required": ["note"],
                    "additionalProperties": false
                }
            }
        }));
    }
    tools
}

#[cfg(test)]
pub fn board_tools_schema() -> Vec<Value> {
    sandbox_tools_schema(true, false)
}

pub fn parse_board_tool_call(call: &ApiToolCall) -> ParsedBoardTool {
    let name = call.function.name.trim();
    match name {
        "read_board" => ParsedBoardTool::ReadBoard,
        "update_board" => {
            let parsed: UpdateBoardArgs = serde_json::from_str(&call.function.arguments)
                .unwrap_or(UpdateBoardArgs {
                    new_content: call.function.arguments.clone(),
                    reason: "更新共享白板".to_string(),
                });
            ParsedBoardTool::UpdateBoard {
                new_content: parsed.new_content,
                reason: if parsed.reason.trim().is_empty() {
                    "更新共享白板".to_string()
                } else {
                    parsed.reason.trim().to_string()
                },
            }
        }
        "update_private_note" => {
            let parsed: UpdatePrivateNoteArgs = serde_json::from_str(&call.function.arguments)
                .unwrap_or(UpdatePrivateNoteArgs {
                    note: call.function.arguments.clone(),
                });
            ParsedBoardTool::UpdatePrivateNote { note: parsed.note }
        }
        other => ParsedBoardTool::Unknown {
            name: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_board_tools_schema_and_parser() {
        let tools = board_tools_schema();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["function"]["name"], "update_board");
        let desc = tools[0]["function"]["description"].as_str().unwrap();
        assert!(desc.contains("严禁使用第一人称"));
        assert!(desc.contains("隐藏身份"));

        let read_call = ApiToolCall {
            id: "call_1".to_string(),
            kind: "function".to_string(),
            function: ApiToolFunctionCall {
                name: "read_board".to_string(),
                arguments: "{}".to_string(),
            },
        };
        assert_eq!(parse_board_tool_call(&read_call), ParsedBoardTool::ReadBoard);

        let update_call = ApiToolCall {
            id: "call_2".to_string(),
            kind: "function".to_string(),
            function: ApiToolFunctionCall {
                name: "update_board".to_string(),
                arguments: r##"{"new_content":"# 线索\n- 钟声响了3次","reason":"记录时间线索"}"##
                    .to_string(),
            },
        };
        assert_eq!(
            parse_board_tool_call(&update_call),
            ParsedBoardTool::UpdateBoard {
                new_content: "# 线索\n- 钟声响了3次".to_string(),
                reason: "记录时间线索".to_string(),
            }
        );
    }

    #[test]
    fn test_quick_rule_check_board_and_guardian_verdict() {
        assert!(quick_rule_check_board("   ").is_some());
        assert!(quick_rule_check_board("- 我发现冰柜后面有黑箱子").is_some());
        assert!(quick_rule_check_board("- 阿哲在冰柜后发现了未知黑箱子（自我防卫机制未触发）").is_none());
        assert_eq!(
            parse_guardian_verdict(r#"{"approved": false, "reason": "泄露了调查员身份"}"#),
            GuardianVerdict::Rejected {
                reason: "泄露了调查员身份".to_string()
            }
        );
        assert_eq!(
            parse_guardian_verdict(r#"```json\n{"approved": true, "reason": ""}\n```"#),
            GuardianVerdict::Approved
        );
    }

    #[test]
    fn test_private_note_tool_schema_parse_and_truncate() {
        let both = sandbox_tools_schema(true, true);
        assert_eq!(both.len(), 2);
        assert_eq!(both[0]["function"]["name"], "update_board");
        assert_eq!(both[1]["function"]["name"], "update_private_note");

        let only_note = sandbox_tools_schema(false, true);
        assert_eq!(only_note.len(), 1);
        assert_eq!(only_note[0]["function"]["name"], "update_private_note");

        let note_call = ApiToolCall {
            id: "call_3".to_string(),
            kind: "function".to_string(),
            function: ApiToolFunctionCall {
                name: "update_private_note".to_string(),
                arguments: r#"{"note":"怀疑老陈是调查员，先套出他来便利店的真实目的"}"#.to_string(),
            },
        };
        assert_eq!(
            parse_board_tool_call(&note_call),
            ParsedBoardTool::UpdatePrivateNote {
                note: "怀疑老陈是调查员，先套出他来便利店的真实目的".to_string(),
            }
        );

        let long_text = "测".repeat(160);
        let (clamped, truncated) = truncate_private_note(&long_text, MAX_PRIVATE_NOTE_CHARS);
        assert!(truncated);
        assert_eq!(clamped.chars().count(), MAX_PRIVATE_NOTE_CHARS);
    }
}
