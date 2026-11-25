<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { Sparkles, EyeOff, MessageCircle, Globe2, Archive, ChevronDown, ChevronRight } from "lucide-vue-next";
import type { ExperimentConfig, SpeakerRole, TimelineMessage } from "../types";
import MessageBubble from "./MessageBubble.vue";

export interface StreamingDraft {
  messageId: string;
  agentId: string;
  agentLabel: string;
  turnNumber: number;
  modelName: string;
  reasoningContent: string;
  reasoningStartedAt: number | null;
  reasoningElapsedMs: number;
  content: string;
}

const props = defineProps<{
  messages: TimelineMessage[];
  streaming: StreamingDraft | null;
  config: ExperimentConfig | null;
  compactedSummary?: string;
  canBranch?: boolean;
}>();

const emit = defineEmits<{
  fork: [turnNumber: number];
  editAndReplay: [payload: { messageId: string; turnNumber: number; content: string }];
  inspectPrompt: [messageId: string];
}>();

const summaryOpen = ref(false);

const containerRef = ref<HTMLElement | null>(null);
const highlightedTurn = ref<number | null>(null);
const stickyBottom = ref(true);

const agentPalettes = [
  {
    bubble: "bg-[#EEF2F7] border-[#DCE3EC]/80 shadow-[0_2px_10px_rgba(15,23,42,0.03)]",
    dot: "bg-slate-600",
    label: "text-slate-800",
  },
  {
    bubble: "bg-[#FAF5EE] border-[#EDE3D4]/90 shadow-[0_2px_10px_rgba(120,85,40,0.03)]",
    dot: "bg-amber-600",
    label: "text-stone-800",
  },
  {
    bubble: "bg-[#EEF6F1] border-[#D5E8DE]/90 shadow-[0_2px_10px_rgba(16,120,80,0.03)]",
    dot: "bg-emerald-600",
    label: "text-emerald-950",
  },
  {
    bubble: "bg-[#F3F0FA] border-[#E1DAF2]/90 shadow-[0_2px_10px_rgba(90,60,160,0.03)]",
    dot: "bg-violet-600",
    label: "text-violet-950",
  },
  {
    bubble: "bg-[#ECF5FA] border-[#D3E7F3]/90 shadow-[0_2px_10px_rgba(14,116,144,0.03)]",
    dot: "bg-sky-600",
    label: "text-sky-950",
  },
  {
    bubble: "bg-[#FAF0F3] border-[#F0D8E0]/90 shadow-[0_2px_10px_rgba(160,40,80,0.03)]",
    dot: "bg-rose-600",
    label: "text-rose-950",
  },
];

const agentsList = computed(() => props.config?.agents ?? []);

function agentIndex(agentId: string | null): number {
  if (!agentId) return 0;
  const idx = agentsList.value.findIndex((a) => a.id === agentId);
  return idx >= 0 ? idx : 0;
}

function agentLabel(agentId: string | null, fallback?: string): string {
  if (!agentId) return fallback ?? "AI";
  const found = agentsList.value.find((a) => a.id === agentId);
  return found?.label ?? fallback ?? agentId;
}

function agentStyle(agentId: string | null) {
  const idx = agentIndex(agentId);
  return {
    alignRight: idx % 2 === 1,
    palette: agentPalettes[idx % agentPalettes.length],
  };
}

function handleScroll() {
  const el = containerRef.value;
  if (!el) return;
  const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
  stickyBottom.value = distanceFromBottom < 90;
}

function scrollToBottom(smooth = true) {
  const el = containerRef.value;
  if (!el) return;
  el.scrollTo({
    top: el.scrollHeight,
    behavior: smooth ? "smooth" : "auto",
  });
}

watch(
  () => [
    props.messages.length,
    props.streaming?.content.length ?? 0,
    props.streaming?.reasoningContent.length ?? 0,
  ],
  async () => {
    if (!stickyBottom.value) return;
    await nextTick();
    scrollToBottom(false);
  }
);

function scrollToTurn(turnNumber: number) {
  const el = containerRef.value;
  if (!el) return;
  const target = el.querySelector(`[data-turn="${turnNumber}"]`) as HTMLElement | null;
  if (target) {
    stickyBottom.value = false;
    target.scrollIntoView({ behavior: "smooth", block: "center" });
    highlightedTurn.value = turnNumber;
    setTimeout(() => {
      if (highlightedTurn.value === turnNumber) {
        highlightedTurn.value = null;
      }
    }, 1800);
  }
}

defineExpose({ scrollToTurn, scrollToBottom });

function godMeta(role: SpeakerRole, agentId: string | null) {
  switch (role) {
    case "GodSystem":
      return {
        label: "环境旁白 · 全知事件",
        icon: Globe2,
        badgeClass: "bg-slate-900/90 text-white border-slate-700/50",
        lineClass: "via-slate-300",
      };
    case "GodWhisper":
      return {
        label: `心灵低语 → ${agentLabel(agentId)}`,
        icon: EyeOff,
        badgeClass: "bg-indigo-50 text-indigo-700 border-indigo-200/80",
        lineClass: "via-indigo-200",
      };
    case "GodPublic":
      return {
        label: "路人降临 · 直接发言",
        icon: MessageCircle,
        badgeClass: "bg-emerald-50 text-emerald-700 border-emerald-200/80",
        lineClass: "via-emerald-200",
      };
    default:
      return {
        label: "造物主干预",
        icon: Sparkles,
        badgeClass: "bg-slate-100 text-slate-700 border-slate-200",
        lineClass: "via-slate-200",
      };
  }
}
</script>

<template>
  <div
    ref="containerRef"
    @scroll="handleScroll"
    class="flex-1 overflow-y-auto px-3 sm:px-6 md:px-10 pt-4 sm:pt-6 pb-4 space-y-4 sm:space-y-5"
  >
    <!-- Compacted Memory Summary Banner -->
    <div
      v-if="compactedSummary && compactedSummary.trim().length > 0"
      class="rounded-2xl bg-slate-100/80 border border-black/[0.06] overflow-hidden"
    >
      <button
        type="button"
        @click="summaryOpen = !summaryOpen"
        class="w-full px-4 py-2.5 flex items-center gap-2 text-xs text-slate-600 hover:bg-slate-200/50 transition text-left min-w-0"
      >
        <Archive class="w-3.5 h-3.5 text-[#007AFF] shrink-0" />
        <span class="font-semibold text-[#1D1D1F] truncate">历史纪要与已确认事实</span>
        <span class="hidden sm:inline text-[11px] text-slate-400 shrink-0">早期对话已自动无损折叠</span>
        <component
          :is="summaryOpen ? ChevronDown : ChevronRight"
          class="w-3.5 h-3.5 text-slate-400 ml-auto shrink-0"
        />
      </button>
      <div
        v-if="summaryOpen"
        class="px-4 pb-3 pt-1 text-xs leading-relaxed text-slate-600 whitespace-pre-wrap border-t border-black/[0.05]"
      >
        {{ compactedSummary }}
      </div>
    </div>

    <!-- Empty State -->
    <div
      v-if="messages.length === 0 && !streaming"
      class="h-full min-h-[360px] flex flex-col items-center justify-center text-center max-w-md mx-auto select-none"
    >
      <div
        class="w-14 h-14 rounded-2xl bg-white shadow-[0_8px_24px_rgba(0,0,0,0.06)] border border-black/[0.06] flex items-center justify-center mb-4"
      >
        <Sparkles class="w-6 h-6 text-[#007AFF]" />
      </div>
      <h2 class="text-base font-semibold text-[#1D1D1F] mb-1.5">
        观察箱已就绪
      </h2>
      <p class="text-xs text-slate-500 leading-relaxed">
        当前生态包含
        <span class="font-medium text-slate-700">{{ agentsList.length }}</span>
        位 AI 参与者（<span class="font-medium text-slate-700">{{ agentsList.map((a) => a.label).join(" · ") }}</span>）。
        点击右上角「开始演化」或「单步推进」即可开启自主对话。
      </p>
      <div
        v-if="config?.initialTopic"
        class="mt-4 px-4 py-2.5 rounded-2xl bg-white/80 border border-black/[0.06] text-xs text-slate-600 shadow-xs"
      >
        <span class="text-slate-400 mr-1.5">开场引子:</span>
        {{ config.initialTopic }}
      </div>
    </div>

    <!-- Timeline Messages -->
    <template v-for="msg in messages" :key="msg.id">
      <MessageBubble
        v-if="msg.role === 'Ai'"
        :message-id="msg.id"
        :turn-number="msg.turnNumber"
        :agent-label="agentLabel(msg.agentId)"
        :model-name="msg.modelName"
        :room-id="msg.roomId"
        :duration-ms="msg.durationMs"
        :reasoning-content="msg.reasoningContent ?? ''"
        :content="msg.content"
        :align-right="agentStyle(msg.agentId).alignRight"
        :palette="agentStyle(msg.agentId).palette"
        :highlighted="highlightedTurn === msg.turnNumber"
        :can-branch="canBranch"
        @fork="emit('fork', $event)"
        @edit-and-replay="emit('editAndReplay', $event)"
        @inspect-prompt="emit('inspectPrompt', $event)"
      />

      <!-- God Intervention Divider -->
      <div v-else class="py-2 flex items-center gap-3">
        <div
          class="flex-1 h-px bg-gradient-to-r from-transparent to-transparent"
          :class="godMeta(msg.role, msg.agentId).lineClass"
        ></div>
        <div
          class="max-w-[72%] px-4 py-2 rounded-2xl border shadow-[0_2px_12px_rgba(0,0,0,0.04)] backdrop-blur-md text-center"
          :class="godMeta(msg.role, msg.agentId).badgeClass"
        >
          <div class="flex items-center justify-center gap-1.5 text-[10.5px] font-semibold opacity-85 mb-0.5">
            <component :is="godMeta(msg.role, msg.agentId).icon" class="w-3 h-3" />
            <span>{{ godMeta(msg.role, msg.agentId).label }}</span>
          </div>
          <div class="text-xs leading-relaxed font-normal whitespace-pre-wrap break-words">
            {{ msg.content }}
          </div>
        </div>
        <div
          class="flex-1 h-px bg-gradient-to-r from-transparent to-transparent"
          :class="godMeta(msg.role, msg.agentId).lineClass"
        ></div>
      </div>
    </template>

    <!-- Active Streaming Bubble -->
    <MessageBubble
      v-if="streaming"
      :turn-number="streaming.turnNumber"
      :agent-label="agentLabel(streaming.agentId, streaming.agentLabel)"
      :model-name="streaming.modelName"
      :reasoning-content="streaming.reasoningContent"
      :reasoning-elapsed-ms="streaming.reasoningElapsedMs"
      :content="streaming.content"
      :is-streaming="true"
      :align-right="agentStyle(streaming.agentId).alignRight"
      :palette="agentStyle(streaming.agentId).palette"
    />
  </div>
</template>
