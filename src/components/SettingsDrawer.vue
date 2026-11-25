<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  X,
  Copy,
  Sparkles,
  ShieldAlert,
  Plus,
  Trash2,
  Download,
  Upload,
  Pencil,
  RotateCcw,
  Save,
} from "lucide-vue-next";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import type { ExperimentConfig, NodeConfig, PresetTemplate } from "../types";

const props = defineProps<{
  open: boolean;
  config: ExperimentConfig | null;
  presets: PresetTemplate[];
}>();

const emit = defineEmits<{
  close: [];
  save: [config: ExperimentConfig];
  savePreset: [preset: PresetTemplate];
  resetPreset: [presetId: string];
  deletePreset: [presetId: string];
  notify: [message: string];
}>();

const draft = ref<ExperimentConfig | null>(null);
const activeTab = ref<"presets" | "agents" | "observer" | "safety">("presets");
const selectedAgentIdx = ref(0);
const fileInputRef = ref<HTMLInputElement | null>(null);
const editingPreset = ref<PresetTemplate | null>(null);

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
  () => [props.open, props.config],
  () => {
    if (props.open && props.config) {
      const cloned: ExperimentConfig = JSON.parse(JSON.stringify(props.config));
      cloned.globalSystemPrompt = cloned.globalSystemPrompt ?? "";
      cloned.tokenBudget = cloned.tokenBudget ?? 0;
      cloned.contextWindowTokens = cloned.contextWindowTokens ?? 32000;
      cloned.compactionThresholdPct = cloned.compactionThresholdPct ?? 80;
      cloned.boardApprovalMode = cloned.boardApprovalMode ?? "auto";
      cloned.enableBoardTools = cloned.enableBoardTools ?? true;
      cloned.initialBoard = cloned.initialBoard ?? "";
      draft.value = cloned;
      selectedAgentIdx.value = 0;
      editingPreset.value = null;
    }
  },
  { immediate: true }
);

const currentAgent = computed<NodeConfig | null>(() => {
  if (!draft.value || draft.value.agents.length === 0) return null;
  const idx = Math.min(selectedAgentIdx.value, draft.value.agents.length - 1);
  return draft.value.agents[idx];
});

function getPresetAgents(preset: PresetTemplate) {
  if (Array.isArray(preset.agentPrompts) && preset.agentPrompts.length > 0) {
    return preset.agentPrompts;
  }
  return [
    { label: preset.nodeALabel || "智能体 1", systemPrompt: preset.nodeAPrompt || "" },
    { label: preset.nodeBLabel || "智能体 2", systemPrompt: preset.nodeBPrompt || "" },
  ];
}

function applyPreset(preset: PresetTemplate) {
  if (!draft.value) return;
  draft.value.presetId = preset.id;
  draft.value.globalSystemPrompt = preset.globalSystemPrompt ?? "";
  draft.value.initialTopic = preset.initialTopic ?? "";

  if (preset.id === "custom") {
    return;
  }

  const presetAgents = getPresetAgents(preset);
  const templateBase = draft.value.agents[0];
  while (draft.value.agents.length < presetAgents.length) {
    const nextNum = draft.value.agents.length + 1;
    draft.value.agents.push({
      id: `ai-${Date.now().toString(36)}-${nextNum}`,
      label: `智能体 ${nextNum}`,
      baseUrl: templateBase?.baseUrl || "https://api.deepseek.com/v1",
      apiKey: templateBase?.apiKey || "",
      modelName: templateBase?.modelName || "deepseek-chat",
      temperature: 0.9,
      systemPrompt: "",
    });
  }
  if (preset.id !== "free" && draft.value.agents.length > presetAgents.length) {
    draft.value.agents = draft.value.agents.slice(0, presetAgents.length);
  }
  presetAgents.forEach((ap, idx) => {
    if (draft.value!.agents[idx]) {
      draft.value!.agents[idx].label = ap.label;
      draft.value!.agents[idx].systemPrompt = ap.systemPrompt;
    }
  });
}

function startEditPreset(preset: PresetTemplate) {
  const cloned: PresetTemplate = JSON.parse(JSON.stringify(preset));
  cloned.globalSystemPrompt = cloned.globalSystemPrompt ?? "";
  cloned.initialTopic = cloned.initialTopic ?? "";
  cloned.agentPrompts = JSON.parse(JSON.stringify(getPresetAgents(preset)));
  editingPreset.value = cloned;
}

function startNewPreset() {
  const currentAgents = draft.value?.agents ?? [];
  editingPreset.value = {
    id: "",
    name: "新建自定义场景",
    description: "自定义全局通用提示词与多智能体角色人设",
    globalSystemPrompt: draft.value?.globalSystemPrompt ?? "",
    initialTopic: draft.value?.initialTopic ?? "",
    agentPrompts:
      currentAgents.length > 0
        ? currentAgents.map((a) => ({ label: a.label, systemPrompt: a.systemPrompt }))
        : [
            { label: "智能体 1", systemPrompt: "" },
            { label: "智能体 2", systemPrompt: "" },
          ],
    isBuiltin: false,
    nodeALabel: currentAgents[0]?.label || "智能体 1",
    nodeAPrompt: currentAgents[0]?.systemPrompt || "",
    nodeBLabel: currentAgents[1]?.label || "智能体 2",
    nodeBPrompt: currentAgents[1]?.systemPrompt || "",
  };
}

function addPresetAgent() {
  if (!editingPreset.value) return;
  const n = editingPreset.value.agentPrompts.length + 1;
  editingPreset.value.agentPrompts.push({
    label: `智能体 ${n}`,
    systemPrompt: "",
  });
}

function removePresetAgent(idx: number) {
  if (!editingPreset.value || editingPreset.value.agentPrompts.length <= 2) return;
  editingPreset.value.agentPrompts.splice(idx, 1);
}

function commitPresetEdit() {
  if (!editingPreset.value) return;
  const p = editingPreset.value;
  p.name = p.name.trim() || "未命名场景";
  p.nodeALabel = p.agentPrompts[0]?.label || "智能体 1";
  p.nodeAPrompt = p.agentPrompts[0]?.systemPrompt || "";
  p.nodeBLabel = p.agentPrompts[1]?.label || "智能体 2";
  p.nodeBPrompt = p.agentPrompts[1]?.systemPrompt || "";
  emit("savePreset", p);
  if (draft.value && (draft.value.presetId === p.id || !p.id)) {
    applyPreset(p);
  }
  editingPreset.value = null;
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
      raw?.boardApprovalMode === "approval" ? "approval" : (fallback.boardApprovalMode ?? "auto"),
    enableBoardTools:
      typeof raw?.enableBoardTools === "boolean"
        ? raw.enableBoardTools
        : (fallback.enableBoardTools ?? true),
    initialBoard:
      typeof raw?.initialBoard === "string"
        ? raw.initialBoard
        : (fallback.initialBoard ?? ""),
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
  <Teleport to="body">
    <div v-if="open && draft" class="fixed inset-0 z-50 flex justify-end">
      <div
        @click="emit('close')"
        class="fixed inset-0 bg-black/20 backdrop-blur-[2px] transition-opacity"
      ></div>

      <div
        class="relative w-full max-w-2xl bg-white h-full shadow-2xl border-l border-black/[0.08] flex flex-col z-10"
      >
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
              @click="triggerImportJson"
              class="flex items-center gap-1 px-2.5 py-1.5 rounded-full text-[11px] font-medium bg-[#F5F5F7] hover:bg-[#EAEAEF] text-slate-700 border border-black/[0.05] transition"
              title="从 JSON 文件导入模型配置与设置"
            >
              <Upload class="w-3.5 h-3.5" />
              <span class="hidden sm:inline">导入 JSON</span>
            </button>
            <button
              @click="exportConfigJson"
              class="flex items-center gap-1 px-2.5 py-1.5 rounded-full text-[11px] font-medium bg-[#F5F5F7] hover:bg-[#EAEAEF] text-slate-700 border border-black/[0.05] transition"
              title="导出当前所有模型配置与设置为 JSON 文件"
            >
              <Download class="w-3.5 h-3.5" />
              <span class="hidden sm:inline">导出 JSON</span>
            </button>
            <button
              @click="emit('close')"
              class="p-2 rounded-full hover:bg-slate-100 text-slate-500 transition"
            >
              <X class="w-4 h-4" />
            </button>
          </div>
        </div>

        <!-- Tabs -->
        <div class="px-5 md:px-6 pt-3 pb-2 border-b border-black/[0.05] flex gap-1.5 overflow-x-auto">
          <button
            v-for="tab in [
              { id: 'presets', label: '模式与性格模板' },
              { id: 'agents', label: `AI 智能体 (${draft.agents.length})` },
              { id: 'observer', label: '观察员 C' },
              { id: 'safety', label: '调度与安全阀' },
            ]"
            :key="tab.id"
            @click="activeTab = tab.id as any"
            class="px-3.5 py-1.5 rounded-full text-xs font-medium transition shrink-0"
            :class="
              activeTab === tab.id
                ? 'bg-[#1D1D1F] text-white'
                : 'bg-[#F5F5F7] text-slate-600 hover:bg-[#EAEAEF]'
            "
          >
            {{ tab.label }}
          </button>
        </div>

        <!-- Body -->
        <div class="flex-1 overflow-y-auto p-5 md:p-6 space-y-5">
          <!-- Presets Tab -->
          <template v-if="activeTab === 'presets'">
            <!-- Global System Prompt for current experiment -->
            <div class="p-4 rounded-2xl bg-[#FAFAFC] border border-black/[0.07] space-y-2">
              <div class="flex items-center justify-between gap-2">
                <label class="text-xs font-semibold text-[#1D1D1F]">
                  全局系统通用提示词（对当前实验所有 AI 生效）
                </label>
                <span class="text-[11px] text-slate-400">注入于各 AI 专属人设之前</span>
              </div>
              <textarea
                v-model="draft.globalSystemPrompt"
                rows="3"
                placeholder="例如：全程保持高信息密度，禁止客套；或统一设定世界观规则、语言风格、输出格式等（可留空）"
                class="w-full rounded-xl border border-black/[0.08] bg-white px-3 py-2 text-sm md:text-xs leading-relaxed outline-none focus:ring-2 focus:ring-[#007AFF]/30"
              ></textarea>
            </div>

            <!-- Preset Template Editor Drawer-in-Tab -->
            <div
              v-if="editingPreset"
              class="p-4 rounded-2xl bg-blue-50/45 border border-[#007AFF]/35 space-y-3"
            >
              <div class="flex items-center justify-between gap-2">
                <span class="text-xs font-semibold text-[#0055B3]">
                  {{ editingPreset.id ? `正在编辑预设模板：${editingPreset.name}` : "新建预设场景模板" }}
                </span>
                <button
                  type="button"
                  @click="editingPreset = null"
                  class="text-xs text-slate-500 hover:text-slate-800"
                >
                  取消编辑
                </button>
              </div>

              <div class="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
                <div>
                  <label class="block text-[11px] font-medium text-slate-600 mb-1">预设名称</label>
                  <input
                    v-model="editingPreset.name"
                    class="w-full rounded-xl border border-black/[0.08] bg-white px-3 py-1.5 text-sm md:text-xs outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                  />
                </div>
                <div>
                  <label class="block text-[11px] font-medium text-slate-600 mb-1">预设简介</label>
                  <input
                    v-model="editingPreset.description"
                    class="w-full rounded-xl border border-black/[0.08] bg-white px-3 py-1.5 text-sm md:text-xs outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                  />
                </div>
              </div>

              <div>
                <label class="block text-[11px] font-medium text-slate-600 mb-1">
                  该预设下的全局系统通用提示词
                </label>
                <textarea
                  v-model="editingPreset.globalSystemPrompt"
                  rows="2"
                  placeholder="此预设对所有 AI 通用的底层规则/风格提示词（可留空）"
                  class="w-full rounded-xl border border-black/[0.08] bg-white px-3 py-2 text-sm md:text-xs leading-relaxed outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                ></textarea>
              </div>

              <div class="space-y-2">
                <div class="flex items-center justify-between">
                  <span class="text-[11px] font-medium text-slate-600">
                    该预设下的各 AI 专属提示词 ({{ editingPreset.agentPrompts.length }} 位)
                  </span>
                  <button
                    type="button"
                    @click="addPresetAgent"
                    class="flex items-center gap-1 text-[11px] font-medium text-[#007AFF] hover:underline"
                  >
                    <Plus class="w-3 h-3" />
                    添加角色
                  </button>
                </div>
                <div
                  v-for="(ap, idx) in editingPreset.agentPrompts"
                  :key="idx"
                  class="p-2.5 rounded-xl bg-white border border-black/[0.06] space-y-1.5"
                >
                  <div class="flex items-center gap-2">
                    <input
                      v-model="ap.label"
                      placeholder="角色名称"
                      class="flex-1 rounded-lg border border-black/[0.08] bg-[#FAFAFC] px-2.5 py-1 text-sm md:text-xs font-medium outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                    />
                    <button
                      v-if="editingPreset.agentPrompts.length > 2"
                      type="button"
                      @click="removePresetAgent(idx)"
                      class="p-1 rounded-lg text-slate-400 hover:text-rose-600 hover:bg-rose-50"
                    >
                      <Trash2 class="w-3.5 h-3.5" />
                    </button>
                  </div>
                  <textarea
                    v-model="ap.systemPrompt"
                    rows="2"
                    placeholder="该 AI 的专属提示词（可留空）"
                    class="w-full rounded-lg border border-black/[0.08] bg-[#FAFAFC] px-2.5 py-1.5 text-sm md:text-xs leading-relaxed outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                  ></textarea>
                </div>
              </div>

              <div>
                <label class="block text-[11px] font-medium text-slate-600 mb-1">默认开场引子（可选）</label>
                <input
                  v-model="editingPreset.initialTopic"
                  placeholder="留空则无开场引子"
                  class="w-full rounded-xl border border-black/[0.08] bg-white px-3 py-1.5 text-sm md:text-xs outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                />
              </div>

              <div class="flex justify-end gap-2 pt-1">
                <button
                  type="button"
                  @click="editingPreset = null"
                  class="px-3 py-1.5 rounded-full text-xs text-slate-600 bg-white border border-black/[0.08] hover:bg-slate-100"
                >
                  取消
                </button>
                <button
                  type="button"
                  @click="commitPresetEdit"
                  class="flex items-center gap-1 px-4 py-1.5 rounded-full text-xs font-medium bg-[#007AFF] hover:bg-[#0062CC] text-white"
                >
                  <Save class="w-3.5 h-3.5" />
                  保存预设并同步应用
                </button>
              </div>
            </div>

            <div class="space-y-2.5">
              <div class="flex items-center justify-between gap-2">
                <div class="text-xs font-semibold text-slate-700 flex items-center gap-1.5">
                  <Sparkles class="w-3.5 h-3.5 text-[#007AFF]" />
                  <span>选择或管理对话场景预设</span>
                </div>
                <button
                  type="button"
                  @click="startNewPreset"
                  class="flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-medium bg-[#F5F5F7] hover:bg-[#EAEAEF] text-[#007AFF] border border-black/[0.05] transition"
                >
                  <Plus class="w-3.5 h-3.5" />
                  存为新预设
                </button>
              </div>

              <div class="grid gap-2.5">
                <div
                  v-for="p in presets"
                  :key="p.id"
                  @click="applyPreset(p)"
                  class="cursor-pointer p-4 rounded-2xl border transition"
                  :class="
                    draft.presetId === p.id
                      ? 'border-[#007AFF] bg-blue-50/50'
                      : 'border-black/[0.07] hover:border-black/20 bg-[#FAFAFC]'
                  "
                >
                  <div class="flex items-center justify-between gap-2 mb-1">
                    <span class="text-sm font-semibold text-[#1D1D1F]">{{ p.name }}</span>
                    <div class="flex items-center gap-1 shrink-0" @click.stop>
                      <button
                        type="button"
                        @click="startEditPreset(p)"
                        class="flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium bg-white hover:bg-slate-100 text-slate-600 border border-black/[0.07] transition"
                        title="修改该预设下的全局通用提示词与各 AI 提示词"
                      >
                        <Pencil class="w-3 h-3" />
                        编辑提示词
                      </button>
                      <button
                        v-if="p.isBuiltin"
                        type="button"
                        @click="emit('resetPreset', p.id)"
                        class="p-1 rounded-full bg-white hover:bg-slate-100 text-slate-400 hover:text-slate-700 border border-black/[0.06] transition"
                        title="恢复内置默认提示词"
                      >
                        <RotateCcw class="w-3 h-3" />
                      </button>
                      <button
                        v-else
                        type="button"
                        @click="emit('deletePreset', p.id)"
                        class="p-1 rounded-full bg-white hover:bg-rose-50 text-slate-400 hover:text-rose-600 border border-black/[0.06] transition"
                        title="删除此自定义预设"
                      >
                        <Trash2 class="w-3 h-3" />
                      </button>
                    </div>
                  </div>
                  <p class="text-xs text-slate-500">{{ p.description }}</p>
                  <div class="mt-1.5 flex flex-wrap items-center gap-1.5 text-[11px] text-slate-400">
                    <span v-if="p.globalSystemPrompt?.trim()" class="px-2 py-0.5 rounded-md bg-white border border-black/[0.05] text-slate-600">
                      含全局通用提示词
                    </span>
                    <span>
                      {{
                        getPresetAgents(p)
                          .map((a) => a.label)
                          .join(" vs ")
                      }}
                    </span>
                  </div>
                </div>
              </div>
            </div>

            <!-- Inline Custom Editor when Custom mode is active -->
            <div
              v-if="draft.presetId === 'custom'"
              class="p-4 rounded-2xl bg-[#FAFAFC] border border-black/[0.07] space-y-3"
            >
              <div class="flex items-center justify-between">
                <span class="text-xs font-semibold text-[#1D1D1F]">
                  自定义参与角色（当前 {{ draft.agents.length }} 个 AI）
                </span>
                <button
                  @click="addAgent"
                  class="flex items-center gap-1 text-[11px] font-medium text-[#007AFF] hover:underline"
                >
                  <Plus class="w-3.5 h-3.5" />
                  添加 AI
                </button>
              </div>
              <div
                v-for="(ag, idx) in draft.agents"
                :key="ag.id"
                class="p-3 rounded-xl bg-white border border-black/[0.06] space-y-2"
              >
                <div class="flex items-center justify-between gap-2">
                  <input
                    v-model="ag.label"
                    placeholder="角色名称"
                    class="flex-1 rounded-lg border border-black/[0.08] bg-[#FAFAFC] px-2.5 py-1 text-xs font-medium outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                  />
                  <button
                    v-if="draft.agents.length > 2"
                    @click="removeAgent(idx)"
                    class="p-1 rounded-lg text-slate-400 hover:text-rose-600 hover:bg-rose-50 transition"
                    title="移除该 AI"
                  >
                    <Trash2 class="w-3.5 h-3.5" />
                  </button>
                </div>
                <textarea
                  v-model="ag.systemPrompt"
                  rows="2"
                  placeholder="自由编写该角色的 System Prompt（可留空）"
                  class="w-full rounded-lg border border-black/[0.08] bg-[#FAFAFC] px-2.5 py-1.5 text-xs leading-relaxed outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                ></textarea>
              </div>
            </div>

            <div>
              <label class="block text-xs font-semibold text-slate-700 mb-1.5">
                第 1 轮开场引子（可选，默认留空不设引子）
              </label>
              <textarea
                v-model="draft.initialTopic"
                rows="2"
                placeholder="留空则不设任何开场引子，让智能体直接开始对话"
                class="w-full rounded-2xl border border-black/[0.08] bg-[#FAFAFC] px-3.5 py-2.5 text-xs leading-relaxed outline-none focus:ring-2 focus:ring-[#007AFF]/30"
              ></textarea>
            </div>

            <div
              v-if="draft.presetId === 'free'"
              class="p-3.5 rounded-2xl bg-emerald-50/80 border border-emerald-200/70 text-xs text-emerald-900 flex gap-2.5"
            >
              <Sparkles class="w-4 h-4 shrink-0 mt-0.5 text-emerald-600" />
              <div>
                <div class="font-semibold mb-0.5">当前为「自由漫谈」零预设模式</div>
                <div class="opacity-85 leading-relaxed">
                  系统不会向模型注入任何性格设定、对话目标或防客套约束，所有 AI 将从完全空白的上下文开始自由轮流对话。
                </div>
              </div>
            </div>

            <div
              v-else
              class="p-3.5 rounded-2xl bg-amber-50/80 border border-amber-200/70 text-xs text-amber-900 flex gap-2.5"
            >
              <ShieldAlert class="w-4 h-4 shrink-0 mt-0.5 text-amber-600" />
              <div>
                <div class="font-semibold mb-0.5">防客套底层约束已启用</div>
                <div class="opacity-85 leading-relaxed">
                  当前模式下，系统会在请求各 AI 的 System Prompt 末尾附加去客套约束（如需完全无约束，请切换到「自由漫谈（零预设）」）。
                </div>
              </div>
            </div>
          </template>

          <!-- Dynamic Multi-Agent Config Tab -->
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
                  正在编辑第 {{ selectedAgentIdx + 1 }} 位智能体 · 快速填入端点：
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

              <div
                v-if="draft.globalSystemPrompt?.trim()"
                class="px-3 py-2 rounded-xl bg-blue-50/70 border border-blue-200/70 text-[11px] text-blue-900"
              >
                <span class="font-semibold">全局系统通用提示词已生效：</span>
                以下角色专属提示词会与全局通用提示词叠加发送给当前 AI。
              </div>

              <div>
                <label class="block text-xs font-medium text-slate-600 mb-1">
                  性格底色 / 该 AI 专属 System Prompt
                </label>
                <textarea
                  v-model="currentAgent.systemPrompt"
                  @input="onCustomPromptEdit"
                  rows="5"
                  placeholder="可留空，或输入自定义 System Prompt"
                  class="w-full rounded-2xl border border-black/[0.08] bg-[#FAFAFC] px-3.5 py-2.5 text-xs leading-relaxed outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                ></textarea>
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
                  <div class="flex items-center justify-between text-xs mb-1">
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
                      向所有 AI 注入 read_board / update_board 工具，实现共同记录设定与线索
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
                    <div class="text-xs font-medium text-slate-700">白板写入审批策略</div>
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
                        draft.boardApprovalMode !== 'approval'
                          ? 'bg-[#1D1D1F] text-white'
                          : 'text-slate-600'
                      "
                    >
                      自动放行
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
                      需造物主审批
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </template>
        </div>

        <!-- Footer -->
        <div class="px-5 md:px-6 py-4 border-t border-black/[0.06] flex items-center justify-between gap-2.5 bg-[#FAFAFC]">
          <div class="flex items-center gap-1.5 sm:hidden">
            <button
              @click="triggerImportJson"
              class="px-3 py-1.5 rounded-full text-[11px] font-medium bg-white border border-black/[0.08] text-slate-700"
            >
              导入
            </button>
            <button
              @click="exportConfigJson"
              class="px-3 py-1.5 rounded-full text-[11px] font-medium bg-white border border-black/[0.08] text-slate-700"
            >
              导出
            </button>
          </div>
          <div class="flex items-center gap-2.5 ml-auto">
            <button
              @click="emit('close')"
              class="px-4 py-2 rounded-full text-xs font-medium text-slate-600 hover:bg-slate-200/70 transition"
            >
              取消
            </button>
            <button
              @click="handleSave"
              class="px-5 py-2 rounded-full text-xs font-medium bg-[#1D1D1F] hover:bg-black text-white shadow-xs transition"
            >
              保存配置
            </button>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>
