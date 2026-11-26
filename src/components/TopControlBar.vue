<script setup lang="ts">
import { computed, ref } from "vue";
import { useMediaQuery } from "@vueuse/core";
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
  MoreHorizontal,
  SlidersHorizontal,
  Eye,
  EyeOff,
} from "lucide-vue-next";
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "reka-ui";
import type { EngineState, ExperimentDetail } from "../types";
import { isMacOS, isWindows, isMobileOS } from "../utils/platform";
import { triggerHaptic } from "../utils/native";

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
  compactMobile?: boolean;
  blindGuessMode?: boolean;
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
  toggleBlindGuess: [];
}>();

const editingTitle = ref(false);
const draftTitle = ref("");
const stepMenuOpen = ref(false);
const fallbackDesktop = useMediaQuery("(min-width: 768px)");
const isDesktopOrWideTablet = computed(() =>
  props.compactMobile !== undefined ? !props.compactMobile : fallbackDesktop.value
);

const isRunning = computed(() => props.engineState === "running");
const isStepping = computed(() => props.engineState === "stepping");

const agents = computed(() => props.experiment?.config.agents ?? []);

const statusInfo = computed(() => {
  switch (props.engineState) {
    case "running":
      return {
        label: "演化中",
        fullLabel: "自主演化中",
        dotClass: "bg-emerald-500 animate-pulse shadow-[0_0_8px_rgba(16,185,129,0.65)]",
        pillClass: "bg-emerald-50 text-emerald-700 border-emerald-200/80",
      };
    case "stepping":
      return {
        label: "推演中",
        fullLabel: "单步推演中",
        dotClass: "bg-amber-500 animate-pulse shadow-[0_0_8px_rgba(245,158,11,0.65)]",
        pillClass: "bg-amber-50 text-amber-700 border-amber-200/80",
      };
    case "paused":
      return {
        label: "已暂停",
        fullLabel: "已暂停",
        dotClass: "bg-amber-400",
        pillClass: "bg-amber-50/70 text-amber-700 border-amber-200/70",
      };
    default:
      return {
        label: "待命",
        fullLabel: "待命",
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
    class="@container shrink-0 flex items-center justify-between gap-2 bg-white z-20 select-none antialiased"
    :class="[
      isDesktopOrWideTablet
        ? isMobileOS
          ? 'pt-[var(--safe-top,env(safe-area-inset-top))] min-h-[calc(46px+var(--safe-top,0px))] border-b border-black/[0.05]'
          : isWindows
            ? 'h-[46px]'
            : 'h-[46px] border-b border-black/[0.05]'
        : 'min-h-[52px] pt-[calc(0.35rem+var(--safe-top,env(safe-area-inset-top)))] pb-1.5 border-b border-black/[0.05]',
      trafficLightClearance ? 'pl-[78px] pr-3 md:pr-4' : 'px-3 md:px-4',
    ]"
  >
    <!-- Left: Sidebar toggle + title block -->
    <div :data-tauri-drag-region="isMacOS ? '' : undefined" class="flex items-center gap-2 min-w-0 flex-1">
      <button
        type="button"
        @click="triggerHaptic('light'); emit('toggleSidebar')"
        class="shrink-0 w-9 h-9 md:w-8 md:h-8 flex items-center justify-center rounded-xl md:rounded-lg text-slate-500 hover:text-[#1D1D1F] hover:bg-black/[0.05] active:bg-black/[0.08] transition-colors"
        title="展开/收起左侧实验档案栏 (Ctrl+B)"
      >
        <PanelLeftClose v-if="sidebarOpen" class="w-[18px] h-[18px] md:w-4 md:h-4" />
        <PanelLeft v-else class="w-[18px] h-[18px] md:w-4 md:h-4" />
      </button>

      <!-- Editable current title + compact mobile subtitle -->
      <div class="min-w-0 flex-1 max-w-[280px]">
        <input
          v-if="editingTitle"
          v-model="draftTitle"
          @blur="commitRename"
          @keydown.enter="commitRename"
          @keydown.esc="editingTitle = false"
          autofocus
          class="text-[14px] font-semibold text-[#1D1D1F] bg-slate-100 px-2.5 py-1 md:py-0.5 rounded-lg outline-none ring-2 ring-[#007AFF]/30 w-full max-w-[240px]"
        />
        <button
          v-else
          type="button"
          @click="startRename"
          class="min-w-0 max-w-full text-left block group"
          :title="experiment?.title ? `${experiment.title}（点击重命名）` : '点击重命名实验'"
        >
          <span class="flex items-center gap-2 min-w-0">
            <span class="text-[15px] md:text-[13.5px] font-semibold text-[#1D1D1F] group-hover:text-[#007AFF] truncate leading-tight transition-colors">
              {{ experiment?.title ?? "加载中..." }}
            </span>
            <span
              v-if="isDesktopOrWideTablet"
              class="inline-block w-1.5 h-1.5 rounded-full shrink-0"
              :class="statusInfo.dotClass"
              :title="statusReason ?? statusInfo.fullLabel"
            ></span>
          </span>
          <!-- Mobile/Compact Portrait single clean subtitle line -->
          <span v-if="!isDesktopOrWideTablet" class="flex items-center gap-1.5 text-[11px] text-slate-500 leading-tight mt-0.5 tabular-nums truncate">
            <span class="inline-flex items-center gap-1 shrink-0">
              <span class="w-1.5 h-1.5 rounded-full" :class="statusInfo.dotClass"></span>
              <span>{{ statusInfo.label }}</span>
            </span>
            <span class="text-slate-300">·</span>
            <span>{{ experiment?.totalTurns ?? 0 }} 轮</span>
            <span class="text-slate-300">·</span>
            <span :class="budgetPct >= 100 ? 'text-rose-600 font-medium' : ''">
              {{ formattedTokens }} tok
            </span>
            <template v-if="contextUsagePct !== null && contextUsagePct !== undefined">
              <span class="text-slate-300">·</span>
              <span>记忆 {{ contextUsagePct }}%</span>
            </template>
          </span>
        </button>
      </div>
    </div>

    <!-- Right Controls (Borderless linear icon-first on desktop, touch-friendly pills on mobile) -->
    <div :data-tauri-drag-region="isMacOS ? '' : undefined" class="flex items-center justify-end gap-1 md:gap-1.5 shrink-0">
      <button
        v-if="missingKeys"
        type="button"
        @click="emit('openSettings')"
        class="flex items-center gap-1 h-8 md:h-7 px-2.5 rounded-full bg-rose-50 border border-rose-200/80 text-rose-600 text-[11.5px] font-medium hover:bg-rose-100/70 transition-colors shrink-0 mr-1"
        title="点击前往设置模型 API Key"
      >
        <AlertCircle class="w-3.5 h-3.5" />
        <span class="hidden @[680px]:inline">配置 API Key</span>
        <span class="@[680px]:hidden">缺 Key</span>
      </button>

      <!-- Desktop Subtle Inline Metrics (Clickable to open settings, zero box clutter) -->
      <button
        v-if="isDesktopOrWideTablet"
        type="button"
        @click="emit('openSettings')"
        class="hidden @[580px]:flex items-center gap-1.5 h-7 px-2 rounded-lg text-[12px] text-slate-400 hover:text-slate-700 hover:bg-black/[0.04] transition-colors shrink-0 mr-1"
        :title="
          tokenBudget > 0
            ? `总轮数 ${experiment?.totalTurns ?? 0} · Token ${(experiment?.totalTokens ?? 0).toLocaleString()} / ${tokenBudget.toLocaleString()} (${budgetPct}%) · 记忆 ${contextUsagePct ?? 0}%`
            : `总轮数 ${experiment?.totalTurns ?? 0} · 累计 ${(experiment?.totalTokens ?? 0).toLocaleString()} Tokens`
        "
      >
        <span class="tabular-nums font-medium text-slate-500">{{ experiment?.totalTurns ?? 0 }} 轮</span>
        <span class="text-slate-300">·</span>
        <span class="tabular-nums" :class="budgetPct >= 100 ? 'text-rose-600 font-medium' : ''">{{ formattedTokens }}</span>
        <template v-if="contextUsagePct !== null && contextUsagePct !== undefined">
          <span class="text-slate-300">·</span>
          <span class="tabular-nums" :class="contextUsagePct >= compactionThreshold ? 'text-rose-500 font-medium' : ''">
            {{ contextUsagePct }}%
          </span>
        </template>
      </button>

      <!-- Start / Pause -->
      <button
        v-if="!isRunning"
        type="button"
        @click="triggerHaptic('medium'); emit('start')"
        :disabled="isStepping"
        class="flex items-center gap-1.5 h-9 md:h-7 px-3.5 md:px-3 rounded-full bg-[#1D1D1F] hover:bg-black active:scale-[0.98] text-white text-[12.5px] md:text-[12px] font-medium transition disabled:opacity-50"
        title="开始演化"
      >
        <Play class="w-3 h-3 fill-current" />
        <span class="hidden min-[380px]:inline md:hidden">演化</span>
        <span class="hidden md:@[520px]:inline">开始演化</span>
      </button>
      <button
        v-else
        type="button"
        @click="triggerHaptic('medium'); emit('pause')"
        class="flex items-center gap-1.5 h-9 md:h-7 px-3.5 md:px-3 rounded-full bg-amber-500 hover:bg-amber-600 active:scale-[0.98] text-white text-[12.5px] md:text-[12px] font-medium transition"
        title="暂停演化"
      >
        <Pause class="w-3 h-3 fill-current" />
        <span>暂停</span>
      </button>

      <!-- Split Step Button (Compact icon-only on < md to keep header clean, borderless ghost on desktop) -->
      <div class="relative flex items-center">
        <div
          v-if="stepMenuOpen"
          @click="stepMenuOpen = false"
          class="fixed inset-0 z-20"
        ></div>

        <div class="flex items-center h-9 md:h-7 rounded-full md:rounded-lg border md:border-transparent border-black/[0.08] bg-[#F2F2F7] md:bg-transparent md:hover:bg-black/[0.04] overflow-hidden transition-colors">
          <button
            type="button"
            @click="triggerHaptic('light'); triggerStep(null)"
            :disabled="isRunning || isStepping"
            class="flex items-center gap-1 h-full px-2.5 md:px-2 text-[12.5px] md:text-[12px] font-medium text-[#1D1D1F] active:bg-black/10 transition-colors disabled:opacity-40"
            title="按顺序单步推进一轮对话"
          >
            <StepForward class="w-3.5 h-3.5 text-slate-600" />
            <span class="hidden md:@[580px]:inline text-slate-700">单步</span>
          </button>
          <button
            v-if="isDesktopOrWideTablet"
            type="button"
            @click="stepMenuOpen = !stepMenuOpen"
            :disabled="isRunning || isStepping || agents.length === 0"
            class="flex items-center h-full px-1.5 text-slate-500 hover:text-[#1D1D1F] active:bg-black/10 transition-colors disabled:opacity-40"
            title="点名指定角色发言"
          >
            <ChevronDown class="w-3.5 h-3.5" />
          </button>
        </div>

        <div
          v-if="stepMenuOpen"
          class="absolute right-0 top-full mt-2 z-30 w-56 md:w-52 p-1.5 rounded-2xl bg-white/95 backdrop-blur-xl border border-black/[0.08] shadow-[0_14px_36px_rgba(0,0,0,0.14)] space-y-0.5"
        >
          <button
            type="button"
            @click="triggerStep(null)"
            class="w-full flex items-center justify-between gap-2 px-3 py-2 rounded-xl text-[12.5px] text-slate-700 hover:bg-black/[0.05] transition-colors text-left"
          >
            <span class="flex items-center gap-2">
              <StepForward class="w-3.5 h-3.5 text-slate-500" />
              <span>顺位单步</span>
            </span>
            <span class="text-[11px] text-slate-400">默认</span>
          </button>
          <div class="my-1 border-t border-black/[0.06]"></div>
          <div class="px-3 py-1 text-[11px] font-semibold text-slate-400">
            点名指定发言角色
          </div>
          <button
            v-for="ag in agents"
            :key="ag.id"
            type="button"
            @click="triggerStep(ag.id)"
            class="w-full flex items-center justify-between gap-2 px-3 py-2 rounded-xl text-[12.5px] text-[#1D1D1F] hover:bg-[#007AFF]/10 hover:text-[#0062CC] transition-colors text-left"
          >
            <span class="flex items-center gap-2 min-w-0">
              <UserCheck class="w-3.5 h-3.5 shrink-0 text-[#007AFF]" />
              <span class="truncate">{{ ag.label }}</span>
            </span>
            <span
              v-if="experiment?.nextSpeaker === ag.id"
              class="text-[10.5px] px-1.5 py-0.5 rounded-md bg-black/[0.05] text-slate-500 shrink-0"
            >
              下一位
            </span>
          </button>
        </div>
      </div>

      <!-- Mobile More Actions Menu -->
      <DropdownMenuRoot v-if="!isDesktopOrWideTablet">
        <DropdownMenuTrigger as-child>
          <button
            type="button"
            class="w-9 h-9 rounded-full flex items-center justify-center border border-black/[0.08] transition-colors active:scale-95"
            :class="replayActive ? 'bg-[#007AFF] text-white border-[#007AFF]' : 'bg-[#F2F2F7] text-slate-700'"
            aria-label="更多功能"
          >
            <MoreHorizontal class="w-4 h-4" />
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuPortal>
          <DropdownMenuContent
            align="end"
            :side-offset="6"
            class="z-50 min-w-[184px] rounded-2xl bg-white/95 backdrop-blur-2xl border border-black/[0.08] shadow-[0_14px_38px_rgba(0,0,0,0.16)] p-1.5 text-[13px] text-[#1D1D1F] outline-none"
          >
            <DropdownMenuItem
              @select="emit('toggleBlindGuess')"
              class="flex items-center gap-2.5 px-3 py-2.5 rounded-xl cursor-pointer outline-none hover:bg-black/[0.05]"
            >
              <EyeOff v-if="blindGuessMode" class="w-4 h-4 text-violet-600" />
              <Eye v-else class="w-4 h-4 text-slate-600" />
              <span>{{ blindGuessMode ? "退出盲猜模式（揭晓底牌）" : "盲猜模式（遮蔽思考与密谈）" }}</span>
            </DropdownMenuItem>
            <DropdownMenuItem
              :disabled="isRunning || isStepping || (experiment?.messages.length ?? 0) === 0"
              @select="emit('toggleReplay')"
              class="flex items-center gap-2.5 px-3 py-2.5 rounded-xl cursor-pointer outline-none hover:bg-black/[0.05] data-[disabled]:opacity-40"
            >
              <Film class="w-4 h-4 text-[#007AFF]" />
              <span>{{ replayActive ? "退出剧场回放" : "剧场回放模式" }}</span>
            </DropdownMenuItem>
            <template v-if="agents.length > 0">
              <DropdownMenuSeparator class="my-1 h-px bg-black/[0.06]" />
              <div class="px-3 py-1 text-[11px] font-semibold text-slate-400">
                点名指定角色发言
              </div>
              <DropdownMenuItem
                v-for="ag in agents"
                :key="ag.id"
                :disabled="isRunning || isStepping"
                @select="triggerStep(ag.id)"
                class="flex items-center justify-between gap-2 px-3 py-2 rounded-xl cursor-pointer outline-none hover:bg-[#007AFF]/10 data-[disabled]:opacity-40"
              >
                <span class="flex items-center gap-2 min-w-0">
                  <UserCheck class="w-3.5 h-3.5 text-[#007AFF] shrink-0" />
                  <span class="truncate">{{ ag.label }}</span>
                </span>
                <span
                  v-if="experiment?.nextSpeaker === ag.id"
                  class="text-[10.5px] px-1.5 py-0.5 rounded bg-black/[0.05] text-slate-500 shrink-0"
                >
                  下一位
                </span>
              </DropdownMenuItem>
            </template>
            <DropdownMenuSeparator class="my-1 h-px bg-black/[0.06]" />
            <DropdownMenuItem
              @select="emit('openSettings')"
              class="flex items-center gap-2.5 px-3 py-2.5 rounded-xl cursor-pointer outline-none hover:bg-black/[0.05]"
            >
              <SlidersHorizontal class="w-4 h-4 text-slate-600" />
              <span>模型与生态设置</span>
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenuPortal>
      </DropdownMenuRoot>

      <!-- Desktop Borderless Linear Icon Actions (Settings entry lives cleanly in LeftSidebar & metric click) -->
      <template v-if="isDesktopOrWideTablet">
        <button
          type="button"
          @click="triggerHaptic('light'); emit('toggleBlindGuess')"
          class="h-8 flex items-center gap-1 px-2.5 rounded-lg text-[12px] font-medium transition-colors outline-none"
          :class="
            blindGuessMode
              ? 'bg-violet-50 text-violet-700'
              : 'text-slate-500 hover:text-[#1D1D1F] hover:bg-black/[0.05]'
          "
          :title="blindGuessMode ? '盲猜模式已开启（点击揭晓全员思考、私密备忘与里屋密谈）' : '开启盲猜模式（遮蔽幕后思考、私密里屋与暗涌关系）'"
        >
          <EyeOff v-if="blindGuessMode" class="w-3.5 h-3.5" />
          <Eye v-else class="w-3.5 h-3.5" />
          <span class="hidden @[640px]:inline">盲猜模式</span>
        </button>

        <button
          type="button"
          @click="emit('toggleReplay')"
          :disabled="isRunning || isStepping || (experiment?.messages.length ?? 0) === 0"
          class="w-8 h-8 flex items-center justify-center rounded-lg transition-colors disabled:opacity-35 outline-none"
          :class="
            replayActive
              ? 'bg-[#007AFF]/12 text-[#007AFF]'
              : 'text-slate-500 hover:text-[#1D1D1F] hover:bg-black/[0.05]'
          "
          :title="replayActive ? '退出剧场回放' : '剧场回放模式'"
        >
          <Film class="w-4 h-4" />
        </button>

        <button
          type="button"
          @click="triggerHaptic('light'); emit('toggleInspector')"
          class="relative w-8 h-8 flex items-center justify-center rounded-lg transition-colors outline-none"
          :class="
            inspectorOpen
              ? 'bg-black/[0.06] text-[#1D1D1F]'
              : 'text-slate-500 hover:text-[#1D1D1F] hover:bg-black/[0.05]'
          "
          title="展开/收起右侧观察舱与共享白板 (Ctrl+\)"
        >
          <PanelRightClose v-if="inspectorOpen" class="w-4 h-4" />
          <PanelRight v-else class="w-4 h-4" />
          <span
            v-if="hasPendingApproval"
            class="absolute top-1.5 right-1.5 w-2 h-2 rounded-full bg-amber-500 animate-ping"
          ></span>
        </button>
      </template>
    </div>
  </header>
</template>
