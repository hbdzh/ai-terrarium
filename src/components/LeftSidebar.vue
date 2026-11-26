<script setup lang="ts">
import { computed, ref } from "vue";
import {
  Search,
  X,
  Trash2,
  GitBranch,
  Upload,
  FileJson,
  Download,
  BookUser,
  Settings,
  FolderInput,
  ChevronUp,
  SquarePen,
  Sparkles,
  Image as ImageIcon,
  Printer,
  FileText,
  Pencil,
  Check,
  RefreshCw,
} from "lucide-vue-next";
import type { CharacterCard, ExperimentSummary } from "../types";
import { isMacOS, isWindows, isDesktopOS, isMobileOS } from "../utils/platform";
import AdaptiveModal from "./ui/AdaptiveModal.vue";

const props = defineProps<{
  experiments: ExperimentSummary[];
  currentExperimentId: string | null;
  currentTotalTurns?: number;
  distillingImprints?: boolean;
  characterCards: CharacterCard[];
  inSheet?: boolean;
}>();

const emit = defineEmits<{
  openNewExperiment: [];
  selectExperiment: [id: string];
  deleteExperiment: [id: string];
  exportScenario: [];
  importScenario: [];
  exportMd: [];
  exportPureMd: [];
  exportPurePng: [];
  exportPurePdf: [];
  exportReplayMd: [];
  exportReplayPdf: [];
  distillImprints: [];
  updateCardImprints: [payload: { cardId: string; memoryImprints: string[] }];
  deleteCharacterCard: [cardId: string];
  openSettings: [];
}>();

const searchQuery = ref("");
const cardModalOpen = ref(false);
const ioMenuOpen = ref(false);
const ioTriggerRef = ref<HTMLButtonElement | null>(null);
const ioMenuPos = ref({ left: 10, bottom: 52 });

function toggleDesktopIoMenu() {
  if (!ioMenuOpen.value && ioTriggerRef.value) {
    const rect = ioTriggerRef.value.getBoundingClientRect();
    ioMenuPos.value = {
      left: Math.max(8, rect.left),
      bottom: Math.max(8, window.innerHeight - rect.top + 8),
    };
  }
  ioMenuOpen.value = !ioMenuOpen.value;
}
const pendingDeleteItem = ref<ExperimentSummary | null>(null);

// Inline memory imprint editing in Soul Character Library modal
const editingImprintKey = ref<string | null>(null);
const editingImprintDraft = ref("");

function startEditImprint(cardId: string, idx: number, currentVal: string) {
  editingImprintKey.value = `${cardId}:${idx}`;
  editingImprintDraft.value = currentVal;
}

function saveEditImprint(card: CharacterCard, idx: number) {
  const next = [...(card.memoryImprints ?? [])];
  const trimmed = editingImprintDraft.value.trim();
  if (trimmed) {
    next[idx] = trimmed;
  } else {
    next.splice(idx, 1);
  }
  editingImprintKey.value = null;
  emit("updateCardImprints", { cardId: card.id, memoryImprints: next });
}

function deleteImprint(card: CharacterCard, idx: number) {
  const next = [...(card.memoryImprints ?? [])];
  next.splice(idx, 1);
  if (editingImprintKey.value === `${card.id}:${idx}`) {
    editingImprintKey.value = null;
  }
  emit("updateCardImprints", { cardId: card.id, memoryImprints: next });
}

const filteredExperiments = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return props.experiments;
  return props.experiments.filter((item) => item.title.toLowerCase().includes(q));
});

function isDerivedBranch(item: ExperimentSummary): boolean {
  return (
    Boolean(item.parentExperimentId) ||
    /·\s*(分支[A-Z0-9]*|改写重演)/.test(item.title)
  );
}

function triggerImportScenario() {
  ioMenuOpen.value = false;
  emit("importScenario");
}

function formatTok(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(2)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

function requestDelete(id: string) {
  const target = props.experiments.find((e) => e.id === id) ?? null;
  pendingDeleteItem.value = target;
}

function confirmDelete() {
  if (!pendingDeleteItem.value) return;
  const id = pendingDeleteItem.value.id;
  pendingDeleteItem.value = null;
  emit("deleteExperiment", id);
}

function cancelDelete() {
  pendingDeleteItem.value = null;
}
</script>

<template>
  <div
    class="h-full w-full flex flex-col select-none overflow-hidden antialiased"
    :class="
      isWindows && !inSheet
        ? 'bg-transparent'
        : 'bg-[#F9F9FB] border-r border-black/[0.035]'
    "
  >
    <!-- Top New Experiment Row + Lightweight Search (ChatGPT/Codex Sidebar style) -->
    <div
      :data-tauri-drag-region="isMacOS && !inSheet ? '' : undefined"
      class="shrink-0 px-3 md:px-2.5 pb-2 space-y-2 md:space-y-1.5"
      :class="
        isMacOS && !inSheet
          ? 'pt-12'
          : isWindows && !inSheet
            ? 'pt-1.5'
            : inSheet
              ? 'pt-3.5'
              : isMobileOS
                ? 'pt-[calc(3rem+var(--safe-top,env(safe-area-inset-top)))]'
                : 'pt-2.5'
      "
    >
      <button
        v-if="inSheet || !isWindows"
        type="button"
        @click="emit('openNewExperiment')"
        class="w-full h-10 md:h-9 flex items-center gap-2.5 px-3 md:px-2.5 rounded-xl md:rounded-[10px] text-[14px] md:text-[13px] font-medium text-[#1D1D1F] hover:bg-black/[0.045] active:bg-black/[0.07] transition-colors"
        :class="inSheet ? 'bg-white border border-black/[0.06] shadow-[0_1px_2px_rgba(0,0,0,0.02)]' : ''"
        title="新建推演 (Ctrl+N)"
      >
        <SquarePen class="w-4 h-4 text-slate-600 shrink-0" />
        <span>新建推演</span>
      </button>

      <div class="relative flex items-center">
        <Search class="w-3.5 h-3.5 text-slate-400 absolute left-3 md:left-2.5 pointer-events-none" />
        <input
          v-model="searchQuery"
          placeholder="搜索推演记录"
          class="w-full pl-8.5 md:pl-8 pr-7 py-2 md:py-1.5 text-[13.5px] md:text-[13px] text-[#1D1D1F] placeholder:text-slate-400 outline-none transition-all"
          :class="
            inSheet
              ? 'rounded-xl bg-white border border-black/[0.06] focus:border-black/20'
              : 'rounded-[10px] bg-transparent hover:bg-black/[0.035] focus:bg-white/55 border border-transparent focus:border-black/[0.09]'
          "
        />
        <button
          v-if="searchQuery"
          type="button"
          @click="searchQuery = ''"
          class="absolute right-2 p-0.5 rounded-full text-slate-400 hover:text-slate-600"
        >
          <X class="w-3 h-3" />
        </button>
      </div>
    </div>

    <!-- Middle History Experiments List (Single-line minimal items on desktop, rich on touch sheet) -->
    <div class="relative flex-1 min-h-0 flex flex-col">
      <div
        v-if="!(isWindows && !inSheet)"
        class="pointer-events-none absolute inset-x-0 top-0 h-3.5 z-10 bg-gradient-to-b from-[#F9F9FB] via-[#F9F9FB]/80 to-transparent"
      ></div>
      <div class="flex-1 min-h-0 overflow-y-auto px-2.5 py-1.5 space-y-0.5">
      <div class="px-2.5 pt-1.5 pb-1 flex items-center justify-between">
        <span class="text-[11px] font-medium text-slate-400">
          推演记录
        </span>
      </div>

      <div
        v-if="filteredExperiments.length === 0"
        class="py-10 text-center text-[13px] text-slate-400"
      >
        无匹配记录
      </div>

      <TransitionGroup name="list-item" tag="div" class="relative space-y-0.5">
      <div
        v-for="item in filteredExperiments"
        :key="item.id"
        @click="emit('selectExperiment', item.id)"
        class="group relative flex items-center justify-between gap-2 py-2 md:py-1.5 pr-1.5 rounded-xl md:rounded-[10px] cursor-pointer transition-colors"
        :class="[
          isDerivedBranch(item) ? 'pl-5' : 'pl-2.5',
          item.id === currentExperimentId
            ? 'bg-black/[0.055] text-[#1D1D1F]'
            : 'hover:bg-black/[0.035] active:bg-black/[0.055] text-slate-700',
        ]"
        :title="`${item.title} · ${item.totalTurns} 轮 · ${formatTok(item.totalTokens)} tok`"
      >
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-1.5 min-w-0">
            <GitBranch
              v-if="isDerivedBranch(item)"
              class="w-3.5 h-3.5 md:w-3 md:h-3 shrink-0 text-[var(--ds-accent)]"
              title="世界线派生分支"
            />
            <span
              class="text-[14px] md:text-[13px] truncate leading-snug"
              :class="item.id === currentExperimentId ? 'font-medium text-[#1D1D1F]' : 'font-normal'"
            >
              {{ item.title }}
            </span>
          </div>
          <div v-if="inSheet" class="text-[11.5px] text-slate-400 tabular-nums mt-0.5 leading-tight">
            {{ item.totalTurns }} 轮 · {{ formatTok(item.totalTokens) }} tok
          </div>
        </div>

        <button
          v-if="experiments.length > 1"
          type="button"
          @click.stop="requestDelete(item.id)"
          class="shrink-0 opacity-100 lg:opacity-0 lg:group-hover:opacity-100 w-8 h-8 md:w-7 md:h-7 flex items-center justify-center rounded-xl md:rounded-lg hover:bg-[var(--ds-danger-soft)] text-slate-400 hover:text-[var(--ds-danger)] transition-all"
          title="删除推演"
        >
          <Trash2 class="w-4 h-4 md:w-3.5 md:h-3.5" />
        </button>
      </div>
      </TransitionGroup>
      </div>
      <div
        v-if="!(isWindows && !inSheet)"
        class="pointer-events-none absolute inset-x-0 bottom-0 h-4 z-10 bg-gradient-to-t from-[#F9F9FB] via-[#F9F9FB]/85 to-transparent"
      ></div>
    </div>

    <!-- Desktop Compact Single-Row Icon Dock (when !inSheet) -->
    <div
      v-if="!inSheet"
      class="relative shrink-0 px-2.5 py-2 border-t border-black/[0.05] flex items-center justify-between"
      :class="isMobileOS ? 'pb-[calc(0.5rem+var(--safe-bottom,env(safe-area-inset-bottom)))]' : ''"
    >
      <div class="flex items-center gap-1">
        <button
          type="button"
          @click="cardModalOpen = true"
          class="flex items-center justify-center rounded-lg text-slate-600 hover:text-[#1D1D1F] hover:bg-black/[0.05] hover:scale-[1.06] active:scale-[0.95] transition-all duration-150 outline-none"
          :class="isMobileOS ? 'w-9 h-9' : 'w-8 h-8'"
          :title="`角色库 (${characterCards.length})`"
        >
          <BookUser class="w-4 h-4 stroke-[1.75]" />
        </button>

        <button
          type="button"
          @click="emit('distillImprints')"
          :disabled="(currentTotalTurns ?? 0) === 0 || distillingImprints"
          class="flex items-center justify-center rounded-lg text-slate-600 hover:text-[#1D1D1F] hover:bg-black/[0.05] hover:scale-[1.06] active:scale-[0.95] transition-all duration-150 outline-none disabled:opacity-35 disabled:pointer-events-none"
          :class="isMobileOS ? 'w-9 h-9' : 'w-8 h-8'"
          title="总结经历到角色库"
        >
          <RefreshCw v-if="distillingImprints" class="w-4 h-4 text-[var(--ds-accent)] animate-spin" />
          <Sparkles v-else class="w-4 h-4 stroke-[1.75]" />
        </button>

        <div class="relative">
          <Teleport to="body">
            <div
              v-if="ioMenuOpen"
              @click="ioMenuOpen = false"
              class="fixed inset-0 z-[110]"
            ></div>
            <div
              v-if="ioMenuOpen"
              :style="{ left: `${ioMenuPos.left}px`, bottom: `${ioMenuPos.bottom}px` }"
              class="animate-popover fixed z-[120] w-[210px] p-1.5 rounded-2xl bg-white border border-black/[0.08] shadow-[0_12px_32px_rgba(0,0,0,0.14)] space-y-0.5 text-[12.5px] select-none antialiased"
            >
              <button
                type="button"
                @click="triggerImportScenario"
                class="w-full flex items-center gap-2.5 px-3 py-2 rounded-xl text-[12.5px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
              >
                <Upload class="w-3.5 h-3.5 text-slate-500 shrink-0" />
                <span>导入剧本 (.json)</span>
              </button>
              <button
                type="button"
                @click="ioMenuOpen = false; emit('exportScenario')"
                class="w-full flex items-center gap-2.5 px-3 py-2 rounded-xl text-[12.5px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
              >
                <FileJson class="w-3.5 h-3.5 text-slate-500 shrink-0" />
                <span>导出剧本 (.json)</span>
              </button>
              <button
                type="button"
                @click="ioMenuOpen = false; emit('exportMd')"
                class="w-full flex items-center gap-2.5 px-3 py-2 rounded-xl text-[12.5px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
              >
                <Download class="w-3.5 h-3.5 text-slate-500 shrink-0" />
                <span>导出完整记录 (.md)</span>
              </button>
              <div class="my-1 border-t border-black/[0.06]"></div>
              <div class="px-3 py-1 text-[10.5px] font-semibold text-slate-400">
                剧本导出（隐藏思考过程）
              </div>
              <button
                type="button"
                @click="ioMenuOpen = false; emit('exportPureMd')"
                class="w-full flex items-center gap-2.5 px-3 py-1.5 rounded-xl text-[12.5px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
              >
                <FileText class="w-3.5 h-3.5 text-slate-500 shrink-0" />
                <span>剧本 Markdown (.md)</span>
              </button>
              <button
                type="button"
                @click="ioMenuOpen = false; emit('exportPurePng')"
                class="w-full flex items-center gap-2.5 px-3 py-1.5 rounded-xl text-[12.5px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
              >
                <ImageIcon class="w-3.5 h-3.5 text-slate-500 shrink-0" />
                <span>剧本长图 (.png)</span>
              </button>
              <button
                type="button"
                @click="ioMenuOpen = false; emit('exportPurePdf')"
                class="w-full flex items-center gap-2.5 px-3 py-1.5 rounded-xl text-[12.5px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
              >
                <Printer class="w-3.5 h-3.5 text-slate-500 shrink-0" />
                <span>剧本 PDF (.pdf)</span>
              </button>
              <button
                type="button"
                @click="ioMenuOpen = false; emit('exportReplayMd')"
                class="w-full flex items-center gap-2.5 px-3 py-1.5 rounded-xl text-[12.5px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
              >
                <FileText class="w-3.5 h-3.5 text-amber-600 shrink-0" />
                <span>剧本杀复盘手卡 (.md)</span>
              </button>
              <button
                type="button"
                @click="ioMenuOpen = false; emit('exportReplayPdf')"
                class="w-full flex items-center gap-2.5 px-3 py-1.5 rounded-xl text-[12.5px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
              >
                <Printer class="w-3.5 h-3.5 text-amber-600 shrink-0" />
                <span>剧本杀复盘手卡 (.pdf)</span>
              </button>
            </div>
          </Teleport>
          <button
            ref="ioTriggerRef"
            type="button"
            @click="toggleDesktopIoMenu"
            class="flex items-center justify-center rounded-lg hover:scale-[1.06] active:scale-[0.95] transition-all duration-150 outline-none"
            :class="[
              isMobileOS ? 'w-9 h-9' : 'w-8 h-8',
              ioMenuOpen ? 'bg-black/[0.06] text-[#1D1D1F]' : 'text-slate-600 hover:text-[#1D1D1F] hover:bg-black/[0.05]',
            ]"
            title="导入 / 导出"
          >
            <FolderInput class="w-4 h-4 stroke-[1.75]" />
          </button>
        </div>
      </div>

      <button
        type="button"
        @click="emit('openSettings')"
        class="flex items-center justify-center rounded-lg text-slate-600 hover:text-[#1D1D1F] hover:bg-black/[0.05] hover:scale-[1.06] active:scale-[0.95] transition-all duration-150 outline-none"
        :class="isMobileOS ? 'w-9 h-9' : 'w-8 h-8'"
        :title="isMacOS ? '设置 (⌘,)' : isDesktopOS ? '设置 (Ctrl+,)' : '设置'"
      >
        <Settings class="w-4 h-4 stroke-[1.75]" />
      </button>
    </div>

    <!-- Bottom Fixed Action Area (Mobile/Sheet only) -->
    <div
      v-if="inSheet"
      class="relative shrink-0 px-2.5 pt-1.5 space-y-0.5"
      :class="[
        'bg-[#F9F9FB]',
        isMobileOS ? 'pb-[calc(0.65rem+var(--safe-bottom,env(safe-area-inset-bottom)))]' : 'pb-2.5',
      ]"
    >
      <!-- Distill Memory Imprints to Character Library -->
      <button
        type="button"
        @click="emit('distillImprints')"
        :disabled="(currentTotalTurns ?? 0) === 0 || distillingImprints"
        class="w-full h-10 md:h-9 flex items-center gap-2.5 px-2.5 rounded-xl text-[13.5px] md:text-[12.5px] font-medium text-slate-700 hover:bg-black/[0.045] active:bg-black/[0.08] hover:text-[#1D1D1F] transition-colors disabled:opacity-40"
        title="提取本局角色经历并存入角色库"
      >
        <RefreshCw v-if="distillingImprints" class="w-4 h-4 text-indigo-600 animate-spin shrink-0" />
        <Sparkles v-else class="w-4 h-4 text-indigo-600 shrink-0" />
        <span class="truncate">{{ distillingImprints ? "总结经历中..." : "总结经历到角色库" }}</span>
      </button>

      <!-- Character Library -->
      <button
        type="button"
        @click="cardModalOpen = true"
        class="w-full h-10 md:h-9 flex items-center justify-between px-2.5 rounded-xl text-[13.5px] md:text-[12.5px] font-medium text-slate-700 hover:bg-black/[0.045] active:bg-black/[0.08] hover:text-[#1D1D1F] transition-colors"
      >
        <span class="flex items-center gap-2.5 min-w-0">
          <BookUser class="w-4 h-4 text-indigo-600 shrink-0" />
          <span class="truncate">角色库</span>
        </span>
        <span class="px-2 py-0.5 rounded-full bg-black/[0.055] text-[11px] font-semibold text-slate-600 tabular-nums">
          {{ characterCards.length }}
        </span>
      </button>

      <!-- Import / Export Menu -->
      <div class="relative">
        <div
          v-if="ioMenuOpen"
          @click="ioMenuOpen = false"
          class="fixed inset-0 z-20"
        ></div>
        <div
          v-if="ioMenuOpen"
          class="absolute bottom-full left-0 right-0 mb-1.5 z-30 p-1.5 rounded-2xl bg-white border border-black/[0.08] shadow-[0_12px_32px_rgba(0,0,0,0.14)] space-y-0.5"
        >
          <button
            type="button"
            @click="triggerImportScenario"
            class="w-full flex items-center gap-2.5 px-3 py-2 rounded-xl text-[13px] md:text-[12.5px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
          >
            <Upload class="w-4 h-4 md:w-3.5 md:h-3.5 text-slate-500 shrink-0" />
            <span>导入剧本 (.json)</span>
          </button>
          <button
            type="button"
            @click="ioMenuOpen = false; emit('exportScenario')"
            class="w-full flex items-center gap-2.5 px-3 py-2 rounded-xl text-[13px] md:text-[12.5px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
          >
            <FileJson class="w-4 h-4 md:w-3.5 md:h-3.5 text-slate-500 shrink-0" />
            <span>导出剧本 (.json)</span>
          </button>
          <button
            type="button"
            @click="ioMenuOpen = false; emit('exportMd')"
            class="w-full flex items-center gap-2.5 px-3 py-2 rounded-xl text-[13px] md:text-[12.5px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
          >
            <Download class="w-4 h-4 md:w-3.5 md:h-3.5 text-slate-500 shrink-0" />
            <span>导出完整记录 (.md)</span>
          </button>

          <div class="my-1 border-t border-black/[0.06]"></div>
          <div class="px-3 py-1 text-[10.5px] font-semibold text-slate-400">
            剧本导出（隐藏思考过程）
          </div>
          <button
            type="button"
            @click="ioMenuOpen = false; emit('exportPureMd')"
            class="w-full flex items-center gap-2.5 px-3 py-1.5 rounded-xl text-[12.5px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
          >
            <FileText class="w-3.5 h-3.5 text-slate-500 shrink-0" />
            <span>剧本 Markdown (.md)</span>
          </button>
          <button
            type="button"
            @click="ioMenuOpen = false; emit('exportPurePng')"
            class="w-full flex items-center gap-2.5 px-3 py-1.5 rounded-xl text-[12.5px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
          >
            <ImageIcon class="w-3.5 h-3.5 text-slate-500 shrink-0" />
            <span>剧本长图 (.png)</span>
          </button>
          <button
            type="button"
            @click="ioMenuOpen = false; emit('exportPurePdf')"
            class="w-full flex items-center gap-2.5 px-3 py-1.5 rounded-xl text-[12.5px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
          >
            <Printer class="w-3.5 h-3.5 text-slate-500 shrink-0" />
            <span>剧本 PDF (.pdf)</span>
          </button>
          <button
            type="button"
            @click="ioMenuOpen = false; emit('exportReplayMd')"
            class="w-full flex items-center gap-2.5 px-3 py-1.5 rounded-xl text-[12.5px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
          >
            <FileText class="w-3.5 h-3.5 text-amber-600 shrink-0" />
            <span>剧本杀复盘手卡 (.md)</span>
          </button>
          <button
            type="button"
            @click="ioMenuOpen = false; emit('exportReplayPdf')"
            class="w-full flex items-center gap-2.5 px-3 py-1.5 rounded-xl text-[12.5px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
          >
            <Printer class="w-3.5 h-3.5 text-amber-600 shrink-0" />
            <span>剧本杀复盘手卡 (.pdf)</span>
          </button>
        </div>

        <button
          type="button"
          @click="ioMenuOpen = !ioMenuOpen"
          class="w-full h-10 md:h-9 flex items-center justify-between px-2.5 rounded-xl text-[13.5px] md:text-[12.5px] font-medium transition-colors"
          :class="ioMenuOpen ? 'bg-black/[0.06] text-[#1D1D1F]' : 'text-slate-700 hover:bg-black/[0.045] active:bg-black/[0.08] hover:text-[#1D1D1F]'"
        >
          <span class="flex items-center gap-2.5 min-w-0">
            <FolderInput class="w-4 h-4 text-slate-500 shrink-0" />
            <span class="truncate">导入 / 导出</span>
          </span>
          <ChevronUp
            class="w-4 h-4 text-slate-400 transition-transform"
            :class="ioMenuOpen ? 'rotate-180' : ''"
          />
        </button>
      </div>

      <!-- Settings Entry -->
      <div class="pt-1 mt-1 border-t border-black/[0.06]">
        <button
          type="button"
          @click="emit('openSettings')"
          class="w-full h-10 md:h-9 flex items-center justify-between px-2.5 rounded-xl text-[13.5px] md:text-[12.5px] font-medium text-slate-700 hover:bg-black/[0.05] active:bg-black/[0.08] hover:text-[#1D1D1F] transition-colors outline-none focus-visible:ring-2 focus-visible:ring-[#007AFF]/30"
          :title="isMacOS ? '模型、接口与系统设置 (⌘,)' : isDesktopOS ? '模型、接口与系统设置 (Ctrl+,)' : '模型、接口与系统设置'"
        >
          <span class="flex items-center gap-2.5">
            <Settings class="w-4 h-4 text-slate-500 shrink-0" />
            <span>设置</span>
          </span>
          <kbd v-if="isDesktopOS" class="px-1.5 py-0.5 rounded bg-black/[0.05] text-[10.5px] font-sans text-slate-500">
            {{ isMacOS ? "⌘," : "Ctrl+," }}
          </kbd>
        </button>
      </div>
    </div>

    <!-- Character Library Management Modal -->
    <AdaptiveModal
      :open="cardModalOpen"
      title="角色库"
      max-width-class="max-w-xl"
      @close="cardModalOpen = false"
    >
      <template #header>
        <div class="px-5 py-3.5 border-b border-black/[0.06] flex items-center justify-between gap-2 shrink-0">
          <div class="flex items-center gap-2">
            <BookUser class="w-4 h-4 text-[var(--ds-accent)]" />
            <h3 class="text-[15px] md:text-sm font-semibold text-[#1D1D1F]">
              角色库 ({{ characterCards.length }})
            </h3>
          </div>
          <button
            type="button"
            @click="cardModalOpen = false"
            class="p-2 md:p-1.5 rounded-full hover:bg-slate-100 text-slate-400 hover:text-slate-700"
          >
            <X class="w-4 h-4" />
          </button>
        </div>
      </template>

      <div class="flex-1 overflow-y-auto p-4 sm:p-5 space-y-2.5 bg-[#F6F6F8]">
        <div
          v-if="characterCards.length === 0"
          class="py-12 text-center text-[13px] md:text-xs text-slate-400"
        >
          暂无角色存档，可在复盘报告中保存角色。
        </div>
        <div
          v-for="card in characterCards"
          :key="card.id"
          class="p-4 md:p-3.5 rounded-2xl bg-white border border-black/[0.07] space-y-1.5"
        >
          <div class="flex items-center justify-between gap-2">
            <div class="min-w-0 flex items-center gap-1.5 flex-wrap">
              <span class="text-[14px] md:text-xs font-semibold text-[#1D1D1F]">{{ card.name }}</span>
              <span
                v-if="card.isBuiltin"
                class="px-1.5 py-0.5 rounded-md bg-[var(--ds-accent-soft)] text-[10.5px] md:text-[10px] font-medium text-[var(--ds-accent)]"
              >
                内置
              </span>
              <span
                v-else-if="card.sourceExperimentTitle"
                class="text-[11px] md:text-[10.5px] text-slate-400"
              >
                来自「{{ card.sourceExperimentTitle }}」
              </span>
            </div>
            <button
              v-if="!card.isBuiltin"
              type="button"
              @click="emit('deleteCharacterCard', card.id)"
              class="p-2 md:p-1.5 rounded-lg text-slate-400 hover:text-[var(--ds-danger)] hover:bg-[var(--ds-danger-soft)] transition-colors shrink-0"
              title="从角色库删除"
            >
              <Trash2 class="w-4 h-4 md:w-3.5 md:h-3.5" />
            </button>
          </div>
          <p v-if="card.basePrompt" class="text-[13px] md:text-[11.5px] text-slate-600 leading-relaxed line-clamp-3">
            {{ card.basePrompt }}
          </p>
          <p
            v-if="card.behaviorTraits"
            class="text-[12px] md:text-[11px] text-[var(--ds-accent)] bg-[var(--ds-accent-soft)] px-2.5 py-1.5 md:py-1 rounded-lg"
          >
            {{ card.behaviorTraits }}
          </p>

          <!-- Cross-Session Memory Imprints (View / Edit / Delete single imprint) -->
          <div
            v-if="card.memoryImprints && card.memoryImprints.length > 0"
            class="pt-1 space-y-1.5"
          >
            <div class="text-[11px] font-semibold text-slate-500 flex items-center gap-1">
              <Sparkles class="w-3 h-3 text-[var(--ds-accent)]" />
              <span>经历记忆 ({{ card.memoryImprints.length }})</span>
            </div>
            <div
              v-for="(imprint, impIdx) in card.memoryImprints"
              :key="impIdx"
              class="group/imp flex items-center gap-1.5 rounded-xl bg-[#F5F5F7] px-2.5 py-1.5 text-[11.5px] text-slate-700"
            >
              <template v-if="editingImprintKey === `${card.id}:${impIdx}`">
                <input
                  v-model="editingImprintDraft"
                  @keydown.enter.prevent="saveEditImprint(card, impIdx)"
                  @keydown.esc="editingImprintKey = null"
                  class="flex-1 min-w-0 bg-white border border-[var(--ds-accent)] rounded-lg px-2 py-1 text-[11.5px] text-[#1D1D1F] outline-none"
                />
                <button
                  type="button"
                  @click="saveEditImprint(card, impIdx)"
                  class="p-1 rounded-md text-emerald-600 hover:bg-emerald-50"
                  title="保存"
                >
                  <Check class="w-3.5 h-3.5" />
                </button>
              </template>
              <template v-else>
                <span class="flex-1 min-w-0 leading-relaxed break-words">{{ imprint }}</span>
                <button
                  type="button"
                  @click="startEditImprint(card.id, impIdx, imprint)"
                  class="p-1 rounded-md text-slate-400 hover:text-[#007AFF] hover:bg-white opacity-70 group-hover/imp:opacity-100 transition shrink-0"
                  title="编辑"
                >
                  <Pencil class="w-3 h-3" />
                </button>
                <button
                  type="button"
                  @click="deleteImprint(card, impIdx)"
                  class="p-1 rounded-md text-slate-400 hover:text-[var(--ds-danger)] hover:bg-[var(--ds-danger-soft)] opacity-70 group-hover/imp:opacity-100 transition shrink-0"
                  title="删除"
                >
                  <Trash2 class="w-3 h-3" />
                </button>
              </template>
            </div>
          </div>
        </div>
      </div>
    </AdaptiveModal>

    <!-- Delete Experiment Confirmation Modal -->
    <AdaptiveModal
      :open="Boolean(pendingDeleteItem)"
      title="删除推演"
      max-width-class="max-w-[360px]"
      @close="cancelDelete"
    >
      <template #header>
        <div class="px-5 pt-5 pb-1 flex items-center justify-between gap-2 shrink-0">
          <h3 class="text-[15px] font-semibold text-[#1D1D1F]">确定删除推演？</h3>
          <button
            type="button"
            @click="cancelDelete"
            class="w-7 h-7 flex items-center justify-center rounded-lg text-[#858A95] hover:text-[#25262A] hover:bg-[#EFEFF2] transition-colors outline-none"
            title="关闭"
          >
            <X class="w-4 h-4" />
          </button>
        </div>
      </template>

      <div class="px-5 py-2.5 text-[13px] text-slate-600 leading-relaxed">
        将永久删除「<span class="font-medium text-[#1D1D1F]">{{ pendingDeleteItem?.title }}</span>」（共 {{ pendingDeleteItem?.totalTurns ?? 0 }} 轮记录），此操作不可恢复。
      </div>

      <template #footer>
        <div class="px-5 py-3.5 flex items-center justify-end gap-2 shrink-0">
          <button
            type="button"
            @click="cancelDelete"
            class="h-8 px-3.5 rounded-lg text-[12.5px] font-medium text-slate-600 hover:bg-black/[0.05] transition outline-none"
          >
            取消
          </button>
          <button
            type="button"
            @click="confirmDelete"
            class="h-8 px-4 rounded-lg text-[12.5px] font-medium bg-[var(--ds-danger)] hover:bg-[var(--ds-danger-hover)] text-white transition outline-none"
          >
            确认删除
          </button>
        </div>
      </template>
    </AdaptiveModal>
  </div>
</template>
