<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from "vue";
import { useMediaQuery } from "@vueuse/core";
import { Sparkles, EyeOff, MessageCircle, Globe2, Archive, ChevronDown, ChevronRight, Lock } from "lucide-vue-next";
import type { ActivePrivateRoom, ExperimentConfig, SpeakerRole, TimelineMessage } from "../types";
import { markdownEnhancersVersion } from "../utils/markdown";
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
  experimentId?: string | null;
  messages: TimelineMessage[];
  streaming: StreamingDraft | null;
  config: ExperimentConfig | null;
  compactedSummary?: string;
  canBranch?: boolean;
  blindGuessMode?: boolean;
  roomMembers?: Record<string, string[]>;
  activePrivateRoom?: ActivePrivateRoom | null;
}>();

const emit = defineEmits<{
  fork: [payload: { turnNumber: number; messageId?: string | null }];
  editAndReplay: [payload: { messageId: string; turnNumber: number; content: string }];
  inspectPrompt: [messageId: string];
}>();

const summaryOpen = ref(false);

const containerRef = ref<HTMLElement | null>(null);
const highlightedTurn = ref<number | null>(null);
const stickyBottom = ref(true);

const INITIAL_VISIBLE_COUNT = 24;
const LOAD_MORE_STEP = 30;
const visibleCount = ref(INITIAL_VISIBLE_COUNT);
const expandingOlder = ref(false);

const firstMsgId = computed(() => props.messages[0]?.id ?? null);
watch([() => props.experimentId ?? null, firstMsgId], ([newExp, newFirst], [oldExp, oldFirst]) => {
  if (newExp !== oldExp || newFirst !== oldFirst) {
    visibleCount.value = INITIAL_VISIBLE_COUNT;
    stickyBottom.value = true;
  }
});

watch(
  () => props.messages.length,
  (newLen, oldLen) => {
    if (newLen === 0) {
      visibleCount.value = INITIAL_VISIBLE_COUNT;
      stickyBottom.value = true;
      return;
    }
    if (!stickyBottom.value && oldLen > 0 && newLen > oldLen) {
      visibleCount.value += newLen - oldLen;
    }
  }
);

const hiddenMessageCount = computed(() =>
  Math.max(0, props.messages.length - visibleCount.value)
);

const visibleMessages = computed(() => {
  if (props.messages.length <= visibleCount.value) return props.messages;
  return props.messages.slice(-visibleCount.value);
});

type TimelineRenderItem =
  | { kind: "msg"; id: string; msg: TimelineMessage }
  | {
      kind: "blindRoom";
      id: string;
      roomId: string;
      membersText: string;
      turnCount: number;
    };

const timelineRenderItems = computed<TimelineRenderItem[]>(() => {
  const list = visibleMessages.value;
  if (!props.blindGuessMode) {
    return list.map((msg) => ({ kind: "msg", id: msg.id, msg }));
  }

  const out: TimelineRenderItem[] = [];
  let i = 0;
  while (i < list.length) {
    const cur = list[i];
    if (cur.roomId && cur.roomId !== "main") {
      const rid = cur.roomId;
      const group: TimelineMessage[] = [];
      while (i < list.length && list[i].roomId === rid) {
        group.push(list[i]);
        i += 1;
      }
      const mappedMembers = props.roomMembers?.[rid];
      const memberIds =
        mappedMembers && mappedMembers.length >= 2
          ? mappedMembers
          : Array.from(
              new Set(
                group
                  .map((m) => m.agentId)
                  .filter((id): id is string => Boolean(id))
              )
            );
      const memberNames = memberIds.map((id) => `[${agentLabel(id)}]`);
      const membersText =
        memberNames.length >= 2
          ? `${memberNames[0]} 与 ${memberNames.slice(1).join("、")}`
          : memberNames[0] || "部分角色";
      const aiCount = Math.max(1, group.filter((m) => m.role === "Ai").length);
      out.push({
        kind: "blindRoom",
        id: `blind-${rid}-${group[0].id}`,
        roomId: rid,
        membersText,
        turnCount: aiCount,
      });
      continue;
    }
    out.push({ kind: "msg", id: cur.id, msg: cur });
    i += 1;
  }
  return out;
});

const streamingInBlindPrivateRoom = computed(() => {
  if (!props.blindGuessMode || !props.streaming || !props.activePrivateRoom) return false;
  return (
    props.activePrivateRoom.remainingTurns > 0 &&
    props.activePrivateRoom.memberIds.includes(props.streaming.agentId)
  );
});

const activeRoomMembersText = computed(() => {
  if (!props.activePrivateRoom) return "";
  const names = props.activePrivateRoom.memberIds.map((id) => `[${agentLabel(id)}]`);
  return names.length >= 2 ? `${names[0]} 与 ${names.slice(1).join("、")}` : names[0] || "";
});

async function loadOlderMessages(count = LOAD_MORE_STEP) {
  if (expandingOlder.value || hiddenMessageCount.value === 0) return;
  const el = containerRef.value;
  expandingOlder.value = true;
  const prevScrollHeight = el?.scrollHeight ?? 0;
  const prevScrollTop = el?.scrollTop ?? 0;
  visibleCount.value += count;
  await nextTick();
  if (el) {
    const addedHeight = el.scrollHeight - prevScrollHeight;
    if (addedHeight > 0) {
      el.scrollTop = prevScrollTop + addedHeight;
    }
  }
  expandingOlder.value = false;
}

const agentPalettes = [
  {
    bubble: "bg-[#EEF2F7] border-[#DCE3EC]/80 shadow-[0_2px_10px_rgba(15,23,42,0.03)]",
    dot: "bg-slate-600",
    label: "text-slate-900",
    avatarBg: "bg-slate-900/10 text-slate-800",
  },
  {
    bubble: "bg-[#FAF5EE] border-[#EDE3D4]/90 shadow-[0_2px_10px_rgba(120,85,40,0.03)]",
    dot: "bg-amber-600",
    label: "text-stone-900",
    avatarBg: "bg-amber-500/15 text-amber-800",
  },
  {
    bubble: "bg-[#EEF6F1] border-[#D5E8DE]/90 shadow-[0_2px_10px_rgba(16,120,80,0.03)]",
    dot: "bg-emerald-600",
    label: "text-emerald-950",
    avatarBg: "bg-emerald-500/15 text-emerald-800",
  },
  {
    bubble: "bg-[#F3F0FA] border-[#E1DAF2]/90 shadow-[0_2px_10px_rgba(90,60,160,0.03)]",
    dot: "bg-violet-600",
    label: "text-violet-950",
    avatarBg: "bg-violet-500/15 text-violet-800",
  },
  {
    bubble: "bg-[#ECF5FA] border-[#D3E7F3]/90 shadow-[0_2px_10px_rgba(14,116,144,0.03)]",
    dot: "bg-sky-600",
    label: "text-sky-950",
    avatarBg: "bg-sky-500/15 text-sky-800",
  },
  {
    bubble: "bg-[#FAF0F3] border-[#F0D8E0]/90 shadow-[0_2px_10px_rgba(160,40,80,0.03)]",
    dot: "bg-rose-600",
    label: "text-rose-950",
    avatarBg: "bg-rose-500/15 text-rose-800",
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

function isAgentLeft(agentId: string | null): boolean {
  if (!agentId) return false;
  return Boolean(agentsList.value.find((a) => a.id === agentId)?.hasLeft);
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
  if (el.scrollTop < 120 && hiddenMessageCount.value > 0 && !expandingOlder.value) {
    loadOlderMessages();
  }
}

const isTouchOrNarrow = useMediaQuery("(hover: none), (max-width: 767px)");

function scrollToBottom(smooth = true) {
  const el = containerRef.value;
  if (!el) return;
  el.scrollTo({
    top: el.scrollHeight,
    behavior: smooth ? "smooth" : "auto",
  });
}

let scrollRafId: number | null = null;
function scheduleScrollToBottom() {
  if (scrollRafId !== null) return;
  scrollRafId = window.requestAnimationFrame(() => {
    scrollRafId = null;
    if (stickyBottom.value) {
      scrollToBottom(false);
    }
  });
}

onUnmounted(() => {
  if (scrollRafId !== null) {
    window.cancelAnimationFrame(scrollRafId);
    scrollRafId = null;
  }
});

watch(
  () => [
    props.messages.length,
    props.streaming?.content.length ?? 0,
    props.streaming?.reasoningContent.length ?? 0,
    markdownEnhancersVersion.value,
  ],
  () => {
    if (!stickyBottom.value) return;
    scheduleScrollToBottom();
  }
);

async function scrollToTurn(turnNumber: number) {
  const el = containerRef.value;
  if (!el) return;
  const idx = props.messages.findIndex((m) => m.turnNumber === turnNumber);
  if (idx >= 0) {
    const neededFromEnd = props.messages.length - idx;
    if (neededFromEnd > visibleCount.value) {
      visibleCount.value = neededFromEnd;
      await nextTick();
    }
  }
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

function isPassNotice(msg: TimelineMessage): boolean {
  return msg.role === "GodSystem" && msg.content.endsWith("看了一眼消息，没接话");
}

function godMeta(role: SpeakerRole, agentId: string | null) {
  switch (role) {
    case "GodSystem":
      return {
        label: "场景事件",
        icon: Globe2,
        badgeClass: "terrarium-stay-dark bg-slate-900/90 text-white border-slate-700/50",
        lineClass: "via-slate-300",
      };
    case "GodWhisper":
      return {
        label: `定向提示 → ${agentLabel(agentId)}`,
        icon: EyeOff,
        badgeClass: "bg-indigo-50 text-indigo-700 border-indigo-200/80",
        lineClass: "via-indigo-200",
      };
    case "GodPublic":
      return {
        label: "外部插话",
        icon: MessageCircle,
        badgeClass: "bg-emerald-50 text-emerald-700 border-emerald-200/80",
        lineClass: "via-emerald-200",
      };
    default:
      return {
        label: "主持人干预",
        icon: Sparkles,
        badgeClass: "bg-slate-100 text-slate-700 border-slate-200",
        lineClass: "via-slate-200",
      };
  }
}
</script>

<template>
  <div class="relative flex-1 min-h-0 flex flex-col">
    <div
      ref="containerRef"
      @scroll="handleScroll"
      class="flex-1 overflow-y-auto md:mr-1 px-3 sm:px-5 md:px-8 pt-5 sm:pt-6 pb-6"
    >
      <div class="max-w-[740px] mx-auto w-full space-y-4 md:space-y-5">
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
        <span class="font-semibold text-[#1D1D1F] truncate">历史对话摘要</span>
        <span class="hidden sm:inline text-[11px] text-slate-400 shrink-0">早期对话已自动折叠</span>
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
        沙盒已就绪
      </h2>
      <p class="text-xs text-slate-500 leading-relaxed">
        当前包含
        <span class="font-medium text-slate-700">{{ agentsList.length }}</span>
        位角色（<span class="font-medium text-slate-700">{{ agentsList.map((a) => a.label).join(" · ") }}</span>），点击右上角「开始」或「下一轮」开启对话。
      </p>
      <div
        v-if="config?.initialTopic"
        class="mt-4 px-4 py-2.5 rounded-2xl bg-white/80 border border-black/[0.06] text-xs text-slate-600 shadow-xs"
      >
        <span class="text-slate-400 mr-1.5">开场设定:</span>
        {{ config.initialTopic }}
      </div>
    </div>

    <!-- Load Older Messages Banner -->
    <div v-if="hiddenMessageCount > 0" class="flex justify-center pt-1">
      <button
        type="button"
        @click="loadOlderMessages(LOAD_MORE_STEP)"
        class="px-3.5 py-1.5 rounded-full text-[11px] font-medium text-slate-600 bg-white/90 hover:bg-white border border-black/[0.08] shadow-xs transition"
      >
        加载更早的 {{ Math.min(LOAD_MORE_STEP, hiddenMessageCount) }} 条对话（还有 {{ hiddenMessageCount }} 条）
      </button>
    </div>

    <!-- Timeline Messages -->
    <template v-for="item in timelineRenderItems" :key="item.id">
      <div
        v-if="item.kind === 'blindRoom'"
        class="py-1.5 flex justify-center select-none"
      >
        <div class="inline-flex items-center gap-2 px-4 py-2 rounded-2xl bg-violet-50/80 border border-violet-200/80 text-[12px] text-violet-800 shadow-[0_1px_6px_rgba(139,92,246,0.06)]">
          <Lock class="w-3.5 h-3.5 text-violet-600 shrink-0" />
          <span>
            {{ item.membersText }} 私下对话了 {{ item.turnCount }} 轮（盲猜模式已隐藏）
          </span>
        </div>
      </div>

      <MessageBubble
        v-else-if="item.msg.role === 'Ai'"
        :message-id="item.msg.id"
        :turn-number="item.msg.turnNumber"
        :agent-label="agentLabel(item.msg.agentId)"
        :model-name="item.msg.modelName"
        :room-id="item.msg.roomId"
        :duration-ms="item.msg.durationMs"
        :reasoning-content="item.msg.reasoningContent ?? ''"
        :content="item.msg.content"
        :micro-cue="item.msg.microCue"
        :agent-has-left="isAgentLeft(item.msg.agentId)"
        :align-right="agentStyle(item.msg.agentId).alignRight"
        :palette="agentStyle(item.msg.agentId).palette"
        :highlighted="highlightedTurn === item.msg.turnNumber"
        :can-branch="canBranch"
        :blind-guess-mode="blindGuessMode"
        :is-touch-or-narrow="isTouchOrNarrow"
        @fork="emit('fork', $event)"
        @edit-and-replay="emit('editAndReplay', $event)"
        @inspect-prompt="emit('inspectPrompt', $event)"
      />

      <!-- Lightweight [PASS] Silence Notice -->
      <div v-else-if="isPassNotice(item.msg)" class="py-0.5 flex justify-center select-none">
        <span class="px-3 py-1 rounded-full bg-slate-100/85 border border-black/[0.05] text-[11px] text-slate-500">
          {{ item.msg.content }}
        </span>
      </div>

      <!-- God Intervention Divider -->
      <div v-else class="py-2 flex items-center gap-3">
        <div
          class="flex-1 h-px bg-gradient-to-r from-transparent to-transparent"
          :class="godMeta(item.msg.role, item.msg.agentId).lineClass"
        ></div>
        <div
          class="max-w-[86%] sm:max-w-[76%] px-4 py-2.5 rounded-2xl border shadow-[0_2px_12px_rgba(0,0,0,0.04)] text-center"
          :class="godMeta(item.msg.role, item.msg.agentId).badgeClass"
        >
          <div class="flex items-center justify-center gap-1.5 text-[11px] font-semibold opacity-85 mb-0.5 select-none">
            <component :is="godMeta(item.msg.role, item.msg.agentId).icon" class="w-3.5 h-3.5" />
            <span>{{ godMeta(item.msg.role, item.msg.agentId).label }}</span>
          </div>
          <div
            v-if="blindGuessMode && item.msg.role === 'GodWhisper'"
            class="text-[12px] italic opacity-75 select-none"
          >
            盲猜模式已隐藏定向提示
          </div>
          <div
            v-else
            class="text-[13px] md:text-xs leading-relaxed font-normal whitespace-pre-wrap break-words select-text"
            :class="item.msg.role === 'GodSystem' ? 'selection:bg-[#0A84FF]/45 selection:text-white' : ''"
          >
            {{ item.msg.content }}
          </div>
        </div>
        <div
          class="flex-1 h-px bg-gradient-to-r from-transparent to-transparent"
          :class="godMeta(item.msg.role, item.msg.agentId).lineClass"
        ></div>
      </div>
    </template>

    <!-- Active Streaming Bubble -->
    <div
      v-if="streaming && streamingInBlindPrivateRoom"
      class="py-1.5 flex justify-center select-none"
    >
      <div class="inline-flex items-center gap-2 px-4 py-2 rounded-2xl bg-violet-50/80 border border-violet-200/80 text-[12px] text-violet-800">
        <Lock class="w-3.5 h-3.5 text-violet-600 animate-pulse shrink-0" />
        <span>{{ activeRoomMembersText }} 正在私下对话（盲猜模式已隐藏）</span>
      </div>
    </div>
    <MessageBubble
      v-else-if="streaming"
      :turn-number="streaming.turnNumber"
      :agent-label="agentLabel(streaming.agentId, streaming.agentLabel)"
      :model-name="streaming.modelName"
      :reasoning-content="streaming.reasoningContent"
      :reasoning-elapsed-ms="streaming.reasoningElapsedMs"
      :content="streaming.content"
      :is-streaming="true"
      :align-right="agentStyle(streaming.agentId).alignRight"
      :palette="agentStyle(streaming.agentId).palette"
      :blind-guess-mode="blindGuessMode"
      :is-touch-or-narrow="isTouchOrNarrow"
    />
      </div>
    </div>
  </div>
</template>
