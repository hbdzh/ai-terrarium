<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { X, GitBranch, Play, Check, Plus, Trash2 } from "lucide-vue-next";
import AdaptiveModal from "./ui/AdaptiveModal.vue";
import CharacterLibraryPopover from "./CharacterLibraryPopover.vue";
import ExperimentTagsPicker from "./ExperimentTagsPicker.vue";
import {
  applyTagLocksToMechanics,
  toggleExperimentTag,
} from "../constants/experimentTags";
import type { MechanicKey } from "../constants/mechanicDescriptions";
import {
  disambiguateAgentLabel,
  duplicateLabelSet,
  effectiveAgentLabel,
  findDuplicateAgentLabel,
  nextUniqueAgentLabel,
} from "../utils/agents";
import type { CharacterCard, ExperimentConfig, NodeConfig } from "../types";

const props = defineProps<{
  open: boolean;
  title: string;
  config: ExperimentConfig | null;
  characterCards?: CharacterCard[];
}>();

const emit = defineEmits<{
  close: [];
  saveAndContinue: [payload: { title: string; config: ExperimentConfig; autoStep: boolean }];
}>();

const draftTitle = ref("");
const draftConfig = ref<ExperimentConfig | null>(null);
const inheritCardMemory = ref(true);
const errorText = ref<string | null>(null);

watch(
  () => [props.open, props.config, props.title] as const,
  ([isOpen, cfg, t]) => {
    if (isOpen && cfg) {
      draftTitle.value = t;
      const cloned: ExperimentConfig = JSON.parse(JSON.stringify(cfg));
      cloned.tags = Array.isArray(cloned.tags) ? [...cloned.tags] : [];
      draftConfig.value = cloned;
      errorText.value = null;
    }
  },
  { immediate: true }
);

const duplicateLabels = computed(() =>
  draftConfig.value ? duplicateLabelSet(draftConfig.value.agents) : new Set<string>()
);

function handleToggleTag(tagId: string) {
  if (!draftConfig.value) return;
  const currentMech: Record<MechanicKey, boolean> = {
    intuitiveFastReply: Boolean(draftConfig.value.intuitiveFastReply),
    allowPassTurn: Boolean(draftConfig.value.allowPassTurn),
    randomMoodDice: Boolean(draftConfig.value.randomMoodDice),
    enablePrivateNotes: Boolean(draftConfig.value.enablePrivateNotes),
    enableBoardTools: draftConfig.value.enableBoardTools ?? true,
    enableSmartSelector: draftConfig.value.enableSmartSelector ?? true,
  };
  const res = toggleExperimentTag(draftConfig.value.tags ?? [], tagId, currentMech);
  draftConfig.value.tags = res.tags;
  draftConfig.value.intuitiveFastReply = res.mechanics.intuitiveFastReply;
  draftConfig.value.allowPassTurn = res.mechanics.allowPassTurn;
  draftConfig.value.randomMoodDice = res.mechanics.randomMoodDice;
  draftConfig.value.enablePrivateNotes = res.mechanics.enablePrivateNotes;
  draftConfig.value.enableBoardTools = res.mechanics.enableBoardTools;
  draftConfig.value.enableSmartSelector = res.mechanics.enableSmartSelector;
}

function handleClearTags() {
  if (!draftConfig.value) return;
  draftConfig.value.tags = [];
}

function applyCard(idx: number, card: CharacterCard) {
  if (!draftConfig.value) return;
  const target = draftConfig.value.agents[idx];
  if (!target) return;
  target.label = disambiguateAgentLabel(card.name, draftConfig.value.agents, idx);
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

function addAgent() {
  if (!draftConfig.value) return;
  const first = draftConfig.value.agents[0];
  const label = nextUniqueAgentLabel(draftConfig.value.agents);
  const newNode: NodeConfig = {
    id: `ai-${Date.now().toString(36)}-${draftConfig.value.agents.length + 1}`,
    label,
    baseUrl: first?.baseUrl || "https://api.deepseek.com/v1",
    apiKey: first?.apiKey || "",
    modelName: first?.modelName || "deepseek-chat",
    temperature: first?.temperature ?? 0.9,
    systemPrompt: "",
  };
  draftConfig.value.agents.push(newNode);
}

function removeAgent(idx: number) {
  if (!draftConfig.value || draftConfig.value.agents.length <= 2) return;
  draftConfig.value.agents.splice(idx, 1);
}

function submit(autoStep: boolean) {
  if (!draftConfig.value) return;
  const dup = findDuplicateAgentLabel(draftConfig.value.agents);
  if (dup) {
    errorText.value = `角色名称「${dup}」存在重复，请修改以区分`;
    return;
  }
  draftConfig.value.agents.forEach((a, idx) => {
    a.label = effectiveAgentLabel(a, idx);
  });
  const patched = applyTagLocksToMechanics(draftConfig.value.tags ?? [], {
    intuitiveFastReply: Boolean(draftConfig.value.intuitiveFastReply),
    allowPassTurn: Boolean(draftConfig.value.allowPassTurn),
    randomMoodDice: Boolean(draftConfig.value.randomMoodDice),
    enablePrivateNotes: Boolean(draftConfig.value.enablePrivateNotes),
    enableBoardTools: draftConfig.value.enableBoardTools ?? true,
    enableSmartSelector: draftConfig.value.enableSmartSelector ?? true,
  });
  Object.assign(draftConfig.value, patched);

  emit("saveAndContinue", {
    title: draftTitle.value.trim() || props.title,
    config: draftConfig.value,
    autoStep,
  });
}
</script>

<template>
  <AdaptiveModal
    :open="open && Boolean(draftConfig)"
    title="平行宇宙分支变量微调"
    max-width-class="max-w-2xl"
    mobile-presentation="fullscreen"
    @close="emit('close')"
  >
    <template #header>
      <div class="px-5 py-3.5 border-b border-black/[0.06] flex items-center justify-between gap-2 bg-white shrink-0">
        <div class="flex items-center gap-2 min-w-0">
          <div class="w-7 h-7 rounded-xl bg-violet-50 text-violet-600 flex items-center justify-center shrink-0">
            <GitBranch class="w-4 h-4" />
          </div>
          <div class="min-w-0">
            <h3 class="text-[14px] font-semibold text-[#1D1D1F] truncate">平行宇宙分支微调</h3>
            <p class="text-[11px] text-slate-400 truncate">可替换角色、改写底牌、切换模型或标签后继续推演</p>
          </div>
        </div>
        <button
          type="button"
          @click="emit('close')"
          class="w-8 h-8 flex items-center justify-center rounded-full hover:bg-slate-100 text-slate-400 hover:text-slate-700"
        >
          <X class="w-4 h-4" />
        </button>
      </div>
    </template>

    <div v-if="draftConfig" class="flex-1 overflow-y-auto p-4 sm:p-5 space-y-4 bg-[#F5F5F7]">
      <div v-if="errorText" class="px-3.5 py-2 rounded-xl bg-rose-50 border border-rose-200 text-xs text-rose-700">
        {{ errorText }}
      </div>

      <div class="rounded-2xl bg-white p-4 border border-black/[0.05] space-y-3">
        <div>
          <label class="block text-[12px] font-medium text-slate-600 mb-1">分支实验标题</label>
          <input
            v-model="draftTitle"
            class="w-full h-9 rounded-xl border border-black/[0.08] bg-[#F5F5F7] focus:bg-white px-3 text-[13.5px] font-medium text-[#1D1D1F] outline-none"
          />
        </div>
        <div class="pt-2 border-t border-black/[0.05]">
          <ExperimentTagsPicker
            :tags="draftConfig.tags ?? []"
            :compact="true"
            @toggle="handleToggleTag"
            @clear="handleClearTags"
          />
        </div>
      </div>

      <div class="space-y-2.5">
        <div class="flex items-center justify-between px-0.5">
          <span class="text-[12.5px] font-semibold text-[#1D1D1F]">
            角色变量（{{ draftConfig.agents.length }} 位）
          </span>
          <button
            type="button"
            @click="addAgent"
            class="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-white border border-black/[0.08] text-[11.5px] font-medium text-[#1D1D1F] hover:bg-black/[0.04]"
          >
            <Plus class="w-3 h-3" />
            <span>新增角色</span>
          </button>
        </div>

        <div
          v-for="(ag, idx) in draftConfig.agents"
          :key="ag.id"
          class="rounded-2xl bg-white p-4 border border-black/[0.05] space-y-2.5"
        >
          <div class="flex flex-wrap items-center justify-between gap-2">
            <div class="flex items-center gap-2 flex-1 min-w-0">
              <span class="px-2 py-0.5 rounded-md bg-[#F2F2F7] text-[11px] font-mono text-slate-500">#{{ idx + 1 }}</span>
              <input
                v-model="ag.label"
                placeholder="角色名称"
                class="flex-1 min-w-0 h-9 rounded-xl border bg-[#F5F5F7] focus:bg-white px-3 text-[13px] font-semibold text-[#1D1D1F] outline-none"
                :class="duplicateLabels.has(effectiveAgentLabel(ag, idx)) ? 'border-rose-400' : 'border-black/[0.08]'"
              />
            </div>
            <div class="flex items-center gap-1.5 shrink-0">
              <CharacterLibraryPopover
                v-if="characterCards && characterCards.length > 0"
                :cards="characterCards"
                :inherit-memory="inheritCardMemory"
                :show-count="false"
                trigger-label="从灵魂库替换"
                @update:inherit-memory="inheritCardMemory = $event"
                @select="applyCard(idx, $event)"
              />
              <button
                v-if="draftConfig.agents.length > 2"
                type="button"
                @click="removeAgent(idx)"
                class="w-9 h-9 flex items-center justify-center rounded-xl text-slate-400 hover:text-rose-600 hover:bg-rose-50"
              >
                <Trash2 class="w-4 h-4" />
              </button>
            </div>
          </div>

          <div class="grid grid-cols-1 sm:grid-cols-2 gap-2">
            <input
              v-model="ag.modelName"
              placeholder="Model Name"
              class="h-8 rounded-lg border border-black/[0.08] bg-[#F7F7FA] focus:bg-white px-2.5 text-[12px] font-mono text-[#1D1D1F] outline-none"
            />
            <input
              v-model="ag.baseUrl"
              placeholder="Base URL"
              class="h-8 rounded-lg border border-black/[0.08] bg-[#F7F7FA] focus:bg-white px-2.5 text-[12px] font-mono text-[#1D1D1F] outline-none"
            />
          </div>

          <textarea
            v-model="ag.systemPrompt"
            rows="2"
            placeholder="改写该分支下的角色底牌或立场..."
            class="w-full resize-none rounded-xl border border-black/[0.08] bg-[#F5F5F7] focus:bg-white px-3 py-2 text-[12.5px] leading-relaxed text-[#1D1D1F] outline-none"
          ></textarea>
        </div>
      </div>
    </div>

    <template #footer>
      <div class="px-5 py-3 border-t border-black/[0.06] bg-white flex flex-wrap items-center justify-end gap-2 shrink-0">
        <button
          type="button"
          @click="emit('close')"
          class="h-9 px-3.5 rounded-xl text-[12.5px] font-medium text-slate-600 hover:bg-black/[0.05]"
        >
          保持原配置
        </button>
        <button
          type="button"
          @click="submit(false)"
          class="h-9 flex items-center gap-1.5 px-4 rounded-xl text-[12.5px] font-medium bg-[#F2F2F7] hover:bg-[#E5E5EA] text-[#1D1D1F]"
        >
          <Check class="w-3.5 h-3.5" />
          <span>保存微调</span>
        </button>
        <button
          type="button"
          @click="submit(true)"
          class="h-9 flex items-center gap-1.5 px-4 rounded-xl text-[12.5px] font-semibold bg-[#1D1D1F] hover:bg-black text-white"
        >
          <Play class="w-3.5 h-3.5 fill-current" />
          <span>保存并推演一轮</span>
        </button>
      </div>
    </template>
  </AdaptiveModal>
</template>
