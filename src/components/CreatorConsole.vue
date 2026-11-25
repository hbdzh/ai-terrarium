<script setup lang="ts">
import { computed, ref, watch } from "vue";
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
} from "lucide-vue-next";
import type { BoardApprovalRequest, CrisisCard, NodeConfig, SpeakerRole } from "../types";
import { computeLineDiff } from "../utils/diff";

const props = defineProps<{
  queueLen: number;
  agents: NodeConfig[];
  pendingApproval?: BoardApprovalRequest | null;
  crisisCards?: CrisisCard[];
  observerRunning?: boolean;
}>();

const emit = defineEmits<{
  submit: [kind: SpeakerRole, agentId: string | null, content: string];
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
const editingApproval = ref(false);
const approvalDraft = ref("");

watch(
  () => props.pendingApproval,
  (req) => {
    editingApproval.value = false;
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

const mode = ref<SpeakerRole>("GodSystem");
const targetAgentId = ref<string>("");
const text = ref("");

watch(
  () => props.agents,
  (list) => {
    if (list.length > 0 && !list.some((a) => a.id === targetAgentId.value)) {
      targetAgentId.value = list[0].id;
    }
  },
  { immediate: true, deep: true }
);

const modes: {
  kind: SpeakerRole;
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
];

function currentHint() {
  return modes.find((m) => m.kind === mode.value)?.hint ?? "";
}

function handleSend() {
  const val = text.value.trim();
  if (!val) return;
  const agentId = mode.value === "GodWhisper" ? targetAgentId.value || null : null;
  emit("submit", mode.value, agentId, val);
  text.value = "";
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
</script>

<template>
  <div class="shrink-0 px-3 sm:px-6 md:px-10 pb-3 pt-1.5 z-10 space-y-2 bg-gradient-to-t from-[#F6F6F8]/95 via-white/75 to-transparent">
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
            <span class="text-slate-600">申请更新共享白板 · 理由：{{ pendingApproval.reason }}</span>
          </div>

          <div class="flex items-center gap-1.5">
            <button
              type="button"
              @click="editingApproval = !editingApproval"
              class="flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-medium bg-white border border-black/[0.08] text-slate-700 hover:bg-slate-50 transition"
            >
              <Pencil class="w-3 h-3" />
              {{ editingApproval ? "返回预览" : "修改内容" }}
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
              class="flex items-center gap-1 px-3 py-1 rounded-full text-[11px] font-medium bg-rose-600 hover:bg-rose-700 text-white transition"
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
              class="flex items-center gap-1 px-3.5 py-1 rounded-full text-[11px] font-medium bg-emerald-600 hover:bg-emerald-700 text-white transition"
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
          v-else
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
      class="pointer-events-auto max-w-4xl mx-auto rounded-3xl bg-white/85 backdrop-blur-2xl border border-black/[0.08] shadow-[0_12px_36px_rgba(0,0,0,0.09)] p-2.5"
    >
      <!-- Mode Capsule + Target Agent Selector + Queue Indicator -->
      <div class="flex flex-wrap items-center justify-between gap-2 px-1 pb-2">
        <div class="flex flex-wrap items-center gap-1.5">
          <div class="flex items-center gap-1 p-0.5 rounded-full bg-[#F2F2F5] border border-black/[0.04]">
            <button
              v-for="m in modes"
              :key="m.kind"
              @click="mode = m.kind"
              class="flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-medium transition"
              :class="
                mode === m.kind
                  ? m.activeClass
                  : 'text-slate-600 hover:text-[#1D1D1F]'
              "
              :title="m.hint"
            >
              <component :is="m.icon" class="w-3 h-3" />
              <span>{{ m.label }}</span>
            </button>
          </div>

          <!-- Dynamic Target AI Selector for Whisper -->
          <div
            v-if="mode === 'GodWhisper' && agents.length > 0"
            class="flex items-center gap-1 bg-indigo-50/80 border border-indigo-200/70 rounded-full px-2 py-0.5"
          >
            <span class="text-[10.5px] text-indigo-700 font-medium">→</span>
            <select
              v-model="targetAgentId"
              class="bg-transparent text-[11px] font-medium text-indigo-800 outline-none cursor-pointer pr-1"
            >
              <option v-for="a in agents" :key="a.id" :value="a.id">
                {{ a.label }}
              </option>
            </select>
          </div>
        </div>

        <div class="flex items-center gap-2 text-[11px] text-slate-400 pr-1">
          <button
            type="button"
            @click="cardsOpen = !cardsOpen"
            class="flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-medium border transition"
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
          <span class="hidden sm:inline">Enter 注入</span>
        </div>
      </div>

      <!-- Crisis Event Cards Drawer -->
      <div
        v-if="cardsOpen"
        class="mb-2 p-2.5 rounded-2xl bg-[#FAFAFC] border border-black/[0.06] space-y-2"
      >
        <div class="flex items-center justify-between px-1">
          <span class="text-[11px] font-semibold text-slate-600 flex items-center gap-1">
            <Sparkles class="w-3.5 h-3.5 text-violet-600" />
            观察员 C 现场危机/转折事件卡（点击即刻注入）
          </span>
          <button
            type="button"
            @click="emit('triggerObserver')"
            :disabled="observerRunning"
            class="text-[11px] text-[#007AFF] hover:underline disabled:opacity-45"
          >
            {{ observerRunning ? "生成中..." : "刷新/召唤抽卡" }}
          </button>
        </div>

        <div
          v-if="!crisisCards || crisisCards.length === 0"
          class="py-3 text-center text-xs text-slate-400"
        >
          尚无实时事件卡，请点击右上角「刷新/召唤抽卡」或等待观察员触发。
        </div>
        <div v-else class="grid grid-cols-1 md:grid-cols-3 gap-2">
          <button
            v-for="(card, idx) in crisisCards"
            :key="idx"
            type="button"
            @click="injectCrisisCard(card)"
            class="text-left p-2.5 rounded-xl bg-white hover:bg-violet-50/60 border border-black/[0.06] hover:border-violet-300 transition space-y-1"
          >
            <div class="flex items-center justify-between gap-1">
              <span class="text-xs font-semibold text-[#1D1D1F] truncate">{{ card.title }}</span>
              <span
                class="px-1.5 py-0.5 rounded-md text-[9.5px] font-medium shrink-0"
                :class="
                  card.kind === 'GodPublic'
                    ? 'bg-emerald-50 text-emerald-700'
                    : 'bg-slate-100 text-slate-700'
                "
              >
                {{ card.kind === "GodPublic" ? "路人降临" : "环境事件" }}
              </span>
            </div>
            <p class="text-[11px] text-slate-500 leading-relaxed line-clamp-3">
              {{ card.content }}
            </p>
          </button>
        </div>
      </div>

      <!-- Input Row -->
      <div class="flex items-end gap-2">
        <textarea
          v-model="text"
          @keydown="onKeydown"
          rows="1"
          :placeholder="currentHint()"
          class="flex-1 resize-none bg-transparent px-3 py-1.5 text-[15px] md:text-[13.5px] text-[#1D1D1F] placeholder:text-slate-400 outline-none max-h-28 leading-relaxed"
        ></textarea>
        <button
          @click="handleSend"
          :disabled="!text.trim()"
          class="shrink-0 w-10 h-10 md:w-9 md:h-9 rounded-2xl bg-[#1D1D1F] hover:bg-black text-white flex items-center justify-center transition active:scale-95 disabled:opacity-35"
          title="插入干预队列"
        >
          <Send class="w-4 h-4" />
        </button>
      </div>
    </div>
  </div>
</template>
