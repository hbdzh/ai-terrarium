use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
pub enum ParsedBoardTool {
    ReadBoard,
    UpdateBoard { new_content: String, reason: String },
    Unknown { name: String },
}

#[derive(Debug, Deserialize)]
struct UpdateBoardArgs {
    #[serde(default)]
    new_content: String,
    #[serde(default)]
    reason: String,
}

/// 生成注入给参与对话 AI 的 OpenAI Function Calling 工具列表
pub fn board_tools_schema() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "read_board",
                "description": "读取当前实验室内所有参与者共享的 Markdown 白板（共享记忆黑板/设定板/线索板）的最新全文内容。",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "update_board",
                "description": "覆盖更新实验室共享 Markdown 白板，用于记录已确认的事实、角色共识、关键线索或世界观设定变更。调用后需在最终回复中继续正常发言。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "new_content": {
                            "type": "string",
                            "description": "更新后的完整 Markdown 白板全文内容（请保留仍有价值的旧内容并融入新增或修订部分）"
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
        }),
    ]
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
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["function"]["name"], "read_board");
        assert_eq!(tools[1]["function"]["name"], "update_board");

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
}
