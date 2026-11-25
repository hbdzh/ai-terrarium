<script setup lang="ts">
import { ref, watch } from "vue";
import { X, Plus, Trash2 } from "lucide-vue-next";
import type { ExperimentConfig, NodeConfig, PresetTemplate } from "../types";

const props = defineProps<{
  open: boolean;
  presets: PresetTemplate[];
  currentConfig: ExperimentConfig | null;
}>();

const emit = defineEmits<{
  close: [];
  create: [title: string, config: ExperimentConfig];
  savePreset: [preset: PresetTemplate];
}>();

const title = ref("");
const selectedPresetId = ref("free");
const inheritKeys = ref(true);

const customAgents = ref<NodeConfig[]>([]);
const customGlobalPrompt = ref("");
const customInitialTopic = ref("");
const syncToPreset = ref(false);

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
      hydrateFromPreset(first);
    }
  }
);

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
  <Teleport to="body">
    <div v-if="open" class="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div
        @click="emit('close')"
        class="fixed inset-0 bg-black/25 backdrop-blur-[2px]"
      ></div>

      <div
        class="relative w-full max-w-xl max-h-[88vh] flex flex-col rounded-3xl bg-white shadow-2xl border border-black/[0.08] p-6 z-10"
      >
        <div class="flex items-center justify-between mb-4 shrink-0">
          <h2 class="text-base font-semibold text-[#1D1D1F]">新建 AI 生态实验</h2>
          <button
            @click="emit('close')"
            class="p-1.5 rounded-full hover:bg-slate-100 text-slate-500"
          >
            <X class="w-4 h-4" />
          </button>
        </div>

        <div class="flex-1 overflow-y-auto pr-1 space-y-4">
          <div>
            <label class="block text-xs font-medium text-slate-600 mb-1">实验标题</label>
            <input
              v-model="title"
              class="w-full rounded-xl border border-black/[0.08] bg-[#FAFAFC] px-3.5 py-2 text-sm outline-none focus:ring-2 focus:ring-[#007AFF]/30"
            />
          </div>

          <div>
            <label class="block text-xs font-medium text-slate-600 mb-1.5">选择模式与性格模板</label>
            <div class="space-y-2">
              <button
                v-for="p in presets"
                :key="p.id"
                @click="selectPreset(p)"
                class="w-full text-left p-3 rounded-2xl border transition"
                :class="
                  selectedPresetId === p.id
                    ? 'border-[#007AFF] bg-blue-50/50'
                    : 'border-black/[0.07] hover:border-black/20 bg-[#FAFAFC]'
                "
              >
                <div class="flex items-center justify-between">
                  <span class="text-xs font-semibold text-[#1D1D1F]">{{ p.name }}</span>
                  <span class="text-[11px] text-slate-500">
                    {{
                      p.id === "free"
                        ? "零提示词 · 无目标"
                        : p.id === "custom"
                          ? "支持多个 AI 自由设定"
                          : `${p.nodeALabel} vs ${p.nodeBLabel}`
                    }}
                  </span>
                </div>
                <p class="text-[11px] text-slate-500 mt-0.5">{{ p.description }}</p>
              </button>
            </div>
          </div>

          <!-- Inline Prompt & Agent Editor (available for ALL presets) -->
          <div class="p-4 rounded-2xl bg-[#FAFAFC] border border-black/[0.07] space-y-3">
            <div class="flex items-center justify-between">
              <span class="text-xs font-semibold text-[#1D1D1F]">
                场景提示词与 AI 节点（{{ customAgents.length }} 个）
              </span>
              <button
                @click="addModalAgent"
                class="flex items-center gap-1 text-xs font-medium text-[#007AFF] hover:underline"
              >
                <Plus class="w-3.5 h-3.5" />
                添加 AI
              </button>
            </div>

            <div>
              <label class="block text-[11px] font-medium text-slate-600 mb-1">
                全局系统通用提示词（对本实验所有 AI 生效，可微调或留空）
              </label>
              <textarea
                v-model="customGlobalPrompt"
                rows="2"
                placeholder="例如：统一规定辩论规则、世界观前提或输出语言风格..."
                class="w-full rounded-xl border border-black/[0.08] bg-white px-3 py-1.5 text-sm md:text-xs outline-none focus:ring-2 focus:ring-[#007AFF]/30"
              ></textarea>
            </div>

            <div
              v-for="(ag, idx) in customAgents"
              :key="ag.id"
              class="p-3 rounded-xl bg-white border border-black/[0.06] space-y-2"
            >
              <div class="flex items-center justify-between gap-2">
                <input
                  v-model="ag.label"
                  placeholder="角色名称"
                  class="flex-1 rounded-lg border border-black/[0.08] bg-[#FAFAFC] px-2.5 py-1 text-sm md:text-xs font-medium outline-none focus:ring-2 focus:ring-[#007AFF]/30"
                />
                <button
                  v-if="customAgents.length > 2"
                  @click="removeModalAgent(idx)"
                  class="p-1 rounded-lg text-slate-400 hover:text-rose-600 hover:bg-rose-50 transition"
                >
                  <Trash2 class="w-3.5 h-3.5" />
                </button>
              </div>
              <textarea
                v-model="ag.systemPrompt"
                rows="2"
                placeholder="设定该 AI 的专属 System Prompt（可留空）"
                class="w-full rounded-lg border border-black/[0.08] bg-[#FAFAFC] px-2.5 py-1.5 text-sm md:text-xs outline-none focus:ring-2 focus:ring-[#007AFF]/30"
              ></textarea>
            </div>

            <div>
              <label class="block text-[11px] font-medium text-slate-600 mb-1">
                开场引子（可选，留空则无引子直接开聊）
              </label>
              <input
                v-model="customInitialTopic"
                placeholder="留空则不设开场引子"
                class="w-full rounded-xl border border-black/[0.08] bg-white px-3 py-1.5 text-sm md:text-xs outline-none focus:ring-2 focus:ring-[#007AFF]/30"
              />
            </div>

            <label class="flex items-center gap-2 text-[11px] text-slate-600 cursor-pointer select-none pt-1">
              <input type="checkbox" v-model="syncToPreset" class="rounded accent-[#007AFF]" />
              <span>同时将以上全局与各角色提示词修改保存回所选预设模板</span>
            </label>
          </div>

          <label class="flex items-center gap-2 text-xs text-slate-600 cursor-pointer select-none">
            <input type="checkbox" v-model="inheritKeys" class="rounded accent-[#007AFF]" />
            <span>复用当前实验的 API Base URL、Key 与模型配置</span>
          </label>
        </div>

        <div class="flex justify-end gap-2 pt-4 mt-2 border-t border-black/[0.05] shrink-0">
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
      </div>
    </div>
  </Teleport>
</template>
