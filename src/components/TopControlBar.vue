<script setup lang="ts">
import { computed, ref } from "vue";
import {
  Play,
  Pause,
  StepForward,
  Download,
  SlidersHorizontal,
  Plus,
  ChevronDown,
  Trash2,
  AlertCircle,
  Film,
  ClipboardList,
  Upload,
  FileJson,
} from "lucide-vue-next";
import type { EngineState, ExperimentDetail, ExperimentSummary } from "../types";

const props = defineProps<{
  experiment: ExperimentDetail | null;
  experiments: ExperimentSummary[];
  engineState: EngineState;
  statusReason: string | null;
  missingKeys: boolean;
  replayActive?: boolean;
  boardOpen?: boolean;
  hasPendingApproval?: boolean;
}>();

const emit = defineEmits<{
  start: [];
  pause: [];
  step: [];
  exportMd: [];
  openSettings: [];
  openNewExperiment: [];
  selectExperiment: [id: string];
  deleteExperiment: [id: string];
  renameExperiment: [title: string];
  toggleReplay: [];
  toggleBoard: [];
  exportScenario: [];
  importScenario: [file: File];
}>();

const dropdownOpen = ref(false);
const editingTitle = ref(false);
const draftTitle = ref("");
const scenarioFileRef = ref<HTMLInputElement | null>(null);

function triggerImportScenario() {
  dropdownOpen.value = false;
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

const isRunning = computed(() => props.engineState === "running");
const isStepping = computed(() => props.engineState === "stepping");

const statusInfo = computed(() => {
  switch (props.engineState) {
    case "running":
      return {
        label: "自主演化中",
        dotClass: "bg-emerald-500 animate-pulse shadow-[0_0_8px_rgba(16,185,129,0.65)]",
        pillClass: "bg-emerald-50 text-emerald-700 border-emerald-200/80",
      };
    case "stepping":
      return {
        label: "单步推演中",
        dotClass: "bg-amber-500 animate-pulse shadow-[0_0_8px_rgba(245,158,11,0.65)]",
        pillClass: "bg-amber-50 text-amber-700 border-amber-200/80",
      };
    case "paused":
      return {
        label: "已暂停",
        dotClass: "bg-amber-400",
        pillClass: "bg-amber-50/70 text-amber-700 border-amber-200/70",
      };
    default:
      return {
        label: "观察箱待命",
        dotClass: "bg-slate-400",
        pillClass: "bg-slate-100 text-slate-600 border-slate-200/80",
      };
  }
});

const formatTok = (n: number) => {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(2)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
};

const formattedTokens = computed(() => formatTok(props.experiment?.totalTokens ?? 0));
const tokenBudget = computed(() => props.experiment?.config.tokenBudget ?? 0);
const budgetPct = computed(() => {
  if (tokenBudget.value <= 0) return 0;
  return Math.min(100, Math.round(((props.experiment?.totalTokens ?? 0) / tokenBudget.value) * 100));
});
const budgetBarClass = computed(() => {
  if (budgetPct.value >= 100) return "bg-rose-500";
  if (budgetPct.value >= 80) return "bg-amber-500";
  return "bg-[#007AFF]";
});

function startRename() {
  if (!props.experiment) return;
  draftTitle.value = props.experiment.title;
  editingTitle.value = true;
}

function commitRename() {
  if (!editingTitle.value) return;
  editingTitle.value = false;
  const trimmed = draftTitle.value.trim();
  if (trimmed && props.experiment && trimmed !== props.experiment.title) {
    emit("renameExperiment", trimmed);
  }
}

function chooseExperiment(id: string) {
  dropdownOpen.value = false;
  if (id !== props.experiment?.id) {
    emit("selectExperiment", id);
  }
}
</script>

<template>
  <header
    class="min-h-14 shrink-0 px-3 md:px-5 py-1.5 md:py-0 flex flex-col justify-center gap-1.5 bg-white/85 backdrop-blur-xl border-b border-black/[0.06] z-20 select-none"
  >
    <div class="flex items-center justify-between gap-2 min-h-[40px]">
      <!-- Left: Experiment switcher + editable title + breathing status -->
      <div class="flex items-center gap-1.5 sm:gap-2 md:gap-3 min-w-0">
        <div class="relative shrink-0">
          <button
            @click="dropdownOpen = !dropdownOpen"
            class="flex items-center gap-1.5 md:gap-2 px-2.5 md:px-3 py-1.5 rounded-xl bg-[#F5F5F7] hover:bg-[#EBEBEF] transition text-xs md:text-sm font-medium text-[#1D1D1F] border border-black/[0.05]"
            title="切换或新建实验档案"
          >
            <span class="w-2 h-2 rounded-full bg-[#007AFF]"></span>
            <span class="max-w-[84px] sm:max-w-[160px] truncate">档案库</span>
            <ChevronDown class="w-3.5 h-3.5 text-slate-500" />
          </button>

          <div
            v-if="dropdownOpen"
            @click="dropdownOpen = false"
            class="fixed inset-0 z-40"
          ></div>

          <div
            v-if="dropdownOpen"
            class="absolute left-0 mt-2 w-72 max-w-[calc(100vw-24px)] rounded-2xl bg-white shadow-[0_16px_40px_rgba(0,0,0,0.12)] border border-black/[0.08] p-1.5 z-50"
          >
          <div class="px-2.5 py-1.5 flex items-center justify-between">
            <span class="text-[11px] font-medium text-slate-400 uppercase tracking-wider">
              历史实验存档 ({{ experiments.length }})
            </span>
            <button
              @click="dropdownOpen = false; emit('openNewExperiment')"
              class="flex items-center gap-1 text-xs font-medium text-[#007AFF] hover:text-[#005FCC] px-2 py-0.5 rounded-lg hover:bg-blue-50 transition"
            >
              <Plus class="w-3.5 h-3.5" />
              新建实验
            </button>
          </div>
          <input
            ref="scenarioFileRef"
            type="file"
            accept=".json,application/json"
            class="hidden"
            @change="onScenarioFileChange"
          />
          <div class="px-2 pb-1.5 flex items-center gap-1.5 border-b border-black/[0.05]">
            <button
              type="button"
              @click="dropdownOpen = false; emit('exportScenario')"
              class="flex-1 flex items-center justify-center gap-1 px-2 py-1 rounded-lg text-[11px] font-medium bg-[#F5F5F7] hover:bg-[#EAEAEF] text-slate-700 transition"
              title="将当前角色设定、初始白板与开局引子导出为 .json 剧本"
            >
              <FileJson class="w-3 h-3" />
              导出剧本
            </button>
            <button
              type="button"
              @click="triggerImportScenario"
              class="flex-1 flex items-center justify-center gap-1 px-2 py-1 rounded-lg text-[11px] font-medium bg-[#F5F5F7] hover:bg-[#EAEAEF] text-slate-700 transition"
              title="从本地 .json 剧本文件一键开局"
            >
              <Upload class="w-3 h-3" />
              导入剧本
            </button>
          </div>
          <div class="max-h-64 overflow-y-auto space-y-0.5 mt-1">
            <div
              v-for="item in experiments"
              :key="item.id"
              class="group flex items-center justify-between px-2.5 py-2 rounded-xl cursor-pointer transition"
              :class="
                item.id === experiment?.id
                  ? 'bg-[#007AFF]/10 text-[#0055B3]'
                  : 'hover:bg-slate-100/80 text-slate-700'
              "
              @click="chooseExperiment(item.id)"
            >
              <div class="min-w-0 pr-2">
                <div class="text-xs font-medium truncate">{{ item.title }}</div>
                <div class="text-[11px] text-slate-400">
                  {{ item.totalTurns }} 轮 · {{ item.totalTokens.toLocaleString() }} tok
                </div>
              </div>
              <button
                v-if="experiments.length > 1"
                @click.stop="emit('deleteExperiment', item.id)"
                class="opacity-100 md:opacity-0 md:group-hover:opacity-100 p-1.5 md:p-1 rounded-lg hover:bg-rose-100 text-slate-400 hover:text-rose-600 transition"
                title="删除存档"
              >
                <Trash2 class="w-3.5 h-3.5" />
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- Editable current title -->
      <div class="min-w-0">
        <input
          v-if="editingTitle"
          v-model="draftTitle"
          @blur="commitRename"
          @keydown.enter="commitRename"
          @keydown.esc="editingTitle = false"
          autofocus
          class="text-sm md:text-[15px] font-semibold text-[#1D1D1F] bg-slate-100 px-2.5 py-1 rounded-lg outline-none ring-2 ring-[#007AFF]/30 w-36 sm:w-56"
        />
        <button
          v-else
          @click="startRename"
          class="text-xs sm:text-[15px] font-semibold text-[#1D1D1F] hover:text-[#007AFF] truncate max-w-[110px] sm:max-w-[180px] lg:max-w-[240px] text-left transition block"
          title="点击重命名实验"
        >
          {{ experiment?.title ?? "加载中..." }}
        </button>
      </div>

      <!-- Breathing Status Light -->
      <div
        class="flex items-center gap-1.5 md:gap-2 px-2 md:px-2.5 py-1 rounded-full border text-[11px] md:text-xs font-medium shrink-0 transition"
        :class="statusInfo.pillClass"
        :title="statusReason ?? statusInfo.label"
      >
        <span class="w-2 h-2 rounded-full" :class="statusInfo.dotClass"></span>
        <span class="hidden sm:inline">{{ statusInfo.label }}</span>
      </div>

      <div
        v-if="missingKeys"
        @click="emit('openSettings')"
        class="hidden xl:flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-rose-50 border border-rose-200/80 text-rose-600 text-xs cursor-pointer hover:bg-rose-100/70 transition"
      >
        <AlertCircle class="w-3.5 h-3.5" />
        <span>请配置模型 API Key</span>
      </div>
    </div>

    <!-- Center: Turn counter + cumulative tokens -->
    <div
      class="hidden md:flex items-center gap-4 px-4 py-1.5 rounded-2xl bg-[#F5F5F7]/90 border border-black/[0.04] text-xs text-slate-600"
    >
      <div class="flex items-center gap-1.5">
        <span class="text-slate-400">总轮数</span>
        <span class="font-semibold text-[#1D1D1F] tabular-nums">
          {{ experiment?.totalTurns ?? 0 }}
        </span>
      </div>
      <div class="w-px h-3.5 bg-black/10"></div>
      <div class="flex items-center gap-1.5">
        <span class="text-slate-400">生态体</span>
        <span class="font-semibold text-[#1D1D1F] tabular-nums">
          {{ experiment?.config.agents.length ?? 2 }} AI
        </span>
      </div>
      <div class="w-px h-3.5 bg-black/10"></div>
      <div
        @click="emit('openSettings')"
        class="flex items-center gap-2 cursor-pointer group"
        :title="
          tokenBudget > 0
            ? `Token 预算: ${(experiment?.totalTokens ?? 0).toLocaleString()} / ${tokenBudget.toLocaleString()} (${budgetPct}%) · 点击调整`
            : `精确累计: ${(experiment?.totalTokens ?? 0).toLocaleString()} Tokens (未设预算上限 · 点击设置)`
        "
      >
        <span class="text-slate-400">Token</span>
        <span
          class="font-semibold tabular-nums"
          :class="budgetPct >= 100 ? 'text-rose-600' : 'text-[#1D1D1F]'"
        >
          {{ formattedTokens }}
          <template v-if="tokenBudget > 0">
            <span class="text-slate-400 font-normal">/ {{ formatTok(tokenBudget) }}</span>
          </template>
        </span>
        <div
          v-if="tokenBudget > 0"
          class="w-16 h-1.5 rounded-full bg-slate-200/90 overflow-hidden"
        >
          <div
            class="h-full rounded-full transition-all duration-300"
            :class="budgetBarClass"
            :style="{ width: `${budgetPct}%` }"
          ></div>
        </div>
      </div>
    </div>

    <!-- Right: Action controls -->
    <div class="flex items-center gap-1.5 md:gap-2 shrink-0">
      <button
        v-if="!isRunning"
        @click="emit('start')"
        :disabled="isStepping"
        class="flex items-center gap-1.5 px-3 md:px-4 py-1.5 rounded-full bg-[#1D1D1F] hover:bg-black text-white text-xs font-medium shadow-sm transition active:scale-[0.98] disabled:opacity-50"
      >
        <Play class="w-3.5 h-3.5 fill-current" />
        <span class="hidden sm:inline">开始演化</span>
      </button>
      <button
        v-else
        @click="emit('pause')"
        class="flex items-center gap-1.5 px-3 md:px-4 py-1.5 rounded-full bg-amber-500 hover:bg-amber-600 text-white text-xs font-medium shadow-sm transition active:scale-[0.98]"
      >
        <Pause class="w-3.5 h-3.5 fill-current" />
        <span class="hidden sm:inline">暂停</span>
      </button>

      <button
        @click="emit('step')"
        :disabled="isRunning || isStepping"
        class="flex items-center gap-1.5 px-2.5 md:px-3.5 py-1.5 rounded-full bg-[#F5F5F7] hover:bg-[#EAEAEF] text-[#1D1D1F] text-xs font-medium border border-black/[0.06] transition disabled:opacity-45"
        title="单步推进一轮对话"
      >
        <StepForward class="w-3.5 h-3.5" />
        <span class="hidden sm:inline">单步推进</span>
      </button>

      <button
        @click="emit('toggleBoard')"
        class="relative flex items-center gap-1.5 px-2.5 md:px-3 py-1.5 rounded-full text-xs font-medium border transition"
        :class="
          boardOpen
            ? 'bg-[#1D1D1F] text-white border-[#1D1D1F]'
            : 'bg-[#F5F5F7] hover:bg-[#EAEAEF] text-slate-700 border-black/[0.06]'
        "
        title="展开/收起共享实验室白板"
      >
        <ClipboardList class="w-3.5 h-3.5" />
        <span class="hidden xl:inline">共享白板</span>
        <span
          v-if="hasPendingApproval"
          class="w-2 h-2 rounded-full bg-amber-500 animate-ping"
        ></span>
      </button>

      <button
        @click="emit('toggleReplay')"
        :disabled="isRunning || isStepping || (experiment?.messages.length ?? 0) === 0"
        class="flex items-center gap-1.5 px-2.5 md:px-3 py-1.5 rounded-full text-xs font-medium border transition disabled:opacity-40"
        :class="
          replayActive
            ? 'bg-[#007AFF] text-white border-[#007AFF]'
            : 'bg-[#F5F5F7] hover:bg-[#EAEAEF] text-slate-700 border-black/[0.06]'
        "
        title="进入剧场回放模式（支持倍速播放）"
      >
        <Film class="w-3.5 h-3.5" />
        <span class="hidden xl:inline">{{ replayActive ? "退出回放" : "剧场回放" }}</span>
      </button>

      <button
        @click="emit('exportMd')"
        class="flex items-center gap-1.5 px-2.5 md:px-3 py-1.5 rounded-full bg-[#F5F5F7] hover:bg-[#EAEAEF] text-slate-700 text-xs font-medium border border-black/[0.06] transition"
        title="导出带观察员批注的 Markdown 档案"
      >
        <Download class="w-3.5 h-3.5" />
        <span class="hidden lg:inline">导出 MD</span>
      </button>

      <button
        @click="emit('openSettings')"
        class="flex items-center gap-1.5 px-2.5 md:px-3 py-1.5 rounded-full bg-[#F5F5F7] hover:bg-[#EAEAEF] text-slate-700 text-xs font-medium border border-black/[0.06] transition"
        title="模型、API 与观察员设置"
      >
        <SlidersHorizontal class="w-3.5 h-3.5" />
        <span class="hidden lg:inline">模型与设置</span>
      </button>
    </div>
    </div>

    <!-- Mobile Compact Secondary Bar (< md): Token Budget + Missing Key Alert -->
    <div
      v-if="missingKeys || tokenBudget > 0"
      class="flex md:hidden items-center justify-between gap-2 pb-0.5 text-[11px]"
    >
      <button
        v-if="missingKeys"
        @click="emit('openSettings')"
        class="flex items-center gap-1 px-2 py-0.5 rounded-full bg-rose-50 border border-rose-200/80 text-rose-600"
      >
        <AlertCircle class="w-3 h-3" />
        <span>点此配置模型 API Key</span>
      </button>
      <div
        v-if="tokenBudget > 0"
        @click="emit('openSettings')"
        class="flex items-center gap-1.5 ml-auto text-slate-600"
      >
        <span class="tabular-nums">{{ formattedTokens }} / {{ formatTok(tokenBudget) }}</span>
        <div class="w-16 h-1.5 rounded-full bg-slate-200 overflow-hidden">
          <div class="h-full rounded-full" :class="budgetBarClass" :style="{ width: `${budgetPct}%` }"></div>
        </div>
      </div>
    </div>
  </header>
</template>
