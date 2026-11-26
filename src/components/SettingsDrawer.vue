<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  X,
  Download,
  Upload,
  KeyRound,
  Telescope,
  Clock,
  MessageCircle,
  Cpu,
  Sun,
  Check,
} from "lucide-vue-next";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog, save } from "@tauri-apps/plugin-dialog";
import type { ApiProfile, ExperimentConfig, NodeConfig, ThemePreference } from "../types";
import { triggerHaptic } from "../utils/native";
import {
  applyApiProfileToNode,
  cloneNormalizedConfig,
  normalizeThinkingLevel,
  resolveNodeApiProfileId,
  syncConfigWithApiProfiles,
} from "../utils/config";
import { themePreference, setThemePreference } from "../utils/theme";
import { MECHANIC_DESCRIPTIONS } from "../constants/mechanicDescriptions";
import AdaptiveModal from "./ui/AdaptiveModal.vue";
import ModelSelector from "./ui/ModelSelector.vue";
import RangeSlider from "./ui/RangeSlider.vue";
import ToggleSwitch from "./ui/ToggleSwitch.vue";
import {
  DEFAULT_BASE_URL,
  DEFAULT_MODEL_NAME,
  PROVIDER_PRESETS,
  isKeylessFriendlyEndpoint,
} from "../constants/providers";
import { fetchProviderModels, normalizeBaseUrl } from "../utils/modelCache";
import {
  applyTagLocksToMechanics,
  getLockedMechanics,
} from "../constants/experimentTags";

const props = defineProps<{
  open: boolean;
  config: ExperimentConfig | null;
  apiProfiles: ApiProfile[];
}>();

const emit = defineEmits<{
  close: [];
  save: [config: ExperimentConfig];
  saveApiProfiles: [profiles: ApiProfile[]];
  notify: [message: string];
}>();

const draft = ref<ExperimentConfig | null>(null);
const initialSnapshot = ref("");
const profilesDraft = ref<ApiProfile[]>([]);
const initialProfilesSnapshot = ref("");
const activeTab = ref<"api" | "observer" | "schedule" | "behavior" | "resources" | "appearance">("api");
const selectedProfileIdx = ref(0);
const corePromptDraft = ref("");
const corePromptInitial = ref("");
const corePromptSaving = ref(false);
const confirmDiscardOpen = ref(false);

const navTabs = [
  { id: "api", label: "模型与 API", icon: KeyRound },
  { id: "observer", label: "观察与分析", icon: Telescope },
  { id: "schedule", label: "推演与调度", icon: Clock },
  { id: "behavior", label: "对话与行为", icon: MessageCircle },
  { id: "resources", label: "性能与资源", icon: Cpu },
  { id: "appearance", label: "外观", icon: Sun },
] as const;

function providerBadgeMeta(id: string): { letter: string; bg: string; fg: string } {
  switch (id) {
    case "deepseek":
      return { letter: "D", bg: "#ECE8FA", fg: "#6B57B5" };
    case "dashscope":
      return { letter: "Q", bg: "#FAF0E1", fg: "#B97D2E" };
    case "moonshot":
      return { letter: "K", bg: "#EAF0FA", fg: "#4B6EB5" };
    case "volces":
      return { letter: "豆", bg: "#EBE8F7", fg: "#6E5BB0" };
    case "openai":
      return { letter: "O", bg: "#FAF0E1", fg: "#B97D2E" };
    case "gemini":
      return { letter: "G", bg: "#EAF0FA", fg: "#4B6EB5" };
    case "zhipu":
      return { letter: "智", bg: "#EBE8F7", fg: "#6E5BB0" };
    case "minimax":
      return { letter: "M", bg: "#FAF0E1", fg: "#B97D2E" };
    case "openrouter":
      return { letter: "R", bg: "#EAF0FA", fg: "#4B6EB5" };
    default:
      return { letter: "+", bg: "#F2F2F5", fg: "#666B76" };
  }
}

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

const providerSlots = [
  ...PROVIDER_PRESETS.map((p) => ({
    id: p.id,
    name: p.name,
    url: p.url,
    model: p.model,
  })),
  {
    id: "custom",
    name: "自定义",
    url: "",
    model: "",
  },
];

function buildProviderProfiles(existing: ApiProfile[]): ApiProfile[] {
  return providerSlots.map((slot) => {
    const matched =
      existing.find((p) => p.id === slot.id) ??
      (slot.url
        ? existing.find((p) => normalizeBaseUrl(p.baseUrl) === normalizeBaseUrl(slot.url))
        : existing.find(
            (p) => !PROVIDER_PRESETS.some((pr) => normalizeBaseUrl(pr.url) === normalizeBaseUrl(p.baseUrl))
          ));
    return {
      id: slot.id,
      name: slot.name,
      baseUrl: matched?.baseUrl || slot.url,
      apiKey: matched?.apiKey || "",
      modelName: matched?.modelName || slot.model,
    };
  });
}

function configuredProfilesList(list: ApiProfile[]): ApiProfile[] {
  const ready = list.filter(
    (p) => p.apiKey.trim().length > 0 || (p.baseUrl.trim().length > 0 && isKeylessFriendlyEndpoint(p.baseUrl))
  );
  if (ready.length > 0) return ready;
  const current = list[selectedProfileIdx.value] ?? list[0];
  return current ? [current] : [];
}

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen && props.config) {
      const built = buildProviderProfiles(props.apiProfiles);
      profilesDraft.value = built;
      initialProfilesSnapshot.value = JSON.stringify(built);

      const cloned = cloneNormalizedConfig(props.config);
      const activeList = configuredProfilesList(built);
      if (cloned.nodeC) {
        cloned.nodeC.apiProfileId = resolveNodeApiProfileId(cloned.nodeC, activeList);
      }
      draft.value = cloned;
      initialSnapshot.value = JSON.stringify(cloned);

      const firstReadyIdx = built.findIndex((p) => p.apiKey.trim().length > 0);
      selectedProfileIdx.value = firstReadyIdx >= 0 ? firstReadyIdx : 0;
      confirmDiscardOpen.value = false;
      loadCoreEnginePrompt();
    }
  },
  { immediate: true }
);

const isDirty = computed(() => {
  const draftChanged =
    Boolean(draft.value) && JSON.stringify(draft.value) !== initialSnapshot.value;
  const profilesChanged =
    JSON.stringify(profilesDraft.value) !== initialProfilesSnapshot.value;
  const coreChanged = corePromptDraft.value !== corePromptInitial.value;
  return draftChanged || profilesChanged || coreChanged;
});

const currentProfile = computed<ApiProfile | null>(() => {
  if (profilesDraft.value.length === 0) return null;
  const idx = Math.min(selectedProfileIdx.value, profilesDraft.value.length - 1);
  return profilesDraft.value[idx];
});

const selectableProfiles = computed(() => configuredProfilesList(profilesDraft.value));

function maybePrefetchModels(target: Pick<ApiProfile, "baseUrl" | "apiKey">) {
  const url = target.baseUrl.trim();
  if (!url.startsWith("http://") && !url.startsWith("https://")) return;
  if (!target.apiKey.trim() && !isKeylessFriendlyEndpoint(url)) return;
  void fetchProviderModels(url, target.apiKey, false).catch(() => {});
}

function selectObserverProfile(profileId: string) {
  if (!draft.value?.nodeC) return;
  const prof = selectableProfiles.value.find((p) => p.id === profileId);
  if (prof) {
    applyApiProfileToNode(draft.value.nodeC, prof);
  }
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
    emit("notify", `已导入 ${draft.value.agents.length} 个角色配置，点击「保存配置」生效`);
  } catch (err: any) {
    emit("notify", `导入失败：${err?.message ?? "JSON 格式无效"}`);
  }
}

function normalizeImportedConfig(raw: any): ExperimentConfig {
  const fallback = draft.value!;
  const normalizeNode = (n: any, fallbackId: string, fallbackLabel: string): NodeConfig => ({
    id: typeof n?.id === "string" && n.id.trim() ? n.id.trim() : fallbackId,
    label: typeof n?.label === "string" && n.label.trim() ? n.label.trim() : fallbackLabel,
    baseUrl: typeof n?.baseUrl === "string" ? n.baseUrl : DEFAULT_BASE_URL,
    apiKey: typeof n?.apiKey === "string" ? n.apiKey : "",
    modelName: typeof n?.modelName === "string" ? n.modelName : DEFAULT_MODEL_NAME,
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
      normalizeNode(a, `ai-${i + 1}`, `角色 ${i + 1}`)
    );
  } else if (raw?.nodeA || raw?.nodeB) {
    if (raw.nodeA) agents.push(normalizeNode(raw.nodeA, "AiA", "角色 1"));
    if (raw.nodeB) agents.push(normalizeNode(raw.nodeB, "AiB", "角色 2"));
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
      ? normalizeNode(raw.nodeC, "observer", "局势分析")
      : JSON.parse(JSON.stringify(fallback.nodeC)),
  };
}

const lockedMechanics = computed(() => getLockedMechanics(draft.value?.tags ?? []));

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

  const normalizedProfiles: ApiProfile[] = configuredProfilesList(profilesDraft.value).map((p) => ({
    id: p.id,
    name: p.name,
    baseUrl: p.baseUrl.trim() || DEFAULT_BASE_URL,
    apiKey: p.apiKey.trim(),
    modelName: p.modelName.trim() || DEFAULT_MODEL_NAME,
  }));

  const syncedConfig = syncConfigWithApiProfiles(draft.value, normalizedProfiles);
  if (corePromptDraft.value !== corePromptInitial.value) {
    await saveCoreEnginePrompt();
  }
  confirmDiscardOpen.value = false;
  triggerHaptic("confirm");
  emit("saveApiProfiles", normalizedProfiles);
  emit("save", syncedConfig);
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
            v-for="tab in navTabs"
            :key="tab.id"
            type="button"
            @click="triggerHaptic('light'); activeTab = tab.id"
            class="h-8 px-3.5 rounded-full text-[12.5px] font-medium transition-colors shrink-0"
            :class="
              activeTab === tab.id
                ? 'bg-[#242426] text-white'
                : 'bg-[#F2F2F5] text-[#666B76] hover:bg-[#EFEFF2]'
            "
          >
            {{ tab.label }}
          </button>
        </div>
      </div>
    </template>

    <!-- Body: Clean 2-Pane Split on Desktop -->
    <div v-if="draft" class="flex-1 min-h-0 flex overflow-hidden bg-[#F6F6F8] md:bg-white">
      <!-- Desktop Full-Height Left Nav (>= md) -->
      <nav class="hidden md:flex md:w-[216px] shrink-0 flex-col justify-between bg-[#F6F6F8] border-r border-[#ECECF0] p-3 select-none">
        <div class="space-y-4">
          <div class="px-2.5 pt-1.5 flex items-center justify-between">
            <span class="text-[15px] font-semibold text-[#25262A] tracking-tight">设置</span>
          </div>

          <div class="space-y-0.5">
            <button
              v-for="tab in navTabs"
              :key="tab.id"
              type="button"
              @click="activeTab = tab.id"
              class="relative w-full h-9 flex items-center gap-2.5 px-3 rounded-[9px] text-[13px] transition-colors text-left outline-none"
              :class="
                activeTab === tab.id
                  ? 'bg-[#EFEFF2] text-[#25262A] font-medium'
                  : 'text-[#666B76] font-normal hover:bg-black/[0.035] hover:text-[#25262A]'
              "
            >
              <span
                v-if="activeTab === tab.id"
                class="absolute left-1 top-2 bottom-2 w-[2.5px] rounded-full bg-[#7965CC]"
              ></span>
              <component
                :is="tab.icon"
                class="w-4 h-4 shrink-0"
                :class="activeTab === tab.id ? 'text-[#25262A]' : 'text-[#858A95]'"
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
        <!-- Desktop Clean Content Header -->
        <div class="hidden md:flex items-center justify-between px-6 h-13 border-b border-[#ECECF0] shrink-0">
          <h2 class="text-[13.5px] font-semibold text-[#25262A]">
            {{ navTabs.find((t) => t.id === activeTab)?.label ?? "设置" }}
          </h2>
          <button
            type="button"
            @click="requestClose"
            class="w-8 h-8 flex items-center justify-center rounded-lg text-[#858A95] hover:text-[#25262A] hover:bg-[#EFEFF2] transition-colors outline-none"
          >
            <X class="w-4 h-4" />
          </button>
        </div>

        <div class="flex-1 min-h-0 overflow-y-auto px-4 py-5 md:px-7 md:py-6">
          <div class="max-w-[600px] mx-auto space-y-5">
          <!-- Tab 1: 模型与 API (2-Column Provider List + Detail) -->
          <template v-if="activeTab === 'api'">
            <div>
              <h3 class="text-[20px] font-semibold text-[#25262A] tracking-tight">模型与 API</h3>
              <p class="text-[12.5px] text-[#858A95] mt-1">管理模型服务商、默认模型与访问凭据。</p>
            </div>

            <div class="grid grid-cols-1 md:grid-cols-[200px_1fr] gap-5 items-start">
              <!-- Provider List -->
              <div class="space-y-1 md:border-r md:border-[#ECECF0] md:pr-4">
                <div class="px-2 pb-1 text-[11.5px] font-medium text-[#858A95]">
                  服务商 · {{ profilesDraft.length }}
                </div>
                <div class="flex md:flex-col gap-1 overflow-x-auto md:overflow-x-visible pb-1 md:pb-0 no-scrollbar">
                  <button
                    v-for="(prof, idx) in profilesDraft"
                    :key="prof.id"
                    type="button"
                    @click="triggerHaptic('light'); selectedProfileIdx = idx"
                    class="shrink-0 md:w-full flex items-center gap-2.5 px-2.5 py-2 rounded-[9px] text-left transition-colors outline-none"
                    :class="
                      selectedProfileIdx === idx
                        ? 'bg-[#F0EDF9] text-[#25262A] font-medium'
                        : 'hover:bg-[#F5F5F7] text-[#666B76]'
                    "
                  >
                    <span
                      class="w-6 h-6 rounded-[6px] grid place-items-center text-[11.5px] font-semibold shrink-0"
                      :style="{
                        backgroundColor: providerBadgeMeta(prof.id).bg,
                        color: providerBadgeMeta(prof.id).fg,
                      }"
                    >
                      {{ providerBadgeMeta(prof.id).letter }}
                    </span>
                    <span class="flex-1 min-w-0 truncate text-[12.5px]">{{ prof.name }}</span>
                    <span
                      v-if="prof.apiKey.trim().length > 0"
                      class="w-1.5 h-1.5 rounded-full bg-[#25A477] shrink-0"
                      title="已配置 API Key"
                    ></span>
                  </button>
                </div>
              </div>

              <!-- Selected Provider Editor -->
              <div v-if="currentProfile" class="space-y-4">
                <div class="flex items-center gap-3">
                  <span
                    class="w-9 h-9 rounded-[10px] grid place-items-center text-[14px] font-semibold shrink-0"
                    :style="{
                      backgroundColor: providerBadgeMeta(currentProfile.id).bg,
                      color: providerBadgeMeta(currentProfile.id).fg,
                    }"
                  >
                    {{ providerBadgeMeta(currentProfile.id).letter }}
                  </span>
                  <div class="min-w-0">
                    <div class="text-[14.5px] font-semibold text-[#25262A] truncate">
                      {{ currentProfile.name }}
                    </div>
                    <div class="text-[11.5px] text-[#858A95]">
                      {{ currentProfile.apiKey.trim() ? "已配置凭据" : "未配置 API Key" }}
                    </div>
                  </div>
                </div>

                <div>
                  <label class="block text-[12px] font-medium text-[#666B76] mb-1.5">默认模型</label>
                  <ModelSelector
                    v-model="currentProfile.modelName"
                    :base-url="currentProfile.baseUrl"
                    :api-key="currentProfile.apiKey"
                    placeholder="选择或输入模型"
                    @notify="emit('notify', $event)"
                  />
                </div>

                <div>
                  <label class="block text-[12px] font-medium text-[#666B76] mb-1.5">接口地址</label>
                  <input
                    v-model="currentProfile.baseUrl"
                    @blur="maybePrefetchModels(currentProfile)"
                    :placeholder="DEFAULT_BASE_URL"
                    class="w-full h-10 rounded-[9px] border border-[#E5E5EA] bg-[#F5F5F7] focus:bg-white px-3.5 text-[13px] text-[#25262A] placeholder:text-[#969BA5] font-mono outline-none focus:border-[#7965CC] transition"
                  />
                </div>

                <div>
                  <label class="block text-[12px] font-medium text-[#666B76] mb-1.5">API Key</label>
                  <input
                    v-model="currentProfile.apiKey"
                    @blur="maybePrefetchModels(currentProfile)"
                    type="password"
                    placeholder="sk-..."
                    class="w-full h-10 rounded-[9px] border border-[#E5E5EA] bg-[#F5F5F7] focus:bg-white px-3.5 text-[13px] text-[#25262A] placeholder:text-[#969BA5] font-mono outline-none focus:border-[#7965CC] transition"
                  />
                </div>
              </div>
            </div>
          </template>

          <!-- Tab 2: 观察与分析 -->
          <template v-else-if="activeTab === 'observer'">
            <div>
              <h3 class="text-[20px] font-semibold text-[#25262A] tracking-tight">观察与分析</h3>
              <p class="text-[12.5px] text-[#858A95] mt-1">设置观察员，以及推演过程中的自动分析方式。</p>
            </div>

            <div class="space-y-4">
              <div class="grid grid-cols-1 sm:grid-cols-2 gap-3.5">
                <div>
                  <label class="block text-[12px] font-medium text-[#666B76] mb-1.5">显示名称</label>
                  <input
                    v-model="draft.nodeC.label"
                    placeholder="局势分析"
                    class="w-full h-10 rounded-[9px] border border-[#E5E5EA] bg-[#F5F5F7] focus:bg-white px-3.5 text-[13px] text-[#25262A] placeholder:text-[#969BA5] outline-none focus:border-[#7965CC] transition"
                  />
                </div>
                <div>
                  <label class="block text-[12px] font-medium text-[#666B76] mb-1.5">使用的 API 配置</label>
                  <select
                    :value="draft.nodeC.apiProfileId || selectableProfiles[0]?.id"
                    @change="selectObserverProfile(($event.target as HTMLSelectElement).value)"
                    class="w-full h-10 rounded-[9px] border border-[#E5E5EA] bg-[#F5F5F7] focus:bg-white px-3 text-[13px] text-[#25262A] outline-none focus:border-[#7965CC] transition"
                  >
                    <option v-for="prof in selectableProfiles" :key="prof.id" :value="prof.id">
                      {{ prof.name }} ({{ prof.modelName }})
                    </option>
                  </select>
                </div>
              </div>

              <div>
                <div class="flex items-center justify-between text-[13px] mb-1">
                  <span class="font-medium text-[#25262A]">Temperature</span>
                  <span class="font-mono font-medium text-[#25262A] tabular-nums text-[12.5px]">
                    {{ Number(draft.nodeC.temperature).toFixed(1) }}
                  </span>
                </div>
                <p class="text-[11.5px] text-[#858A95] mb-1">观察员生成内容的随机性</p>
                <RangeSlider
                  :model-value="draft.nodeC.temperature"
                  :min="0"
                  :max="1.5"
                  :step="0.05"
                  @update:model-value="draft.nodeC.temperature = $event"
                />
              </div>

              <div>
                <label class="block text-[12px] font-medium text-[#666B76] mb-1.5">分析提示词</label>
                <textarea
                  v-model="draft.nodeC.systemPrompt"
                  rows="4"
                  placeholder="输入局势分析提示词..."
                  class="w-full resize-none rounded-[9px] border border-[#E5E5EA] bg-[#F5F5F7] focus:bg-white px-3.5 py-2.5 text-[13px] text-[#25262A] placeholder:text-[#969BA5] leading-relaxed outline-none focus:border-[#7965CC] transition"
                ></textarea>
              </div>

              <div class="pt-4 border-t border-[#ECECF0] space-y-4">
                <div>
                  <div class="flex items-center justify-between text-[13px]">
                    <span class="font-medium text-[#25262A]">局势分析间隔</span>
                    <span class="font-medium tabular-nums text-[#25262A] text-[12.5px]">{{ draft.observerInterval }} 轮</span>
                  </div>
                  <p class="text-[11.5px] text-[#858A95] mt-0.5 mb-1">每推进 N 轮后台分析一次</p>
                  <RangeSlider
                    :model-value="draft.observerInterval"
                    :min="2"
                    :max="30"
                    :step="1"
                    @update:model-value="draft.observerInterval = $event"
                  />
                </div>

                <div class="flex items-center justify-between gap-3">
                  <div class="min-w-0">
                    <div class="text-[13px] font-medium text-[#25262A]">自动情景推进</div>
                    <div class="text-[11.5px] text-[#858A95] mt-0.5">对话停滞时自动触发场景事件推进剧情。</div>
                  </div>
                  <ToggleSwitch v-model="draft.enableAutoDirector" />
                </div>
              </div>
            </div>
          </template>

          <!-- Tab 3: 推演与调度 -->
          <template v-else-if="activeTab === 'schedule'">
            <div>
              <h3 class="text-[20px] font-semibold text-[#25262A] tracking-tight">推演与调度</h3>
              <p class="text-[12.5px] text-[#858A95] mt-1">控制推演运行节奏、轮次限制及全局规则。</p>
            </div>

            <div class="space-y-4">
              <div>
                <div class="flex items-center justify-between text-[13px]">
                  <span class="font-medium text-[#25262A]">轮次停顿间隔</span>
                  <span class="font-medium tabular-nums text-[#25262A] text-[12.5px]">{{ Number(draft.pauseSeconds).toFixed(1) }} s</span>
                </div>
                <p class="text-[11.5px] text-[#858A95] mt-0.5 mb-1">每轮发言结束后的等待间隔。</p>
                <RangeSlider
                  :model-value="draft.pauseSeconds"
                  :min="0.5"
                  :max="10"
                  :step="0.5"
                  @update:model-value="draft.pauseSeconds = $event"
                />
              </div>

              <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2">
                <div class="min-w-0">
                  <div class="text-[13px] font-medium text-[#25262A]">连续运行上限</div>
                  <div class="text-[11.5px] text-[#858A95] mt-0.5">单次开始推进该轮数后自动暂停。</div>
                </div>
                <input
                  v-model.number="draft.maxConsecutiveTurns"
                  type="number"
                  min="1"
                  max="1000"
                  class="w-full sm:w-28 h-9 rounded-[9px] border border-[#E5E5EA] bg-[#F5F5F7] focus:bg-white px-3 text-[13px] font-mono text-[#25262A] outline-none focus:border-[#7965CC] transition shrink-0"
                />
              </div>

              <div class="pt-4 border-t border-[#ECECF0] space-y-4">
                <div class="text-[12px] font-medium text-[#858A95]">剧情与规则</div>

                <div class="space-y-2">
                  <div class="flex items-center justify-between gap-3">
                    <div class="min-w-0">
                      <div class="text-[13px] font-medium text-[#25262A]">三幕式剧情节奏</div>
                      <div class="text-[11.5px] text-[#858A95] mt-0.5">按目标总轮数控制剧情推进与收尾。</div>
                    </div>
                    <ToggleSwitch v-model="draft.enableNarrativeArc" />
                  </div>
                  <div
                    v-if="draft.enableNarrativeArc"
                    class="flex items-center justify-between gap-2 pl-3 py-1 border-l-2 border-[#E5E5EA]"
                  >
                    <span class="text-[12px] text-[#666B76]">目标总轮数</span>
                    <input
                      v-model.number="draft.targetTotalTurns"
                      type="number"
                      min="5"
                      max="500"
                      class="w-24 h-8 rounded-[8px] border border-[#E5E5EA] bg-[#F5F5F7] px-2.5 text-[12.5px] font-mono text-[#25262A] outline-none focus:border-[#7965CC]"
                    />
                  </div>
                </div>

                <div class="space-y-2">
                  <div class="flex items-center justify-between gap-2">
                    <span class="text-[12.5px] font-medium text-[#25262A]">全局基础规则 · 自由漫谈跳过</span>
                    <button
                      type="button"
                      @click="resetCoreEnginePrompt"
                      :disabled="corePromptSaving"
                      class="text-[11.5px] text-[#666B76] hover:text-[#25262A] transition disabled:opacity-50"
                    >
                      恢复默认
                    </button>
                  </div>
                  <textarea
                    v-model="corePromptDraft"
                    rows="4"
                    placeholder="编辑全局规则..."
                    class="w-full resize-none rounded-[9px] border border-[#E5E5EA] bg-[#F5F5F7] focus:bg-white px-3.5 py-2.5 text-[13px] text-[#25262A] placeholder:text-[#969BA5] leading-relaxed outline-none focus:border-[#7965CC] transition"
                  ></textarea>
                </div>
              </div>
            </div>
          </template>

          <!-- Tab 4: 对话与行为 -->
          <template v-else-if="activeTab === 'behavior'">
            <div>
              <h3 class="text-[20px] font-semibold text-[#25262A] tracking-tight">对话与行为</h3>
              <p class="text-[12.5px] text-[#858A95] mt-1">管理智能体的发言方式、独立记忆与互动行为。</p>
            </div>

            <div class="pt-3 border-t border-[#ECECF0] space-y-4">
              <div class="text-[12px] font-medium text-[#858A95]">角色行为</div>

              <div
                class="flex items-center justify-between gap-3"
                :class="lockedMechanics.enablePrivateNotes ? 'opacity-60' : ''"
              >
                <div class="min-w-0">
                  <div class="text-[13px] font-medium text-[#25262A] flex items-center gap-1.5">
                    <span>角色私密备忘</span>
                    <span v-if="lockedMechanics.enablePrivateNotes" class="px-1.5 py-0.5 rounded bg-amber-50 text-amber-700 text-[10.5px]">
                      由 [{{ lockedMechanics.enablePrivateNotes }}] 锁定
                    </span>
                  </div>
                  <div class="text-[11.5px] text-[#858A95] mt-0.5">允许角色记录仅自己可见的判断与计划。</div>
                </div>
                <ToggleSwitch v-model="draft.enablePrivateNotes" :disabled="Boolean(lockedMechanics.enablePrivateNotes)" />
              </div>

              <div class="flex items-center justify-between gap-3">
                <div class="min-w-0">
                  <div class="text-[13px] font-medium text-[#25262A]">智能指定发言角色</div>
                  <div class="text-[11.5px] text-[#858A95] mt-0.5">根据上下文动态选择下一位发言者。</div>
                </div>
                <ToggleSwitch v-model="draft.enableSmartSelector" />
              </div>

              <div class="flex items-center justify-between gap-3">
                <div class="min-w-0">
                  <div class="text-[13px] font-medium text-[#25262A]">后台直觉提示</div>
                  <div class="text-[11.5px] text-[#858A95] mt-0.5">分析局势时为角色生成简短直觉提示。</div>
                </div>
                <ToggleSwitch v-model="draft.enablePeriodicReflection" />
              </div>

              <div class="space-y-2.5">
                <div class="flex items-center justify-between gap-3">
                  <div class="min-w-0">
                    <div class="text-[13px] font-medium text-[#25262A]">共享白板工具</div>
                    <div class="text-[11.5px] text-[#858A95] mt-0.5">允许角色共同维护一份全局可见的共享信息区。</div>
                  </div>
                  <ToggleSwitch v-model="draft.enableBoardTools" />
                </div>
                <div v-if="draft.enableBoardTools" class="flex flex-wrap items-center justify-between gap-3 pl-3 border-l-2 border-[#E5E5EA]">
                  <span class="text-[12px] text-[#666B76]">白板写入审核</span>
                  <div class="flex p-0.5 rounded-full bg-[#F2F2F5]">
                    <button
                      type="button"
                      @click="draft.boardApprovalMode = 'auto'"
                      class="px-3 py-1 rounded-full text-[11.5px] font-medium transition"
                      :class="
                        draft.boardApprovalMode !== 'approval' && draft.boardApprovalMode !== 'guardian'
                          ? 'bg-white text-[#25262A] shadow-2xs'
                          : 'text-[#666B76]'
                      "
                    >
                      自动通过
                    </button>
                    <button
                      type="button"
                      @click="draft.boardApprovalMode = 'guardian'"
                      class="px-3 py-1 rounded-full text-[11.5px] font-medium transition"
                      :class="
                        draft.boardApprovalMode === 'guardian'
                          ? 'bg-[#7965CC] text-white'
                          : 'text-[#666B76]'
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
                          ? 'bg-[#C38B38] text-white'
                          : 'text-[#666B76]'
                      "
                    >
                      人工审核
                    </button>
                  </div>
                </div>
              </div>
            </div>

            <div class="pt-4 border-t border-[#ECECF0] space-y-4">
              <div class="text-[12px] font-medium text-[#858A95]">对话质感</div>

              <div class="flex items-center justify-between gap-3">
                <div class="min-w-0">
                  <div class="text-[13px] font-medium text-[#25262A]">允许跳过发言 [PASS]</div>
                  <div class="text-[11.5px] text-[#858A95] mt-0.5">允许角色在适当时机跳过本轮发言。</div>
                </div>
                <ToggleSwitch v-model="draft.allowPassTurn" />
              </div>

              <div
                class="flex items-center justify-between gap-3"
                :class="lockedMechanics.randomMoodDice ? 'opacity-60' : ''"
              >
                <div class="min-w-0">
                  <div class="text-[13px] font-medium text-[#25262A]">随机情绪波动</div>
                  <div class="text-[11.5px] text-[#858A95] mt-0.5">{{ MECHANIC_DESCRIPTIONS.randomMoodDice.onEffect }}</div>
                </div>
                <ToggleSwitch v-model="draft.randomMoodDice" :disabled="Boolean(lockedMechanics.randomMoodDice)" />
              </div>

              <div
                class="flex items-center justify-between gap-3"
                :class="lockedMechanics.intuitiveFastReply ? 'opacity-60' : ''"
              >
                <div class="min-w-0">
                  <div class="text-[13px] font-medium text-[#25262A]">快反应模式</div>
                  <div class="text-[11.5px] text-[#858A95] mt-0.5">{{ MECHANIC_DESCRIPTIONS.intuitiveFastReply.onEffect }}</div>
                </div>
                <ToggleSwitch v-model="draft.intuitiveFastReply" :disabled="Boolean(lockedMechanics.intuitiveFastReply)" />
              </div>
            </div>
          </template>

          <!-- Tab 5: 性能与资源 -->
          <template v-else-if="activeTab === 'resources'">
            <div>
              <h3 class="text-[20px] font-semibold text-[#25262A] tracking-tight">性能与资源</h3>
              <p class="text-[12.5px] text-[#858A95] mt-1">控制推理深度、缓存策略与 Token 使用上限。</p>
            </div>

            <div class="space-y-4">
              <div class="flex items-center justify-between gap-3">
                <div class="min-w-0">
                  <div class="text-[13px] font-medium text-[#25262A]">提示词缓存优化</div>
                  <div class="text-[11.5px] text-[#858A95] mt-0.5">固定角色设定顺序以复用平台缓存折扣。</div>
                </div>
                <ToggleSwitch v-model="draft.enablePrefixCacheOpt" />
              </div>

              <div class="flex items-center justify-between gap-3">
                <div class="min-w-0">
                  <div class="text-[13px] font-medium text-[#25262A]">防循环自动暂停</div>
                  <div class="text-[11.5px] text-[#858A95] mt-0.5">上下文过小导致频繁压缩时自动暂停。</div>
                </div>
                <ToggleSwitch v-model="draft.enableCompactionGuard" />
              </div>

              <div class="space-y-2">
                <div class="flex items-center justify-between gap-3">
                  <div class="min-w-0">
                    <div class="text-[13px] font-medium text-[#25262A]">动态推理深度</div>
                    <div class="text-[11.5px] text-[#858A95] mt-0.5">平淡对话降低开销，冲突或收尾恢复完整推理。</div>
                  </div>
                  <ToggleSwitch v-model="draft.enableAdaptiveReasoning" />
                </div>
                <div
                  v-if="draft.enableAdaptiveReasoning"
                  class="flex flex-wrap items-center justify-between gap-2 pl-3 py-1.5 border-l-2 border-[#E5E5EA]"
                >
                  <span class="text-[12px] text-[#666B76]">低冲突档位</span>
                  <div class="flex p-0.5 rounded-[8px] bg-[#F2F2F5]">
                    <button
                      v-for="tier in [
                        { id: 'minimal', label: 'minimal' },
                        { id: 'low', label: 'low' },
                        { id: 'medium', label: 'medium' },
                      ]"
                      :key="tier.id"
                      type="button"
                      @click="draft.adaptiveLowReasoningEffort = tier.id"
                      class="px-2.5 py-1 rounded-[6px] text-[11px] font-mono font-medium transition"
                      :class="
                        (draft.adaptiveLowReasoningEffort ?? 'low') === tier.id
                          ? 'bg-white text-[#25262A] shadow-2xs'
                          : 'text-[#666B76] hover:text-[#25262A]'
                      "
                    >
                      {{ tier.label }}
                    </button>
                  </div>
                </div>
              </div>

              <div class="pt-4 border-t border-[#ECECF0] space-y-4">
                <div class="text-[12px] font-medium text-[#858A95]">资源上限</div>

                <div class="space-y-2">
                  <div class="flex flex-wrap items-center justify-between gap-2">
                    <div>
                      <div class="text-[13px] font-medium text-[#25262A]">单局 Token 上限</div>
                      <div class="text-[11.5px] text-[#858A95]">0 = 不限</div>
                    </div>
                    <input
                      v-model.number="draft.tokenBudget"
                      type="number"
                      min="0"
                      step="5000"
                      class="w-32 h-9 rounded-[9px] border border-[#E5E5EA] bg-[#F5F5F7] focus:bg-white px-3 text-[13px] font-mono text-[#25262A] outline-none focus:border-[#7965CC] transition"
                    />
                  </div>
                  <div class="flex justify-end">
                    <div class="inline-flex p-0.5 rounded-[8px] bg-[#F2F2F5]">
                      <button
                        v-for="presetVal in [0, 50000, 150000, 500000]"
                        :key="presetVal"
                        type="button"
                        @click="draft.tokenBudget = presetVal"
                        class="px-2.5 py-1 rounded-[6px] text-[11.5px] font-medium transition"
                        :class="
                          (draft.tokenBudget ?? 0) === presetVal
                            ? 'bg-white text-[#25262A] shadow-2xs'
                            : 'text-[#666B76] hover:text-[#25262A]'
                        "
                      >
                        {{ presetVal === 0 ? "不限" : `${presetVal / 1000}k` }}
                      </button>
                    </div>
                  </div>
                </div>

                <div class="grid grid-cols-1 sm:grid-cols-2 gap-4 items-end">
                  <div>
                    <label class="block text-[12px] font-medium text-[#666B76] mb-1.5">上下文窗口（Tokens）</label>
                    <input
                      v-model.number="draft.contextWindowTokens"
                      type="number"
                      min="2048"
                      step="4096"
                      class="w-full h-9 rounded-[9px] border border-[#E5E5EA] bg-[#F5F5F7] focus:bg-white px-3 text-[13px] font-mono text-[#25262A] outline-none focus:border-[#7965CC] transition"
                    />
                  </div>
                  <div>
                    <div class="flex items-center justify-between text-[12px] mb-1">
                      <span class="font-medium text-[#666B76]">记忆摘要阈值</span>
                      <span class="font-mono font-medium text-[#25262A]">{{ draft.compactionThresholdPct ?? 80 }}%</span>
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
              </div>
            </div>
          </template>

          <!-- Tab 6: 外观 -->
          <template v-else-if="activeTab === 'appearance'">
            <div>
              <h3 class="text-[20px] font-semibold text-[#25262A] tracking-tight">外观</h3>
              <p class="text-[12.5px] text-[#858A95] mt-1">根据系统环境或个人偏好调整界面主题。</p>
            </div>

            <div class="grid grid-cols-1 sm:grid-cols-3 gap-3">
              <button
                v-for="opt in [
                  { id: 'system', label: '跟随系统', darkPreview: false },
                  { id: 'light', label: '浅色', darkPreview: false },
                  { id: 'dark', label: '深色', darkPreview: true },
                ]"
                :key="opt.id"
                type="button"
                @click="triggerHaptic('light'); setThemePreference(opt.id as ThemePreference)"
                class="text-left rounded-[12px] border p-2.5 transition-all outline-none"
                :class="
                  themePreference === opt.id
                    ? 'border-[#7965CC] bg-[#F0EDF9]/50 ring-1 ring-[#7965CC]/30'
                    : 'border-[#E5E5EA] bg-white hover:bg-[#F5F5F7]'
                "
              >
                <div
                  class="h-14 rounded-[8px] border overflow-hidden flex mb-2.5"
                  :class="opt.darkPreview ? 'bg-[#1C1C21] border-white/10' : 'bg-[#FFFFFF] border-[#ECECF0]'"
                >
                  <div
                    class="w-7 h-full shrink-0"
                    :class="opt.darkPreview ? 'bg-[#28282F]' : 'bg-[#EFEFF2]'"
                  ></div>
                  <div class="flex-1 p-2 space-y-1.5 flex flex-col justify-center">
                    <div
                      v-for="n in 4"
                      :key="n"
                      class="h-1 rounded-full"
                      :class="opt.darkPreview ? 'bg-white/15' : 'bg-[#D6D8DE]'"
                    ></div>
                  </div>
                </div>
                <div class="flex items-center justify-between px-0.5">
                  <span class="text-[12.5px] font-medium text-[#25262A]">{{ opt.label }}</span>
                  <Check v-if="themePreference === opt.id" class="w-3.5 h-3.5 text-[#7965CC]" />
                </div>
              </button>
            </div>

            <p class="text-[12px] text-[#858A95]">
              主题实时预览，统一应用于主窗口、观察面板和设置弹窗。
            </p>
          </template>
          </div>
        </div>

        <!-- Clean Minimal Footer -->
        <div class="shrink-0 px-4 md:px-6 py-3 border-t border-[#ECECF0] bg-white">
          <div class="max-w-[600px] mx-auto flex items-center justify-between gap-3">
            <span class="hidden sm:inline text-[11.5px] text-[#858A95]">
              全局 API 与本地推演设置
            </span>
            <div class="flex-1 sm:flex-initial grid grid-cols-[1fr_2fr] sm:flex items-center justify-end gap-2.5">
              <button
                type="button"
                @click="requestClose"
                class="h-9 md:h-8 px-4 rounded-[9px] text-[13px] md:text-[12.5px] font-medium text-[#666B76] hover:bg-[#EFEFF2] transition-colors outline-none"
              >
                关闭
              </button>
              <button
                type="button"
                @click="handleSave"
                class="h-9 md:h-8 px-5 md:px-4 rounded-[9px] text-[13px] md:text-[12.5px] font-medium bg-[#242426] hover:bg-[#363639] text-white transition-colors outline-none"
              >
                保存配置
              </button>
            </div>
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
