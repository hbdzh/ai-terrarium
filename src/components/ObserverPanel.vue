<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  Telescope,
  Sparkles,
  Zap,
  RefreshCw,
  FileText,
  X,
  Copy,
  Check,
  BookmarkPlus,
  GitCompare,
  Network,
  EyeOff,
} from "lucide-vue-next";
import type { ExperimentSummary, NodeConfig, ObserverNote } from "../types";
import { renderMarkdown } from "../utils/markdown";
import { writeClipboardText } from "../utils/clipboard";
import AdaptiveModal from "./ui/AdaptiveModal.vue";

const props = defineProps<{
  notes: ObserverNote[];
  observerRunning: boolean;
  observerInterval: number;
  totalTurns: number;
  hasObserverKey: boolean;
  finalReport: string;
  generatingReport?: boolean;
  agents?: NodeConfig[];
  blindGuessMode?: boolean;
  relatedBranches?: ExperimentSummary[];
  distillingImprints?: boolean;
  comparisonReport?: string;
  generatingComparison?: boolean;
}>();

const emit = defineEmits<{
  jumpToTurn: [turnNumber: number];
  triggerNow: [];
  generateReport: [];
  saveCharacterCard: [agentId: string];
  distillImprints: [];
  generateComparison: [otherExpId: string];
}>();

const reportModalOpen = ref(false);
const reportViewTab = ref<"final" | "comparison">("final");
const selectedCompareBranchId = ref("");
const copiedReport = ref(false);
const savedAgentIds = ref<Record<string, boolean>>({});
const networkExpanded = ref(true);

watch(
  () => props.relatedBranches,
  (list) => {
    if (list && list.length > 0 && !list.some((b) => b.id === selectedCompareBranchId.value)) {
      selectedCompareBranchId.value = list[0].id;
    } else if (!list || list.length === 0) {
      selectedCompareBranchId.value = "";
      reportViewTab.value = "final";
    }
  },
  { immediate: true }
);

watch(
  () => [props.finalReport, props.agents],
  () => {
    savedAgentIds.value = {};
  }
);
const chartExpanded = ref(false);
const hoveredTurn = ref<ObserverNote | null>(null);
const showTension = ref(true);
const showAbstraction = ref(true);
const showStagnation = ref(true);

const latestNote = computed(() => props.notes[0] ?? null);
const chronologicalNotes = computed(() =>
  [...props.notes].sort((a, b) => a.triggerTurn - b.triggerTurn)
);

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

const polylines = computed(() => {
  const pts = chartPoints.value;
  if (pts.length < 2) {
    return { tension: "", abstraction: "", stagnation: "" };
  }
  const t: string[] = [];
  const a: string[] = [];
  const s: string[] = [];
  for (const p of pts) {
    const x = p.x.toFixed(1);
    t.push(`${x},${p.tensionY.toFixed(1)}`);
    a.push(`${x},${p.abstractionY.toFixed(1)}`);
    s.push(`${x},${p.stagnationY.toFixed(1)}`);
  }
  return {
    tension: t.join(" "),
    abstraction: a.join(" "),
    stagnation: s.join(" "),
  };
});

function showXAxisLabel(idx: number): boolean {
  const n = chartPoints.value.length;
  if (n <= 6) return true;
  const step = Math.ceil(n / 6);
  return idx === 0 || idx === n - 1 || idx % step === 0;
}

function openReportModal() {
  reportModalOpen.value = true;
  if (!props.finalReport.trim() && !props.generatingReport && props.totalTurns > 0) {
    emit("generateReport");
  }
}

async function copyReport() {
  const text =
    reportViewTab.value === "comparison"
      ? (props.comparisonReport ?? "")
      : props.finalReport;
  if (!text.trim()) return;
  try {
    await writeClipboardText(text);
    copiedReport.value = true;
    setTimeout(() => {
      copiedReport.value = false;
    }, 2000);
  } catch {
    // ignore clipboard errors
  }
}

function handleSaveCard(agentId: string) {
  emit("saveCharacterCard", agentId);
  savedAgentIds.value[agentId] = true;
}

defineExpose({ openReportModal });
</script>

<template>
  <aside
    class="h-full flex flex-col bg-white overflow-y-auto md:overflow-hidden antialiased"
  >
    <!-- Top Eco-Dashboard (clean hairline-separated surface, minimal borders) -->
    <div class="p-3.5 md:p-4 border-b border-black/[0.05] bg-white space-y-3 shrink-0">
      <!-- Top Header -->
      <div class="flex items-center justify-between gap-2">
        <div class="flex items-center gap-2.5 min-w-0">
          <div class="w-7 h-7 rounded-lg bg-[#1D1D1F] text-white flex items-center justify-center shrink-0">
            <Telescope class="w-3.5 h-3.5" />
          </div>
          <div class="min-w-0">
            <h3 class="text-[13.5px] font-semibold text-[#1D1D1F] truncate leading-snug">观察员视角</h3>
            <p class="text-[11.5px] text-slate-400 truncate leading-snug">
              每 {{ observerInterval }} 轮异步多维诊断
            </p>
          </div>
        </div>

        <div class="flex items-center gap-1.5 shrink-0">
          <button
            type="button"
            @click="openReportModal"
            :disabled="totalTurns === 0"
            class="h-8 flex items-center gap-1.5 px-2.5 rounded-lg text-[12px] font-medium text-slate-600 hover:text-[#1D1D1F] hover:bg-black/[0.05] transition-colors disabled:opacity-40"
            title="生成或查看实验结案复盘简报"
          >
            <FileText class="w-3.5 h-3.5" />
            <span>复盘简报</span>
          </button>

          <button
            type="button"
            @click="emit('triggerNow')"
            :disabled="observerRunning || totalTurns === 0 || !hasObserverKey"
            class="h-8 flex items-center gap-1.5 px-3 rounded-full text-[12px] font-medium bg-[#1D1D1F] hover:bg-black text-white transition-colors disabled:opacity-40"
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

      <!-- 3-Metric Summary Tiles (borderless subtle tint pills) -->
      <div v-if="latestNote" class="grid grid-cols-3 gap-2">
        <div class="rounded-xl bg-black/[0.035] px-3 py-2 flex items-center justify-between">
          <span class="text-[11.5px] text-slate-500">火药味</span>
          <span class="text-[13.5px] font-semibold tabular-nums text-rose-600">{{ latestNote.tensionScore ?? 0 }}</span>
        </div>
        <div class="rounded-xl bg-black/[0.035] px-3 py-2 flex items-center justify-between">
          <span class="text-[11.5px] text-slate-500">抽象度</span>
          <span class="text-[13.5px] font-semibold tabular-nums text-[#007AFF]">{{ latestNote.abstractionScore ?? 0 }}</span>
        </div>
        <div class="rounded-xl bg-black/[0.035] px-3 py-2 flex items-center justify-between">
          <span class="text-[11.5px] text-slate-500">僵化度</span>
          <span class="text-[13.5px] font-semibold tabular-nums" :class="stagnationTextColor">{{ latestNote.stagnationIndex ?? 0 }}</span>
        </div>
      </div>

      <!-- Collapsible Multi-Dimensional Evolution Curves (SVG) -->
      <div class="rounded-xl bg-white/80 border border-black/[0.045] px-3 py-2">
        <div class="flex items-center justify-between gap-2">
          <button
            type="button"
            @click="chartExpanded = !chartExpanded"
            class="w-full flex items-center justify-between gap-2 text-[12px] font-medium text-slate-600 hover:text-[#1D1D1F] transition-colors"
          >
            <span>认知演化曲线</span>
            <span class="text-[11.5px] font-medium text-[#007AFF]">
              {{ chartExpanded ? "收起" : latestNote ? "展开轨迹" : "暂无轨迹" }}
            </span>
          </button>
        </div>

        <template v-if="chartExpanded">
          <div class="flex flex-wrap items-center justify-end gap-1.5 text-[10px] mt-2 mb-1">
            <button
              type="button"
              @click="showTension = !showTension"
              class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full border transition-colors"
              :class="showTension ? 'bg-rose-50 text-rose-700 border-rose-200' : 'text-slate-400 border-transparent opacity-50'"
            >
              <span class="w-1.5 h-1.5 rounded-full bg-rose-500"></span>
              火药味
            </button>
            <button
              type="button"
              @click="showAbstraction = !showAbstraction"
              class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full border transition-colors"
              :class="showAbstraction ? 'bg-blue-50 text-[#007AFF] border-blue-200' : 'text-slate-400 border-transparent opacity-50'"
            >
              <span class="w-1.5 h-1.5 rounded-full bg-[#007AFF]"></span>
              抽象度
            </button>
            <button
              type="button"
              @click="showStagnation = !showStagnation"
              class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full border transition-colors"
              :class="showStagnation ? 'bg-amber-50 text-amber-700 border-amber-200' : 'text-slate-400 border-transparent opacity-50'"
            >
              <span class="w-1.5 h-1.5 rounded-full bg-amber-500"></span>
              僵化度
            </button>
          </div>

          <div v-if="chartPoints.length === 0" class="py-3 text-center text-[11px] text-slate-400">
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
                :points="polylines.tension"
                fill="none"
                stroke="#F43F5E"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
              <polyline
                v-if="showAbstraction && chartPoints.length >= 2"
                :points="polylines.abstraction"
                fill="none"
                stroke="#007AFF"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
              <polyline
                v-if="showStagnation && chartPoints.length >= 2"
                :points="polylines.stagnation"
                fill="none"
                stroke="#F59E0B"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              />

              <!-- Clickable Data Nodes -->
              <g v-for="(pt, idx) in chartPoints" :key="pt.note.id">
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
                  v-if="showXAxisLabel(idx)"
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
        </template>
      </div>

      <!-- Collapsible Relationship Tension Network (暗涌关系网) -->
      <div class="rounded-xl bg-white/80 border border-black/[0.045] px-3 py-2">
        <button
          type="button"
          @click="networkExpanded = !networkExpanded"
          class="w-full flex items-center justify-between gap-2 text-[12px] font-medium text-slate-600 hover:text-[#1D1D1F] transition-colors"
        >
          <span class="flex items-center gap-1.5">
            <Network class="w-3.5 h-3.5 text-slate-500" />
            <span>暗涌关系网</span>
          </span>
          <span class="text-[11.5px] font-medium text-[#007AFF]">
            {{
              blindGuessMode
                ? "盲猜遮蔽中"
                : networkExpanded
                  ? "收起"
                  : latestNote?.relationshipMatrix?.length
                    ? `${latestNote.relationshipMatrix.length} 条暗流`
                    : "暂无数据"
            }}
          </span>
        </button>

        <template v-if="networkExpanded">
          <div
            v-if="blindGuessMode"
            class="mt-2 py-2.5 px-3 rounded-lg bg-[#F5F5F7] text-[11px] text-slate-500 flex items-center gap-1.5"
          >
            <EyeOff class="w-3.5 h-3.5 text-slate-400 shrink-0" />
            <span>盲猜模式已开启，人物信任温度与心理定性已遮蔽</span>
          </div>
          <div
            v-else-if="!latestNote?.relationshipMatrix || latestNote.relationshipMatrix.length === 0"
            class="py-2.5 text-center text-[11px] text-slate-400"
          >
            等待观察员诊断后自动生成角色实时信任温度
          </div>
          <div v-else class="mt-2 space-y-2">
            <div
              v-for="(edge, eIdx) in latestNote.relationshipMatrix"
              :key="`${edge.from}-${edge.to}-${eIdx}`"
              class="space-y-1"
            >
              <div class="flex items-center justify-between gap-2 text-[11px]">
                <div class="flex items-center gap-1 min-w-0 font-medium text-[#1D1D1F]">
                  <span class="truncate">{{ edge.from }}</span>
                  <span class="text-slate-400 shrink-0">→</span>
                  <span class="truncate">{{ edge.to }}</span>
                </div>
                <div class="flex items-center gap-1.5 shrink-0">
                  <span
                    class="px-1.5 py-0.5 rounded-full text-[10px] font-medium"
                    :class="
                      edge.trustScore < 0
                        ? 'bg-rose-50 text-rose-600'
                        : edge.trustScore >= 25
                          ? 'bg-emerald-50 text-emerald-700'
                          : 'bg-slate-100 text-slate-600'
                    "
                  >
                    {{ edge.attitudeTag }}
                  </span>
                  <span
                    class="font-mono font-semibold tabular-nums w-8 text-right"
                    :class="
                      edge.trustScore < 0
                        ? 'text-rose-600'
                        : edge.trustScore >= 25
                          ? 'text-emerald-600'
                          : 'text-slate-500'
                    "
                  >
                    {{ edge.trustScore > 0 ? `+${edge.trustScore}` : edge.trustScore }}
                  </span>
                </div>
              </div>
              <!-- Bidirectional mini progress bar centered at 0 (-100 .. 0 .. +100) -->
              <div class="relative h-1.5 rounded-full bg-slate-100 overflow-hidden">
                <div class="absolute left-1/2 top-0 bottom-0 w-px bg-slate-300 z-10"></div>
                <div
                  v-if="edge.trustScore < 0"
                  class="absolute top-0 bottom-0 right-1/2 bg-rose-500 rounded-l-full transition-all duration-300"
                  :style="{ width: `${Math.min(50, Math.abs(edge.trustScore) / 2)}%` }"
                ></div>
                <div
                  v-else
                  class="absolute top-0 bottom-0 left-1/2 bg-emerald-500 rounded-r-full transition-all duration-300"
                  :style="{ width: `${Math.min(50, edge.trustScore / 2)}%` }"
                ></div>
              </div>
            </div>
          </div>
        </template>
      </div>

      <!-- Topic Tags & Dynamic Depth Stage -->
      <div class="grid grid-cols-1 gap-2">
        <div v-if="latestNote && latestNote.tags.length > 0" class="flex flex-wrap gap-1.5">
          <span
            v-for="tag in latestNote.tags"
            :key="tag"
            class="px-2 py-0.5 rounded-full text-[10.5px] leading-relaxed font-medium bg-[#F0F4F9] text-slate-700 border border-slate-200/70 antialiased"
          >
            # {{ tag }}
          </span>
        </div>

        <div>
          <div class="flex items-center justify-between gap-2 text-[10.5px] leading-relaxed mb-1">
            <div class="flex items-center gap-1.5 min-w-0">
              <span class="text-slate-400 shrink-0">阶段</span>
              <span
                v-if="latestNote?.narrativeAct"
                class="px-1.5 py-0.5 rounded-full bg-violet-100 text-violet-700 font-semibold shrink-0 leading-normal antialiased"
                title="三幕式导演节拍器当前幕次"
              >
                {{ latestNote.narrativeAct }}
              </span>
              <span class="px-2 py-0.5 rounded-full bg-[#007AFF]/10 text-[#0062CC] font-semibold truncate leading-normal antialiased">
                {{ latestNote?.depthStage ?? "尚未定级" }}
              </span>
            </div>
            <div class="flex items-center gap-1.5 shrink-0">
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
          <div class="h-1.5 rounded-full bg-slate-200/80 overflow-hidden">
            <div
              class="h-full rounded-full transition-[width] duration-300"
              :class="stagnationColor"
              :style="{ width: `${latestNote?.stagnationIndex ?? 0}%` }"
            ></div>
          </div>
        </div>
      </div>
    </div>

    <!-- Bottom Observer Notes Timeline (reverse chronological) -->
    <div class="md:flex-1 md:overflow-y-auto p-4 pb-[calc(1.25rem+env(safe-area-inset-bottom))] md:pb-5 space-y-3 antialiased">
      <div class="flex items-center justify-between px-0.5">
        <span class="text-[12.5px] md:text-[12px] font-semibold text-slate-500 leading-relaxed">
          观察手记时间轴 ({{ notes.length }})
        </span>
        <span class="text-[11px] text-slate-400 leading-relaxed">点击轮次定位原文</span>
      </div>

      <div
        v-if="notes.length === 0"
        class="rounded-2xl border border-dashed border-slate-200 bg-white/60 p-6 text-center text-[13px] md:text-xs text-slate-400 leading-relaxed"
      >
        <Sparkles class="w-5 h-5 mx-auto mb-2 text-slate-300" />
        <span v-if="!hasObserverKey">
          请先在「模型与设置」中配置观察员 C 的 API Key。
        </span>
        <span v-else>
          对话推进满 {{ observerInterval }} 轮后，观察员将在此留下多维评分、犀利手记与转折事件卡。
        </span>
      </div>

      <div v-else class="relative pl-4 border-l-2 border-slate-200/80 space-y-3 ml-1.5">
        <div
          v-for="(item, idx) in notes"
          :key="item.id"
          class="relative w-full text-left rounded-2xl bg-white p-4 border border-black/[0.06] shadow-[0_1px_3px_rgba(0,0,0,0.03)] hover:border-[#007AFF]/40 transition-colors antialiased"
        >
          <!-- Timeline dot -->
          <span
            class="absolute -left-[22px] top-4 w-2.5 h-2.5 rounded-full ring-4 ring-white"
            :class="idx === 0 ? 'bg-[#007AFF]' : 'bg-slate-300'"
          ></span>

          <div class="flex flex-wrap items-center justify-between mb-2.5 gap-2">
            <div class="flex items-center gap-1.5 min-w-0">
              <button
                type="button"
                @click="emit('jumpToTurn', item.triggerTurn)"
                class="px-2.5 py-1 rounded-full text-[11.5px] leading-none font-semibold bg-[#F2F2F7] text-[#1D1D1F] hover:bg-[#007AFF]/15 hover:text-[#0062CC] transition-colors tabular-nums shrink-0 cursor-pointer"
                title="点击定位到该轮对话"
              >
                第 {{ item.triggerTurn }} 轮 ↗
              </button>
              <span
                v-if="item.narrativeAct"
                class="px-2 py-0.5 rounded-full bg-violet-50 text-violet-700 text-[11px] font-semibold shrink-0"
              >
                {{ item.narrativeAct }}
              </span>
              <span class="text-[12px] font-medium text-slate-500 truncate">
                {{ item.depthStage }}
              </span>
            </div>
            <span class="text-[11px] text-slate-400 tabular-nums shrink-0">
              火{{ item.tensionScore ?? 0 }} · 抽{{ item.abstractionScore ?? 0 }} · 僵{{ item.stagnationIndex }}
            </span>
          </div>

          <p class="text-[14px] md:text-[13px] leading-relaxed text-[#1D1D1F] mb-3 select-text break-words antialiased">
            {{ item.note }}
          </p>

          <div class="flex flex-wrap items-center gap-1.5">
            <span
              v-for="t in item.tags"
              :key="t"
              class="px-2.5 py-0.5 rounded-full bg-[#F5F5F7] text-slate-600 text-[11.5px] border border-black/[0.04] antialiased"
            >
              # {{ t }}
            </span>
            <span
              v-if="item.perturbationInjected"
              class="ml-auto text-[11px] font-medium text-rose-600"
            >
              ⚡ 自动扰动
            </span>
          </div>
        </div>
      </div>
    </div>

    <!-- Final Report Modal -->
    <AdaptiveModal
      :open="reportModalOpen"
      title="实验结案复盘简报"
      max-width-class="max-w-2xl"
      mobile-presentation="fullscreen"
      @close="reportModalOpen = false"
    >
      <template #header>
        <div class="px-5 sm:px-6 py-3.5 sm:py-4 border-b border-black/[0.06] flex flex-wrap items-center justify-between gap-2 shrink-0">
          <div class="flex items-center gap-2">
            <FileText class="w-4 h-4 text-[#007AFF]" />
            <h2 class="text-sm font-semibold text-[#1D1D1F]">实验结案复盘简报</h2>
          </div>
          <div class="flex flex-wrap items-center gap-1.5">
            <button
              type="button"
              @click="emit('distillImprints')"
              :disabled="distillingImprints || totalTurns === 0"
              class="flex items-center gap-1 px-3 py-1 rounded-full text-xs font-medium bg-indigo-50 hover:bg-indigo-100/80 text-indigo-700 transition disabled:opacity-40"
              title="调用观察员 C 提炼本局角色高浓度经历，写入灵魂角色库"
            >
              <RefreshCw v-if="distillingImprints" class="w-3.5 h-3.5 animate-spin" />
              <Sparkles v-else class="w-3.5 h-3.5" />
              <span>{{ distillingImprints ? "提炼中..." : "提炼本局经历写入灵魂库" }}</span>
            </button>
            <button
              type="button"
              @click="copyReport"
              :disabled="!(reportViewTab === 'comparison' ? (comparisonReport ?? '').trim() : finalReport.trim())"
              class="flex items-center gap-1 px-3 py-1 rounded-full text-xs bg-[#F5F5F7] hover:bg-[#EAEAEF] text-slate-700 disabled:opacity-40"
            >
              <Check v-if="copiedReport" class="w-3.5 h-3.5 text-emerald-600" />
              <Copy v-else class="w-3.5 h-3.5" />
              <span>{{ copiedReport ? "已复制" : "复制" }}</span>
            </button>
            <button
              type="button"
              @click="reportViewTab = 'final'; emit('generateReport')"
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
      </template>

      <div class="flex-1 overflow-y-auto p-5 sm:p-6 space-y-4">
        <!-- Worldline Branch Comparison Bar (when sibling/parent branch exists) -->
        <div
          v-if="relatedBranches && relatedBranches.length > 0"
          class="p-3 rounded-2xl bg-[#F5F5F7] border border-black/[0.06] flex flex-wrap items-center justify-between gap-2"
        >
          <div class="flex items-center gap-2 min-w-0 flex-wrap">
            <GitCompare class="w-4 h-4 text-violet-600 shrink-0" />
            <span class="text-[12px] font-medium text-[#1D1D1F]">同源平行世界线对照：</span>
            <select
              v-model="selectedCompareBranchId"
              class="h-7 rounded-lg border border-black/[0.08] bg-white px-2 text-[11.5px] text-[#1D1D1F] outline-none max-w-[200px]"
            >
              <option v-for="b in relatedBranches" :key="b.id" :value="b.id">
                {{ b.title }} ({{ b.totalTurns }}轮)
              </option>
            </select>
          </div>
          <div class="flex items-center gap-1.5 shrink-0">
            <div v-if="(comparisonReport ?? '').trim()" class="flex p-0.5 rounded-full bg-white border border-black/[0.06]">
              <button
                type="button"
                @click="reportViewTab = 'final'"
                class="px-2.5 py-0.5 rounded-full text-[11px] font-medium transition"
                :class="reportViewTab === 'final' ? 'bg-[#1D1D1F] text-white' : 'text-slate-600'"
              >
                结案简报
              </button>
              <button
                type="button"
                @click="reportViewTab = 'comparison'"
                class="px-2.5 py-0.5 rounded-full text-[11px] font-medium transition"
                :class="reportViewTab === 'comparison' ? 'bg-violet-600 text-white' : 'text-slate-600'"
              >
                分歧对照
              </button>
            </div>
            <button
              type="button"
              @click="reportViewTab = 'comparison'; emit('generateComparison', selectedCompareBranchId)"
              :disabled="generatingComparison || !selectedCompareBranchId"
              class="flex items-center gap-1 px-3 py-1 rounded-full text-[11.5px] font-medium bg-violet-600 hover:bg-violet-700 text-white transition disabled:opacity-45"
            >
              <RefreshCw v-if="generatingComparison" class="w-3 h-3 animate-spin" />
              <GitCompare v-else class="w-3 h-3" />
              <span>{{ generatingComparison ? "比对中..." : "生成世界线分歧对照报告" }}</span>
            </button>
          </div>
        </div>

        <template v-if="reportViewTab === 'comparison'">
          <div
            v-if="generatingComparison && !(comparisonReport ?? '').trim()"
            class="py-16 text-center text-xs text-slate-400 space-y-2"
          >
            <RefreshCw class="w-6 h-6 mx-auto animate-spin text-violet-600" />
            <div>观察员 C 正在对照两条平行宇宙的时间线分歧与蝴蝶效应...</div>
          </div>
          <div
            v-else-if="!(comparisonReport ?? '').trim()"
            class="py-12 text-center text-xs text-slate-400"
          >
            点击上方「生成世界线分歧对照报告」即可对比分叉点后的关键转折与结局差异。
          </div>
          <div
            v-else
            class="terrarium-prose"
            v-html="renderMarkdown(comparisonReport ?? '', false)"
          ></div>
        </template>

        <template v-else>
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
            class="space-y-4"
          >
            <div
              v-if="agents && agents.length > 0"
              class="p-3 rounded-2xl bg-[#FAFAFC] border border-black/[0.06] flex flex-wrap items-center justify-between gap-2"
            >
              <span class="text-[11px] font-medium text-slate-600">
                存入跨世界灵魂角色库（提取基础人设与博弈特质）：
              </span>
              <div class="flex flex-wrap items-center gap-1.5">
                <button
                  v-for="ag in agents"
                  :key="ag.id"
                  type="button"
                  @click="handleSaveCard(ag.id)"
                  class="flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-medium border transition-colors"
                  :class="
                    savedAgentIds[ag.id]
                      ? 'bg-emerald-50 text-emerald-700 border-emerald-200'
                      : 'bg-white hover:bg-indigo-50 text-slate-700 hover:text-indigo-700 border-black/[0.08]'
                  "
                >
                  <Check v-if="savedAgentIds[ag.id]" class="w-3 h-3" />
                  <BookmarkPlus v-else class="w-3 h-3" />
                  <span>{{ savedAgentIds[ag.id] ? `已存 · ${ag.label}` : `存入 ${ag.label}` }}</span>
                </button>
              </div>
            </div>
            <div
              class="terrarium-prose"
              v-html="renderMarkdown(finalReport, false)"
            ></div>
          </div>
        </template>
      </div>
    </AdaptiveModal>
  </aside>
</template>
