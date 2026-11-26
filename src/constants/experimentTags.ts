import type { MechanicKey } from "./mechanicDescriptions";

export interface ExperimentTagSpec {
  id: string;
  label: string;
  desc: string;
  autoOn?: MechanicKey[];
  autoOff?: MechanicKey[];
  lockOff?: MechanicKey[];
}

export const MAIN_TONE_TAGS: ExperimentTagSpec[] = [
  {
    id: "日常瞎侃",
    label: "日常瞎侃",
    desc: "群聊口语风格，支持自然跑题与调侃",
    autoOn: ["randomMoodDice", "allowPassTurn", "intuitiveFastReply"],
    lockOff: ["enablePrivateNotes"],
  },
  {
    id: "悬疑推理",
    label: "悬疑推理",
    desc: "信息差试探，各自保留底牌",
    autoOn: ["enablePrivateNotes", "enableBoardTools"],
    lockOff: ["randomMoodDice"],
  },
  {
    id: "硬核科研",
    label: "硬核科研",
    desc: "紧扣核心命题推演，注重逻辑与事实",
    autoOn: ["enableBoardTools"],
    lockOff: ["randomMoodDice", "enablePrivateNotes", "intuitiveFastReply"],
  },
  {
    id: "哲学思辨",
    label: "哲学思辨",
    desc: "专注概念拆解与逻辑交锋，深入追问",
    autoOn: ["enableBoardTools"],
    lockOff: ["randomMoodDice", "intuitiveFastReply"],
  },
  {
    id: "科幻推演",
    label: "科幻推演",
    desc: "基于设定的边界推演，表达冷静克制",
    autoOn: ["enableBoardTools"],
    lockOff: ["randomMoodDice"],
  },
  {
    id: "阵营博弈",
    label: "阵营博弈",
    desc: "多方利益冲突，支持拉拢与私下密谋",
    autoOn: ["enablePrivateNotes", "enableBoardTools"],
    lockOff: ["randomMoodDice"],
  },
  {
    id: "荒诞喜剧",
    label: "荒诞喜剧",
    desc: "一本正经地制造反差幽默",
    autoOn: ["randomMoodDice", "allowPassTurn", "intuitiveFastReply"],
    lockOff: ["enablePrivateNotes"],
  },
  {
    id: "治愈夜话",
    label: "治愈夜话",
    desc: "温和陪伴与深夜倾诉，语气细腻自然",
    autoOn: ["intuitiveFastReply"],
    lockOff: ["enablePrivateNotes", "randomMoodDice"],
  },
];

export interface BoundaryPairSpec {
  id: string;
  left: ExperimentTagSpec;
  right: ExperimentTagSpec;
}

export const BOUNDARY_TAG_PAIRS: BoundaryPairSpec[] = [
  {
    id: "topic_focus",
    left: {
      id: "严防跑题",
      label: "严防跑题",
      desc: "紧扣主线话题，关闭随机情绪扰动",
      lockOff: ["randomMoodDice"],
    },
    right: {
      id: "自由发散",
      label: "自由发散",
      desc: "允许跨话题跳跃与自由联想",
    },
  },
  {
    id: "ai_identity",
    left: {
      id: "坦诚AI身份",
      label: "坦诚AI身份",
      desc: "允许角色坦然承认 AI 身份",
    },
    right: {
      id: "完全沉浸入戏",
      label: "完全沉浸入戏",
      desc: "保持角色扮演，不跳出设定",
    },
  },
  {
    id: "pacing_depth",
    left: {
      id: "短句快节奏",
      label: "短句快节奏",
      desc: "开启快反应模式，发言控制在 70 字内",
      autoOn: ["intuitiveFastReply"],
    },
    right: {
      id: "深度长推演",
      label: "深度长推演",
      desc: "关闭快反应模式，保留完整思考过程",
      autoOff: ["intuitiveFastReply"],
    },
  },
];

const ALL_TAG_MAP = new Map<string, ExperimentTagSpec>();
for (const t of MAIN_TONE_TAGS) {
  ALL_TAG_MAP.set(t.id, t);
}
for (const p of BOUNDARY_TAG_PAIRS) {
  ALL_TAG_MAP.set(p.left.id, p.left);
  ALL_TAG_MAP.set(p.right.id, p.right);
}

export function getTagSpec(tagId: string): ExperimentTagSpec | undefined {
  return ALL_TAG_MAP.get(tagId);
}

/** Returns a map of MechanicKey -> locking tag label (or null if unlocked). */
export function getLockedMechanics(tags: string[]): Record<MechanicKey, string | null> {
  const locked: Record<MechanicKey, string | null> = {
    intuitiveFastReply: null,
    allowPassTurn: null,
    randomMoodDice: null,
    enablePrivateNotes: null,
    enableBoardTools: null,
    enableSmartSelector: null,
  };
  for (const tagId of tags) {
    const spec = ALL_TAG_MAP.get(tagId);
    if (!spec?.lockOff) continue;
    for (const key of spec.lockOff) {
      if (!locked[key]) {
        locked[key] = spec.label;
      }
    }
  }
  return locked;
}

/** Applies tag lock-offs onto a mechanics state object without altering unlocked toggles. */
export function applyTagLocksToMechanics(
  tags: string[],
  mechanics: Record<MechanicKey, boolean>
): Record<MechanicKey, boolean> {
  const locked = getLockedMechanics(tags);
  const next = { ...mechanics };
  (Object.keys(locked) as MechanicKey[]).forEach((k) => {
    if (locked[k]) {
      next[k] = false;
    }
  });
  return next;
}

/**
 * Toggles a tag (respecting Row 1 single-select mutual exclusion & Row 2 pairwise mutual exclusion),
 * then applies newly selected tag's autoOn/autoOff rules and enforces all active lockOff constraints.
 */
export function toggleExperimentTag(
  currentTags: string[],
  targetTagId: string,
  currentMechanics: Record<MechanicKey, boolean>
): { tags: string[]; mechanics: Record<MechanicKey, boolean> } {
  const isAlreadySelected = currentTags.includes(targetTagId);
  let nextTags: string[];

  if (isAlreadySelected) {
    nextTags = currentTags.filter((t) => t !== targetTagId);
    return {
      tags: nextTags,
      mechanics: applyTagLocksToMechanics(nextTags, currentMechanics),
    };
  }

  // Remove mutually exclusive peers
  const isMain = MAIN_TONE_TAGS.some((t) => t.id === targetTagId);
  if (isMain) {
    const mainIds = new Set(MAIN_TONE_TAGS.map((t) => t.id));
    nextTags = currentTags.filter((t) => !mainIds.has(t));
  } else {
    const pair = BOUNDARY_TAG_PAIRS.find(
      (p) => p.left.id === targetTagId || p.right.id === targetTagId
    );
    if (pair) {
      nextTags = currentTags.filter((t) => t !== pair.left.id && t !== pair.right.id);
    } else {
      nextTags = [...currentTags];
    }
  }
  nextTags.push(targetTagId);

  const nextMechanics = { ...currentMechanics };
  const spec = ALL_TAG_MAP.get(targetTagId);
  if (spec?.autoOn) {
    for (const k of spec.autoOn) {
      nextMechanics[k] = true;
    }
  }
  if (spec?.autoOff) {
    for (const k of spec.autoOff) {
      nextMechanics[k] = false;
    }
  }

  return {
    tags: nextTags,
    mechanics: applyTagLocksToMechanics(nextTags, nextMechanics),
  };
}
