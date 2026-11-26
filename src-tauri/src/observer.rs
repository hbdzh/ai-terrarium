use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::compactor::select_compactor_node;
use crate::llm::{resolve_chat_completions_url, ApiChatMessage};
use crate::models::{
    CrisisCard, ExperimentConfig, ExperimentDetail, RelationshipEdge, SpeakerRole, TimelineMessage,
};
use crate::sanitizer::strip_think_tags;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReflectionWhisper {
    #[serde(default, alias = "agent_id")]
    pub agent_id: String,
    #[serde(default, alias = "agent_label")]
    pub agent_label: String,
    #[serde(default)]
    pub whisper: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DistilledImprint {
    #[serde(default, alias = "agent_id")]
    pub agent_id: String,
    #[serde(default, alias = "agent_label")]
    pub agent_label: String,
    #[serde(default)]
    pub imprint: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ObserverJsonResult {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_depth_stage")]
    pub depth_stage: String,
    #[serde(default)]
    pub narrative_act: String,
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
    #[serde(default)]
    pub reflection_whispers: Vec<ReflectionWhisper>,
    #[serde(default)]
    pub relationship_matrix: Vec<RelationshipEdge>,
    #[serde(default)]
    pub director_event: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedAgentDraft {
    pub label: String,
    #[serde(default)]
    pub system_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedScenario {
    pub title: String,
    #[serde(default)]
    pub global_system_prompt: String,
    #[serde(default)]
    pub initial_topic: String,
    #[serde(default)]
    pub initial_board: String,
    pub agents: Vec<GeneratedAgentDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DirectorParsedAction {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub observer_interval: Option<u32>,
    #[serde(default)]
    pub pause_seconds: Option<f64>,
    #[serde(default)]
    pub target_total_turns: Option<u32>,
    #[serde(default)]
    pub enable_narrative_arc: Option<bool>,
    #[serde(default)]
    pub global_system_prompt_append: Option<String>,
    #[serde(default)]
    pub whispers: Vec<ReflectionWhisper>,
    #[serde(default)]
    pub system_event: Option<String>,
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
    parsed.narrative_act = parsed.narrative_act.trim().to_string();
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

    parsed.reflection_whispers = parsed
        .reflection_whispers
        .into_iter()
        .filter_map(|mut w| {
            let trimmed: String = w.whisper.trim().chars().take(30).collect();
            if trimmed.is_empty() {
                return None;
            }
            w.agent_id = w.agent_id.trim().to_string();
            w.agent_label = w.agent_label.trim().to_string();
            w.whisper = trimmed;
            Some(w)
        })
        .collect();

    parsed.relationship_matrix = parsed
        .relationship_matrix
        .into_iter()
        .filter_map(|mut edge| {
            edge.from = edge.from.trim().to_string();
            edge.to = edge.to.trim().to_string();
            edge.attitude_tag = edge.attitude_tag.trim().chars().take(24).collect();
            if edge.from.is_empty() || edge.to.is_empty() || edge.from == edge.to {
                return None;
            }
            edge.trust_score = edge.trust_score.clamp(-100, 100);
            if edge.attitude_tag.is_empty() {
                edge.attitude_tag = if edge.trust_score < -20 {
                    "暗中戒备".to_string()
                } else if edge.trust_score > 25 {
                    "初步信任".to_string()
                } else {
                    "谨慎试探".to_string()
                };
            }
            Some(edge)
        })
        .take(12)
        .collect();

    parsed.director_event = parsed
        .director_event
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Ok(parsed)
}

pub fn build_observer_tag_calibration(tags: &[String]) -> String {
    if tags.is_empty() {
        return String::new();
    }
    let mut rules: Vec<&'static str> = Vec::new();
    for t in tags {
        match t.trim() {
            "硬核科研" => rules.push("- 【硬核科研】严厉惩罚偏离核心命题的闲聊或无端审讯；若出现跑题水字数，直接拉高 stagnation_index。"),
            "日常瞎侃" => rules.push("- 【日常瞎侃】宽容自然跑题、生活吐槽与话题跳跃；只要有活人感与真实流动，不判高僵化。"),
            "悬疑推理" => rules.push("- 【悬疑推理】重点观察信息差试探、隐瞒与微表情博弈；表面客套而实则怀疑时，在 relationship_matrix 中如实给出低信任或负分。"),
            "哲学思辨" => rules.push("- 【哲学思辨】严厉惩罚廉价共识与敷衍夸赞；鼓励以反例推进抽象纵深。"),
            "科幻推演" => rules.push("- 【科幻推演】聚焦设定推演严密性与边界后果，避免空洞煽情。"),
            "阵营博弈" => rules.push("- 【阵营博弈】重点追踪利益分化、拉拢与暗中背刺，在 relationship_matrix 体现阵营信任波动。"),
            "荒诞喜剧" => rules.push("- 【荒诞喜剧】宽容跳跃与反差幽默，只要角色一本正经推进荒诞逻辑即属良性演化。"),
            "治愈夜话" => rules.push("- 【治愈夜话】宽容温和倾诉与低冲突节奏，不因火药味低而惩罚评分。"),
            "严防跑题" => rules.push("- 【严防跑题】对偏离主线话题零容忍，跑题即明显提升 stagnation_index。"),
            "自由发散" => rules.push("- 【自由发散】允许跨话题跳跃与自由联想。"),
            "短句快节奏" => rules.push("- 【短句快节奏】重点考察互动节奏是否明快，不苛求长篇论证。"),
            "深度长推演" => rules.push("- 【深度长推演】惩罚浅层敷衍短答，鼓励完整逻辑推演。"),
            _ => {}
        }
    }
    if rules.is_empty() {
        format!("\n\n【本局实验标签】{}", tags.join(" · "))
    } else {
        format!(
            "\n\n【本局定调与评分校准（标签：{}）】\n{}",
            tags.join(" · "),
            rules.join("\n")
        )
    }
}

pub async fn run_observer_analysis(
    client: &Client,
    config: &ExperimentConfig,
    recent_messages: &[TimelineMessage],
    board_content: &str,
    compacted_summary: &str,
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
                format!(
                    "[造物主·低语→{}]",
                    config.agent_label(m.agent_id.as_deref())
                )
            }
        };
        transcript.push_str(&format!("{}: {}\n\n", label, clean));
    }

    let reflection_instruction = if config.enable_periodic_reflection {
        let agent_roster = config
            .agents
            .iter()
            .map(|a| format!("- id: \"{}\", label: \"{}\"", a.id, a.label))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "\n\n此外，请在 JSON 中额外输出 \"reflection_whispers\" 数组，为下方每位参与角色各生成一句 30 字以内的「潜意识直觉提示」（仅本人察觉的直觉怀疑、心理暗流或微妙冲动）：\n{}\n格式示例：\"reflection_whispers\": [{{\"agentId\": \"AiA\", \"agentLabel\": \"角色名\", \"whisper\": \"30字以内直觉低语\"}}]",
            agent_roster
        )
    } else {
        String::new()
    };

    let arc_instruction = if config.enable_narrative_arc && config.target_total_turns > 0 {
        let latest_turn = recent_messages.last().map(|m| m.turn_number).unwrap_or(0);
        let pct = ((latest_turn as u64 * 100) / (config.target_total_turns as u64)).min(100);
        format!(
            "\n\n【三幕式节拍器已开启】当前进度第 {}/{} 轮（{}%）。请在 JSON 中额外输出 \"narrative_act\" 字段，取值为 \"起\"、\"承\"、\"转\" 或 \"合\"（0~25% 为起，25~60% 为承，60~85% 为转，85~100% 为合），并紧扣当前幕次节奏生成事件卡与诊断。",
            latest_turn, config.target_total_turns, pct
        )
    } else {
        String::new()
    };

    let tag_calibration = build_observer_tag_calibration(&config.tags);
    let cast_names = config
        .agents
        .iter()
        .map(|a| a.label.as_str())
        .collect::<Vec<_>>()
        .join("、");

    let director_instruction = if config.enable_auto_director {
        "\n\n【戏剧情境导演模式已开启】当检测到对话陷入高僵化客套循环（stagnation_index >= 70）或连续多轮缺乏实质新信息推进时，请结合当前剧本世界观、场景物理规则与定调标签，在 JSON 中额外输出 \"director_event\" 字段：一条 40~90 字、完全符合当前现场物理规则的客观突发环境事件描写（例如便利店推门闯入受伤陌生人、实验室捕获第二波异常脉冲等），严禁出现“请推翻共识”等元指令或说教；若剧情推进顺畅无需加戏则设为 null。"
    } else {
        ""
    };

    let system_prompt = format!(
        "{}\n\n当前参演角色：{}\n你必须只输出严格合法的 JSON 对象，禁止输出任何 Markdown 围栏或多余解释文字，字段必须包含：\n\
        {{\n  \
          \"tags\": [\"标签1\", \"标签2\"],\n  \
          \"depth_stage\": \"根据当前剧情氛围提炼的4字阶段词（如：气氛铺垫、暗流试探、线索浮现、真相逼近、观点交锋、荒诞升级）\",\n  \
          \"note\": \"80字以内的第三人称犀利观察手记，一针见血指出参与者们的思维博弈或盲区\",\n  \
          \"tension_score\": 50,\n  \
          \"abstraction_score\": 40,\n  \
          \"stagnation_index\": 20,\n  \
          \"relationship_matrix\": [\n    \
            {{ \"from\": \"角色A\", \"to\": \"角色B\", \"trust_score\": -35, \"attitude_tag\": \"表面客套暗中试探\" }}\n  \
          ],\n  \
          \"crisis_cards\": [\n    \
            {{ \"title\": \"短标题(8字内)\", \"kind\": \"GodSystem\", \"content\": \"具体可直接注入对话的突发事件或路人发言(40~80字)\" }},\n    \
            {{ \"title\": \"短标题2\", \"kind\": \"GodPublic\", \"content\": \"...\" }},\n    \
            {{ \"title\": \"短标题3\", \"kind\": \"GodSystem\", \"content\": \"...\" }}\n  \
          ]\n\
        }}\n\
        评分说明：tension_score 为火药味/冲突指数(0~100)，abstraction_score 为哲学/概念抽象度(0~100)，stagnation_index 为复读/客套僵化指数(0~100，>80表示陷入死循环)。relationship_matrix 请基于对话暗流评估参演角色两两之间的实时信任度（trust_score 范围 -100 到 100，负分代表猜忌/敌意，正分代表信任/同盟）与简短心理定性 attitude_tag（12字以内）。crisis_cards 请紧扣当前剧情现场生成 3 张截然不同风格的破局/转折事件卡。{}{}{}{}",
        node_c.system_prompt.trim(),
        cast_names,
        tag_calibration,
        arc_instruction,
        director_instruction,
        reflection_instruction
    );

    let mut user_prompt = String::new();
    if !compacted_summary.trim().is_empty() {
        user_prompt.push_str("【早期折叠历史纪要】\n");
        user_prompt.push_str(compacted_summary.trim());
        user_prompt.push_str("\n\n");
    }
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

pub fn select_utility_node<'a>(
    config: &'a ExperimentConfig,
) -> Option<&'a crate::models::NodeConfig> {
    config
        .agents
        .iter()
        .find(|a| !a.api_key.trim().is_empty() && !a.model_name.trim().is_empty())
        .or_else(|| select_compactor_node(config))
}

async fn call_json_completion(
    client: &Client,
    node: &crate::models::NodeConfig,
    system_prompt: &str,
    user_prompt: &str,
    temperature: f32,
) -> Result<String> {
    let url = resolve_chat_completions_url(&node.base_url);
    let api_messages = vec![
        ApiChatMessage::text("system", system_prompt),
        ApiChatMessage::text("user", user_prompt),
    ];
    let mut use_json_format = true;
    loop {
        let body = ObserverChatRequest {
            model: node.model_name.trim(),
            messages: &api_messages,
            temperature,
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
            .bearer_auth(node.api_key.trim())
            .json(&body)
            .send()
            .await
            .with_context(|| format!("请求 {} 失败", url))?;

        if resp.status().is_success() {
            let parsed: NonStreamResponse = resp.json().await.context("解析模型响应失败")?;
            let content = parsed
                .choices
                .into_iter()
                .next()
                .map(|c| c.message.content)
                .unwrap_or_default();
            return Ok(content);
        }
        let status = resp.status();
        let err_text = resp.text().await.unwrap_or_default();
        if use_json_format && (status.as_u16() == 400 || status.as_u16() == 422) {
            use_json_format = false;
            continue;
        }
        return Err(anyhow!("模型 API 错误 ({}): {}", status, err_text));
    }
}

pub fn parse_generated_scenario_json(raw: &str) -> Result<GeneratedScenario> {
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

    let mut parsed: GeneratedScenario = serde_json::from_str(candidate)
        .with_context(|| format!("AI 捏剧返回非合法 JSON: {}", raw))?;

    parsed.title = parsed.title.trim().to_string();
    if parsed.title.is_empty() {
        parsed.title = "未命名脑洞沙盒".to_string();
    }
    parsed.global_system_prompt = parsed.global_system_prompt.trim().to_string();
    parsed.initial_topic = parsed.initial_topic.trim().to_string();
    parsed.initial_board = parsed.initial_board.trim().to_string();

    parsed.agents = parsed
        .agents
        .into_iter()
        .enumerate()
        .filter(|(_, a)| !a.label.trim().is_empty() || !a.system_prompt.trim().is_empty())
        .take(4)
        .map(|(idx, mut a)| {
            a.label = if a.label.trim().is_empty() {
                format!("角色 {}", idx + 1)
            } else {
                a.label.trim().to_string()
            };
            a.system_prompt = a.system_prompt.trim().to_string();
            a
        })
        .collect();

    if parsed.agents.len() < 2 {
        return Err(anyhow!("生成剧本至少需要包含 2 位角色"));
    }

    Ok(parsed)
}

pub async fn generate_scenario_from_idea(
    client: &Client,
    config: &ExperimentConfig,
    idea: &str,
) -> Result<GeneratedScenario> {
    let Some(node) = select_utility_node(config) else {
        return Err(anyhow!(
            "请先在「设置」中为至少一个 AI 节点或观察员 C 配置 API Key"
        ));
    };

    let system_prompt = "你是注重真实生活质感与强口语交锋的多智能体沙盒编剧。请根据用户的一句话脑洞，生成一个信息不对称、张力饱满、全员说人话的对话剧本。\n\
        你必须只输出严格合法的 JSON 对象（禁止任何 Markdown 围栏或多余说明），字段如下：\n\
        {\n  \
          \"title\": \"简短吸睛的场景标题（4~12字）\",\n  \
          \"globalSystemPrompt\": \"明确写出具体场景细节与硬性文风限制：全员大白话口语短句、每次严格 60~90 字、严禁打比方、严禁文青腔或掉书袋、严禁结尾升华主题；底层防出戏规则已由系统自动注入，无需重复编写\",\n  \
          \"initialTopic\": \"第1轮开场引子（必须是一个具体的动作描写、生活突发事件或第一句大白话台词开场，禁止抽象概括）\",\n  \
          \"initialBoard\": \"初始共享白板 Markdown（列出全员已知的公开现场事实与基础线索，严禁泄露各自秘密）\",\n  \
          \"agents\": [\n    \
            { \"label\": \"真实人名或具体称呼1（如：店员阿哲）\", \"systemPrompt\": \"具体职业/身份 + 接地气的口语说话习惯 + 只有自己知道的不对等秘密与暗中目标，并重申60~90字大白话短句、不打比方不升华\" },\n    \
            { \"label\": \"真实人名或具体称呼2（如：司机老陈）\", \"systemPrompt\": \"具体职业/身份 + 接地气的口语说话习惯 + 只有自己知道的不对等秘密与暗中目标，并重申60~90字大白话短句、不打比方不升华\" }\n  \
          ]\n\
        }\n\
        铁律要求：\n\
        1. agents 必须包含 2~4 个角色，全部使用具体真实人名与具体职业/身份，严禁使用“乐观派/悲观派/直觉派/解构派”等抽象概念标签命名；\n\
        2. globalSystemPrompt 必须明确限制每次发言字数（60~90字），禁止打比方、文青腔、掉书袋和结尾升华；\n\
        3. initialTopic 必须是具体动作或生活突发事件开场；角色间秘密相互咬合、冲突具体可演。";

    let user_prompt = format!("一句话脑洞：{}", idea.trim());
    let raw = call_json_completion(client, node, system_prompt, &user_prompt, 0.8).await?;
    parse_generated_scenario_json(&raw)
}

pub fn parse_director_command_json(raw: &str) -> Result<DirectorParsedAction> {
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

    let mut parsed: DirectorParsedAction = serde_json::from_str(candidate)
        .with_context(|| format!("导演指令解析返回非合法 JSON: {}", raw))?;

    parsed.summary = parsed.summary.trim().to_string();
    if let Some(iv) = parsed.observer_interval {
        parsed.observer_interval = Some(iv.clamp(1, 50));
    }
    if let Some(ps) = parsed.pause_seconds {
        parsed.pause_seconds = Some(ps.clamp(0.2, 30.0));
    }
    if let Some(tt) = parsed.target_total_turns {
        parsed.target_total_turns = Some(tt.clamp(5, 500));
    }
    parsed.global_system_prompt_append = parsed
        .global_system_prompt_append
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    parsed.system_event = parsed
        .system_event
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    parsed.whispers = parsed
        .whispers
        .into_iter()
        .filter_map(|mut w| {
            let text = w.whisper.trim().to_string();
            if text.is_empty() {
                return None;
            }
            w.agent_id = w.agent_id.trim().to_string();
            w.agent_label = w.agent_label.trim().to_string();
            w.whisper = text;
            Some(w)
        })
        .collect();

    Ok(parsed)
}

pub async fn parse_director_command(
    client: &Client,
    config: &ExperimentConfig,
    instruction: &str,
) -> Result<DirectorParsedAction> {
    let Some(node) = select_compactor_node(config) else {
        return Err(anyhow!("请先配置观察员 C 或至少一个 AI 节点的 API Key"));
    };

    let roster = config
        .agents
        .iter()
        .map(|a| format!("- id: \"{}\", label: \"{}\"", a.id, a.label))
        .collect::<Vec<_>>()
        .join("\n");

    let system_prompt = format!(
        "你是多智能体生态沙盒的「后台导演指令解析器」。请将造物主的自然语言指令解析为可热更新的配置与干预 JSON（只输出严格 JSON 对象，禁止多余文字）。\n\
        当前参数状态：observerInterval={}，pauseSeconds={}，enableNarrativeArc={}，targetTotalTurns={}。\n\
        当前参与角色：\n{}\n\n\
        输出字段说明（不需要变更的字段请设为 null 或空数组）：\n\
        {{\n  \
          \"summary\": \"15字以内概括本次遥控生效内容\",\n  \
          \"observerInterval\": null,\n  \
          \"pauseSeconds\": null,\n  \
          \"targetTotalTurns\": null,\n  \
          \"enableNarrativeArc\": null,\n  \
          \"globalSystemPromptAppend\": null,\n  \
          \"whispers\": [{{\"agentId\": \"角色id\", \"agentLabel\": \"角色名\", \"whisper\": \"发给该角色的一次性潜意识低语\"}}],\n  \
          \"systemEvent\": null\n\
        }}",
        config.observer_interval,
        config.pause_seconds,
        config.enable_narrative_arc,
        config.target_total_turns,
        roster
    );

    let raw = call_json_completion(client, node, &system_prompt, instruction.trim(), 0.1).await?;
    parse_director_command_json(&raw)
}

/// 从《实验结案复盘简报》Markdown 中自动提取某智能体的行为画像特质。
pub fn extract_agent_traits_from_report(report: &str, agent_label: &str) -> String {
    let label = agent_label.trim();
    if report.trim().is_empty() || label.is_empty() {
        return String::new();
    }
    let lines: Vec<&str> = report.lines().collect();
    let mut matched = Vec::new();
    let mut capturing = false;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with('#')
            && (trimmed.contains("核心转折") || trimmed.contains("高光金句"))
        {
            capturing = false;
        }
        if trimmed.contains(label)
            && (trimmed.starts_with('-')
                || trimmed.starts_with('*')
                || trimmed.starts_with('#')
                || trimmed.contains("**"))
        {
            capturing = true;
            matched.push(trimmed.trim_start_matches(['-', '*', '#', ' ']).to_string());
            continue;
        }
        if capturing {
            if trimmed.is_empty()
                || trimmed.starts_with('#')
                || trimmed.starts_with("---")
                || (trimmed.starts_with("- **") && !trimmed.contains(label))
            {
                if !matched.is_empty() {
                    break;
                }
            } else {
                matched.push(trimmed.to_string());
            }
        }
    }
    let joined = matched.join("\n");
    joined.chars().take(280).collect()
}

#[derive(Deserialize)]
struct DistilledImprintsEnvelope {
    #[serde(default)]
    imprints: Vec<DistilledImprint>,
}

pub fn parse_distilled_imprints_json(raw: &str) -> Result<Vec<DistilledImprint>> {
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
    let parsed: DistilledImprintsEnvelope =
        serde_json::from_str(candidate).with_context(|| format!("提炼记忆烙印返回非合法 JSON: {}", raw))?;
    Ok(parsed
        .imprints
        .into_iter()
        .filter_map(|mut item| {
            item.agent_id = item.agent_id.trim().to_string();
            item.agent_label = item.agent_label.trim().to_string();
            item.imprint = item.imprint.trim().chars().take(120).collect();
            if item.imprint.is_empty() || (item.agent_id.is_empty() && item.agent_label.is_empty()) {
                None
            } else {
                Some(item)
            }
        })
        .collect())
}

/// 调用观察员 C / 可用推理节点，结合本局对话自动为参演角色各提炼 1 条高浓度跨宇宙经历烙印。
pub async fn distill_memory_imprints(
    client: &Client,
    detail: &ExperimentDetail,
) -> Result<Vec<DistilledImprint>> {
    if detail.messages.is_empty() {
        return Err(anyhow!("当前实验尚无对话记录，无法提炼记忆烙印"));
    }
    let Some(node) = select_compactor_node(&detail.config) else {
        return Err(anyhow!("请先为观察员 C 或至少一个 AI 节点配置 API Key"));
    };

    let roster = detail
        .config
        .agents
        .iter()
        .map(|a| format!("- id: \"{}\", label: \"{}\"", a.id, a.label))
        .collect::<Vec<_>>()
        .join("\n");

    let mut transcript = String::new();
    let start = detail.messages.len().saturating_sub(45);
    for m in &detail.messages[start..] {
        let clean = strip_think_tags(&m.content);
        let speaker = match m.role {
            SpeakerRole::Ai => format!("[{}]", detail.config.agent_label(m.agent_id.as_deref())),
            SpeakerRole::GodSystem => "[环境旁白]".to_string(),
            SpeakerRole::GodPublic => "[路人]".to_string(),
            SpeakerRole::GodWhisper => continue,
        };
        transcript.push_str(&format!("{}: {}\n", speaker, clean));
    }

    let system_prompt = format!(
        "你是跨宇宙灵魂档案管理员。请结合本局对话经历，为下列每位参演角色各提炼 1 条高浓度「跨世界经历烙印」（20~70字，第一/第三人称性格沉淀皆可，聚焦本局形成的执念、本能敏感点、看破的套路或标志性遭遇，例如：“曾在密闭空间识破潜逃者微表情，对随身携带银色手提箱有本能职业敏感”）。\n\
        参演角色列表：\n{}\n\n\
        必须只输出严格合法 JSON 对象：\n\
        {{\"imprints\": [{{\"agentId\": \"AiA\", \"agentLabel\": \"角色名\", \"imprint\": \"高浓度记忆烙印\"}}]}}",
        roster
    );

    let mut user_prompt = format!("实验标题：{}\n", detail.title);
    if !detail.final_report.trim().is_empty() {
        user_prompt.push_str(&format!("【复盘简报摘要】\n{}\n\n", detail.final_report.trim()));
    }
    user_prompt.push_str(&format!("【本局对话实录】\n{}", transcript));

    let raw = call_json_completion(client, node, &system_prompt, &user_prompt, 0.3).await?;
    let imprints = parse_distilled_imprints_json(&raw)?;
    if imprints.is_empty() {
        return Err(anyhow!("未能提炼出有效的角色记忆烙印"));
    }
    Ok(imprints)
}

/// 对比同源父分支或兄弟分支在相同分叉点之后的演化分歧，生成《世界线分歧对照报告》。
pub async fn generate_worldline_comparison_markdown(
    client: &Client,
    exp_a: &ExperimentDetail,
    exp_b: &ExperimentDetail,
) -> Result<String> {
    let Some(node) = select_compactor_node(&exp_a.config).or_else(|| select_compactor_node(&exp_b.config)) else {
        return Err(anyhow!("请先为观察员 C 或至少一个 AI 节点配置 API Key"));
    };

    let mut diverge_turn = exp_a.fork_turn.or(exp_b.fork_turn).unwrap_or(0);
    if diverge_turn == 0 {
        let min_len = exp_a.messages.len().min(exp_b.messages.len());
        for i in 0..min_len {
            if exp_a.messages[i].content != exp_b.messages[i].content
                || exp_a.messages[i].agent_id != exp_b.messages[i].agent_id
            {
                break;
            }
            diverge_turn = exp_a.messages[i].turn_number;
        }
    }

    let format_timeline = |exp: &ExperimentDetail| -> String {
        let mut s = format!(
            "标题：{}\n总轮数：{} 轮\n标签：{}\n参演角色：{}\n",
            exp.title,
            exp.total_turns,
            if exp.config.tags.is_empty() {
                "无".to_string()
            } else {
                exp.config.tags.join(" / ")
            },
            exp.config
                .agents
                .iter()
                .map(|a| format!("{}({})", a.label, a.model_name))
                .collect::<Vec<_>>()
                .join("、")
        );
        if !exp.board_content.trim().is_empty() {
            s.push_str(&format!("最终共享白板：{}\n", exp.board_content.trim()));
        }
        s.push_str("分叉点后关键对话：\n");
        let after: Vec<_> = exp
            .messages
            .iter()
            .filter(|m| m.turn_number >= diverge_turn)
            .take(32)
            .collect();
        for m in after {
            let clean = strip_think_tags(&m.content);
            let speaker = match m.role {
                SpeakerRole::Ai => format!("[#{} {}]", m.turn_number, exp.config.agent_label(m.agent_id.as_deref())),
                SpeakerRole::GodSystem => "[环境旁白]".to_string(),
                SpeakerRole::GodPublic => "[路人]".to_string(),
                SpeakerRole::GodWhisper => format!("[低语→{}]", exp.config.agent_label(m.agent_id.as_deref())),
            };
            s.push_str(&format!("{}: {}\n", speaker, clean));
        }
        s
    };

    let system_prompt = "你是平行宇宙观测首席观察员。请对比两条同源世界线在相同分叉点之后的演化走向，生成结构清晰、洞察犀利的中文《世界线分歧对照报告》（Markdown 格式），必须包含：\n\
        1. **分叉点与变量差异**：简述分叉轮次以及两条世界线在角色设定、模型或标签上的变量变化\n\
        2. **关键转折与蝴蝶效应**：对比分叉后导致剧情走向分道扬镳的 2~3 个关键节点\n\
        3. **信息差与暗涌关系演化差异**：对比双方在试探、信任度与秘密揭露上的不同路径\n\
        4. **最终结局与实验结论对照**：总结两版平行宇宙的结局差异及变量因果";

    let user_prompt = format!(
        "共同参考分叉轮次：约第 {} 轮\n\n=== 世界线 A（当前） ===\n{}\n\n=== 世界线 B（对照） ===\n{}",
        diverge_turn,
        format_timeline(exp_a),
        format_timeline(exp_b)
    );

    let url = resolve_chat_completions_url(&node.base_url);
    let api_messages = vec![
        ApiChatMessage::text("system", system_prompt),
        ApiChatMessage::text("user", user_prompt),
    ];
    let body = ObserverChatRequest {
        model: node.model_name.trim(),
        messages: &api_messages,
        temperature: 0.35,
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
        .with_context(|| format!("生成世界线分歧对照报告请求 {} 失败", url))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err = resp.text().await.unwrap_or_default();
        return Err(anyhow!("世界线分歧对照 API 错误 ({}): {}", status, err));
    }

    let parsed: NonStreamResponse = resp.json().await.context("解析世界线对照报告响应失败")?;
    let report = parsed
        .choices
        .into_iter()
        .next()
        .map(|c| strip_think_tags(&c.message.content))
        .unwrap_or_default();
    if report.trim().is_empty() {
        return Err(anyhow!("观察员未返回有效世界线对照报告"));
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
        assert!(res.reflection_whispers.is_empty());
    }

    #[test]
    fn test_parse_observer_reflection_whispers_clamps_30_chars() {
        let long_whisper = "直".repeat(45);
        let raw = format!(
            r#"{{"note":"记录","narrative_act":"转","reflection_whispers":[{{"agentId":"AiA","agentLabel":"阿哲","whisper":"{}"}},{{"agentId":"AiB","agentLabel":"老陈","whisper":"   "}}]}}"#,
            long_whisper
        );
        let res = parse_observer_json(&raw).unwrap();
        assert_eq!(res.narrative_act, "转");
        assert_eq!(res.reflection_whispers.len(), 1);
        assert_eq!(res.reflection_whispers[0].agent_id, "AiA");
        assert_eq!(res.reflection_whispers[0].whisper.chars().count(), 30);
    }

    #[test]
    fn test_parse_generated_scenario_and_director_command() {
        let raw_scenario = r#"```json
        {
            "title": "暴雪山庄时间局",
            "globalSystemPrompt": "每次发言不超过120字。",
            "initialTopic": "壁炉的时钟突然倒走了一小时。",
            "initialBoard": "- 六人被暴雪困在山庄",
            "agents": [
                {"label": "钟表匠", "systemPrompt": "你是时间修复师，暗中寻找怀表。"},
                {"label": "医生", "systemPrompt": "你其实是二十年后的逃亡者。"},
                {"label": "管家", "systemPrompt": "你知道地下室藏着时间锚。"}
            ]
        }
        ```"#;
        let sc = parse_generated_scenario_json(raw_scenario).unwrap();
        assert_eq!(sc.title, "暴雪山庄时间局");
        assert_eq!(sc.agents.len(), 3);
        assert_eq!(sc.agents[1].label, "医生");

        let raw_dir = r#"{"summary":"加速观察并敲打阿哲","observerInterval":3,"whispers":[{"agentId":"AiA","agentLabel":"阿哲","whisper":"别再装傻，直接试探箱子"}]}"#;
        let act = parse_director_command_json(raw_dir).unwrap();
        assert_eq!(act.observer_interval, Some(3));
        assert_eq!(act.whispers.len(), 1);

        let report = "## 3. 智能体行为画像\n- **阿哲**: 表面插科打诨，实则步步试探对方底细。\n- **老陈**: 沉稳施压，善于反向设套。";
        let traits = extract_agent_traits_from_report(report, "阿哲");
        assert!(traits.contains("步步试探"));
    }

    #[test]
    fn test_parse_relationship_matrix_director_event_and_distill_imprints() {
        let raw = r#"{
            "note": "表面和气实则互相提防。",
            "tension_score": 68,
            "stagnation_index": 82,
            "relationship_matrix": [
                {"from": "阿哲", "to": "老陈", "trust_score": -150, "attitude_tag": "高度戒备"},
                {"from": "老陈", "to": "阿哲", "trust_score": 25, "attitude_tag": "表面客套暗中试探"},
                {"from": "阿哲", "to": "阿哲", "trust_score": 100, "attitude_tag": "忽略自己"}
            ],
            "director_event": "便利店玻璃门忽然被风撞开，一个浑身湿透的外卖员扶着流血的胳膊闯了进来。"
        }"#;
        let parsed = parse_observer_json(raw).unwrap();
        assert_eq!(parsed.relationship_matrix.len(), 2);
        assert_eq!(parsed.relationship_matrix[0].trust_score, -100);
        assert_eq!(parsed.relationship_matrix[0].attitude_tag, "高度戒备");
        assert_eq!(parsed.relationship_matrix[1].trust_score, 25);
        assert!(parsed.director_event.as_deref().unwrap().contains("外卖员"));

        let cal = build_observer_tag_calibration(&["硬核科研".to_string(), "严防跑题".to_string()]);
        assert!(cal.contains("硬核科研"));
        assert!(cal.contains("严防跑题"));

        let imp_raw = r#"```json
        {"imprints":[{"agentId":"AiA","agentLabel":"阿哲","imprint":"曾在暴雨夜便利店死守仓库门，对手机定位追踪声音极其敏感。"}]}
        ```"#;
        let imps = parse_distilled_imprints_json(imp_raw).unwrap();
        assert_eq!(imps.len(), 1);
        assert_eq!(imps[0].agent_label, "阿哲");
    }
}
