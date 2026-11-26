<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { X, Plus, Trash2, Sparkles, Loader2, Save, Lock } from "lucide-vue-next";
import {
  disambiguateAgentLabel,
  duplicateLabelSet,
  effectiveAgentLabel,
  findDuplicateAgentLabel,
  nextUniqueAgentLabel,
} from "../utils/agents";
import AdaptiveModal from "./ui/AdaptiveModal.vue";
import CharacterLibraryPopover from "./CharacterLibraryPopover.vue";
import ExperimentTagsPicker from "./ExperimentTagsPicker.vue";
import ToggleSwitch from "./ui/ToggleSwitch.vue";
import { triggerHaptic } from "../utils/native";
import {
  MECHANIC_DESCRIPTIONS,
  summarizeActiveMechanics,
  type MechanicKey,
} from "../constants/mechanicDescriptions";
import {
  applyTagLocksToMechanics,
  getLockedMechanics,
  toggleExperimentTag,
} from "../constants/experimentTags";
import type {
  CharacterCard,
  ExperimentConfig,
  GeneratedScenario,
  NodeConfig,
  PresetConfigOverrides,
  PresetTemplate,
} from "../types";

const props = defineProps<{
  open: boolean;
  presets: PresetTemplate[];
  currentConfig: ExperimentConfig | null;
  characterCards?: CharacterCard[];
}>();

const emit = defineEmits<{
  close: [];
  create: [title: string, config: ExperimentConfig];
  savePreset: [preset: PresetTemplate];
  deletePreset: [presetId: string];
}>();

const title = ref("");
const selectedPresetId = ref("free");
const inheritKeys = ref(true);

const customAgents = ref<NodeConfig[]>([]);
const customGlobalPrompt = ref("");
const customInitialTopic = ref("");
const customInitialBoard = ref("");
const enableNarrativeArc = ref(false);
const targetTotalTurns = ref(30);
const syncToPreset = ref(false);

const selectedTags = ref<string[]>([]);
const intuitiveFastReply = ref(false);
const allowPassTurn = ref(false);
const randomMoodDice = ref(false);
const enablePrivateNotes = ref(false);
const enableBoardTools = ref(true);
const enableSmartSelector = ref(true);
const hoveredMechanic = ref<MechanicKey | null>(null);
const supportsHover =
  typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia("(hover: hover)").matches
    : true;

const lockedMechanics = computed(() => getLockedMechanics(selectedTags.value));

function currentMechanicsSnapshot(): Record<MechanicKey, boolean> {
  return {
    intuitiveFastReply: intuitiveFastReply.value,
    allowPassTurn: allowPassTurn.value,
    randomMoodDice: randomMoodDice.value,
    enablePrivateNotes: enablePrivateNotes.value,
    enableBoardTools: enableBoardTools.value,
    enableSmartSelector: enableSmartSelector.value,
  };
}

function applyMechanicsSnapshot(next: Record<MechanicKey, boolean>) {
  intuitiveFastReply.value = next.intuitiveFastReply;
  allowPassTurn.value = next.allowPassTurn;
  randomMoodDice.value = next.randomMoodDice;
  enablePrivateNotes.value = next.enablePrivateNotes;
  enableBoardTools.value = next.enableBoardTools;
  enableSmartSelector.value = next.enableSmartSelector;
}

function handleToggleTag(tagId: string) {
  const res = toggleExperimentTag(selectedTags.value, tagId, currentMechanicsSnapshot());
  selectedTags.value = res.tags;
  applyMechanicsSnapshot(res.mechanics);
}

function handleClearTags() {
  selectedTags.value = [];
}

function toggleMechanicPill(key: MechanicKey, refVal: { value: boolean }) {
  if (lockedMechanics.value[key]) return;
  refVal.value = !refVal.value;
}

const engineMechanismPills = [
  { key: "intuitiveFastReply" as const, label: MECHANIC_DESCRIPTIONS.intuitiveFastReply.title, refVal: intuitiveFastReply },
  { key: "allowPassTurn" as const, label: MECHANIC_DESCRIPTIONS.allowPassTurn.title, refVal: allowPassTurn },
  { key: "randomMoodDice" as const, label: MECHANIC_DESCRIPTIONS.randomMoodDice.title, refVal: randomMoodDice },
  { key: "enablePrivateNotes" as const, label: MECHANIC_DESCRIPTIONS.enablePrivateNotes.title, refVal: enablePrivateNotes },
  { key: "enableBoardTools" as const, label: MECHANIC_DESCRIPTIONS.enableBoardTools.title, refVal: enableBoardTools },
  { key: "enableSmartSelector" as const, label: MECHANIC_DESCRIPTIONS.enableSmartSelector.title, refVal: enableSmartSelector },
] as const;

const mechanicGuidance = computed(() =>
  summarizeActiveMechanics(currentMechanicsSnapshot())
);

function applyRecommendedConfig(rec?: PresetConfigOverrides | null) {
  const next = applyTagLocksToMechanics(selectedTags.value, {
    intuitiveFastReply: rec?.intuitiveFastReply ?? false,
    allowPassTurn: rec?.allowPassTurn ?? false,
    randomMoodDice: rec?.randomMoodDice ?? false,
    enablePrivateNotes: rec?.enablePrivateNotes ?? false,
    enableBoardTools: rec?.enableWhiteboard ?? true,
    enableSmartSelector: rec?.smartTurnOrder ?? true,
  });
  applyMechanicsSnapshot(next);
}

function buildRecommendedConfig(): PresetConfigOverrides {
  return {
    allowPassTurn: allowPassTurn.value,
    randomMoodDice: randomMoodDice.value,
    intuitiveFastReply: intuitiveFastReply.value,
    enableWhiteboard: enableBoardTools.value,
    enablePrivateNotes: enablePrivateNotes.value,
    smartTurnOrder: enableSmartSelector.value,
  };
}

const ideaInput = ref("");
const generatingScenario = ref(false);
const ideaError = ref<string | null>(null);

function makeDefaultAgents(): NodeConfig[] {
  const first = props.currentConfig?.agents[0];
  return [
    {
      id: "AiA",
      label: "智能体 1",
      baseUrl: first?.baseUrl || "https://api.deepseek.com/v1",
      apiKey: inheritKeys.value ? first?.apiKey || "" : "",
      modelName: first?.modelName || "deepseek-chat",
      temperature: 0.9,
      systemPrompt: "",
    },
    {
      id: "AiB",
      label: "智能体 2",
      baseUrl: first?.baseUrl || "https://api.deepseek.com/v1",
      apiKey: inheritKeys.value ? first?.apiKey || "" : "",
      modelName: first?.modelName || "deepseek-chat",
      temperature: 0.9,
      systemPrompt: "",
    },
  ];
}

function getPresetAgents(p: PresetTemplate) {
  if (Array.isArray(p.agentPrompts) && p.agentPrompts.length > 0) {
    return p.agentPrompts;
  }
  return [
    { label: p.nodeALabel || "智能体 1", systemPrompt: p.nodeAPrompt || "" },
    { label: p.nodeBLabel || "智能体 2", systemPrompt: p.nodeBPrompt || "" },
  ];
}

function hydrateFromPreset(p?: PresetTemplate) {
  const baseAgents: NodeConfig[] = props.currentConfig
    ? JSON.parse(JSON.stringify(props.currentConfig.agents))
    : makeDefaultAgents();

  if (!p) {
    selectedTags.value = [];
    customAgents.value = baseAgents;
    customGlobalPrompt.value = "";
    customInitialTopic.value = "";
    applyRecommendedConfig(null);
    return;
  }

  selectedTags.value = [...(p.tags ?? [])];
  customGlobalPrompt.value = p.globalSystemPrompt ?? "";
  customInitialTopic.value = p.initialTopic ?? "";
  applyRecommendedConfig(p.recommendedConfig);

  if (p.id === "custom") {
    customAgents.value = baseAgents;
    return;
  }

  const pAgents = getPresetAgents(p);
  const first = baseAgents[0];
  while (baseAgents.length < pAgents.length) {
    const n = baseAgents.length + 1;
    baseAgents.push({
      id: `ai-${Date.now().toString(36)}-${n}`,
      label: `智能体 ${n}`,
      baseUrl: first?.baseUrl || "https://api.deepseek.com/v1",
      apiKey: inheritKeys.value ? first?.apiKey || "" : "",
      modelName: first?.modelName || "deepseek-chat",
      temperature: 0.9,
      systemPrompt: "",
    });
  }
  const sized = p.id === "free" ? baseAgents : baseAgents.slice(0, pAgents.length);
  pAgents.forEach((ap, idx) => {
    if (sized[idx]) {
      sized[idx].label = ap.label;
      sized[idx].systemPrompt = ap.systemPrompt;
    }
  });
  customAgents.value = sized;
}

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) {
      const first = props.presets[0];
      selectedPresetId.value = first?.id ?? "free";
      title.value = `${first?.name.replace(/（.*?）/g, "") ?? "自由漫谈"} #${Math.floor(10 + Math.random() * 90)}`;
      syncToPreset.value = false;
      customInitialBoard.value = "";
      enableNarrativeArc.value = props.currentConfig?.enableNarrativeArc ?? false;
      targetTotalTurns.value = props.currentConfig?.targetTotalTurns ?? 30;
      ideaInput.value = "";
      ideaError.value = null;
      hydrateFromPreset(first);
    }
  }
);

async function generateFromIdea() {
  const idea = ideaInput.value.trim();
  if (!idea || generatingScenario.value) return;
  generatingScenario.value = true;
  ideaError.value = null;
  try {
    const res = await invoke<GeneratedScenario>("generate_scenario_from_idea", {
      idea,
      baseConfig: props.currentConfig,
    });
    selectedPresetId.value = "custom";
    selectedTags.value = [];
    title.value = res.title;
    customGlobalPrompt.value = res.globalSystemPrompt;
    customInitialTopic.value = res.initialTopic;
    customInitialBoard.value = res.initialBoard;
    applyRecommendedConfig({
      allowPassTurn: false,
      randomMoodDice: false,
      intuitiveFastReply: false,
      enableWhiteboard: true,
      enablePrivateNotes: true,
      smartTurnOrder: true,
    });
    const baseFirst = props.currentConfig?.agents[0];
    customAgents.value = res.agents.map((ag, idx) => {
      const existing = props.currentConfig?.agents[idx];
      return {
        id: existing?.id || `ai-${Date.now().toString(36)}-${idx + 1}`,
        label: ag.label,
        baseUrl: existing?.baseUrl || baseFirst?.baseUrl || "https://api.deepseek.com/v1",
        apiKey: inheritKeys.value ? (existing?.apiKey || baseFirst?.apiKey || "") : "",
        modelName: existing?.modelName || baseFirst?.modelName || "deepseek-chat",
        temperature: existing?.temperature ?? 0.9,
        systemPrompt: ag.systemPrompt,
      };
    });
  } catch (err: any) {
    ideaError.value = err?.toString?.() ?? "生成剧本失败";
  } finally {
    generatingScenario.value = false;
  }
}

function formatPresetCast(p: PresetTemplate): string {
  if (p.id === "free") return "零提示词 · 无目标";
  if (p.id === "custom") return "支持多个 AI 自由设定";
  return getPresetAgents(p)
    .map((a) => a.label)
    .join(" vs ");
}

const inheritCardMemory = ref(true);

function applyCardToAgent(idx: number, card: CharacterCard) {
  const target = customAgents.value[idx];
  if (!target) return;
  target.label = disambiguateAgentLabel(card.name, customAgents.value, idx);
  const parts = [card.basePrompt.trim()];
  if (inheritCardMemory.value) {
    if (card.behaviorTraits.trim()) {
      parts.push(`【跨世界博弈特质】\n${card.behaviorTraits.trim()}`);
    }
    if (card.memoryImprints && card.memoryImprints.length > 0) {
      parts.push(
        `【跨宇宙记忆烙印】\n${card.memoryImprints.map((m) => `- ${m}`).join("\n")}`
      );
    }
  }
  target.systemPrompt = parts.filter(Boolean).join("\n\n");
}

function selectPreset(p: PresetTemplate) {
  selectedPresetId.value = p.id;
  const cleanName = p.name.replace(/（.*?）/g, "");
  title.value = `${cleanName} #${Math.floor(10 + Math.random() * 90)}`;
  hydrateFromPreset(p);
}

const duplicateLabels = computed(() => duplicateLabelSet(customAgents.value));

function addModalAgent() {
  const first = customAgents.value[0];
  const label = nextUniqueAgentLabel(customAgents.value);
  customAgents.value.push({
    id: `ai-${Date.now().toString(36)}-${customAgents.value.length + 1}`,
    label,
    baseUrl: first?.baseUrl || "https://api.deepseek.com/v1",
    apiKey: first?.apiKey || "",
    modelName: first?.modelName || "deepseek-chat",
    temperature: 0.9,
    systemPrompt: "",
  });
}

function removeModalAgent(idx: number) {
  if (customAgents.value.length <= 2) return;
  customAgents.value.splice(idx, 1);
}

function handleDeletePresetItem(p: PresetTemplate) {
  if (p.isBuiltin) return;
  emit("deletePreset", p.id);
  if (selectedPresetId.value === p.id) {
    const next = props.presets.find((item) => item.id !== p.id) ?? props.presets[0];
    if (next) {
      selectPreset(next);
    }
  }
}

function overwriteSelectedPreset() {
  const preset = props.presets.find((p) => p.id === selectedPresetId.value);
  if (!preset || preset.id === "free" || preset.id === "custom") return;
  const dup = findDuplicateAgentLabel(customAgents.value);
  if (dup) {
    ideaError.value = `角色名称「${dup}」存在重复，请修改后再覆盖预设`;
    return;
  }
  const agents = customAgents.value.map((a, idx) => ({
    label: effectiveAgentLabel(a, idx),
    systemPrompt: a.systemPrompt,
  }));
  const updatedPreset: PresetTemplate = {
    ...preset,
    globalSystemPrompt: customGlobalPrompt.value,
    initialTopic: customInitialTopic.value.trim(),
    tags: [...selectedTags.value],
    agentPrompts: agents,
    recommendedConfig: buildRecommendedConfig(),
    nodeALabel: agents[0]?.label || "智能体 1",
    nodeAPrompt: agents[0]?.systemPrompt || "",
    nodeBLabel: agents[1]?.label || "智能体 2",
    nodeBPrompt: agents[1]?.systemPrompt || "",
  };
  emit("savePreset", updatedPreset);
}

function handleCreate() {
  const dup = findDuplicateAgentLabel(customAgents.value);
  if (dup) {
    ideaError.value = `角色名称「${dup}」存在重复，请修改名称以区分不同智能体`;
    return;
  }
  const preset =
    props.presets.find((p) => p.id === selectedPresetId.value) ?? props.presets[0];
  const base: ExperimentConfig = props.currentConfig
    ? JSON.parse(JSON.stringify(props.currentConfig))
    : {
        presetId: preset?.id ?? selectedPresetId.value,
        globalSystemPrompt: "",
        initialTopic: "",
        observerInterval: 10,
        pauseSeconds: 2.5,
        maxConsecutiveTurns: 100,
        tokenBudget: 0,
        contextWindowTokens: 32000,
        compactionThresholdPct: 80,
        boardApprovalMode: "auto",
        enableBoardTools: true,
        initialBoard: "",
        enablePrefixCacheOpt: true,
        enableCompactionGuard: true,
        enablePrivateNotes: false,
        enableSmartSelector: true,
        enableAdaptiveReasoning: false,
        adaptiveLowReasoningEffort: "low",
        enablePeriodicReflection: false,
        enableNarrativeArc: enableNarrativeArc.value,
        targetTotalTurns: Math.max(5, Math.round(targetTotalTurns.value || 30)),
        agents: makeDefaultAgents(),
        nodeC: {
          id: "observer",
          label: "观察员 C",
          baseUrl: "https://api.deepseek.com/v1",
          apiKey: "",
          modelName: "deepseek-chat",
          temperature: 0.3,
          systemPrompt:
            "你是一个冷静、犀利的第三人称对话观察员，负责对多智能体对话进行阶段性诊断与深度评估。",
        },
      };

  base.presetId = preset?.id ?? selectedPresetId.value;
  base.tags = [...selectedTags.value];
  base.globalSystemPrompt = customGlobalPrompt.value;
  base.initialTopic = customInitialTopic.value.trim();
  base.initialBoard = customInitialBoard.value.trim();
  base.enableNarrativeArc = enableNarrativeArc.value;
  base.targetTotalTurns = Math.max(5, Math.round(targetTotalTurns.value || 30));
  base.intuitiveFastReply = intuitiveFastReply.value;
  base.allowPassTurn = allowPassTurn.value;
  base.randomMoodDice = randomMoodDice.value;
  base.enablePrivateNotes = enablePrivateNotes.value;
  base.enableBoardTools = enableBoardTools.value;
  base.enableSmartSelector = enableSmartSelector.value;
  base.agents = customAgents.value.map((a, idx) => ({
    ...a,
    label: a.label.trim() || `智能体 ${idx + 1}`,
  }));

  if (syncToPreset.value && preset && preset.id !== "free" && preset.id !== "custom") {
    const updatedPreset: PresetTemplate = {
      ...preset,
      globalSystemPrompt: customGlobalPrompt.value,
      initialTopic: customInitialTopic.value.trim(),
      tags: [...selectedTags.value],
      agentPrompts: base.agents.map((a) => ({
        label: a.label,
        systemPrompt: a.systemPrompt,
      })),
      recommendedConfig: buildRecommendedConfig(),
      nodeALabel: base.agents[0]?.label || "智能体 1",
      nodeAPrompt: base.agents[0]?.systemPrompt || "",
      nodeBLabel: base.agents[1]?.label || "智能体 2",
      nodeBPrompt: base.agents[1]?.systemPrompt || "",
    };
    emit("savePreset", updatedPreset);
  }

  if (!inheritKeys.value) {
    base.agents.forEach((a) => {
      a.apiKey = "";
    });
    base.nodeC.apiKey = "";
  }

  triggerHaptic("confirm");
  emit("create", title.value.trim() || "未命名生态实验", base);
  emit("close");
}
</script>

<template>
  <AdaptiveModal
    :open="open"
    title="新建 AI 生态实验"
    description="选择预设剧本或自定义角色，也可用 AI 一句话捏剧"
    max-width-class="max-w-4xl"
    mobile-presentation="fullscreen"
    @close="emit('close')"
  >
    <template #header>
      <div class="flex items-center justify-between px-4 sm:px-6 py-3.5 border-b border-black/[0.07] bg-white shrink-0">
        <div class="min-w-0">
          <h2 class="text-[16px] md:text-base font-semibold text-[#1D1D1F] truncate">新建 AI 生态实验</h2>
          <p class="text-[11.5px] md:text-[11px] text-slate-400 mt-0.5 truncate">选择场景预设或自定义角色，也可一句话 AI 捏剧</p>
        </div>
        <button
          type="button"
          @click="emit('close')"
          class="w-9 h-9 md:w-8 md:h-8 flex items-center justify-center rounded-full bg-[#F2F2F7] md:bg-transparent hover:bg-slate-100 text-slate-500 shrink-0"
        >
          <X class="w-4 h-4" />
        </button>
      </div>
    </template>

        <!-- One-Sentence AI Scenario Generator Bar (Clean neutral surface matching Inset Grouped aesthetic) -->
        <div class="px-4 sm:px-6 py-3 border-b border-black/[0.06] bg-white shrink-0 space-y-1.5">
          <div class="flex items-center gap-2">
            <div class="flex-1 flex items-center gap-2 rounded-2xl bg-[#F2F2F7] focus-within:bg-white border border-transparent focus-within:border-black/15 px-3.5 py-2 md:py-1.5 transition">
              <Sparkles class="w-4 h-4 md:w-3.5 md:h-3.5 text-[#007AFF] shrink-0" />
              <input
                v-model="ideaInput"
                @keydown.enter.prevent="generateFromIdea"
                placeholder="一句话生成剧本（如：暴风雪山庄的三个时间旅行者）"
                class="flex-1 bg-transparent text-[13.5px] md:text-xs text-[#1D1D1F] placeholder:text-slate-400 outline-none min-w-0"
              />
            </div>
            <button
              type="button"
              @click="generateFromIdea"
              :disabled="!ideaInput.trim() || generatingScenario"
              class="shrink-0 h-10 md:h-auto flex items-center gap-1.5 px-4 md:px-3.5 py-1.5 rounded-full text-[13px] md:text-xs font-medium bg-[#1D1D1F] hover:bg-black text-white transition disabled:opacity-45"
            >
              <Loader2 v-if="generatingScenario" class="w-3.5 h-3.5 animate-spin" />
              <Sparkles v-else class="w-3.5 h-3.5" />
              <span>{{ generatingScenario ? "编剧中..." : "AI 捏剧" }}</span>
            </button>
          </div>
          <p v-if="ideaError" class="text-[11.5px] text-rose-600 px-1">{{ ideaError }}</p>
        </div>

        <!-- Unified Single-Scroll on Mobile (< 768px), Master-Detail 2-Col on Desktop (>= 768px) -->
        <div class="flex-1 min-h-0 flex flex-col md:flex-row overflow-y-auto md:overflow-hidden bg-[#F5F5F7]">
          <!-- Presets & Title (Horizontal Snap Carousel on Mobile, Left Sidebar on Desktop) -->
          <div class="w-full md:w-[295px] lg:w-[310px] shrink-0 md:border-r border-black/[0.06] bg-[#F5F5F7] px-4 pt-4 pb-2 md:p-4 space-y-4 md:overflow-y-auto">
            <div class="rounded-2xl bg-white p-3.5 border border-black/[0.05] shadow-[0_1px_2px_rgba(0,0,0,0.02)]">
              <label class="block text-[12px] font-medium text-slate-600 mb-1.5">实验标题</label>
              <input
                v-model="title"
                placeholder="输入实验标题"
                class="w-full rounded-xl border border-black/[0.08] bg-[#F5F5F7] focus:bg-white px-3.5 py-2 text-[14px] font-medium text-[#1D1D1F] placeholder:text-slate-400 outline-none focus:ring-2 focus:ring-[#007AFF]/25 transition"
              />
            </div>

            <div class="space-y-2">
              <div class="flex items-center justify-between px-0.5">
                <label class="text-[12px] font-semibold text-slate-600">场景与性格预设 ({{ presets.length }})</label>
                <span class="md:hidden text-[11px] text-slate-400">左右滑动选择</span>
              </div>
              <div class="flex md:flex-col gap-2.5 md:gap-2 overflow-x-auto md:overflow-x-visible snap-x snap-mandatory pb-1 md:pb-0 no-scrollbar">
                <div
                  v-for="p in presets"
                  :key="p.id"
                  @click="selectPreset(p)"
                  class="group cursor-pointer w-[248px] md:w-full shrink-0 snap-start text-left p-3.5 rounded-2xl border transition-all"
                  :class="
                    selectedPresetId === p.id
                      ? 'border-[#007AFF]/70 bg-white ring-2 ring-[#007AFF]/12 shadow-[0_2px_8px_rgba(0,122,255,0.08)]'
                      : 'border-black/[0.05] hover:border-black/15 bg-white shadow-[0_1px_2px_rgba(0,0,0,0.02)]'
                  "
                >
                  <div class="flex items-center justify-between gap-1.5">
                    <span class="text-[13.5px] font-semibold text-[#1D1D1F] truncate">{{ p.name }}</span>
                    <button
                      v-if="!p.isBuiltin"
                      type="button"
                      @click.stop="handleDeletePresetItem(p)"
                      class="p-1.5 md:p-1 rounded-lg text-slate-400 hover:text-rose-600 hover:bg-rose-50 transition shrink-0"
                      title="删除此自定义预设"
                    >
                      <Trash2 class="w-3.5 h-3.5" />
                    </button>
                  </div>
                  <div class="text-[11.5px] text-[#007AFF]/90 font-medium mt-1 truncate">
                    {{ formatPresetCast(p) }}
                  </div>
                  <p class="text-[11.5px] text-slate-500 mt-1 line-clamp-2 leading-relaxed">
                    {{ p.description }}
                  </p>
                </div>
              </div>
            </div>
          </div>

          <!-- Prompt & Agent Editor (Crisp White Cards on Soft Gray Canvas) -->
          <div class="flex-1 min-w-0 md:overflow-y-auto px-4 pb-6 pt-2 md:p-5 space-y-4 bg-[#F5F5F7]">
            <div class="flex flex-wrap items-center justify-between gap-2 px-0.5">
              <span class="text-[13.5px] font-semibold text-[#1D1D1F]">
                剧本设定与角色底牌（{{ customAgents.length }} 位）
              </span>
              <div class="flex items-center gap-2.5">
                <button
                  v-if="selectedPresetId !== 'free' && selectedPresetId !== 'custom'"
                  type="button"
                  @click="overwriteSelectedPreset"
                  class="flex items-center gap-1 text-[12px] font-medium text-slate-600 hover:text-[#007AFF] transition"
                  title="将右侧修改直接覆盖保存回当前选中的预设模板"
                >
                  <Save class="w-3.5 h-3.5" />
                  <span>覆盖保存</span>
                </button>
                <button
                  type="button"
                  @click="addModalAgent"
                  class="flex items-center gap-1 px-3 py-1.5 rounded-full bg-white border border-black/[0.08] text-[12px] font-medium text-[#1D1D1F] hover:bg-black/[0.04] transition"
                >
                  <Plus class="w-3.5 h-3.5" />
                  <span>添加角色</span>
                </button>
              </div>
            </div>

            <div class="rounded-2xl bg-white p-4 border border-black/[0.05] shadow-[0_1px_2px_rgba(0,0,0,0.02)]">
              <div class="flex items-center justify-between gap-2 mb-2">
                <label class="text-[13px] font-semibold text-[#1D1D1F]">
                  本局场景与文风规则
                </label>
                <span class="text-[11.5px] text-slate-400">对本局所有角色生效</span>
              </div>
              <textarea
                v-model="customGlobalPrompt"
                rows="3"
                placeholder="例如：场景背景、文风规范、单次发言字数限制等（可留空）"
                class="w-full resize-none rounded-xl border border-black/[0.08] bg-[#F5F5F7] focus:bg-white px-3.5 py-2.5 text-[13.5px] text-[#1D1D1F] placeholder:text-slate-400 leading-relaxed outline-none focus:border-black/20 transition"
              ></textarea>
            </div>

            <div class="space-y-3">
              <div
                v-for="(ag, idx) in customAgents"
                :key="ag.id"
                class="p-4 rounded-2xl bg-white border border-black/[0.05] shadow-[0_1px_2px_rgba(0,0,0,0.02)] space-y-2.5"
              >
                <div class="flex items-center justify-between gap-2">
                  <div class="flex items-center gap-2 flex-1 min-w-0">
                    <span class="px-2 py-0.5 rounded-md bg-[#F2F2F7] text-[11px] font-mono font-semibold text-slate-500 shrink-0">#{{ idx + 1 }}</span>
                    <input
                      v-model="ag.label"
                      placeholder="角色名称"
                      class="flex-1 min-w-0 rounded-xl border bg-[#F5F5F7] focus:bg-white px-3 py-2 text-[13.5px] font-semibold text-[#1D1D1F] placeholder:text-slate-400 outline-none transition"
                      :class="
                        duplicateLabels.has(effectiveAgentLabel(ag, idx))
                          ? 'border-rose-400 ring-2 ring-rose-200/70'
                          : 'border-black/[0.08] focus:ring-2 focus:ring-[#007AFF]/25'
                      "
                    />
                  </div>
                  <div v-if="characterCards && characterCards.length > 0" class="shrink-0">
                    <CharacterLibraryPopover
                      :cards="characterCards"
                      :inherit-memory="inheritCardMemory"
                      :show-count="false"
                      trigger-label="从灵魂库替换"
                      @update:inherit-memory="inheritCardMemory = $event"
                      @select="applyCardToAgent(idx, $event)"
                    />
                  </div>
                  <button
                    v-if="customAgents.length > 2"
                    type="button"
                    @click="removeModalAgent(idx)"
                    class="w-9 h-9 flex items-center justify-center rounded-xl text-slate-400 hover:text-rose-600 hover:bg-rose-50 transition shrink-0"
                  >
                    <Trash2 class="w-4 h-4" />
                  </button>
                </div>
                <p
                  v-if="duplicateLabels.has(effectiveAgentLabel(ag, idx))"
                  class="text-[11.5px] text-rose-600 font-medium"
                >
                  角色名称不能重复，请修改名称以区分不同智能体
                </p>
                <div>
                  <label class="block text-[11.5px] font-medium text-slate-500 mb-1.5">
                    角色身份与私密底牌
                  </label>
                  <textarea
                    v-model="ag.systemPrompt"
                    rows="3"
                    placeholder="设定该角色的身份背景、性格底色与隐藏动机（可留空）"
                    class="w-full resize-none rounded-xl border border-black/[0.08] bg-[#F5F5F7] focus:bg-white px-3.5 py-2.5 text-[13.5px] text-[#1D1D1F] placeholder:text-slate-400 leading-relaxed outline-none focus:border-black/20 transition"
                  ></textarea>
                </div>
              </div>
            </div>

            <div class="rounded-2xl bg-white p-4 border border-black/[0.05] shadow-[0_1px_2px_rgba(0,0,0,0.02)] space-y-4">
              <ExperimentTagsPicker
                :tags="selectedTags"
                @toggle="handleToggleTag"
                @clear="handleClearTags"
              />

              <div class="pt-3 border-t border-black/[0.06] space-y-2.5">
                <div class="flex items-center justify-between gap-2">
                  <label class="text-[13px] font-semibold text-[#1D1D1F]">
                    本局联动引擎机制（点击可微调）
                  </label>
                  <span class="text-[11px] text-slate-400">随剧本与标签自动联动</span>
                </div>
                <div class="flex flex-wrap gap-2">
                  <div
                    v-for="item in engineMechanismPills"
                    :key="item.key"
                    class="relative"
                    @mouseenter="supportsHover && (hoveredMechanic = item.key)"
                    @mouseleave="hoveredMechanic === item.key && (hoveredMechanic = null)"
                  >
                    <button
                      type="button"
                      @click="toggleMechanicPill(item.key, item.refVal)"
                      @focus="hoveredMechanic = item.key"
                      @blur="hoveredMechanic === item.key && (hoveredMechanic = null)"
                      :title="
                        lockedMechanics[item.key]
                          ? `已由标签 [${lockedMechanics[item.key]}] 自动锁定，取消该标签即可手动调节`
                          : MECHANIC_DESCRIPTIONS[item.key].title
                      "
                      class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-[12px] font-medium border transition select-none"
                      :class="
                        lockedMechanics[item.key]
                          ? 'bg-[#F2F2F5] border-black/[0.06] text-slate-400 opacity-60 cursor-not-allowed'
                          : item.refVal.value
                            ? 'bg-blue-50 border-[#007AFF]/40 text-[#007AFF] shadow-2xs cursor-pointer'
                            : 'bg-[#F7F7FA] border-black/[0.07] text-slate-500 hover:border-black/20 hover:text-slate-700 cursor-pointer'
                      "
                    >
                      <Lock v-if="lockedMechanics[item.key]" class="w-3 h-3 text-slate-400 shrink-0" />
                      <span
                        v-else
                        class="w-1.5 h-1.5 rounded-full transition-colors"
                        :class="item.refVal.value ? 'bg-[#007AFF]' : 'bg-slate-300'"
                      ></span>
                      <span>{{ item.label }}</span>
                    </button>

                    <!-- Apple-style Floating Dual-State Tooltip (Desktop Hover) -->
                    <div
                      v-if="supportsHover && hoveredMechanic === item.key"
                      class="pointer-events-none absolute left-0 bottom-full mb-2 z-30 w-72 rounded-xl bg-white/95 backdrop-blur-xl border border-black/[0.08] shadow-[0_12px_32px_rgba(0,0,0,0.16)] p-2.5 text-left space-y-1.5"
                    >
                      <div class="flex items-center justify-between gap-2 text-[11.5px] font-semibold text-[#1D1D1F]">
                        <span>{{ MECHANIC_DESCRIPTIONS[item.key].title }}</span>
                        <span
                          class="px-1.5 py-0.5 rounded text-[10px]"
                          :class="
                            lockedMechanics[item.key]
                              ? 'bg-amber-50 text-amber-700'
                              : item.refVal.value
                                ? 'bg-blue-50 text-[#007AFF]'
                                : 'bg-[#F2F2F5] text-slate-500'
                          "
                        >
                          {{ lockedMechanics[item.key] ? "标签锁定关闭" : `当前：${item.refVal.value ? "已开启" : "已关闭"}` }}
                        </span>
                      </div>
                      <div
                        v-if="lockedMechanics[item.key]"
                        class="text-[11px] leading-relaxed text-amber-700 font-medium"
                      >
                        已由标签 [{{ lockedMechanics[item.key] }}] 自动锁定，取消该标签即可手动调节。
                      </div>
                      <template v-else>
                        <div class="text-[11px] leading-relaxed text-slate-600">
                          <span class="font-medium text-emerald-700">✅ 开启：</span>{{ MECHANIC_DESCRIPTIONS[item.key].onEffect }}
                        </div>
                        <div class="text-[11px] leading-relaxed text-slate-500">
                          <span class="font-medium text-slate-600">⬜ 关闭：</span>{{ MECHANIC_DESCRIPTIONS[item.key].offEffect }}
                        </div>
                      </template>
                    </div>
                  </div>
                </div>

                <!-- Live Dual-State & Scenario Advice Line -->
                <p
                  class="text-[11px] leading-relaxed transition-colors"
                  :class="
                    hoveredMechanic && lockedMechanics[hoveredMechanic]
                      ? 'text-amber-700'
                      : hoveredMechanic
                        ? 'text-slate-500'
                        : mechanicGuidance.isCaution
                          ? 'text-amber-700'
                          : 'text-slate-400'
                  "
                >
                  <template v-if="hoveredMechanic && lockedMechanics[hoveredMechanic]">
                    已由标签 [{{ lockedMechanics[hoveredMechanic] }}] 自动锁定，取消该标签即可手动调节
                  </template>
                  <template v-else-if="hoveredMechanic">
                    <span class="font-semibold text-slate-600">{{ MECHANIC_DESCRIPTIONS[hoveredMechanic].title }}</span>
                    <span class="mx-1">·</span>
                    <span>✅ 开：{{ MECHANIC_DESCRIPTIONS[hoveredMechanic].onEffect }} ｜ ⬜ 关：{{ MECHANIC_DESCRIPTIONS[hoveredMechanic].offEffect }}</span>
                  </template>
                  <template v-else>
                    {{ mechanicGuidance.text }}
                  </template>
                </p>
              </div>
            </div>

            <div class="rounded-2xl bg-white p-4 border border-black/[0.05] shadow-[0_1px_2px_rgba(0,0,0,0.02)]">
              <label class="block text-[13px] font-semibold text-[#1D1D1F] mb-2">
                第 1 轮开场引子（可选，留空则无引子直接开聊）
              </label>
              <textarea
                v-model="customInitialTopic"
                rows="2"
                placeholder="留空则不设开场引子"
                class="w-full resize-none rounded-xl border border-black/[0.08] bg-[#F7F7FA] focus:bg-white px-3.5 py-2 text-[13.5px] leading-relaxed outline-none focus:border-black/20 transition"
              ></textarea>
            </div>

            <div v-if="customInitialBoard.trim().length > 0" class="rounded-2xl bg-white p-4 border border-black/[0.05] shadow-[0_1px_2px_rgba(0,0,0,0.02)]">
              <label class="block text-[13px] font-semibold text-[#1D1D1F] mb-2">
                初始共享白板线索（全员开局可见）
              </label>
              <textarea
                v-model="customInitialBoard"
                rows="3"
                class="w-full resize-none rounded-xl border border-black/[0.08] bg-[#F7F7FA] focus:bg-white px-3.5 py-2 text-[13px] font-mono leading-relaxed outline-none focus:border-black/20 transition"
              ></textarea>
            </div>

            <div class="rounded-2xl bg-white border border-black/[0.05] shadow-[0_1px_2px_rgba(0,0,0,0.02)] divide-y divide-black/[0.05]">
              <div class="p-4 flex flex-wrap items-center justify-between gap-2">
                <span class="text-[13px] font-medium text-[#1D1D1F]">三幕式节奏与自然结局</span>
                <div class="flex items-center gap-3">
                  <div v-if="enableNarrativeArc" class="flex items-center gap-1.5 text-[12px] text-slate-600">
                    <span>目标</span>
                    <input
                      v-model.number="targetTotalTurns"
                      type="number"
                      min="5"
                      max="500"
                      class="w-16 rounded-lg border border-black/[0.08] bg-[#F7F7FA] px-2 py-1 text-[13px] font-mono outline-none focus:ring-2 focus:ring-[#007AFF]/25"
                    />
                    <span>轮</span>
                  </div>
                  <ToggleSwitch v-model="enableNarrativeArc" />
                </div>
              </div>

              <div
                v-if="selectedPresetId !== 'free' && selectedPresetId !== 'custom'"
                class="p-4 flex items-center justify-between gap-3"
              >
                <span class="text-[12.5px] text-slate-600">创建实验时，同时覆盖保存回所选预设模板</span>
                <ToggleSwitch v-model="syncToPreset" />
              </div>

              <div class="p-4 flex items-center justify-between gap-3">
                <span class="text-[12.5px] text-slate-600">复用当前实验的 API Base URL、Key 与模型配置</span>
                <ToggleSwitch v-model="inheritKeys" />
              </div>
            </div>
          </div>
        </div>

    <template #footer>
      <div class="grid grid-cols-[1fr_2fr] min-[520px]:flex min-[520px]:items-center min-[520px]:justify-end gap-2.5 px-4 sm:px-6 py-3 border-t border-black/[0.06] bg-white shrink-0">
        <button
          type="button"
          @click="emit('close')"
          class="h-10 md:h-9 px-4 rounded-xl text-[13.5px] md:text-[13px] font-medium text-slate-600 bg-[#F2F2F7] md:bg-transparent hover:bg-slate-100 transition"
        >
          取消
        </button>
        <button
          type="button"
          @click="handleCreate"
          class="h-10 md:h-9 px-6 md:px-5 rounded-xl text-[13.5px] md:text-[13px] font-semibold bg-[#1D1D1F] hover:bg-black text-white shadow-[0_1px_3px_rgba(0,0,0,0.12)] transition"
        >
          创建并进入观察箱
        </button>
      </div>
    </template>
  </AdaptiveModal>
</template>
