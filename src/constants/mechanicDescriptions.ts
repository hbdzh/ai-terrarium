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
    title: "快反应回复",
    onEffect: "跳过冗长内心戏，用简短口语快速接话",
    offEffect: "允许模型充分思考逻辑与潜台词后回复",
  },
  allowPassTurn: {
    key: "allowPassTurn",
    title: "允许跳过发言",
    onEffect: "无话可说时可跳过本轮，避免强行凑字数",
    offEffect: "每轮轮到的角色都必须发言",
  },
  randomMoodDice: {
    key: "randomMoodDice",
    title: "随机情绪扰动",
    onEffect: "随机注入走神、换话题等状态，对话更随意",
    offEffect: "保持角色专注度与主线连贯性",
  },
  enablePrivateNotes: {
    key: "enablePrivateNotes",
    title: "角色私密备忘",
    onEffect: "允许角色记录仅自己可见的判断与计划",
    offEffect: "不记录私密备忘，全员公开对话",
  },
  enableBoardTools: {
    key: "enableBoardTools",
    title: "共享白板",
    onEffect: "提供公共白板记录共识、线索与分歧",
    offEffect: "纯对话推进，不维护公共记录",
  },
  enableSmartSelector: {
    key: "enableSmartSelector",
    title: "智能点名",
    onEffect: "根据上下文相关性动态决定下一位发言者",
    offEffect: "按固定顺序轮流发言",
  },
};

export function summarizeActiveMechanics(state: Record<MechanicKey, boolean>): {
  text: string;
  isCaution: boolean;
} {
  if (state.randomMoodDice && state.enablePrivateNotes) {
    return {
      isCaution: true,
      text: "同时开启了「随机情绪扰动」与「角色私密备忘」，对话易发散且偏博弈；严肃讨论建议关闭。",
    };
  }
  if (state.randomMoodDice) {
    return {
      isCaution: true,
      text: "已开启「随机情绪扰动」，角色会偶发走神或换话题；深度讨论或推理建议关闭。",
    };
  }
  if (state.enablePrivateNotes) {
    return {
      isCaution: true,
      text: "已开启「角色私密备忘」，角色会维护个人隐藏笔记，适合推理或阵营局。",
    };
  }
  if (state.intuitiveFastReply && state.enableBoardTools) {
    return {
      isCaution: false,
      text: "已开启「共享白板」与「快反应回复」，兼顾共识记录与短句节奏。",
    };
  }
  if (state.intuitiveFastReply) {
    return {
      isCaution: false,
      text: "当前偏向轻松口语短句；需要深度推演时可关闭「快反应回复」。",
    };
  }
  if (state.enableBoardTools) {
    return {
      isCaution: false,
      text: "当前偏向理性协作与推理，共享白板将持续记录关键信息。",
    };
  }
  return {
    isCaution: false,
    text: "当前为纯粹对话模式，悬停开关可查看各项规则说明。",
  };
}
