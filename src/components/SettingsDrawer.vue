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
  BookUser,
  ChevronDown,
  ChevronRight,
} from "lucide-vue-next";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import type { CharacterCard, ExperimentConfig, NodeConfig, PresetTemplate } from "../types";
import AdaptiveModal from "./ui/AdaptiveModal.vue";

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
  notify: [message: string];
}>();

const draft = ref<ExperimentConfig | null>(null);
const activeTab = ref<"script" | "agents" | "observer" | "safety">("script");
const selectedAgentIdx = ref(0);
const fileInputRef = ref<HTMLInputElement | null>(null);
const cardLibraryOpenIdx = ref<number | null>(null);
const saveAsPresetOpen = ref(false);
const newPresetName = ref("");
const corePromptOpen = ref(false);
const corePromptDraft = ref("");
const corePromptSaving = ref(false);

async function loadCoreEnginePrompt() {
  try {
    corePromptDraft.value = await invoke<string>("get_core_engine_prompt");
  } catch {
    // ignore load failure
  }
}

async function saveCoreEnginePrompt() {
  if (corePromptSaving.value) return;
  corePromptSaving.value = true;
  try {
    corePromptDraft.value = await invoke<string>("save_core_engine_prompt", {
      prompt: corePromptDraft.value,
    });
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
    corePromptDraft.value = await invoke<string>("reset_core_engine_prompt");
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
      cloned.enableNarrativeArc = cloned.enableNarrativeArc ?? false;
      cloned.targetTotalTurns = cloned.targetTotalTurns ?? 30;
      draft.value = cloned;
      selectedAgentIdx.value = 0;
      cardLibraryOpenIdx.value = null;
      saveAsPresetOpen.value = false;
      newPresetName.value = "";
      loadCoreEnginePrompt();
    }
  },
  { immediate: true }
);

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
  const nextNum = draft.value.agents.length + 1;
  const newId = `ai-${Date.now().toString(36)}-${nextNum}`;
  draft.value.agents.push({
    id: newId,
    label: `智能体 ${nextNum}`,
    baseUrl: first?.baseUrl || "https://api.deepseek.com/v1",
    apiKey: first?.apiKey || "",
    modelName: first?.modelName || "deepseek-chat",
    temperature: 0.9,
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
  target.label = card.name;
  const parts = [card.basePrompt.trim()];
  if (card.behaviorTraits.trim()) {
    parts.push(`【跨世界博弈特质】\n${card.behaviorTraits.trim()}`);
  }
  target.systemPrompt = parts.filter(Boolean).join("\n\n");
  onCustomPromptEdit();
  cardLibraryOpenIdx.value = null;
  emit("notify", `已将灵魂角色「${card.name}」导入当前槽位`);
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
  } catch {
    const blob = new Blob([jsonText], { type: "application/json;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "ai-terrarium-config.json";
    a.click();
    URL.revokeObjectURL(url);
    emit("notify", "已触发配置 JSON 下载");
  }
}

function triggerImportJson() {
  fileInputRef.value?.click();
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
    enableNarrativeArc:
      typeof raw?.enableNarrativeArc === "boolean"
        ? raw.enableNarrativeArc
        : (fallback.enableNarrativeArc ?? false),
    targetTotalTurns:
      typeof raw?.targetTotalTurns === "number" && raw.targetTotalTurns >= 5
        ? Math.round(raw.targetTotalTurns)
        : (fallback.targetTotalTurns ?? 30),
    agents,
    nodeC: raw?.nodeC
      ? normalizeNode(raw.nodeC, "observer", "观察员 C")
      : JSON.parse(JSON.stringify(fallback.nodeC)),
  };
}

async function onJsonFileSelected(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (!file) return;
  try {
    const text = await file.text();
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

function handleSave() {
  if (!draft.value) return;
  emit("save", draft.value);
  emit("close");
}
</script>

<template>
  <AdaptiveModal
    :open="open && Boolean(draft)"
    title="模型节点与生态规则设置"
    max-width-class="max-w-3xl"
    content-class="h-[min(86vh,780px)]"
    @close="emit('close')"
  >
    <template #header>
      <div v-if="draft" class="shrink-0 bg-white">
        <!-- Header -->
        <div class="px-5 md:px-6 py-4 border-b border-black/[0.06] flex items-center justify-between gap-2">
          <div class="min-w-0">
            <h2 class="text-base font-semibold text-[#1D1D1F]">模型节点与生态规则设置</h2>
            <p class="text-xs text-slate-400 truncate">
              支持任意数量 AI 节点，支持 JSON 导入导出完整配置
            </p>
          </div>
          <div class="flex items-center gap-1.5 shrink-0">
            <input
              ref="fileInputRef"
              type="file"
              accept=".json,application/json"
              class="hidden"
              @change="onJsonFileSelected"
            />
            <button
              type="button"
              @click="triggerImportJson"
              class="flex items-center gap-1 px-2.5 py-1.5 rounded-full text-[11px] font-medium bg-[#F5F5F7] hover:bg-[#EAEAEF] text-slate-700 border border-black/[0.05] transition-colors"
              title="从 JSON 文件导入模型配置与设置"
            >
              <Upload class="w-3.5 h-3.5" />
              <span class="hidden sm:inline">导入 JSON</span>
            </button>
            <button
              type="button"
              @click="exportConfigJson"
              class="flex items-center gap-1 px-2.5 py-1.5 rounded-full text-[11px] font-medium bg-[#F5F5F7] hover:bg-[#EAEAEF] text-slate-700 border border-black/[0.05] transition-colors"
              title="导出当前所有模型配置与设置为 JSON 文件"
            >
              <Download class="w-3.5 h-3.5" />
              <span class="hidden sm:inline">导出 JSON</span>
            </button>
            <button
              type="button"
              @click="emit('close')"
              class="p-2 rounded-full hover:bg-slate-100 text-slate-500 transition-colors"
            >
              <X class="w-4 h-4" />
            </button>
          </div>
        </div>

        <!-- Tabs -->
        <div class="px-5 md:px-6 pt-3 pb-2.5 border-b border-black/[0.05] flex gap-1.5 overflow-x-auto">
          <button
            v-for="tab in [
              { id: 'script', label: '本局剧本与角色' },
              { id: 'agents', label: `模型与 API (${draft.agents.length})` },
              { id: 'observer', label: '观察员 C' },
              { id: 'safety', label: '调度与安全阀' },
            ]"
            :key="tab.id"
            type="button"
            @click="activeTab = tab.id as any"
            class="px-3.5 py-1.5 rounded-full text-xs font-medium transition-colors shrink-0"
            :class="
              activeTab === tab.id
                ? 'bg-[#1D1D1F] text-white'
                : 'bg-[#F5F5F7] text-slate-600 hover:bg-[#EAEAEF]'
            "
          >
            {{ tab.label }}
          </button>
        </div>
      </div>
    </template>

    <!-- Body -->
    <div v-if="draft" class="flex-1 min-h-0 overflow-y-auto p-5 md:p-6 space-y-5 bg-white">
          <!-- Tab 1: Current Experiment Script & Roles -->
          <template v-if="activeTab === 'script'">
            <!-- Compact Save-as-Preset Action Bar -->
            <div class="flex flex-wrap items-center justify-between gap-2 px-1">
              <span class="text-xs text-slate-500">
                仅调整当前实验设定；如需切换或管理剧本库，请点击顶栏「新建实验」。
              </span>
              <button
                type="button"
                @click="openSaveAsPreset"
                class="shrink-0 flex items-center gap-1 px-3 py-1.5 rounded-full text-[11px] font-medium bg-[#F5F5F7] hover:bg-[#EAEAEF] text-[#007AFF] border border-black/[0.06] transition"
              >
                <Save class="w-3 h-3" />
                <span>将当前设定存为新预设模板</span>
              </button>
            </div>

            <div
              v-if="saveAsPresetOpen"
              class="p-3 rounded-2xl bg-blue-50/60 border border-[#007AFF]/30 flex flex-wrap items-center gap-2"
            >
              <input
                v-model="newPresetName"
                @keydown.enter.prevent="confirmSaveAsPreset"
                placeholder="输入新预设模板名称（例如：深夜便利店推理）"
                class="flex-1 min-w-[180px] rounded-xl border border-black/[0.08] bg-white px-3 py-1.5 text-xs outline-none focus:ring-2 focus:ring-[#007AFF]/30"
              />
              <button
                type="button"
                @click="saveAsPresetOpen = false"
                class="px-3 py-1.5 rounded-full text-xs text-slate-600 bg-white border border-black/[0.08] hover:bg-slate-100"
              >
                取消
              </button>
              <button
                type="button"
                @click="confirmSaveAsPreset"
                class="px-3.5 py-1.5 rounded-full text-xs font-medium bg-[#007AFF] hover:bg-[#0062CC] text-white"
              >
                确认保存
              </button>
            </div>

            <!-- 1. 本局场景与文风规则 -->
            <div class="p-4 rounded-2xl bg-[#FAFAFC] border border-black/[0.07] space-y-2">
              <div class="flex items-center justify-between gap-2">
                <label class="text-xs font-semibold text-[#1D1D1F]">
                  本局场景与文风规则
                </label>
                <span class="text-[11px] text-slate-400">对本局所有角色生效</span>
              </div>
              <textarea
                v-model="draft.globalSystemPrompt"
                rows="3"
                placeholder="例如：场景背景、文风要求、单次发言字数限制等（可留空）"
                class="w-full rounded-xl border border-black/[0.08] bg-white px-3 py-2 text-sm md:text-xs leading-relaxed outline-none focus:ring-2 focus:ring-[#007AFF]/30"
              ></textarea>
            </div>

            <!-- 2. 第 1 轮开场引子 -->
            <div class="p-4 rounded-2xl bg-[#FAFAFC] border border-black/[0.07] space-y-2">
              <div class="flex items-center justify-between gap-2">
                <label class="text-xs font-semibold text-[#1D1D1F]">
                  第 1 轮开场引子
                </label>
                <span class="text-[11px] text-slate-400">可选，留空则无引子直接开聊</span>
              </div>
              <textarea
                v-model="draft.initialTopic"
                rows="2"
                placeholder="留空则不设开场引子，让智能体直接开始对话"
                class="w-full rounded-xl border border-black/[0.08] bg-white px-3 py-2 text-sm md:text-xs leading-relaxed outline-none focus:ring-2 focus:ring-[#007AFF]/30"
              ></textarea>
            </div>

            <!-- 3. 各角色身份与私密底牌 -->
            <div class="space-y-3">
              <div class="flex items-center justify-between gap-2">
                <span class="text-xs font-semibold text-[#1D1D1F]">
                  各角色身份与私密底牌（{{ draft.agents.length }} 位）
                </span>
                <button
                  type="button"
                  @click="addAgent"
                  class="flex items-center gap-1 px-3 py-1 rounded-full text-xs font-medium bg-[#007AFF] hover:bg-[#0062CC] text-white transition"
                >
                  <Plus class="w-3.5 h-3.5" />
                  <span>添加角色</span>
                </button>
              </div>

              <div
                v-for="(ag, idx) in draft.agents"
                :key="ag.id"
                class="p-3.5 rounded-2xl bg-[#FAFAFC] border border-black/[0.07] space-y-2.5"
              >
                <div class="flex items-center justify-between gap-2">
                  <div class="flex items-center gap-2 flex-1 min-w-0">
                    <span class="text-[11px] font-mono text-slate-400 shrink-0">#{{ idx + 1 }}</span>
                    <input
                      v-model="ag.label"
                      placeholder="角色名称"
                      class="flex-1 min-w-0 rounded-xl border border-black/[0.08] bg-white px-3 py-1.5 text-xs font-semibold outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                    />
                  </div>
                  <div class="flex items-center gap-1.5 shrink-0">
                    <div class="relative">
                      <button
                        type="button"
                        @click="cardLibraryOpenIdx = cardLibraryOpenIdx === idx ? null : idx"
                        class="flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-medium bg-white border border-black/[0.08] text-indigo-600 hover:bg-indigo-50 transition"
                      >
                        <BookUser class="w-3 h-3" />
                        <span>角色库 ({{ characterCards?.length ?? 0 }})</span>
                      </button>
                      <div
                        v-if="cardLibraryOpenIdx === idx"
                        class="absolute right-0 mt-1.5 w-72 rounded-2xl bg-white shadow-xl border border-black/[0.08] p-2 z-30 space-y-1.5 max-h-64 overflow-y-auto"
                      >
                        <div v-if="!characterCards || characterCards.length === 0" class="py-4 text-center text-[11px] text-slate-400">
                          角色库暂无存档，可在《实验结案复盘简报》中一键存入。
                        </div>
                        <div
                          v-for="card in characterCards"
                          :key="card.id"
                          class="p-2 rounded-xl bg-[#FAFAFC] hover:bg-indigo-50/60 border border-black/[0.05] space-y-1"
                        >
                          <div class="flex items-center justify-between gap-2">
                            <button
                              type="button"
                              @click="importCharacterCardToAgent(ag, card)"
                              class="text-xs font-semibold text-[#1D1D1F] hover:text-indigo-700 truncate text-left flex-1"
                            >
                              {{ card.name }}
                            </button>
                            <button
                              type="button"
                              @click="emit('deleteCharacterCard', card.id)"
                              class="p-0.5 text-slate-400 hover:text-rose-600"
                              title="从角色库移除"
                            >
                              <Trash2 class="w-3 h-3" />
                            </button>
                          </div>
                          <p v-if="card.behaviorTraits" class="text-[10.5px] text-slate-500 line-clamp-2">
                            {{ card.behaviorTraits }}
                          </p>
                        </div>
                      </div>
                    </div>
                    <button
                      v-if="draft.agents.length > 2"
                      type="button"
                      @click="removeAgent(idx)"
                      class="p-1.5 rounded-full text-slate-400 hover:text-rose-600 hover:bg-rose-50 transition"
                      title="删除该角色"
                    >
                      <Trash2 class="w-3.5 h-3.5" />
                    </button>
                  </div>
                </div>

                <div>
                  <label class="block text-[11px] font-medium text-slate-500 mb-1">
                    角色身份与私密底牌
                  </label>
                  <textarea
                    v-model="ag.systemPrompt"
                    @input="onCustomPromptEdit"
                    rows="3"
                    placeholder="设定该角色的身份背景、性格底色与隐藏动机（可留空）"
                    class="w-full rounded-xl border border-black/[0.08] bg-white px-3 py-2 text-xs leading-relaxed outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                  ></textarea>
                </div>
              </div>
            </div>

            <div
              v-if="draft.presetId === 'free'"
              class="p-3.5 rounded-2xl bg-emerald-50/80 border border-emerald-200/70 text-xs text-emerald-900 flex gap-2.5"
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

          <!-- Tab 2: Models & API (Decoupled from Role Prompts) -->
          <template v-else-if="activeTab === 'agents'">
            <div class="flex items-center justify-between gap-2">
              <div class="flex flex-wrap items-center gap-1.5">
                <button
                  v-for="(ag, idx) in draft.agents"
                  :key="ag.id"
                  @click="selectedAgentIdx = idx"
                  class="flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs font-medium border transition"
                  :class="
                    selectedAgentIdx === idx
                      ? 'bg-[#1D1D1F] text-white border-[#1D1D1F]'
                      : 'bg-[#F5F5F7] text-slate-700 border-black/[0.05] hover:bg-[#EAEAEF]'
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
                @click="addAgent"
                class="shrink-0 flex items-center gap-1 px-3 py-1.5 rounded-full text-xs font-medium bg-[#007AFF] hover:bg-[#0062CC] text-white transition"
              >
                <Plus class="w-3.5 h-3.5" />
                <span>添加 AI</span>
              </button>
            </div>

            <div v-if="currentAgent" class="space-y-4 pt-2 border-t border-black/[0.05]">
              <div class="flex flex-wrap items-center justify-between gap-2">
                <div class="text-xs text-slate-500">
                  正在配置「{{ currentAgent.label || `智能体 ${selectedAgentIdx + 1}` }}」的模型与接口 · 快速填入端点：
                </div>
                <div class="flex items-center gap-3">
                  <button
                    v-if="selectedAgentIdx > 0"
                    @click="copyCredentialsFromFirst(currentAgent)"
                    class="flex items-center gap-1 text-xs font-medium text-[#007AFF] hover:underline"
                  >
                    <Copy class="w-3.5 h-3.5" />
                    同步第 1 位 Key 与地址
                  </button>
                  <button
                    v-if="draft.agents.length > 2"
                    @click="removeAgent(selectedAgentIdx)"
                    class="flex items-center gap-1 text-xs font-medium text-rose-600 hover:underline"
                  >
                    <Trash2 class="w-3.5 h-3.5" />
                    删除此 AI
                  </button>
                </div>
              </div>

              <div class="flex flex-wrap gap-1.5">
                <button
                  v-for="prov in providerQuickUrls"
                  :key="prov.name"
                  @click="applyProvider(currentAgent, prov.url, prov.model)"
                  class="px-2.5 py-1 rounded-full text-[11px] bg-[#F5F5F7] hover:bg-[#EAEAEF] text-slate-700 border border-black/[0.05] transition"
                >
                  {{ prov.name }}
                </button>
              </div>

              <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <div>
                  <label class="block text-xs font-medium text-slate-600 mb-1">角色名称</label>
                  <input
                    v-model="currentAgent.label"
                    class="w-full rounded-xl border border-black/[0.08] bg-[#FAFAFC] px-3 py-2 text-xs outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                  />
                </div>
                <div>
                  <label class="block text-xs font-medium text-slate-600 mb-1">Model Name</label>
                  <input
                    v-model="currentAgent.modelName"
                    placeholder="如 deepseek-chat / qwen-plus"
                    class="w-full rounded-xl border border-black/[0.08] bg-[#FAFAFC] px-3 py-2 text-xs font-mono outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                  />
                </div>
              </div>

              <div>
                <label class="block text-xs font-medium text-slate-600 mb-1">Base URL</label>
                <input
                  v-model="currentAgent.baseUrl"
                  placeholder="https://api.deepseek.com/v1"
                  class="w-full rounded-xl border border-black/[0.08] bg-[#FAFAFC] px-3 py-2 text-xs font-mono outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                />
              </div>

              <div>
                <label class="block text-xs font-medium text-slate-600 mb-1">API Key</label>
                <input
                  v-model="currentAgent.apiKey"
                  type="password"
                  placeholder="sk-..."
                  class="w-full rounded-xl border border-black/[0.08] bg-[#FAFAFC] px-3 py-2 text-xs font-mono outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                />
              </div>

              <div>
                <div class="flex items-center justify-between text-xs mb-1">
                  <span class="font-medium text-slate-600">Temperature（温度）</span>
                  <span class="font-mono font-semibold text-[#1D1D1F]">
                    {{ Number(currentAgent.temperature).toFixed(2) }}
                  </span>
                </div>
                <input
                  v-model.number="currentAgent.temperature"
                  type="range"
                  min="0"
                  max="1.5"
                  step="0.05"
                  class="w-full accent-[#007AFF]"
                />
              </div>
            </div>
          </template>

          <!-- Observer C Config Tab -->
          <template v-else-if="activeTab === 'observer'">
            <div class="flex items-center justify-between">
              <div class="text-xs text-slate-500">快速填入常用兼容端点：</div>
              <button
                @click="copyCredentialsFromFirst(draft.nodeC)"
                class="flex items-center gap-1 text-xs font-medium text-[#007AFF] hover:underline"
              >
                <Copy class="w-3.5 h-3.5" />
                同步第 1 位 AI 的 Base URL 与 API Key
              </button>
            </div>

            <div class="flex flex-wrap gap-1.5">
              <button
                v-for="prov in providerQuickUrls"
                :key="prov.name"
                @click="applyProvider(draft.nodeC, prov.url, prov.model)"
                class="px-2.5 py-1 rounded-full text-[11px] bg-[#F5F5F7] hover:bg-[#EAEAEF] text-slate-700 border border-black/[0.05] transition"
              >
                {{ prov.name }}
              </button>
            </div>

            <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div>
                <label class="block text-xs font-medium text-slate-600 mb-1">角色名称</label>
                <input
                  v-model="draft.nodeC.label"
                  class="w-full rounded-xl border border-black/[0.08] bg-[#FAFAFC] px-3 py-2 text-xs outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                />
              </div>
              <div>
                <label class="block text-xs font-medium text-slate-600 mb-1">Model Name</label>
                <input
                  v-model="draft.nodeC.modelName"
                  placeholder="建议使用 Flash/Turbo 级高速模型"
                  class="w-full rounded-xl border border-black/[0.08] bg-[#FAFAFC] px-3 py-2 text-xs font-mono outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                />
              </div>
            </div>

            <div>
              <label class="block text-xs font-medium text-slate-600 mb-1">Base URL</label>
              <input
                v-model="draft.nodeC.baseUrl"
                class="w-full rounded-xl border border-black/[0.08] bg-[#FAFAFC] px-3 py-2 text-xs font-mono outline-none focus:ring-2 focus:ring-[#007AFF]/30"
              />
            </div>

            <div>
              <label class="block text-xs font-medium text-slate-600 mb-1">API Key</label>
              <input
                v-model="draft.nodeC.apiKey"
                type="password"
                placeholder="sk-..."
                class="w-full rounded-xl border border-black/[0.08] bg-[#FAFAFC] px-3 py-2 text-xs font-mono outline-none focus:ring-2 focus:ring-[#007AFF]/30"
              />
            </div>

            <div>
              <div class="flex items-center justify-between text-xs mb-1">
                <span class="font-medium text-slate-600">Temperature（温度）</span>
                <span class="font-mono font-semibold text-[#1D1D1F]">
                  {{ Number(draft.nodeC.temperature).toFixed(2) }}
                </span>
              </div>
              <input
                v-model.number="draft.nodeC.temperature"
                type="range"
                min="0"
                max="1.5"
                step="0.05"
                class="w-full accent-[#007AFF]"
              />
            </div>

            <div>
              <label class="block text-xs font-medium text-slate-600 mb-1">
                观察员 System Prompt
              </label>
              <textarea
                v-model="draft.nodeC.systemPrompt"
                rows="4"
                class="w-full rounded-2xl border border-black/[0.08] bg-[#FAFAFC] px-3.5 py-2.5 text-xs leading-relaxed outline-none focus:ring-2 focus:ring-[#007AFF]/30"
              ></textarea>
            </div>
          </template>

          <!-- Safety & Scheduling Tab -->
          <template v-else>
            <div class="space-y-4">
              <!-- Collapsible Core Engine Prompt (Global across all experiments) -->
              <div class="rounded-2xl bg-[#FAFAFC] border border-black/[0.07] overflow-hidden">
                <button
                  type="button"
                  @click="corePromptOpen = !corePromptOpen"
                  class="w-full px-4 py-3 flex items-center justify-between gap-2 text-left hover:bg-slate-100/60 transition"
                >
                  <div class="min-w-0">
                    <div class="flex flex-wrap items-center gap-2">
                      <span class="text-xs font-semibold text-[#1D1D1F]">
                        系统底层演戏总纲
                      </span>
                      <span class="px-1.5 py-0.5 rounded-md bg-slate-200/70 text-[10px] font-medium text-slate-600">
                        跨所有实验生效 · 自由漫谈自动跳过
                      </span>
                    </div>
                  </div>
                  <component
                    :is="corePromptOpen ? ChevronDown : ChevronRight"
                    class="w-4 h-4 text-slate-400 shrink-0"
                  />
                </button>
                <div v-if="corePromptOpen" class="px-4 pb-3.5 pt-1 space-y-2.5 border-t border-black/[0.05]">
                  <textarea
                    v-model="corePromptDraft"
                    rows="4"
                    placeholder="输入全局底层演戏法则..."
                    class="w-full rounded-xl border border-black/[0.08] bg-white px-3 py-2 text-xs leading-relaxed outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                  ></textarea>
                  <div class="flex items-center justify-end gap-2">
                    <button
                      type="button"
                      @click="resetCoreEnginePrompt"
                      :disabled="corePromptSaving"
                      class="flex items-center gap-1 px-3 py-1 rounded-full text-[11px] font-medium bg-white border border-black/[0.08] text-slate-600 hover:bg-slate-100 transition disabled:opacity-50"
                    >
                      <RotateCcw class="w-3 h-3" />
                      <span>恢复默认</span>
                    </button>
                    <button
                      type="button"
                      @click="saveCoreEnginePrompt"
                      :disabled="corePromptSaving"
                      class="flex items-center gap-1 px-3.5 py-1 rounded-full text-[11px] font-medium bg-[#1D1D1F] hover:bg-black text-white transition disabled:opacity-50"
                    >
                      <Save class="w-3 h-3" />
                      <span>保存总纲</span>
                    </button>
                  </div>
                </div>
              </div>

              <div>
                <div class="flex items-center justify-between text-xs mb-1">
                  <span class="font-medium text-slate-700">观察员 C 触发周期（轮）</span>
                  <span class="font-semibold tabular-nums">每 {{ draft.observerInterval }} 轮</span>
                </div>
                <input
                  v-model.number="draft.observerInterval"
                  type="range"
                  min="2"
                  max="30"
                  step="1"
                  class="w-full accent-[#007AFF]"
                />
                <p class="text-[11px] text-slate-400 mt-1">
                  每当主对话推进满 N 轮，后台异步唤醒观察员生成结构化手记，不阻塞主对话流。
                </p>
              </div>

              <div>
                <div class="flex items-center justify-between text-xs mb-1">
                  <span class="font-medium text-slate-700">回合呼吸停顿时间（秒）</span>
                  <span class="font-semibold tabular-nums">{{ Number(draft.pauseSeconds).toFixed(1) }} s</span>
                </div>
                <input
                  v-model.number="draft.pauseSeconds"
                  type="range"
                  min="0.5"
                  max="10"
                  step="0.5"
                  class="w-full accent-[#007AFF]"
                />
                <p class="text-[11px] text-slate-400 mt-1">
                  每轮 AI 发言完毕后的观察缓冲间隔，防止请求频率过快或来不及阅读。
                </p>
              </div>

              <div>
                <label class="block text-xs font-medium text-slate-700 mb-1">
                  最大连续运行轮数熔断上限
                </label>
                <input
                  v-model.number="draft.maxConsecutiveTurns"
                  type="number"
                  min="1"
                  max="1000"
                  class="w-40 rounded-xl border border-black/[0.08] bg-[#FAFAFC] px-3 py-2 text-xs font-mono outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                />
                <p class="text-[11px] text-slate-400 mt-1">
                  单次点击「开始演化」后最多连续推进该轮数即自动暂停，避免后台忘关导致 API 额度耗尽。
                </p>
              </div>

              <div class="pt-3 border-t border-black/[0.05] space-y-4">
                <div>
                  <div class="flex flex-wrap items-center justify-between gap-2 text-xs mb-1.5">
                    <span class="font-medium text-slate-700">单局最大 Token 预算熔断（0 = 不限制）</span>
                    <div class="flex items-center gap-1">
                      <button
                        v-for="presetVal in [0, 50000, 150000, 500000]"
                        :key="presetVal"
                        type="button"
                        @click="draft.tokenBudget = presetVal"
                        class="px-2 py-0.5 rounded-full text-[10.5px] border transition"
                        :class="
                          (draft.tokenBudget ?? 0) === presetVal
                            ? 'bg-[#1D1D1F] text-white border-[#1D1D1F]'
                            : 'bg-[#F5F5F7] text-slate-600 border-black/[0.05] hover:bg-[#EAEAEF]'
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
                    class="w-48 rounded-xl border border-black/[0.08] bg-[#FAFAFC] px-3 py-2 text-xs font-mono outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                  />
                  <p class="text-[11px] text-slate-400 mt-1">
                    实验累计消耗触达此预算红线时，引擎立即自动暂停并弹出熔断提示。
                  </p>
                </div>

                <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
                  <div>
                    <label class="block text-xs font-medium text-slate-700 mb-1">
                      模型上下文预算窗口（Tokens）
                    </label>
                    <input
                      v-model.number="draft.contextWindowTokens"
                      type="number"
                      min="2048"
                      step="4096"
                      class="w-full rounded-xl border border-black/[0.08] bg-[#FAFAFC] px-3 py-2 text-xs font-mono outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                    />
                  </div>
                  <div>
                    <div class="flex items-center justify-between text-xs mb-1">
                      <span class="font-medium text-slate-700">自动记忆折叠阈值</span>
                      <span class="font-semibold tabular-nums">
                        {{ draft.compactionThresholdPct ?? 80 }}%
                      </span>
                    </div>
                    <input
                      v-model.number="draft.compactionThresholdPct"
                      type="range"
                      min="50"
                      max="95"
                      step="5"
                      class="w-full accent-[#007AFF] mt-2"
                    />
                  </div>
                </div>
                <p class="text-[11px] text-slate-400">
                  当估算上下文占用达到阈值时，后台自动将早期对话浓缩为「历史纪要」，完整保留最近 6~8 轮细节，支持超长轮次不断档演化。
                </p>
              </div>

              <div class="pt-3 border-t border-black/[0.05] space-y-3">
                <div class="flex items-center justify-between">
                  <div>
                    <div class="text-xs font-medium text-slate-700">启用实验室共享白板工具</div>
                    <div class="text-[11px] text-slate-400">
                      将白板全文实时注入 System Prompt，并向 AI 提供 update_board 工具以共同记录共识与线索
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    v-model="draft.enableBoardTools"
                    class="w-4 h-4 accent-[#007AFF] cursor-pointer"
                  />
                </div>

                <div v-if="draft.enableBoardTools" class="flex items-center justify-between">
                  <div>
                    <div class="text-xs font-medium text-slate-700">白板修改权限</div>
                    <div class="text-[11px] text-slate-400">
                      开启审批时，AI 调用 update_board 需等待造物主批准后才会生效
                    </div>
                  </div>
                  <div class="flex p-0.5 rounded-full bg-[#F2F2F5] border border-black/[0.05]">
                    <button
                      type="button"
                      @click="draft.boardApprovalMode = 'auto'"
                      class="px-3 py-1 rounded-full text-[11px] font-medium transition"
                      :class="
                        draft.boardApprovalMode !== 'approval' && draft.boardApprovalMode !== 'guardian'
                          ? 'bg-[#1D1D1F] text-white'
                          : 'text-slate-600'
                      "
                    >
                      自动放行
                    </button>
                    <button
                      type="button"
                      @click="draft.boardApprovalMode = 'guardian'"
                      class="px-3 py-1 rounded-full text-[11px] font-medium transition"
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
                      class="px-3 py-1 rounded-full text-[11px] font-medium transition"
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
              <div class="pt-4 border-t border-black/[0.06] space-y-3">
                <div class="flex items-center justify-between">
                  <span class="text-xs font-semibold text-[#1D1D1F]">实验进阶规则与优化</span>
                  <span class="text-[10.5px] text-slate-400">按需开启 · 随时可调</span>
                </div>

                <div class="rounded-2xl bg-[#FAFAFC] border border-black/[0.06] divide-y divide-black/[0.05]">
                  <label class="flex items-center justify-between gap-3 px-3.5 py-2.5 cursor-pointer">
                    <div class="min-w-0">
                      <div class="text-xs font-medium text-slate-700">前缀缓存优化</div>
                      <div class="text-[11px] text-slate-400">
                        固定人设置顶以复用平台缓存折扣，动态状态后置兼顾记忆准度
                      </div>
                    </div>
                    <input
                      type="checkbox"
                      v-model="draft.enablePrefixCacheOpt"
                      class="w-4 h-4 accent-[#007AFF] cursor-pointer shrink-0"
                    />
                  </label>

                  <label class="flex items-center justify-between gap-3 px-3.5 py-2.5 cursor-pointer">
                    <div class="min-w-0">
                      <div class="text-xs font-medium text-slate-700">记忆压缩防循环保护</div>
                      <div class="text-[11px] text-slate-400">
                        若记忆窗口过小导致连续频繁压缩，自动暂停并提醒，防止额度异常消耗
                      </div>
                    </div>
                    <input
                      type="checkbox"
                      v-model="draft.enableCompactionGuard"
                      class="w-4 h-4 accent-[#007AFF] cursor-pointer shrink-0"
                    />
                  </label>

                  <label class="flex items-center justify-between gap-3 px-3.5 py-2.5 cursor-pointer">
                    <div class="min-w-0">
                      <div class="text-xs font-medium text-slate-700">角色私密备忘</div>
                      <div class="text-[11px] text-slate-400">
                        为每位角色维护一份仅自己可见的私密笔记，用于记录怀疑与计划，避免写入公共白板泄密
                      </div>
                    </div>
                    <input
                      type="checkbox"
                      v-model="draft.enablePrivateNotes"
                      class="w-4 h-4 accent-[#007AFF] cursor-pointer shrink-0"
                    />
                  </label>

                  <label class="flex items-center justify-between gap-3 px-3.5 py-2.5 cursor-pointer">
                    <div class="min-w-0">
                      <div class="text-xs font-medium text-slate-700">多人智能点名对话</div>
                      <div class="text-[11px] text-slate-400">
                        3人及以上群聊时，优先由被点名的角色接话，避免机械按顺序轮询
                      </div>
                    </div>
                    <input
                      type="checkbox"
                      v-model="draft.enableSmartSelector"
                      class="w-4 h-4 accent-[#007AFF] cursor-pointer shrink-0"
                    />
                  </label>

                  <div class="px-3.5 py-2.5 space-y-2.5">
                    <label class="flex items-center justify-between gap-3 cursor-pointer">
                      <div class="min-w-0">
                        <div class="text-xs font-medium text-slate-700">按冲突强度调节推理深度</div>
                        <div class="text-[11px] text-slate-400">
                          平淡对话时降低思考深度以节省开销；遇到冲突、突发事件或终局阶段自动恢复全速推理
                        </div>
                      </div>
                      <input
                        type="checkbox"
                        v-model="draft.enableAdaptiveReasoning"
                        class="w-4 h-4 accent-[#007AFF] cursor-pointer shrink-0"
                      />
                    </label>

                    <div
                      v-if="draft.enableAdaptiveReasoning"
                      class="flex flex-wrap items-center justify-between gap-2 pl-2.5 py-1.5 rounded-xl bg-white border border-black/[0.06]"
                    >
                      <span class="text-[11px] font-medium text-slate-600">
                        低张力档位（reasoning_effort）
                      </span>
                      <div class="flex p-0.5 rounded-full bg-[#F2F2F5] border border-black/[0.04]">
                        <button
                          v-for="tier in [
                            { id: 'minimal', label: 'minimal' },
                            { id: 'low', label: 'low (默认)' },
                            { id: 'medium', label: 'medium' },
                          ]"
                          :key="tier.id"
                          type="button"
                          @click="draft.adaptiveLowReasoningEffort = tier.id"
                          class="px-2.5 py-0.5 rounded-full text-[10.5px] font-mono font-medium transition"
                          :class="
                            (draft.adaptiveLowReasoningEffort ?? 'low') === tier.id
                              ? 'bg-[#1D1D1F] text-white'
                              : 'text-slate-600 hover:text-[#1D1D1F]'
                          "
                        >
                          {{ tier.label }}
                        </button>
                      </div>
                    </div>
                  </div>

                  <label class="flex items-center justify-between gap-3 px-3.5 py-2.5 cursor-pointer">
                    <div class="min-w-0">
                      <div class="text-xs font-medium text-slate-700">周期直觉反思提示</div>
                      <div class="text-[11px] text-slate-400">
                        观察员每次分析时，顺带为角色生成一句直觉提示，推动剧情自主演进
                      </div>
                    </div>
                    <input
                      type="checkbox"
                      v-model="draft.enablePeriodicReflection"
                      class="w-4 h-4 accent-[#007AFF] cursor-pointer shrink-0"
                    />
                  </label>

                  <div class="px-3.5 py-2.5 space-y-2.5">
                    <label class="flex items-center justify-between gap-3 cursor-pointer">
                      <div class="min-w-0">
                        <div class="text-xs font-medium text-slate-700">三幕式节奏与自然结局</div>
                        <div class="text-[11px] text-slate-400">
                          按设定的目标总轮数把控剧情推进节奏，临近尾声时引导角色收束伏笔并自然落幕
                        </div>
                      </div>
                      <input
                        type="checkbox"
                        v-model="draft.enableNarrativeArc"
                        class="w-4 h-4 accent-[#007AFF] cursor-pointer shrink-0"
                      />
                    </label>

                    <div
                      v-if="draft.enableNarrativeArc"
                      class="flex flex-wrap items-center justify-between gap-2 pl-2.5 py-1.5 rounded-xl bg-white border border-black/[0.06]"
                    >
                      <span class="text-[11px] font-medium text-slate-600">
                        目标总轮数（默认 30 轮）
                      </span>
                      <input
                        v-model.number="draft.targetTotalTurns"
                        type="number"
                        min="5"
                        max="500"
                        class="w-24 rounded-lg border border-black/[0.08] bg-[#FAFAFC] px-2.5 py-1 text-xs font-mono outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                      />
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </template>
        </div>

    <template #footer>
      <div v-if="draft" class="shrink-0 px-5 md:px-6 py-4 border-t border-black/[0.06] flex items-center justify-between gap-2.5 bg-[#FAFAFC]">
        <div class="flex items-center gap-1.5 sm:hidden">
          <button
            type="button"
            @click="triggerImportJson"
            class="px-3 py-1.5 rounded-full text-[11px] font-medium bg-white border border-black/[0.08] text-slate-700"
          >
            导入
          </button>
          <button
            type="button"
            @click="exportConfigJson"
            class="px-3 py-1.5 rounded-full text-[11px] font-medium bg-white border border-black/[0.08] text-slate-700"
          >
            导出
          </button>
        </div>
        <div class="flex items-center gap-2.5 ml-auto">
          <button
            type="button"
            @click="emit('close')"
            class="px-4 py-2 rounded-full text-xs font-medium text-slate-600 hover:bg-slate-200/70 transition-colors"
          >
            取消
          </button>
          <button
            type="button"
            @click="handleSave"
            class="px-5 py-2 rounded-full text-xs font-medium bg-[#1D1D1F] hover:bg-black text-white shadow-xs transition-colors"
          >
            保存配置
          </button>
        </div>
      </div>
    </template>
  </AdaptiveModal>
</template>
