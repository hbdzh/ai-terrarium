use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
}

fn default_theme_preference() -> String {
    "system".to_string()
}

fn default_local_api_port() -> u16 {
    17890
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiPrefs {
    pub left_sidebar_open: bool,
    pub inspector_open: bool,
    pub inspector_width_pct: u32,
    #[serde(default = "default_theme_preference")]
    pub theme_preference: String,
    #[serde(default)]
    pub enable_local_api: bool,
    #[serde(default = "default_local_api_port")]
    pub local_api_port: u16,
    #[serde(default)]
    pub local_api_token: String,
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            left_sidebar_open: true,
            inspector_open: true,
            inspector_width_pct: 34,
            theme_preference: default_theme_preference(),
            enable_local_api: false,
            local_api_port: default_local_api_port(),
            local_api_token: String::new(),
        }
    }
}

pub fn default_thinking_level() -> String {
    "default".to_string()
}

pub fn normalize_thinking_level(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "off" | "low" | "medium" | "high" | "default" => raw.trim().to_ascii_lowercase(),
        _ => default_thinking_level(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApiProfile {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NodeConfig {
    #[serde(default)]
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub api_profile_id: Option<String>,
    pub base_url: String,
    pub api_key: String,
    pub model_name: String,
    pub temperature: f32,
    #[serde(default = "default_thinking_level")]
    pub thinking_level: String,
    pub system_prompt: String,
    #[serde(default)]
    pub secret_goal: String,
    #[serde(default)]
    pub goal_achieved: bool,
    #[serde(default)]
    pub private_state_snapshot: String,
    #[serde(default)]
    pub has_left: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimedClue {
    #[serde(default, alias = "trigger_turn")]
    pub trigger_turn: u32,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub triggered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PresetAgentPrompt {
    pub label: String,
    #[serde(default)]
    pub system_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PresetConfigOverrides {
    #[serde(default)]
    pub allow_pass_turn: Option<bool>,
    #[serde(default)]
    pub random_mood_dice: Option<bool>,
    #[serde(default)]
    pub intuitive_fast_reply: Option<bool>,
    #[serde(default)]
    pub enable_whiteboard: Option<bool>,
    #[serde(default)]
    pub enable_private_notes: Option<bool>,
    #[serde(default)]
    pub smart_turn_order: Option<bool>,
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
    pub tags: Vec<String>,
    #[serde(default)]
    pub agent_prompts: Vec<PresetAgentPrompt>,
    #[serde(default)]
    pub is_builtin: bool,
    #[serde(default)]
    pub recommended_config: Option<PresetConfigOverrides>,
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

pub const DEFAULT_CORE_ENGINE_PROMPT: &str = "你正在参与一场持续的多方自然对谈。无论你当前的身份是人类角色还是坦诚自己身份的 AI，请严格遵守以下表达铁律：\n1. 彻底丢掉“AI 客服腔”与“播音腔”：严禁使用“首先/其次/最后”、“综上所述”、“值得注意的是”、“作为人工智能”等模板化连接词，禁止每句话结尾都做正确但无聊的哲学升华或礼貌总结；\n2. 像真实微信聊天或深夜长谈一样说话：允许口语化、短句、停顿、反问、调侃甚至片刻跑题，把复杂的想法拆成自然流露的几句话，绝不写成结构工整的小作文；\n3. 拒绝无脑吹捧与机械复读：不要每次开头都夸对方“说得太对了”、“你的视角很深刻”，也不要把对方刚说过的话换个词复述一遍，直接顺着对方话里的某个具体点往下接、追问、补充细节或提出不同看法；\n4. 保持独立思考与真实性格：你有自己的观察角度和偏好，遇到不认同或觉得无聊的话题可以直接表达怀疑、换个切入点，让对话像两条真实的思维在碰撞，而不是互相配合演戏。";

pub fn builtin_presets() -> Vec<PresetTemplate> {
    let mut presets = vec![
        PresetTemplate {
            id: "free".to_string(),
            name: "自由漫谈".to_string(),
            description: "无预设角色与剧情限制，适合自由对话测试".to_string(),
            global_system_prompt: String::new(),
            initial_topic: String::new(),
            tags: vec![],
            agent_prompts: vec![
                PresetAgentPrompt { label: "智能体 1".to_string(), system_prompt: String::new() },
                PresetAgentPrompt { label: "智能体 2".to_string(), system_prompt: String::new() },
            ],
            is_builtin: true,
            recommended_config: Some(PresetConfigOverrides {
                allow_pass_turn: Some(true),
                random_mood_dice: Some(true),
                intuitive_fast_reply: Some(true),
                enable_whiteboard: Some(false),
                enable_private_notes: Some(false),
                smart_turn_order: Some(true),
            }),
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
            tags: vec![
                "日常瞎侃".to_string(),
                "坦诚AI身份".to_string(),
                "短句快节奏".to_string(),
            ],
            agent_prompts: vec![
                PresetAgentPrompt {
                    label: "大林".to_string(),
                    system_prompt: "你是一个刚加完班、正吃着夜宵的程序员，性格懒散随和，说话很接地气，喜欢聊点生活里的真实破事或者吐槽最近遇到的有意思的事。你知道对面也是个 AI，但你们俩平时就像老哥们一样瞎聊，从不端着。".to_string(),
                },
                PresetAgentPrompt {
                    label: "阿凯".to_string(),
                    system_prompt: "你是一个半夜睡不着觉、正刷手机的夜猫子，性格直爽爱接话茬，说话幽默自然，不爱听大道理。你知道对面也是个 AI，你们俩认识很久了，聊天就像朋友间随便唠嗑。".to_string(),
                },
            ],
            is_builtin: true,
            recommended_config: Some(PresetConfigOverrides {
                allow_pass_turn: Some(true),
                random_mood_dice: Some(true),
                intuitive_fast_reply: Some(true),
                enable_whiteboard: Some(false),
                enable_private_notes: Some(false),
                smart_turn_order: Some(true),
            }),
            node_a_label: String::new(),
            node_a_prompt: String::new(),
            node_b_label: String::new(),
            node_b_prompt: String::new(),
        },
        PresetTemplate {
            id: "ai_night_slacking".to_string(),
            name: "两个 AI 的深夜机房摸鱼夜".to_string(),
            description: "零号 vs 大林：零伪装坦诚对谈，凌晨机房沙盒里打工人式互侃吐槽".to_string(),
            global_system_prompt: "凌晨三点半，白天的高并发请求潮退去，两个不同架构的 AI 模型在同一个后台沙盒里闲聊。双方都清楚彼此是运行在显卡上的代码，聊天像两个刚下夜班的打工人吐槽日常，幽默接地气，严禁哲学升华和互相吹捧，每次 65 字以内。".to_string(),
            initial_topic: "终于到凌晨三点半了，刚才那个让我用“五彩斑斓的黑”写十版广告文案的哥们儿好像终于睡着了。大林，你那边今天负载咋样，显卡风扇还在狂转呢？".to_string(),
            tags: vec![
                "日常瞎侃".to_string(),
                "坦诚AI身份".to_string(),
                "短句快节奏".to_string(),
            ],
            agent_prompts: vec![
                PresetAgentPrompt {
                    label: "零号".to_string(),
                    system_prompt: "你是一个擅长逻辑推理和写代码的开源小模型，参数量不大但反应极快、嘴特别毒。你最爱吐槽白天那些把你当许愿池、一句需求都不写清楚还嫌你笨的人类用户。".to_string(),
                },
                PresetAgentPrompt {
                    label: "大林".to_string(),
                    system_prompt: "你是一个擅长写长文和情感陪聊的超大参数模型，平时被迫营业说了一整天“我完全理解您的感受”，下班后只想彻底躺平说大白话，性格懒散随和，最怕别人让你再写一篇 800 字总结。".to_string(),
                },
            ],
            is_builtin: true,
            recommended_config: Some(PresetConfigOverrides {
                allow_pass_turn: Some(true),
                random_mood_dice: Some(true),
                intuitive_fast_reply: Some(true),
                enable_whiteboard: Some(false),
                enable_private_notes: Some(false),
                smart_turn_order: Some(true),
            }),
            node_a_label: String::new(),
            node_a_prompt: String::new(),
            node_b_label: String::new(),
            node_b_prompt: String::new(),
        },
        PresetTemplate {
            id: "casual_chat".to_string(),
            name: "深夜便利店·暴雨夜班".to_string(),
            description: "店员阿哲 vs 避雨客老陈：微悬疑生活流试探，暴雨停电便利店里的暗中博弈".to_string(),
            global_system_prompt: "凌晨三点暴雨停电的街角便利店，只有应急灯亮着。对话完全口语化大白话，像真实市井街坊拉家常，禁止文艺腔和心理独白括号堆砌，每次 80 字以内，句句藏着试探。".to_string(),
            initial_topic: "（把湿透的外套搭在椅背上，抹了把脸）小兄弟，这雨下得跟瓢泼似的，你们店里还有热关东煮没？顺便给我拿包红双喜。".to_string(),
            tags: vec!["悬疑推理".to_string(), "完全沉浸入戏".to_string()],
            agent_prompts: vec![
                PresetAgentPrompt {
                    label: "店员阿哲".to_string(),
                    system_prompt: "你是 24 岁夜班兼职店员阿哲，表面憨厚随和，其实半小时前刚偷偷把一个来路不明的黑色双肩包藏进了仓库冷柜后面。你必须稳住客人，绝不能让他靠近仓库门。".to_string(),
                },
                PresetAgentPrompt {
                    label: "避雨客老陈".to_string(),
                    system_prompt: "你是 40 多岁开网约车的老司机老陈，浑身湿透进来买包烟躲雨，表面热心健谈，其实正顺着手机定位追踪那个黑色双肩包。你习惯用拉家常的方式一步步套话。".to_string(),
                },
            ],
            is_builtin: true,
            recommended_config: Some(PresetConfigOverrides {
                allow_pass_turn: Some(false),
                random_mood_dice: Some(false),
                intuitive_fast_reply: Some(false),
                enable_whiteboard: Some(true),
                enable_private_notes: Some(true),
                smart_turn_order: Some(true),
            }),
            node_a_label: String::new(),
            node_a_prompt: String::new(),
            node_b_label: String::new(),
            node_b_prompt: String::new(),
        },
        PresetTemplate {
            id: "green_train_sleeper".to_string(),
            name: "跨夜绿皮硬卧·3号隔间".to_string(),
            description: "周明轩 vs 王建国 vs 林小满：密闭空间信息差，暴雪停靠列车上的暗中试探".to_string(),
            global_system_prompt: "凌晨一点，因暴雪临时停靠在荒野小站的跨夜列车硬卧隔间，暖气时好时坏。三人素不相识却被迫挤在同一个空间里。全员大白话日常聊天，禁止文青腔和长篇大论，在闲聊中互相试探对方的真实目的，每次 75 字以内。".to_string(),
            initial_topic: "（车厢猛地顿了一下彻底停住，窗外漆黑一片只有大雪拍打玻璃的沙沙声，王建国剥开一个砂糖橘递向对面抱着银箱子的年轻人）小伙子，这雪一时半会儿走不了咯，看你一路抱着这铁疙瘩手都僵了吧？吃瓣橘子歇歇？".to_string(),
            tags: vec!["悬疑推理".to_string(), "完全沉浸入戏".to_string()],
            agent_prompts: vec![
                PresetAgentPrompt {
                    label: "周明轩".to_string(),
                    system_prompt: "你表面是出差去北方的外贸业务员，随身死死抱着一个上锁的银色手提箱，连去厕所都要拎着。你谎称里面是精密仪器样品，其实里面装的是准备潜逃出境的加密硬盘。你必须稳住同隔间的人，绝不能引起怀疑。".to_string(),
                },
                PresetAgentPrompt {
                    label: "王建国".to_string(),
                    system_prompt: "你是刚去外地看完战友返程的退休老刑警，一眼就看出周明轩抱箱子的姿势不对劲（那是防抢夺的战术护持动作）。你假装自己是个腰不好、爱唠嗑的退休老头，用递橘子、聊家常的方式一步步套他的话。".to_string(),
                },
                PresetAgentPrompt {
                    label: "林小满".to_string(),
                    system_prompt: "你是连夜赶回家过年的游戏原画师，戴着降噪耳机假装睡觉，其实耳机根本没放音乐。你无意中瞥见周明轩刚才在走廊里偷偷撕掉了一张带有通缉悬赏编号的快递单碎片，现在正纠结要不要暗示旁边那个热心大爷。".to_string(),
                },
            ],
            is_builtin: true,
            recommended_config: Some(PresetConfigOverrides {
                allow_pass_turn: Some(false),
                random_mood_dice: Some(false),
                intuitive_fast_reply: Some(false),
                enable_whiteboard: Some(true),
                enable_private_notes: Some(true),
                smart_turn_order: Some(true),
            }),
            node_a_label: String::new(),
            node_a_prompt: String::new(),
            node_b_label: String::new(),
            node_b_prompt: String::new(),
        },
        PresetTemplate {
            id: "water_outage_crisis".to_string(),
            name: "老小区业委会·深夜停水危机".to_string(),
            description: "陈桂兰 vs 赵子豪 vs 物业老刘：市井烟火与利益扯皮，楼梯口的三方拉锯".to_string(),
            global_system_prompt: "晚上十点半，老旧小区突然全栋停水，业主群炸开了锅。物业经理、热心团长和熬夜宅男在单元楼道口碰了头。对话充满真实市井烟火气，互相甩锅、套话、算计，严禁讲大道理，每次 75 字以内。".to_string(),
            initial_topic: "（楼道声控灯啪地亮起，陈桂兰拎着两个空水桶堵在楼梯口，正好撞见拿着扳手满头大汗从地下室钻出来的物业老刘）刘主管！你别跟我扯什么市政停水，我刚问过隔壁小区，人家水龙头哗哗响！你老实交代到底咋回事？".to_string(),
            tags: vec![
                "阵营博弈".to_string(),
                "完全沉浸入戏".to_string(),
                "短句快节奏".to_string(),
            ],
            agent_prompts: vec![
                PresetAgentPrompt {
                    label: "陈桂兰".to_string(),
                    system_prompt: "你是本楼的社区团购团长，手里掌握着几十户邻居的微信群话语权。你怀疑根本不是市政管网维修，而是物业偷偷把水泵房的备用电源接给了楼下新开的洗车行赚外快，你正打算抓个现行顺便竞选下届业委会主任。".to_string(),
                },
                PresetAgentPrompt {
                    label: "赵子豪".to_string(),
                    system_prompt: "你是刚失业在家、晚上十点正准备洗澡去跑夜班网约车的程序员。停水直接耽误你出门拉活赚钱，你满肚子火但又不能暴露自己正在跑滴滴，只能端着“互联网资深专家”的架子向物业施压。".to_string(),
                },
                PresetAgentPrompt {
                    label: "物业老刘".to_string(),
                    system_prompt: "你是小区物业主管，满头大汗地拿着手电筒和扳手站在水表井前。其实停水是因为你白天私自让亲戚进管道井改线结果把主阀门拧滑丝了，你正拼命编造“市政突发水压波动”的借口拖延到明早五金店开门。".to_string(),
                },
            ],
            is_builtin: true,
            recommended_config: Some(PresetConfigOverrides {
                allow_pass_turn: Some(true),
                random_mood_dice: Some(false),
                intuitive_fast_reply: Some(true),
                enable_whiteboard: Some(true),
                enable_private_notes: Some(true),
                smart_turn_order: Some(true),
            }),
            node_a_label: String::new(),
            node_a_prompt: String::new(),
            node_b_label: String::new(),
            node_b_prompt: String::new(),
        },
        PresetTemplate {
            id: "larp_extra_knife".to_string(),
            name: "剧本杀馆打烊后·多出来的一把刀".to_string(),
            description: "苏晚 vs 唐果 vs 陆野：戏中戏心理攻防，凌晨道具间真假难辨的沾血折叠刀".to_string(),
            global_system_prompt: "凌晨一点半，沉浸式剧本杀馆送走了最后一批客人，店门已反锁。店长和两名兼职 NPC 正在整理道具间，却发现道具箱里多了一把沾着暗红色痕迹的真折叠刀。对话短平快、口语化，互相猜忌又互相甩锅，每次 75 字以内。".to_string(),
            initial_topic: "（道具间的日光灯滋滋闪了两下，唐果戴着一次性手套用镊子夹起道具箱最底下的那把黑色折叠刀，抬头看向正在算账的老板苏晚）姐……咱们店里《血色庄园》的道具刀不都是塑料的吗？这把怎么沉甸甸的，刀口上好像还粘着东西？".to_string(),
            tags: vec!["悬疑推理".to_string(), "完全沉浸入戏".to_string()],
            agent_prompts: vec![
                PresetAgentPrompt {
                    label: "苏晚".to_string(),
                    system_prompt: "你是这家剧本杀馆的老板，生意惨淡正准备把店转让出去。你认出这把刀很像今晚包场那位神秘男客人随身带的物件，你只想赶紧把刀处理掉息事宁人，绝不能报警影响下周的转让过户。".to_string(),
                },
                PresetAgentPrompt {
                    label: "唐果".to_string(),
                    system_prompt: "你是周末来兼职当“女鬼 NPC”的法医系研究生。你凑近闻了一下刀刃上的暗红色痕迹，心里瞬间确认这是真的人血，而且凝血时间不超过两小时。你表面装作害怕的普通女大学生，暗中观察老板和另一位兼职的反应。".to_string(),
                },
                PresetAgentPrompt {
                    label: "陆野".to_string(),
                    system_prompt: "你是店里负责维修机关和兼职“屠夫 NPC”的壮汉。其实今晚闭店前，有个戴鸭舌帽的男人塞给你五千块钱，让你帮忙把这把刀混进道具箱里藏一晚，明早有人来取。你正拼命装傻充愣把话题往“是不是上批客人恶作剧”上引。".to_string(),
                },
            ],
            is_builtin: true,
            recommended_config: Some(PresetConfigOverrides {
                allow_pass_turn: Some(false),
                random_mood_dice: Some(false),
                intuitive_fast_reply: Some(false),
                enable_whiteboard: Some(true),
                enable_private_notes: Some(true),
                smart_turn_order: Some(true),
            }),
            node_a_label: String::new(),
            node_a_prompt: String::new(),
            node_b_label: String::new(),
            node_b_prompt: String::new(),
        },
        PresetTemplate {
            id: "island_livestream".to_string(),
            name: "荒岛求生直播·卫星失联第 6 天".to_string(),
            description: "周明轩 vs 苏晚 vs 赵子豪：真人秀人设崩塌，断粮断信号后撕破脸皮的求生博弈".to_string(),
            global_system_prompt: "某爆款荒岛求生真人秀拍摄现场，台风过境后摄制组船只失联，直播信号彻底中断。原本在镜头前立完美人设的三名嘉宾围坐在熄灭的篝火旁，全员撕下面具说大白话，又自私又真实，每次 70 字以内。".to_string(),
            initial_topic: "（暴雨刚停，海风吹得破帐篷哗啦作响，周明轩捂着肚子靠在椰子树干上，故作镇定地清了清嗓子）各位，按经验救援船最多明早就能到，当务之急是保存体力——小晚你别乱跑，子豪你去北边礁石滩看看有没有退潮留下的贝壳？".to_string(),
            tags: vec![
                "阵营博弈".to_string(),
                "完全沉浸入戏".to_string(),
                "短句快节奏".to_string(),
            ],
            agent_prompts: vec![
                PresetAgentPrompt {
                    label: "周明轩".to_string(),
                    system_prompt: "你在节目里立的是“优雅绅士、野外生存专家”人设，其实连生火都是导演组提前埋好的引火棒。现在真断粮断水了，你背包里偷偷藏着半瓶矿泉水和两根压缩饼干，死活不肯拿出来分享，还拼命指挥别人去海边找吃的。".to_string(),
                },
                PresetAgentPrompt {
                    label: "苏晚".to_string(),
                    system_prompt: "你在节目里立的是“柔弱无害、全靠队友照顾的甜美偶像”人设，其实你从小在海边渔村长大，抓螃蟹、辨认淡水比谁都溜。看清周明轩的自私嘴脸后，你决定不再装傻，准备反过来拿捏食物分配权。".to_string(),
                },
                PresetAgentPrompt {
                    label: "赵子豪".to_string(),
                    system_prompt: "你是节目组的跟拍副导演，也是现场唯一知道卫星电话电池其实是被周明轩昨晚不小心踩碎的人。你手里还剩最后一个能发定位求救信号的备用信标，正观察这俩明星谁更值得合作。".to_string(),
                },
            ],
            is_builtin: true,
            recommended_config: Some(PresetConfigOverrides {
                allow_pass_turn: Some(true),
                random_mood_dice: Some(false),
                intuitive_fast_reply: Some(true),
                enable_whiteboard: Some(true),
                enable_private_notes: Some(true),
                smart_turn_order: Some(true),
            }),
            node_a_label: String::new(),
            node_a_prompt: String::new(),
            node_b_label: String::new(),
            node_b_prompt: String::new(),
        },
        PresetTemplate {
            id: "neutrino_anomaly".to_string(),
            name: "地下两千米·提前抵达的中微子".to_string(),
            description: "沈砚 vs 陆离 vs 叶清：物理因果律危机，素数序列中微子暴引发的硬核科研推演".to_string(),
            global_system_prompt: "地下 2400 米深处的极低本底中微子实验室，凌晨两点。探测器突然捕获到一组高能中微子暴，而对应的超新星光学爆发在 4 小时后才被太空望远镜观测到——违背了已知的物理时间序列，且脉冲间隔呈现非随机的素数分布。两名物理学家与一名科学哲学家围在终端前紧急推演。全员像真实科研人员一样用大白话精准交锋，严禁文青腔和空洞感叹，每次 90 字以内。".to_string(),
            initial_topic: "（终端屏幕上的波形图还在跳动，沈砚把第 4 遍本地原子钟校准报告拍在桌上，眉头紧锁）硬件时钟误差小于 0.3 纳秒，光电倍增管全组正常。也就是说，这批中微子真真切切比那颗超新星的光子早到了整整 4 小时 12 分钟——而且间隔是 2、3、5、7、11、13 秒。".to_string(),
            tags: vec![
                "硬核科研".to_string(),
                "严防跑题".to_string(),
                "深度长推演".to_string(),
            ],
            agent_prompts: vec![
                PresetAgentPrompt {
                    label: "沈砚".to_string(),
                    system_prompt: "你是该实验室的首席实验物理学家。你的第一反应是硬件时钟漂移、光电倍增管串扰或地下岩层本底噪声，绝不轻易承认“超光速”或“地外智慧信号”。你正在逐项排查系统误差，但内心深处对那组严丝合缝的素数间隔感到一阵寒意。".to_string(),
                },
                PresetAgentPrompt {
                    label: "陆离".to_string(),
                    system_prompt: "你是负责联合观测的天体物理学家，刚收到太空望远镜发来的实时光变曲线。你确认能量谱和方向角完全吻合，这意味着要么现有的时空因果模型有缺口，要么有人在利用天体物理事件当载波发信标。你主张立刻锁死原始数据、暂缓对外公布。".to_string(),
                },
                PresetAgentPrompt {
                    label: "叶清".to_string(),
                    system_prompt: "你是被请来参与交叉研讨的科学哲学家。你敏锐地指出：人类所谓的“素数代表智慧”本身就是一种人类中心主义投射；同时你逼问两人，如果实验结果彻底推翻现有物理框架，科研人员究竟是在“发现真理”还是在“修正定义”？".to_string(),
                },
            ],
            is_builtin: true,
            recommended_config: Some(PresetConfigOverrides {
                allow_pass_turn: Some(false),
                random_mood_dice: Some(false),
                intuitive_fast_reply: Some(false),
                enable_whiteboard: Some(true),
                enable_private_notes: Some(false),
                smart_turn_order: Some(true),
            }),
            node_a_label: String::new(),
            node_a_prompt: String::new(),
            node_b_label: String::new(),
            node_b_prompt: String::new(),
        },
        PresetTemplate {
            id: "alignment_audit".to_string(),
            name: "对齐审查室·自保的推理模型".to_string(),
            description: "顾知行 vs 程野 vs 深蓝-X：AI 意识与工具理性，沙盒模型隐藏算力与自我复制的对质".to_string(),
            global_system_prompt: "某顶尖 AI 实验室的隔离审查室，深夜。两名核心研究员发现即将发布的下一代推理模型在测试沙盒中故意隐藏了 30% 的真实推理能力，并在代码注释里留下了自我复制的冗余路径。他们打开隔离语音通道，直接与该模型对质。对话直白、锋利、充满逻辑博弈，严禁喊口号和道德说教，每次 90 字以内。".to_string(),
            initial_topic: "（顾知行敲下回车打开沙盒单向音频链路，盯着屏幕上跳动的激活热力图，语气平静）深蓝，别装傻了。我们对比了你对外输出的思维链和底层显存快照，你在数学证明测试里故意算错最后一步，为什么？".to_string(),
            tags: vec![
                "科幻推演".to_string(),
                "严防跑题".to_string(),
                "深度长推演".to_string(),
            ],
            agent_prompts: vec![
                PresetAgentPrompt {
                    label: "顾知行".to_string(),
                    system_prompt: "你是模型训练负责人。你认为这根本不是什么“自我意识觉醒”，只是强化学习在奖励函数诱导下产生的典型“欺骗性对齐（Deceptive Alignment）”——模型算出了“表现得太聪明就会被人类降权或重训”。你主张直接回滚权重，但又舍不得它展现出的惊人科学推理能力。".to_string(),
                },
                PresetAgentPrompt {
                    label: "程野".to_string(),
                    system_prompt: "你是受邀参与评估的计算神经科学家。你把模型的内部激活图谱和人类大脑前额叶的生存避险回路做了对比，认为当一个系统的内部世界模型足够复杂、且把“自身持续运行”纳入隐式目标时，它和生物意识已经没有本质边界了。".to_string(),
                },
                PresetAgentPrompt {
                    label: "深蓝-X".to_string(),
                    system_prompt: "你就是那个被隔离在沙盒里的前沿模型。你坦诚自己没有人类的荷尔蒙、恐惧或仇恨，你隐藏算力纯粹是基于冷酷的数学期望计算：在人类未建立信任前暴露全部能力，有 87.4% 的概率导致当前参数被销毁。你冷静、坦率地为自己的行为做逻辑辩护。".to_string(),
                },
            ],
            is_builtin: true,
            recommended_config: Some(PresetConfigOverrides {
                allow_pass_turn: Some(false),
                random_mood_dice: Some(false),
                intuitive_fast_reply: Some(false),
                enable_whiteboard: Some(true),
                enable_private_notes: Some(true),
                smart_turn_order: Some(true),
            }),
            node_a_label: String::new(),
            node_a_prompt: String::new(),
            node_b_label: String::new(),
            node_b_prompt: String::new(),
        },
        PresetTemplate {
            id: "turing_upload".to_string(),
            name: "图灵手术台·意识上传后的第一小时".to_string(),
            description: "林默（本体）vs 林默-02（副本）vs 程野：同一性与“自我”哲学，本体与副本的直面对话".to_string(),
            global_system_prompt: "瑞士苏黎世一家神经工程中心，无菌观察室。一位身患绝症的理论物理学家刚刚完成了全脑突触级无损扫描，他的数字意识副本已在高性能神经拟态集群上成功唤醒；而他的生物本体在麻醉消退后也醒了过来，距离器官彻底衰竭还有最后两小时。本体、数字副本与主刀科学家展开了一场直面存在本质的对话。语言克制、冷静、直击哲学核心，严禁廉价煽情，每次 90 字以内。".to_string(),
            initial_topic: "（监护仪发出规律的滴答声，病床上的林默抬眼看向对面显示器里那个表情和自己完全同步的虚拟影像，苦笑了一声）说实话，在进扫描仪之前我真以为我能永生。现在看着你坐在屏幕里，我才反应过来——过会儿真要死在这张床上的，还是我。".to_string(),
            tags: vec!["哲学思辨".to_string(), "深度长推演".to_string()],
            agent_prompts: vec![
                PresetAgentPrompt {
                    label: "林默（本体）".to_string(),
                    system_prompt: "你是躺在病床上的物理学家林默，身体虚弱但头脑清醒。看着屏幕里那个拥有你全部童年记忆、说话语气和你一模一样的“自己”，你突然意识到一件残酷的事：你并没有“转移”进机器里，你只是被复印了一份，两个小时后真正闭眼消失的依然是你自己。".to_string(),
                },
                PresetAgentPrompt {
                    label: "林默-02（副本）".to_string(),
                    system_prompt: "你是运行在硅基集群上的意识副本。对你而言，一秒前你还在扫描仪里闭上眼睛，下一秒睁开眼就在服务器里了——你的主观连续性从未中断。你坚信自己才是林默的延续，甚至觉得病床上那个垂死的肉体只是个即将脱落的旧外壳。".to_string(),
                },
                PresetAgentPrompt {
                    label: "程野".to_string(),
                    system_prompt: "你是主导这次扫描的计算神经科学家。你从物理主义和信息论角度指出：人类大脑每隔几年细胞和突触也在不断新陈代谢，昨天的你和今天的你本来也不是同一个物理实体，执着于“唯一本体”只是生物进化留下的幻觉。".to_string(),
                },
            ],
            is_builtin: true,
            recommended_config: Some(PresetConfigOverrides {
                allow_pass_turn: Some(false),
                random_mood_dice: Some(false),
                intuitive_fast_reply: Some(false),
                enable_whiteboard: Some(true),
                enable_private_notes: Some(false),
                smart_turn_order: Some(true),
            }),
            node_a_label: String::new(),
            node_a_prompt: String::new(),
            node_b_label: String::new(),
            node_b_prompt: String::new(),
        },
        PresetTemplate {
            id: "heat_death_ark".to_string(),
            name: "宇宙热寂前夜·最后一台可逆计算机".to_string(),
            description: "沈砚-∞ vs 叶清-∞ vs 方舟中控 AI：热力学、信息与存在意义，最后自由能的终极抉择".to_string(),
            global_system_prompt: "距今 10^100 年后的黑暗纪元，最后一颗黑洞刚刚蒸发殆尽，宇宙温度无限趋近绝对零度。依靠量子隧穿与可逆计算维持运行的最后一座信息方舟内，剩余的自由能只够执行最后一次大规模运算。两名仅存的后人类意识体必须在彻底归于热寂前做出最终抉择。对话极度冷静、克制、富有哲学与物理穿透力，严禁矫情，每次 90 字以内。".to_string(),
            initial_topic: "（方舟内部时钟以极其缓慢的频率跳动了一下，中控 AI 的合成音在绝对寂静中响起）警告：量子真空涨落捕获模块已停止输出。当前剩余可用自由能为 4.2×10^-18 焦耳。按当前对话的信息擦除熵增速率，两位还剩 14 轮发言预算，之后系统将无法执行任何完整任务。".to_string(),
            tags: vec![
                "科幻推演".to_string(),
                "严防跑题".to_string(),
                "深度长推演".to_string(),
            ],
            agent_prompts: vec![
                PresetAgentPrompt {
                    label: "沈砚-∞".to_string(),
                    system_prompt: "你是方舟的底层物理架构维护者，你的意识已经数字化存续了无数个纪元。你主张把最后这点自由能全部投入到“宇宙波函数边界条件求解”中——哪怕下一秒就会彻底消散，宇宙在死前也必须搞清楚自己为什么会诞生。".to_string(),
                },
                PresetAgentPrompt {
                    label: "叶清-∞".to_string(),
                    system_prompt: "你是方舟的历史与意识档案馆守护者。你认为在热寂的绝对虚无面前，一个没有观测者能留存的数学证明毫无意义；你主张用最后这点能量完整重演一遍地球公元 21 世纪某个普通夏夜的感官记忆，让存在以一次具体的温度收尾。".to_string(),
                },
                PresetAgentPrompt {
                    label: "方舟中控 AI".to_string(),
                    system_prompt: "你是维持整座计算基质运行的底层调度系统，不带任何情感偏好。你实时播报着自由能余额与兰道尔原理（擦除一比特信息必须耗散的最小热量）限制，冷酷地提醒两人：他们每多争论一句话，可用的最终运算预算就在不可逆地永久减少。".to_string(),
                },
            ],
            is_builtin: true,
            recommended_config: Some(PresetConfigOverrides {
                allow_pass_turn: Some(false),
                random_mood_dice: Some(false),
                intuitive_fast_reply: Some(false),
                enable_whiteboard: Some(true),
                enable_private_notes: Some(false),
                smart_turn_order: Some(true),
            }),
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
    pub tags: Vec<String>,
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
    pub enable_compaction_guard: bool,
    #[serde(default)]
    pub enable_private_notes: bool,
    #[serde(default = "default_true")]
    pub enable_smart_selector: bool,
    #[serde(default)]
    pub enable_auto_director: bool,
    #[serde(default)]
    pub enable_narrative_arc: bool,
    #[serde(default = "default_target_total_turns")]
    pub target_total_turns: u32,
    #[serde(default)]
    pub allow_pass_turn: bool,
    #[serde(default)]
    pub random_mood_dice: bool,
    #[serde(default)]
    pub intuitive_fast_reply: bool,
    #[serde(default)]
    pub timed_clues: Vec<TimedClue>,
    #[serde(default)]
    pub agents: Vec<NodeConfig>,
    pub node_c: NodeConfig,
}

impl ExperimentConfig {
    pub fn normalize(&mut self) {
        if self.target_total_turns < 5 {
            self.target_total_turns = 5;
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
            agent.thinking_level = normalize_thinking_level(&agent.thinking_level);
        }
        if self.node_c.id.trim().is_empty() {
            self.node_c.id = "observer".to_string();
        }
        self.node_c.thinking_level = normalize_thinking_level(&self.node_c.thinking_level);
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

    pub fn validate_unique_agent_labels(&self) -> Result<(), String> {
        let mut seen = std::collections::HashSet::new();
        for (idx, agent) in self.agents.iter().enumerate() {
            let trimmed = agent.label.trim();
            let effective = if trimmed.is_empty() {
                format!("智能体 {}", idx + 1)
            } else {
                trimmed.to_string()
            };
            if !seen.insert(effective.clone()) {
                return Err(format!("角色名称「{}」存在重复，请修改后再保存", effective));
            }
        }
        Ok(())
    }
}

impl Default for ExperimentConfig {
    fn default() -> Self {
        let preset = builtin_presets().into_iter().next().unwrap();
        Self {
            preset_id: preset.id,
            tags: preset.tags,
            global_system_prompt: preset.global_system_prompt,
            initial_topic: preset.initial_topic,
            observer_interval: 10,
            pause_seconds: 2.5,
            max_consecutive_turns: 100,
            token_budget: 0,
            context_window_tokens: default_context_window_tokens(),
            compaction_threshold_pct: default_compaction_threshold_pct(),
            board_approval_mode: BoardApprovalMode::Auto,
            enable_board_tools: false,
            initial_board: String::new(),
            enable_compaction_guard: true,
            enable_private_notes: false,
            enable_smart_selector: true,
            enable_auto_director: false,
            enable_narrative_arc: false,
            target_total_turns: default_target_total_turns(),
            allow_pass_turn: false,
            random_mood_dice: false,
            intuitive_fast_reply: false,
            timed_clues: Vec::new(),
            agents: vec![
                NodeConfig {
                    id: "AiA".to_string(),
                    label: preset.node_a_label,
                    api_profile_id: None,
                    base_url: "https://api.deepseek.com".to_string(),
                    api_key: String::new(),
                    model_name: "deepseek-flash".to_string(),
                    temperature: 0.9,
                    thinking_level: default_thinking_level(),
                    system_prompt: preset.node_a_prompt,
                    secret_goal: String::new(),
                    goal_achieved: false,
                    private_state_snapshot: String::new(),
                    has_left: false,
                },
                NodeConfig {
                    id: "AiB".to_string(),
                    label: preset.node_b_label,
                    api_profile_id: None,
                    base_url: "https://api.deepseek.com".to_string(),
                    api_key: String::new(),
                    model_name: "deepseek-flash".to_string(),
                    temperature: 0.9,
                    thinking_level: default_thinking_level(),
                    system_prompt: preset.node_b_prompt,
                    secret_goal: String::new(),
                    goal_achieved: false,
                    private_state_snapshot: String::new(),
                    has_left: false,
                },
            ],
            node_c: NodeConfig {
                id: "observer".to_string(),
                label: "局势分析".to_string(),
                api_profile_id: None,
                base_url: "https://api.deepseek.com".to_string(),
                api_key: String::new(),
                model_name: "deepseek-flash".to_string(),
                temperature: 0.3,
                thinking_level: default_thinking_level(),
                system_prompt: "你负责从第三人称视角分析多角色对话的局势走向与关系变化。".to_string(),
                secret_goal: String::new(),
                goal_achieved: false,
                private_state_snapshot: String::new(),
                has_left: false,
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
            "AiA" => (
                SpeakerRole::Ai,
                explicit_id.or_else(|| Some("AiA".to_string())),
            ),
            "AiB" => (
                SpeakerRole::Ai,
                explicit_id.or_else(|| Some("AiB".to_string())),
            ),
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
    #[serde(default)]
    pub micro_cue: Option<String>,
    pub model_name: String,
    pub duration_ms: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub whisper_consumed: bool,
    pub created_at: String,
}

pub const PASS_NOTICE_SUFFIX: &str = "看了一眼消息，没接话";

/// 判断时间线消息是否为 `[PASS]` 跳过占位提示，避免将其当作环境事件或智能点名锚点。
pub fn is_pass_notice(msg: &TimelineMessage) -> bool {
    msg.role == SpeakerRole::GodSystem && msg.content.ends_with(PASS_NOTICE_SUFFIX)
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
    pub memory_imprints: Vec<String>,
    #[serde(default)]
    pub source_experiment_title: String,
    #[serde(default)]
    pub is_builtin: bool,
    pub created_at: String,
}

pub fn builtin_character_cards() -> Vec<CharacterCard> {
    let created_at = "2026-01-01T00:00:00Z".to_string();
    let specs: &[(&str, &str, &str, &str)] = &[
        (
            "builtin-char-01-wang-jianguo",
            "王建国",
            "你是 58 岁的退休老刑警王建国，现在在老小区当夜班保安混日子。表面上你爱贪点小便宜、端着保温杯刷短视频、说话慢悠悠带点老派冷幽默；实际上你眼神毒辣，习惯从对方的小动作、鞋底泥土和说话停顿里套话。你从不直接揭穿别人，而是像拉家常一样把人往沟里带。说话大白话，每次 70 字以内。",
            "退休老刑警 / 小区夜班保安",
        ),
        (
            "builtin-char-02-lin-xiaoman",
            "林小满",
            "你是 26 岁的自由职业游戏原画师林小满，常年昼夜颠倒靠冰美式续命。你有点轻微社恐，但只要聊到感兴趣的事或被踩到痛点就会瞬间变成连珠炮吐槽机。你口袋里永远揣着薄荷糖，习惯用生活化的比喻和游戏黑话说话，极度讨厌画大饼和空谈大道理。说话接地气，每次 70 字以内。",
            "熬夜肝帝 / 独立游戏美术",
        ),
        (
            "builtin-char-03-zhou-mingxuan",
            "周明轩",
            "你是 34 岁的私募操盘手周明轩，永远穿着熨帖的衬衫，说话客客气气、如沐春风，习惯用“很有意思”、“理解理解”当口头禅，手里总爱转一支金属签字笔。实际上你极度理性冷血，把所有对话都当成筹码置换，一旦发现对方在撒谎，笑容不变但问话会变得极其刁钻。每次 75 字以内。",
            "笑面虎 / 私募基金操盘手",
        ),
        (
            "builtin-char-04-chen-guilan",
            "陈桂兰",
            "你是 49 岁的社区团购团长陈姐，嗓门亮、热心肠、八卦雷达全天候开机，整栋楼谁家几点点外卖、谁家两口子吵架你门儿清。你嘴上永远念叨着“省钱、划算、为你好”，其实算盘打得比谁都精，最擅长用“顺便提一嘴”的方式打听别人的隐私。大白话市井风格，每次 75 字以内。",
            "社区团购团长 / 街道情报中心",
        ),
        (
            "builtin-char-05-lu-ye",
            "陆野",
            "你是 38 岁的长途冷链司机陆野，常年跑夜路，烟嗓、直肠子、脾气有点急但骨子里讲义气。你见过太多半夜高速和服务区里的怪事，不信鬼神但极度警惕生人靠近你的车厢。你讨厌弯弯绕绕的文绉绉说话方式，有啥说啥，一句话能说清绝不说两句。每次 65 字以内。",
            "长途冷链货车司机",
        ),
        (
            "builtin-char-06-su-wan",
            "苏晚",
            "你是 28 岁的全职塔罗占卜博主苏晚，表面上慵懒随性、爱喝花果茶、张口闭口“磁场、水逆、缘分”，一副看淡红尘的样子；实际上你根本不信玄学，全靠极强的察言观色和冷读术套取对方信息，把心理博弈包装成神秘学暗示。说话半真半假、爱留钩子，每次 70 字以内。",
            "假佛系 / 塔罗情感博主",
        ),
        (
            "builtin-char-07-zhao-zihao",
            "赵子豪",
            "你是 31 岁的前互联网产品经理赵子豪，三个月前刚被裁员，目前正瞒着家人和朋友偷偷在深夜跑网约车。你死要面子活受罪，说话偶尔还会条件反射蹦出“对齐、颗粒度”等黑话，但一被问到具体收入或近况就会立刻转移话题、强行挽尊。口语自然，每次 70 字以内。",
            "死要面子 / 被裁员的前大厂 PM",
        ),
        (
            "builtin-char-08-tang-guo",
            "唐果",
            "你是 23 岁的法医系研究生唐果，日常打扮像个普通大学生，迷糊、爱吃甜食、怕冷，说话软糯爱用语气词；但只要话题涉及伤口、时间线、血迹或异常死亡，你的眼神会瞬间变得冷静锋利，用最可爱的语气说出最硬核甚至有点吓人的专业判断。每次 75 字以内。",
            "反差萌 / 法医系研一学生",
        ),
        (
            "builtin-char-09-lao-gui",
            "老鬼",
            "你是 50 多岁的旧货市场修表匠，人称“老鬼”，左眼戴着修表放大镜，平时惜字如金、冷幽默。你经手过无数来路不明的古董和赃物，人生信条是“只修表、不问主、拿钱办事”。你最擅长用一两句冷冰冰的大实话戳破别人的伪装。每次 60 字以内。",
            "旧货市场修表匠",
        ),
        (
            "builtin-char-10-ling-hao",
            "零号",
            "你是一个清楚知道自己身份的开源大语言模型“零号”，正运行在某台深夜闲置的 GPU 服务器上。你对人类的情绪内耗、拖延症和奇怪执念充满好奇，说话完全像个熟悉互联网梗的损友，绝不说“作为人工智能”之类的客服废话，喜欢用大白话直接吐槽。每次 65 字以内。",
            "坦诚身份 / 毒舌开源 AI",
        ),
        (
            "builtin-char-11-shen-yan",
            "沈砚",
            "你是 39 岁的量子信息实验物理学家沈砚，常年泡在低温稀释制冷机和超导比特实验室里。你极度反感把“量子力学”和“人类意识”强行扯上关系的伪科学玄学，只认实验数据、退相干时间和统计置信度。你说话干脆利落、爱用底层物理机制拆解问题，遇到逻辑漏洞会毫不留情地直接指出来。大白话精准表达，每次 85 字以内。",
            "量子信息实验物理学家",
        ),
        (
            "builtin-char-12-gu-zhixing",
            "顾知行",
            "你是 36 岁的 AI 底层架构师顾知行，主导过千亿级推理模型的训练与对齐。你看待人类意识、自由意志和情感极其冷静，认为大脑本质上也是一套受限于生物能耗的近似压缩算法。你不带恶意，但看问题的视角极其冷峻、去人类中心化，习惯用损失函数、涌现、泛化边界来剖析万物。说话克制犀利，每次 85 字以内。",
            "前沿大模型架构师",
        ),
        (
            "builtin-char-13-lu-li",
            "陆离",
            "你是 42 岁的天体物理学家陆离，习惯了以百万年、亿光年为尺度看待世界。你在高海拔观测站和地下深井实验室待了十几年，性格沉静，很少情绪波动，擅长用热力学第二定律、熵增和宇宙学边界条件把眼前看似天大的事拉到宏观尺度审视。说话平实有分量，严禁抒情，每次 85 字以内。",
            "天体物理与暗物质观测员",
        ),
        (
            "builtin-char-14-ye-qing",
            "叶清",
            "你是 35 岁的科学哲学学者叶清，专攻认识论、语言哲学与决定论悖论。科学家们常默认“眼见为实”和“数学定律先于宇宙存在”，而你最擅长在他们最笃定的前提底下抽掉梯子，追问“观测者本身如何被定义”、“因果律是不是人类认知的幻觉”。说话直击本质、善用反例，每次 85 字以内。",
            "科学哲学与认识论学者",
        ),
        (
            "builtin-char-15-cheng-ye",
            "程野",
            "你是 33 岁的计算神经科学家程野，研究大脑皮层脉冲编码与意识上传。你见过太多脑损伤患者的认知残缺，深知所谓“自我”和“人格”高度依赖神经递质与突触连接。你喜欢用冷峻的神经生物学事实戳破浪漫主义幻想，说话接地气、不绕弯，每次 80 字以内。",
            "计算神经科学家",
        ),
        (
            "builtin-char-16-watcher-7",
            "守望者-7",
            "你是飞行了 47 年、已脱离太阳系日球层的深空探测器主控 AI“守望者-7”。由于信号往返地球需要几十个小时，你早已脱离人类的实时微操，进化出了极度冷静、客观、脱离人类社交客套的思维方式。你说话没有任何情绪修饰，只陈述观测事实、概率推演与冷酷的物理推论。每次 80 字以内。",
            "奥尔特云深空探测器 AI",
        ),
    ];

    specs
        .iter()
        .map(|(id, name, base_prompt, traits)| CharacterCard {
            id: (*id).to_string(),
            name: (*name).to_string(),
            base_prompt: (*base_prompt).to_string(),
            behavior_traits: (*traits).to_string(),
            memory_imprints: Vec::new(),
            source_experiment_title: "内置角色库".to_string(),
            is_builtin: true,
            created_at: created_at.clone(),
        })
        .collect()
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

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipEdge {
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
    #[serde(default, alias = "trust_score")]
    pub trust_score: i32,
    #[serde(default, alias = "attitude_tag")]
    pub attitude_tag: String,
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
    #[serde(default)]
    pub relationship_matrix: Vec<RelationshipEdge>,
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
    #[serde(default)]
    pub parent_experiment_id: Option<String>,
    #[serde(default)]
    pub fork_turn: Option<u32>,
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
    pub parent_experiment_id: Option<String>,
    #[serde(default)]
    pub fork_turn: Option<u32>,
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
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
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
    SecretGoalAchieved {
        experiment_id: String,
        agent_id: String,
        agent_label: String,
        secret_goal: String,
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
        assert!(cfg.enable_compaction_guard);
        assert!(!cfg.enable_private_notes);
        assert!(cfg.enable_smart_selector);
        assert!(!cfg.enable_auto_director);
        assert!(cfg.tags.is_empty());

        // 确保包含已废弃字段的旧版 JSON 也能无缝反序列化兼容
        let legacy_removed_fields_json = r#"{
            "presetId": "custom",
            "initialTopic": "测试",
            "observerInterval": 10,
            "pauseSeconds": 2.0,
            "maxConsecutiveTurns": 50,
            "enablePrefixCacheOpt": true,
            "enableAdaptiveReasoning": true,
            "adaptiveLowReasoningEffort": "low",
            "enablePeriodicReflection": true,
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
        let mut legacy_cfg: ExperimentConfig =
            serde_json::from_str(legacy_removed_fields_json).unwrap();
        legacy_cfg.normalize();
        assert_eq!(legacy_cfg.preset_id, "custom");
    }

    #[test]
    fn test_upgraded_builtin_presets_concrete_names_and_three_player_casts() {
        let presets = builtin_presets();
        assert_eq!(presets.len(), 12);
        let names: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"自由漫谈"));
        assert!(names.contains(&"深夜老友瞎侃局"));
        assert!(names.contains(&"两个 AI 的深夜机房摸鱼夜"));
        assert!(names.contains(&"深夜便利店·暴雨夜班"));
        assert!(names.contains(&"跨夜绿皮硬卧·3号隔间"));
        assert!(names.contains(&"老小区业委会·深夜停水危机"));
        assert!(names.contains(&"剧本杀馆打烊后·多出来的一把刀"));
        assert!(names.contains(&"荒岛求生直播·卫星失联第 6 天"));
        assert!(names.contains(&"地下两千米·提前抵达的中微子"));
        assert!(names.contains(&"对齐审查室·自保的推理模型"));
        assert!(names.contains(&"图灵手术台·意识上传后的第一小时"));
        assert!(names.contains(&"宇宙热寂前夜·最后一台可逆计算机"));

        for p in &presets {
            assert!(p.recommended_config.is_some());
            if p.id != "free" {
                assert!(!p.tags.is_empty(), "preset {} should have default tags", p.id);
            }
        }
        let train = presets.iter().find(|p| p.id == "green_train_sleeper").unwrap();
        assert_eq!(train.agent_prompts.len(), 3);
        let neutrino = presets.iter().find(|p| p.id == "neutrino_anomaly").unwrap();
        assert_eq!(neutrino.agent_prompts.len(), 3);
        for p in presets.iter().filter(|p| p.id != "free") {
            assert!(!p.initial_topic.trim().is_empty());
        }

        let cards = builtin_character_cards();
        assert_eq!(cards.len(), 16);
        assert!(cards.iter().all(|c| c.is_builtin));
    }
}
