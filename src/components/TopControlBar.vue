<script setup lang="ts">
import { computed, ref } from "vue";
import {
  Play,
  Pause,
  StepForward,
  ChevronDown,
  AlertCircle,
  Film,
  PanelLeft,
  PanelLeftClose,
  PanelRight,
  PanelRightClose,
  UserCheck,
} from "lucide-vue-next";
import type { EngineState, ExperimentDetail } from "../types";
import { isMacOS } from "../utils/platform";

const props = defineProps<{
  experiment: ExperimentDetail | null;
  engineState: EngineState;
  statusReason: string | null;
  missingKeys: boolean;
  replayActive?: boolean;
  sidebarOpen?: boolean;
  trafficLightClearance?: boolean;
  inspectorOpen?: boolean;
  hasPendingApproval?: boolean;
  contextUsagePct?: number | null;
  contextActiveTokens?: number;
}>();

const emit = defineEmits<{
  start: [];
  pause: [];
  step: [targetAgentId?: string | null];
  openSettings: [];
  renameExperiment: [title: string];
  toggleReplay: [];
  toggleSidebar: [];
  toggleInspector: [];
}>();

const editingTitle = ref(false);
const draftTitle = ref("");
const stepMenuOpen = ref(false);

const isRunning = computed(() => props.engineState === "running");
const isStepping = computed(() => props.engineState === "stepping");

const agents = computed(() => props.experiment?.config.agents ?? []);

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
        label: "待命",
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

const compactionThreshold = computed(
  () => props.experiment?.config.compactionThresholdPct ?? 80
);
const memoryWatermarkClass = computed(() => {
  const pct = props.contextUsagePct ?? 0;
  if (pct >= compactionThreshold.value) {
    return "bg-rose-50 text-rose-700 border-rose-200/80";
  }
  if (pct >= Math.max(40, compactionThreshold.value - 15)) {
    return "bg-amber-50 text-amber-700 border-amber-200/80";
  }
  return "bg-[#F5F5F7] text-slate-600 border-black/[0.06]";
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

function triggerStep(agentId?: string | null) {
  stepMenuOpen.value = false;
  emit("step", agentId ?? null);
}
</script>

<template>
  <header
    :data-tauri-drag-region="isMacOS ? '' : undefined"
    class="@container h-12 shrink-0 flex items-center justify-between gap-2 bg-white border-b border-black/[0.07] z-20 select-none antialiased"
    :class="trafficLightClearance ? 'pl-[78px] pr-3 md:pr-4' : 'px-3 md:px-4'"
  >
    <!-- Left: Sidebar toggle + editable title + running status capsule -->
    <div :data-tauri-drag-region="isMacOS ? '' : undefined" class="flex items-center gap-2 min-w-0 flex-1">
      <button
        type="button"
        @click="emit('toggleSidebar')"
        class="shrink-0 p-1.5 rounded-lg text-slate-600 hover:text-[#1D1D1F] hover:bg-black/[0.05] transition-colors"
        title="展开/收起左侧实验档案栏"
      >
        <PanelLeftClose v-if="sidebarOpen" class="w-4 h-4" />
        <PanelLeft v-else class="w-4 h-4" />
      </button>

      <!-- Editable current title -->
      <div class="min-w-0 flex-1 max-w-[260px]">
        <input
          v-if="editingTitle"
          v-model="draftTitle"
          @blur="commitRename"
          @keydown.enter="commitRename"
          @keydown.esc="editingTitle = false"
          autofocus
          class="text-[13px] font-semibold text-[#1D1D1F] bg-slate-100 px-2 py-0.5 rounded-md outline-none ring-2 ring-[#007AFF]/30 w-full max-w-[220px]"
        />
        <button
          v-else
          type="button"
          @click="startRename"
          class="text-[13px] font-semibold text-[#1D1D1F] hover:text-[#007AFF] truncate max-w-full text-left transition-colors block"
          :title="experiment?.title ? `${experiment.title}（点击重命名）` : '点击重命名实验'"
        >
          {{ experiment?.title ?? "加载中..." }}
        </button>
      </div>

      <!-- Running Status Capsule -->
      <div
        class="flex items-center gap-1.5 px-2 py-0.5 rounded-full border text-[11px] font-medium shrink-0"
        :class="statusInfo.pillClass"
        :title="statusReason ?? statusInfo.label"
      >
        <span class="w-1.5 h-1.5 rounded-full" :class="statusInfo.dotClass"></span>
        <span class="hidden @[520px]:inline">{{ statusInfo.label }}</span>
      </div>
    </div>

    <!-- Right: Start/Pause + Step with Target Dropdown + Replay + Turn/Memory Watermark + Inspector Toggle -->
    <div :data-tauri-drag-region="isMacOS ? '' : undefined" class="flex items-center justify-end gap-1.5 shrink-0">
      <button
        v-if="missingKeys"
        type="button"
        @click="emit('openSettings')"
        class="flex items-center gap-1 px-2 py-1 rounded-full bg-rose-50 border border-rose-200/80 text-rose-600 text-[11px] font-medium hover:bg-rose-100/70 transition-colors shrink-0"
        title="点击前往设置模型 API Key"
      >
        <AlertCircle class="w-3.5 h-3.5" />
        <span class="hidden @[680px]:inline">请配置 API Key</span>
        <span class="@[680px]:hidden">缺 Key</span>
      </button>

      <!-- Start / Pause -->
      <button
        v-if="!isRunning"
        type="button"
        @click="emit('start')"
        :disabled="isStepping"
        class="flex items-center gap-1.5 px-2.5 @[560px]:px-3 py-1.5 rounded-lg bg-[#1D1D1F] hover:bg-black text-white text-xs font-medium transition-colors disabled:opacity-50"
        title="开始演化"
      >
        <Play class="w-3.5 h-3.5 fill-current" />
        <span class="hidden @[540px]:inline">开始演化</span>
      </button>
      <button
        v-else
        type="button"
        @click="emit('pause')"
        class="flex items-center gap-1.5 px-2.5 @[560px]:px-3 py-1.5 rounded-lg bg-amber-500 hover:bg-amber-600 text-white text-xs font-medium transition-colors"
        title="暂停演化"
      >
        <Pause class="w-3.5 h-3.5 fill-current" />
        <span class="hidden @[540px]:inline">暂停</span>
      </button>

      <!-- Split Step Button with Agent Roll-Call Dropdown -->
      <div class="relative flex items-center">
        <div
          v-if="stepMenuOpen"
          @click="stepMenuOpen = false"
          class="fixed inset-0 z-20"
        ></div>

        <div class="flex items-center rounded-lg border border-black/[0.08] bg-[#F5F5F7] overflow-hidden">
          <button
            type="button"
            @click="triggerStep(null)"
            :disabled="isRunning || isStepping"
            class="flex items-center gap-1 px-2 @[560px]:px-2.5 py-1.5 text-xs font-medium text-[#1D1D1F] hover:bg-[#EAEAEF] transition-colors disabled:opacity-45"
            title="按顺序单步推进一轮对话"
          >
            <StepForward class="w-3.5 h-3.5" />
            <span class="hidden @[580px]:inline">单步</span>
          </button>
          <button
            type="button"
            @click="stepMenuOpen = !stepMenuOpen"
            :disabled="isRunning || isStepping || agents.length === 0"
            class="px-1.5 py-1.5 border-l border-black/[0.07] text-slate-600 hover:text-[#1D1D1F] hover:bg-[#EAEAEF] transition-colors disabled:opacity-45"
            title="点名指定角色发言"
          >
            <ChevronDown class="w-3.5 h-3.5" />
          </button>
        </div>

        <div
          v-if="stepMenuOpen"
          class="absolute right-0 top-full mt-1.5 z-30 w-48 p-1 rounded-xl bg-white border border-black/[0.08] shadow-[0_12px_32px_rgba(0,0,0,0.12)] space-y-0.5"
        >
          <button
            type="button"
            @click="triggerStep(null)"
            class="w-full flex items-center justify-between gap-2 px-2.5 py-1.5 rounded-lg text-[12px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
          >
            <span class="flex items-center gap-1.5">
              <StepForward class="w-3.5 h-3.5 text-slate-500" />
              <span>顺位单步</span>
            </span>
            <span class="text-[10px] text-slate-400">默认</span>
          </button>
          <div class="my-1 border-t border-black/[0.06]"></div>
          <div class="px-2.5 py-0.5 text-[10px] font-semibold text-slate-400">
            点名指定发言角色
          </div>
          <button
            v-for="ag in agents"
            :key="ag.id"
            type="button"
            @click="triggerStep(ag.id)"
            class="w-full flex items-center justify-between gap-2 px-2.5 py-1.5 rounded-lg text-[12px] text-[#1D1D1F] hover:bg-[#007AFF]/10 hover:text-[#0062CC] transition-colors text-left"
          >
            <span class="flex items-center gap-1.5 min-w-0">
              <UserCheck class="w-3.5 h-3.5 shrink-0 text-[#007AFF]" />
              <span class="truncate">{{ ag.label }}</span>
            </span>
            <span
              v-if="experiment?.nextSpeaker === ag.id"
              class="text-[10px] px-1.5 py-0.5 rounded bg-black/[0.05] text-slate-500 shrink-0"
            >
              下一位
            </span>
          </button>
        </div>
      </div>

      <!-- Replay -->
      <button
        type="button"
        @click="emit('toggleReplay')"
        :disabled="isRunning || isStepping || (experiment?.messages.length ?? 0) === 0"
        class="flex items-center gap-1 px-2 @[560px]:px-2.5 py-1.5 rounded-lg text-xs font-medium border transition-colors disabled:opacity-40"
        :class="
          replayActive
            ? 'bg-[#007AFF] text-white border-[#007AFF]'
            : 'bg-[#F5F5F7] hover:bg-[#EAEAEF] text-slate-700 border-black/[0.07]'
        "
        title="剧场回放模式（支持倍速播放）"
      >
        <Film class="w-3.5 h-3.5" />
        <span class="hidden @[620px]:inline">{{ replayActive ? "退出回放" : "回放" }}</span>
      </button>

      <!-- Turn / Token / Memory Watermark Indicator -->
      <button
        type="button"
        @click="emit('openSettings')"
        class="hidden @[480px]:flex items-center gap-1.5 px-2.5 py-1 rounded-lg border text-[11px] transition-colors shrink-0"
        :class="memoryWatermarkClass"
        :title="
          tokenBudget > 0
            ? `总轮数 ${experiment?.totalTurns ?? 0} · Token ${(experiment?.totalTokens ?? 0).toLocaleString()} / ${tokenBudget.toLocaleString()} (${budgetPct}%) · 记忆 ${contextUsagePct ?? 0}%`
            : `总轮数 ${experiment?.totalTurns ?? 0} · 累计 ${(experiment?.totalTokens ?? 0).toLocaleString()} Tokens`
        "
      >
        <span class="font-semibold text-[#1D1D1F] tabular-nums">
          {{ experiment?.totalTurns ?? 0 }} 轮
        </span>
        <span class="text-slate-300">·</span>
        <span
          class="font-medium tabular-nums"
          :class="budgetPct >= 100 ? 'text-rose-600' : 'text-slate-600'"
        >
          {{ formattedTokens }}
        </span>
        <template v-if="contextUsagePct !== null && contextUsagePct !== undefined">
          <span class="text-slate-300">·</span>
          <span class="tabular-nums font-medium">记忆 {{ contextUsagePct }}%</span>
        </template>
      </button>

      <!-- Right Inspector Panel Toggle -->
      <button
        type="button"
        @click="emit('toggleInspector')"
        class="hidden md:flex relative items-center justify-center p-1.5 rounded-lg border transition-colors shrink-0"
        :class="
          inspectorOpen
            ? 'bg-black/[0.06] text-[#1D1D1F] border-black/[0.08]'
            : 'bg-transparent hover:bg-black/[0.05] text-slate-600 border-transparent'
        "
        title="展开/收起右侧观察舱与共享白板"
      >
        <PanelRightClose v-if="inspectorOpen" class="w-4 h-4" />
        <PanelRight v-else class="w-4 h-4" />
        <span
          v-if="hasPendingApproval"
          class="absolute -top-0.5 -right-0.5 w-2 h-2 rounded-full bg-amber-500 animate-ping"
        ></span>
      </button>
    </div>
  </header>
</template>
