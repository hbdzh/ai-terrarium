<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useMediaQuery } from "@vueuse/core";
import {
  Globe2,
  EyeOff,
  MessageCircle,
  ShieldAlert,
  Check,
  X,
  Pencil,
  Dices,
  Sparkles,
  Wand2,
  DoorClosed,
  ArrowUpRight,
  ChevronDown,
  ArrowUp,
} from "lucide-vue-next";
import { invoke } from "@tauri-apps/api/core";
import type {
  ActivePrivateRoom,
  BoardApprovalRequest,
  CrisisCard,
  DiffLine,
  NodeConfig,
  SpeakerRole,
} from "../types";
import { isMobileOS } from "../utils/platform";
import { triggerHaptic } from "../utils/native";
import AdaptiveModal from "./ui/AdaptiveModal.vue";

const props = defineProps<{
  queueLen: number;
  agents: NodeConfig[];
  pendingApproval?: BoardApprovalRequest | null;
  crisisCards?: CrisisCard[];
  observerRunning?: boolean;
  activePrivateRoom?: ActivePrivateRoom | null;
  directorBusy?: boolean;
}>();

const emit = defineEmits<{
  submit: [kind: SpeakerRole, agentId: string | null, content: string];
  directorCommand: [instruction: string];
  startPrivateRoom: [memberIds: string[], turns: number];
  closePrivateRoom: [];
  resolveApproval: [
    payload: {
      requestId: string;
      approved: boolean;
      modifiedContent?: string;
      rejectReason?: string;
    }
  ];
  triggerObserver: [];
}>();

const isDesktopComposer = useMediaQuery("(min-width: 768px)");
const hasHoverPointer = useMediaQuery("(hover: hover) and (pointer: fine)");
const showKeyboardHints = computed(() => !isMobileOS && hasHoverPointer.value);

const cardsOpen = ref(false);
const roomMember1 = ref("");
const roomMember2 = ref("");
const roomTurns = ref(2);
const editingApproval = ref(false);
const showApprovalDiff = ref(false);
const approvalDraft = ref("");

watch(
  () => props.pendingApproval,
  (req) => {
    editingApproval.value = false;
    showApprovalDiff.value = false;
    approvalDraft.value = req?.proposedContent ?? "";
  }
);

const approvalDiff = ref<DiffLine[]>([]);

watch(
  [() => props.pendingApproval, showApprovalDiff],
  async ([req, show]) => {
    if (!req || !show) {
      approvalDiff.value = [];
      return;
    }
    try {
      approvalDiff.value = await invoke<DiffLine[]>("compute_line_diff", {
        oldText: req.currentContent || "",
        newText: req.proposedContent || "",
      });
    } catch {
      approvalDiff.value = [];
    }
  },
  { immediate: true }
);

type ConsoleMode = SpeakerRole | "DirectorRemote" | "PrivateRoom";
const mode = ref<ConsoleMode>("GodSystem");
const modeMenuOpen = ref(false);
const targetAgentId = ref<string>("");
const text = ref("");
const textareaRef = ref<HTMLTextAreaElement | null>(null);

function adjustTextareaHeight() {
  const el = textareaRef.value;
  if (!el) return;
  el.style.height = "auto";
  el.style.height = `${Math.min(el.scrollHeight, 128)}px`;
}

watch(text, async () => {
  await nextTick();
  adjustTextareaHeight();
});

watch(
  () => props.agents,
  (list) => {
    if (list.length > 0 && !list.some((a) => a.id === targetAgentId.value)) {
      targetAgentId.value = list[0].id;
    }
    if (list.length >= 2) {
      if (!list.some((a) => a.id === roomMember1.value)) {
        roomMember1.value = list[0].id;
      }
      if (!list.some((a) => a.id === roomMember2.value) || roomMember2.value === roomMember1.value) {
        roomMember2.value = list.find((a) => a.id !== roomMember1.value)?.id ?? list[1].id;
      }
    }
  },
  { immediate: true, deep: true }
);

const modes: {
  kind: ConsoleMode;
  label: string;
  shortLabel: string;
  hint: string;
  shortHint: string;
  icon: typeof Globe2;
  activeClass: string;
}[] = [
  {
    kind: "GodSystem",
    label: "环境旁白",
    shortLabel: "旁白",
    hint: "全知事件：同时注入所有智能体的系统上下文（如：窗外突然下起了红色的雨）",
    shortHint: "输入全知环境旁白（如：窗外突然下起了暴雨）",
    icon: Globe2,
    activeClass: "bg-[#1D1D1F] text-white shadow-xs",
  },
  {
    kind: "GodWhisper",
    label: "心灵低语",
    shortLabel: "低语",
    hint: "定向潜意识：仅悄悄塞入指定 AI 下一轮的 System Prompt，其余参与者完全不知情",
    shortHint: "向选定角色注入只有他能听见的暗语...",
    icon: EyeOff,
    activeClass: "bg-indigo-600 text-white shadow-xs",
  },
  {
    kind: "GodPublic",
    label: "亲自发言",
    shortLabel: "发言",
    hint: "路人降临：人类直接下场说一句话，所有 AI 均可见并回应",
    shortHint: "以人类/路人身份公开发言...",
    icon: MessageCircle,
    activeClass: "bg-emerald-600 text-white shadow-xs",
  },
  {
    kind: "DirectorRemote",
    label: "导演遥控",
    shortLabel: "遥控",
    hint: "后台自然语言指令：不中断运行热更新参数或向角色下达暗语（如：让阿哲别再装傻了，把观察间隔改为3轮）",
    shortHint: "输入后台导演指令（如：把观察周期改为3轮）",
    icon: Wand2,
    activeClass: "bg-amber-600 text-white shadow-xs",
  },
  {
    kind: "PrivateRoom",
    label: "私密房间",
    shortLabel: "里屋",
    hint: "空间隔离里屋：指定 2 名角色开启 1~5 轮临时私密房间，大厅其余角色完全无法听见",
    shortHint: "选择 2 名角色开启限时私密对话...",
    icon: DoorClosed,
    activeClass: "bg-violet-600 text-white shadow-xs",
  },
];

function currentHint() {
  const m = modes.find((item) => item.kind === mode.value);
  if (!m) return "";
  return showKeyboardHints.value ? m.hint : m.shortHint;
}

function currentModeMeta() {
  return modes.find((item) => item.kind === mode.value) ?? modes[0];
}

function selectMode(kind: ConsoleMode) {
  triggerHaptic("light");
  mode.value = kind;
  modeMenuOpen.value = false;
}

function handleSend() {
  if (mode.value === "PrivateRoom") return;
  const val = text.value.trim();
  if (!val) return;
  triggerHaptic("medium");
  if (mode.value === "DirectorRemote") {
    if (props.directorBusy) return;
    emit("directorCommand", val);
    text.value = "";
    return;
  }
  const agentId = mode.value === "GodWhisper" ? targetAgentId.value || null : null;
  emit("submit", mode.value, agentId, val);
  text.value = "";
}

function submitPrivateRoom() {
  if (props.agents.length < 3) return;
  if (!roomMember1.value || !roomMember2.value || roomMember1.value === roomMember2.value) return;
  const turns = Math.max(1, Math.min(5, roomTurns.value));
  emit("startPrivateRoom", [roomMember1.value, roomMember2.value], turns);
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey && showKeyboardHints.value) {
    e.preventDefault();
    handleSend();
  }
}

function injectCrisisCard(card: CrisisCard) {
  triggerHaptic("medium");
  const kind: SpeakerRole = card.kind === "GodPublic" ? "GodPublic" : "GodSystem";
  emit("submit", kind, null, card.content);
  cardsOpen.value = false;
}

function onTextareaFocus() {
  nextTick(() => {
    textareaRef.value?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  });
}

function roomMemberNames(memberIds: string[]): string {
  return memberIds
    .map((id) => props.agents.find((a) => a.id === id)?.label ?? id)
    .join(" × ");
}
</script>

<template>
  <div class="shrink-0 px-3 sm:px-5 md:px-8 pb-[calc(0.65rem+var(--safe-bottom,env(safe-area-inset-bottom)))] md:pb-5 pt-1.5 z-10 space-y-2 bg-gradient-to-t from-white via-white/95 to-transparent">
    <!-- Board Modification Approval Banner -->
    <Transition
      enter-active-class="transition duration-200 ease-out"
      enter-from-class="opacity-0 translate-y-3"
      enter-to-class="opacity-100 translate-y-0"
      leave-active-class="transition duration-150 ease-in"
      leave-from-class="opacity-100 translate-y-0"
      leave-to-class="opacity-0 translate-y-3"
    >
      <div
        v-if="pendingApproval"
        class="pointer-events-auto max-w-[740px] mx-auto rounded-3xl bg-amber-50/95 backdrop-blur-2xl border border-amber-300/80 shadow-[0_12px_36px_rgba(245,158,11,0.16)] p-3.5 space-y-2.5"
      >
        <div class="flex flex-wrap items-center justify-between gap-2">
          <div class="flex flex-wrap items-center gap-2 text-[13px] md:text-xs">
            <span class="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full bg-amber-500 text-white font-semibold text-[11px]">
              <ShieldAlert class="w-3 h-3" />
              白板修改待审批
            </span>
            <span class="font-semibold text-[#1D1D1F]">{{ pendingApproval.agentLabel }}</span>
            <span class="text-slate-600 break-words">申请更新共享白板 · {{ pendingApproval.reason }}</span>
          </div>

          <div class="flex items-center gap-1.5 flex-wrap">
            <button
              v-if="!editingApproval"
              type="button"
              @click="showApprovalDiff = !showApprovalDiff"
              class="h-8 md:h-auto px-3 md:px-2.5 py-1 rounded-full text-[12px] md:text-[11px] font-medium bg-white border border-black/[0.08] text-slate-700 hover:bg-slate-50 transition-colors"
            >
              {{ showApprovalDiff ? "收起 Diff" : "展开 Diff" }}
            </button>
            <button
              type="button"
              @click="editingApproval = !editingApproval"
              class="h-8 md:h-auto flex items-center gap-1 px-3 md:px-2.5 py-1 rounded-full text-[12px] md:text-[11px] font-medium bg-white border border-black/[0.08] text-slate-700 hover:bg-slate-50 transition-colors"
            >
              <Pencil class="w-3 h-3" />
              {{ editingApproval ? "返回" : "修改" }}
            </button>
            <button
              type="button"
              @click="
                emit('resolveApproval', {
                  requestId: pendingApproval.requestId,
                  approved: false,
                  rejectReason: '造物主驳回了本次白板写入申请',
                })
              "
              class="h-8 md:h-auto flex items-center gap-1 px-3 py-1 rounded-full text-[12px] md:text-[11px] font-medium bg-rose-600 hover:bg-rose-700 text-white transition-colors"
            >
              <X class="w-3.5 h-3.5 md:w-3 md:h-3" />
              驳回
            </button>
            <button
              type="button"
              @click="
                emit('resolveApproval', {
                  requestId: pendingApproval.requestId,
                  approved: true,
                  modifiedContent: editingApproval ? approvalDraft : undefined,
                })
              "
              class="h-8 md:h-auto flex items-center gap-1 px-3.5 py-1 rounded-full text-[12px] md:text-[11px] font-medium bg-emerald-600 hover:bg-emerald-700 text-white transition-colors"
            >
              <Check class="w-3.5 h-3.5 md:w-3 md:h-3" />
              {{ editingApproval ? "修改后放行" : "批准放行" }}
            </button>
          </div>
        </div>

        <textarea
          v-if="editingApproval"
          v-model="approvalDraft"
          rows="4"
          class="w-full rounded-2xl bg-white border border-amber-200 px-3 py-2 text-[14px] md:text-xs font-mono leading-relaxed text-[#1D1D1F] outline-none focus:ring-2 focus:ring-amber-400/40"
        ></textarea>
        <div
          v-else-if="showApprovalDiff"
          class="max-h-36 overflow-y-auto rounded-2xl bg-white/90 border border-amber-200/70 p-2 font-mono text-[11.5px] md:text-[11px] leading-relaxed"
        >
          <div
            v-for="(line, idx) in approvalDiff"
            :key="idx"
            class="px-2 py-0.5 whitespace-pre-wrap break-words"
            :class="{
              'bg-emerald-50 text-emerald-900': line.type === 'added',
              'bg-rose-50 text-rose-700 line-through': line.type === 'removed',
              'text-slate-500': line.type === 'same',
            }"
          >
            {{ line.type === "added" ? "+ " : line.type === "removed" ? "- " : "  " }}{{ line.text }}
          </div>
        </div>
      </div>
    </Transition>

    <!-- Floating Minimalist ChatGPT-Style Composer -->
    <div
      class="relative pointer-events-auto max-w-[740px] mx-auto rounded-[24px] bg-white border border-black/[0.1] shadow-[0_4px_24px_rgba(0,0,0,0.055)] p-2.5 md:px-4 md:pt-3 md:pb-2.5 space-y-2 md:space-y-1.5"
    >
      <!-- Crisis Event Cards Adaptive Modal -->
      <AdaptiveModal
        :open="cardsOpen"
        title="观察员 C 现场危机/转折事件卡"
        max-width-class="max-w-3xl"
        @close="cardsOpen = false"
      >
        <template #header>
          <div class="px-5 py-3.5 border-b border-black/[0.06] flex items-center justify-between gap-2 shrink-0">
            <div class="min-w-0">
              <div class="text-[14px] md:text-xs font-semibold text-[#1D1D1F] flex items-center gap-1.5">
                <Sparkles class="w-4 h-4 md:w-3.5 md:h-3.5 text-violet-600 shrink-0" />
                <span class="truncate">现场危机 / 转折事件卡</span>
              </div>
              <p class="text-[11.5px] md:text-[11px] text-slate-400 mt-0.5">轻触任意卡片即可立即注入主对话</p>
            </div>
            <div class="flex items-center gap-2 shrink-0">
              <button
                type="button"
                @click="emit('triggerObserver')"
                :disabled="observerRunning"
                class="h-8 px-3 rounded-full bg-blue-50 text-[12px] md:text-[11px] font-medium text-[#007AFF] hover:bg-blue-100/70 transition disabled:opacity-45"
              >
                {{ observerRunning ? "生成中..." : "刷新抽卡" }}
              </button>
              <button
                type="button"
                @click="cardsOpen = false"
                class="w-8 h-8 flex items-center justify-center rounded-full hover:bg-slate-100 text-slate-400 hover:text-slate-600"
              >
                <X class="w-4 h-4" />
              </button>
            </div>
          </div>
        </template>

        <div class="p-4 sm:p-5 overflow-y-auto flex-1 bg-[#F7F7FA]">
          <div
            v-if="!crisisCards || crisisCards.length === 0"
            class="py-10 text-center text-[13px] md:text-xs text-slate-400"
          >
            尚无实时事件卡，请点击右上角「刷新抽卡」或等待观察员触发。
          </div>
          <div v-else class="grid grid-cols-1 lg:grid-cols-3 gap-3">
            <button
              v-for="(card, idx) in crisisCards"
              :key="idx"
              type="button"
              @click="injectCrisisCard(card)"
              class="group text-left p-4 rounded-2xl bg-white hover:bg-violet-50/50 active:scale-[0.99] border border-black/[0.07] hover:border-violet-300 shadow-[0_1px_4px_rgba(0,0,0,0.03)] transition-all space-y-2.5 flex flex-col justify-between"
            >
              <div class="space-y-2">
                <div class="flex items-center justify-between gap-2">
                  <span class="text-[15px] md:text-[13px] font-semibold text-[#1D1D1F] truncate">{{ card.title }}</span>
                  <span
                    class="px-2 py-0.5 rounded-full text-[11px] md:text-[10px] font-medium shrink-0"
                    :class="
                      card.kind === 'GodPublic'
                        ? 'bg-emerald-50 text-emerald-700 border border-emerald-200/70'
                        : 'bg-slate-100 text-slate-700 border border-slate-200/70'
                    "
                  >
                    {{ card.kind === "GodPublic" ? "路人降临" : "环境事件" }}
                  </span>
                </div>
                <p class="text-[13.5px] md:text-[12px] text-slate-600 leading-relaxed break-words">
                  {{ card.content }}
                </p>
              </div>
              <div class="pt-1 flex items-center justify-end text-[11.5px] font-medium text-violet-600 opacity-85 group-hover:opacity-100">
                <span>点击立即注入</span>
                <ArrowUpRight class="w-3.5 h-3.5 ml-0.5" />
              </div>
            </button>
          </div>
        </div>
      </AdaptiveModal>

      <!-- Mobile Native Bottom Sheet for Mode Picker (< 768px) -->
      <AdaptiveModal
        v-if="!isDesktopComposer"
        :open="modeMenuOpen"
        title="选择干预方式"
        mobile-presentation="sheet"
        @close="modeMenuOpen = false"
      >
        <div class="px-4 pt-1 pb-4 space-y-1.5">
          <div class="px-1 pb-1.5 text-[13px] font-semibold text-[#1D1D1F]">
            选择干预方式
          </div>
          <button
            v-for="m in modes"
            :key="m.kind"
            type="button"
            @click="selectMode(m.kind)"
            class="w-full flex items-start gap-3 px-3.5 py-3 rounded-2xl text-left transition-colors"
            :class="
              mode === m.kind
                ? 'bg-[#1D1D1F]/[0.06] text-[#1D1D1F]'
                : 'bg-[#F7F7FA] text-slate-700 active:bg-black/[0.06]'
            "
          >
            <div
              class="w-8 h-8 rounded-xl flex items-center justify-center shrink-0 mt-0.5"
              :class="mode === m.kind ? m.activeClass : 'bg-white text-slate-600 border border-black/[0.06]'"
            >
              <component :is="m.icon" class="w-4 h-4" />
            </div>
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-1.5 text-[14px] font-semibold text-[#1D1D1F]">
                <span>{{ m.label }}</span>
                <span
                  v-if="m.kind === 'PrivateRoom' && activePrivateRoom"
                  class="px-1.5 py-0.5 rounded-full bg-violet-100 text-violet-700 text-[10.5px]"
                >
                  进行中 · {{ activePrivateRoom.remainingTurns }}轮
                </span>
              </div>
              <div class="text-[12px] text-slate-500 mt-0.5 leading-snug">{{ m.shortHint }}</div>
            </div>
            <Check v-if="mode === m.kind" class="w-4 h-4 text-[#1D1D1F] shrink-0 mt-1" />
          </button>
        </div>
      </AdaptiveModal>

      <!-- Private Room Inline Configuration & Countdown Panel -->
      <div
        v-if="mode === 'PrivateRoom'"
        class="px-2.5 py-2 rounded-2xl bg-[#F7F7FA] border border-black/[0.05]"
      >
        <!-- Active Room View -->
        <div
          v-if="activePrivateRoom"
          class="flex flex-wrap items-center justify-between gap-2.5 px-1 py-0.5"
        >
          <div class="flex items-center gap-2.5 min-w-0">
            <div class="w-8 h-8 rounded-xl bg-violet-100 text-violet-700 flex items-center justify-center shrink-0">
              <DoorClosed class="w-4 h-4" />
            </div>
            <div class="min-w-0">
              <div class="flex items-center gap-2 flex-wrap">
                <span class="text-[13px] md:text-xs font-semibold text-[#1D1D1F] truncate">
                  {{ activePrivateRoom.title || roomMemberNames(activePrivateRoom.memberIds) }}
                </span>
                <span class="px-2 py-0.5 rounded-full bg-violet-100 text-violet-700 text-[11px] font-semibold tabular-nums">
                  剩余 {{ activePrivateRoom.remainingTurns }} 轮
                </span>
              </div>
              <p class="text-[11.5px] md:text-[11px] text-slate-500 mt-0.5">
                空间隔离生效中：仅房间内 2 名角色互相可见对话。
              </p>
            </div>
          </div>

          <button
            type="button"
            @click="emit('closePrivateRoom')"
            class="h-9 md:h-auto flex items-center gap-1 px-3.5 py-1.5 rounded-xl bg-rose-600 hover:bg-rose-700 text-white text-xs font-medium transition-colors shrink-0"
          >
            <X class="w-3.5 h-3.5" />
            <span>一键解散房间</span>
          </button>
        </div>

        <!-- Insufficient Agents Notice -->
        <div
          v-else-if="agents.length < 3"
          class="px-2 py-2 text-[12.5px] md:text-xs text-slate-500"
        >
          临时私密里屋需要至少 <strong class="text-[#1D1D1F]">3 名</strong> AI 角色才能触发空间隔离（当前 {{ agents.length }} 名）。
        </div>

        <!-- Create Private Room Form -->
        <div
          v-else
          class="flex flex-wrap items-center justify-between gap-2.5 px-1 py-0.5"
        >
          <div class="flex flex-wrap items-center gap-2 min-w-0">
            <div class="flex items-center gap-1.5">
              <select
                v-model="roomMember1"
                class="h-9 md:h-auto rounded-xl border border-black/[0.08] bg-white px-2.5 py-1.5 text-[13px] md:text-xs text-[#1D1D1F] outline-none focus:ring-2 focus:ring-violet-400/30"
              >
                <option v-for="a in agents" :key="a.id" :value="a.id">{{ a.label }}</option>
              </select>
              <span class="text-xs text-slate-400">×</span>
              <select
                v-model="roomMember2"
                class="h-9 md:h-auto rounded-xl border border-black/[0.08] bg-white px-2.5 py-1.5 text-[13px] md:text-xs text-[#1D1D1F] outline-none focus:ring-2 focus:ring-violet-400/30"
              >
                <option v-for="a in agents" :key="a.id" :value="a.id">{{ a.label }}</option>
              </select>
            </div>

            <div class="flex items-center gap-1 p-0.5 rounded-xl bg-white border border-black/[0.06]">
              <span class="px-1.5 text-[11px] text-slate-400">轮次</span>
              <button
                v-for="t in [1, 2, 3, 4, 5]"
                :key="t"
                type="button"
                @click="roomTurns = t"
                class="w-7 h-7 md:w-auto md:h-auto md:px-2 md:py-0.5 rounded-lg text-[12px] md:text-[11px] font-medium tabular-nums transition-colors"
                :class="
                  roomTurns === t
                    ? 'bg-violet-600 text-white'
                    : 'text-slate-600 hover:bg-black/[0.05]'
                "
              >
                {{ t }}
              </button>
            </div>
          </div>

          <button
            type="button"
            @click="submitPrivateRoom"
            :disabled="!roomMember1 || !roomMember2 || roomMember1 === roomMember2"
            class="h-9 md:h-auto flex items-center gap-1.5 px-4 py-1.5 rounded-xl text-[12.5px] md:text-xs font-medium bg-violet-600 hover:bg-violet-700 text-white disabled:opacity-40 transition-colors shrink-0"
          >
            <DoorClosed class="w-3.5 h-3.5" />
            <span>开启私密里屋 ({{ roomTurns }} 轮)</span>
          </button>
        </div>
      </div>

      <!-- Standard Text Input Row (Unified clean layout across mobile & desktop) -->
      <div v-else>
        <textarea
          ref="textareaRef"
          v-model="text"
          @focus="onTextareaFocus"
          @keydown="onKeydown"
          rows="1"
          :placeholder="currentHint()"
          class="w-full resize-none bg-transparent px-1 md:px-0.5 py-1.5 md:py-1 min-h-[38px] md:min-h-[36px] text-[15px] md:text-[14.5px] text-[#1D1D1F] placeholder:text-slate-400 outline-none max-h-32 overflow-y-auto leading-relaxed"
        ></textarea>
      </div>

      <!-- Unified Bottom Action Bar inside Composer -->
      <div class="flex items-center justify-between gap-2 pt-0.5">
        <div class="flex items-center gap-1 min-w-0 flex-wrap">
          <!-- Mode Dropdown Selector (Bottom Sheet on mobile, Popover on desktop) -->
          <div class="relative">
            <div
              v-if="modeMenuOpen && isDesktopComposer"
              @click="modeMenuOpen = false"
              class="fixed inset-0 z-20"
            ></div>
            <button
              type="button"
              @click="triggerHaptic('light'); modeMenuOpen = !modeMenuOpen"
              class="h-8 flex items-center gap-1.5 px-2.5 rounded-full text-[12.5px] font-medium text-slate-700 bg-[#F4F4F6] md:bg-transparent hover:text-[#1D1D1F] hover:bg-black/[0.045] transition-colors"
              :title="currentModeMeta().hint"
            >
              <component :is="currentModeMeta().icon" class="w-3.5 h-3.5" />
              <span>{{ currentModeMeta().label }}</span>
              <ChevronDown class="w-3 h-3 opacity-60" />
            </button>

            <div
              v-if="modeMenuOpen && isDesktopComposer"
              class="absolute left-0 bottom-full mb-2 z-30 w-64 p-1.5 rounded-2xl bg-white/95 backdrop-blur-xl border border-black/[0.08] shadow-[0_14px_38px_rgba(0,0,0,0.14)] space-y-0.5"
            >
              <button
                v-for="m in modes"
                :key="m.kind"
                type="button"
                @click="selectMode(m.kind)"
                class="w-full flex items-start gap-2.5 px-3 py-2 rounded-xl text-left transition-colors"
                :class="mode === m.kind ? 'bg-black/[0.055] text-[#1D1D1F]' : 'text-slate-600 hover:bg-black/[0.035] hover:text-[#1D1D1F]'"
              >
                <component :is="m.icon" class="w-4 h-4 mt-0.5 shrink-0" />
                <div class="min-w-0">
                  <div class="text-[12.5px] font-medium flex items-center gap-1.5">
                    <span>{{ m.label }}</span>
                    <span
                      v-if="m.kind === 'PrivateRoom' && activePrivateRoom"
                      class="w-1.5 h-1.5 rounded-full bg-violet-600"
                    ></span>
                  </div>
                  <div class="text-[11px] text-slate-400 line-clamp-1 mt-0.5">{{ m.shortHint }}</div>
                </div>
              </button>
            </div>
          </div>

          <!-- Whisper Target Selector -->
          <div
            v-if="mode === 'GodWhisper' && agents.length > 0"
            class="flex items-center gap-1 px-2.5 h-7 rounded-full bg-indigo-50 text-indigo-700 text-[12px]"
          >
            <span class="opacity-65">→</span>
            <select
              v-model="targetAgentId"
              class="bg-transparent font-medium outline-none cursor-pointer"
            >
              <option v-for="a in agents" :key="a.id" :value="a.id">
                {{ a.label }}
              </option>
            </select>
          </div>

          <!-- Crisis Cards Trigger -->
          <button
            type="button"
            @click="triggerHaptic('light'); cardsOpen = !cardsOpen"
            class="h-8 flex items-center gap-1.5 px-2.5 rounded-full text-[12.5px] font-medium transition-colors"
            :class="
              cardsOpen
                ? 'bg-violet-50 text-violet-700'
                : 'text-slate-600 md:text-slate-500 bg-[#F4F4F6] md:bg-transparent hover:text-[#1D1D1F] hover:bg-black/[0.045]'
            "
            title="抽取观察员 C 现场生成的转折/危机事件卡"
          >
            <Dices class="w-3.5 h-3.5" />
            <span>抽卡</span>
            <span
              v-if="crisisCards && crisisCards.length > 0"
              class="px-1.5 rounded-full bg-black/[0.06] text-[10.5px] text-slate-600 tabular-nums"
            >
              {{ crisisCards.length }}
            </span>
          </button>

          <!-- Active Private Room Badge -->
          <div
            v-if="activePrivateRoom"
            @click="mode = 'PrivateRoom'"
            class="flex items-center gap-1 h-7 px-2.5 rounded-full bg-violet-50 text-violet-700 text-[11.5px] font-medium cursor-pointer"
            :title="activePrivateRoom.title || '临时私密房间激活中'"
          >
            <DoorClosed class="w-3 h-3" />
            <span class="tabular-nums">里屋余 {{ activePrivateRoom.remainingTurns }} 轮</span>
            <button
              type="button"
              @click.stop="emit('closePrivateRoom')"
              class="ml-0.5 hover:text-rose-600"
              title="一键解散房间"
            >
              <X class="w-3 h-3" />
            </button>
          </div>
        </div>

        <div class="flex items-center gap-2 shrink-0">
          <span
            v-if="queueLen > 0"
            class="px-2.5 py-0.5 rounded-full bg-blue-50 text-[#007AFF] text-[11.5px] font-medium"
          >
            待注入 {{ queueLen }}
          </span>
          <button
            v-if="mode !== 'PrivateRoom'"
            type="button"
            @click="handleSend"
            :disabled="!text.trim()"
            class="w-9 h-9 md:w-8 md:h-8 rounded-full bg-[#1D1D1F] hover:bg-black active:scale-95 text-white flex items-center justify-center transition disabled:opacity-20"
            title="发送注入 (Enter)"
          >
            <ArrowUp class="w-4 h-4 stroke-[2.2]" />
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
