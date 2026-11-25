<script setup lang="ts">
import { computed, ref } from "vue";
import {
  Plus,
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
} from "lucide-vue-next";
import type { CharacterCard, ExperimentSummary } from "../types";
import { isMacOS, isDesktopOS } from "../utils/platform";
import AdaptiveModal from "./ui/AdaptiveModal.vue";

const props = defineProps<{
  experiments: ExperimentSummary[];
  currentExperimentId: string | null;
  characterCards: CharacterCard[];
  inSheet?: boolean;
}>();

const emit = defineEmits<{
  openNewExperiment: [];
  selectExperiment: [id: string];
  deleteExperiment: [id: string];
  exportScenario: [];
  importScenario: [file: File];
  exportMd: [];
  deleteCharacterCard: [cardId: string];
  openSettings: [];
}>();

const searchQuery = ref("");
const scenarioFileRef = ref<HTMLInputElement | null>(null);
const cardModalOpen = ref(false);
const ioMenuOpen = ref(false);

const filteredExperiments = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return props.experiments;
  return props.experiments.filter((item) => item.title.toLowerCase().includes(q));
});

function isDerivedBranch(title: string): boolean {
  return /·\s*(分支|改写重演)\s*#\d+/.test(title);
}

function triggerImportScenario() {
  ioMenuOpen.value = false;
  scenarioFileRef.value?.click();
}

function onScenarioFileChange(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (file) {
    emit("importScenario", file);
  }
}

function formatTok(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(2)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}
</script>

<template>
  <div class="h-full w-full flex flex-col bg-[#F3F3F5] border-r border-black/[0.08] select-none overflow-hidden antialiased">
    <!-- Top Traffic-Light Clearance Drag Region (macOS desktop only) + Lightweight Search Box -->
    <div
      :data-tauri-drag-region="isMacOS && !inSheet ? '' : undefined"
      class="shrink-0 px-3 pb-2.5"
      :class="isMacOS && !inSheet ? 'pt-10' : 'pt-3'"
    >
      <div class="relative flex items-center">
        <Search class="w-3.5 h-3.5 text-slate-400 absolute left-2.5 pointer-events-none" />
        <input
          v-model="searchQuery"
          placeholder="搜索实验存档"
          class="w-full rounded-lg bg-black/[0.05] focus:bg-white border border-transparent focus:border-black/[0.08] pl-8 pr-7 py-1.5 text-[12px] text-[#1D1D1F] placeholder:text-slate-400 outline-none focus:ring-2 focus:ring-[#007AFF]/20 transition-colors"
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

    <!-- Middle History Experiments List -->
    <div class="flex-1 min-h-0 overflow-y-auto px-2 py-1 space-y-0.5">
      <div class="px-2 pt-1 pb-1.5 flex items-center justify-between">
        <span class="text-[11px] font-semibold text-slate-500 tracking-wide">
          实验存档
        </span>
        <button
          type="button"
          @click="emit('openNewExperiment')"
          class="p-1 rounded-md text-slate-500 hover:text-[#1D1D1F] hover:bg-black/[0.06] transition-colors"
          title="新建实验"
        >
          <Plus class="w-3.5 h-3.5" />
        </button>
      </div>

      <div
        v-if="filteredExperiments.length === 0"
        class="py-8 text-center text-xs text-slate-400"
      >
        无匹配实验存档
      </div>

      <div
        v-for="item in filteredExperiments"
        :key="item.id"
        @click="emit('selectExperiment', item.id)"
        class="group relative flex items-center justify-between gap-1.5 py-1.5 pr-2 rounded-lg cursor-pointer transition-colors"
        :class="[
          isDerivedBranch(item.title) ? 'pl-5' : 'pl-2.5',
          item.id === currentExperimentId
            ? 'bg-black/[0.06] text-[#1D1D1F]'
            : 'hover:bg-black/[0.035] text-slate-700',
        ]"
      >
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-1.5 min-w-0">
            <GitBranch
              v-if="isDerivedBranch(item.title)"
              class="w-3 h-3 shrink-0 text-violet-500"
              title="平行宇宙派生分支"
            />
            <span
              class="text-[12.5px] truncate leading-snug"
              :class="item.id === currentExperimentId ? 'font-semibold' : 'font-medium'"
            >
              {{ item.title }}
            </span>
          </div>
          <div class="text-[10.5px] text-slate-400 tabular-nums mt-0.5 leading-tight">
            {{ item.totalTurns }} 轮 · {{ formatTok(item.totalTokens) }} tok
          </div>
        </div>

        <button
          v-if="experiments.length > 1"
          type="button"
          @click.stop="emit('deleteExperiment', item.id)"
          class="opacity-100 lg:opacity-0 lg:group-hover:opacity-100 p-1 rounded-md hover:bg-rose-100 text-slate-400 hover:text-rose-600 transition-colors shrink-0"
          title="删除存档"
        >
          <Trash2 class="w-3.5 h-3.5" />
        </button>
      </div>
    </div>

    <!-- Bottom Fixed Action Area -->
    <div class="relative shrink-0 px-2.5 pt-2 pb-[calc(0.625rem+env(safe-area-inset-bottom))] border-t border-black/[0.06] space-y-0.5 bg-[#F3F3F5]">
      <input
        ref="scenarioFileRef"
        type="file"
        accept=".json,application/json"
        class="hidden"
        @change="onScenarioFileChange"
      />

      <!-- Compact Soul Character Library -->
      <button
        type="button"
        @click="cardModalOpen = true"
        class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-[12px] font-medium text-slate-700 hover:bg-black/[0.05] hover:text-[#1D1D1F] transition-colors"
      >
        <span class="flex items-center gap-2 min-w-0">
          <BookUser class="w-3.5 h-3.5 text-indigo-600 shrink-0" />
          <span class="truncate">灵魂角色库</span>
        </span>
        <span class="px-1.5 py-0.5 rounded-md bg-black/[0.05] text-[10px] font-semibold text-slate-500 tabular-nums">
          {{ characterCards.length }}
        </span>
      </button>

      <!-- Compact Import / Export Menu -->
      <div class="relative">
        <div
          v-if="ioMenuOpen"
          @click="ioMenuOpen = false"
          class="fixed inset-0 z-20"
        ></div>
        <div
          v-if="ioMenuOpen"
          class="absolute bottom-full left-0 right-0 mb-1.5 z-30 p-1 rounded-xl bg-white border border-black/[0.08] shadow-[0_10px_30px_rgba(0,0,0,0.12)] space-y-0.5"
        >
          <button
            type="button"
            @click="triggerImportScenario"
            class="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg text-[12px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
          >
            <Upload class="w-3.5 h-3.5 text-slate-500 shrink-0" />
            <span>导入剧本 (.json)</span>
          </button>
          <button
            type="button"
            @click="ioMenuOpen = false; emit('exportScenario')"
            class="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg text-[12px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
          >
            <FileJson class="w-3.5 h-3.5 text-slate-500 shrink-0" />
            <span>导出剧本 (.json)</span>
          </button>
          <button
            type="button"
            @click="ioMenuOpen = false; emit('exportMd')"
            class="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg text-[12px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
          >
            <Download class="w-3.5 h-3.5 text-slate-500 shrink-0" />
            <span>导出档案 (.md)</span>
          </button>
        </div>

        <button
          type="button"
          @click="ioMenuOpen = !ioMenuOpen"
          class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-[12px] font-medium transition-colors"
          :class="ioMenuOpen ? 'bg-black/[0.06] text-[#1D1D1F]' : 'text-slate-700 hover:bg-black/[0.05] hover:text-[#1D1D1F]'"
        >
          <span class="flex items-center gap-2 min-w-0">
            <FolderInput class="w-3.5 h-3.5 text-slate-500 shrink-0" />
            <span class="truncate">导入 / 导出</span>
          </span>
          <ChevronUp
            class="w-3.5 h-3.5 text-slate-400 transition-transform"
            :class="ioMenuOpen ? 'rotate-180' : ''"
          />
        </button>
      </div>

      <!-- Native Classic Settings Entry -->
      <div class="pt-1 mt-1 border-t border-black/[0.06]">
        <button
          type="button"
          @click="emit('openSettings')"
          class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-[12px] font-medium text-slate-700 hover:bg-black/[0.06] hover:text-[#1D1D1F] transition-colors"
          :title="isMacOS ? '模型、API 与观察员设置 (⌘,)' : isDesktopOS ? '模型、API 与观察员设置 (Ctrl+,)' : '模型、API 与观察员设置'"
        >
          <span class="flex items-center gap-2">
            <Settings class="w-3.5 h-3.5 text-slate-500 shrink-0" />
            <span>设置</span>
          </span>
          <kbd v-if="isDesktopOS" class="text-[10.5px] font-sans text-slate-400">
            {{ isMacOS ? "⌘," : "Ctrl+," }}
          </kbd>
        </button>
      </div>
    </div>

    <!-- Soul Character Library Management Modal -->
    <AdaptiveModal
      :open="cardModalOpen"
      title="跨世界灵魂角色库"
      max-width-class="max-w-xl"
      @close="cardModalOpen = false"
    >
      <template #header>
        <div class="px-5 py-3.5 border-b border-black/[0.06] flex items-center justify-between gap-2 shrink-0">
          <div class="flex items-center gap-2">
            <BookUser class="w-4 h-4 text-indigo-600" />
            <h3 class="text-sm font-semibold text-[#1D1D1F]">
              跨世界灵魂角色库 ({{ characterCards.length }})
            </h3>
          </div>
          <button
            type="button"
            @click="cardModalOpen = false"
            class="p-1.5 rounded-full hover:bg-slate-100 text-slate-400 hover:text-slate-700"
          >
            <X class="w-4 h-4" />
          </button>
        </div>
      </template>

      <div class="flex-1 overflow-y-auto p-4 sm:p-5 space-y-2.5 bg-[#F6F6F8]">
        <div
          v-if="characterCards.length === 0"
          class="py-12 text-center text-xs text-slate-400"
        >
          角色库暂无存档，可在《实验结案复盘简报》中将高光角色一键存入。
        </div>
        <div
          v-for="card in characterCards"
          :key="card.id"
          class="p-3.5 rounded-2xl bg-white border border-black/[0.07] space-y-1.5"
        >
          <div class="flex items-center justify-between gap-2">
            <div class="min-w-0">
              <span class="text-xs font-semibold text-[#1D1D1F]">{{ card.name }}</span>
              <span
                v-if="card.sourceExperimentTitle"
                class="ml-2 text-[10.5px] text-slate-400"
              >
                来自「{{ card.sourceExperimentTitle }}」
              </span>
            </div>
            <button
              type="button"
              @click="emit('deleteCharacterCard', card.id)"
              class="p-1.5 rounded-lg text-slate-400 hover:text-rose-600 hover:bg-rose-50 transition-colors shrink-0"
              title="从角色库删除"
            >
              <Trash2 class="w-3.5 h-3.5" />
            </button>
          </div>
          <p v-if="card.basePrompt" class="text-[11.5px] text-slate-600 leading-relaxed line-clamp-3">
            {{ card.basePrompt }}
          </p>
          <p
            v-if="card.behaviorTraits"
            class="text-[11px] text-indigo-700 bg-indigo-50/70 px-2.5 py-1 rounded-lg"
          >
            {{ card.behaviorTraits }}
          </p>
        </div>
      </div>
    </AdaptiveModal>
  </div>
</template>
