<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  ClipboardList,
  X,
  Shield,
  ShieldCheck,
  ShieldAlert,
  Pencil,
  Eye,
  GitCompare,
  Save,
} from "lucide-vue-next";
import type { BoardApprovalMode } from "../types";
import { renderMarkdown } from "../utils/markdown";
import { computeLineDiff } from "../utils/diff";

const props = defineProps<{
  open: boolean;
  embedded?: boolean;
  boardContent: string;
  boardPrevContent: string;
  approvalMode: BoardApprovalMode;
  lastActor?: string;
  lastReason?: string;
  readOnly?: boolean;
}>();

const emit = defineEmits<{
  close: [];
  saveBoard: [content: string];
  toggleApprovalMode: [mode: BoardApprovalMode];
}>();

const viewMode = ref<"rendered" | "diff" | "edit">("rendered");
const draftText = ref("");

watch(
  () => props.boardContent,
  (val) => {
    if (viewMode.value !== "edit") {
      draftText.value = val;
    }
  },
  { immediate: true }
);

const diffLines = computed(() =>
  computeLineDiff(props.boardPrevContent || "", props.boardContent || "")
);

const hasDiff = computed(
  () =>
    props.boardPrevContent !== props.boardContent &&
    (props.boardPrevContent.length > 0 || props.boardContent.length > 0)
);

function startEdit() {
  draftText.value = props.boardContent;
  viewMode.value = "edit";
}

function commitEdit() {
  emit("saveBoard", draftText.value);
  viewMode.value = "rendered";
}

function cycleApprovalMode() {
  const next: BoardApprovalMode =
    props.approvalMode === "auto"
      ? "guardian"
      : props.approvalMode === "guardian"
        ? "approval"
        : "auto";
  emit("toggleApprovalMode", next);
}
</script>

<template>
  <aside
    v-if="open"
    class="h-full w-full flex flex-col bg-[#FAFAFC] overflow-hidden"
    :class="embedded ? '' : 'md:w-[320px] lg:w-[360px] shrink-0 border-l border-black/[0.06] z-10'"
  >
    <!-- Header (hidden when embedded in desktop Inspector, shown on mobile or standalone) -->
    <div
      class="px-4 py-2.5 border-b border-black/[0.06] bg-white/85 flex items-center justify-between gap-2"
      :class="embedded ? 'md:hidden' : ''"
    >
      <div class="flex items-center gap-2 min-w-0">
        <div class="w-7 h-7 rounded-xl bg-[#007AFF]/10 text-[#007AFF] flex items-center justify-center shrink-0">
          <ClipboardList class="w-4 h-4" />
        </div>
        <div class="min-w-0">
          <h3 class="text-xs font-semibold text-[#1D1D1F] truncate">共享实验室白板</h3>
          <p class="text-[10.5px] text-slate-400 truncate">
            <template v-if="lastActor">{{ lastActor }}: {{ lastReason || "已更新" }}</template>
            <template v-else>AI 共享记忆 / 设定与线索板</template>
          </p>
        </div>
      </div>

      <button
        type="button"
        @click="emit('close')"
        class="p-1.5 rounded-full hover:bg-slate-100 text-slate-400 hover:text-slate-700 transition"
        title="收起白板面板"
      >
        <X class="w-4 h-4" />
      </button>
    </div>

    <!-- Approval Mode & View Switcher -->
    <div class="px-3 py-2 border-b border-black/[0.05] bg-white/60 flex items-center justify-between gap-1.5 shrink-0">
      <div class="flex items-center gap-0.5 p-0.5 rounded-full bg-[#F2F2F5] border border-black/[0.04]">
        <button
          type="button"
          @click="viewMode = 'rendered'"
          class="flex items-center gap-1 px-2 py-0.5 rounded-full text-[10.5px] font-medium transition whitespace-nowrap"
          :class="viewMode === 'rendered' ? 'bg-[#1D1D1F] text-white' : 'text-slate-600'"
        >
          <Eye class="w-3 h-3" />
          预览
        </button>
        <button
          type="button"
          @click="viewMode = 'diff'"
          class="flex items-center gap-1 px-2 py-0.5 rounded-full text-[10.5px] font-medium transition whitespace-nowrap"
          :class="viewMode === 'diff' ? 'bg-[#1D1D1F] text-white' : 'text-slate-600'"
        >
          <GitCompare class="w-3 h-3" />
          Diff
          <span v-if="hasDiff" class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
        </button>
        <button
          v-if="!readOnly"
          type="button"
          @click="startEdit"
          class="flex items-center gap-1 px-2 py-0.5 rounded-full text-[10.5px] font-medium transition whitespace-nowrap"
          :class="viewMode === 'edit' ? 'bg-[#1D1D1F] text-white' : 'text-slate-600'"
        >
          <Pencil class="w-3 h-3" />
          编辑
        </button>
      </div>

      <button
        v-if="!readOnly"
        type="button"
        @click="cycleApprovalMode"
        class="flex items-center gap-1 px-2 py-1 rounded-full text-[10.5px] font-medium border transition whitespace-nowrap shrink-0"
        :class="
          approvalMode === 'approval'
            ? 'bg-amber-50 text-amber-800 border-amber-200 hover:bg-amber-100/70'
            : approvalMode === 'guardian'
              ? 'bg-indigo-50 text-indigo-700 border-indigo-200 hover:bg-indigo-100/70'
              : 'bg-emerald-50 text-emerald-700 border-emerald-200 hover:bg-emerald-100/70'
        "
        :title="
          approvalMode === 'approval'
            ? '当前：人工审批（点击切换为自动放行）'
            : approvalMode === 'guardian'
              ? '当前：防泄密审核（点击切换为人工审批）'
              : '当前：自动放行（点击切换为防泄密审核）'
        "
      >
        <ShieldAlert v-if="approvalMode === 'approval'" class="w-3 h-3" />
        <Shield v-else-if="approvalMode === 'guardian'" class="w-3 h-3" />
        <ShieldCheck v-else class="w-3 h-3" />
        <span>{{
          approvalMode === "approval"
            ? "人工审批"
            : approvalMode === "guardian"
              ? "防泄密审核"
              : "自动放行"
        }}</span>
      </button>
    </div>

    <!-- Body -->
    <div class="flex-1 overflow-y-auto p-4">
      <template v-if="viewMode === 'edit'">
        <div class="h-full flex flex-col gap-2">
          <textarea
            v-model="draftText"
            placeholder="在此输入或编辑共享 Markdown 白板（如：角色设定、案件线索、已确认共识...）"
            class="flex-1 w-full rounded-2xl border border-black/[0.08] bg-white p-3 text-xs font-mono leading-relaxed text-[#1D1D1F] outline-none focus:ring-2 focus:ring-[#007AFF]/30 resize-none"
          ></textarea>
          <div class="flex justify-end gap-2">
            <button
              type="button"
              @click="viewMode = 'rendered'"
              class="px-3 py-1.5 rounded-full text-xs text-slate-600 hover:bg-slate-200/70 transition"
            >
              取消
            </button>
            <button
              type="button"
              @click="commitEdit"
              class="flex items-center gap-1 px-4 py-1.5 rounded-full text-xs font-medium bg-[#1D1D1F] hover:bg-black text-white transition"
            >
              <Save class="w-3.5 h-3.5" />
              保存白板
            </button>
          </div>
        </div>
      </template>

      <template v-else-if="viewMode === 'diff'">
        <div v-if="!boardContent && !boardPrevContent" class="text-xs text-slate-400 italic text-center py-8">
          白板尚无内容与修改记录
        </div>
        <div v-else class="rounded-2xl bg-white border border-black/[0.06] overflow-hidden font-mono text-[11.5px] leading-relaxed">
          <div
            v-for="(line, idx) in diffLines"
            :key="idx"
            class="px-3 py-0.5 whitespace-pre-wrap break-words"
            :class="{
              'bg-emerald-50/90 text-emerald-900 border-l-2 border-emerald-500': line.type === 'added',
              'bg-rose-50/90 text-rose-800 line-through border-l-2 border-rose-400': line.type === 'removed',
              'text-slate-600': line.type === 'same',
            }"
          >
            <span class="select-none mr-1.5 opacity-60">{{
              line.type === "added" ? "+" : line.type === "removed" ? "-" : " "
            }}</span>{{ line.text || " " }}
          </div>
        </div>
      </template>

      <template v-else>
        <div
          v-if="!boardContent.trim()"
          class="rounded-2xl border border-dashed border-slate-200 p-6 text-center text-xs text-slate-400 leading-relaxed"
        >
          共享白板当前为空。<br />
          白板内容已实时注入 AI 系统提示词，参与者可通过
          <code class="px-1 py-0.5 bg-slate-100 rounded text-[11px]">update_board</code> 工具共同维护此空间。
        </div>
        <div
          v-else
          class="terrarium-prose rounded-2xl bg-white p-4 border border-black/[0.06] shadow-[0_2px_10px_rgba(0,0,0,0.03)]"
          v-html="renderMarkdown(boardContent, false)"
        ></div>
      </template>
    </div>
  </aside>
</template>
