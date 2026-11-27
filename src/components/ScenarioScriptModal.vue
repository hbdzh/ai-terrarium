<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  X,
  Sparkles,
  Plus,
  Trash2,
  Save,
  BookmarkPlus,
  ScrollText,
  BookOpen,
  Dices,
} from "lucide-vue-next";
import type { ApiProfile, CharacterCard, ExperimentConfig, NodeConfig, PresetTemplate } from "../types";
import { triggerHaptic } from "../utils/native";
import {
  applyApiProfileToNode,
  cloneNormalizedConfig,
  resolveNodeApiProfileId,
  syncConfigWithApiProfiles,
} from "../utils/config";
import { randomAvatarSeed, seedFromString } from "../utils/avatarSvg";
import { DEFAULT_BASE_URL, DEFAULT_MODEL_NAME } from "../constants/providers";
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
import {
  applyTagLocksToMechanics,
  toggleExperimentTag,
} from "../constants/experimentTags";
import type { MechanicKey } from "../constants/mechanicDescriptions";

const props = defineProps<{
  open: boolean;
  experimentTitle?: string;
  config: ExperimentConfig | null;
  characterCards?: CharacterCard[];
  apiProfiles: ApiProfile[];
  blindGuessMode?: boolean;
}>();

const emit = defineEmits<{
  close: [];
  save: [config: ExperimentConfig];
  savePreset: [preset: PresetTemplate];
  deleteCharacterCard: [cardId: string];
  saveCharacterCard: [payload: { agentId: string; label: string; systemPrompt: string }];
  notify: [message: string];
}>();

const draft = ref<ExperimentConfig | null>(null);
const initialSnapshot = ref("");
const inheritCardMemory = ref(false);
const timedCluesOpen = ref(false);
const saveAsPresetOpen = ref(false);
const newPresetName = ref("");
const confirmDiscardOpen = ref(false);
const activeTab = ref<"basic" | "rules">("basic");
const editingAgentIdx = ref<number | null>(null);
const isNewRoleDraft = ref(false);
const avatarSeeds = ref<Record<string, number>>({});
const editingDraft = ref<{
  label: string;
  apiProfileId: string;
  temperature: number;
  thinkingLevel: string;
  systemPrompt: string;
  secretGoal: string;
  avatarSeed: number;
}>({
  label: "",
  apiProfileId: "",
  temperature: 0.9,
  thinkingLevel: "default",
  systemPrompt: "",
  secretGoal: "",
  avatarSeed: 1,
});

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen && props.config) {
      const cloned = cloneNormalizedConfig(props.config);
      cloned.agents.forEach((a) => {
        a.apiProfileId = resolveNodeApiProfileId(a, props.apiProfiles);
      });
      if (!cloned.timedClues) {
        cloned.timedClues = [];
      }
      draft.value = cloned;
      timedCluesOpen.value = false;
      initialSnapshot.value = JSON.stringify(cloned);
      saveAsPresetOpen.value = false;
      newPresetName.value = "";
      confirmDiscardOpen.value = false;
      activeTab.value = "basic";
      editingAgentIdx.value = null;
      isNewRoleDraft.value = false;
    }
  },
  { immediate: true }
);

function getRoleAvatarSeed(ag: NodeConfig): number {
  if (avatarSeeds.value[ag.id] !== undefined) {
    return avatarSeeds.value[ag.id];
  }
  return seedFromString(`${ag.id}-${ag.label}`);
}

function openRoleEditor(idx: number) {
  if (!draft.value) return;
  const ag = draft.value.agents[idx];
  if (!ag) return;
  isNewRoleDraft.value = false;
  editingAgentIdx.value = idx;
  editingDraft.value = {
    label: ag.label,
    apiProfileId: ag.apiProfileId || props.apiProfiles[0]?.id || "",
    temperature: ag.temperature ?? 0.9,
    thinkingLevel: String(ag.thinkingLevel || "default"),
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
  if (!draft.value || editingAgentIdx.value === null) return;
  if (isNewRoleDraft.value) {
    const prof = props.apiProfiles.find((p) => p.id === editingDraft.value.apiProfileId) ?? props.apiProfiles[0];
    const first = draft.value.agents[0];
    const newId = `ai-${Date.now().toString(36)}-${draft.value.agents.length + 1}`;
    const newNode: NodeConfig = {
      id: newId,
      label: editingDraft.value.label.trim() || nextUniqueAgentLabel(draft.value.agents),
      apiProfileId: prof?.id ?? null,
      baseUrl: prof?.baseUrl || first?.baseUrl || DEFAULT_BASE_URL,
      apiKey: prof?.apiKey || first?.apiKey || "",
      modelName: prof?.modelName || first?.modelName || DEFAULT_MODEL_NAME,
      temperature: editingDraft.value.temperature,
      thinkingLevel: editingDraft.value.thinkingLevel,
      systemPrompt: editingDraft.value.systemPrompt,
      secretGoal: editingDraft.value.secretGoal.trim(),
    };
    avatarSeeds.value[newId] = editingDraft.value.avatarSeed;
    draft.value.agents.push(newNode);
    onCustomPromptEdit();
    closeRoleEditor();
    return;
  }
  const ag = draft.value.agents[editingAgentIdx.value];
  if (!ag) {
    closeRoleEditor();
    return;
  }
  ag.label = editingDraft.value.label;
  ag.temperature = editingDraft.value.temperature;
  ag.thinkingLevel = editingDraft.value.thinkingLevel;
  ag.systemPrompt = editingDraft.value.systemPrompt;
  ag.secretGoal = editingDraft.value.secretGoal.trim();
  avatarSeeds.value[ag.id] = editingDraft.value.avatarSeed;
  if (editingDraft.value.apiProfileId) {
    selectAgentProfile(ag, editingDraft.value.apiProfileId);
  }
  onCustomPromptEdit();
  closeRoleEditor();
}

function addAgentAndEdit() {
  if (!draft.value) return;
  isNewRoleDraft.value = true;
  editingAgentIdx.value = draft.value.agents.length;
  editingDraft.value = {
    label: nextUniqueAgentLabel(draft.value.agents),
    apiProfileId: props.apiProfiles[0]?.id || "",
    temperature: 0.9,
    thinkingLevel: "default",
    systemPrompt: "",
    secretGoal: "",
    avatarSeed: randomAvatarSeed(),
  };
}

function applyCardToEditor(card: CharacterCard) {
  if (!draft.value || editingAgentIdx.value === null) return;
  editingDraft.value.label = disambiguateAgentLabel(
    card.name,
    draft.value.agents,
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

const thinkingLevelOptions = [
  { value: "default", label: "模型默认" },
  { value: "off", label: "关闭" },
  { value: "low", label: "低" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" },
] as const;

function selectAgentProfile(ag: NodeConfig, profileId: string) {
  const prof = props.apiProfiles.find((p) => p.id === profileId) ?? props.apiProfiles[0];
  if (prof) {
    applyApiProfileToNode(ag, prof);
  }
}

const duplicateLabels = computed(() =>
  draft.value ? duplicateLabelSet(draft.value.agents) : new Set<string>()
);

const isDirty = computed(
  () => Boolean(draft.value) && JSON.stringify(draft.value) !== initialSnapshot.value
);

function openSaveAsPreset() {
  newPresetName.value = "";
  saveAsPresetOpen.value = true;
}

function confirmSaveAsPreset() {
  if (!draft.value) return;
  const currentAgents = draft.value.agents;
  const preset: PresetTemplate = {
    id: "",
    name: newPresetName.value.trim() || "新建自定义剧本",
    description: "由当前推演保存的剧本预设",
    globalSystemPrompt: draft.value.globalSystemPrompt ?? "",
    initialTopic: draft.value.initialTopic ?? "",
    tags: [...(draft.value.tags ?? [])],
    agentPrompts: currentAgents.map((a) => ({
      label: a.label,
      systemPrompt: a.systemPrompt,
    })),
    isBuiltin: false,
    nodeALabel: currentAgents[0]?.label || "角色 1",
    nodeAPrompt: currentAgents[0]?.systemPrompt || "",
    nodeBLabel: currentAgents[1]?.label || "角色 2",
    nodeBPrompt: currentAgents[1]?.systemPrompt || "",
  };
  emit("savePreset", preset);
  saveAsPresetOpen.value = false;
  newPresetName.value = "";
  emit("notify", `已存为预设「${preset.name}」`);
}

function onCustomPromptEdit() {
  if (!draft.value) return;
  if (draft.value.presetId === "free") {
    draft.value.presetId = "custom";
  }
}

function removeAgent(idx: number) {
  if (!draft.value || draft.value.agents.length <= 2) return;
  draft.value.agents.splice(idx, 1);
}

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

function handleSave() {
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
    confirmDiscardOpen.value = false;
    emit("notify", `角色名称「${dup}」存在重复，请修改后再保存`);
    return;
  }
  draft.value.agents.forEach((a, idx) => {
    a.label = effectiveAgentLabel(a, idx);
  });
  const synced = syncConfigWithApiProfiles(draft.value, props.apiProfiles);
  confirmDiscardOpen.value = false;
  triggerHaptic("confirm");
  emit("save", synced);
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
    title="本局剧本与角色"
    max-width-class="max-w-[660px]"
    content-class="md:h-[min(82dvh,680px)]"
    mobile-presentation="fullscreen"
    @close="requestClose"
  >
    <template #header>
      <div
        v-if="draft"
        class="px-4 sm:px-6 py-3.5 border-b border-black/[0.06] bg-white flex items-center justify-between gap-3 shrink-0"
      >
        <div class="flex items-center gap-2.5 min-w-0">
          <div class="w-8 h-8 rounded-xl bg-[#F2F2F7] text-[#1D1D1F] flex items-center justify-center shrink-0">
            <ScrollText class="w-4 h-4" />
          </div>
          <div class="min-w-0">
            <h2 class="text-[15.5px] md:text-[14.5px] font-semibold text-[#1D1D1F] truncate">
              本局剧本与角色
            </h2>
            <p class="text-[11.5px] text-slate-400 truncate">
              {{ experimentTitle ? `当前推演：${experimentTitle}` : "仅修改本局设定，切换预设请新建推演" }}
            </p>
          </div>
        </div>

        <div class="flex items-center gap-1.5 shrink-0">
          <button
            type="button"
            @click="openSaveAsPreset"
            class="h-8 flex items-center gap-1.5 px-2.5 rounded-lg text-[12px] font-medium text-slate-600 bg-[#F2F2F7] md:bg-transparent hover:text-[#1D1D1F] hover:bg-black/[0.05] transition outline-none"
          >
            <Save class="w-3.5 h-3.5" />
            <span class="hidden sm:inline">存为预设</span>
            <span class="sm:hidden">存预设</span>
          </button>
          <button
            type="button"
            @click="requestClose"
            class="w-8 h-8 flex items-center justify-center rounded-full md:rounded-lg bg-[#F2F2F7] md:bg-transparent text-slate-500 hover:text-[#1D1D1F] hover:bg-black/[0.05] transition outline-none"
          >
            <X class="w-4 h-4" />
          </button>
        </div>
      </div>
    </template>

    <div v-if="draft" class="relative flex-1 min-h-0 flex flex-col bg-white overflow-hidden">
      <!-- Two-Tab Bar -->
      <div class="flex items-stretch gap-6 px-4 sm:px-6 h-[43px] border-b border-[#ECECEF] shrink-0">
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
          @click="activeTab = 'rules'"
          class="relative flex items-center gap-1.5 text-[13px] transition-colors outline-none"
          :class="activeTab === 'rules' ? 'text-[#292A30] font-semibold' : 'text-[#82838C] font-medium hover:text-[#292A30]'"
        >
          <span>规则与开场</span>
          <span
            v-if="(draft.tags?.length ?? 0) > 0"
            class="text-[10px] text-[#777780] bg-[#F0F0F2] px-1.5 py-0.5 rounded-[5px] font-medium"
          >
            {{ draft.tags?.length }}
          </span>
          <span
            v-if="activeTab === 'rules'"
            class="absolute bottom-0 left-0 right-0 h-[2px] bg-[#2C2C32] rounded-full"
          ></span>
        </button>
      </div>

      <div class="flex-1 min-h-0 overflow-y-auto px-4 py-5 sm:px-6">
        <div class="space-y-5">
          <div
            v-if="saveAsPresetOpen"
            class="p-3 rounded-xl bg-[#F7F7F8] border border-black/[0.07] flex flex-wrap items-center gap-2"
          >
            <input
              v-model="newPresetName"
              @keydown.enter.prevent="confirmSaveAsPreset"
              placeholder="输入预设名称（例如：深夜便利店推理）"
              class="flex-1 min-w-[180px] rounded-lg border border-black/[0.08] bg-white px-3 py-1.5 text-[13px] text-[#1D1D1F] placeholder:text-slate-400 outline-none focus:ring-2 focus:ring-black/10"
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

          <!-- TAB 1: 基础设定 -->
          <template v-if="activeTab === 'basic'">
            <!-- Scene & Rules -->
            <section>
              <div class="flex items-center gap-2 mb-2.5">
                <h3 class="text-[14px] font-semibold text-[#202222]">场景与文风规则</h3>
                <span class="text-[11px] text-[#AAAAAA]">对所有角色生效</span>
              </div>
              <div class="flex items-start gap-2.5 px-3 py-2.5 rounded-[9px] bg-[#FAFAFA] border border-[#EFEFF0] focus-within:border-[#007AFF]/50 focus-within:bg-white transition">
                <BookOpen class="w-4 h-4 text-[#A0A2A8] mt-0.5 shrink-0" />
                <textarea
                  v-model="draft.globalSystemPrompt"
                  @input="onCustomPromptEdit"
                  rows="3"
                  placeholder="描述场景背景、文风或发言字数要求（可选）"
                  class="w-full resize-y min-h-[54px] bg-transparent text-[12.5px] text-[#202222] placeholder:text-[#A0A1A8] leading-relaxed outline-none"
                ></textarea>
              </div>
            </section>

            <!-- Roles 2-Column Card Grid -->
            <section>
              <div class="flex items-center gap-2 mb-2.5">
                <h3 class="text-[14px] font-semibold text-[#202222]">参与角色</h3>
                <span class="text-[11px] text-[#92949B] bg-[#F2F2F4] px-2 py-0.5 rounded-[6px] font-medium">
                  {{ draft.agents.length }} 位
                </span>
                <div class="flex-1"></div>
                <button
                  type="button"
                  @click="addAgentAndEdit"
                  class="inline-flex items-center gap-1 px-2 py-1 rounded-[6px] text-[12px] text-[#686B73] hover:bg-[#F2F2F5] hover:text-[#202222] transition"
                >
                  <Plus class="w-3.5 h-3.5" />
                  <span>添加角色</span>
                </button>
              </div>

              <div class="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
                <article
                  v-for="(ag, idx) in draft.agents"
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
                      <div class="flex items-center gap-1.5 min-w-0">
                        <span class="text-[13px] font-semibold text-[#202222] truncate">
                          {{ effectiveAgentLabel(ag, idx) }}
                        </span>
                        <span
                          v-if="!blindGuessMode && ag.goalAchieved"
                          class="px-1.5 py-0.5 rounded-full bg-amber-100 text-amber-800 text-[10px] font-medium shrink-0"
                          title="该角色已达成暗线胜负目标"
                        >
                          🏆 已达成
                        </span>
                      </div>
                      <div class="text-[11px] text-[#7C7F89] mt-0.5 truncate">
                        {{ apiProfiles.find((p) => p.id === ag.apiProfileId)?.name ?? "默认" }} · {{ ag.modelName }}
                      </div>
                    </div>
                    <button
                      v-if="draft.agents.length > 2"
                      type="button"
                      @click.stop="removeAgent(idx)"
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

            <!-- Timed Clues (hidden in blindGuessMode) -->
            <section v-if="!blindGuessMode" class="pt-3 border-t border-[#F0F0F2]">
              <button
                type="button"
                @click="timedCluesOpen = !timedCluesOpen"
                class="w-full flex items-center justify-between text-left"
              >
                <span class="text-[13px] font-semibold text-[#202222]">
                  定时线索（分幕物证）
                  <span
                    v-if="(draft.timedClues?.length ?? 0) > 0"
                    class="ml-1 text-[11px] font-normal text-slate-400"
                  >
                    ({{ draft.timedClues?.length }} 条)
                  </span>
                </span>
                <span class="text-[11.5px] text-slate-400">{{ timedCluesOpen ? "收起" : "展开" }}</span>
              </button>

              <div v-if="timedCluesOpen" class="mt-2.5 space-y-2">
                <div
                  v-for="(clue, cIdx) in (draft.timedClues ?? [])"
                  :key="cIdx"
                  class="flex items-center gap-2"
                >
                  <span
                    class="px-1.5 py-0.5 rounded text-[10px] font-medium shrink-0"
                    :class="clue.triggered ? 'bg-emerald-100 text-emerald-700' : 'bg-slate-100 text-slate-500'"
                  >
                    {{ clue.triggered ? "已释放" : "待触发" }}
                  </span>
                  <span class="text-[11px] text-slate-500 shrink-0">第</span>
                  <input
                    v-model.number="clue.triggerTurn"
                    :disabled="Boolean(clue.triggered)"
                    type="number"
                    min="2"
                    max="200"
                    class="w-14 h-7 rounded-md border border-[#E2E2E6] bg-white disabled:bg-slate-50 px-1.5 text-[12px] text-center outline-none"
                  />
                  <span class="text-[11px] text-slate-500 shrink-0">轮</span>
                  <input
                    v-model="clue.content"
                    :disabled="Boolean(clue.triggered)"
                    type="text"
                    placeholder="输入届时释放的客观物证或突发线索…"
                    class="flex-1 h-7 rounded-md border border-[#E2E2E6] bg-white disabled:bg-slate-50 px-2 text-[12px] outline-none"
                  />
                  <button
                    v-if="!clue.triggered"
                    type="button"
                    @click="draft.timedClues?.splice(cIdx, 1)"
                    class="text-[11px] text-slate-400 hover:text-rose-600 px-1.5"
                  >
                    删除
                  </button>
                </div>
                <button
                  type="button"
                  @click="
                    if (!draft.timedClues) draft.timedClues = [];
                    draft.timedClues.push({ triggerTurn: 6 + draft.timedClues.length * 4, content: '', triggered: false });
                  "
                  class="text-[11.5px] font-medium text-[#007AFF] hover:underline"
                >
                  + 添加定时线索
                </button>
              </div>
            </section>
          </template>

          <!-- TAB 2: 规则与开场 -->
          <template v-else>
            <ExperimentTagsPicker
              :tags="draft.tags ?? []"
              @toggle="handleToggleTag"
              @clear="handleClearTags"
            />

            <div class="pt-4 border-t border-[#F0F0F2] space-y-2">
              <div class="flex items-center justify-between gap-2">
                <label class="text-[13px] font-semibold text-[#202222]">开场设定</label>
                <span class="text-[11px] text-[#AAAAAA]">可选，留空直接开聊</span>
              </div>
              <textarea
                v-model="draft.initialTopic"
                rows="3"
                placeholder="留空则不设开场内容，由角色直接开始对话"
                class="w-full resize-none rounded-[9px] border border-[#E9E9ED] bg-[#FAFAFA] focus:bg-white px-3.5 py-2.5 text-[13px] text-[#202222] placeholder:text-[#A0A1A8] leading-relaxed outline-none transition"
              ></textarea>
            </div>

            <div
              v-if="draft.presetId === 'free'"
              class="p-3.5 rounded-xl bg-emerald-50/70 border border-emerald-200/60 text-[12.5px] text-emerald-900 flex gap-2.5"
            >
              <Sparkles class="w-4 h-4 shrink-0 mt-0.5 text-emerald-600" />
              <div>
                <div class="font-semibold mb-0.5">当前为「自由漫谈」模式</div>
                <div class="opacity-85 leading-relaxed">
                  已跳过全局演戏规则与白板工具，角色将直接开始自由对话。
                </div>
              </div>
            </div>
          </template>
        </div>
      </div>

      <!-- Role Editor Sub-Dialog -->
      <div
        v-if="editingAgentIdx !== null"
        class="absolute inset-0 z-30 bg-[#171820]/30 backdrop-blur-[2px] grid place-items-center p-4"
        @click.self="closeRoleEditor"
      >
        <div class="w-full max-w-[460px] rounded-[15px] bg-white border border-[#EDEDF0] shadow-[0_15px_60px_rgba(0,0,0,0.2)] p-5 space-y-3.5">
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
            <div class="flex items-center gap-1.5">
              <CharacterLibraryPopover
                v-if="characterCards && characterCards.length > 0"
                :cards="characterCards"
                :inherit-memory="inheritCardMemory"
                :allow-delete="true"
                :show-count="false"
                :compact="true"
                trigger-label="角色库"
                @update:inherit-memory="inheritCardMemory = $event"
                @select="applyCardToEditor"
                @delete="emit('deleteCharacterCard', $event)"
              />
              <button
                v-if="!isNewRoleDraft && draft.agents[editingAgentIdx]"
                type="button"
                @click="
                  triggerHaptic('light');
                  emit('saveCharacterCard', {
                    agentId: draft.agents[editingAgentIdx].id,
                    label: editingDraft.label.trim() || effectiveAgentLabel(draft.agents[editingAgentIdx], editingAgentIdx),
                    systemPrompt: editingDraft.systemPrompt,
                  });
                "
                class="h-8 px-2.5 flex items-center gap-1 rounded-lg text-[12px] font-medium bg-[#F5F5F7] hover:bg-[var(--ds-accent-soft)] text-slate-600 hover:text-[var(--ds-accent)] border border-black/[0.08] transition"
                title="保存到角色库"
              >
                <BookmarkPlus class="w-3.5 h-3.5" />
                <span>存角色库</span>
              </button>
            </div>
          </div>

          <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
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
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="block text-[12px] text-[#757780] mb-1.5">Temperature</label>
              <input
                v-model.number="editingDraft.temperature"
                type="number"
                min="0"
                max="1.5"
                step="0.05"
                class="w-full h-9 rounded-[8px] border border-[#E2E2E6] bg-[#FAFAFA] focus:bg-white px-3 text-[13px] font-mono outline-none"
              />
            </div>
            <div>
              <label class="block text-[12px] text-[#757780] mb-1.5">思考深度</label>
              <select
                v-model="editingDraft.thinkingLevel"
                :disabled="draft.intuitiveFastReply"
                class="w-full h-9 rounded-[8px] border border-[#E2E2E6] bg-[#FAFAFA] focus:bg-white px-2.5 text-[13px] outline-none disabled:opacity-55"
              >
                <option v-for="opt in thinkingLevelOptions" :key="opt.value" :value="opt.value">
                  {{ opt.label }}
                </option>
              </select>
            </div>
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

          <div v-if="!blindGuessMode">
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

      <!-- Unsaved Changes Confirmation Dialog -->
      <AdaptiveModal
        :open="confirmDiscardOpen"
        title="尚未保存修改"
        max-width-class="max-w-[360px]"
        @close="confirmDiscardOpen = false"
      >
        <div class="p-5 space-y-4">
          <div class="space-y-1.5">
            <h3 class="text-[15px] font-semibold text-[#1D1D1F]">尚未保存修改</h3>
            <p class="text-[13px] text-slate-500 leading-relaxed">
              当前设定中有未保存的变更，是否保存后再关闭？
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
              class="h-8 px-3 rounded-lg text-[12.5px] font-medium text-[var(--ds-danger)] hover:bg-[var(--ds-danger-soft)] transition outline-none"
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
      </AdaptiveModal>
    </div>

    <template #footer>
      <div
        v-if="draft"
        class="shrink-0 px-4 md:px-6 py-3 bg-white"
      >
        <div class="max-w-[580px] mx-auto grid grid-cols-[1fr_2fr] min-[520px]:flex min-[520px]:items-center min-[520px]:justify-end gap-2.5">
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
            保存设定
          </button>
        </div>
      </div>
    </template>
  </AdaptiveModal>
</template>
