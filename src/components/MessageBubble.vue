<script setup lang="ts">
import { computed, ref } from "vue";
import {
  ChevronDown,
  ChevronRight,
  Brain,
  GitBranch,
  Pencil,
  Check,
  X,
} from "lucide-vue-next";
import { renderMarkdown } from "../utils/markdown";

export interface BubblePalette {
  bubble: string;
  dot: string;
  label: string;
}

const props = defineProps<{
  messageId?: string;
  turnNumber: number;
  agentLabel: string;
  modelName: string;
  durationMs?: number;
  reasoningContent: string;
  content: string;
  isStreaming?: boolean;
  reasoningElapsedMs?: number;
  alignRight: boolean;
  palette: BubblePalette;
  highlighted?: boolean;
  canBranch?: boolean;
}>();

const emit = defineEmits<{
  fork: [turnNumber: number];
  editAndReplay: [payload: { messageId: string; turnNumber: number; content: string }];
}>();

const reasoningOpen = ref(false);
const editing = ref(false);
const draftContent = ref("");

const hasReasoning = computed(() => props.reasoningContent.trim().length > 0);
const isActivelyThinking = computed(
  () => Boolean(props.isStreaming) && hasReasoning.value && props.content.length === 0
);

const thinkingSummary = computed(() => {
  if (isActivelyThinking.value) {
    const secs = ((props.reasoningElapsedMs ?? 0) / 1000).toFixed(1);
    return `正在思考中 (${secs}s)... 点击查看内心戏`;
  }
  if (props.reasoningElapsedMs && props.reasoningElapsedMs > 0) {
    return `思考了 ${(props.reasoningElapsedMs / 1000).toFixed(1)}s · 点击查看内心戏`;
  }
  const chars = props.reasoningContent.trim().length;
  return `幕后心声（${chars} 字）· 点击查看内心戏`;
});

function startEdit() {
  draftContent.value = props.content;
  editing.value = true;
}

function cancelEdit() {
  editing.value = false;
  draftContent.value = "";
}

function submitEditReplay() {
  if (!props.messageId) return;
  const trimmed = draftContent.value.trim();
  if (!trimmed) return;
  editing.value = false;
  emit("editAndReplay", {
    messageId: props.messageId,
    turnNumber: props.turnNumber,
    content: trimmed,
  });
}
</script>

<template>
  <div
    :data-turn="turnNumber"
    class="flex min-w-0"
    :class="alignRight ? 'justify-end' : 'justify-start'"
  >
    <div
      class="group relative min-w-0 max-w-[92%] sm:max-w-[82%] md:max-w-[78%] rounded-2xl sm:rounded-3xl px-4 py-3 sm:px-5 sm:py-4 border transition"
      :class="[
        palette.bubble,
        alignRight ? 'rounded-tr-lg' : 'rounded-tl-lg',
        { 'animate-turn-highlight': highlighted },
        isStreaming ? 'shadow-[0_4px_18px_rgba(0,0,0,0.04)]' : '',
      ]"
    >
      <!-- Desktop Hover Branch & Replay Actions -->
      <div
        v-if="canBranch && !isStreaming && !editing"
        class="hidden md:flex opacity-0 group-hover:opacity-100 focus-within:opacity-100 absolute -top-3 right-3 items-center gap-1 px-1.5 py-0.5 rounded-full bg-white/95 backdrop-blur-md border border-black/[0.08] shadow-sm transition-opacity z-10"
      >
        <button
          type="button"
          @click="emit('fork', turnNumber)"
          class="flex items-center gap-1 px-2 py-0.5 rounded-full text-[10.5px] font-medium text-slate-600 hover:text-[#007AFF] hover:bg-blue-50/80 transition"
          title="从本轮分叉创建平行宇宙分支"
        >
          <GitBranch class="w-3 h-3" />
          <span>从此轮分叉</span>
        </button>
        <div class="w-px h-3 bg-black/[0.08]"></div>
        <button
          type="button"
          @click="startEdit"
          class="flex items-center gap-1 px-2 py-0.5 rounded-full text-[10.5px] font-medium text-slate-600 hover:text-amber-700 hover:bg-amber-50/80 transition"
          title="改写此轮发言内容并开启新分支重演"
        >
          <Pencil class="w-3 h-3" />
          <span>改写重演</span>
        </button>
      </div>

      <!-- Header -->
      <div class="flex items-center gap-2 text-[11px] text-slate-500 mb-2 select-none">
        <span class="w-2 h-2 rounded-full shrink-0" :class="palette.dot"></span>
        <span class="font-semibold" :class="palette.label">
          {{ agentLabel }}
        </span>
        <span class="text-slate-300">·</span>
        <span class="font-mono text-[10.5px] text-slate-400">{{ modelName }}</span>
        <span class="text-slate-300">·</span>
        <template v-if="isStreaming">
          <span class="inline-flex items-center gap-1 text-[#007AFF] font-medium">
            <span class="w-1.5 h-1.5 rounded-full bg-[#007AFF] animate-ping"></span>
            {{ isActivelyThinking ? "思考中" : "生成中" }}
          </span>
        </template>
        <template v-else-if="durationMs !== undefined">
          <span class="tabular-nums">{{ (durationMs / 1000).toFixed(1) }}s</span>
        </template>
        <span class="ml-auto text-[10px] text-slate-400 tabular-nums">#{{ turnNumber }}</span>
      </div>

      <!-- Collapsible Reasoning / Inner Monologue Card -->
      <div v-if="hasReasoning" class="mb-2.5">
        <button
          type="button"
          @click="reasoningOpen = !reasoningOpen"
          class="w-full flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-slate-900/[0.045] hover:bg-slate-900/[0.075] border border-black/[0.06] text-[11px] text-slate-600 transition text-left"
        >
          <span
            v-if="isActivelyThinking"
            class="w-1.5 h-1.5 rounded-full bg-slate-500 animate-pulse shrink-0"
          ></span>
          <Brain v-else class="w-3.5 h-3.5 text-slate-400 shrink-0" />
          <span class="truncate font-medium">{{ thinkingSummary }}</span>
          <component
            :is="reasoningOpen ? ChevronDown : ChevronRight"
            class="w-3.5 h-3.5 text-slate-400 ml-auto shrink-0"
          />
        </button>

        <div
          v-if="reasoningOpen"
          class="mt-1.5 px-3.5 py-2.5 rounded-xl bg-slate-900/[0.035] border border-black/[0.05] text-xs leading-relaxed text-slate-500 whitespace-pre-wrap break-words max-h-64 overflow-y-auto font-mono"
        >
          {{ reasoningContent }}
        </div>
      </div>

      <!-- Inline Edit Mode -->
      <div v-if="editing" class="space-y-2">
        <textarea
          v-model="draftContent"
          rows="5"
          class="w-full rounded-xl bg-white/95 border border-black/[0.12] px-3 py-2 text-[13.5px] leading-relaxed text-[#1D1D1F] outline-none focus:ring-2 focus:ring-[#007AFF]/35"
        ></textarea>
        <div class="flex items-center justify-end gap-1.5">
          <button
            type="button"
            @click="cancelEdit"
            class="flex items-center gap-1 px-3 py-1 rounded-full text-xs text-slate-600 bg-white border border-black/[0.08] hover:bg-slate-100 transition"
          >
            <X class="w-3 h-3" />
            取消
          </button>
          <button
            type="button"
            @click="submitEditReplay"
            :disabled="!draftContent.trim()"
            class="flex items-center gap-1 px-3.5 py-1 rounded-full text-xs font-medium text-white bg-[#1D1D1F] hover:bg-black transition disabled:opacity-40"
          >
            <Check class="w-3 h-3" />
            改写并重演分支
          </button>
        </div>
      </div>

      <!-- Main Dialogue Content -->
      <div
        v-else-if="content"
        class="terrarium-prose"
        v-html="renderMarkdown(content, Boolean(isStreaming))"
      ></div>
      <div
        v-else-if="isStreaming && !hasReasoning"
        class="flex items-center gap-1.5 py-1 text-slate-400 text-xs"
      >
        <span class="w-1.5 h-1.5 rounded-full bg-slate-400 animate-bounce"></span>
        <span class="w-1.5 h-1.5 rounded-full bg-slate-400 animate-bounce [animation-delay:120ms]"></span>
        <span class="w-1.5 h-1.5 rounded-full bg-slate-400 animate-bounce [animation-delay:240ms]"></span>
      </div>

      <!-- Mobile Touch-friendly Branch & Edit Footer -->
      <div
        v-if="canBranch && !isStreaming && !editing"
        class="flex md:hidden items-center justify-end gap-2 mt-2.5 pt-2 border-t border-black/[0.05]"
      >
        <button
          type="button"
          @click="emit('fork', turnNumber)"
          class="flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-medium text-slate-600 bg-white/80 border border-black/[0.06] active:scale-95 transition"
        >
          <GitBranch class="w-3 h-3" />
          <span>从此轮分叉</span>
        </button>
        <button
          type="button"
          @click="startEdit"
          class="flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-medium text-slate-600 bg-white/80 border border-black/[0.06] active:scale-95 transition"
        >
          <Pencil class="w-3 h-3" />
          <span>改写重演</span>
        </button>
      </div>
    </div>
  </div>
</template>
