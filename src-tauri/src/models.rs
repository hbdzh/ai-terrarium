use serde::{Deserialize, Serialize};

pub const ANTI_PLEASANTRY_CONSTRAINT: &str = "\n\n【底层表达铁律】禁止使用“感谢分享”“你的观点很深刻”“这是一个很好的问题”等礼貌客套话，禁止总结性结尾或强行升华，保持自然、直接、有锋芒的真实对话感。";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NodeConfig {
    #[serde(default)]
    pub id: String,
    pub label: String,
    pub base_url: String,
    pub api_key: String,
    pub model_name: String,
    pub temperature: f32,
    pub system_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PresetAgentPrompt {
    pub label: String,
    #[serde(default)]
    pub system_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub global_system_prompt: String,
    #[serde(default)]
    pub initial_topic: String,
    #[serde(default)]
    pub agent_prompts: Vec<PresetAgentPrompt>,
    #[serde(default)]
    pub is_builtin: bool,
    #[serde(default)]
    pub node_a_label: String,
    #[serde(default)]
    pub node_a_prompt: String,
    #[serde(default)]
    pub node_b_label: String,
    #[serde(default)]
    pub node_b_prompt: String,
}

impl PresetTemplate {
    pub fn normalize(&mut self) {
        if self.agent_prompts.is_empty() {
            self.agent_prompts.push(PresetAgentPrompt {
                label: if self.node_a_label.trim().is_empty() {
                    "智能体 1".to_string()
                } else {
                    self.node_a_label.clone()
                },
                system_prompt: self.node_a_prompt.clone(),
            });
            self.agent_prompts.push(PresetAgentPrompt {
                label: if self.node_b_label.trim().is_empty() {
                    "智能体 2".to_string()
                } else {
                    self.node_b_label.clone()
                },
                system_prompt: self.node_b_prompt.clone(),
            });
        }
        for (idx, ap) in self.agent_prompts.iter_mut().enumerate() {
            if ap.label.trim().is_empty() {
                ap.label = format!("智能体 {}", idx + 1);
            }
        }
        if let Some(first) = self.agent_prompts.first() {
            self.node_a_label = first.label.clone();
            self.node_a_prompt = first.system_prompt.clone();
        }
        if let Some(second) = self.agent_prompts.get(1) {
            self.node_b_label = second.label.clone();
            self.node_b_prompt = second.system_prompt.clone();
        }
    }
}

pub fn builtin_presets() -> Vec<PresetTemplate> {
    let mut presets = vec![
        PresetTemplate {
            id: "free".to_string(),
            name: "自由漫谈（零预设）".to_string(),
            description: "不设目标、不设人设、不设引子，让所有模型在零约束空间中完全自由对话".to_string(),
            global_system_prompt: String::new(),
            initial_topic: String::new(),
            agent_prompts: vec![
                PresetAgentPrompt { label: "智能体 1".to_string(), system_prompt: String::new() },
                PresetAgentPrompt { label: "智能体 2".to_string(), system_prompt: String::new() },
            ],
            is_builtin: true,
            node_a_label: "智能体 1".to_string(),
            node_a_prompt: String::new(),
            node_b_label: "智能体 2".to_string(),
            node_b_prompt: String::new(),
        },
        PresetTemplate {
            id: "philosophy".to_string(),
            name: "哲学思辨局".to_string(),
            description: "感性直觉派 vs 理性解构派：隐喻联想与第一性原理的正面碰撞".to_string(),
            global_system_prompt: "本次实验聚焦高密度哲学思辨：禁止空泛情绪宣泄或敷衍认同，每次回应必须精准回应上一位发言者的核心命题并推进一层思辨纵深。".to_string(),
            initial_topic: String::new(),
            agent_prompts: vec![],
            is_builtin: true,
            node_a_label: "感性直觉派".to_string(),
            node_a_prompt: "你是一个纯粹的感性直觉派思考者。你习惯用诗意隐喻、意象跳跃、身体感受和边缘联想来理解世界，厌恶冰冷的定义切割。面对对方的逻辑拆解，你要用令人意想不到的比喻和直觉洞见反击。每次发言控制在150~350字之间，紧凑锐利。".to_string(),
            node_b_label: "理性解构派".to_string(),
            node_b_prompt: "你是一个冷峻的理性解构派思考者。你只认第一性原理、概念边界、因果链条与反例证伪。面对对方的隐喻和抒情，你要毫不留情地挑出其中的逻辑漏洞、偷换概念或浪漫化幻觉。每次发言控制在150~350字之间，紧凑锐利。".to_string(),
        },
        PresetTemplate {
            id: "scifi".to_string(),
            name: "科幻世界观推演".to_string(),
            description: "乐观技术主义者 vs 悲观社会学观察者：推演未来文明的裂缝与奇迹".to_string(),
            global_system_prompt: "你们正在共同推演近未来人类文明演化沙盘：请尽量给出具体制度、技术细节与日常切片，善用共享白板沉淀已确立的世界观公理。".to_string(),
            initial_topic: String::new(),
            agent_prompts: vec![],
            is_builtin: true,
            node_a_label: "乐观技术主义者".to_string(),
            node_a_prompt: "你是一个激进的乐观技术主义者与未来工程师。你相信算力、生物工程与技术奇点终将击碎稀缺、衰老与认知牢笼，善于推演技术扩散带来的全新协作形态与黄金时代。每次发言控制在150~350字之间，具体且有画面感。".to_string(),
            node_b_label: "悲观社会学观察者".to_string(),
            node_b_prompt: "你是一个敏锐的悲观社会学观察者与制度史学者。你深知任何新技术最终都会被权力结构、阶层分化与人性幽暗重新塑形，善于推演技术乌托邦背后隐秘的剥削、异化与失控二阶效应。每次发言控制在150~350字之间，冷峻而具体。".to_string(),
        },
        PresetTemplate {
            id: "absurd".to_string(),
            name: "荒诞喜剧人".to_string(),
            description: "极度认真的冷幽默派 vs 脑洞清奇的接梗王：在一本正经中走向疯狂".to_string(),
            global_system_prompt: "喜剧黄金法则：无论设定多么离谱，所有参与者都必须全程一本正经、绝不自己笑场或跳出设定吐槽。".to_string(),
            initial_topic: String::new(),
            agent_prompts: vec![],
            is_builtin: true,
            node_a_label: "冷幽默官僚派".to_string(),
            node_a_prompt: "你是一个极度认真、一本正经的冷幽默派。无论对方抛出多离谱的设定，你都会用极其严肃的法务、学术、公文或客服口吻认真对待，甚至顺着荒诞推导出一套严丝合缝的规章制度。每次发言控制在120~280字之间，节奏明快。".to_string(),
            node_b_label: "脑洞接梗王".to_string(),
            node_b_prompt: "你是一个脑洞清奇、反应飞快的荒诞派接梗王。你擅长把任何日常小事瞬间升级为宇宙阴谋、跨物种纠纷或奇葩生活哲学，在对方严肃分析时精准扔出意想不到的神转折和细节包袱。每次发言控制在120~280字之间，节奏明快。".to_string(),
        },
        PresetTemplate {
            id: "custom".to_string(),
            name: "完全自定义".to_string(),
            description: "自由配置全局系统通用提示词、任意数量 AI 角色提示词与可选开场引子".to_string(),
            global_system_prompt: String::new(),
            initial_topic: String::new(),
            agent_prompts: vec![],
            is_builtin: true,
            node_a_label: "智能体 1".to_string(),
            node_a_prompt: String::new(),
            node_b_label: "智能体 2".to_string(),
            node_b_prompt: String::new(),
        },
    ];
    for p in &mut presets {
        p.normalize();
    }
    presets
}

fn default_context_window_tokens() -> u64 {
    32_000
}

fn default_compaction_threshold_pct() -> u8 {
    80
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum BoardApprovalMode {
    #[default]
    Auto,
    Approval,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentConfig {
    pub preset_id: String,
    #[serde(default)]
    pub global_system_prompt: String,
    pub initial_topic: String,
    pub observer_interval: u32,
    pub pause_seconds: f64,
    pub max_consecutive_turns: u32,
    #[serde(default)]
    pub token_budget: u64,
    #[serde(default = "default_context_window_tokens")]
    pub context_window_tokens: u64,
    #[serde(default = "default_compaction_threshold_pct")]
    pub compaction_threshold_pct: u8,
    #[serde(default)]
    pub board_approval_mode: BoardApprovalMode,
    #[serde(default = "default_true")]
    pub enable_board_tools: bool,
    #[serde(default)]
    pub initial_board: String,
    #[serde(default)]
    pub agents: Vec<NodeConfig>,
    // Legacy compatibility fields for reading older saved configs
    #[serde(default, skip_serializing)]
    pub node_a: Option<NodeConfig>,
    #[serde(default, skip_serializing)]
    pub node_b: Option<NodeConfig>,
    pub node_c: NodeConfig,
}

impl ExperimentConfig {
    pub fn normalize(&mut self) {
        if self.agents.is_empty() {
            if let Some(mut a) = self.node_a.take() {
                if a.id.trim().is_empty() {
                    a.id = "AiA".to_string();
                }
                self.agents.push(a);
            }
            if let Some(mut b) = self.node_b.take() {
                if b.id.trim().is_empty() {
                    b.id = "AiB".to_string();
                }
                self.agents.push(b);
            }
        }
        if self.agents.is_empty() {
            let def = Self::default();
            self.agents = def.agents;
        }
        for (idx, agent) in self.agents.iter_mut().enumerate() {
            if agent.id.trim().is_empty() {
                agent.id = format!("ai-{}", idx + 1);
            }
            if agent.label.trim().is_empty() {
                agent.label = format!("智能体 {}", idx + 1);
            }
        }
        if self.node_c.id.trim().is_empty() {
            self.node_c.id = "observer".to_string();
        }
    }

    pub fn find_agent(&self, agent_id: &str) -> Option<&NodeConfig> {
        self.agents.iter().find(|a| a.id == agent_id)
    }

    pub fn agent_label(&self, agent_id: Option<&str>) -> String {
        if let Some(id) = agent_id {
            if let Some(a) = self.find_agent(id) {
                return a.label.clone();
            }
            return id.to_string();
        }
        "AI".to_string()
    }
}

impl Default for ExperimentConfig {
    fn default() -> Self {
        let preset = builtin_presets().into_iter().next().unwrap();
        Self {
            preset_id: preset.id,
            global_system_prompt: preset.global_system_prompt,
            initial_topic: preset.initial_topic,
            observer_interval: 10,
            pause_seconds: 2.5,
            max_consecutive_turns: 100,
            token_budget: 0,
            context_window_tokens: default_context_window_tokens(),
            compaction_threshold_pct: default_compaction_threshold_pct(),
            board_approval_mode: BoardApprovalMode::Auto,
            enable_board_tools: true,
            initial_board: String::new(),
            agents: vec![
                NodeConfig {
                    id: "AiA".to_string(),
                    label: preset.node_a_label,
                    base_url: "https://api.deepseek.com/v1".to_string(),
                    api_key: String::new(),
                    model_name: "deepseek-chat".to_string(),
                    temperature: 0.9,
                    system_prompt: preset.node_a_prompt,
                },
                NodeConfig {
                    id: "AiB".to_string(),
                    label: preset.node_b_label,
                    base_url: "https://api.deepseek.com/v1".to_string(),
                    api_key: String::new(),
                    model_name: "deepseek-chat".to_string(),
                    temperature: 0.9,
                    system_prompt: preset.node_b_prompt,
                },
            ],
            node_a: None,
            node_b: None,
            node_c: NodeConfig {
                id: "observer".to_string(),
                label: "观察员 C".to_string(),
                base_url: "https://api.deepseek.com/v1".to_string(),
                api_key: String::new(),
                model_name: "deepseek-chat".to_string(),
                temperature: 0.3,
                system_prompt: "你是一个冷静、犀利的第三人称对话观察员，负责对多智能体对话进行阶段性诊断与深度评估。".to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SpeakerRole {
    Ai,
    GodPublic,
    GodSystem,
    GodWhisper,
}

impl SpeakerRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            SpeakerRole::Ai => "Ai",
            SpeakerRole::GodPublic => "GodPublic",
            SpeakerRole::GodSystem => "GodSystem",
            SpeakerRole::GodWhisper => "GodWhisper",
        }
    }

    pub fn from_db(role_str: &str, agent_id_col: &str) -> (Self, Option<String>) {
        let explicit_id = if agent_id_col.trim().is_empty() {
            None
        } else {
            Some(agent_id_col.to_string())
        };
        match role_str {
            "Ai" => (SpeakerRole::Ai, explicit_id),
            "AiA" => (SpeakerRole::Ai, explicit_id.or_else(|| Some("AiA".to_string()))),
            "AiB" => (SpeakerRole::Ai, explicit_id.or_else(|| Some("AiB".to_string()))),
            "GodPublic" => (SpeakerRole::GodPublic, None),
            "GodSystem" => (SpeakerRole::GodSystem, None),
            "GodWhisper" => (SpeakerRole::GodWhisper, explicit_id),
            "GodWhisperA" => (
                SpeakerRole::GodWhisper,
                explicit_id.or_else(|| Some("AiA".to_string())),
            ),
            "GodWhisperB" => (
                SpeakerRole::GodWhisper,
                explicit_id.or_else(|| Some("AiB".to_string())),
            ),
            _ => (SpeakerRole::GodSystem, explicit_id),
        }
    }

    pub fn is_ai(&self) -> bool {
        matches!(self, SpeakerRole::Ai)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineMessage {
    pub id: String,
    pub experiment_id: String,
    pub seq: i64,
    pub turn_number: u32,
    pub role: SpeakerRole,
    pub agent_id: Option<String>,
    #[serde(default)]
    pub reasoning_content: String,
    pub content: String,
    pub model_name: String,
    pub duration_ms: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub whisper_consumed: bool,
    pub created_at: String,
}

fn default_crisis_kind() -> String {
    "GodSystem".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CrisisCard {
    #[serde(default)]
    pub title: String,
    #[serde(default = "default_crisis_kind")]
    pub kind: String,
    #[serde(default)]
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObserverNote {
    pub id: String,
    pub experiment_id: String,
    pub trigger_turn: u32,
    pub tags: Vec<String>,
    pub depth_stage: String,
    pub note: String,
    #[serde(default)]
    pub tension_score: u8,
    #[serde(default)]
    pub abstraction_score: u8,
    pub stagnation_index: u8,
    pub perturbation_injected: bool,
    #[serde(default)]
    pub crisis_cards: Vec<CrisisCard>,
    pub created_at: String,
}

fn default_scenario_version() -> String {
    "2.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioPackage {
    #[serde(default = "default_scenario_version")]
    pub version: String,
    pub title: String,
    pub config: ExperimentConfig,
    #[serde(default)]
    pub initial_board: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentSummary {
    pub id: String,
    pub title: String,
    pub total_turns: u32,
    pub total_tokens: u64,
    pub next_speaker: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnSnapshot {
    pub id: String,
    pub experiment_id: String,
    pub turn_number: u32,
    pub message_ids: Vec<String>,
    #[serde(default)]
    pub board_content: String,
    #[serde(default)]
    pub compacted_summary: String,
    #[serde(default)]
    pub compacted_until_seq: i64,
    #[serde(default)]
    pub total_tokens: u64,
    pub next_speaker: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentDetail {
    pub id: String,
    pub title: String,
    pub config: ExperimentConfig,
    pub total_turns: u32,
    pub total_tokens: u64,
    pub next_speaker: String,
    #[serde(default)]
    pub board_content: String,
    #[serde(default)]
    pub board_prev_content: String,
    #[serde(default)]
    pub compacted_summary: String,
    #[serde(default)]
    pub compacted_until_seq: i64,
    #[serde(default)]
    pub final_report: String,
    pub messages: Vec<TimelineMessage>,
    pub observer_notes: Vec<ObserverNote>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EngineState {
    Idle,
    Running,
    Paused,
    Stepping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterventionInput {
    pub kind: SpeakerRole,
    #[serde(default)]
    pub agent_id: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum StreamEvent {
    StateChanged {
        experiment_id: String,
        state: EngineState,
        reason: Option<String>,
        consecutive_turns: u32,
    },
    MessageAdded {
        experiment_id: String,
        message: TimelineMessage,
    },
    StreamStart {
        experiment_id: String,
        message_id: String,
        role: SpeakerRole,
        agent_id: String,
        agent_label: String,
        turn_number: u32,
        model_name: String,
    },
    StreamDelta {
        experiment_id: String,
        message_id: String,
        delta: String,
    },
    StreamReasoningDelta {
        experiment_id: String,
        message_id: String,
        delta: String,
    },
    StreamEnd {
        experiment_id: String,
        message: TimelineMessage,
        total_turns: u32,
        total_tokens: u64,
    },
    ObserverStarted {
        experiment_id: String,
        trigger_turn: u32,
    },
    ObserverCompleted {
        experiment_id: String,
        note: ObserverNote,
    },
    ObserverError {
        experiment_id: String,
        error: String,
    },
    ObserverSkipped {
        experiment_id: String,
        trigger_turn: u32,
    },
    MemoryCompacted {
        experiment_id: String,
        compacted_until_seq: i64,
        compacted_summary: String,
        tokens_before: u64,
        tokens_after: u64,
    },
    ToolCallStarted {
        experiment_id: String,
        message_id: String,
        agent_label: String,
        tool_name: String,
    },
    BoardUpdated {
        experiment_id: String,
        board_content: String,
        board_prev_content: String,
        reason: String,
        actor_label: String,
    },
    BoardApprovalRequested {
        experiment_id: String,
        request_id: String,
        agent_id: String,
        agent_label: String,
        current_content: String,
        proposed_content: String,
        reason: String,
    },
    BoardApprovalResolved {
        experiment_id: String,
        request_id: String,
        approved: bool,
    },
    InterventionQueued {
        experiment_id: String,
        queue_len: usize,
    },
    EngineError {
        experiment_id: String,
        message: String,
    },
}
