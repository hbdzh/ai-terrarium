<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  X,
  Copy,
  Sparkles,
  Plus,
  Trash2,
  Download,
  Upload,
  RotateCcw,
  Save,
  ChevronDown,
  ChevronRight,
  FileText,
  Cpu,
  Telescope,
  SlidersHorizontal,
  BookmarkPlus,
  Sun,
  Moon,
  Monitor,
} from "lucide-vue-next";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog, save } from "@tauri-apps/plugin-dialog";
import type { CharacterCard, ExperimentConfig, NodeConfig, PresetTemplate, ThemePreference } from "../types";
import { triggerHaptic } from "../utils/native";
import { themePreference, setThemePreference } from "../utils/theme";
import { MECHANIC_DESCRIPTIONS } from "../constants/mechanicDescriptions";
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
import RangeSlider from "./ui/RangeSlider.vue";
import ToggleSwitch from "./ui/ToggleSwitch.vue";
import {
  applyTagLocksToMechanics,
  getLockedMechanics,
  toggleExperimentTag,
} from "../constants/experimentTags";
import type { MechanicKey } from "../constants/mechanicDescriptions";

const props = defineProps<{
  open: boolean;
  config: ExperimentConfig | null;
  presets: PresetTemplate[];
  characterCards?: CharacterCard[];
}>();

const emit = defineEmits<{
  close: [];
  save: [config: ExperimentConfig];
  savePreset: [preset: PresetTemplate];
  resetPreset: [presetId: string];
  deletePreset: [presetId: string];
  deleteCharacterCard: [cardId: string];
  saveCharacterCard: [payload: { agentId: string; label: string; systemPrompt: string }];
  notify: [message: string];
}>();

const draft = ref<ExperimentConfig | null>(null);
const initialSnapshot = ref("");
const activeTab = ref<"script" | "agents" | "observer" | "safety">("script");
const selectedAgentIdx = ref(0);
const inheritCardMemory = ref(true);
const saveAsPresetOpen = ref(false);
const newPresetName = ref("");
const corePromptOpen = ref(false);
const corePromptDraft = ref("");
const corePromptInitial = ref("");
const corePromptSaving = ref(false);
const confirmDiscardOpen = ref(false);

async function loadCoreEnginePrompt() {
  try {
    const val = await invoke<string>("get_core_engine_prompt");
    corePromptDraft.value = val;
    corePromptInitial.value = val;
  } catch {
    // ignore load failure
  }
}

async function saveCoreEnginePrompt() {
  if (corePromptSaving.value) return;
  corePromptSaving.value = true;
  try {
    const saved = await invoke<string>("save_core_engine_prompt", {
      prompt: corePromptDraft.value,
    });
    corePromptDraft.value = saved;
    corePromptInitial.value = saved;
    emit("notify", "系统底层演戏总纲已保存（跨所有实验生效）");
  } catch (err: any) {
    emit("notify", `保存总纲失败：${err?.toString?.() ?? err}`);
  } finally {
    corePromptSaving.value = false;
  }
}

async function resetCoreEnginePrompt() {
  if (corePromptSaving.value) return;
  corePromptSaving.value = true;
  try {
    const reset = await invoke<string>("reset_core_engine_prompt");
    corePromptDraft.value = reset;
    corePromptInitial.value = reset;
    emit("notify", "已恢复系统底层演戏总纲默认设置");
  } catch (err: any) {
    emit("notify", `恢复默认失败：${err?.toString?.() ?? err}`);
  } finally {
    corePromptSaving.value = false;
  }
}

const providerQuickUrls = [
  { name: "DeepSeek", url: "https://api.deepseek.com/v1", model: "deepseek-chat" },
  {
    name: "阿里云百炼 (Qwen)",
    url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    model: "qwen-plus",
  },
  {
    name: "月之暗面 (Kimi)",
    url: "https://api.moonshot.cn/v1",
    model: "moonshot-v1-8k",
  },
  {
    name: "火山引擎 (豆包)",
    url: "https://ark.cn-beijing.volces.com/api/v3",
    model: "doubao-pro-32k",
  },
  {
    name: "OpenAI",
    url: "https://api.openai.com/v1",
    model: "gpt-4o-mini",
  },
];

const thinkingLevelOptions = [
  { value: "default", label: "模型默认" },
  { value: "off", label: "关闭" },
  { value: "low", label: "低" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" },
] as const;

function normalizeThinkingLevel(raw: unknown): string {
  const s = typeof raw === "string" ? raw.trim().toLowerCase() : "";
  return s === "off" || s === "low" || s === "medium" || s === "high" || s === "default"
    ? s
    : "default";
}

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen && props.config) {
      const cloned: ExperimentConfig = JSON.parse(JSON.stringify(props.config));
      cloned.globalSystemPrompt = cloned.globalSystemPrompt ?? "";
      cloned.tokenBudget = cloned.tokenBudget ?? 0;
      cloned.contextWindowTokens = cloned.contextWindowTokens ?? 32000;
      cloned.compactionThresholdPct = cloned.compactionThresholdPct ?? 80;
      cloned.boardApprovalMode = cloned.boardApprovalMode ?? "auto";
      cloned.enableBoardTools = cloned.enableBoardTools ?? true;
      cloned.initialBoard = cloned.initialBoard ?? "";
      cloned.enablePrefixCacheOpt = cloned.enablePrefixCacheOpt ?? true;
      cloned.enableCompactionGuard = cloned.enableCompactionGuard ?? true;
      cloned.enablePrivateNotes = cloned.enablePrivateNotes ?? false;
      cloned.enableSmartSelector = cloned.enableSmartSelector ?? true;
      cloned.enableAdaptiveReasoning = cloned.enableAdaptiveReasoning ?? false;
      cloned.adaptiveLowReasoningEffort = cloned.adaptiveLowReasoningEffort ?? "low";
      cloned.enablePeriodicReflection = cloned.enablePeriodicReflection ?? false;
      cloned.enableAutoDirector = cloned.enableAutoDirector ?? false;
      cloned.enableNarrativeArc = cloned.enableNarrativeArc ?? false;
      cloned.tags = Array.isArray(cloned.tags) ? [...cloned.tags] : [];
      cloned.targetTotalTurns = cloned.targetTotalTurns ?? 30;
      cloned.allowPassTurn = cloned.allowPassTurn ?? false;
      cloned.randomMoodDice = cloned.randomMoodDice ?? false;
      cloned.intuitiveFastReply = cloned.intuitiveFastReply ?? false;
      cloned.agents = (cloned.agents ?? []).map((a) => ({
        ...a,
        thinkingLevel: normalizeThinkingLevel(a.thinkingLevel),
      }));
      if (cloned.nodeC) {
        cloned.nodeC.thinkingLevel = normalizeThinkingLevel(cloned.nodeC.thinkingLevel);
      }
      draft.value = cloned;
      initialSnapshot.value = JSON.stringify(cloned);
      selectedAgentIdx.value = 0;
      saveAsPresetOpen.value = false;
      newPresetName.value = "";
      confirmDiscardOpen.value = false;
      loadCoreEnginePrompt();
    }
  },
  { immediate: true }
);

const duplicateLabels = computed(() =>
  draft.value ? duplicateLabelSet(draft.value.agents) : new Set<string>()
);

const isDirty = computed(() => {
  const draftChanged =
    Boolean(draft.value) && JSON.stringify(draft.value) !== initialSnapshot.value;
  const coreChanged = corePromptDraft.value !== corePromptInitial.value;
  return draftChanged || coreChanged;
});

const currentAgent = computed<NodeConfig | null>(() => {
  if (!draft.value || draft.value.agents.length === 0) return null;
  const idx = Math.min(selectedAgentIdx.value, draft.value.agents.length - 1);
  return draft.value.agents[idx];
});

function openSaveAsPreset() {
  newPresetName.value = "";
  saveAsPresetOpen.value = true;
}

function confirmSaveAsPreset() {
  if (!draft.value) return;
  const currentAgents = draft.value.agents;
  const preset: PresetTemplate = {
    id: "",
    name: newPresetName.value.trim() || "新建自定义场景",
    description: "由当前实验设定保存的场景预设模板",
    globalSystemPrompt: draft.value.globalSystemPrompt ?? "",
    initialTopic: draft.value.initialTopic ?? "",
    tags: [...(draft.value.tags ?? [])],
    agentPrompts: currentAgents.map((a) => ({
      label: a.label,
      systemPrompt: a.systemPrompt,
    })),
    isBuiltin: false,
    nodeALabel: currentAgents[0]?.label || "智能体 1",
    nodeAPrompt: currentAgents[0]?.systemPrompt || "",
    nodeBLabel: currentAgents[1]?.label || "智能体 2",
    nodeBPrompt: currentAgents[1]?.systemPrompt || "",
  };
  emit("savePreset", preset);
  saveAsPresetOpen.value = false;
  newPresetName.value = "";
  emit("notify", `已将当前设定存为新预设「${preset.name}」`);
}

function onCustomPromptEdit() {
  if (!draft.value) return;
  if (draft.value.presetId === "free") {
    draft.value.presetId = "custom";
  }
}

function addAgent() {
  if (!draft.value) return;
  const first = draft.value.agents[0];
  const label = nextUniqueAgentLabel(draft.value.agents);
  const newId = `ai-${Date.now().toString(36)}-${draft.value.agents.length + 1}`;
  draft.value.agents.push({
    id: newId,
    label,
    baseUrl: first?.baseUrl || "https://api.deepseek.com/v1",
    apiKey: first?.apiKey || "",
    modelName: first?.modelName || "deepseek-chat",
    temperature: 0.9,
    thinkingLevel: "default",
    systemPrompt: "",
  });
  selectedAgentIdx.value = draft.value.agents.length - 1;
}

function removeAgent(idx: number) {
  if (!draft.value || draft.value.agents.length <= 2) return;
  draft.value.agents.splice(idx, 1);
  if (selectedAgentIdx.value >= draft.value.agents.length) {
    selectedAgentIdx.value = draft.value.agents.length - 1;
  }
}

function copyCredentialsFromFirst(target: NodeConfig) {
  if (!draft.value || !draft.value.agents[0]) return;
  const first = draft.value.agents[0];
  target.baseUrl = first.baseUrl;
  target.apiKey = first.apiKey;
  if (!target.modelName) {
    target.modelName = first.modelName;
  }
}

function applyProvider(target: NodeConfig, url: string, model: string) {
  target.baseUrl = url;
  target.modelName = model;
}

function importCharacterCardToAgent(target: NodeConfig, card: CharacterCard) {
  if (!draft.value) return;
  const idx = draft.value.agents.indexOf(target);
  const assignedLabel = disambiguateAgentLabel(card.name, draft.value.agents, idx);
  target.label = assignedLabel;
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
  onCustomPromptEdit();
  emit(
    "notify",
    assignedLabel === card.name
      ? `已将灵魂角色「${card.name}」导入当前槽位`
      : `为避免重名，已将「${card.name}」命名为「${assignedLabel}」`
  );
}

async function exportConfigJson() {
  if (!draft.value) return;
  const jsonText = JSON.stringify(draft.value, null, 2);
  try {
    const filePath = await save({
      defaultPath: "ai-terrarium-config.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (filePath) {
      await invoke("save_markdown_to_file", { path: filePath, content: jsonText });
      emit("notify", "模型配置与设置已导出为 JSON");
    }
  } catch (err: any) {
    emit("notify", `导出失败：${err?.message ?? err}`);
  }
}

async function triggerImportJson() {
  try {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!selected || Array.isArray(selected)) return;
    const text = await invoke<string>("read_text_file", { path: selected });
    const parsed = JSON.parse(text);
    if (!parsed || typeof parsed !== "object") {
      throw new Error("无效的 JSON 对象");
    }
    draft.value = normalizeImportedConfig(parsed);
    selectedAgentIdx.value = 0;
    emit("notify", `已导入 ${draft.value.agents.length} 个 AI 节点配置，点击「保存配置」生效`);
  } catch (err: any) {
    emit("notify", `导入失败：${err?.message ?? "JSON 格式无效"}`);
  }
}

function normalizeImportedConfig(raw: any): ExperimentConfig {
  const fallback = draft.value!;
  const normalizeNode = (n: any, fallbackId: string, fallbackLabel: string): NodeConfig => ({
    id: typeof n?.id === "string" && n.id.trim() ? n.id.trim() : fallbackId,
    label: typeof n?.label === "string" && n.label.trim() ? n.label.trim() : fallbackLabel,
    baseUrl: typeof n?.baseUrl === "string" ? n.baseUrl : "https://api.deepseek.com/v1",
    apiKey: typeof n?.apiKey === "string" ? n.apiKey : "",
    modelName: typeof n?.modelName === "string" ? n.modelName : "deepseek-chat",
    temperature:
      typeof n?.temperature === "number" && !Number.isNaN(n.temperature)
        ? Math.min(2, Math.max(0, n.temperature))
        : 0.9,
    thinkingLevel: normalizeThinkingLevel(n?.thinkingLevel),
    systemPrompt: typeof n?.systemPrompt === "string" ? n.systemPrompt : "",
  });

  let agents: NodeConfig[] = [];
  if (Array.isArray(raw?.agents) && raw.agents.length > 0) {
    agents = raw.agents.map((a: any, i: number) =>
      normalizeNode(a, `ai-${i + 1}`, `智能体 ${i + 1}`)
    );
  } else if (raw?.nodeA || raw?.nodeB) {
    if (raw.nodeA) agents.push(normalizeNode(raw.nodeA, "AiA", "智能体 1"));
    if (raw.nodeB) agents.push(normalizeNode(raw.nodeB, "AiB", "智能体 2"));
  }
  if (agents.length === 0) {
    agents = JSON.parse(JSON.stringify(fallback.agents));
  }

  return {
    presetId: typeof raw?.presetId === "string" ? raw.presetId : fallback.presetId,
    globalSystemPrompt:
      typeof raw?.globalSystemPrompt === "string"
        ? raw.globalSystemPrompt
        : (fallback.globalSystemPrompt ?? ""),
    initialTopic: typeof raw?.initialTopic === "string" ? raw.initialTopic : "",
    observerInterval:
      typeof raw?.observerInterval === "number" && raw.observerInterval >= 1
        ? Math.round(raw.observerInterval)
        : fallback.observerInterval,
    pauseSeconds:
      typeof raw?.pauseSeconds === "number" && raw.pauseSeconds >= 0.2
        ? raw.pauseSeconds
        : fallback.pauseSeconds,
    maxConsecutiveTurns:
      typeof raw?.maxConsecutiveTurns === "number" && raw.maxConsecutiveTurns >= 1
        ? Math.round(raw.maxConsecutiveTurns)
        : fallback.maxConsecutiveTurns,
    tokenBudget:
      typeof raw?.tokenBudget === "number" && raw.tokenBudget >= 0
        ? Math.round(raw.tokenBudget)
        : (fallback.tokenBudget ?? 0),
    contextWindowTokens:
      typeof raw?.contextWindowTokens === "number" && raw.contextWindowTokens >= 2048
        ? Math.round(raw.contextWindowTokens)
        : (fallback.contextWindowTokens ?? 32000),
    compactionThresholdPct:
      typeof raw?.compactionThresholdPct === "number" &&
      raw.compactionThresholdPct >= 40 &&
      raw.compactionThresholdPct <= 95
        ? Math.round(raw.compactionThresholdPct)
        : (fallback.compactionThresholdPct ?? 80),
    boardApprovalMode:
      raw?.boardApprovalMode === "approval" || raw?.boardApprovalMode === "guardian"
        ? raw.boardApprovalMode
        : (fallback.boardApprovalMode ?? "auto"),
    enableBoardTools:
      typeof raw?.enableBoardTools === "boolean"
        ? raw.enableBoardTools
        : (fallback.enableBoardTools ?? true),
    initialBoard:
      typeof raw?.initialBoard === "string"
        ? raw.initialBoard
        : (fallback.initialBoard ?? ""),
    enablePrefixCacheOpt:
      typeof raw?.enablePrefixCacheOpt === "boolean"
        ? raw.enablePrefixCacheOpt
        : (fallback.enablePrefixCacheOpt ?? true),
    enableCompactionGuard:
      typeof raw?.enableCompactionGuard === "boolean"
        ? raw.enableCompactionGuard
        : (fallback.enableCompactionGuard ?? true),
    enablePrivateNotes:
      typeof raw?.enablePrivateNotes === "boolean"
        ? raw.enablePrivateNotes
        : (fallback.enablePrivateNotes ?? false),
    enableSmartSelector:
      typeof raw?.enableSmartSelector === "boolean"
        ? raw.enableSmartSelector
        : (fallback.enableSmartSelector ?? true),
    enableAdaptiveReasoning:
      typeof raw?.enableAdaptiveReasoning === "boolean"
        ? raw.enableAdaptiveReasoning
        : (fallback.enableAdaptiveReasoning ?? false),
    adaptiveLowReasoningEffort:
      raw?.adaptiveLowReasoningEffort === "minimal" ||
      raw?.adaptiveLowReasoningEffort === "low" ||
      raw?.adaptiveLowReasoningEffort === "medium"
        ? raw.adaptiveLowReasoningEffort
        : (fallback.adaptiveLowReasoningEffort ?? "low"),
    enablePeriodicReflection:
      typeof raw?.enablePeriodicReflection === "boolean"
        ? raw.enablePeriodicReflection
        : (fallback.enablePeriodicReflection ?? false),
    enableAutoDirector:
      typeof raw?.enableAutoDirector === "boolean"
        ? raw.enableAutoDirector
        : (fallback.enableAutoDirector ?? false),
    enableNarrativeArc:
      typeof raw?.enableNarrativeArc === "boolean"
        ? raw.enableNarrativeArc
        : (fallback.enableNarrativeArc ?? false),
    tags: Array.isArray(raw?.tags)
      ? raw.tags.filter((t: unknown): t is string => typeof t === "string")
      : [...(fallback.tags ?? [])],
    targetTotalTurns:
      typeof raw?.targetTotalTurns === "number" && raw.targetTotalTurns >= 5
        ? Math.round(raw.targetTotalTurns)
        : (fallback.targetTotalTurns ?? 30),
    allowPassTurn:
      typeof raw?.allowPassTurn === "boolean"
        ? raw.allowPassTurn
        : (fallback.allowPassTurn ?? false),
    randomMoodDice:
      typeof raw?.randomMoodDice === "boolean"
        ? raw.randomMoodDice
        : (fallback.randomMoodDice ?? false),
    intuitiveFastReply:
      typeof raw?.intuitiveFastReply === "boolean"
        ? raw.intuitiveFastReply
        : (fallback.intuitiveFastReply ?? false),
    agents,
    nodeC: raw?.nodeC
      ? normalizeNode(raw.nodeC, "observer", "观察员 C")
      : JSON.parse(JSON.stringify(fallback.nodeC)),
  };
}

const lockedMechanics = computed(() => getLockedMechanics(draft.value?.tags ?? []));

function handleToggleTag(tagId: string) {
  if (!draft.value) return;
  const currentMech: Record<MechanicKey, boolean> = {
    intuitiveFastReply: Boolean(draft.value.intuitiveFastReply),
    allowPassTurn: Boolean(draft.value.allowPassTurn),
    randomMoodDice: Boolean(draft.value.randomMoodDice),
    enablePrivateNotes: Boolean(draft.value.enablePrivateNotes),
    enableBoardTools: draft.value.enableBoardTools ?? true,
    enableSmartSelector: draft.value.enableSmartSelector ?? true,
  };
  const res = toggleExperimentTag(draft.value.tags ?? [], tagId, currentMech);
  draft.value.tags = res.tags;
  draft.value.intuitiveFastReply = res.mechanics.intuitiveFastReply;
  draft.value.allowPassTurn = res.mechanics.allowPassTurn;
  draft.value.randomMoodDice = res.mechanics.randomMoodDice;
  draft.value.enablePrivateNotes = res.mechanics.enablePrivateNotes;
  draft.value.enableBoardTools = res.mechanics.enableBoardTools;
  draft.value.enableSmartSelector = res.mechanics.enableSmartSelector;
}

function handleClearTags() {
  if (!draft.value) return;
  draft.value.tags = [];
}

async function handleSave() {
  if (!draft.value) return;
  const patchedMech = applyTagLocksToMechanics(draft.value.tags ?? [], {
    intuitiveFastReply: Boolean(draft.value.intuitiveFastReply),
    allowPassTurn: Boolean(draft.value.allowPassTurn),
    randomMoodDice: Boolean(draft.value.randomMoodDice),
    enablePrivateNotes: Boolean(draft.value.enablePrivateNotes),
    enableBoardTools: draft.value.enableBoardTools ?? true,
    enableSmartSelector: draft.value.enableSmartSelector ?? true,
  });
  draft.value.intuitiveFastReply = patchedMech.intuitiveFastReply;
  draft.value.allowPassTurn = patchedMech.allowPassTurn;
  draft.value.randomMoodDice = patchedMech.randomMoodDice;
  draft.value.enablePrivateNotes = patchedMech.enablePrivateNotes;
  draft.value.enableBoardTools = patchedMech.enableBoardTools;
  draft.value.enableSmartSelector = patchedMech.enableSmartSelector;
  const dup = findDuplicateAgentLabel(draft.value.agents);
  if (dup) {
    activeTab.value = "script";
    confirmDiscardOpen.value = false;
    emit("notify", `角色名称「${dup}」存在重复，请修改后再保存`);
    return;
  }
  draft.value.agents.forEach((a, idx) => {
    a.label = effectiveAgentLabel(a, idx);
  });
  if (corePromptDraft.value !== corePromptInitial.value) {
    await saveCoreEnginePrompt();
  }
  confirmDiscardOpen.value = false;
  triggerHaptic("confirm");
  emit("save", draft.value);
  emit("close");
}

function requestClose() {
  if (confirmDiscardOpen.value) {
    confirmDiscardOpen.value = false;
    return;
  }
  if (!isDirty.value) {
    emit("close");
    return;
  }
  confirmDiscardOpen.value = true;
}

function discardAndClose() {
  confirmDiscardOpen.value = false;
  emit("close");
}

defineExpose({
  requestClose,
});
</script>

<template>
  <AdaptiveModal
    :open="open && Boolean(draft)"
    title="设置"
    max-width-class="max-w-[780px]"
    content-class="md:h-[min(82dvh,660px)]"
    mobile-presentation="fullscreen"
    @close="requestClose"
  >
    <template #header>
      <!-- Mobile Header + Tabs (< md) -->
      <div v-if="draft" class="md:hidden shrink-0 bg-white border-b border-black/[0.06]">
        <div class="px-4 py-3 flex items-center justify-between gap-2">
          <h2 class="text-[17px] font-semibold text-[#1D1D1F] tracking-tight">设置</h2>
          <div class="flex items-center gap-1.5 shrink-0">
            <button
              type="button"
              @click="triggerImportJson"
              class="h-8 flex items-center gap-1 px-2.5 rounded-full text-[12px] font-medium bg-[#F2F2F7] text-slate-700 active:bg-[#E5E5EA] transition"
            >
              <Upload class="w-3.5 h-3.5" />
              <span>导入</span>
            </button>
            <button
              type="button"
              @click="exportConfigJson"
              class="h-8 flex items-center gap-1 px-2.5 rounded-full text-[12px] font-medium bg-[#F2F2F7] text-slate-700 active:bg-[#E5E5EA] transition"
            >
              <Download class="w-3.5 h-3.5" />
              <span>导出</span>
            </button>
            <button
              type="button"
              @click="requestClose"
              class="w-8 h-8 flex items-center justify-center rounded-full bg-[#F2F2F7] text-slate-600 active:bg-[#E5E5EA] transition"
            >
              <X class="w-4 h-4" />
            </button>
          </div>
        </div>

        <div class="px-4 pb-2.5 flex gap-1.5 overflow-x-auto no-scrollbar">
          <button
            v-for="tab in [
              { id: 'script', label: '本局剧本' },
              { id: 'agents', label: `模型与 API (${draft.agents.length})` },
              { id: 'observer', label: '观察员 C' },
              { id: 'safety', label: '外观与调度' },
            ]"
            :key="tab.id"
            type="button"
            @click="triggerHaptic('light'); activeTab = tab.id as any"
            class="h-8 px-3.5 rounded-full text-[12.5px] font-medium transition-colors shrink-0"
            :class="
              activeTab === tab.id
                ? 'bg-[#1D1D1F] text-white'
                : 'bg-[#F2F2F7] text-slate-600 hover:bg-[#EAEAEF]'
            "
          >
            {{ tab.label }}
          </button>
        </div>
      </div>
    </template>

    <!-- Body: ChatGPT / macOS Settings Clean 2-Pane Split on Desktop, Inset Grouped Cards on Mobile -->
    <div v-if="draft" class="flex-1 min-h-0 flex overflow-hidden bg-[#F2F2F7] md:bg-white">
      <!-- Desktop Full-Height Left Nav (>= md) -->
      <nav class="hidden md:flex md:w-[212px] shrink-0 flex-col justify-between bg-[#F7F7F8] border-r border-black/[0.06] p-3 select-none">
        <div class="space-y-4">
          <div class="px-2.5 pt-1.5 flex items-center justify-between">
            <span class="text-[15px] font-semibold text-[#1D1D1F] tracking-tight">设置</span>
          </div>

          <div class="space-y-0.5">
            <button
              v-for="tab in [
                { id: 'script', label: '本局剧本与角色', icon: FileText },
                { id: 'agents', label: `模型与 API (${draft.agents.length})`, icon: Cpu },
                { id: 'observer', label: '观察员 C', icon: Telescope },
                { id: 'safety', label: '外观、调度与安全阀', icon: SlidersHorizontal },
              ]"
              :key="tab.id"
              type="button"
              @click="activeTab = tab.id as any"
              class="w-full h-9 flex items-center gap-2.5 px-2.5 rounded-[10px] text-[13px] transition-colors text-left outline-none"
              :class="
                activeTab === tab.id
                  ? 'bg-black/[0.065] text-[#1D1D1F] font-medium'
                  : 'text-slate-600 font-normal hover:bg-black/[0.035] hover:text-[#1D1D1F]'
              "
            >
              <component
                :is="tab.icon"
                class="w-4 h-4 shrink-0"
                :class="activeTab === tab.id ? 'text-[#1D1D1F]' : 'text-slate-400'"
              />
              <span class="truncate">{{ tab.label }}</span>
            </button>
          </div>
        </div>

        <!-- Subtle JSON Config Import/Export inside Settings Nav Footer -->
        <div class="pt-2 border-t border-black/[0.05] space-y-0.5">
          <button
            type="button"
            @click="triggerImportJson"
            class="w-full h-8 flex items-center gap-2 px-2.5 rounded-lg text-[12px] text-slate-500 hover:text-[#1D1D1F] hover:bg-black/[0.04] transition-colors outline-none"
          >
            <Upload class="w-3.5 h-3.5 shrink-0" />
            <span>导入配置 JSON</span>
          </button>
          <button
            type="button"
            @click="exportConfigJson"
            class="w-full h-8 flex items-center gap-2 px-2.5 rounded-lg text-[12px] text-slate-500 hover:text-[#1D1D1F] hover:bg-black/[0.04] transition-colors outline-none"
          >
            <Download class="w-3.5 h-3.5 shrink-0" />
            <span>导出配置 JSON</span>
          </button>
        </div>
      </nav>

      <!-- Right Pure White Content Column on Desktop, Inset Grouped Canvas on Mobile -->
      <div class="flex-1 min-w-0 flex flex-col bg-[#F2F2F7] md:bg-white overflow-hidden">
        <!-- Desktop Clean Content Header (Title + Close X only, zero button clutter) -->
        <div class="hidden md:flex items-center justify-between px-6 h-13 border-b border-black/[0.05] shrink-0">
          <h2 class="text-[14.5px] font-semibold text-[#1D1D1F]">
            {{
              activeTab === "script"
                ? "本局剧本与角色"
                : activeTab === "agents"
                  ? "模型与 API"
                  : activeTab === 'observer'
                    ? "观察员 C"
                    : "外观、调度与安全阀"
            }}
          </h2>
          <button
            type="button"
            @click="requestClose"
            class="w-8 h-8 flex items-center justify-center rounded-lg text-slate-400 hover:text-[#1D1D1F] hover:bg-black/[0.05] transition-colors outline-none"
          >
            <X class="w-4 h-4" />
          </button>
        </div>

        <div class="flex-1 min-h-0 overflow-y-auto px-3.5 py-4 md:px-6 md:py-5">
          <div class="max-w-[540px] mx-auto space-y-4 md:space-y-5">
          <!-- Tab 1: Current Experiment Script & Roles (Inset Grouped cards on mobile, flat hairlines on desktop) -->
          <template v-if="activeTab === 'script'">
            <div class="flex flex-wrap items-center justify-between gap-2 px-1 md:px-0">
              <span class="text-[12px] text-slate-500 md:text-slate-400">
                仅调整当前实验设定 · 切换剧本请使用「新建实验」
              </span>
              <button
                type="button"
                @click="openSaveAsPreset"
                class="shrink-0 flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-[12px] font-medium text-slate-600 bg-white/80 md:bg-transparent border border-black/[0.06] md:border-transparent hover:text-[#1D1D1F] hover:bg-black/[0.05] transition outline-none"
              >
                <Save class="w-3.5 h-3.5" />
                <span>存为预设模板</span>
              </button>
            </div>

            <div
              v-if="saveAsPresetOpen"
              class="p-3 rounded-2xl md:rounded-xl bg-white md:bg-[#F7F7F8] border border-black/[0.07] flex flex-wrap items-center gap-2 shadow-2xs md:shadow-none"
            >
              <input
                v-model="newPresetName"
                @keydown.enter.prevent="confirmSaveAsPreset"
                placeholder="输入新预设模板名称（例如：深夜便利店推理）"
                class="flex-1 min-w-[180px] rounded-xl md:rounded-lg border border-black/[0.08] bg-[#F5F5F7] md:bg-white px-3 py-1.5 text-[13px] text-[#1D1D1F] placeholder:text-slate-400 outline-none focus:ring-2 focus:ring-black/10"
              />
              <button
                type="button"
                @click="saveAsPresetOpen = false"
                class="h-8 px-3 rounded-lg text-[12.5px] text-slate-600 hover:bg-black/[0.05]"
              >
                取消
              </button>
              <button
                type="button"
                @click="confirmSaveAsPreset"
                class="h-8 px-3.5 rounded-lg text-[12.5px] font-medium bg-[#1D1D1F] hover:bg-black text-white"
              >
                保存
              </button>
            </div>

            <!-- 1 & 2. 本局场景与开场引子 (Grouped White Card on Mobile) -->
            <div class="rounded-2xl bg-white p-4 border border-black/[0.05] shadow-[0_1px_2px_rgba(0,0,0,0.02)] md:rounded-none md:bg-transparent md:p-0 md:border-0 md:shadow-none space-y-4">
              <div class="pb-3 border-b border-black/[0.06]">
                <ExperimentTagsPicker
                  :tags="draft.tags ?? []"
                  @toggle="handleToggleTag"
                  @clear="handleClearTags"
                />
              </div>

              <div class="space-y-1.5">
                <div class="flex items-center justify-between gap-2">
                  <label class="text-[13px] font-semibold md:font-medium text-[#1D1D1F]">
                    本局场景与文风规则
                  </label>
                  <span class="text-[11.5px] text-slate-400">对本局所有角色生效</span>
                </div>
                <textarea
                  v-model="draft.globalSystemPrompt"
                  @input="onCustomPromptEdit"
                  rows="3"
                  placeholder="例如：场景背景、文风要求、单次发言字数限制等（可留空）"
                  class="w-full resize-none rounded-xl border border-black/[0.08] bg-[#F5F5F7] md:bg-[#F7F7F9] focus:bg-white px-3.5 py-2.5 text-[13.5px] text-[#1D1D1F] placeholder:text-slate-400 leading-relaxed outline-none focus:border-black/20 transition"
                ></textarea>
              </div>

              <div class="space-y-1.5">
                <div class="flex items-center justify-between gap-2">
                  <label class="text-[13px] font-semibold md:font-medium text-[#1D1D1F]">
                    第 1 轮开场引子
                  </label>
                  <span class="text-[11.5px] text-slate-400">可选，留空直接开聊</span>
                </div>
                <textarea
                  v-model="draft.initialTopic"
                  rows="2"
                  placeholder="留空则不设开场引子，让智能体直接开始对话"
                  class="w-full resize-none rounded-xl border border-black/[0.08] bg-[#F5F5F7] md:bg-[#F7F7F9] focus:bg-white px-3.5 py-2.5 text-[13.5px] text-[#1D1D1F] placeholder:text-slate-400 leading-relaxed outline-none focus:border-black/20 transition"
                ></textarea>
              </div>
            </div>

            <!-- 3. 各角色身份与私密底牌 -->
            <div class="pt-1 md:pt-3 md:border-t md:border-black/[0.06] space-y-3">
              <div class="flex items-center justify-between gap-2 px-1 md:px-0">
                <span class="text-[13px] font-semibold text-[#1D1D1F]">
                  角色与私密底牌（{{ draft.agents.length }} 位）
                </span>
                <button
                  type="button"
                  @click="addAgent"
                  class="h-8 flex items-center gap-1 px-3 rounded-full text-[12px] font-medium bg-white md:bg-[#F2F2F5] border border-black/[0.07] md:border-transparent hover:bg-[#E7E7EC] text-[#1D1D1F] transition outline-none"
                >
                  <Plus class="w-3.5 h-3.5" />
                  <span>添加角色</span>
                </button>
              </div>

              <div
                v-for="(ag, idx) in draft.agents"
                :key="ag.id"
                class="rounded-2xl bg-white p-4 border border-black/[0.05] shadow-[0_1px_2px_rgba(0,0,0,0.02)] md:rounded-none md:bg-transparent md:px-0 md:pb-0 md:pt-3 md:border-0 md:shadow-none space-y-2.5 md:space-y-2"
                :class="idx > 0 ? 'md:border-t md:border-black/[0.05]' : ''"
              >
                <div class="flex items-center justify-between gap-2">
                  <div class="flex items-center gap-2 flex-1 min-w-0">
                    <span class="text-[11.5px] font-mono text-slate-400 shrink-0">#{{ idx + 1 }}</span>
                    <input
                      v-model="ag.label"
                      placeholder="角色名称"
                      class="flex-1 min-w-0 h-9 rounded-xl border bg-[#F5F5F7] md:bg-[#F7F7F9] focus:bg-white px-3 text-[13.5px] font-medium text-[#1D1D1F] placeholder:text-slate-400 outline-none transition"
                      :class="
                        duplicateLabels.has(effectiveAgentLabel(ag, idx))
                          ? 'border-rose-400 ring-2 ring-rose-200/70'
                          : 'border-black/[0.08] focus:border-black/20'
                      "
                    />
                  </div>
                  <div class="flex items-center gap-1.5 shrink-0">
                    <CharacterLibraryPopover
                      :cards="characterCards"
                      :inherit-memory="inheritCardMemory"
                      :allow-delete="true"
                      @update:inherit-memory="inheritCardMemory = $event"
                      @select="importCharacterCardToAgent(ag, $event)"
                      @delete="emit('deleteCharacterCard', $event)"
                    />
                    <button
                      type="button"
                      @click="
                        triggerHaptic('light');
                        emit('saveCharacterCard', {
                          agentId: ag.id,
                          label: effectiveAgentLabel(ag, idx),
                          systemPrompt: ag.systemPrompt,
                        });
                      "
                      class="h-9 md:h-8 px-2.5 flex items-center gap-1 rounded-xl md:rounded-lg text-[12px] font-medium bg-[#F2F2F7] md:bg-[#F4F4F6] hover:bg-indigo-50 text-slate-600 hover:text-indigo-700 transition"
                      title="将该角色设定存入跨世界灵魂角色库"
                    >
                      <BookmarkPlus class="w-3.5 h-3.5" />
                      <span class="hidden sm:inline">存入库</span>
                    </button>
                    <button
                      v-if="draft.agents.length > 2"
                      type="button"
                      @click="removeAgent(idx)"
                      class="w-9 h-9 flex items-center justify-center rounded-xl text-slate-400 hover:text-rose-600 hover:bg-rose-50 transition"
                      title="删除该角色"
                    >
                      <Trash2 class="w-4 h-4" />
                    </button>
                  </div>
                </div>

                <p
                  v-if="duplicateLabels.has(effectiveAgentLabel(ag, idx))"
                  class="text-[11.5px] text-rose-600 font-medium pl-6"
                >
                  角色名称不能重复，请修改名称以区分不同智能体
                </p>
                <textarea
                  v-model="ag.systemPrompt"
                  @input="onCustomPromptEdit"
                  rows="3"
                  placeholder="设定该角色的身份背景、性格底色与隐藏动机（可留空）"
                  class="w-full resize-none rounded-xl border border-black/[0.08] bg-[#F5F5F7] md:bg-[#F7F7F9] focus:bg-white px-3.5 py-2.5 text-[13.5px] text-[#1D1D1F] placeholder:text-slate-400 leading-relaxed outline-none focus:border-black/20 transition"
                ></textarea>
              </div>
            </div>

            <div
              v-if="draft.presetId === 'free'"
              class="p-3.5 rounded-xl bg-emerald-50/70 border border-emerald-200/60 text-[12.5px] text-emerald-900 flex gap-2.5"
            >
              <Sparkles class="w-4 h-4 shrink-0 mt-0.5 text-emerald-600" />
              <div>
                <div class="font-semibold mb-0.5">当前为「自由漫谈」零预设模式</div>
                <div class="opacity-85 leading-relaxed">
                  系统自动跳过底层演戏总纲与白板工具，所有 AI 将从空白上下文开始自由对话。
                </div>
              </div>
            </div>
          </template>

          <!-- Tab 2: Models & API -->
          <template v-else-if="activeTab === 'agents'">
            <div class="flex items-center justify-between gap-2 px-0.5 md:px-0">
              <div class="flex flex-wrap items-center gap-1.5">
                <button
                  v-for="(ag, idx) in draft.agents"
                  :key="ag.id"
                  type="button"
                  @click="triggerHaptic('light'); selectedAgentIdx = idx"
                  class="h-8 flex items-center gap-1.5 px-3 rounded-full text-[12.5px] font-medium transition"
                  :class="
                    selectedAgentIdx === idx
                      ? 'bg-[#1D1D1F] text-white'
                      : 'bg-white md:bg-[#F2F2F5] text-slate-700 border border-black/[0.06] md:border-transparent hover:bg-[#E7E7EC]'
                  "
                >
                  <span>{{ ag.label || `智能体 ${idx + 1}` }}</span>
                  <span
                    v-if="!ag.apiKey.trim()"
                    class="w-1.5 h-1.5 rounded-full bg-rose-400"
                    title="未配置 API Key"
                  ></span>
                </button>
              </div>

              <button
                type="button"
                @click="addAgent"
                class="shrink-0 h-8 flex items-center gap-1 px-3 rounded-full text-[12px] font-medium bg-white md:bg-[#F2F2F5] border border-black/[0.07] md:border-transparent hover:bg-[#E7E7EC] text-[#1D1D1F] transition"
              >
                <Plus class="w-3.5 h-3.5" />
                <span>添加 AI</span>
              </button>
            </div>

            <div
              v-if="currentAgent"
              class="rounded-2xl bg-white p-4 border border-black/[0.05] shadow-[0_1px_2px_rgba(0,0,0,0.02)] md:rounded-none md:bg-transparent md:p-0 md:pt-4 md:border-0 md:border-t md:border-black/[0.06] md:shadow-none space-y-4"
            >
              <div class="flex flex-wrap items-center justify-between gap-2">
                <div class="text-[12.5px] text-slate-500">
                  正在配置「<span class="font-medium text-[#1D1D1F]">{{ currentAgent.label || `智能体 ${selectedAgentIdx + 1}` }}</span>」
                </div>
                <div class="flex items-center gap-3">
                  <button
                    v-if="selectedAgentIdx > 0"
                    type="button"
                    @click="copyCredentialsFromFirst(currentAgent)"
                    class="flex items-center gap-1 text-[12px] font-medium text-slate-600 hover:text-[#1D1D1F] transition"
                  >
                    <Copy class="w-3.5 h-3.5" />
                    同步第 1 位 Key 与地址
                  </button>
                  <button
                    v-if="draft.agents.length > 2"
                    type="button"
                    @click="removeAgent(selectedAgentIdx)"
                    class="flex items-center gap-1 text-[12px] font-medium text-rose-600 hover:underline"
                  >
                    <Trash2 class="w-3.5 h-3.5" />
                    删除
                  </button>
                </div>
              </div>

              <div class="space-y-1.5">
                <div class="text-[11.5px] font-medium text-slate-400">快捷填入端点</div>
                <div class="flex flex-wrap gap-1.5">
                  <button
                    v-for="prov in providerQuickUrls"
                    :key="prov.name"
                    type="button"
                    @click="applyProvider(currentAgent, prov.url, prov.model)"
                    class="px-2.5 py-1 rounded-lg text-[12px] bg-[#F2F2F7] md:bg-[#F4F4F6] hover:bg-[#EAEAEF] text-slate-700 transition"
                  >
                    {{ prov.name }}
                  </button>
                </div>
              </div>

              <div class="grid grid-cols-1 sm:grid-cols-2 gap-3.5">
                <div>
                  <label class="block text-[12px] font-medium text-slate-600 mb-1.5">角色名称</label>
                  <input
                    v-model="currentAgent.label"
                    placeholder="角色名称"
                    class="w-full h-10 rounded-xl border border-black/[0.08] bg-[#F5F5F7] md:bg-[#F7F7F9] focus:bg-white px-3.5 text-[13.5px] text-[#1D1D1F] placeholder:text-slate-400 outline-none focus:border-black/20 transition"
                  />
                </div>
                <div>
                  <label class="block text-[12px] font-medium text-slate-600 mb-1.5">Model Name</label>
                  <input
                    v-model="currentAgent.modelName"
                    placeholder="如 deepseek-chat / qwen-plus"
                    class="w-full h-10 rounded-xl border border-black/[0.08] bg-[#F5F5F7] md:bg-[#F7F7F9] focus:bg-white px-3.5 text-[13.5px] text-[#1D1D1F] placeholder:text-slate-400 font-mono outline-none focus:border-black/20 transition"
                  />
                </div>
              </div>

              <div>
                <label class="block text-[12px] font-medium text-slate-600 mb-1.5">Base URL</label>
                <input
                  v-model="currentAgent.baseUrl"
                  placeholder="https://api.deepseek.com/v1"
                  class="w-full h-10 rounded-xl border border-black/[0.08] bg-[#F5F5F7] md:bg-[#F7F7F9] focus:bg-white px-3.5 text-[13.5px] text-[#1D1D1F] placeholder:text-slate-400 font-mono outline-none focus:border-black/20 transition"
                />
              </div>

              <div>
                <label class="block text-[12px] font-medium text-slate-600 mb-1.5">API Key</label>
                <input
                  v-model="currentAgent.apiKey"
                  type="password"
                  placeholder="sk-..."
                  class="w-full h-10 rounded-xl border border-black/[0.08] bg-[#F5F5F7] md:bg-[#F7F7F9] focus:bg-white px-3.5 text-[13.5px] text-[#1D1D1F] placeholder:text-slate-400 font-mono outline-none focus:border-black/20 transition"
                />
              </div>

              <div>
                <div class="flex items-center justify-between text-[13px] mb-1">
                  <span class="font-medium text-slate-700">Temperature（温度）</span>
                  <span class="font-mono font-medium text-[#1D1D1F] tabular-nums px-2 py-0.5 rounded-md bg-[#F2F2F7] md:bg-transparent text-[12.5px]">
                    {{ Number(currentAgent.temperature).toFixed(2) }}
                  </span>
                </div>
                <RangeSlider
                  :model-value="currentAgent.temperature"
                  :min="0"
                  :max="1.5"
                  :step="0.05"
                  @update:model-value="currentAgent.temperature = $event"
                />
              </div>

              <div>
                <div class="flex flex-wrap items-center justify-between gap-2 text-[13px] mb-1.5">
                  <span class="font-medium text-slate-700">思考深度（Thinking / Reasoning）</span>
                  <span
                    v-if="draft.intuitiveFastReply"
                    class="px-2 py-0.5 rounded-full bg-amber-50 border border-amber-200/80 text-amber-700 text-[10.5px] font-medium"
                  >
                    已由本局「直觉快反应」临时接管为快反应模式
                  </span>
                </div>
                <div
                  class="grid grid-cols-5 p-1 rounded-xl bg-[#F2F2F7] md:bg-[#EFEFF4] gap-1 transition-opacity"
                  :class="{ 'opacity-65': draft.intuitiveFastReply }"
                >
                  <button
                    v-for="opt in thinkingLevelOptions"
                    :key="opt.value"
                    type="button"
                    @click="currentAgent.thinkingLevel = opt.value"
                    class="h-8 rounded-lg text-[12px] font-medium transition-all"
                    :class="
                      (currentAgent.thinkingLevel || 'default') === opt.value
                        ? 'bg-white text-[#1D1D1F] shadow-[0_1px_2px_rgba(0,0,0,0.08)]'
                        : 'text-slate-600 hover:text-[#1D1D1F]'
                    "
                  >
                    {{ opt.label }}
                  </button>
                </div>
                <p class="mt-1.5 text-[11px] leading-relaxed text-slate-400">
                  仅在关闭「直觉快反应」时按此档位独立生效；开启「直觉快反应」时全员自动强制快反应直出。
                </p>
              </div>
            </div>
          </template>

          <!-- Observer C Config Tab -->
          <template v-else-if="activeTab === 'observer'">
            <div class="rounded-2xl bg-white p-4 border border-black/[0.05] shadow-[0_1px_2px_rgba(0,0,0,0.02)] md:rounded-none md:bg-transparent md:p-0 md:border-0 md:shadow-none space-y-4">
              <div class="flex flex-wrap items-center justify-between gap-2">
                <div class="text-[12.5px] text-slate-500">快捷填入常用兼容端点</div>
                <button
                  type="button"
                  @click="copyCredentialsFromFirst(draft.nodeC)"
                  class="flex items-center gap-1 text-[12px] font-medium text-slate-600 hover:text-[#1D1D1F] transition"
                >
                  <Copy class="w-3.5 h-3.5" />
                  同步第 1 位 AI 的地址与 Key
                </button>
              </div>

              <div class="flex flex-wrap gap-1.5">
                <button
                  v-for="prov in providerQuickUrls"
                  :key="prov.name"
                  type="button"
                  @click="applyProvider(draft.nodeC, prov.url, prov.model)"
                  class="px-2.5 py-1 rounded-lg text-[12px] bg-[#F2F2F7] md:bg-[#F4F4F6] hover:bg-[#EAEAEF] text-slate-700 transition"
                >
                  {{ prov.name }}
                </button>
              </div>

              <div class="grid grid-cols-1 sm:grid-cols-2 gap-3.5">
                <div>
                  <label class="block text-[12px] font-medium text-slate-600 mb-1.5">角色名称</label>
                  <input
                    v-model="draft.nodeC.label"
                    placeholder="观察员 C"
                    class="w-full h-10 rounded-xl border border-black/[0.08] bg-[#F5F5F7] md:bg-[#F7F7F9] focus:bg-white px-3.5 text-[13.5px] text-[#1D1D1F] placeholder:text-slate-400 outline-none focus:border-black/20 transition"
                  />
                </div>
                <div>
                  <label class="block text-[12px] font-medium text-slate-600 mb-1.5">Model Name</label>
                  <input
                    v-model="draft.nodeC.modelName"
                    placeholder="建议使用 Flash/Turbo 级高速模型"
                    class="w-full h-10 rounded-xl border border-black/[0.08] bg-[#F5F5F7] md:bg-[#F7F7F9] focus:bg-white px-3.5 text-[13.5px] text-[#1D1D1F] placeholder:text-slate-400 font-mono outline-none focus:border-black/20 transition"
                  />
                </div>
              </div>

              <div>
                <label class="block text-[12px] font-medium text-slate-600 mb-1.5">Base URL</label>
                <input
                  v-model="draft.nodeC.baseUrl"
                  placeholder="https://api.deepseek.com/v1"
                  class="w-full h-10 rounded-xl border border-black/[0.08] bg-[#F5F5F7] md:bg-[#F7F7F9] focus:bg-white px-3.5 text-[13.5px] text-[#1D1D1F] placeholder:text-slate-400 font-mono outline-none focus:border-black/20 transition"
                />
              </div>

              <div>
                <label class="block text-[12px] font-medium text-slate-600 mb-1.5">API Key</label>
                <input
                  v-model="draft.nodeC.apiKey"
                  type="password"
                  placeholder="sk-..."
                  class="w-full h-10 rounded-xl border border-black/[0.08] bg-[#F5F5F7] md:bg-[#F7F7F9] focus:bg-white px-3.5 text-[13.5px] text-[#1D1D1F] placeholder:text-slate-400 font-mono outline-none focus:border-black/20 transition"
                />
              </div>

              <div>
                <div class="flex items-center justify-between text-[13px] mb-1">
                  <span class="font-medium text-slate-700">Temperature（温度）</span>
                  <span class="font-mono font-medium text-[#1D1D1F] tabular-nums px-2 py-0.5 rounded-md bg-[#F2F2F7] md:bg-transparent text-[12.5px]">
                    {{ Number(draft.nodeC.temperature).toFixed(2) }}
                  </span>
                </div>
                <RangeSlider
                  :model-value="draft.nodeC.temperature"
                  :min="0"
                  :max="1.5"
                  :step="0.05"
                  @update:model-value="draft.nodeC.temperature = $event"
                />
              </div>

              <div>
                <label class="block text-[12px] font-medium text-slate-600 mb-1.5">
                  观察员 System Prompt
                </label>
                <textarea
                  v-model="draft.nodeC.systemPrompt"
                  rows="4"
                  placeholder="输入观察员诊断视角提示词..."
                  class="w-full resize-none rounded-xl border border-black/[0.08] bg-[#F5F5F7] md:bg-[#F7F7F9] focus:bg-white px-3.5 py-2.5 text-[13.5px] text-[#1D1D1F] placeholder:text-slate-400 leading-relaxed outline-none focus:border-black/20 transition"
                ></textarea>
              </div>

              <div class="pt-3 border-t border-black/[0.06] flex items-center justify-between gap-3">
                <div class="min-w-0">
                  <div class="text-[13px] font-medium text-[#1D1D1F]">戏剧情境导演模式</div>
                  <div class="text-[12px] text-slate-400 mt-0.5 leading-relaxed">
                    陷入高僵化客套循环或缺乏新信息推进时，由观察员 C 结合剧本世界观注入客观突发环境事件（环境旁白），取代生硬的元指令。
                  </div>
                </div>
                <ToggleSwitch v-model="draft.enableAutoDirector" />
              </div>
            </div>
          </template>

          <!-- Appearance, Safety & Scheduling Tab (Grouped cards on mobile, hairline rows on desktop) -->
          <template v-else>
            <div class="space-y-3 md:space-y-0 md:divide-y md:divide-black/[0.06]">
              <!-- Theme Mode Switcher -->
              <div class="rounded-2xl bg-white p-4 border border-black/[0.05] shadow-[0_1px_2px_rgba(0,0,0,0.02)] md:rounded-none md:bg-transparent md:px-0 md:pt-0 md:pb-4 md:border-0 md:shadow-none flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
                <div class="min-w-0">
                  <div class="text-[13.5px] font-medium text-[#1D1D1F]">界面主题外观</div>
                  <div class="text-[12px] text-slate-400 mt-0.5">
                    支持浅色、深色或自动跟随系统外观（即时生效）
                  </div>
                </div>
                <div class="grid grid-cols-3 p-1 rounded-xl bg-[#EFEFF4] gap-1 shrink-0 sm:w-[240px]">
                  <button
                    v-for="opt in ([
                      { id: 'system', label: '跟随系统', icon: Monitor },
                      { id: 'light', label: '浅色', icon: Sun },
                      { id: 'dark', label: '深色', icon: Moon },
                    ] as const)"
                    :key="opt.id"
                    type="button"
                    @click="triggerHaptic('light'); setThemePreference(opt.id as ThemePreference)"
                    class="h-8 flex items-center justify-center gap-1 rounded-lg text-[12px] font-medium transition-all outline-none"
                    :class="
                      themePreference === opt.id
                        ? 'bg-white text-[#1D1D1F] shadow-[0_1px_3px_rgba(0,0,0,0.08)]'
                        : 'text-slate-600 hover:text-[#1D1D1F]'
                    "
                  >
                    <component :is="opt.icon" class="w-3.5 h-3.5 shrink-0" />
                    <span>{{ opt.label }}</span>
                  </button>
                </div>
              </div>

              <!-- Collapsible Core Engine Prompt -->
              <div class="rounded-2xl bg-white p-4 border border-black/[0.05] shadow-[0_1px_2px_rgba(0,0,0,0.02)] md:rounded-none md:bg-transparent md:px-0 md:py-4 md:border-0 md:shadow-none">
                <button
                  type="button"
                  @click="corePromptOpen = !corePromptOpen"
                  class="w-full py-0.5 md:py-2 flex items-center justify-between gap-2 text-left group"
                >
                  <div class="flex flex-wrap items-center gap-2 min-w-0">
                    <span class="text-[13.5px] font-medium text-[#1D1D1F]">
                      系统底层演戏总纲
                    </span>
                    <span class="px-2 py-0.5 rounded-full bg-[#F2F2F5] text-[11px] text-slate-500">
                      跨所有实验生效 · 自由漫谈跳过
                    </span>
                  </div>
                  <component
                    :is="corePromptOpen ? ChevronDown : ChevronRight"
                    class="w-4 h-4 text-slate-400 group-hover:text-slate-700 shrink-0"
                  />
                </button>
                <div v-if="corePromptOpen" class="pt-3 space-y-2.5">
                  <textarea
                    v-model="corePromptDraft"
                    rows="5"
                    placeholder="输入全局底层演戏法则..."
                    class="w-full resize-none rounded-xl border border-black/[0.08] bg-[#F5F5F7] md:bg-[#F7F7F9] focus:bg-white px-3.5 py-2.5 text-[13.5px] text-[#1D1D1F] placeholder:text-slate-400 leading-relaxed outline-none focus:border-black/20 transition"
                  ></textarea>
                  <div class="flex items-center justify-end gap-2">
                    <button
                      type="button"
                      @click="resetCoreEnginePrompt"
                      :disabled="corePromptSaving"
                      class="h-8 flex items-center gap-1 px-3 rounded-lg text-[12px] font-medium text-slate-600 hover:bg-black/[0.05] transition disabled:opacity-50"
                    >
                      <RotateCcw class="w-3 h-3" />
                      <span>恢复默认</span>
                    </button>
                    <button
                      type="button"
                      @click="saveCoreEnginePrompt"
                      :disabled="corePromptSaving"
                      class="h-8 flex items-center gap-1 px-3.5 rounded-lg text-[12px] font-medium bg-[#1D1D1F] hover:bg-black text-white transition disabled:opacity-50"
                    >
                      <Save class="w-3 h-3" />
                      <span>保存总纲</span>
                    </button>
                  </div>
                </div>
              </div>

              <!-- Cadence Sliders -->
              <div class="py-4 space-y-1.5">
                <div class="flex items-center justify-between text-[13px]">
                  <span class="font-medium text-[#1D1D1F]">观察员 C 触发周期（轮）</span>
                  <span class="font-medium tabular-nums text-slate-600">每 {{ draft.observerInterval }} 轮</span>
                </div>
                <RangeSlider
                  :model-value="draft.observerInterval"
                  :min="2"
                  :max="30"
                  :step="1"
                  @update:model-value="draft.observerInterval = $event"
                />
                <p class="text-[12px] text-slate-400">
                  主对话推进满 N 轮后后台异步诊断，不阻塞对话流。
                </p>
              </div>

              <div class="py-4 space-y-1.5">
                <div class="flex items-center justify-between text-[13px]">
                  <span class="font-medium text-[#1D1D1F]">回合呼吸停顿时间（秒）</span>
                  <span class="font-medium tabular-nums text-slate-600">{{ Number(draft.pauseSeconds).toFixed(1) }} s</span>
                </div>
                <RangeSlider
                  :model-value="draft.pauseSeconds"
                  :min="0.5"
                  :max="10"
                  :step="0.5"
                  @update:model-value="draft.pauseSeconds = $event"
                />
                <p class="text-[12px] text-slate-400">
                  每轮 AI 发言完毕后的观察缓冲间隔。
                </p>
              </div>

              <!-- Limits -->
              <div class="py-4 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2">
                <div class="min-w-0">
                  <div class="text-[13px] font-medium text-[#1D1D1F]">最大连续运行轮数熔断</div>
                  <div class="text-[12px] text-slate-400 mt-0.5">单次「开始演化」最多推进该轮数即自动暂停</div>
                </div>
                <input
                  v-model.number="draft.maxConsecutiveTurns"
                  type="number"
                  min="1"
                  max="1000"
                  class="w-full sm:w-28 h-9 rounded-xl border border-black/[0.08] bg-[#F7F7F9] focus:bg-white px-3 text-[13px] font-mono outline-none focus:border-black/20 transition shrink-0"
                />
              </div>

              <div class="py-4 space-y-2.5">
                <div class="flex flex-wrap items-center justify-between gap-2">
                  <span class="text-[13px] font-medium text-[#1D1D1F]">单局 Token 预算熔断（0 = 不限）</span>
                  <div class="flex items-center gap-1">
                    <button
                      v-for="presetVal in [0, 50000, 150000, 500000]"
                      :key="presetVal"
                      type="button"
                      @click="draft.tokenBudget = presetVal"
                      class="px-2.5 py-0.5 rounded-full text-[11.5px] transition"
                      :class="
                        (draft.tokenBudget ?? 0) === presetVal
                          ? 'bg-[#1D1D1F] text-white'
                          : 'bg-[#F2F2F5] text-slate-600 hover:bg-[#E7E7EC]'
                      "
                    >
                      {{ presetVal === 0 ? "不限" : `${presetVal / 1000}k` }}
                    </button>
                  </div>
                </div>
                <input
                  v-model.number="draft.tokenBudget"
                  type="number"
                  min="0"
                  step="5000"
                  class="w-full sm:w-44 h-9 rounded-xl border border-black/[0.08] bg-[#F7F7F9] focus:bg-white px-3 text-[13px] font-mono outline-none focus:border-black/20 transition"
                />
              </div>

              <div class="py-4 space-y-3">
                <div class="grid grid-cols-1 sm:grid-cols-2 gap-4 items-end">
                  <div>
                    <label class="block text-[13px] font-medium text-[#1D1D1F] mb-1.5">
                      上下文预算窗口（Tokens）
                    </label>
                    <input
                      v-model.number="draft.contextWindowTokens"
                      type="number"
                      min="2048"
                      step="4096"
                      class="w-full h-9 rounded-xl border border-black/[0.08] bg-[#F7F7F9] focus:bg-white px-3 text-[13px] font-mono outline-none focus:border-black/20 transition"
                    />
                  </div>
                  <div>
                    <div class="flex items-center justify-between text-[13px] mb-1">
                      <span class="font-medium text-[#1D1D1F]">自动记忆折叠阈值</span>
                      <span class="font-medium tabular-nums text-slate-600">
                        {{ draft.compactionThresholdPct ?? 80 }}%
                      </span>
                    </div>
                    <RangeSlider
                      :model-value="draft.compactionThresholdPct ?? 80"
                      :min="50"
                      :max="95"
                      :step="5"
                      @update:model-value="draft.compactionThresholdPct = $event"
                    />
                  </div>
                </div>
                <p class="text-[12px] text-slate-400">
                  达到阈值时自动浓缩早期对话为「历史纪要」，保留最近 6~8 轮细节。
                </p>
              </div>

              <!-- Whiteboard Section -->
              <div class="py-4 space-y-3">
                <div class="flex items-center justify-between gap-3">
                  <div class="min-w-0">
                    <div class="text-[13px] font-medium text-[#1D1D1F]">启用实验室共享白板工具</div>
                    <div class="text-[11.5px] leading-relaxed text-slate-400 mt-0.5">
                      开：{{ MECHANIC_DESCRIPTIONS.enableBoardTools.onEffect }} ｜ 关：{{ MECHANIC_DESCRIPTIONS.enableBoardTools.offEffect }}
                    </div>
                  </div>
                  <ToggleSwitch v-model="draft.enableBoardTools" />
                </div>

                <div v-if="draft.enableBoardTools" class="flex flex-wrap items-center justify-between gap-3 pt-1">
                  <span class="text-[12.5px] text-slate-600">白板修改权限</span>
                  <div class="flex p-0.5 rounded-full bg-[#F2F2F5]">
                    <button
                      type="button"
                      @click="draft.boardApprovalMode = 'auto'"
                      class="px-3 py-1 rounded-full text-[11.5px] font-medium transition"
                      :class="
                        draft.boardApprovalMode !== 'approval' && draft.boardApprovalMode !== 'guardian'
                          ? 'bg-white text-[#1D1D1F] shadow-2xs'
                          : 'text-slate-600'
                      "
                    >
                      自动放行
                    </button>
                    <button
                      type="button"
                      @click="draft.boardApprovalMode = 'guardian'"
                      class="px-3 py-1 rounded-full text-[11.5px] font-medium transition"
                      :class="
                        draft.boardApprovalMode === 'guardian'
                          ? 'bg-indigo-600 text-white'
                          : 'text-slate-600'
                      "
                    >
                      防泄密审核
                    </button>
                    <button
                      type="button"
                      @click="draft.boardApprovalMode = 'approval'"
                      class="px-3 py-1 rounded-full text-[11.5px] font-medium transition"
                      :class="
                        draft.boardApprovalMode === 'approval'
                          ? 'bg-amber-500 text-white'
                          : 'text-slate-600'
                      "
                    >
                      人工审批
                    </button>
                  </div>
                </div>
              </div>

              <!-- Advanced Engine Features -->
              <div class="py-4 space-y-3.5">
                <div class="text-[12px] font-semibold text-slate-400 uppercase tracking-wider">进阶引擎优化</div>

                <div class="flex items-center justify-between gap-3">
                  <div class="min-w-0">
                    <div class="text-[13px] font-medium text-[#1D1D1F]">前缀缓存优化</div>
                    <div class="text-[12px] text-slate-400 mt-0.5">固定人设置顶以复用平台缓存折扣</div>
                  </div>
                  <ToggleSwitch v-model="draft.enablePrefixCacheOpt" />
                </div>

                <div class="flex items-center justify-between gap-3">
                  <div class="min-w-0">
                    <div class="text-[13px] font-medium text-[#1D1D1F]">记忆压缩防循环保护</div>
                    <div class="text-[12px] text-slate-400 mt-0.5">记忆窗口过小时自动暂停并提醒</div>
                  </div>
                  <ToggleSwitch v-model="draft.enableCompactionGuard" />
                </div>

                <div
                  class="flex items-center justify-between gap-3"
                  :class="lockedMechanics.enablePrivateNotes ? 'opacity-60' : ''"
                  :title="lockedMechanics.enablePrivateNotes ? `已由标签 [${lockedMechanics.enablePrivateNotes}] 自动锁定，取消该标签即可手动调节` : ''"
                >
                  <div class="min-w-0">
                    <div class="text-[13px] font-medium text-[#1D1D1F] flex items-center gap-1.5">
                      <span>角色私密备忘</span>
                      <span v-if="lockedMechanics.enablePrivateNotes" class="px-1.5 py-0.5 rounded bg-amber-50 text-amber-700 text-[10.5px]">
                        已由 [{{ lockedMechanics.enablePrivateNotes }}] 锁定
                      </span>
                    </div>
                    <div class="text-[11.5px] leading-relaxed text-slate-400 mt-0.5">
                      开：{{ MECHANIC_DESCRIPTIONS.enablePrivateNotes.onEffect }} ｜ 关：{{ MECHANIC_DESCRIPTIONS.enablePrivateNotes.offEffect }}
                    </div>
                  </div>
                  <ToggleSwitch v-model="draft.enablePrivateNotes" :disabled="Boolean(lockedMechanics.enablePrivateNotes)" />
                </div>

                <div class="flex items-center justify-between gap-3">
                  <div class="min-w-0">
                    <div class="text-[13px] font-medium text-[#1D1D1F]">多人智能点名对话</div>
                    <div class="text-[11.5px] leading-relaxed text-slate-400 mt-0.5">
                      开：{{ MECHANIC_DESCRIPTIONS.enableSmartSelector.onEffect }} ｜ 关：{{ MECHANIC_DESCRIPTIONS.enableSmartSelector.offEffect }}
                    </div>
                  </div>
                  <ToggleSwitch v-model="draft.enableSmartSelector" />
                </div>

                <div class="space-y-2">
                  <div class="flex items-center justify-between gap-3">
                    <div class="min-w-0">
                      <div class="text-[13px] font-medium text-[#1D1D1F]">按冲突强度调节推理深度</div>
                      <div class="text-[12px] text-slate-400 mt-0.5">平淡对话降耗，冲突或终局恢复全速</div>
                    </div>
                    <ToggleSwitch v-model="draft.enableAdaptiveReasoning" />
                  </div>
                  <div
                    v-if="draft.enableAdaptiveReasoning"
                    class="flex flex-wrap items-center justify-between gap-2 pl-3 py-1.5 border-l-2 border-slate-200"
                  >
                    <span class="text-[12px] text-slate-500">低张力档位</span>
                    <div class="flex p-0.5 rounded-full bg-[#F2F2F5]">
                      <button
                        v-for="tier in [
                          { id: 'minimal', label: 'minimal' },
                          { id: 'low', label: 'low' },
                          { id: 'medium', label: 'medium' },
                        ]"
                        :key="tier.id"
                        type="button"
                        @click="draft.adaptiveLowReasoningEffort = tier.id"
                        class="px-2.5 py-0.5 rounded-full text-[11px] font-mono font-medium transition"
                        :class="
                          (draft.adaptiveLowReasoningEffort ?? 'low') === tier.id
                            ? 'bg-white text-[#1D1D1F] shadow-2xs'
                            : 'text-slate-500 hover:text-[#1D1D1F]'
                        "
                      >
                        {{ tier.label }}
                      </button>
                    </div>
                  </div>
                </div>

                <div class="flex items-center justify-between gap-3">
                  <div class="min-w-0">
                    <div class="text-[13px] font-medium text-[#1D1D1F]">周期直觉反思提示</div>
                    <div class="text-[12px] text-slate-400 mt-0.5">观察员顺带为角色生成直觉提示推动剧情</div>
                  </div>
                  <ToggleSwitch v-model="draft.enablePeriodicReflection" />
                </div>

                <div class="space-y-2">
                  <div class="flex items-center justify-between gap-3">
                    <div class="min-w-0">
                      <div class="text-[13px] font-medium text-[#1D1D1F]">三幕式节奏与自然结局</div>
                      <div class="text-[12px] text-slate-400 mt-0.5">按目标总轮数把控节奏并引导自然落幕</div>
                    </div>
                    <ToggleSwitch v-model="draft.enableNarrativeArc" />
                  </div>
                  <div
                    v-if="draft.enableNarrativeArc"
                    class="flex items-center justify-between gap-2 pl-3 py-1 border-l-2 border-slate-200"
                  >
                    <span class="text-[12px] text-slate-500">目标总轮数</span>
                    <input
                      v-model.number="draft.targetTotalTurns"
                      type="number"
                      min="5"
                      max="500"
                      class="w-24 h-8 rounded-lg border border-black/[0.08] bg-[#F7F7F9] px-2.5 text-[12.5px] font-mono outline-none focus:border-black/20"
                    />
                  </div>
                </div>
              </div>

              <!-- Natural Social & Human-like Enhancements -->
              <div class="py-4 space-y-3.5">
                <div class="text-[12px] font-semibold text-slate-400 uppercase tracking-wider">自然社交与活人感</div>

                <div class="flex items-center justify-between gap-3">
                  <div class="min-w-0">
                    <div class="text-[13px] font-medium text-[#1D1D1F]">允许选择性沉默 [PASS]</div>
                    <div class="text-[11.5px] leading-relaxed text-slate-400 mt-0.5">
                      开：{{ MECHANIC_DESCRIPTIONS.allowPassTurn.onEffect }} ｜ 关：{{ MECHANIC_DESCRIPTIONS.allowPassTurn.offEffect }}
                    </div>
                  </div>
                  <ToggleSwitch v-model="draft.allowPassTurn" />
                </div>

                <div
                  class="flex items-center justify-between gap-3"
                  :class="lockedMechanics.randomMoodDice ? 'opacity-60' : ''"
                  :title="lockedMechanics.randomMoodDice ? `已由标签 [${lockedMechanics.randomMoodDice}] 自动锁定，取消该标签即可手动调节` : ''"
                >
                  <div class="min-w-0">
                    <div class="text-[13px] font-medium text-[#1D1D1F] flex items-center gap-1.5">
                      <span>随机微情绪骰子</span>
                      <span v-if="lockedMechanics.randomMoodDice" class="px-1.5 py-0.5 rounded bg-amber-50 text-amber-700 text-[10.5px]">
                        已由 [{{ lockedMechanics.randomMoodDice }}] 锁定
                      </span>
                    </div>
                    <div class="text-[11.5px] leading-relaxed text-slate-400 mt-0.5">
                      开：{{ MECHANIC_DESCRIPTIONS.randomMoodDice.onEffect }} ｜ 关：{{ MECHANIC_DESCRIPTIONS.randomMoodDice.offEffect }}
                    </div>
                  </div>
                  <ToggleSwitch v-model="draft.randomMoodDice" :disabled="Boolean(lockedMechanics.randomMoodDice)" />
                </div>

                <div
                  class="flex items-center justify-between gap-3"
                  :class="lockedMechanics.intuitiveFastReply ? 'opacity-60' : ''"
                  :title="lockedMechanics.intuitiveFastReply ? `已由标签 [${lockedMechanics.intuitiveFastReply}] 自动锁定，取消该标签即可手动调节` : ''"
                >
                  <div class="min-w-0">
                    <div class="text-[13px] font-medium text-[#1D1D1F] flex items-center gap-1.5">
                      <span>直觉快反应模式</span>
                      <span v-if="lockedMechanics.intuitiveFastReply" class="px-1.5 py-0.5 rounded bg-amber-50 text-amber-700 text-[10.5px]">
                        已由 [{{ lockedMechanics.intuitiveFastReply }}] 锁定
                      </span>
                    </div>
                    <div class="text-[11.5px] leading-relaxed text-slate-400 mt-0.5">
                      开：{{ MECHANIC_DESCRIPTIONS.intuitiveFastReply.onEffect }} ｜ 关：{{ MECHANIC_DESCRIPTIONS.intuitiveFastReply.offEffect }}
                    </div>
                  </div>
                  <ToggleSwitch v-model="draft.intuitiveFastReply" :disabled="Boolean(lockedMechanics.intuitiveFastReply)" />
                </div>
              </div>
            </div>
          </template>
          </div>
        </div>

        <!-- Clean Minimal Footer (Avoids stretched giant black bar on foldables/tablets) -->
        <div class="shrink-0 px-4 md:px-6 py-3 border-t border-black/[0.06] bg-white">
          <div class="max-w-[540px] mx-auto grid grid-cols-[1fr_2fr] min-[520px]:flex min-[520px]:items-center min-[520px]:justify-end gap-2.5">
            <button
              type="button"
              @click="requestClose"
              class="h-10 md:h-8 px-4 rounded-xl md:rounded-lg text-[13.5px] md:text-[12.5px] font-medium text-slate-600 bg-[#F2F2F7] md:bg-transparent hover:bg-black/[0.05] transition-colors outline-none"
            >
              取消
            </button>
            <button
              type="button"
              @click="handleSave"
              class="h-10 md:h-8 px-6 md:px-4 rounded-xl md:rounded-lg text-[13.5px] md:text-[12.5px] font-medium bg-[#1D1D1F] hover:bg-black text-white transition-colors outline-none"
            >
              保存配置
            </button>
          </div>
        </div>
      </div>

      <!-- Unsaved Changes Confirmation Dialog Overlay -->
      <div
        v-if="confirmDiscardOpen"
        class="absolute inset-0 z-50 bg-black/25 backdrop-blur-[2px] flex items-center justify-center p-4"
        @click.self="confirmDiscardOpen = false"
      >
        <div class="w-full max-w-[360px] rounded-2xl bg-white border border-black/[0.08] shadow-[0_20px_50px_rgba(0,0,0,0.22)] p-5 space-y-4">
          <div class="space-y-1.5">
            <h3 class="text-[15px] font-semibold text-[#1D1D1F]">尚未保存配置修改</h3>
            <p class="text-[13px] text-slate-500 leading-relaxed">
              当前设置中有未保存的变更，是否保存后再关闭？
            </p>
          </div>
          <div class="flex flex-wrap items-center justify-end gap-2 pt-1">
            <button
              type="button"
              @click="confirmDiscardOpen = false"
              class="h-8 px-3 rounded-lg text-[12.5px] font-medium text-slate-600 hover:bg-black/[0.05] transition outline-none"
            >
              继续编辑
            </button>
            <button
              type="button"
              @click="discardAndClose"
              class="h-8 px-3 rounded-lg text-[12.5px] font-medium text-rose-600 hover:bg-rose-50 transition outline-none"
            >
              放弃修改
            </button>
            <button
              type="button"
              @click="handleSave"
              class="h-8 px-4 rounded-lg text-[12.5px] font-medium bg-[#1D1D1F] hover:bg-black text-white transition outline-none"
            >
              保存并关闭
            </button>
          </div>
        </div>
      </div>
    </div>
  </AdaptiveModal>
</template>
