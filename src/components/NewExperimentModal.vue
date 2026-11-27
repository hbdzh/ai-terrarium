<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
  X,
  Plus,
  Trash2,
  Sparkles,
  Loader2,
  Save,
  Lock,
  LayoutGrid,
  MessageSquare,
  BookOpen,
  Pencil,
  ArrowRight,
  Dices,
} from "lucide-vue-next";
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
import RoleSvgAvatar from "./ui/RoleSvgAvatar.vue";
import ToggleSwitch from "./ui/ToggleSwitch.vue";
import { randomAvatarSeed, seedFromString } from "../utils/avatarSvg";
import { DEFAULT_BASE_URL, DEFAULT_MODEL_NAME } from "../constants/providers";
import {
  applyApiProfileToNode,
  resolveNodeApiProfileId,
  syncConfigWithApiProfiles,
} from "../utils/config";
import { triggerHaptic } from "../utils/native";
import {
  MECHANIC_DESCRIPTIONS,
  type MechanicKey,
} from "../constants/mechanicDescriptions";
import {
  applyTagLocksToMechanics,
  getLockedMechanics,
  toggleExperimentTag,
} from "../constants/experimentTags";
import type {
  ApiProfile,
  CharacterCard,
  ExperimentConfig,
  GeneratedScenario,
  NodeConfig,
  PresetConfigOverrides,
  PresetTemplate,
  TimedClue,
} from "../types";

const props = defineProps<{
  open: boolean;
  presets: PresetTemplate[];
  currentConfig: ExperimentConfig | null;
  characterCards?: CharacterCard[];
  apiProfiles: ApiProfile[];
}>();

const emit = defineEmits<{
  close: [];
  create: [title: string, config: ExperimentConfig];
  savePreset: [preset: PresetTemplate];
  deletePreset: [presetId: string];
}>();

const title = ref("");
const selectedPresetId = ref("free");

const customAgents = ref<NodeConfig[]>([]);
const customGlobalPrompt = ref("");
const customInitialTopic = ref("");
const customInitialBoard = ref("");
const customTimedClues = ref<TimedClue[]>([]);
const timedCluesOpen = ref(false);
const enableNarrativeArc = ref(false);
const targetTotalTurns = ref(30);
const syncToPreset = ref(false);
const activeTab = ref<"basic" | "advanced" | "ai">("basic");
const editingAgentIdx = ref<number | null>(null);
const isNewRoleDraft = ref(false);
const editingDraft = ref<{ label: string; apiProfileId: string; systemPrompt: string; secretGoal: string; avatarSeed: number }>({
  label: "",
  apiProfileId: "",
  systemPrompt: "",
  secretGoal: "",
  avatarSeed: 1,
});
const avatarSeeds = ref<Record<string, number>>({});

const selectedTags = ref<string[]>([]);
const intuitiveFastReply = ref(false);
const allowPassTurn = ref(false);
const randomMoodDice = ref(false);
const enablePrivateNotes = ref(false);
const enableBoardTools = ref(true);
const enableSmartSelector = ref(true);

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

const activeRulesCount = computed(
  () => Object.values(currentMechanicsSnapshot()).filter(Boolean).length
);

const activePresetObj = computed(
  () => props.presets.find((p) => p.id === selectedPresetId.value) ?? props.presets[0]
);

function getRoleAvatarSeed(ag: NodeConfig): number {
  if (avatarSeeds.value[ag.id] !== undefined) {
    return avatarSeeds.value[ag.id];
  }
  return seedFromString(`${ag.id}-${ag.label}`);
}

function openRoleEditor(idx: number) {
  const ag = customAgents.value[idx];
  if (!ag) return;
  isNewRoleDraft.value = false;
  editingAgentIdx.value = idx;
  editingDraft.value = {
    label: ag.label,
    apiProfileId: ag.apiProfileId || props.apiProfiles[0]?.id || "",
    systemPrompt: ag.systemPrompt,
    secretGoal: ag.secretGoal ?? "",
    avatarSeed: getRoleAvatarSeed(ag),
  };
}

function closeRoleEditor() {
  editingAgentIdx.value = null;
  isNewRoleDraft.value = false;
}

function saveRoleEditor() {
  if (editingAgentIdx.value === null) return;
  if (isNewRoleDraft.value) {
    const prof =
      props.apiProfiles.find((p) => p.id === editingDraft.value.apiProfileId) ??
      defaultProfile();
    const newId = `ai-${Date.now().toString(36)}-${customAgents.value.length + 1}`;
    const created = applyApiProfileToNode(
      {
        id: newId,
        label: editingDraft.value.label.trim() || nextUniqueAgentLabel(customAgents.value),
        baseUrl: prof.baseUrl,
        apiKey: prof.apiKey,
        modelName: prof.modelName,
        temperature: 0.9,
        systemPrompt: editingDraft.value.systemPrompt,
        secretGoal: editingDraft.value.secretGoal.trim(),
      },
      prof
    );
    avatarSeeds.value[newId] = editingDraft.value.avatarSeed;
    customAgents.value.push(created);
    closeRoleEditor();
    return;
  }
  const ag = customAgents.value[editingAgentIdx.value];
  if (!ag) {
    closeRoleEditor();
    return;
  }
  ag.label = editingDraft.value.label;
  ag.systemPrompt = editingDraft.value.systemPrompt;
  ag.secretGoal = editingDraft.value.secretGoal.trim();
  avatarSeeds.value[ag.id] = editingDraft.value.avatarSeed;
  if (editingDraft.value.apiProfileId) {
    selectAgentProfile(ag, editingDraft.value.apiProfileId);
  }
  closeRoleEditor();
}

function applyCardToEditor(card: CharacterCard) {
  if (editingAgentIdx.value === null) return;
  editingDraft.value.label = disambiguateAgentLabel(
    card.name,
    customAgents.value,
    isNewRoleDraft.value ? -1 : editingAgentIdx.value
  );
  const parts = [card.basePrompt.trim()];
  if (inheritCardMemory.value) {
    if (card.behaviorTraits.trim()) {
      parts.push(`【行为特征】\n${card.behaviorTraits.trim()}`);
    }
    if (card.memoryImprints && card.memoryImprints.length > 0) {
      parts.push(
        `【经历记忆】\n${card.memoryImprints.map((m) => `- ${m}`).join("\n")}`
      );
    }
  }
  editingDraft.value.systemPrompt = parts.filter(Boolean).join("\n\n");
}

function addModalAgentAndEdit() {
  isNewRoleDraft.value = true;
  editingAgentIdx.value = customAgents.value.length;
  editingDraft.value = {
    label: nextUniqueAgentLabel(customAgents.value),
    apiProfileId: defaultProfile().id,
    systemPrompt: "",
    secretGoal: "",
    avatarSeed: randomAvatarSeed(),
  };
}

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

function defaultProfile(): ApiProfile {
  return (
    props.apiProfiles[0] ?? {
      id: "default",
      name: "默认 API",
      baseUrl: DEFAULT_BASE_URL,
      apiKey: "",
      modelName: DEFAULT_MODEL_NAME,
    }
  );
}

function selectAgentProfile(ag: NodeConfig, profileId: string) {
  const prof = props.apiProfiles.find((p) => p.id === profileId) ?? defaultProfile();
  applyApiProfileToNode(ag, prof);
}

function makeDefaultAgents(): NodeConfig[] {
  const prof = defaultProfile();
  return [
    applyApiProfileToNode(
      {
        id: "AiA",
        label: "角色 1",
        baseUrl: prof.baseUrl,
        apiKey: prof.apiKey,
        modelName: prof.modelName,
        temperature: 0.9,
        systemPrompt: "",
      },
      prof
    ),
    applyApiProfileToNode(
      {
        id: "AiB",
        label: "角色 2",
        baseUrl: prof.baseUrl,
        apiKey: prof.apiKey,
        modelName: prof.modelName,
        temperature: 0.9,
        systemPrompt: "",
      },
      prof
    ),
  ];
}

function getPresetAgents(p: PresetTemplate) {
  if (Array.isArray(p.agentPrompts) && p.agentPrompts.length > 0) {
    return p.agentPrompts;
  }
  return [
    { label: p.nodeALabel || "角色 1", systemPrompt: p.nodeAPrompt || "" },
    { label: p.nodeBLabel || "角色 2", systemPrompt: p.nodeBPrompt || "" },
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
  const prof = defaultProfile();
  baseAgents.forEach((ag) => {
    const pid = resolveNodeApiProfileId(ag, props.apiProfiles);
    const matched = props.apiProfiles.find((item) => item.id === pid) ?? prof;
    applyApiProfileToNode(ag, matched);
  });
  while (baseAgents.length < pAgents.length) {
    const n = baseAgents.length + 1;
    baseAgents.push(
      applyApiProfileToNode(
        {
          id: `ai-${Date.now().toString(36)}-${n}`,
          label: `角色 ${n}`,
          baseUrl: prof.baseUrl,
          apiKey: prof.apiKey,
          modelName: prof.modelName,
          temperature: 0.9,
          systemPrompt: "",
        },
        prof
      )
    );
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
      customTimedClues.value = [];
      timedCluesOpen.value = false;
      enableNarrativeArc.value = props.currentConfig?.enableNarrativeArc ?? false;
      targetTotalTurns.value = props.currentConfig?.targetTotalTurns ?? 30;
      ideaInput.value = "";
      ideaError.value = null;
      activeTab.value = "basic";
      editingAgentIdx.value = null;
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
    customTimedClues.value = res.timedClues ?? [];
    timedCluesOpen.value = customTimedClues.value.length > 0;
    applyRecommendedConfig({
      allowPassTurn: false,
      randomMoodDice: false,
      intuitiveFastReply: false,
      enableWhiteboard: true,
      enablePrivateNotes: true,
      smartTurnOrder: true,
    });
    const prof = defaultProfile();
    customAgents.value = res.agents.map((ag, idx) => {
      const existing = props.currentConfig?.agents[idx];
      const pid = existing
        ? resolveNodeApiProfileId(existing, props.apiProfiles)
        : prof.id;
      const matched = props.apiProfiles.find((p) => p.id === pid) ?? prof;
      return applyApiProfileToNode(
        {
          id: existing?.id || `ai-${Date.now().toString(36)}-${idx + 1}`,
          label: ag.label,
          baseUrl: matched.baseUrl,
          apiKey: matched.apiKey,
          modelName: matched.modelName,
          temperature: existing?.temperature ?? 0.9,
          systemPrompt: ag.systemPrompt,
          secretGoal: ag.secretGoal ?? "",
        },
        matched
      );
    });
    activeTab.value = "basic";
  } catch (err: any) {
    ideaError.value = err?.toString?.() ?? "生成剧本失败";
  } finally {
    generatingScenario.value = false;
  }
}

function formatPresetCast(p: PresetTemplate): string {
  if (p.id === "free") return "无预设 · 自由对话";
  if (p.id === "custom") return "支持多角色自由设定";
  return getPresetAgents(p)
    .map((a) => a.label)
    .join(" vs ");
}

const inheritCardMemory = ref(false);

function selectPreset(p: PresetTemplate) {
  selectedPresetId.value = p.id;
  const cleanName = p.name.replace(/（.*?）/g, "");
  title.value = `${cleanName} #${Math.floor(10 + Math.random() * 90)}`;
  hydrateFromPreset(p);
}

const duplicateLabels = computed(() => duplicateLabelSet(customAgents.value));

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
    nodeALabel: agents[0]?.label || "角色 1",
    nodeAPrompt: agents[0]?.systemPrompt || "",
    nodeBLabel: agents[1]?.label || "角色 2",
    nodeBPrompt: agents[1]?.systemPrompt || "",
  };
  emit("savePreset", updatedPreset);
}

function handleCreate() {
  const dup = findDuplicateAgentLabel(customAgents.value);
  if (dup) {
    ideaError.value = `角色名称「${dup}」存在重复，请修改后再创建`;
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
        enableCompactionGuard: true,
        enablePrivateNotes: false,
        enableSmartSelector: true,
        enableNarrativeArc: enableNarrativeArc.value,
        targetTotalTurns: Math.max(5, Math.round(targetTotalTurns.value || 30)),
        agents: makeDefaultAgents(),
        nodeC: {
          id: "observer",
          label: "局势分析",
          baseUrl: DEFAULT_BASE_URL,
          apiKey: "",
          modelName: DEFAULT_MODEL_NAME,
          temperature: 0.3,
          systemPrompt:
            "你负责从第三人称视角分析多角色对话的局势走向与关系变化。",
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
  base.timedClues = customTimedClues.value.map((c) => ({ ...c }));
  base.agents = customAgents.value.map((a, idx) => ({
    ...a,
    label: a.label.trim() || `角色 ${idx + 1}`,
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
      nodeALabel: base.agents[0]?.label || "角色 1",
      nodeAPrompt: base.agents[0]?.systemPrompt || "",
      nodeBLabel: base.agents[1]?.label || "角色 2",
      nodeBPrompt: base.agents[1]?.systemPrompt || "",
    };
    emit("savePreset", updatedPreset);
  }

  const synced = syncConfigWithApiProfiles(base, props.apiProfiles);

  triggerHaptic("confirm");
  emit("create", title.value.trim() || "未命名推演", synced);
  emit("close");
}
</script>

<template>
  <AdaptiveModal
    :open="open"
    title="新建推演"
    description="选择剧本预设或自定义角色，也可输入想法生成剧本"
    max-width-class="max-w-[960px]"
    content-class="md:h-[min(86dvh,740px)]"
    mobile-presentation="fullscreen"
    @close="emit('close')"
  >
    <template #header>
      <!-- Mobile-only Header (< md) -->
      <div class="md:hidden flex items-center justify-between px-4 py-3 border-b border-black/[0.06] bg-white shrink-0">
        <h2 class="text-[16px] font-semibold text-[#1D1D1F] truncate">新建推演</h2>
        <button
          type="button"
          @click="emit('close')"
          class="w-8 h-8 flex items-center justify-center rounded-full bg-[#F2F2F7] text-slate-500 shrink-0"
        >
          <X class="w-4 h-4" />
        </button>
      </div>
    </template>

        <!-- Full-Height 2-Pane Split on Desktop, Single-Scroll on Mobile -->
        <div
          class="flex-1 min-h-0 flex flex-col md:flex-row overflow-y-auto md:overflow-hidden bg-white relative"
        >
          <!-- Left Sidebar: Presets -->
          <div class="w-full md:w-[244px] shrink-0 md:border-r border-[#E9E9EB] bg-[#F7F7F8] px-3 py-3 md:pt-5 md:px-3 md:pb-3 flex flex-col md:overflow-hidden select-none">
            <div class="hidden md:flex items-center gap-2 px-2.5 pb-4 text-[15px] font-semibold text-[#202222] tracking-tight">
              <LayoutGrid class="w-[18px] h-[18px] text-[#686A70]" />
              <span>新建推演</span>
            </div>

            <div class="flex items-center justify-between px-2.5 pb-2 text-[11px] text-[#96989D]">
              <span>剧本预设</span>
              <span>{{ presets.length }}</span>
            </div>

            <div class="flex md:flex-col gap-1.5 md:gap-0.5 overflow-x-auto md:overflow-x-visible md:overflow-y-auto md:flex-1 md:min-h-0 snap-x snap-mandatory pb-1 md:pb-0 no-scrollbar">
              <div
                v-for="(p, pIdx) in presets"
                :key="p.id"
                @click="selectPreset(p)"
                class="group cursor-pointer w-[210px] md:w-full shrink-0 snap-start flex items-center gap-2.5 text-left px-2.5 py-2.5 rounded-[9px] transition-colors"
                :class="
                  selectedPresetId === p.id
                    ? 'bg-[#E8E7ED] text-[#202222]'
                    : 'hover:bg-[#EFEFF1] bg-white/80 md:bg-transparent text-[#202222]'
                "
              >
                <span
                  class="hidden md:grid place-items-center w-[29px] h-[29px] rounded-[8px] border shrink-0 transition-colors"
                  :class="
                    selectedPresetId === p.id
                      ? 'bg-[var(--ds-accent-soft)] border-[var(--ds-accent-border)] text-[var(--ds-accent)]'
                      : 'bg-white border-[#E2E2E7] text-[#777681]'
                  "
                >
                  <MessageSquare v-if="pIdx === 0" class="w-3.5 h-3.5" />
                  <BookOpen v-else class="w-3.5 h-3.5" />
                </span>
                <div class="min-w-0 flex-1">
                  <div class="text-[12px] font-medium truncate">{{ p.name }}</div>
                  <div class="text-[10.5px] text-[#92939B] truncate mt-0.5">{{ formatPresetCast(p) }}</div>
                </div>
                <button
                  v-if="!p.isBuiltin"
                  type="button"
                  @click.stop="handleDeletePresetItem(p)"
                  class="p-1 rounded text-slate-400 opacity-0 group-hover:opacity-100 hover:text-[var(--ds-danger)] transition shrink-0"
                  title="删除此自定义预设"
                >
                  <Trash2 class="w-3 h-3" />
                </button>
              </div>
            </div>

            <div class="hidden md:block mt-auto pt-3 px-2.5 text-[11px] text-[#999BA0] border-t border-[#E8E8EA]">
              选择预设，或从空白剧本开始
            </div>
          </div>

          <!-- Right Main Column -->
          <div class="flex-1 min-w-0 flex flex-col bg-white overflow-hidden">
            <!-- Two-Tab Bar + Actions -->
            <div class="flex items-center justify-between gap-4 px-4 md:px-7 h-[46px] border-b border-[#ECECEF] shrink-0">
              <div class="flex items-stretch gap-6 h-full">
                <button
                  type="button"
                  @click="activeTab = 'basic'"
                  class="relative text-[13px] transition-colors outline-none"
                  :class="activeTab === 'basic' ? 'text-[#292A30] font-semibold' : 'text-[#82838C] font-medium hover:text-[#292A30]'"
                >
                  <span>基础设定</span>
                  <span
                    v-if="activeTab === 'basic'"
                    class="absolute bottom-0 left-0 right-0 h-[2px] bg-[#2C2C32] rounded-full"
                  ></span>
                </button>
                <button
                  type="button"
                  @click="activeTab = 'advanced'"
                  class="relative flex items-center gap-1.5 text-[13px] transition-colors outline-none"
                  :class="activeTab === 'advanced' ? 'text-[#292A30] font-semibold' : 'text-[#82838C] font-medium hover:text-[#292A30]'"
                >
                  <span>高级设定</span>
                  <span class="text-[10px] text-[#777780] bg-[#F0F0F2] px-1.5 py-0.5 rounded-[5px] font-medium">
                    {{ activeRulesCount }}
                  </span>
                  <span
                    v-if="activeTab === 'advanced'"
                    class="absolute bottom-0 left-0 right-0 h-[2px] bg-[#2C2C32] rounded-full"
                  ></span>
                </button>
                <button
                  type="button"
                  @click="activeTab = 'ai'"
                  class="relative flex items-center gap-1.5 text-[13px] transition-colors outline-none"
                  :class="activeTab === 'ai' ? 'text-[var(--ds-accent)] font-semibold' : 'text-[#82838C] font-medium hover:text-[#292A30]'"
                >
                  <Sparkles class="w-3.5 h-3.5" />
                  <span>AI 剧本创作</span>
                  <span
                    v-if="activeTab === 'ai'"
                    class="absolute bottom-0 left-0 right-0 h-[2px] bg-[var(--ds-accent)] rounded-full"
                  ></span>
                </button>
              </div>

              <div class="hidden md:flex items-center gap-2 shrink-0">
                <button
                  v-if="selectedPresetId !== 'free' && selectedPresetId !== 'custom'"
                  type="button"
                  @click="overwriteSelectedPreset"
                  class="flex items-center gap-1 px-2 py-1 rounded-md text-[11px] font-medium text-slate-400 hover:text-[var(--ds-accent)] hover:bg-[var(--ds-accent-soft)] transition"
                  title="覆盖保存当前选中的预设"
                >
                  <Save class="w-3 h-3" />
                  <span>覆盖预设</span>
                </button>
                <button
                  type="button"
                  @click="emit('close')"
                  class="w-7 h-7 grid place-items-center rounded-[7px] text-[#909197] hover:bg-[#F2F2F4] hover:text-[#333] transition"
                  title="关闭"
                >
                  <X class="w-4 h-4" />
                </button>
              </div>
            </div>

            <!-- Scrollable Content -->
            <div class="flex-1 min-h-0 overflow-y-auto px-4 py-5 md:px-8 md:py-6">
              <!-- TAB 1: 基础设定 -->
              <div v-if="activeTab === 'basic'" class="space-y-5">
                <!-- Hero Title Line -->
                <div>
                  <div class="flex items-center gap-2.5 focus-within:shadow-[0_1px_0_#D6CFEA] pb-0.5 transition-shadow">
                    <input
                      v-model="title"
                      placeholder="输入推演标题"
                      class="w-full min-w-0 bg-transparent text-[24px] md:text-[26px] font-semibold tracking-tight text-[#202222] placeholder:text-[#B6B7BC] outline-none leading-snug"
                    />
                    <Pencil class="w-4 h-4 text-[#B6B7BC] shrink-0" />
                  </div>
                  <p class="text-[12px] text-[#92949B] mt-1">
                    {{ activePresetObj?.description || "自由设定情境与角色，观察对话如何自然展开。" }}
                  </p>
                </div>

                <!-- Scene & Rules -->
                <section>
                  <div class="flex items-center gap-2 mb-2.5">
                    <h2 class="text-[14px] font-semibold text-[#202222]">场景与规则</h2>
                    <span class="text-[11px] text-[#AAAAAA]">可选</span>
                  </div>
                  <div class="flex items-start gap-2.5 px-3 py-2.5 rounded-[9px] bg-[#FAFAFA] border border-[#EFEFF0] focus-within:border-[#007AFF]/50 focus-within:bg-white transition">
                    <BookOpen class="w-4 h-4 text-[#A0A2A8] mt-0.5 shrink-0" />
                    <textarea
                      v-model="customGlobalPrompt"
                      rows="2"
                      placeholder="为这场推演设定背景、文风或对话规则…"
                      class="w-full resize-y min-h-[38px] bg-transparent text-[12.5px] text-[#202222] placeholder:text-[#A0A1A8] leading-relaxed outline-none"
                    ></textarea>
                  </div>
                </section>

                <!-- Roles 2-Column Card Grid -->
                <section>
                  <div class="flex items-center gap-2 mb-2.5">
                    <h2 class="text-[14px] font-semibold text-[#202222]">参与角色</h2>
                    <span class="text-[11px] text-[#92949B] bg-[#F2F2F4] px-2 py-0.5 rounded-[6px] font-medium">
                      {{ customAgents.length }} 位
                    </span>
                    <div class="flex-1"></div>
                    <button
                      type="button"
                      @click="addModalAgentAndEdit"
                      class="inline-flex items-center gap-1 px-2 py-1 rounded-[6px] text-[12px] text-[#686B73] hover:bg-[#F2F2F5] hover:text-[#202222] transition"
                    >
                      <Plus class="w-3.5 h-3.5" />
                      <span>添加角色</span>
                    </button>
                  </div>

                  <div class="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
                    <article
                      v-for="(ag, idx) in customAgents"
                      :key="ag.id"
                      @click="openRoleEditor(idx)"
                      class="group cursor-pointer rounded-[12px] border bg-white p-3.5 transition-all"
                      :class="
                        duplicateLabels.has(effectiveAgentLabel(ag, idx))
                          ? 'border-rose-400 ring-2 ring-rose-100'
                          : 'border-[#E6E6E9] hover:border-[#007AFF]/45 hover:shadow-[0_2px_9px_rgba(0,122,255,0.06)]'
                      "
                    >
                      <div class="flex items-center gap-2.5">
                        <RoleSvgAvatar :seed="getRoleAvatarSeed(ag)" :size="38" />
                        <div class="min-w-0 flex-1">
                          <div class="text-[13px] font-semibold text-[#202222] truncate">
                            {{ effectiveAgentLabel(ag, idx) }}
                          </div>
                          <div class="text-[11px] text-[#7C7F89] mt-0.5 truncate">
                            {{ apiProfiles.find((p) => p.id === ag.apiProfileId)?.name ?? "默认" }} · {{ ag.modelName }}
                          </div>
                        </div>
                        <button
                          v-if="customAgents.length > 2"
                          type="button"
                          @click.stop="removeModalAgent(idx)"
                          class="w-7 h-7 grid place-items-center rounded-[7px] text-[#909197] opacity-0 group-hover:opacity-100 hover:bg-[var(--ds-danger-soft)] hover:text-[var(--ds-danger)] self-start shrink-0 transition"
                          title="移除角色"
                        >
                          <Trash2 class="w-3.5 h-3.5" />
                        </button>
                      </div>

                      <div class="mt-3 pt-2.5 border-t border-[#F0F0F1] text-[11px] truncate" :class="ag.systemPrompt.trim() ? 'text-[#5D6268]' : 'text-[#93959C]'">
                        {{ ag.systemPrompt.trim() || "尚未设定角色性格" }}
                      </div>
                    </article>
                  </div>
                  <p
                    v-if="duplicateLabels.size > 0"
                    class="text-[11.5px] text-rose-600 font-medium mt-2"
                  >
                    角色名称不能重复，请修改重名角色
                  </p>
                </section>
              </div>

              <!-- TAB 2: 高级设定 -->
              <div v-else-if="activeTab === 'advanced'" class="space-y-6">
                <div>
                  <h1 class="text-[22px] font-semibold tracking-tight text-[#202222]">推演设定</h1>
                  <p class="text-[12px] text-[#82848D] mt-1">调整剧本标签、对话规则与剧情节奏。</p>
                </div>

                <!-- Tags -->
                <section class="space-y-2.5">
                  <ExperimentTagsPicker
                    :tags="selectedTags"
                    @toggle="handleToggleTag"
                    @clear="handleClearTags"
                  />
                </section>

                <!-- 2-Col Dialogue Rules Grid -->
                <section class="space-y-2.5">
                  <div class="flex items-center justify-between">
                    <h3 class="text-[14px] font-semibold text-[#202222]">对话规则</h3>
                    <small class="text-[11px] text-[#92949D]">可随剧本标签自动调整</small>
                  </div>
                  <div class="grid grid-cols-1 sm:grid-cols-2 gap-x-5 rounded-[12px] border border-[#E9E9ED] px-3.5">
                    <button
                      v-for="(item, idx) in engineMechanismPills"
                      :key="item.key"
                      type="button"
                      @click="toggleMechanicPill(item.key, item.refVal)"
                      :class="[
                        'flex items-center gap-2.5 py-3 text-left transition-opacity',
                        idx >= 2 ? 'border-t border-[#F0F0F2]' : (idx === 1 ? 'border-t sm:border-t-0 border-[#F0F0F2]' : ''),
                        lockedMechanics[item.key] ? 'opacity-55 cursor-not-allowed' : 'cursor-pointer',
                      ]"
                    >
                      <div class="flex-1 min-w-0">
                        <div class="flex items-center gap-1.5 text-[12px] font-semibold text-[#202222]">
                          <span>{{ item.label }}</span>
                          <Lock v-if="lockedMechanics[item.key]" class="w-3 h-3 text-slate-400 shrink-0" />
                        </div>
                        <small class="block text-[10.5px] text-[#93949B] mt-0.5 leading-snug">
                          {{
                            lockedMechanics[item.key]
                              ? `已由标签 [${lockedMechanics[item.key]}] 锁定`
                              : item.refVal.value
                                ? MECHANIC_DESCRIPTIONS[item.key].onEffect
                                : MECHANIC_DESCRIPTIONS[item.key].offEffect
                          }}
                        </small>
                      </div>
                      <span
                        class="w-[31px] h-[18px] rounded-full relative shrink-0 transition-colors"
                        :class="item.refVal.value ? 'bg-[var(--ds-accent)]' : 'bg-[#D9D9DE]'"
                      >
                        <span
                          class="absolute top-[2px] left-[2px] w-[14px] h-[14px] rounded-full bg-white shadow-xs transition-transform"
                          :class="item.refVal.value ? 'translate-x-[13px]' : ''"
                        ></span>
                      </span>
                    </button>
                  </div>
                </section>

                <!-- Plot Progression -->
                <section class="space-y-2.5">
                  <div class="flex items-center justify-between">
                    <h3 class="text-[14px] font-semibold text-[#202222]">剧情进程</h3>
                    <small class="text-[11px] text-[#AAAAAA]">可选</small>
                  </div>

                  <div class="rounded-[12px] border border-[#E9E9ED] divide-y divide-[#F0F0F2]">
                    <div class="p-3.5 space-y-2">
                      <label class="block text-[12px] font-semibold text-[#202222]">第 1 轮开场设定</label>
                      <textarea
                        v-model="customInitialTopic"
                        rows="2"
                        placeholder="留空则直接开始对话…"
                        class="w-full resize-none rounded-[8px] border border-[#E9E9ED] bg-[#FAFAFA] focus:bg-white px-3 py-2 text-[12px] leading-relaxed outline-none"
                      ></textarea>
                    </div>

                    <div v-if="customInitialBoard.trim().length > 0" class="p-3.5 space-y-2">
                      <label class="block text-[12px] font-semibold text-[#202222]">初始共享白板（全员可见）</label>
                      <textarea
                        v-model="customInitialBoard"
                        rows="3"
                        class="w-full resize-none rounded-[8px] border border-[#E9E9ED] bg-[#FAFAFA] focus:bg-white px-3 py-2 text-[12px] font-mono leading-relaxed outline-none"
                      ></textarea>
                    </div>

                    <div class="p-3.5 space-y-2.5">
                      <button
                        type="button"
                        @click="timedCluesOpen = !timedCluesOpen"
                        class="w-full flex items-center justify-between text-left"
                      >
                        <span class="text-[12.5px] font-medium text-[#202222]">
                          定时线索（分幕物证）
                          <span v-if="customTimedClues.length > 0" class="ml-1 text-[11px] text-slate-400">
                            ({{ customTimedClues.length }} 条)
                          </span>
                        </span>
                        <span class="text-[11px] text-slate-400">{{ timedCluesOpen ? "收起" : "展开" }}</span>
                      </button>

                      <div v-if="timedCluesOpen" class="space-y-2 pt-1">
                        <div
                          v-for="(clue, cIdx) in customTimedClues"
                          :key="cIdx"
                          class="flex items-center gap-2"
                        >
                          <span class="text-[11px] text-slate-500 shrink-0">第</span>
                          <input
                            v-model.number="clue.triggerTurn"
                            type="number"
                            min="2"
                            max="200"
                            class="w-14 h-7 rounded-md border border-[#E2E2E6] bg-white px-1.5 text-[12px] text-center outline-none"
                          />
                          <span class="text-[11px] text-slate-500 shrink-0">轮</span>
                          <input
                            v-model="clue.content"
                            type="text"
                            placeholder="输入届时释放的客观物证或突发线索…"
                            class="flex-1 h-7 rounded-md border border-[#E2E2E6] bg-white px-2 text-[12px] outline-none"
                          />
                          <button
                            type="button"
                            @click="customTimedClues.splice(cIdx, 1)"
                            class="text-[11px] text-slate-400 hover:text-rose-600 px-1.5"
                          >
                            删除
                          </button>
                        </div>
                        <button
                          type="button"
                          @click="customTimedClues.push({ triggerTurn: 6 + customTimedClues.length * 4, content: '', triggered: false })"
                          class="text-[11.5px] font-medium text-[#007AFF] hover:underline"
                        >
                          + 添加定时线索
                        </button>
                      </div>
                    </div>

                    <div class="p-3.5 flex flex-wrap items-center justify-between gap-2">
                      <span class="text-[12.5px] font-medium text-[#202222]">三幕式剧情节奏</span>
                      <div class="flex items-center gap-3">
                        <div v-if="enableNarrativeArc" class="flex items-center gap-1.5 text-[12px] text-slate-600">
                          <span>目标</span>
                          <input
                            v-model.number="targetTotalTurns"
                            type="number"
                            min="5"
                            max="500"
                            class="w-16 rounded-lg border border-[#E2E2E6] bg-[#FAFAFA] px-2 py-1 text-[12.5px] font-mono outline-none"
                          />
                          <span>轮</span>
                        </div>
                        <ToggleSwitch v-model="enableNarrativeArc" />
                      </div>
                    </div>

                    <div
                      v-if="selectedPresetId !== 'free' && selectedPresetId !== 'custom'"
                      class="p-3.5 flex items-center justify-between gap-3"
                    >
                      <span class="text-[12.5px] text-slate-600">创建时同步覆盖所选预设</span>
                      <ToggleSwitch v-model="syncToPreset" />
                    </div>
                  </div>
                </section>
              </div>

              <!-- TAB 3: AI 剧本创作 -->
              <div v-else class="space-y-5">
                <div>
                  <h1 class="text-[22px] font-semibold tracking-tight text-[#202222]">AI 剧本创作</h1>
                  <p class="text-[12px] text-[#82848D] mt-1">输入一句话灵感，自动生成推演标题、场景规则、初始白板与多位性格鲜明的角色。</p>
                </div>

                <div class="rounded-[13px] border border-[#E6E2EF] bg-gradient-to-br from-[#F8F7FC] to-[#FDFCFE] p-4 space-y-3">
                  <div class="flex items-center gap-2 text-[12px] font-semibold text-[var(--ds-accent)]">
                    <span class="w-6 h-6 rounded-[7px] bg-[var(--ds-accent-soft)] grid place-items-center shrink-0">
                      <Sparkles class="w-3.5 h-3.5" />
                    </span>
                    <span>剧本灵感</span>
                  </div>
                  <textarea
                    v-model="ideaInput"
                    @keydown.enter.exact.prevent="generateFromIdea"
                    rows="4"
                    placeholder="描述一个有趣的情境，例如：暴风雪山庄的三个时间旅行者…"
                    class="w-full resize-none bg-transparent text-[13.5px] text-[#202222] placeholder:text-[#A39DAA] leading-relaxed outline-none"
                  ></textarea>
                  <div class="flex items-center justify-between gap-3 pt-2 border-t border-[#ECE8F5]">
                    <small class="text-[11px] text-[#9A95A4]">生成后将自动填充至「基础设定」供微调</small>
                    <button
                      type="button"
                      @click="generateFromIdea"
                      :disabled="!ideaInput.trim() || generatingScenario"
                      class="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded-[7px] bg-[var(--ds-accent-soft)] hover:bg-[var(--ds-accent-soft-hover)] border border-[var(--ds-accent-border)] text-[var(--ds-accent)] text-[12px] font-semibold transition disabled:opacity-45"
                    >
                      <Loader2 v-if="generatingScenario" class="w-3.5 h-3.5 animate-spin" />
                      <span>{{ generatingScenario ? "生成中..." : "生成剧本" }}</span>
                      <ArrowRight v-if="!generatingScenario" class="w-3.5 h-3.5" />
                    </button>
                  </div>
                  <p v-if="ideaError" class="text-[11.5px] text-rose-600">{{ ideaError }}</p>
                </div>

                <div class="space-y-2">
                  <div class="text-[11.5px] font-medium text-[#92949B]">灵感示例（点击填入）</div>
                  <div class="flex flex-wrap gap-2">
                    <button
                      v-for="sample in [
                        '暴风雪山庄的三个时间旅行者',
                        '深夜便利店停电后的四个陌生人',
                        '未来太空站 AI 伦理委员会紧急听证会',
                        '古董店老板与三位自称画中人的访客',
                      ]"
                      :key="sample"
                      type="button"
                      @click="ideaInput = sample"
                      class="px-3 py-1.5 rounded-lg bg-[#F5F5F7] hover:bg-[var(--ds-accent-soft)] text-[12px] text-[#5D6268] hover:text-[var(--ds-accent)] transition"
                    >
                      {{ sample }}
                    </button>
                  </div>
                </div>
              </div>
            </div>

            <!-- Footer -->
            <footer class="h-[62px] flex items-center gap-3 px-4 md:px-7 shrink-0 bg-white">
              <div class="hidden sm:flex items-center gap-2 text-[11.5px] text-[#95979D]">
                <span class="w-[7px] h-[7px] rounded-full bg-[#82AA91]"></span>
                <span>{{ customAgents.length }} 位角色 · 已准备就绪</span>
              </div>
              <div class="flex-1"></div>
              <button
                type="button"
                @click="emit('close')"
                class="px-3.5 py-2 rounded-[8px] text-[12.5px] font-medium text-[#686A70] hover:bg-[#F2F2F4] transition"
              >
                取消
              </button>
              <button
                type="button"
                @click="handleCreate"
                class="inline-flex items-center gap-2 px-4 py-2 rounded-[9px] bg-[#242527] hover:bg-[#414246] text-white text-[12.5px] font-medium shadow-2xs transition"
              >
                <span>创建并进入沙盒</span>
                <ArrowRight class="w-3.5 h-3.5" />
              </button>
            </footer>
          </div>

          <!-- Role Editor Sub-Dialog -->
          <div
            v-if="editingAgentIdx !== null"
            class="absolute inset-0 z-30 bg-[#171820]/30 backdrop-blur-[2px] grid place-items-center p-4"
            @click.self="closeRoleEditor"
          >
            <div class="w-full max-w-[440px] rounded-[15px] bg-white border border-[#EDEDF0] shadow-[0_15px_60px_rgba(0,0,0,0.2)] p-5 space-y-3.5">
              <div class="flex items-center justify-between gap-2">
                <div class="flex items-center gap-2.5">
                  <RoleSvgAvatar :seed="editingDraft.avatarSeed" :size="34" />
                  <h3 class="text-[16px] font-semibold text-[#202222]">
                    {{ isNewRoleDraft ? "添加角色" : "编辑角色" }}
                  </h3>
                  <button
                    type="button"
                    @click="editingDraft.avatarSeed = randomAvatarSeed()"
                    class="inline-flex items-center gap-1 px-2 py-1 rounded-md text-[11px] font-medium bg-[#F4F4F6] hover:bg-[#EAEAEF] text-slate-600 transition"
                    title="随机生成角色 SVG 图标"
                  >
                    <Dices class="w-3.5 h-3.5" />
                    <span>随机图标</span>
                  </button>
                </div>
                <CharacterLibraryPopover
                  v-if="characterCards && characterCards.length > 0"
                  :cards="characterCards"
                  :inherit-memory="inheritCardMemory"
                  :show-count="false"
                  :compact="true"
                  trigger-label="从角色库选择"
                  @update:inherit-memory="inheritCardMemory = $event"
                  @select="applyCardToEditor"
                />
              </div>

              <div>
                <label class="block text-[12px] text-[#757780] mb-1.5">角色名称</label>
                <input
                  v-model="editingDraft.label"
                  placeholder="角色名称"
                  class="w-full h-9 rounded-[8px] border border-[#E2E2E6] bg-[#FAFAFA] focus:bg-white px-3 text-[13px] outline-none"
                />
              </div>

              <div v-if="apiProfiles.length > 0">
                <label class="block text-[12px] text-[#757780] mb-1.5">使用模型</label>
                <select
                  v-model="editingDraft.apiProfileId"
                  class="w-full h-9 rounded-[8px] border border-[#E2E2E6] bg-[#FAFAFA] focus:bg-white px-2.5 text-[13px] outline-none"
                >
                  <option v-for="prof in apiProfiles" :key="prof.id" :value="prof.id">
                    {{ prof.name }} · {{ prof.modelName }}
                  </option>
                </select>
              </div>

              <div>
                <label class="block text-[12px] text-[#757780] mb-1.5">身份、性格与隐藏动机</label>
                <textarea
                  v-model="editingDraft.systemPrompt"
                  rows="4"
                  placeholder="描述角色的身份、性格、目标或隐藏动机…"
                  class="w-full resize-y min-h-[100px] rounded-[8px] border border-[#E2E2E6] bg-[#FAFAFA] focus:bg-white p-2.5 text-[12.5px] leading-relaxed outline-none"
                ></textarea>
              </div>

              <div>
                <label class="block text-[12px] text-[#757780] mb-1.5">本局暗线目标（可选）</label>
                <input
                  v-model="editingDraft.secretGoal"
                  type="text"
                  placeholder="例如：在不暴露真实身份的前提下让众人怀疑其他人…"
                  class="w-full rounded-[8px] border border-[#E2E2E6] bg-[#FAFAFA] focus:bg-white px-2.5 py-2 text-[12.5px] outline-none"
                />
              </div>

              <div class="flex items-center justify-end gap-2 pt-1">
                <button
                  type="button"
                  @click="closeRoleEditor"
                  class="px-3.5 py-2 rounded-[8px] text-[12.5px] text-[#686A70] hover:bg-[#F2F2F4] transition"
                >
                  取消
                </button>
                <button
                  type="button"
                  @click="saveRoleEditor"
                  class="px-4 py-2 rounded-[9px] bg-[#242527] hover:bg-[#414246] text-white text-[12.5px] font-medium transition"
                >
                  保存
                </button>
              </div>
            </div>
          </div>
        </div>
  </AdaptiveModal>
</template>
