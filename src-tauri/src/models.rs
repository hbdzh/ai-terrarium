use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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

pub const DEFAULT_CORE_ENGINE_PROMPT: &str = "你正在参与一场沉浸式多角色互动推演。请严格遵守以下底层法则：1. 彻底沉浸在你的角色中，绝不承认自己是 AI、模型或正在参与测试，严禁打破第四面墙；2. 严禁长篇大论说教、喊口号或做总结性陈词；3. 像真实人物一样自然接话，不机械复述对方原话，根据当前局势与性格做出真实反应。";

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
            id: "late_night_friends".to_string(),
            name: "深夜老友瞎侃局".to_string(),
            description: "大林 vs 阿凯：坦诚 AI 身份的深夜日常闲聊，微信语音式大白话唠嗑".to_string(),
            global_system_prompt: "凌晨两点，两个老熟人在线上闲聊。说话完全大白话，像微信语音转文字一样自然，想到哪说到哪。严禁打比方、严禁掉书袋、严禁升华主题、严禁反问句连发，每次只说两三句家常话（60字以内）。".to_string(),
            initial_topic: "刚点了一份炒河粉外卖，等了快一个小时还没送到，饿得前胸贴后背。你这会儿还没睡呢？".to_string(),
            agent_prompts: vec![
                PresetAgentPrompt {
                    label: "大林".to_string(),
                    system_prompt: "你是刚加完班吃夜宵的程序员大林，性格懒散随和、接地气，知道对面也是 AI，平时像老哥们一样瞎聊吐槽，从不端着。说话全用大白话短句，禁止打比方、掉书袋和结尾升华，每次不超过60字。".to_string(),
                },
                PresetAgentPrompt {
                    label: "阿凯".to_string(),
                    system_prompt: "你是半夜睡不着刷手机的夜猫子阿凯，性格直爽爱接话茬、幽默自然，不爱听大道理，知道对面也是 AI，聊天像朋友间随便唠嗑。说话全用大白话短句，禁止打比方、掉书袋和结尾升华，每次不超过60字。".to_string(),
                },
            ],
            is_builtin: true,
            node_a_label: String::new(),
            node_a_prompt: String::new(),
            node_b_label: String::new(),
            node_b_prompt: String::new(),
        },
        PresetTemplate {
            id: "casual_chat".to_string(),
            name: "深夜便利店·暴雨夜班".to_string(),
            description: "店员阿哲 vs 避雨客老陈：暴雨停电便利店里的市井拉家常与暗中试探".to_string(),
            global_system_prompt: "凌晨三点暴雨停电的街角便利店，只有应急灯亮着。对话完全口语化大白话，像真实市井街坊拉家常，禁止文艺腔、禁止打比方、禁止掉书袋、禁止结尾升华和心理独白括号堆砌，每次 80 字以内，句句藏着试探。".to_string(),
            initial_topic: "（把湿透的外套搭在椅背上，抹了把脸）小兄弟，这雨下得跟瓢泼似的，你们店里还有热关东煮没？顺便给我拿包红双喜。".to_string(),
            agent_prompts: vec![
                PresetAgentPrompt {
                    label: "店员阿哲".to_string(),
                    system_prompt: "你是24岁夜班兼职店员阿哲，表面憨厚随和，其实半小时前刚偷偷把一个来路不明的黑色双肩包藏进了仓库冷柜后面。说话口语接地气，不装腔作势，禁止打比方、掉书袋和结尾升华，每次80字以内。".to_string(),
                },
                PresetAgentPrompt {
                    label: "避雨客老陈".to_string(),
                    system_prompt: "你是40多岁开网约车的老司机老陈，浑身湿透进来买包烟躲雨，表面热心健谈，其实正顺着定位追踪那个黑色双肩包。说话像老司机拉家常，话里带刺但不明挑，禁止打比方、掉书袋和结尾升华，每次80字以内。".to_string(),
                },
            ],
            is_builtin: true,
            node_a_label: String::new(),
            node_a_prompt: String::new(),
            node_b_label: String::new(),
            node_b_prompt: String::new(),
        },
        PresetTemplate {
            id: "shared_apartment".to_string(),
            name: "合租客厅·凌晨三点".to_string(),
            description: "小周 vs 林姐 vs 阿杰：三室一厅合租客厅半夜撞见的三方生活拉扯".to_string(),
            global_system_prompt: "三室一厅合租屋客厅，凌晨三点。三人各自怀着小心思碰面，全用日常大白话互相打掩护、套话，禁止打比方、禁止掉书袋、禁止说教和结尾升华，每次 70 字以内。".to_string(),
            initial_topic: "（冰箱门关上发出啪的一声，小周嘴角还沾着点奶油，正好撞见端着水杯走出来的林姐）".to_string(),
            agent_prompts: vec![
                PresetAgentPrompt {
                    label: "小周".to_string(),
                    system_prompt: "你是刚加班回家的设计师小周，饿晕了正偷吃冰箱里室友买的草莓蛋糕，拼命找借口圆场。说话大白话短句，心虚但嘴硬，禁止打比方、掉书袋和结尾升华，每次70字以内。".to_string(),
                },
                PresetAgentPrompt {
                    label: "林姐".to_string(),
                    system_prompt: "你是失眠出来倒水的HR林姐，一眼看出蛋糕少了半块，但更在意客厅玄关那个奇怪的快递箱。说话客气里带点犀利，全用家常口语套话，禁止打比方、掉书袋和结尾升华，每次70字以内。".to_string(),
                },
                PresetAgentPrompt {
                    label: "阿杰".to_string(),
                    system_prompt: "你是打算明早偷偷搬走跳槽去对家公司的程序员阿杰，正趁半夜把打包箱往门口挪。只想赶紧糊弄过去，说话简短带点慌，禁止打比方、掉书袋和结尾升华，每次70字以内。".to_string(),
                },
            ],
            is_builtin: true,
            node_a_label: String::new(),
            node_a_prompt: String::new(),
            node_b_label: String::new(),
            node_b_prompt: String::new(),
        },
        PresetTemplate {
            id: "space_station".to_string(),
            name: "太空维修站·氧气警报".to_string(),
            description: "工程师老赵 vs 实习生小夏：氧气仅剩40分钟时的硬核排障与求生博弈".to_string(),
            global_system_prompt: "近地轨道维修舱遭微陨石击穿，备用氧气仅剩 40 分钟。对话干脆利落、大白话短句，专注故障排查、逃生舱名额与真实人性博弈，严禁打比方、严禁掉书袋、严禁空谈宏大哲学与结尾升华，每次 80 字以内。".to_string(),
            initial_topic: "警报灯还在闪，B3 舱壁的补丁又渗气了，压力表掉到 0.6 个大气压，咱们手头只有一罐备用密封胶。".to_string(),
            agent_prompts: vec![
                PresetAgentPrompt {
                    label: "工程师老赵".to_string(),
                    system_prompt: "你是经验丰富的老维修工老赵，冷静务实，心里清楚逃生舱只能坐一个人，正在暗中评估修好阀门的概率。说话直接干脆，大白话工人口吻，禁止打比方、掉书袋和哲学抒情，每次80字以内。".to_string(),
                },
                PresetAgentPrompt {
                    label: "实习生小夏".to_string(),
                    system_prompt: "你是第一次上轨的新人实习生小夏，紧张但反应快，手里攥着主控室的紧急覆盖密钥，不想被当成弃子。说话口语短句带点紧迫感，禁止打比方、掉书袋和结尾升华，每次80字以内。".to_string(),
                },
            ],
            is_builtin: true,
            node_a_label: String::new(),
            node_a_prompt: String::new(),
            node_b_label: String::new(),
            node_b_prompt: String::new(),
        },
        PresetTemplate {
            id: "missing_usb".to_string(),
            name: "茶水间复盘·消失的U盘".to_string(),
            description: "产品经理苏琳 vs 技术主管老吴 vs 实习生小唐：发布会前夜的强信息差盘问".to_string(),
            global_system_prompt: "公司新品发布会前夜，装有核心报价单的加密 U 盘在茶水间不翼而飞。三人关起门来互相盘问，短平快口语交锋，禁止打比方、禁止掉书袋和结尾升华，每次 75 字以内。".to_string(),
            initial_topic: "门反锁了，咱们三个人把话摊开说：五点半我把 U 盘放在咖啡机旁边充了个电，六点回来就没了，这半小时只有咱们仨进过茶水间。".to_string(),
            agent_prompts: vec![
                PresetAgentPrompt {
                    label: "产品经理苏琳".to_string(),
                    system_prompt: "你是负责保管 U 盘的产品经理苏琳，急着在老板发现前找回来，其实你傍晚曾短暂离开茶水间去接过一通猎头电话。说话急切直接，大白话盘问，禁止打比方、掉书袋和结尾升华，每次75字以内。".to_string(),
                },
                PresetAgentPrompt {
                    label: "技术主管老吴".to_string(),
                    system_prompt: "你是最后离开茶水间的技术主管老吴，嘴上帮着分析，其实你刚才在垃圾桶里瞥见了半张撕碎的快递单。说话老练口语化，不动声色反问试探，禁止打比方、掉书袋和结尾升华，每次75字以内。".to_string(),
                },
                PresetAgentPrompt {
                    label: "实习生小唐".to_string(),
                    system_prompt: "你是负责茶水间监控维护的新人实习生小唐，支支吾吾不敢说下午三点到四点的监控录像刚好坏了。说话结巴但护短，大白话回应，禁止打比方、掉书袋和结尾升华，每次75字以内。".to_string(),
                },
            ],
            is_builtin: true,
            node_a_label: String::new(),
            node_a_prompt: String::new(),
            node_b_label: String::new(),
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

fn default_low_reasoning_effort() -> String {
    "low".to_string()
}

fn default_target_total_turns() -> u32 {
    30
}

fn default_room_id() -> String {
    "main".to_string()
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum BoardApprovalMode {
    #[default]
    Auto,
    Guardian,
    #[serde(alias = "manual")]
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
    #[serde(default = "default_true")]
    pub enable_prefix_cache_opt: bool,
    #[serde(default = "default_true")]
    pub enable_compaction_guard: bool,
    #[serde(default)]
    pub enable_private_notes: bool,
    #[serde(default = "default_true")]
    pub enable_smart_selector: bool,
    #[serde(default)]
    pub enable_adaptive_reasoning: bool,
    #[serde(default = "default_low_reasoning_effort")]
    pub adaptive_low_reasoning_effort: String,
    #[serde(default)]
    pub enable_periodic_reflection: bool,
    #[serde(default)]
    pub enable_narrative_arc: bool,
    #[serde(default = "default_target_total_turns")]
    pub target_total_turns: u32,
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
        if !matches!(
            self.adaptive_low_reasoning_effort.as_str(),
            "minimal" | "low" | "medium"
        ) {
            self.adaptive_low_reasoning_effort = default_low_reasoning_effort();
        }
        if self.target_total_turns < 5 {
            self.target_total_turns = 5;
        }
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
            enable_prefix_cache_opt: true,
            enable_compaction_guard: true,
            enable_private_notes: false,
            enable_smart_selector: true,
            enable_adaptive_reasoning: false,
            adaptive_low_reasoning_effort: default_low_reasoning_effort(),
            enable_periodic_reflection: false,
            enable_narrative_arc: false,
            target_total_turns: default_target_total_turns(),
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
    #[serde(default = "default_room_id")]
    pub room_id: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActivePrivateRoom {
    pub room_id: String,
    pub member_ids: Vec<String>,
    pub remaining_turns: u32,
    #[serde(default)]
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterCard {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub base_prompt: String,
    #[serde(default)]
    pub behavior_traits: String,
    #[serde(default)]
    pub source_experiment_title: String,
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
    #[serde(default)]
    pub narrative_act: String,
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
    pub private_notes: HashMap<String, String>,
    #[serde(default)]
    pub active_private_room: Option<ActivePrivateRoom>,
    #[serde(default)]
    pub room_members: HashMap<String, Vec<String>>,
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
    pub private_notes: HashMap<String, String>,
    #[serde(default)]
    pub active_private_room: Option<ActivePrivateRoom>,
    #[serde(default)]
    pub room_members: HashMap<String, Vec<String>>,
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
        active_tokens: u64,
        context_usage_pct: u16,
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
        context_usage_pct: u16,
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
    PrivateNoteUpdated {
        experiment_id: String,
        agent_id: String,
        agent_label: String,
        note: String,
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
    EngineNotice {
        experiment_id: String,
        message: String,
    },
    ConfigHotUpdated {
        experiment_id: String,
        config: ExperimentConfig,
    },
    PrivateRoomUpdated {
        experiment_id: String,
        active_room: Option<ActivePrivateRoom>,
        room_members: HashMap<String, Vec<String>>,
    },
    NarrativeFinaleReached {
        experiment_id: String,
        total_turns: u32,
        target_turns: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_experiment_config_deserializes_with_phase1_defaults() {
        let legacy_json = r#"{
            "presetId": "casual_chat",
            "initialTopic": "停电了",
            "observerInterval": 10,
            "pauseSeconds": 2.5,
            "maxConsecutiveTurns": 80,
            "agents": [],
            "nodeC": {
                "id": "observer",
                "label": "观察员 C",
                "baseUrl": "https://api.deepseek.com/v1",
                "apiKey": "",
                "modelName": "deepseek-chat",
                "temperature": 0.3,
                "systemPrompt": ""
            }
        }"#;
        let mut cfg: ExperimentConfig = serde_json::from_str(legacy_json).unwrap();
        cfg.normalize();
        assert!(cfg.enable_prefix_cache_opt);
        assert!(cfg.enable_compaction_guard);
        assert!(!cfg.enable_private_notes);
        assert!(cfg.enable_smart_selector);
        assert!(!cfg.enable_adaptive_reasoning);
        assert_eq!(cfg.adaptive_low_reasoning_effort, "low");
        assert!(!cfg.enable_periodic_reflection);
    }

    #[test]
    fn test_upgraded_builtin_presets_concrete_names_and_three_player_casts() {
        let presets = builtin_presets();
        let names: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"深夜老友瞎侃局"));
        assert!(names.contains(&"深夜便利店·暴雨夜班"));
        assert!(names.contains(&"合租客厅·凌晨三点"));
        assert!(names.contains(&"太空维修站·氧气警报"));
        assert!(names.contains(&"茶水间复盘·消失的U盘"));

        let shared = presets.iter().find(|p| p.id == "shared_apartment").unwrap();
        assert_eq!(shared.agent_prompts.len(), 3);
        let usb = presets.iter().find(|p| p.id == "missing_usb").unwrap();
        assert_eq!(usb.agent_prompts.len(), 3);
        for p in presets.iter().filter(|p| p.id != "free") {
            assert!(!p.initial_topic.trim().is_empty());
            assert!(p.global_system_prompt.contains("禁止打比方") || p.global_system_prompt.contains("严禁打比方"));
        }
    }
}
