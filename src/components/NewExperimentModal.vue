<script setup lang="ts">
import { ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { X, Plus, Trash2, Sparkles, Loader2, BookUser, Save } from "lucide-vue-next";
import AdaptiveModal from "./ui/AdaptiveModal.vue";
import type {
  CharacterCard,
  ExperimentConfig,
  GeneratedScenario,
  NodeConfig,
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

const ideaInput = ref("");
const generatingScenario = ref(false);
const ideaError = ref<string | null>(null);
const activeCardPickerIdx = ref<number | null>(null);

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
    customAgents.value = baseAgents;
    customGlobalPrompt.value = "";
    customInitialTopic.value = "";
    return;
  }

  customGlobalPrompt.value = p.globalSystemPrompt ?? "";
  customInitialTopic.value = p.initialTopic ?? "";

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
      activeCardPickerIdx.value = null;
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
    title.value = res.title;
    customGlobalPrompt.value = res.globalSystemPrompt;
    customInitialTopic.value = res.initialTopic;
    customInitialBoard.value = res.initialBoard;
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

function applyCardToAgent(idx: number, card: CharacterCard) {
  const target = customAgents.value[idx];
  if (!target) return;
  target.label = card.name;
  const parts = [card.basePrompt.trim()];
  if (card.behaviorTraits.trim()) {
    parts.push(`【跨世界博弈特质】\n${card.behaviorTraits.trim()}`);
  }
  target.systemPrompt = parts.filter(Boolean).join("\n\n");
  activeCardPickerIdx.value = null;
}

function selectPreset(p: PresetTemplate) {
  selectedPresetId.value = p.id;
  const cleanName = p.name.replace(/（.*?）/g, "");
  title.value = `${cleanName} #${Math.floor(10 + Math.random() * 90)}`;
  hydrateFromPreset(p);
}

function addModalAgent() {
  const first = customAgents.value[0];
  const nextNum = customAgents.value.length + 1;
  customAgents.value.push({
    id: `ai-${Date.now().toString(36)}-${nextNum}`,
    label: `智能体 ${nextNum}`,
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
  const agents = customAgents.value.map((a, idx) => ({
    label: a.label.trim() || `智能体 ${idx + 1}`,
    systemPrompt: a.systemPrompt,
  }));
  const updatedPreset: PresetTemplate = {
    ...preset,
    globalSystemPrompt: customGlobalPrompt.value,
    initialTopic: customInitialTopic.value.trim(),
    agentPrompts: agents,
    nodeALabel: agents[0]?.label || "智能体 1",
    nodeAPrompt: agents[0]?.systemPrompt || "",
    nodeBLabel: agents[1]?.label || "智能体 2",
    nodeBPrompt: agents[1]?.systemPrompt || "",
  };
  emit("savePreset", updatedPreset);
}

function handleCreate() {
  const preset =
    props.presets.find((p) => p.id === selectedPresetId.value) ?? props.presets[0];
  const base: ExperimentConfig = props.currentConfig
    ? JSON.parse(JSON.stringify(props.currentConfig))
    : {
        presetId: preset.id,
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
  base.globalSystemPrompt = customGlobalPrompt.value;
  base.initialTopic = customInitialTopic.value.trim();
  base.initialBoard = customInitialBoard.value.trim();
  base.enableNarrativeArc = enableNarrativeArc.value;
  base.targetTotalTurns = Math.max(5, Math.round(targetTotalTurns.value || 30));
  base.agents = customAgents.value.map((a, idx) => ({
    ...a,
    label: a.label.trim() || `智能体 ${idx + 1}`,
  }));

  if (syncToPreset.value && preset) {
    const updatedPreset: PresetTemplate = {
      ...preset,
      globalSystemPrompt: customGlobalPrompt.value,
      initialTopic: customInitialTopic.value.trim(),
      agentPrompts: base.agents.map((a) => ({
        label: a.label,
        systemPrompt: a.systemPrompt,
      })),
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

  emit("create", title.value.trim() || "未命名生态实验", base);
  emit("close");
}
</script>

<template>
  <AdaptiveModal
    :open="open"
    title="新建 AI 生态实验"
    description="左侧选择场景预设，或顶部输入一句话由 AI 一键生成剧本"
    max-width-class="max-w-4xl"
    @close="emit('close')"
  >
    <template #header>
      <div class="flex items-center justify-between px-5 sm:px-6 py-3.5 sm:py-4 border-b border-black/[0.06] shrink-0">
        <div>
          <h2 class="text-base font-semibold text-[#1D1D1F]">新建 AI 生态实验</h2>
          <p class="text-[11px] text-slate-400 mt-0.5">左侧选择场景预设，或顶部输入一句话由 AI 一键生成剧本</p>
        </div>
        <button
          @click="emit('close')"
          class="p-1.5 rounded-full hover:bg-slate-100 text-slate-500"
        >
          <X class="w-4 h-4" />
        </button>
      </div>
    </template>

        <!-- One-Sentence AI Scenario Generator Bar -->
        <div class="px-6 py-2.5 border-b border-black/[0.06] bg-gradient-to-r from-blue-50/55 via-[#FAFAFC] to-violet-50/55 shrink-0 space-y-1.5">
          <div class="flex items-center gap-2">
            <div class="flex-1 flex items-center gap-2 rounded-2xl bg-white border border-black/[0.08] px-3 py-1.5 focus-within:ring-2 focus-within:ring-[#007AFF]/30">
              <Sparkles class="w-3.5 h-3.5 text-[#007AFF] shrink-0" />
              <input
                v-model="ideaInput"
                @keydown.enter.prevent="generateFromIdea"
                placeholder="一句话生成剧本：例如「暴风雪山庄里三个互相怀疑对方是时间旅行者的乘客」"
                class="flex-1 bg-transparent text-xs text-[#1D1D1F] placeholder:text-slate-400 outline-none"
              />
            </div>
            <button
              type="button"
              @click="generateFromIdea"
              :disabled="!ideaInput.trim() || generatingScenario"
              class="shrink-0 flex items-center gap-1.5 px-3.5 py-1.5 rounded-full text-xs font-medium bg-[#1D1D1F] hover:bg-black text-white transition disabled:opacity-45"
            >
              <Loader2 v-if="generatingScenario" class="w-3.5 h-3.5 animate-spin" />
              <Sparkles v-else class="w-3.5 h-3.5" />
              <span>{{ generatingScenario ? "编剧中..." : "AI 捏剧" }}</span>
            </button>
          </div>
          <p v-if="ideaError" class="text-[11px] text-rose-600 px-1">{{ ideaError }}</p>
        </div>

        <div class="flex-1 min-h-0 flex flex-col md:flex-row overflow-hidden">
          <!-- Left Column: Title + Preset Selector -->
          <div class="w-full md:w-[270px] lg:w-[290px] shrink-0 border-b md:border-b-0 md:border-r border-black/[0.06] bg-[#FAFAFC] p-4 space-y-3 overflow-y-auto max-h-[36vh] md:max-h-none">
            <div>
              <label class="block text-[11px] font-medium text-slate-600 mb-1">实验标题</label>
              <input
                v-model="title"
                class="w-full rounded-xl border border-black/[0.08] bg-white px-3 py-1.5 text-sm md:text-xs font-medium outline-none focus:ring-2 focus:ring-[#007AFF]/30"
              />
            </div>

            <div class="space-y-1.5">
              <label class="block text-[11px] font-medium text-slate-600">场景与性格预设 ({{ presets.length }})</label>
              <div
                v-for="p in presets"
                :key="p.id"
                @click="selectPreset(p)"
                class="group cursor-pointer w-full text-left p-2.5 rounded-xl border transition"
                :class="
                  selectedPresetId === p.id
                    ? 'border-[#007AFF] bg-blue-50/70 shadow-xs'
                    : 'border-black/[0.06] hover:border-black/20 bg-white'
                "
              >
                <div class="flex items-center justify-between gap-1">
                  <span class="text-xs font-semibold text-[#1D1D1F] truncate">{{ p.name }}</span>
                  <button
                    v-if="!p.isBuiltin"
                    type="button"
                    @click.stop="handleDeletePresetItem(p)"
                    class="p-1 rounded-md text-slate-400 hover:text-rose-600 hover:bg-rose-50 transition shrink-0"
                    title="删除此自定义预设"
                  >
                    <Trash2 class="w-3 h-3" />
                  </button>
                </div>
                <div class="text-[10.5px] text-[#007AFF]/80 font-medium mt-0.5 truncate">
                  {{ formatPresetCast(p) }}
                </div>
                <p class="text-[10.5px] text-slate-500 mt-0.5 line-clamp-2 leading-relaxed">
                  {{ p.description }}
                </p>
              </div>
            </div>
          </div>

          <!-- Right Column: Prompt & Agent Editor -->
          <div class="flex-1 min-w-0 overflow-y-auto p-5 space-y-3.5 bg-white">
            <div class="flex flex-wrap items-center justify-between gap-2">
              <span class="text-xs font-semibold text-[#1D1D1F]">
                剧本设定与角色底牌（{{ customAgents.length }} 位）
              </span>
              <div class="flex items-center gap-3">
                <button
                  v-if="selectedPresetId !== 'free' && selectedPresetId !== 'custom'"
                  type="button"
                  @click="overwriteSelectedPreset"
                  class="flex items-center gap-1 text-[11px] font-medium text-slate-600 hover:text-[#007AFF] transition"
                  title="将右侧修改直接覆盖保存回当前选中的预设模板"
                >
                  <Save class="w-3 h-3" />
                  <span>覆盖保存修改</span>
                </button>
                <button
                  @click="addModalAgent"
                  class="flex items-center gap-1 text-xs font-medium text-[#007AFF] hover:underline"
                >
                  <Plus class="w-3.5 h-3.5" />
                  添加角色
                </button>
              </div>
            </div>

            <div>
              <div class="flex items-center justify-between gap-2 mb-1">
                <label class="text-[11px] font-medium text-slate-600">
                  本局场景与文风规则
                </label>
                <span class="text-[10.5px] text-slate-400">对本局所有角色生效</span>
              </div>
              <textarea
                v-model="customGlobalPrompt"
                rows="3"
                placeholder="例如：场景背景、文风规范、单次发言字数限制等（可留空）"
                class="w-full rounded-xl border border-black/[0.08] bg-[#FAFAFC] px-3 py-2 text-sm md:text-xs leading-relaxed outline-none focus:ring-2 focus:ring-[#007AFF]/30"
              ></textarea>
            </div>

            <div class="space-y-2.5">
              <div
                v-for="(ag, idx) in customAgents"
                :key="ag.id"
                class="p-3 rounded-2xl bg-[#FAFAFC] border border-black/[0.06] space-y-2"
              >
                <div class="flex items-center justify-between gap-2">
                  <input
                    v-model="ag.label"
                    placeholder="角色名称"
                    class="flex-1 rounded-lg border border-black/[0.08] bg-white px-2.5 py-1 text-sm md:text-xs font-medium outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                  />
                  <div v-if="characterCards && characterCards.length > 0" class="relative">
                    <button
                      type="button"
                      @click="activeCardPickerIdx = activeCardPickerIdx === idx ? null : idx"
                      class="flex items-center gap-1 px-2 py-1 rounded-lg text-[11px] font-medium bg-white border border-black/[0.07] text-indigo-600 hover:bg-indigo-50 transition"
                      title="从灵魂角色库导入人设"
                    >
                      <BookUser class="w-3 h-3" />
                      <span>角色库</span>
                    </button>
                    <div
                      v-if="activeCardPickerIdx === idx"
                      class="absolute right-0 mt-1 w-60 rounded-xl bg-white shadow-xl border border-black/[0.08] p-1.5 z-20 max-h-48 overflow-y-auto space-y-1"
                    >
                      <button
                        v-for="card in characterCards"
                        :key="card.id"
                        type="button"
                        @click="applyCardToAgent(idx, card)"
                        class="w-full text-left px-2.5 py-1.5 rounded-lg hover:bg-indigo-50/70 transition"
                      >
                        <div class="text-xs font-medium text-[#1D1D1F] truncate">{{ card.name }}</div>
                        <div v-if="card.behaviorTraits" class="text-[10px] text-slate-400 truncate">
                          {{ card.behaviorTraits }}
                        </div>
                      </button>
                    </div>
                  </div>
                  <button
                    v-if="customAgents.length > 2"
                    @click="removeModalAgent(idx)"
                    class="p-1 rounded-lg text-slate-400 hover:text-rose-600 hover:bg-rose-50 transition"
                  >
                    <Trash2 class="w-3.5 h-3.5" />
                  </button>
                </div>
                <div>
                  <label class="block text-[10.5px] font-medium text-slate-500 mb-1">
                    角色身份与私密底牌
                  </label>
                  <textarea
                    v-model="ag.systemPrompt"
                    rows="3"
                    placeholder="设定该角色的身份背景、性格底色与隐藏动机（可留空）"
                    class="w-full rounded-lg border border-black/[0.08] bg-white px-2.5 py-1.5 text-sm md:text-xs leading-relaxed outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                  ></textarea>
                </div>
              </div>
            </div>

            <div>
              <label class="block text-[11px] font-medium text-slate-600 mb-1">
                第 1 轮开场引子（可选，留空则无引子直接开聊）
              </label>
              <textarea
                v-model="customInitialTopic"
                rows="2"
                placeholder="留空则不设开场引子"
                class="w-full rounded-xl border border-black/[0.08] bg-[#FAFAFC] px-3 py-1.5 text-sm md:text-xs leading-relaxed outline-none focus:ring-2 focus:ring-[#007AFF]/30"
              ></textarea>
            </div>

            <div v-if="customInitialBoard.trim().length > 0">
              <label class="block text-[11px] font-medium text-slate-600 mb-1">
                初始共享白板线索（全员开局可见）
              </label>
              <textarea
                v-model="customInitialBoard"
                rows="3"
                class="w-full rounded-xl border border-black/[0.08] bg-[#FAFAFC] px-3 py-1.5 text-xs font-mono leading-relaxed outline-none focus:ring-2 focus:ring-[#007AFF]/30"
              ></textarea>
            </div>

            <div class="p-3 rounded-2xl bg-[#FAFAFC] border border-black/[0.06] flex flex-wrap items-center justify-between gap-2">
              <label class="flex items-center gap-2 text-xs font-medium text-slate-700 cursor-pointer select-none">
                <input type="checkbox" v-model="enableNarrativeArc" class="rounded accent-[#007AFF]" />
                <span>三幕式节奏与自然结局</span>
              </label>
              <div v-if="enableNarrativeArc" class="flex items-center gap-1.5 text-[11px] text-slate-600">
                <span>目标总轮数</span>
                <input
                  v-model.number="targetTotalTurns"
                  type="number"
                  min="5"
                  max="500"
                  class="w-16 rounded-lg border border-black/[0.08] bg-white px-2 py-0.5 text-xs font-mono outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                />
                <span>轮</span>
              </div>
            </div>

            <div class="pt-1 space-y-2 border-t border-black/[0.05]">
              <label class="flex items-center gap-2 text-[11px] text-slate-600 cursor-pointer select-none">
                <input type="checkbox" v-model="syncToPreset" class="rounded accent-[#007AFF]" />
                <span>创建实验时，同时将以上修改覆盖保存回所选预设模板</span>
              </label>
              <label class="flex items-center gap-2 text-[11px] text-slate-600 cursor-pointer select-none">
                <input type="checkbox" v-model="inheritKeys" class="rounded accent-[#007AFF]" />
                <span>复用当前实验的 API Base URL、Key 与模型配置</span>
              </label>
            </div>
          </div>
        </div>

    <template #footer>
      <div class="flex justify-end gap-2 px-5 sm:px-6 py-3.5 border-t border-black/[0.06] bg-[#FAFAFC] shrink-0">
        <button
          @click="emit('close')"
          class="px-4 py-2 rounded-full text-xs font-medium text-slate-600 hover:bg-slate-100"
        >
          取消
        </button>
        <button
          @click="handleCreate"
          class="px-5 py-2 rounded-full text-xs font-medium bg-[#1D1D1F] hover:bg-black text-white"
        >
          创建并进入观察箱
        </button>
      </div>
    </template>
  </AdaptiveModal>
</template>
