<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import {
  Send,
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
} from "lucide-vue-next";
import type {
  ActivePrivateRoom,
  BoardApprovalRequest,
  CrisisCard,
  NodeConfig,
  SpeakerRole,
} from "../types";
import { computeLineDiff } from "../utils/diff";
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

const approvalDiff = computed(() => {
  if (!props.pendingApproval) return [];
  return computeLineDiff(
    props.pendingApproval.currentContent || "",
    props.pendingApproval.proposedContent || ""
  );
});

type ConsoleMode = SpeakerRole | "DirectorRemote" | "PrivateRoom";
const mode = ref<ConsoleMode>("GodSystem");
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
  hint: string;
  icon: typeof Globe2;
  activeClass: string;
}[] = [
  {
    kind: "GodSystem",
    label: "环境旁白",
    hint: "全知事件：同时注入所有智能体的系统上下文（如：窗外突然下起了红色的雨）",
    icon: Globe2,
    activeClass: "bg-[#1D1D1F] text-white shadow-xs",
  },
  {
    kind: "GodWhisper",
    label: "心灵低语",
    hint: "定向潜意识：仅悄悄塞入指定 AI 下一轮的 System Prompt，其余参与者完全不知情",
    icon: EyeOff,
    activeClass: "bg-indigo-600 text-white shadow-xs",
  },
  {
    kind: "GodPublic",
    label: "亲自发言",
    hint: "路人降临：人类直接下场说一句话，所有 AI 均可见并回应",
    icon: MessageCircle,
    activeClass: "bg-emerald-600 text-white shadow-xs",
  },
  {
    kind: "DirectorRemote",
    label: "导演遥控",
    hint: "后台自然语言指令：不中断运行热更新参数或向角色下达暗语（如：让阿哲别再装傻了，把观察间隔改为3轮）",
    icon: Wand2,
    activeClass: "bg-amber-600 text-white shadow-xs",
  },
  {
    kind: "PrivateRoom",
    label: "私密房间",
    hint: "空间隔离里屋：指定 2 名角色开启 1~5 轮临时私密房间，大厅其余角色完全无法听见",
    icon: DoorClosed,
    activeClass: "bg-violet-600 text-white shadow-xs",
  },
];

function currentHint() {
  return modes.find((m) => m.kind === mode.value)?.hint ?? "";
}

function handleSend() {
  if (mode.value === "PrivateRoom") return;
  const val = text.value.trim();
  if (!val) return;
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
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    handleSend();
  }
}

function injectCrisisCard(card: CrisisCard) {
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
  <div class="shrink-0 px-2.5 sm:px-6 md:px-10 pb-[calc(0.5rem+env(safe-area-inset-bottom))] md:pb-[calc(0.75rem+env(safe-area-inset-bottom))] pt-1.5 z-10 space-y-2 bg-gradient-to-t from-[#F6F6F8]/95 via-white/75 to-transparent">
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
        class="pointer-events-auto max-w-4xl mx-auto rounded-3xl bg-amber-50/95 backdrop-blur-2xl border border-amber-300/80 shadow-[0_12px_36px_rgba(245,158,11,0.16)] p-3.5 space-y-2.5"
      >
        <div class="flex flex-wrap items-center justify-between gap-2">
          <div class="flex items-center gap-2 text-xs">
            <span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-amber-500 text-white font-semibold text-[11px]">
              <ShieldAlert class="w-3 h-3" />
              白板修改待审批
            </span>
            <span class="font-semibold text-[#1D1D1F]">{{ pendingApproval.agentLabel }}</span>
            <span class="text-slate-600 break-words">申请更新共享白板 · 理由：{{ pendingApproval.reason }}</span>
          </div>

          <div class="flex items-center gap-1.5 flex-wrap">
            <button
              v-if="!editingApproval"
              type="button"
              @click="showApprovalDiff = !showApprovalDiff"
              class="px-2.5 py-1 rounded-full text-[11px] font-medium bg-white border border-black/[0.08] text-slate-700 hover:bg-slate-50 transition-colors"
            >
              {{ showApprovalDiff ? "收起 Diff" : "展开 Diff" }}
            </button>
            <button
              type="button"
              @click="editingApproval = !editingApproval"
              class="flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-medium bg-white border border-black/[0.08] text-slate-700 hover:bg-slate-50 transition-colors"
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
              class="flex items-center gap-1 px-3 py-1 rounded-full text-[11px] font-medium bg-rose-600 hover:bg-rose-700 text-white transition-colors"
            >
              <X class="w-3 h-3" />
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
              class="flex items-center gap-1 px-3.5 py-1 rounded-full text-[11px] font-medium bg-emerald-600 hover:bg-emerald-700 text-white transition-colors"
            >
              <Check class="w-3 h-3" />
              {{ editingApproval ? "修改后放行" : "批准放行" }}
            </button>
          </div>
        </div>

        <textarea
          v-if="editingApproval"
          v-model="approvalDraft"
          rows="4"
          class="w-full rounded-2xl bg-white border border-amber-200 px-3 py-2 text-xs font-mono leading-relaxed text-[#1D1D1F] outline-none focus:ring-2 focus:ring-amber-400/40"
        ></textarea>
        <div
          v-else-if="showApprovalDiff"
          class="max-h-36 overflow-y-auto rounded-2xl bg-white/90 border border-amber-200/70 p-2 font-mono text-[11px] leading-relaxed"
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

    <div
      class="relative pointer-events-auto max-w-4xl mx-auto rounded-3xl bg-white/90 backdrop-blur-xl border border-black/[0.08] shadow-[0_12px_36px_rgba(0,0,0,0.09)] p-2.5"
    >
      <!-- Crisis Event Cards Adaptive Modal -->
      <AdaptiveModal
        :open="cardsOpen"
        title="观察员 C 现场危机/转折事件卡"
        max-width-class="max-w-2xl"
        @close="cardsOpen = false"
      >
        <template #header>
          <div class="px-5 py-3.5 border-b border-black/[0.06] flex items-center justify-between gap-2 shrink-0">
            <span class="text-xs font-semibold text-slate-700 flex items-center gap-1.5">
              <Sparkles class="w-3.5 h-3.5 text-violet-600 shrink-0" />
              <span>观察员 C 现场危机/转折事件卡（点击即刻注入）</span>
            </span>
            <div class="flex items-center gap-2 shrink-0">
              <button
                type="button"
                @click="emit('triggerObserver')"
                :disabled="observerRunning"
                class="text-[11px] font-medium text-[#007AFF] hover:underline disabled:opacity-45"
              >
                {{ observerRunning ? "生成中..." : "刷新/召唤抽卡" }}
              </button>
              <button
                type="button"
                @click="cardsOpen = false"
                class="p-1 rounded-full hover:bg-slate-100 text-slate-400 hover:text-slate-600"
              >
                <X class="w-3.5 h-3.5" />
              </button>
            </div>
          </div>
        </template>

        <div class="p-4 sm:p-5 overflow-y-auto flex-1">
          <div
            v-if="!crisisCards || crisisCards.length === 0"
            class="py-8 text-center text-xs text-slate-400"
          >
            尚无实时事件卡，请点击右上角「刷新/召唤抽卡」或等待观察员触发。
          </div>
          <div v-else class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2.5">
            <button
              v-for="(card, idx) in crisisCards"
              :key="idx"
              type="button"
              @click="injectCrisisCard(card)"
              class="text-left p-3 rounded-2xl bg-[#FAFAFC] hover:bg-violet-50/70 border border-black/[0.06] hover:border-violet-300 transition-colors space-y-1.5 flex flex-col justify-between"
            >
              <div class="flex items-center justify-between gap-1.5">
                <span class="text-xs font-semibold text-[#1D1D1F] truncate">{{ card.title }}</span>
                <span
                  class="px-1.5 py-0.5 rounded-md text-[9.5px] font-medium shrink-0"
                  :class="
                    card.kind === 'GodPublic'
                      ? 'bg-emerald-50 text-emerald-700'
                      : 'bg-slate-200/70 text-slate-700'
                  "
                >
                  {{ card.kind === "GodPublic" ? "路人降临" : "环境事件" }}
                </span>
              </div>
              <p class="text-[11.5px] text-slate-600 leading-relaxed break-words">
                {{ card.content }}
              </p>
            </button>
          </div>
        </div>
      </AdaptiveModal>

      <!-- Mode Capsule + Target Agent Selector + Private Room Badge + Queue Indicator -->
      <div class="flex flex-wrap items-center justify-between gap-1.5 md:gap-2 px-0.5 md:px-1 pb-2">
        <div class="flex flex-wrap items-center gap-1.5 min-w-0 max-w-full">
          <div class="flex flex-nowrap md:flex-wrap items-center gap-1 p-0.5 rounded-full bg-[#F2F2F5] border border-black/[0.04] overflow-x-auto no-scrollbar max-w-full">
            <button
              v-for="m in modes"
              :key="m.kind"
              type="button"
              @click="mode = m.kind"
              class="relative flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-medium transition-colors whitespace-nowrap shrink-0"
              :class="
                mode === m.kind
                  ? m.activeClass
                  : 'text-slate-600 hover:text-[#1D1D1F]'
              "
              :title="m.hint"
            >
              <component :is="m.icon" class="w-3 h-3" />
              <span>{{ m.label }}</span>
              <span
                v-if="m.kind === 'PrivateRoom' && activePrivateRoom"
                class="w-1.5 h-1.5 rounded-full"
                :class="mode === 'PrivateRoom' ? 'bg-white' : 'bg-violet-600 animate-pulse'"
              ></span>
            </button>
          </div>

          <!-- Dynamic Target AI Selector for Whisper -->
          <div
            v-if="mode === 'GodWhisper' && agents.length > 0"
            class="flex items-center gap-1 bg-indigo-50/80 border border-indigo-200/70 rounded-full px-2 py-0.5 max-w-[160px]"
          >
            <span class="text-[10.5px] text-indigo-700 font-medium shrink-0">→</span>
            <select
              v-model="targetAgentId"
              class="bg-transparent text-[11px] font-medium text-indigo-800 outline-none cursor-pointer pr-1 truncate min-w-0"
            >
              <option v-for="a in agents" :key="a.id" :value="a.id">
                {{ a.label }}
              </option>
            </select>
          </div>
        </div>

        <div class="flex items-center gap-1.5 text-[11px] text-slate-400 pr-1">
          <!-- Always-visible live Private Room countdown badge when active -->
          <div
            v-if="activePrivateRoom"
            @click="mode = 'PrivateRoom'"
            class="flex items-center gap-1 px-2.5 py-1 rounded-full bg-violet-50 border border-violet-200 text-violet-700 text-[11px] font-medium cursor-pointer"
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

          <button
            type="button"
            @click="cardsOpen = !cardsOpen"
            class="flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-medium border transition-colors"
            :class="
              cardsOpen
                ? 'bg-violet-600 text-white border-violet-600'
                : 'bg-[#F2F2F5] hover:bg-[#E7E7EC] text-slate-700 border-black/[0.05]'
            "
            title="抽取观察员 C 现场生成的转折/危机事件卡"
          >
            <Dices class="w-3.5 h-3.5" />
            <span>事件抽卡</span>
            <span
              v-if="crisisCards && crisisCards.length > 0"
              class="px-1 rounded-full bg-black/10 text-[10px] tabular-nums"
            >
              {{ crisisCards.length }}
            </span>
          </button>
          <span
            v-if="queueLen > 0"
            class="px-2 py-0.5 rounded-full bg-blue-50 text-[#007AFF] font-medium"
          >
            队列待注入: {{ queueLen }}
          </span>
          <span v-if="mode !== 'PrivateRoom'" class="hidden sm:inline">Enter 注入</span>
        </div>
      </div>

      <!-- Private Room Inline Configuration & Countdown Panel -->
      <div
        v-if="mode === 'PrivateRoom'"
        class="px-2 py-1.5 rounded-2xl bg-[#FAFAFC] border border-black/[0.05]"
      >
        <!-- Active Room View -->
        <div
          v-if="activePrivateRoom"
          class="flex flex-wrap items-center justify-between gap-2.5 px-1.5 py-1"
        >
          <div class="flex items-center gap-2 min-w-0">
            <div class="w-7 h-7 rounded-xl bg-violet-100 text-violet-700 flex items-center justify-center shrink-0">
              <DoorClosed class="w-3.5 h-3.5" />
            </div>
            <div class="min-w-0">
              <div class="flex items-center gap-2 flex-wrap">
                <span class="text-xs font-semibold text-[#1D1D1F] truncate">
                  {{ activePrivateRoom.title || roomMemberNames(activePrivateRoom.memberIds) }}
                </span>
                <span class="px-2 py-0.5 rounded-full bg-violet-100 text-violet-700 text-[10.5px] font-semibold tabular-nums">
                  剩余 {{ activePrivateRoom.remainingTurns }} 轮倒计时
                </span>
              </div>
              <p class="text-[11px] text-slate-500 mt-0.5">
                空间隔离生效中：仅房间内 2 名角色互相可见对话，大厅其余角色无法听见。
              </p>
            </div>
          </div>

          <button
            type="button"
            @click="emit('closePrivateRoom')"
            class="flex items-center gap-1 px-3 py-1.5 rounded-xl bg-rose-600 hover:bg-rose-700 text-white text-xs font-medium transition-colors shrink-0"
          >
            <X class="w-3.5 h-3.5" />
            <span>一键解散房间</span>
          </button>
        </div>

        <!-- Insufficient Agents Notice -->
        <div
          v-else-if="agents.length < 3"
          class="px-2 py-2 text-xs text-slate-500 flex items-center justify-between gap-2"
        >
          <span>
            临时私密里屋需要至少 <strong class="text-[#1D1D1F]">3 名</strong> AI 角色才能触发空间隔离（当前 {{ agents.length }} 名）。
          </span>
        </div>

        <!-- Create Private Room Form -->
        <div
          v-else
          class="flex flex-wrap items-center justify-between gap-2.5 px-1.5 py-1"
        >
          <div class="flex flex-wrap items-center gap-2 min-w-0">
            <div class="flex items-center gap-1.5">
              <select
                v-model="roomMember1"
                class="rounded-xl border border-black/[0.08] bg-white px-2.5 py-1.5 text-xs text-[#1D1D1F] outline-none focus:ring-2 focus:ring-violet-400/30"
              >
                <option v-for="a in agents" :key="a.id" :value="a.id">{{ a.label }}</option>
              </select>
              <span class="text-xs text-slate-400">×</span>
              <select
                v-model="roomMember2"
                class="rounded-xl border border-black/[0.08] bg-white px-2.5 py-1.5 text-xs text-[#1D1D1F] outline-none focus:ring-2 focus:ring-violet-400/30"
              >
                <option v-for="a in agents" :key="a.id" :value="a.id">{{ a.label }}</option>
              </select>
            </div>

            <div class="flex items-center gap-1 p-0.5 rounded-xl bg-white border border-black/[0.06]">
              <span class="px-1.5 text-[10.5px] text-slate-400">轮次</span>
              <button
                v-for="t in [1, 2, 3, 4, 5]"
                :key="t"
                type="button"
                @click="roomTurns = t"
                class="px-2 py-0.5 rounded-lg text-[11px] font-medium tabular-nums transition-colors"
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
            class="flex items-center gap-1.5 px-3.5 py-1.5 rounded-xl text-xs font-medium bg-violet-600 hover:bg-violet-700 text-white disabled:opacity-40 transition-colors shrink-0"
          >
            <DoorClosed class="w-3.5 h-3.5" />
            <span>开启私密里屋 ({{ roomTurns }} 轮)</span>
          </button>
        </div>
      </div>

      <!-- Standard Text Input Row -->
      <div v-else class="flex items-end gap-2">
        <textarea
          ref="textareaRef"
          v-model="text"
          @focus="onTextareaFocus"
          @keydown="onKeydown"
          rows="1"
          :placeholder="currentHint()"
          class="flex-1 resize-none bg-transparent px-3 py-1.5 text-[15px] md:text-[13.5px] text-[#1D1D1F] placeholder:text-slate-400 outline-none max-h-32 overflow-y-auto leading-relaxed"
        ></textarea>
        <button
          type="button"
          @click="handleSend"
          :disabled="!text.trim()"
          class="shrink-0 w-10 h-10 md:w-9 md:h-9 rounded-2xl bg-[#1D1D1F] hover:bg-black text-white flex items-center justify-center transition-colors disabled:opacity-35"
          title="插入干预队列"
        >
          <Send class="w-4 h-4" />
        </button>
      </div>
    </div>
  </div>
</template>
