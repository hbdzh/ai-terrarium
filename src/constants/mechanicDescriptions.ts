export type MechanicKey =
  | "intuitiveFastReply"
  | "allowPassTurn"
  | "randomMoodDice"
  | "enablePrivateNotes"
  | "enableBoardTools"
  | "enableSmartSelector";

export interface MechanicDescriptor {
  key: MechanicKey;
  title: string;
  onEffect: string;
  offEffect: string;
}

export const MECHANIC_DESCRIPTIONS: Record<MechanicKey, MechanicDescriptor> = {
  intuitiveFastReply: {
    key: "intuitiveFastReply",
    title: "直觉快反应",
    onEffect: "抑制长篇内心独白，强制第一反应大白话短句秒回（适合日常闲聊、快节奏互怼）",
    offEffect: "允许模型在 <think> 中充分推演逻辑与潜台词（适合硬核物理/哲学研讨、悬疑推理）",
  },
  allowPassTurn: {
    key: "allowPassTurn",
    title: "允许[PASS]跳过",
    onEffect: "无话可说或插不上嘴时允许输出 [PASS] 跳过本轮，避免尬聊凑字数（推荐 3 人以上群聊开启）",
    offEffect: "强制每轮轮到的角色必须开口发言（适合双人紧密对戏）",
  },
  randomMoodDice: {
    key: "randomMoodDice",
    title: "随机微情绪",
    onEffect: "后台每数轮随机注入走神、犯困、想换话题等生活扰动，对话更像活人冲浪（易跑题，仅建议闲聊局开启）",
    offEffect: "严格保持角色专注度与话题连贯性（推荐深度研讨、剧本推理关闭）",
  },
  enablePrivateNotes: {
    key: "enablePrivateNotes",
    title: "角色私密备忘",
    onEffect: "允许 AI 维护仅自己可见的隐藏心计与怀疑笔记（易激发猜忌与暗中博弈，适合悬疑/宫斗/卧底局；学术局请关闭）",
    offEffect: "所有角色坦诚公开思考，不搞暗中猜忌与狼人杀式盘问",
  },
  enableBoardTools: {
    key: "enableBoardTools",
    title: "共享白板",
    onEffect: "提供全员可见的公共黑板，引导 AI 实时记录共识、线索与核心分歧（强力锚定主线防跑题，适合研讨与推理）",
    offEffect: "纯靠口头对话推进，不维护公共笔记（适合轻松日常闲聊）",
  },
  enableSmartSelector: {
    key: "enableSmartSelector",
    title: "智能点名",
    onEffect: "根据上文被点名或话题相关性动态决定下一位发言者，插话更自然",
    offEffect: "严格按固定顺序轮流发言",
  },
};

export function summarizeActiveMechanics(state: Record<MechanicKey, boolean>): {
  text: string;
  isCaution: boolean;
} {
  if (state.randomMoodDice && state.enablePrivateNotes) {
    return {
      isCaution: true,
      text: "提示：已同时开启「随机微情绪」与「角色私密备忘」，对话易跑题且容易衍生暗中猜忌；若用于严肃学术或推理局，建议关闭上述两项。",
    };
  }
  if (state.randomMoodDice) {
    return {
      isCaution: true,
      text: "提示：已开启「随机微情绪」，角色会随机走神或换话题（仅建议日常闲聊开启；深度研讨或剧本推理请关闭）。",
    };
  }
  if (state.enablePrivateNotes) {
    return {
      isCaution: true,
      text: "提示：已开启「角色私密备忘」，角色会维护私下怀疑笔记并激发猜忌博弈（适合悬疑/卧底局；学术讨论请关闭）。",
    };
  }
  if (state.intuitiveFastReply && state.enableBoardTools) {
    return {
      isCaution: false,
      text: "当前已开启「共享白板」与「直觉快反应」：兼顾主线共识沉淀与口语短句互动（如需深入 <think> 推演可关闭直觉快反应）。",
    };
  }
  if (state.intuitiveFastReply) {
    return {
      isCaution: false,
      text: "当前偏向轻松口语快反应节奏，适合日常闲聊或快节奏互怼；硬核物理/哲学推演建议关闭「直觉快反应」。",
    };
  }
  if (state.enableBoardTools) {
    return {
      isCaution: false,
      text: "当前偏向深度理性协作与推理模式（共享白板锚定主线、关闭情绪扰动与暗中备忘）；悬停上方开关可查看开/关对照。",
    };
  }
  return {
    isCaution: false,
    text: "当前偏向纯粹严谨对话流（专注主题推演、不设白板与私密猜忌）；悬停上方任意开关可查看开/关对照。",
  };
}
