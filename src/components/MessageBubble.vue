<script setup lang="ts">
import { computed, ref } from "vue";
import { useMediaQuery } from "@vueuse/core";
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "reka-ui";
import {
  ChevronDown,
  ChevronRight,
  Brain,
  GitBranch,
  Pencil,
  Check,
  X,
  Code2,
  MoreHorizontal,
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
  roomId?: string;
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
  inspectPrompt: [messageId: string];
}>();

const reasoningOpen = ref(false);
const editing = ref(false);
const draftContent = ref("");
const isTouchOrNarrow = useMediaQuery("(hover: none), (max-width: 767px)");

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
      <!-- Desktop Hover Branch, Inspect & Replay Actions -->
      <div
        v-if="!isTouchOrNarrow && !isStreaming && !editing && (canBranch || messageId)"
        class="hidden md:flex opacity-0 group-hover:opacity-100 focus-within:opacity-100 absolute top-2 right-3 items-center gap-1 px-1.5 py-0.5 rounded-full bg-white/95 backdrop-blur-md border border-black/[0.08] shadow-sm transition-opacity z-10"
      >
        <button
          v-if="messageId"
          type="button"
          @click="emit('inspectPrompt', messageId)"
          class="flex items-center gap-1 px-2 py-0.5 rounded-full text-[10.5px] font-medium text-slate-600 hover:text-indigo-700 hover:bg-indigo-50/80 transition"
          title="查看本轮发给模型的完整提示词"
        >
          <Code2 class="w-3 h-3" />
          <span>查看完整提示词</span>
        </button>
        <template v-if="canBranch">
          <div v-if="messageId" class="w-px h-3 bg-black/[0.08]"></div>
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
        </template>
      </div>

      <!-- Header -->
      <div class="flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[11px] text-slate-500 mb-2 select-none min-w-0">
        <span class="w-2 h-2 rounded-full shrink-0" :class="palette.dot"></span>
        <span class="font-semibold truncate max-w-[140px] sm:max-w-[200px]" :class="palette.label">
          {{ agentLabel }}
        </span>
        <span
          v-if="roomId && roomId !== 'main'"
          class="px-1.5 py-0.2 rounded-md bg-violet-100/90 text-violet-700 text-[9.5px] font-medium shrink-0"
          title="仅该私密房间成员互相可见"
        >
          里屋私聊
        </span>
        <span class="text-slate-300 shrink-0">·</span>
        <span class="font-mono text-[10.5px] text-slate-400 truncate max-w-[140px] sm:max-w-[220px]">{{ modelName }}</span>
        <span class="text-slate-300 shrink-0">·</span>
        <template v-if="isStreaming">
          <span class="inline-flex items-center gap-1 text-[#007AFF] font-medium shrink-0">
            <span class="w-1.5 h-1.5 rounded-full bg-[#007AFF] animate-ping"></span>
            {{ isActivelyThinking ? "思考中" : "生成中" }}
          </span>
        </template>
        <template v-else-if="durationMs !== undefined">
          <span class="tabular-nums shrink-0">{{ (durationMs / 1000).toFixed(1) }}s</span>
        </template>
        <span class="ml-auto text-[10px] text-slate-400 tabular-nums shrink-0">#{{ turnNumber }}</span>

        <!-- Touch / Narrow Screen Compact Action Menu -->
        <DropdownMenuRoot v-if="isTouchOrNarrow && !isStreaming && !editing && (canBranch || messageId)">
          <DropdownMenuTrigger as-child>
            <button
              type="button"
              class="ml-0.5 p-1 -mr-1 rounded-full text-slate-400 hover:text-slate-700 hover:bg-black/[0.05] active:scale-95 transition shrink-0"
              aria-label="更多操作"
            >
              <MoreHorizontal class="w-4 h-4" />
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuPortal>
            <DropdownMenuContent
              align="end"
              :side-offset="4"
              class="z-50 min-w-[164px] rounded-2xl bg-white/95 backdrop-blur-2xl border border-black/[0.08] shadow-[0_12px_36px_rgba(0,0,0,0.16)] p-1.5 text-xs text-slate-700 outline-none"
            >
              <DropdownMenuItem
                v-if="messageId"
                @select="emit('inspectPrompt', messageId)"
                class="flex items-center gap-2 px-2.5 py-2 rounded-xl cursor-pointer outline-none hover:bg-indigo-50 hover:text-indigo-700 focus:bg-indigo-50 focus:text-indigo-700"
              >
                <Code2 class="w-3.5 h-3.5" />
                <span>查看完整提示词</span>
              </DropdownMenuItem>
              <template v-if="canBranch">
                <DropdownMenuSeparator v-if="messageId" class="my-1 h-px bg-black/[0.06]" />
                <DropdownMenuItem
                  @select="emit('fork', turnNumber)"
                  class="flex items-center gap-2 px-2.5 py-2 rounded-xl cursor-pointer outline-none hover:bg-blue-50 hover:text-[#007AFF] focus:bg-blue-50 focus:text-[#007AFF]"
                >
                  <GitBranch class="w-3.5 h-3.5" />
                  <span>从此轮分叉</span>
                </DropdownMenuItem>
                <DropdownMenuItem
                  @select="startEdit"
                  class="flex items-center gap-2 px-2.5 py-2 rounded-xl cursor-pointer outline-none hover:bg-amber-50 hover:text-amber-700 focus:bg-amber-50 focus:text-amber-700"
                >
                  <Pencil class="w-3.5 h-3.5" />
                  <span>改写重演</span>
                </DropdownMenuItem>
              </template>
            </DropdownMenuContent>
          </DropdownMenuPortal>
        </DropdownMenuRoot>
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

    </div>
  </div>
</template>
