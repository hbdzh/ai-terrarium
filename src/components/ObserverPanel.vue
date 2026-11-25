<script setup lang="ts">
import { computed, ref } from "vue";
import {
  Telescope,
  Sparkles,
  Zap,
  RefreshCw,
  FileText,
  X,
  Copy,
  Check,
} from "lucide-vue-next";
import type { ObserverNote } from "../types";
import { renderMarkdown } from "../utils/markdown";

const props = defineProps<{
  notes: ObserverNote[];
  observerRunning: boolean;
  observerInterval: number;
  totalTurns: number;
  hasObserverKey: boolean;
  finalReport: string;
  generatingReport?: boolean;
}>();

const emit = defineEmits<{
  jumpToTurn: [turnNumber: number];
  triggerNow: [];
  generateReport: [];
}>();

const reportModalOpen = ref(false);
const copiedReport = ref(false);
const hoveredTurn = ref<ObserverNote | null>(null);
const showTension = ref(true);
const showAbstraction = ref(true);
const showStagnation = ref(true);

const latestNote = computed(() => props.notes[0] ?? null);
const chronologicalNotes = computed(() =>
  [...props.notes].sort((a, b) => a.triggerTurn - b.triggerTurn)
);

const stages = ["破冰试探", "观点交锋", "概念抽象", "存在主义深渊"];

const activeStageIndex = computed(() => {
  if (!latestNote.value) return -1;
  return stages.findIndex((s) => latestNote.value?.depthStage.includes(s));
});

const stagnationColor = computed(() => {
  const v = latestNote.value?.stagnationIndex ?? 0;
  if (v > 80) return "bg-rose-500";
  if (v >= 55) return "bg-amber-500";
  return "bg-emerald-500";
});

const stagnationTextColor = computed(() => {
  const v = latestNote.value?.stagnationIndex ?? 0;
  if (v > 80) return "text-rose-600";
  if (v >= 55) return "text-amber-600";
  return "text-emerald-600";
});

// SVG Chart geometry (320 x 116)
const chartW = 320;
const chartH = 116;
const padL = 24;
const padR = 12;
const padT = 10;
const padB = 20;

interface ChartPoint {
  x: number;
  tensionY: number;
  abstractionY: number;
  stagnationY: number;
  note: ObserverNote;
}

const chartPoints = computed<ChartPoint[]>(() => {
  const list = chronologicalNotes.value;
  if (list.length === 0) return [];
  const innerW = chartW - padL - padR;
  const innerH = chartH - padT - padB;
  const yFor = (val: number) =>
    padT + innerH - (Math.max(0, Math.min(100, val)) / 100) * innerH;

  return list.map((note, idx) => {
    const x =
      list.length === 1
        ? padL + innerW / 2
        : padL + (idx / (list.length - 1)) * innerW;
    return {
      x,
      tensionY: yFor(note.tensionScore ?? 0),
      abstractionY: yFor(note.abstractionScore ?? 0),
      stagnationY: yFor(note.stagnationIndex ?? 0),
      note,
    };
  });
});

function buildPolyline(getter: (p: ChartPoint) => number): string {
  return chartPoints.value.map((p) => `${p.x.toFixed(1)},${getter(p).toFixed(1)}`).join(" ");
}

function openReportModal() {
  reportModalOpen.value = true;
  if (!props.finalReport.trim() && !props.generatingReport && props.totalTurns > 0) {
    emit("generateReport");
  }
}

async function copyReport() {
  if (!props.finalReport.trim()) return;
  try {
    await navigator.clipboard.writeText(props.finalReport);
    copiedReport.value = true;
    setTimeout(() => {
      copiedReport.value = false;
    }, 2000);
  } catch {
    // ignore clipboard errors
  }
}
</script>

<template>
  <aside
    class="h-full flex flex-col bg-[#FAFAFC] border-l border-black/[0.06] overflow-y-auto md:overflow-hidden"
  >
    <!-- Top Eco-Dashboard -->
    <div class="p-4 border-b border-black/[0.06] bg-white/80 backdrop-blur-md space-y-3.5 shrink-0 md:shrink md:overflow-y-auto md:max-h-[60%]">
      <div class="flex items-center justify-between gap-1.5">
        <div class="flex items-center gap-2 min-w-0">
          <div class="w-7 h-7 rounded-xl bg-slate-900 text-white flex items-center justify-center shrink-0">
            <Telescope class="w-3.5 h-3.5" />
          </div>
          <div class="min-w-0">
            <h3 class="text-xs font-semibold text-[#1D1D1F] truncate">观察员视界 · Node C</h3>
            <p class="text-[10.5px] text-slate-400 truncate">
              每 {{ observerInterval }} 轮异步多维诊断
            </p>
          </div>
        </div>

        <div class="flex items-center gap-1 shrink-0">
          <button
            @click="openReportModal"
            :disabled="totalTurns === 0"
            class="flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-medium bg-[#F5F5F7] hover:bg-[#EAEAEF] text-slate-700 border border-black/[0.05] transition disabled:opacity-45"
            title="生成或查看实验结案复盘简报"
          >
            <FileText class="w-3 h-3" />
            <span>复盘简报</span>
          </button>

          <button
            @click="emit('triggerNow')"
            :disabled="observerRunning || totalTurns === 0 || !hasObserverKey"
            class="flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-medium bg-[#1D1D1F] hover:bg-black text-white transition disabled:opacity-45"
            title="立即对当前对话执行一次观察分析"
          >
            <RefreshCw
              class="w-3 h-3"
              :class="{ 'animate-spin': observerRunning }"
            />
            <span>{{ observerRunning ? "分析中" : "观察" }}</span>
          </button>
        </div>
      </div>

      <!-- Multi-Dimensional Evolution Curves (SVG) -->
      <div class="rounded-2xl bg-[#FAFAFC] border border-black/[0.06] p-2.5">
        <div class="flex flex-wrap items-center justify-between gap-1 mb-1.5">
          <span class="text-[10.5px] font-semibold text-slate-500">认知演化曲线</span>
          <div class="flex items-center gap-1.5 text-[10px]">
            <button
              type="button"
              @click="showTension = !showTension"
              class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full border transition"
              :class="showTension ? 'bg-rose-50 text-rose-700 border-rose-200' : 'text-slate-400 border-transparent opacity-50'"
            >
              <span class="w-1.5 h-1.5 rounded-full bg-rose-500"></span>
              火药味
            </button>
            <button
              type="button"
              @click="showAbstraction = !showAbstraction"
              class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full border transition"
              :class="showAbstraction ? 'bg-blue-50 text-[#007AFF] border-blue-200' : 'text-slate-400 border-transparent opacity-50'"
            >
              <span class="w-1.5 h-1.5 rounded-full bg-[#007AFF]"></span>
              抽象度
            </button>
            <button
              type="button"
              @click="showStagnation = !showStagnation"
              class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full border transition"
              :class="showStagnation ? 'bg-amber-50 text-amber-700 border-amber-200' : 'text-slate-400 border-transparent opacity-50'"
            >
              <span class="w-1.5 h-1.5 rounded-full bg-amber-500"></span>
              僵化度
            </button>
          </div>
        </div>

        <div v-if="chartPoints.length === 0" class="py-4 text-center text-[11px] text-slate-400">
          暂无评分轨迹，等待首次观察触发
        </div>

        <div v-else class="relative">
          <svg
            :viewBox="`0 0 ${chartW} ${chartH}`"
            class="w-full h-28 overflow-visible select-none"
          >
            <!-- Y gridlines -->
            <line :x1="padL" :y1="padT" :x2="chartW - padR" :y2="padT" stroke="#E2E8F0" stroke-dasharray="2 2" stroke-width="1" />
            <line :x1="padL" :y1="padT + (chartH - padT - padB) / 2" :x2="chartW - padR" :y2="padT + (chartH - padT - padB) / 2" stroke="#E2E8F0" stroke-dasharray="2 2" stroke-width="1" />
            <line :x1="padL" :y1="chartH - padB" :x2="chartW - padR" :y2="chartH - padB" stroke="#CBD5E1" stroke-width="1" />

            <text :x="padL - 4" :y="padT + 3" text-anchor="end" class="fill-slate-400 text-[8px]">100</text>
            <text :x="padL - 4" :y="padT + (chartH - padT - padB) / 2 + 3" text-anchor="end" class="fill-slate-400 text-[8px]">50</text>
            <text :x="padL - 4" :y="chartH - padB + 3" text-anchor="end" class="fill-slate-400 text-[8px]">0</text>

            <!-- Polylines -->
            <polyline
              v-if="showTension && chartPoints.length >= 2"
              :points="buildPolyline((p) => p.tensionY)"
              fill="none"
              stroke="#F43F5E"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
            <polyline
              v-if="showAbstraction && chartPoints.length >= 2"
              :points="buildPolyline((p) => p.abstractionY)"
              fill="none"
              stroke="#007AFF"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
            <polyline
              v-if="showStagnation && chartPoints.length >= 2"
              :points="buildPolyline((p) => p.stagnationY)"
              fill="none"
              stroke="#F59E0B"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            />

            <!-- Clickable Data Nodes -->
            <g v-for="pt in chartPoints" :key="pt.note.id">
              <circle
                v-if="showTension"
                :cx="pt.x"
                :cy="pt.tensionY"
                r="3.5"
                fill="#F43F5E"
                stroke="#ffffff"
                stroke-width="1.2"
                class="cursor-pointer"
                @mouseenter="hoveredTurn = pt.note"
                @mouseleave="hoveredTurn = null"
                @click="emit('jumpToTurn', pt.note.triggerTurn)"
              />
              <circle
                v-if="showAbstraction"
                :cx="pt.x"
                :cy="pt.abstractionY"
                r="3.5"
                fill="#007AFF"
                stroke="#ffffff"
                stroke-width="1.2"
                class="cursor-pointer"
                @mouseenter="hoveredTurn = pt.note"
                @mouseleave="hoveredTurn = null"
                @click="emit('jumpToTurn', pt.note.triggerTurn)"
              />
              <circle
                v-if="showStagnation"
                :cx="pt.x"
                :cy="pt.stagnationY"
                r="3.5"
                fill="#F59E0B"
                stroke="#ffffff"
                stroke-width="1.2"
                class="cursor-pointer"
                @mouseenter="hoveredTurn = pt.note"
                @mouseleave="hoveredTurn = null"
                @click="emit('jumpToTurn', pt.note.triggerTurn)"
              />
              <text
                :x="pt.x"
                :y="chartH - 5"
                text-anchor="middle"
                class="fill-slate-400 text-[8.5px] cursor-pointer hover:fill-[#007AFF]"
                @click="emit('jumpToTurn', pt.note.triggerTurn)"
              >
                #{{ pt.note.triggerTurn }}
              </text>
            </g>
          </svg>

          <div
            v-if="hoveredTurn"
            class="mt-1 px-2 py-1 rounded-lg bg-white border border-black/[0.06] text-[10.5px] text-slate-600 flex items-center justify-between"
          >
            <span class="font-semibold text-[#1D1D1F]">第 {{ hoveredTurn.triggerTurn }} 轮 (点击定位)</span>
            <span class="tabular-nums">
              火 {{ hoveredTurn.tensionScore }} · 抽 {{ hoveredTurn.abstractionScore }} · 僵 {{ hoveredTurn.stagnationIndex }}
            </span>
          </div>
        </div>
      </div>

      <!-- Topic Tags & Depth Stage -->
      <div class="grid grid-cols-1 gap-2.5">
        <div v-if="latestNote && latestNote.tags.length > 0" class="flex flex-wrap gap-1.5">
          <span
            v-for="tag in latestNote.tags"
            :key="tag"
            class="px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-[#F0F4F9] text-slate-700 border border-slate-200/70"
          >
            # {{ tag }}
          </span>
        </div>

        <div>
          <div class="flex items-center justify-between text-[10.5px] mb-1">
            <span class="font-medium text-slate-400">深度阶段：{{ latestNote?.depthStage ?? "尚未定级" }}</span>
            <div class="flex items-center gap-1.5">
              <span
                v-if="latestNote?.perturbationInjected"
                class="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded-md bg-rose-50 text-rose-600 text-[10px] font-medium"
              >
                <Zap class="w-2.5 h-2.5" />
                破局扰动
              </span>
              <span class="font-semibold tabular-nums" :class="stagnationTextColor">
                僵化 {{ latestNote?.stagnationIndex ?? 0 }}/100
              </span>
            </div>
          </div>
          <div class="grid grid-cols-4 gap-1 mb-1.5">
            <div
              v-for="(stage, idx) in stages"
              :key="stage"
              class="h-1 rounded-full transition-all"
              :class="activeStageIndex >= idx ? 'bg-[#007AFF]' : 'bg-slate-200/80'"
              :title="stage"
            ></div>
          </div>
          <div class="h-1.5 rounded-full bg-slate-200/80 overflow-hidden">
            <div
              class="h-full rounded-full transition-all duration-500"
              :class="stagnationColor"
              :style="{ width: `${latestNote?.stagnationIndex ?? 0}%` }"
            ></div>
          </div>
        </div>
      </div>
    </div>

    <!-- Bottom Observer Notes Timeline (reverse chronological) -->
    <div class="md:flex-1 md:overflow-y-auto p-4 space-y-3">
      <div class="flex items-center justify-between px-0.5">
        <span class="text-[11px] font-medium text-slate-400">
          观察手记时间轴 ({{ notes.length }})
        </span>
        <span class="text-[10px] text-slate-400">点击节点或卡片定位轮次</span>
      </div>

      <div
        v-if="notes.length === 0"
        class="rounded-2xl border border-dashed border-slate-200 p-6 text-center text-xs text-slate-400 leading-relaxed"
      >
        <Sparkles class="w-5 h-5 mx-auto mb-2 text-slate-300" />
        <span v-if="!hasObserverKey">
          请先在「模型与设置」中配置观察员 C 的 API Key。
        </span>
        <span v-else>
          对话推进满 {{ observerInterval }} 轮后，观察员将在此留下多维评分、犀利手记与转折事件卡。
        </span>
      </div>

      <button
        v-for="item in notes"
        :key="item.id"
        @click="emit('jumpToTurn', item.triggerTurn)"
        class="w-full text-left rounded-2xl bg-white p-3.5 border border-black/[0.06] shadow-[0_2px_10px_rgba(0,0,0,0.03)] hover:shadow-[0_6px_20px_rgba(0,0,0,0.07)] hover:border-[#007AFF]/30 transition group"
      >
        <div class="flex items-center justify-between mb-1.5 gap-2">
          <span
            class="px-2 py-0.5 rounded-full text-[10.5px] font-semibold bg-slate-100 text-slate-700 group-hover:bg-[#007AFF]/10 group-hover:text-[#0062CC] transition tabular-nums shrink-0"
          >
            第 {{ item.triggerTurn }} 轮
          </span>
          <span class="text-[10.5px] font-medium text-slate-500 truncate">
            {{ item.depthStage }}
          </span>
          <span class="text-[10px] text-slate-400 tabular-nums shrink-0">
            火{{ item.tensionScore ?? 0 }} / 抽{{ item.abstractionScore ?? 0 }} / 僵{{ item.stagnationIndex }}
          </span>
        </div>

        <p class="text-[12.5px] leading-relaxed text-[#1D1D1F] mb-2">
          {{ item.note }}
        </p>

        <div class="flex flex-wrap items-center gap-1">
          <span
            v-for="t in item.tags"
            :key="t"
            class="px-2 py-0.5 rounded-md bg-slate-50 text-slate-500 text-[10px] border border-slate-200/60"
          >
            {{ t }}
          </span>
          <span
            v-if="item.perturbationInjected"
            class="ml-auto text-[10px] font-medium text-rose-600"
          >
            ⚡ 自动扰动
          </span>
        </div>
      </button>
    </div>

    <!-- Final Report Modal -->
    <Teleport to="body">
      <div v-if="reportModalOpen" class="fixed inset-0 z-50 flex items-center justify-center p-4">
        <div
          @click="reportModalOpen = false"
          class="fixed inset-0 bg-black/25 backdrop-blur-[2px]"
        ></div>
        <div
          class="relative w-full max-w-2xl max-h-[85vh] flex flex-col rounded-3xl bg-white shadow-2xl border border-black/[0.08] overflow-hidden z-10"
        >
          <div class="px-6 py-4 border-b border-black/[0.06] flex items-center justify-between">
            <div class="flex items-center gap-2">
              <FileText class="w-4 h-4 text-[#007AFF]" />
              <h2 class="text-sm font-semibold text-[#1D1D1F]">实验结案复盘简报</h2>
            </div>
            <div class="flex items-center gap-1.5">
              <button
                type="button"
                @click="copyReport"
                :disabled="!finalReport.trim()"
                class="flex items-center gap-1 px-3 py-1 rounded-full text-xs bg-[#F5F5F7] hover:bg-[#EAEAEF] text-slate-700 disabled:opacity-40"
              >
                <Check v-if="copiedReport" class="w-3.5 h-3.5 text-emerald-600" />
                <Copy v-else class="w-3.5 h-3.5" />
                <span>{{ copiedReport ? "已复制" : "复制" }}</span>
              </button>
              <button
                type="button"
                @click="emit('generateReport')"
                :disabled="generatingReport || totalTurns === 0"
                class="flex items-center gap-1 px-3 py-1 rounded-full text-xs bg-[#1D1D1F] hover:bg-black text-white disabled:opacity-45"
              >
                <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': generatingReport }" />
                <span>{{ generatingReport ? "生成中..." : "重新生成" }}</span>
              </button>
              <button
                type="button"
                @click="reportModalOpen = false"
                class="p-1.5 rounded-full hover:bg-slate-100 text-slate-500"
              >
                <X class="w-4 h-4" />
              </button>
            </div>
          </div>

          <div class="flex-1 overflow-y-auto p-6">
            <div
              v-if="generatingReport && !finalReport.trim()"
              class="py-16 text-center text-xs text-slate-400 space-y-2"
            >
              <RefreshCw class="w-6 h-6 mx-auto animate-spin text-[#007AFF]" />
              <div>观察员 C 正在研读全量演化档案并撰写结案复盘简报...</div>
            </div>
            <div
              v-else-if="!finalReport.trim()"
              class="py-12 text-center text-xs text-slate-400"
            >
              点击右上角「重新生成」即可总结核心转折点、高光金句 Top 3 与智能体行为画像。
            </div>
            <div
              v-else
              class="terrarium-prose"
              v-html="renderMarkdown(finalReport, false)"
            ></div>
          </div>
        </div>
      </div>
    </Teleport>
  </aside>
</template>
