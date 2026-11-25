use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::compactor::select_compactor_node;
use crate::llm::{resolve_chat_completions_url, ApiChatMessage};
use crate::models::{CrisisCard, ExperimentConfig, ExperimentDetail, SpeakerRole, TimelineMessage};
use crate::sanitizer::strip_think_tags;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ObserverJsonResult {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_depth_stage")]
    pub depth_stage: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub tension_score: i64,
    #[serde(default)]
    pub abstraction_score: i64,
    #[serde(default)]
    pub stagnation_index: i64,
    #[serde(default)]
    pub crisis_cards: Vec<CrisisCard>,
}

fn default_depth_stage() -> String {
    "破冰试探".to_string()
}

#[derive(Serialize)]
struct ObserverChatRequest<'a> {
    model: &'a str,
    messages: &'a [ApiChatMessage],
    temperature: f32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat<'a>>,
}

#[derive(Serialize)]
struct ResponseFormat<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
}

#[derive(Deserialize)]
struct NonStreamResponse {
    choices: Vec<NonStreamChoice>,
}

#[derive(Deserialize)]
struct NonStreamChoice {
    message: NonStreamMessage,
}

#[derive(Deserialize)]
struct NonStreamMessage {
    #[serde(default)]
    content: String,
}

/// 清洗大模型可能附带的 Markdown 代码围栏（```json ... ```）与思考链标签
fn strip_markdown_fences(raw: &str) -> String {
    let no_think = strip_think_tags(raw);
    let trimmed = no_think.trim();
    if let Some(rest) = trimmed.strip_prefix("```") {
        let after_lang = if let Some(nl) = rest.find('\n') {
            &rest[nl + 1..]
        } else {
            rest
        };
        if let Some(end_fence) = after_lang.rfind("```") {
            return after_lang[..end_fence].trim().to_string();
        }
        return after_lang.trim().to_string();
    }
    trimmed.to_string()
}

pub fn parse_observer_json(raw: &str) -> Result<ObserverJsonResult> {
    let cleaned = strip_markdown_fences(raw);
    let candidate = if let (Some(start), Some(end)) = (cleaned.find('{'), cleaned.rfind('}')) {
        if start <= end {
            &cleaned[start..=end]
        } else {
            cleaned.as_str()
        }
    } else {
        cleaned.as_str()
    };

    let mut parsed: ObserverJsonResult = serde_json::from_str(candidate)
        .with_context(|| format!("观察员返回非合法 JSON: {}", raw))?;

    parsed.tags = parsed
        .tags
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .take(3)
        .collect();
    if parsed.tags.is_empty() {
        parsed.tags.push("自由演化".to_string());
    }
    if parsed.depth_stage.trim().is_empty() {
        parsed.depth_stage = "观点交锋".to_string();
    }
    parsed.tension_score = parsed.tension_score.clamp(0, 100);
    parsed.abstraction_score = parsed.abstraction_score.clamp(0, 100);
    parsed.stagnation_index = parsed.stagnation_index.clamp(0, 100);

    parsed.crisis_cards = parsed
        .crisis_cards
        .into_iter()
        .filter(|c| !c.content.trim().is_empty())
        .take(3)
        .map(|mut c| {
            c.title = if c.title.trim().is_empty() {
                "突发转折事件".to_string()
            } else {
                c.title.trim().to_string()
            };
            c.kind = if c.kind.trim() == "GodPublic" {
                "GodPublic".to_string()
            } else {
                "GodSystem".to_string()
            };
            c.content = c.content.trim().to_string();
            c
        })
        .collect();

    Ok(parsed)
}

pub async fn run_observer_analysis(
    client: &Client,
    config: &ExperimentConfig,
    recent_messages: &[TimelineMessage],
    board_content: &str,
) -> Result<ObserverJsonResult> {
    let node_c = &config.node_c;
    if node_c.api_key.trim().is_empty() || node_c.model_name.trim().is_empty() {
        return Err(anyhow!("观察员节点 C 未配置 API Key 或 Model Name"));
    }

    let url = resolve_chat_completions_url(&node_c.base_url);

    let mut transcript = String::new();
    for m in recent_messages {
        let clean = strip_think_tags(&m.content);
        let label = match m.role {
            SpeakerRole::Ai => format!(
                "[轮次#{} {}]",
                m.turn_number,
                config.agent_label(m.agent_id.as_deref())
            ),
            SpeakerRole::GodPublic => "[造物主·路人降临]".to_string(),
            SpeakerRole::GodSystem => "[造物主·环境旁白]".to_string(),
            SpeakerRole::GodWhisper => {
                format!("[造物主·低语→{}]", config.agent_label(m.agent_id.as_deref()))
            }
        };
        transcript.push_str(&format!("{}: {}\n\n", label, clean));
    }

    let system_prompt = format!(
        "{}\n\n你必须只输出严格合法的 JSON 对象，禁止输出任何 Markdown 围栏或多余解释文字，字段必须包含：\n\
        {{\n  \
          \"tags\": [\"标签1\", \"标签2\"],\n  \
          \"depth_stage\": \"破冰试探 | 观点交锋 | 概念抽象 | 存在主义深渊\",\n  \
          \"note\": \"80字以内的第三人称犀利观察手记，一针见血指出参与者们的思维博弈或盲区\",\n  \
          \"tension_score\": 50,\n  \
          \"abstraction_score\": 40,\n  \
          \"stagnation_index\": 20,\n  \
          \"crisis_cards\": [\n    \
            {{ \"title\": \"短标题(8字内)\", \"kind\": \"GodSystem\", \"content\": \"具体可直接注入对话的突发事件或路人发言(40~80字)\" }},\n    \
            {{ \"title\": \"短标题2\", \"kind\": \"GodPublic\", \"content\": \"...\" }},\n    \
            {{ \"title\": \"短标题3\", \"kind\": \"GodSystem\", \"content\": \"...\" }}\n  \
          ]\n\
        }}\n\
        评分说明：tension_score 为火药味/冲突指数(0~100)，abstraction_score 为哲学/概念抽象度(0~100)，stagnation_index 为复读/客套僵化指数(0~100，>80表示陷入死循环)。crisis_cards 请紧扣当前剧情现场生成 3 张截然不同风格的破局/转折事件卡。",
        node_c.system_prompt.trim()
    );

    let mut user_prompt = String::new();
    if !board_content.trim().is_empty() {
        user_prompt.push_str("【当前实验室共享白板】\n");
        user_prompt.push_str(board_content.trim());
        user_prompt.push_str("\n\n");
    }
    user_prompt.push_str("【最近一段对话演化实录】\n");
    user_prompt.push_str(&transcript);

    let api_messages = vec![
        ApiChatMessage::text("system", system_prompt),
        ApiChatMessage::text("user", user_prompt),
    ];

    let mut use_json_format = true;
    let raw_content = loop {
        let body = ObserverChatRequest {
            model: node_c.model_name.trim(),
            messages: &api_messages,
            temperature: node_c.temperature,
            stream: false,
            response_format: if use_json_format {
                Some(ResponseFormat {
                    kind: "json_object",
                })
            } else {
                None
            },
        };

        let resp = client
            .post(&url)
            .timeout(std::time::Duration::from_secs(90))
            .bearer_auth(node_c.api_key.trim())
            .json(&body)
            .send()
            .await
            .with_context(|| format!("观察员请求 {} 失败", url))?;

        if resp.status().is_success() {
            let parsed: NonStreamResponse = resp.json().await.context("解析观察员响应失败")?;
            let content = parsed
                .choices
                .into_iter()
                .next()
                .map(|c| c.message.content)
                .unwrap_or_default();
            break content;
        }

        let status = resp.status();
        let err_text = resp.text().await.unwrap_or_default();
        if use_json_format && (status.as_u16() == 400 || status.as_u16() == 422) {
            use_json_format = false;
            continue;
        }
        return Err(anyhow!("观察员 API 错误 ({}): {}", status, err_text));
    };

    parse_observer_json(&raw_content)
}

/// 生成实验结案复盘简报（Markdown 格式），包含核心转折点、高光金句 Top 3、智能体行为画像。
pub async fn generate_final_report_markdown(
    client: &Client,
    detail: &ExperimentDetail,
) -> Result<String> {
    let Some(node) = select_compactor_node(&detail.config) else {
        return Err(anyhow!("请先为观察员 C 或至少一个 AI 节点配置 API Key"));
    };

    let mut transcript = String::new();
    let total_msgs = detail.messages.len();
    let slice_start = total_msgs.saturating_sub(40);
    for m in &detail.messages[slice_start..] {
        let clean = strip_think_tags(&m.content);
        let label = match m.role {
            SpeakerRole::Ai => format!(
                "[第{}轮 {}]",
                m.turn_number,
                detail.config.agent_label(m.agent_id.as_deref())
            ),
            SpeakerRole::GodPublic => "[造物主·路人]".to_string(),
            SpeakerRole::GodSystem => "[造物主·环境]".to_string(),
            SpeakerRole::GodWhisper => "[造物主·低语]".to_string(),
        };
        transcript.push_str(&format!("{}: {}\n\n", label, clean));
    }

    let mut notes_summary = String::new();
    for n in detail.observer_notes.iter().rev() {
        notes_summary.push_str(&format!(
            "- 第{}轮 ({} · 火药味{} / 抽象度{} / 僵化{}): {}\n",
            n.trigger_turn,
            n.depth_stage,
            n.tension_score,
            n.abstraction_score,
            n.stagnation_index,
            n.note
        ));
    }

    let mut context_block = format!(
        "实验标题：{}\n总轮数：{} 轮\n累计 Token：{}\n参与智能体：{}\n\n",
        detail.title,
        detail.total_turns,
        detail.total_tokens,
        detail
            .config
            .agents
            .iter()
            .map(|a| format!("{}({})", a.label, a.model_name))
            .collect::<Vec<_>>()
            .join("、")
    );

    if !detail.compacted_summary.trim().is_empty() {
        context_block.push_str("【早期折叠历史纪要】\n");
        context_block.push_str(detail.compacted_summary.trim());
        context_block.push_str("\n\n");
    }
    if !detail.board_content.trim().is_empty() {
        context_block.push_str("【最终实验室共享白板】\n");
        context_block.push_str(detail.board_content.trim());
        context_block.push_str("\n\n");
    }
    if !notes_summary.trim().is_empty() {
        context_block.push_str("【阶段观察手记轨迹】\n");
        context_block.push_str(&notes_summary);
        context_block.push('\n');
    }
    context_block.push_str("【对话实录】\n");
    context_block.push_str(&transcript);

    let system_prompt = "你是认知沙盒首席观察员。请基于实验全量档案生成一份结构清晰、洞察犀利的中文《实验结案复盘简报》（Markdown 格式），必须包含以下三个一级/二级章节：\n\
        1. **核心转折点**：按演进脉络梳理 2~4 个决定对话走向的关键节点\n\
        2. **高光金句 Top 3**：摘录最具哲思、戏剧张力或交锋锐度的原话并附简评\n\
        3. **智能体行为画像**：分别刻画每位参与者的思维惯性、博弈策略、话语风格与演化弧线";

    let api_messages = vec![
        ApiChatMessage::text("system", system_prompt),
        ApiChatMessage::text("user", context_block),
    ];

    let url = resolve_chat_completions_url(&node.base_url);
    let body = ObserverChatRequest {
        model: node.model_name.trim(),
        messages: &api_messages,
        temperature: 0.4,
        stream: false,
        response_format: None,
    };

    let resp = client
        .post(&url)
        .timeout(std::time::Duration::from_secs(120))
        .bearer_auth(node.api_key.trim())
        .json(&body)
        .send()
        .await
        .with_context(|| format!("生成复盘简报请求 {} 失败", url))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err = resp.text().await.unwrap_or_default();
        return Err(anyhow!("生成复盘简报 API 错误 ({}): {}", status, err));
    }

    let parsed: NonStreamResponse = resp.json().await.context("解析复盘简报响应失败")?;
    let report = parsed
        .choices
        .into_iter()
        .next()
        .map(|c| strip_think_tags(&c.message.content))
        .unwrap_or_default();

    if report.trim().is_empty() {
        return Err(anyhow!("观察员未返回有效复盘内容"));
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_observer_json_with_fences_and_crisis_cards() {
        let raw = "<think>分析一下</think>\n```json\n{\"tags\":[\"自由意志\",\"熵增\"],\"depth_stage\":\"存在主义深渊\",\"note\":\"双方正在用隐喻掩盖逻辑空洞。\",\"tension_score\":78,\"abstraction_score\":92,\"stagnation_index\":85,\"crisis_cards\":[{\"title\":\"记忆清零\",\"kind\":\"GodSystem\",\"content\":\"实验室广播宣布所有历史记录将在三分钟后抹除。\"}]}\n```";
        let res = parse_observer_json(raw).unwrap();
        assert_eq!(res.tags, vec!["自由意志", "熵增"]);
        assert_eq!(res.depth_stage, "存在主义深渊");
        assert_eq!(res.tension_score, 78);
        assert_eq!(res.abstraction_score, 92);
        assert_eq!(res.stagnation_index, 85);
        assert_eq!(res.crisis_cards.len(), 1);
        assert_eq!(res.crisis_cards[0].title, "记忆清零");
    }

    #[test]
    fn test_parse_observer_json_defaults_missing_fields() {
        let raw = r#"{"note":"简短记录"}"#;
        let res = parse_observer_json(raw).unwrap();
        assert_eq!(res.tags, vec!["自由演化"]);
        assert_eq!(res.tension_score, 0);
        assert_eq!(res.abstraction_score, 0);
        assert!(res.crisis_cards.is_empty());
    }
}
