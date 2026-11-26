<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
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
    <!-- Top Eco-Dashboard -->
    <div class="px-4 pt-3 pb-3.5 bg-white space-y-3.5 shrink-0 border-b border-[#ECECF0]">
      <!-- Top Header -->
      <div class="flex items-center justify-between gap-2">
        <div class="min-w-0">
          <h3 class="text-[14.5px] font-semibold text-[#25262A] truncate leading-snug">局势概览</h3>
          <p class="flex items-center gap-1.5 text-[11.5px] text-[#858A95] truncate leading-snug mt-0.5">
            <span class="w-1.5 h-1.5 rounded-full bg-[#25A477] shrink-0"></span>
            <span>每 {{ observerInterval }} 轮自动分析</span>
          </p>
        </div>

        <div class="flex items-center gap-1.5 shrink-0">
          <button
            type="button"
            @click="openReportModal"
            :disabled="totalTurns === 0"
            class="w-8 h-8 flex items-center justify-center rounded-[8px] text-[#666B76] hover:text-[#25262A] hover:bg-[#EFEFF2] transition-colors disabled:opacity-40"
            title="生成或查看复盘报告"
          >
            <FileText class="w-4 h-4" />
          </button>

          <button
            type="button"
            @click="emit('triggerNow')"
            :disabled="observerRunning || totalTurns === 0 || !hasObserverKey"
            class="h-8 flex items-center gap-1.5 px-3 rounded-[9px] text-[12px] font-medium border border-[#E5E5EA] bg-white hover:bg-[#F5F5F7] text-[#25262A] transition-colors disabled:opacity-40"
            title="立即分析当前对话"
          >
            <RefreshCw
              class="w-3.5 h-3.5 text-[#666B76]"
              :class="{ 'animate-spin': observerRunning }"
            />
            <span>{{ observerRunning ? "分析中" : "分析" }}</span>
          </button>
        </div>
      </div>

      <!-- 3-Metric Summary Columns with thin progress bars (matching Screenshot 7) -->
      <div v-if="latestNote" class="grid grid-cols-3 gap-3 pt-1">
        <div class="space-y-1.5">
          <div class="text-[11.5px] text-[#858A95]">冲突度</div>
          <div class="flex items-baseline gap-0.5">
            <span class="text-[22px] font-semibold tabular-nums leading-none text-[#D15B5B]">{{ latestNote.tensionScore ?? 0 }}</span>
            <span class="text-[11px] text-[#969BA5]">/100</span>
          </div>
          <div class="h-1 rounded-full bg-[#EFEFF2] overflow-hidden">
            <div class="h-full rounded-full bg-[#D15B5B] transition-all duration-300" :style="{ width: `${Math.min(100, Math.max(0, latestNote.tensionScore ?? 0))}%` }"></div>
          </div>
        </div>

        <div class="space-y-1.5 pl-2 border-l border-[#ECECF0]">
          <div class="text-[11.5px] text-[#858A95]">抽象度</div>
          <div class="flex items-baseline gap-0.5">
            <span class="text-[22px] font-semibold tabular-nums leading-none text-[#7965CC]">{{ latestNote.abstractionScore ?? 0 }}</span>
            <span class="text-[11px] text-[#969BA5]">/100</span>
          </div>
          <div class="h-1 rounded-full bg-[#EFEFF2] overflow-hidden">
            <div class="h-full rounded-full bg-[#7965CC] transition-all duration-300" :style="{ width: `${Math.min(100, Math.max(0, latestNote.abstractionScore ?? 0))}%` }"></div>
          </div>
        </div>

        <div class="space-y-1.5 pl-2 border-l border-[#ECECF0]">
          <div class="text-[11.5px] text-[#858A95]">僵化度</div>
          <div class="flex items-baseline gap-0.5">
            <span class="text-[22px] font-semibold tabular-nums leading-none text-[#25A477]">{{ latestNote.stagnationIndex ?? 0 }}</span>
            <span class="text-[11px] text-[#969BA5]">/100</span>
          </div>
          <div class="h-1 rounded-full bg-[#EFEFF2] overflow-hidden">
            <div class="h-full rounded-full bg-[#25A477] transition-all duration-300" :style="{ width: `${Math.min(100, Math.max(0, latestNote.stagnationIndex ?? 0))}%` }"></div>
          </div>
        </div>
      </div>

      <!-- Collapsible Multi-Dimensional Evolution Curves (SVG) -->
      <div class="rounded-xl bg-black/[0.03] px-3 py-2">
        <div class="flex items-center justify-between gap-2">
          <button
            type="button"
            @click="chartExpanded = !chartExpanded"
            class="w-full flex items-center justify-between gap-2 text-[12px] font-medium text-slate-600 hover:text-[#1D1D1F] transition-colors"
          >
            <span>趋势曲线</span>
            <span class="text-[11.5px] font-medium text-[#007AFF]">
              {{ chartExpanded ? "收起" : latestNote ? "展开" : "暂无数据" }}
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
              冲突度
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
            暂无评分记录，等待触发分析
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
              <span class="font-semibold text-[#1D1D1F]">第 {{ hoveredTurn.triggerTurn }} 轮（跳转）</span>
              <span class="tabular-nums">
                冲突 {{ hoveredTurn.tensionScore }} · 抽象 {{ hoveredTurn.abstractionScore }} · 僵化 {{ hoveredTurn.stagnationIndex }}
              </span>
            </div>
          </div>
        </template>
      </div>

      <!-- Collapsible Relationship Network -->
      <div class="rounded-xl bg-black/[0.03] px-3 py-2">
        <button
          type="button"
          @click="networkExpanded = !networkExpanded"
          class="w-full flex items-center justify-between gap-2 text-[12px] font-medium text-slate-600 hover:text-[#1D1D1F] transition-colors"
        >
          <span class="flex items-center gap-1.5">
            <Network class="w-3.5 h-3.5 text-slate-500" />
            <span>角色关系</span>
          </span>
          <span class="text-[11.5px] font-medium text-[#007AFF]">
            {{
              blindGuessMode
                ? "已隐藏"
                : networkExpanded
                  ? "收起"
                  : latestNote?.relationshipMatrix?.length
                    ? `${latestNote.relationshipMatrix.length} 条关系`
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
            <span>盲猜模式已隐藏人物信任度与态度</span>
          </div>
          <div
            v-else-if="!latestNote?.relationshipMatrix || latestNote.relationshipMatrix.length === 0"
            class="py-2.5 text-center text-[11px] text-slate-400"
          >
            等待分析后生成角色关系
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
                title="三幕式当前幕次"
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
                话题破局
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

    <!-- Bottom Observer Notes Timeline (reverse chronological, soft top fade on scroll) -->
    <div class="relative md:flex-1 md:min-h-0 flex flex-col">
      <div
        class="pointer-events-none hidden md:block absolute inset-x-0 top-0 h-5 z-10 bg-gradient-to-b from-white via-white/80 to-transparent"
      ></div>
      <div class="md:flex-1 md:overflow-y-auto px-3.5 md:px-4 pt-3 pb-[calc(1.25rem+env(safe-area-inset-bottom))] md:pb-5 space-y-3 antialiased">
      <div class="flex items-center justify-between px-0.5">
        <span class="text-[12.5px] md:text-[12px] font-semibold text-slate-500 leading-relaxed">
          分析笔记 ({{ notes.length }})
        </span>
        <span class="text-[11px] text-slate-400 leading-relaxed">点击轮次跳转</span>
      </div>

      <div
        v-if="notes.length === 0"
        class="rounded-2xl border border-black/[0.03] bg-black/[0.025] p-6 text-center text-[13px] md:text-xs text-slate-400 leading-relaxed"
      >
        <Sparkles class="w-5 h-5 mx-auto mb-2 text-slate-300" />
        <span v-if="!hasObserverKey">
          请先在「设置 → 局势分析」中配置 API Key。
        </span>
        <span v-else>
          对话推进满 {{ observerInterval }} 轮后将自动生成评分与分析笔记。
        </span>
      </div>

      <div v-else class="relative pl-4 border-l border-black/[0.06] space-y-2.5 ml-1.5">
        <div
          v-for="(item, idx) in notes"
          :key="item.id"
          class="relative w-full text-left rounded-2xl bg-[#F7F7F9]/85 p-3.5 border border-transparent hover:border-[#007AFF]/35 transition-colors antialiased"
        >
          <!-- Timeline dot -->
          <span
            class="absolute -left-[21.5px] top-4 w-2.5 h-2.5 rounded-full ring-4 ring-white"
            :class="idx === 0 ? 'bg-[#007AFF]' : 'bg-slate-300'"
          ></span>

          <div class="flex flex-wrap items-center justify-between mb-2.5 gap-2">
            <div class="flex items-center gap-1.5 min-w-0">
              <button
                type="button"
                @click="emit('jumpToTurn', item.triggerTurn)"
                class="px-2.5 py-1 rounded-full text-[11.5px] leading-none font-semibold bg-[#F2F2F7] text-[#1D1D1F] hover:bg-[#007AFF]/15 hover:text-[#0062CC] transition-colors tabular-nums shrink-0 cursor-pointer"
                title="跳转到该轮对话"
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
              冲突{{ item.tensionScore ?? 0 }} · 抽象{{ item.abstractionScore ?? 0 }} · 僵化{{ item.stagnationIndex }}
            </span>
          </div>

          <p class="text-[14px] md:text-[13px] leading-relaxed text-[#1D1D1F] mb-3 select-text break-words antialiased">
            {{ item.note }}
          </p>

          <div class="flex flex-wrap items-center gap-1.5">
            <span
              v-for="t in item.tags"
              :key="t"
              class="px-2.5 py-0.5 rounded-full bg-black/[0.045] text-slate-600 text-[11.5px] antialiased"
            >
              # {{ t }}
            </span>
            <span
              v-if="item.perturbationInjected"
              class="ml-auto text-[11px] font-medium text-rose-600"
            >
              ⚡ 已注入破局
            </span>
          </div>
        </div>
      </div>
      </div>
    </div>

    <!-- Final Report Modal -->
    <AdaptiveModal
      :open="reportModalOpen"
      title="推演复盘报告"
      max-width-class="max-w-2xl"
      mobile-presentation="fullscreen"
      @close="reportModalOpen = false"
    >
      <template #header>
        <div class="px-5 sm:px-6 py-3.5 sm:py-4 border-b border-black/[0.06] flex flex-wrap items-center justify-between gap-2 shrink-0">
          <div class="flex items-center gap-2">
            <FileText class="w-4 h-4 text-[#007AFF]" />
            <h2 class="text-sm font-semibold text-[#1D1D1F]">推演复盘报告</h2>
          </div>
          <div class="flex flex-wrap items-center gap-1.5">
            <button
              type="button"
              @click="emit('distillImprints')"
              :disabled="distillingImprints || totalTurns === 0"
              class="flex items-center gap-1 px-3 py-1 rounded-full text-xs font-medium bg-indigo-50 hover:bg-indigo-100/80 text-indigo-700 transition disabled:opacity-40"
              title="提取本局角色经历并存入角色库"
            >
              <RefreshCw v-if="distillingImprints" class="w-3.5 h-3.5 animate-spin" />
              <Sparkles v-else class="w-3.5 h-3.5" />
              <span>{{ distillingImprints ? "总结中..." : "总结经历到角色库" }}</span>
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
            <span class="text-[12px] font-medium text-[#1D1D1F]">世界线对照：</span>
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
                复盘报告
              </button>
              <button
                type="button"
                @click="reportViewTab = 'comparison'"
                class="px-2.5 py-0.5 rounded-full text-[11px] font-medium transition"
                :class="reportViewTab === 'comparison' ? 'bg-violet-600 text-white' : 'text-slate-600'"
              >
                世界线对比
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
              <span>{{ generatingComparison ? "比对中..." : "世界线分歧对照" }}</span>
            </button>
          </div>
        </div>

        <template v-if="reportViewTab === 'comparison'">
          <div
            v-if="generatingComparison && !(comparisonReport ?? '').trim()"
            class="py-16 text-center text-xs text-slate-400 space-y-2"
          >
            <RefreshCw class="w-6 h-6 mx-auto animate-spin text-violet-600" />
            <div>正在比对两条世界线分歧...</div>
          </div>
          <div
            v-else-if="!(comparisonReport ?? '').trim()"
            class="py-12 text-center text-xs text-slate-400"
          >
            选择关联分支并生成世界线分歧对照报告。
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
            <div>正在分析对话并生成复盘报告...</div>
          </div>
          <div
            v-else-if="!finalReport.trim()"
            class="py-12 text-center text-xs text-slate-400"
          >
            点击「重新生成」汇总核心转折、精彩发言与角色表现。
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
                保存到角色库：
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
