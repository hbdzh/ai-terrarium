<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
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
  ChevronRight,
  GitBranch,
  Pencil,
  Check,
  X,
  Code2,
  MoreHorizontal,
  Copy,
} from "lucide-vue-next";
import { markdownEnhancersVersion, renderMarkdown } from "../utils/markdown";
import { writeClipboardText } from "../utils/clipboard";

export interface BubblePalette {
  bubble: string;
  dot: string;
  label: string;
  avatarBg?: string;
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
  microCue?: string | null;
  agentHasLeft?: boolean;
  isStreaming?: boolean;
  reasoningElapsedMs?: number;
  alignRight: boolean;
  palette: BubblePalette;
  highlighted?: boolean;
  canBranch?: boolean;
  blindGuessMode?: boolean;
  isTouchOrNarrow?: boolean;
}>();

const emit = defineEmits<{
  fork: [payload: { turnNumber: number; messageId?: string | null }];
  editAndReplay: [payload: { messageId: string; turnNumber: number; content: string }];
  inspectPrompt: [messageId: string];
}>();

const reasoningOpen = ref(false);
const editing = ref(false);
const draftContent = ref("");
const copied = ref(false);
const fallbackTouchOrNarrow =
  props.isTouchOrNarrow === undefined
    ? useMediaQuery("(hover: none), (max-width: 767px)")
    : null;
const effectiveTouchOrNarrow = computed(
  () => props.isTouchOrNarrow ?? fallbackTouchOrNarrow?.value ?? false
);

const renderedContent = ref("");
let mdThrottleTimer: number | null = null;
let lastMdRenderAt = 0;
let lastFinalRaw = "";
let lastFinalVer = -1;
let lastFinalHtml = "";

function updateRenderedContentNow() {
  if (mdThrottleTimer !== null) {
    window.clearTimeout(mdThrottleTimer);
    mdThrottleTimer = null;
  }
  const content = props.content;
  const streaming = Boolean(props.isStreaming);
  const ver = markdownEnhancersVersion.value;
  if (!content) {
    renderedContent.value = "";
    lastFinalRaw = "";
    lastFinalVer = ver;
    lastFinalHtml = "";
    lastMdRenderAt = performance.now();
    return;
  }
  if (!streaming) {
    if (content === lastFinalRaw && ver === lastFinalVer) {
      renderedContent.value = lastFinalHtml;
      return;
    }
    const html = renderMarkdown(content, false);
    lastFinalRaw = content;
    lastFinalVer = ver;
    lastFinalHtml = html;
    renderedContent.value = html;
    lastMdRenderAt = performance.now();
    return;
  }
  renderedContent.value = renderMarkdown(content, true);
  lastMdRenderAt = performance.now();
}

watch(
  () => [props.content, props.isStreaming, markdownEnhancersVersion.value] as const,
  ([content, isStreaming]) => {
    if (!content || !isStreaming) {
      updateRenderedContentNow();
      return;
    }
    const elapsed = performance.now() - lastMdRenderAt;
    if (elapsed >= 50) {
      updateRenderedContentNow();
    } else if (mdThrottleTimer === null) {
      mdThrottleTimer = window.setTimeout(
        updateRenderedContentNow,
        Math.max(10, 50 - elapsed)
      );
    }
  },
  { immediate: true }
);

onUnmounted(() => {
  if (mdThrottleTimer !== null) {
    window.clearTimeout(mdThrottleTimer);
    mdThrottleTimer = null;
  }
});

const hasReasoning = computed(() => props.reasoningContent.trim().length > 0);
const isActivelyThinking = computed(
  () => Boolean(props.isStreaming) && hasReasoning.value && props.content.length === 0
);

const thinkingSummary = computed(() => {
  if (isActivelyThinking.value) {
    const secs = ((props.reasoningElapsedMs ?? 0) / 1000).toFixed(1);
    return `正在思考中 (${secs}s)`;
  }
  if (props.reasoningElapsedMs && props.reasoningElapsedMs > 0) {
    return `思考用时 ${(props.reasoningElapsedMs / 1000).toFixed(1)}s`;
  }
  const chars = props.reasoningContent.trim().length;
  return `思考过程（${chars} 字）`;
});

const agentInitial = computed(() => {
  const trimmed = props.agentLabel.trim();
  return trimmed ? trimmed.slice(0, 1).toUpperCase() : "A";
});

async function copyContent() {
  if (!props.content) return;
  try {
    await writeClipboardText(props.content);
    copied.value = true;
    setTimeout(() => {
      copied.value = false;
    }, 1800);
  } catch {
    // ignore
  }
}

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
    class="terrarium-msg-item animate-msg-in flex min-w-0"
    :class="alignRight ? 'justify-end md:justify-start' : 'justify-start'"
  >
    <!-- Mobile (< md): Compact rounded bubble; Desktop (>= md): ChatGPT/Codex-style borderless document thread -->
    <div
      class="group relative min-w-0 max-w-[92%] md:max-w-none md:w-full rounded-[22px] px-4 py-3.5 border transition md:rounded-2xl md:border-0 md:bg-transparent md:shadow-none md:px-0 md:py-1.5"
      :class="[
        palette.bubble,
        alignRight ? 'rounded-tr-lg md:rounded-2xl' : 'rounded-tl-lg md:rounded-2xl',
        roomId && roomId !== 'main' ? 'md:bg-[#F7F6FC] md:px-4 md:py-3.5' : '',
        { 'animate-turn-highlight': highlighted },
        isStreaming ? 'shadow-[0_4px_18px_rgba(0,0,0,0.04)] md:shadow-none' : '',
      ]"
    >
      <!-- Speaker Header -->
      <div class="flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[12px] text-slate-500 mb-2 md:mb-1.5 select-none min-w-0">
        <!-- Mobile dot -->
        <span class="md:hidden w-2 h-2 rounded-full shrink-0" :class="palette.dot"></span>
        <!-- Desktop clean tinted avatar badge -->
        <span
          class="hidden md:inline-flex w-6 h-6 rounded-full items-center justify-center text-[11px] font-semibold shrink-0"
          :class="palette.avatarBg ?? 'bg-slate-100 text-slate-700'"
        >
          {{ agentInitial }}
        </span>

        <span class="font-semibold truncate max-w-[160px] sm:max-w-[220px] md:text-[13.5px]" :class="palette.label">
          {{ agentLabel }}
        </span>
        <span
          v-if="agentHasLeft"
          class="px-1.5 py-0.5 rounded-full bg-slate-100 text-slate-400 border border-slate-200/70 text-[10.5px] font-medium shrink-0"
          title="该角色已主动退场"
        >
          已离场
        </span>
        <span
          v-if="microCue"
          class="px-1.5 py-0.5 rounded-full bg-slate-100/90 text-slate-500 border border-slate-200/70 text-[10.5px] font-medium shrink-0"
          title="微表情 / 生理破绽信号"
        >
          {{ microCue }}
        </span>
        <span
          v-if="roomId && roomId !== 'main'"
          class="px-1.5 py-0.5 rounded-md bg-violet-100/90 text-violet-700 text-[10px] font-medium shrink-0"
          title="仅私聊成员可见"
        >
          私聊
        </span>
        <span class="font-mono text-[11px] text-slate-400 truncate max-w-[140px] sm:max-w-[220px]">{{ modelName }}</span>
        <template v-if="isStreaming">
          <span class="inline-flex items-center gap-1 text-[#007AFF] text-[11.5px] font-medium shrink-0">
            <span class="w-1.5 h-1.5 rounded-full bg-[#007AFF] animate-ping"></span>
            {{ isActivelyThinking ? "思考中" : "生成中" }}
          </span>
        </template>
        <template v-else-if="durationMs !== undefined">
          <span class="tabular-nums text-[11.5px] text-slate-400 shrink-0">· {{ (durationMs / 1000).toFixed(1) }}s</span>
        </template>
        <span class="ml-auto text-[11px] font-mono text-slate-300 md:text-slate-400/75 tabular-nums shrink-0">#{{ turnNumber }}</span>

        <!-- Touch / Narrow Screen Compact Action Menu -->
        <DropdownMenuRoot v-if="effectiveTouchOrNarrow && !isStreaming && !editing && (canBranch || messageId)">
          <DropdownMenuTrigger as-child>
            <button
              type="button"
              class="ml-0.5 w-8 h-8 -my-1.5 -mr-1.5 flex items-center justify-center rounded-full text-slate-400 hover:text-slate-700 hover:bg-black/[0.05] active:scale-95 transition shrink-0"
              aria-label="更多操作"
            >
              <MoreHorizontal class="w-4 h-4" />
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuPortal>
            <DropdownMenuContent
              align="end"
              :side-offset="4"
              class="z-50 min-w-[180px] rounded-2xl bg-white/95 backdrop-blur-2xl border border-black/[0.08] shadow-[0_12px_36px_rgba(0,0,0,0.16)] p-1.5 text-[13px] text-slate-700 outline-none"
            >
              <DropdownMenuItem
                v-if="content"
                @select="copyContent"
                class="flex items-center gap-2.5 px-3 py-2.5 rounded-xl cursor-pointer outline-none hover:bg-black/[0.05]"
              >
                <Copy class="w-4 h-4" />
                <span>复制内容</span>
              </DropdownMenuItem>
              <DropdownMenuItem
                v-if="messageId"
                @select="emit('inspectPrompt', messageId)"
                class="flex items-center gap-2.5 px-3 py-2.5 rounded-xl cursor-pointer outline-none hover:bg-indigo-50 hover:text-indigo-700 focus:bg-indigo-50 focus:text-indigo-700"
              >
                <Code2 class="w-4 h-4" />
                <span>查看完整提示词</span>
              </DropdownMenuItem>
              <template v-if="canBranch">
                <DropdownMenuSeparator v-if="messageId" class="my-1 h-px bg-black/[0.06]" />
                <DropdownMenuItem
                  @select="emit('fork', { turnNumber, messageId: messageId ?? null })"
                  class="flex items-center gap-2.5 px-3 py-2.5 rounded-xl cursor-pointer outline-none hover:bg-blue-50 hover:text-[#007AFF] focus:bg-blue-50 focus:text-[#007AFF]"
                >
                  <GitBranch class="w-4 h-4" />
                  <span>从此处分叉世界线</span>
                </DropdownMenuItem>
                <DropdownMenuItem
                  @select="startEdit"
                  class="flex items-center gap-2.5 px-3 py-2.5 rounded-xl cursor-pointer outline-none hover:bg-amber-50 hover:text-amber-700 focus:bg-amber-50 focus:text-amber-700"
                >
                  <Pencil class="w-4 h-4" />
                  <span>改写重演</span>
                </DropdownMenuItem>
              </template>
            </DropdownMenuContent>
          </DropdownMenuPortal>
        </DropdownMenuRoot>
      </div>

      <!-- Message Content Column (Aligned neatly under speaker name on Desktop: md:pl-8) -->
      <div class="md:pl-8 space-y-2">
        <!-- Collapsible Reasoning (ChatGPT-style quiet inline trigger on desktop; hidden in blindGuessMode) -->
        <div v-if="hasReasoning && !blindGuessMode">
          <button
            type="button"
            @click="reasoningOpen = !reasoningOpen"
            class="w-full md:w-auto flex items-center gap-1.5 px-3 py-2 md:px-0 md:py-0.5 rounded-xl md:rounded-none bg-slate-900/[0.045] md:bg-transparent hover:bg-slate-900/[0.075] md:hover:bg-transparent border border-black/[0.06] md:border-0 text-[12px] md:text-[12.5px] text-slate-600 md:text-slate-400 md:hover:text-slate-700 transition text-left"
          >
            <span
              v-if="isActivelyThinking"
              class="w-1.5 h-1.5 rounded-full bg-slate-500 animate-pulse shrink-0"
            ></span>
            <span class="truncate font-medium md:font-normal">{{ thinkingSummary }}</span>
            <ChevronRight
              class="w-3.5 h-3.5 text-slate-400 ml-auto md:ml-0 shrink-0 transition-transform duration-200"
              :class="reasoningOpen ? 'rotate-90' : ''"
            />
          </button>

          <div
            v-if="reasoningOpen"
            class="animate-tab-in mt-1.5 px-3.5 py-2.5 md:px-3.5 md:py-2 rounded-xl md:rounded-none bg-slate-900/[0.035] md:bg-transparent border border-black/[0.05] md:border-0 md:border-l-2 md:border-slate-200 text-xs md:text-[13px] leading-relaxed text-slate-500 whitespace-pre-wrap break-words max-h-64 overflow-y-auto font-mono md:font-sans"
          >
            {{ reasoningContent }}
          </div>
        </div>

        <!-- Inline Edit Mode -->
        <div v-if="editing" class="space-y-2">
          <textarea
            v-model="draftContent"
            rows="5"
            class="w-full rounded-xl bg-white border border-black/[0.12] px-3.5 py-2.5 text-[14px] leading-relaxed text-[#1D1D1F] outline-none focus:ring-2 focus:ring-[#007AFF]/30"
          ></textarea>
          <div class="flex items-center justify-end gap-1.5">
            <button
              type="button"
              @click="cancelEdit"
              class="flex items-center gap-1 px-3 py-1.5 rounded-full text-xs text-slate-600 bg-white border border-black/[0.08] hover:bg-slate-100 transition"
            >
              <X class="w-3 h-3" />
              取消
            </button>
            <button
              type="button"
              @click="submitEditReplay"
              :disabled="!draftContent.trim()"
              class="flex items-center gap-1 px-3.5 py-1.5 rounded-full text-xs font-medium text-white bg-[#1D1D1F] hover:bg-black transition disabled:opacity-40"
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
          v-html="renderedContent"
        ></div>
        <div
          v-else-if="isStreaming && !hasReasoning"
          class="flex items-center gap-1.5 py-1 text-slate-400 text-xs"
        >
          <span class="w-1.5 h-1.5 rounded-full bg-slate-400 animate-bounce"></span>
          <span class="w-1.5 h-1.5 rounded-full bg-slate-400 animate-bounce [animation-delay:120ms]"></span>
          <span class="w-1.5 h-1.5 rounded-full bg-slate-400 animate-bounce [animation-delay:240ms]"></span>
        </div>

        <!-- Desktop Silent Linear Footer Actions (ChatGPT style: quiet icons directly below text, never overlapping content) -->
        <div
          v-if="!effectiveTouchOrNarrow && !isStreaming && !editing && (canBranch || messageId || content)"
          class="hidden md:flex items-center gap-0.5 pt-0.5 opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity select-none"
        >
          <button
            v-if="content"
            type="button"
            @click="copyContent"
            class="w-7 h-7 flex items-center justify-center rounded-lg text-slate-400 hover:text-[#1D1D1F] hover:bg-black/[0.045] transition-colors"
            title="复制本轮内容"
          >
            <Check v-if="copied" class="w-3.5 h-3.5 text-emerald-600" />
            <Copy v-else class="w-3.5 h-3.5" />
          </button>
          <button
            v-if="messageId"
            type="button"
            @click="emit('inspectPrompt', messageId)"
            class="w-7 h-7 flex items-center justify-center rounded-lg text-slate-400 hover:text-indigo-600 hover:bg-indigo-50/70 transition-colors"
            title="查看完整提示词"
          >
            <Code2 class="w-3.5 h-3.5" />
          </button>
          <button
            v-if="canBranch"
            type="button"
            @click="emit('fork', { turnNumber, messageId: messageId ?? null })"
            class="w-7 h-7 flex items-center justify-center rounded-lg text-slate-400 hover:text-[#007AFF] hover:bg-blue-50/70 transition-colors"
            title="从此处分叉世界线"
          >
            <GitBranch class="w-3.5 h-3.5" />
          </button>
          <button
            v-if="canBranch"
            type="button"
            @click="startEdit"
            class="w-7 h-7 flex items-center justify-center rounded-lg text-slate-400 hover:text-amber-600 hover:bg-amber-50/70 transition-colors"
            title="改写内容并重演"
          >
            <Pencil class="w-3.5 h-3.5" />
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
