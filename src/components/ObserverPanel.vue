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
} from "lucide-vue-next";
import type { NodeConfig, ObserverNote } from "../types";
import { renderMarkdown } from "../utils/markdown";
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
}>();

const emit = defineEmits<{
  jumpToTurn: [turnNumber: number];
  triggerNow: [];
  generateReport: [];
  saveCharacterCard: [agentId: string];
}>();

const reportModalOpen = ref(false);
const copiedReport = ref(false);
const savedAgentIds = ref<Record<string, boolean>>({});

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

function buildPolyline(getter: (p: ChartPoint) => number): string {
  return chartPoints.value.map((p) => `${p.x.toFixed(1)},${getter(p).toFixed(1)}`).join(" ");
}

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

function handleSaveCard(agentId: string) {
  emit("saveCharacterCard", agentId);
  savedAgentIds.value[agentId] = true;
}

defineExpose({ openReportModal });
</script>

<template>
  <aside
    class="h-full flex flex-col bg-[#FAFAFC] overflow-y-auto md:overflow-hidden antialiased"
  >
    <!-- Top Eco-Dashboard (solid background, no backdrop-blur to prevent WKWebView text ghosting) -->
    <div class="p-3.5 border-b border-black/[0.06] bg-white space-y-2.5 shrink-0">
      <div class="flex items-center justify-between gap-1.5">
        <div class="flex items-center gap-2 min-w-0">
          <div class="w-7 h-7 rounded-xl bg-slate-900 text-white flex items-center justify-center shrink-0">
            <Telescope class="w-3.5 h-3.5" />
          </div>
          <div class="min-w-0">
            <h3 class="text-xs font-semibold text-[#1D1D1F] truncate leading-snug">观察员视角</h3>
            <p class="text-[10.5px] text-slate-400 truncate leading-snug">
              每 {{ observerInterval }} 轮异步多维诊断
            </p>
          </div>
        </div>

        <div class="flex items-center gap-1 shrink-0">
          <button
            type="button"
            @click="openReportModal"
            :disabled="totalTurns === 0"
            class="flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-medium bg-[#F5F5F7] hover:bg-[#EAEAEF] text-slate-700 border border-black/[0.05] transition-colors disabled:opacity-45"
            title="生成或查看实验结案复盘简报"
          >
            <FileText class="w-3 h-3" />
            <span>复盘简报</span>
          </button>

          <button
            type="button"
            @click="emit('triggerNow')"
            :disabled="observerRunning || totalTurns === 0 || !hasObserverKey"
            class="flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-medium bg-[#1D1D1F] hover:bg-black text-white transition-colors disabled:opacity-45"
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

      <!-- Collapsible Multi-Dimensional Evolution Curves (SVG) -->
      <div class="rounded-2xl bg-[#FAFAFC] border border-black/[0.06] p-2.5">
        <div class="flex items-center justify-between gap-2">
          <button
            type="button"
            @click="chartExpanded = !chartExpanded"
            class="flex items-center gap-1.5 text-[11px] font-semibold text-slate-600 hover:text-[#1D1D1F] transition-colors"
          >
            <span>认知演化曲线</span>
            <span class="text-[10px] font-normal text-[#007AFF]">
              {{ chartExpanded ? "收起" : "展开" }}
            </span>
          </button>

          <div v-if="latestNote" class="flex items-center gap-2 text-[10.5px] tabular-nums text-slate-500">
            <span class="text-rose-600 font-medium">火 {{ latestNote.tensionScore ?? 0 }}</span>
            <span class="text-[#007AFF] font-medium">抽 {{ latestNote.abstractionScore ?? 0 }}</span>
            <span class="font-medium" :class="stagnationTextColor">僵 {{ latestNote.stagnationIndex ?? 0 }}</span>
          </div>
          <span v-else class="text-[10.5px] text-slate-400">暂无轨迹</span>
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
    <div class="md:flex-1 md:overflow-y-auto p-4 space-y-3 antialiased">
      <div class="flex items-center justify-between px-0.5">
        <span class="text-[11px] font-medium text-slate-400 leading-relaxed">
          观察手记时间轴 ({{ notes.length }})
        </span>
        <span class="text-[10px] text-slate-400 leading-relaxed">点击节点或卡片定位轮次</span>
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

      <div
        v-for="item in notes"
        :key="item.id"
        class="w-full text-left rounded-2xl bg-white p-3.5 border border-black/[0.06] shadow-[0_2px_10px_rgba(0,0,0,0.03)] hover:border-[#007AFF]/35 transition-colors antialiased"
      >
        <div class="flex items-center justify-between mb-2 gap-2">
          <button
            type="button"
            @click="emit('jumpToTurn', item.triggerTurn)"
            class="px-2 py-0.5 rounded-full text-[10.5px] leading-relaxed font-semibold bg-slate-100 text-slate-700 hover:bg-[#007AFF]/15 hover:text-[#0062CC] transition-colors tabular-nums shrink-0 cursor-pointer"
            title="点击定位到该轮对话"
          >
            第 {{ item.triggerTurn }} 轮 ↗
          </button>
          <div class="flex items-center gap-1 min-w-0 flex-1 justify-center">
            <span
              v-if="item.narrativeAct"
              class="px-1.5 py-0.5 rounded bg-violet-50 text-violet-700 text-[10.5px] font-semibold leading-normal shrink-0"
            >
              {{ item.narrativeAct }}
            </span>
            <span class="text-[10.5px] font-medium text-slate-500 truncate leading-relaxed">
              {{ item.depthStage }}
            </span>
          </div>
          <span class="text-[10px] text-slate-400 tabular-nums shrink-0 leading-relaxed">
            火{{ item.tensionScore ?? 0 }} / 抽{{ item.abstractionScore ?? 0 }} / 僵{{ item.stagnationIndex }}
          </span>
        </div>

        <p class="text-[12.5px] leading-relaxed text-[#1D1D1F] mb-2.5 select-text break-words antialiased">
          {{ item.note }}
        </p>

        <div class="flex flex-wrap items-center gap-1.5">
          <span
            v-for="t in item.tags"
            :key="t"
            class="px-2 py-0.5 rounded-md bg-slate-50 text-slate-500 text-[10.5px] leading-relaxed border border-slate-200/60 antialiased"
          >
            {{ t }}
          </span>
          <span
            v-if="item.perturbationInjected"
            class="ml-auto text-[10px] font-medium text-rose-600 leading-relaxed"
          >
            ⚡ 自动扰动
          </span>
        </div>
      </div>
    </div>

    <!-- Final Report Modal -->
    <AdaptiveModal
      :open="reportModalOpen"
      title="实验结案复盘简报"
      max-width-class="max-w-2xl"
      @close="reportModalOpen = false"
    >
      <template #header>
        <div class="px-5 sm:px-6 py-3.5 sm:py-4 border-b border-black/[0.06] flex items-center justify-between gap-2 shrink-0">
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
      </template>

      <div class="flex-1 overflow-y-auto p-5 sm:p-6">
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
      </div>
    </AdaptiveModal>
  </aside>
</template>
