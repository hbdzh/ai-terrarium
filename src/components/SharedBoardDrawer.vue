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
  Lock,
} from "lucide-vue-next";
import { invoke } from "@tauri-apps/api/core";
import type { BoardApprovalMode, DiffLine, NodeConfig } from "../types";
import { renderMarkdown } from "../utils/markdown";

const props = defineProps<{
  open: boolean;
  embedded?: boolean;
  boardContent: string;
  boardPrevContent: string;
  approvalMode: BoardApprovalMode;
  lastActor?: string;
  lastReason?: string;
  readOnly?: boolean;
  agents?: NodeConfig[];
  privateNotes?: Record<string, string>;
  enablePrivateNotes?: boolean;
  blindGuessMode?: boolean;
}>();

const emit = defineEmits<{
  close: [];
  saveBoard: [content: string];
  toggleApprovalMode: [mode: BoardApprovalMode];
}>();

const viewMode = ref<"rendered" | "diff" | "edit" | "notes">("rendered");
const draftText = ref("");

const agentNoteEntries = computed(() => {
  const notesMap = props.privateNotes ?? {};
  const list = props.agents ?? [];
  const entries = list.map((ag, idx) => ({
    id: ag.id,
    label: ag.label || `角色 ${idx + 1}`,
    modelName: ag.modelName,
    note: (notesMap[ag.id] ?? "").trim(),
  }));
  // Include any orphaned notes keyed by agentId if agents list changed
  const knownIds = new Set(list.map((a) => a.id));
  Object.entries(notesMap).forEach(([agentId, raw]) => {
    const trimmed = (raw ?? "").trim();
    if (!knownIds.has(agentId) && trimmed) {
      entries.push({
        id: agentId,
        label: agentId,
        modelName: "",
        note: trimmed,
      });
    }
  });
  return entries;
});

const nonEmptyNoteCount = computed(
  () => agentNoteEntries.value.filter((e) => e.note.length > 0).length
);

watch(
  () => props.boardContent,
  (val) => {
    if (viewMode.value !== "edit") {
      draftText.value = val;
    }
  },
  { immediate: true }
);

const diffLines = ref<DiffLine[]>([]);

watch(
  [() => props.boardPrevContent, () => props.boardContent, viewMode],
  async ([prev, curr, mode]) => {
    if (mode !== "diff") return;
    try {
      diffLines.value = await invoke<DiffLine[]>("compute_line_diff", {
        oldText: prev || "",
        newText: curr || "",
      });
    } catch {
      diffLines.value = [];
    }
  },
  { immediate: true }
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
    class="h-full w-full flex flex-col bg-white overflow-hidden"
    :class="embedded ? '' : 'md:w-[320px] lg:w-[360px] shrink-0 border-l border-black/[0.035] z-10'"
  >
    <!-- Header (hidden when embedded in mobile/desktop tab switcher) -->
    <div
      v-if="!embedded"
      class="px-4 py-2.5 border-b border-black/[0.06] bg-white/85 flex items-center justify-between gap-2"
    >
      <div class="flex items-center gap-2 min-w-0">
        <div class="w-7 h-7 rounded-xl bg-[#007AFF]/10 text-[#007AFF] flex items-center justify-center shrink-0">
          <ClipboardList class="w-4 h-4" />
        </div>
        <div class="min-w-0">
          <h3 class="text-xs font-semibold text-[#1D1D1F] truncate">共享白板</h3>
          <p class="text-[10.5px] text-slate-400 truncate">
            <template v-if="lastActor">{{ lastActor }}: {{ lastReason || "已更新" }}</template>
            <template v-else>角色共享设定与线索记录</template>
          </p>
        </div>
      </div>

      <button
        type="button"
        @click="emit('close')"
        class="p-1.5 rounded-full hover:bg-slate-100 text-slate-400 hover:text-slate-700 transition"
        title="收起面板"
      >
        <X class="w-4 h-4" />
      </button>
    </div>

    <!-- Approval Mode & Light Underline View Switcher -->
    <div class="bg-white border-b border-[#ECECF0] shrink-0">
      <div v-if="!readOnly" class="px-4 pt-2.5 pb-1 flex items-center justify-between gap-2">
        <span class="text-[11.5px] text-[#858A95] truncate">
          <template v-if="lastActor">{{ lastActor }}: {{ lastReason || "已更新" }}</template>
          <template v-else>角色共享设定与线索</template>
        </span>
        <button
          type="button"
          @click="cycleApprovalMode"
          class="h-7 flex items-center gap-1 px-2.5 rounded-full text-[11.5px] font-medium border transition whitespace-nowrap shrink-0"
          :class="
            approvalMode === 'approval'
              ? 'bg-amber-50 text-amber-800 border-amber-200 hover:bg-amber-100/70'
              : approvalMode === 'guardian'
                ? 'bg-[var(--ds-accent-soft)] text-[var(--ds-accent)] border-[var(--ds-accent-border)] hover:bg-[var(--ds-accent-soft-hover)]'
                : 'bg-emerald-50 text-emerald-700 border-emerald-200 hover:bg-emerald-100/70'
          "
          :title="
            approvalMode === 'approval'
              ? '当前：人工审核（点击切换为自动放行）'
              : approvalMode === 'guardian'
                ? '当前：防泄密审核（点击切换为人工审核）'
                : '当前：自动放行（点击切换为防泄密审核）'
          "
        >
          <ShieldAlert v-if="approvalMode === 'approval'" class="w-3.5 h-3.5" />
          <Shield v-else-if="approvalMode === 'guardian'" class="w-3.5 h-3.5" />
          <ShieldCheck v-else class="w-3.5 h-3.5" />
          <span>{{
            approvalMode === "approval"
              ? "人工审核"
              : approvalMode === "guardian"
                ? "防泄密审核"
                : "自动放行"
          }}</span>
        </button>
      </div>

      <div class="px-4 flex items-center gap-4 text-[12.5px] overflow-x-auto no-scrollbar">
        <button
          type="button"
          @click="viewMode = 'rendered'"
          class="relative flex items-center gap-1.5 py-2.5 transition-colors whitespace-nowrap shrink-0"
          :class="viewMode === 'rendered' ? 'text-[#1D1D1F] font-semibold' : 'text-[#858A95] hover:text-[#1D1D1F] font-medium'"
        >
          <Eye class="w-3.5 h-3.5" />
          <span>预览</span>
          <span
            v-if="viewMode === 'rendered'"
            class="absolute inset-x-0 bottom-0 h-[2px] rounded-full bg-[#1D1D1F]"
          ></span>
        </button>
        <button
          type="button"
          @click="viewMode = 'diff'"
          class="relative flex items-center gap-1.5 py-2.5 transition-colors whitespace-nowrap shrink-0"
          :class="viewMode === 'diff' ? 'text-[#1D1D1F] font-semibold' : 'text-[#858A95] hover:text-[#1D1D1F] font-medium'"
        >
          <GitCompare class="w-3.5 h-3.5" />
          <span>修改对比</span>
          <span v-if="hasDiff" class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
          <span
            v-if="viewMode === 'diff'"
            class="absolute inset-x-0 bottom-0 h-[2px] rounded-full bg-[#1D1D1F]"
          ></span>
        </button>
        <button
          v-if="!readOnly"
          type="button"
          @click="startEdit"
          class="relative flex items-center gap-1.5 py-2.5 transition-colors whitespace-nowrap shrink-0"
          :class="viewMode === 'edit' ? 'text-[#1D1D1F] font-semibold' : 'text-[#858A95] hover:text-[#1D1D1F] font-medium'"
        >
          <Pencil class="w-3.5 h-3.5" />
          <span>编辑</span>
          <span
            v-if="viewMode === 'edit'"
            class="absolute inset-x-0 bottom-0 h-[2px] rounded-full bg-[#1D1D1F]"
          ></span>
        </button>
        <button
          type="button"
          @click="viewMode = 'notes'"
          class="relative flex items-center gap-1.5 py-2.5 transition-colors whitespace-nowrap shrink-0"
          :class="viewMode === 'notes' ? 'text-[var(--ds-accent)] font-semibold' : 'text-[#858A95] hover:text-[#1D1D1F] font-medium'"
          title="查看各角色的私密备忘"
        >
          <Lock class="w-3.5 h-3.5" />
          <span>备忘</span>
          <span
            v-if="nonEmptyNoteCount > 0"
            class="px-1.5 py-0.5 rounded-full text-[10.5px] leading-none tabular-nums"
            :class="viewMode === 'notes' ? 'bg-[var(--ds-accent-soft)] text-[var(--ds-accent)]' : 'bg-black/[0.05] text-[#666B76]'"
          >
            {{ nonEmptyNoteCount }}
          </span>
          <span
            v-if="viewMode === 'notes'"
            class="absolute inset-x-0 bottom-0 h-[2px] rounded-full bg-[var(--ds-accent)]"
          ></span>
        </button>
      </div>
    </div>

    <!-- Body -->
    <div class="relative flex-1 min-h-0 flex flex-col">
      <div
        class="pointer-events-none absolute inset-x-0 top-0 h-5 z-10 bg-gradient-to-b from-white via-white/80 to-transparent"
      ></div>
      <div class="flex-1 overflow-y-auto px-4 pt-3 pb-[calc(1.25rem+env(safe-area-inset-bottom))]">
      <template v-if="viewMode === 'edit'">
        <div class="h-full flex flex-col gap-2.5">
          <textarea
            v-model="draftText"
            placeholder="输入或编辑共享白板内容（支持 Markdown）"
            class="flex-1 w-full rounded-2xl border border-black/[0.08] bg-white p-3.5 text-[14px] md:text-xs font-mono leading-relaxed text-[#1D1D1F] outline-none focus:ring-2 focus:ring-[#007AFF]/30 resize-none"
          ></textarea>
          <div class="flex justify-end gap-2">
            <button
              type="button"
              @click="viewMode = 'rendered'"
              class="h-10 md:h-auto px-4 md:px-3 py-1.5 rounded-full text-[13px] md:text-xs text-slate-600 hover:bg-slate-200/70 transition"
            >
              取消
            </button>
            <button
              type="button"
              @click="commitEdit"
              class="h-10 md:h-auto flex items-center gap-1.5 px-5 md:px-4 py-1.5 rounded-full text-[13px] md:text-xs font-medium bg-[#1D1D1F] hover:bg-black text-white transition"
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

      <template v-else-if="viewMode === 'notes'">
        <div class="space-y-3">
          <div class="flex items-center justify-between gap-2 px-0.5">
            <div class="flex items-center gap-1.5 text-[12px] font-semibold text-[#1D1D1F]">
              <Lock class="w-3.5 h-3.5 text-[var(--ds-accent)]" />
              <span>角色私密备忘</span>
            </div>
            <span class="text-[11px] text-slate-400">仅本人可见</span>
          </div>

          <div
            v-if="blindGuessMode"
            class="rounded-2xl border border-dashed border-[var(--ds-accent-border)] bg-[var(--ds-accent-soft)] p-5 text-center text-xs text-[var(--ds-accent)] leading-relaxed space-y-1"
          >
            <div class="font-semibold">盲猜模式已开启</div>
            <div class="text-[11px] text-slate-500">
              私密备忘已隐藏，退出盲猜模式后可查看。
            </div>
          </div>

          <div
            v-else-if="!enablePrivateNotes && nonEmptyNoteCount === 0"
            class="rounded-2xl border border-dashed border-slate-200 p-5 text-center text-xs text-slate-500 leading-relaxed space-y-1"
          >
            <div>未开启「角色私密备忘」</div>
            <div class="text-[11px] text-slate-400">
              可在「设置 → 角色行为」或新建推演时开启。
            </div>
          </div>

          <div v-else class="space-y-2.5">
            <div
              v-for="item in agentNoteEntries"
              :key="item.id"
              class="rounded-2xl border p-3.5 space-y-1.5 transition"
              :class="
                item.note
                  ? 'border-[var(--ds-accent-border)] bg-[var(--ds-accent-soft)]'
                  : 'border-black/[0.06] bg-[#FAFAFC]'
              "
            >
              <div class="flex items-center justify-between gap-2">
                <div class="flex items-center gap-1.5 min-w-0">
                  <span
                    class="w-2 h-2 rounded-full shrink-0"
                    :class="item.note ? 'bg-[var(--ds-accent)]' : 'bg-slate-300'"
                  ></span>
                  <span class="text-[12.5px] font-semibold text-[#1D1D1F] truncate">{{ item.label }}</span>
                  <span v-if="item.modelName" class="text-[10.5px] text-slate-400 font-mono truncate">
                    {{ item.modelName }}
                  </span>
                </div>
                <span class="text-[10.5px] text-slate-400 tabular-nums shrink-0">
                  {{ item.note.length }} / 150 字
                </span>
              </div>
              <p
                v-if="item.note"
                class="text-[12.5px] leading-relaxed text-[#1D1D1F] whitespace-pre-wrap break-words"
              >
                {{ item.note }}
              </p>
              <p v-else class="text-[11.5px] text-slate-400 italic">
                暂未写下私密备忘
              </p>
            </div>
          </div>
        </div>
      </template>

      <template v-else>
        <div
          v-if="!boardContent.trim()"
          class="rounded-2xl border border-dashed border-slate-200 p-6 text-center text-xs text-slate-400 leading-relaxed"
        >
          共享白板为空。<br />
          角色在对话中会通过工具自动记录关键线索与共识。
        </div>
        <div
          v-else
          class="terrarium-prose rounded-2xl bg-white p-4 border border-black/[0.06] shadow-[0_2px_10px_rgba(0,0,0,0.03)]"
          v-html="renderMarkdown(boardContent, false)"
        ></div>

        <!-- Inline Private Notes Overview in Preview when enabled or populated -->
        <div
          v-if="enablePrivateNotes || nonEmptyNoteCount > 0"
          class="mt-4 pt-4 border-t border-black/[0.06] space-y-2.5"
        >
          <div class="flex items-center justify-between gap-2 px-0.5">
            <div class="flex items-center gap-1.5 text-[12px] font-semibold text-[#1D1D1F]">
              <Lock class="w-3.5 h-3.5 text-[var(--ds-accent)]" />
              <span>角色私密备忘</span>
              <span class="px-1.5 py-0.5 rounded-full bg-[var(--ds-accent-soft)] text-[var(--ds-accent)] text-[10.5px] tabular-nums">
                {{ nonEmptyNoteCount }}/{{ agentNoteEntries.length }}
              </span>
            </div>
            <button
              type="button"
              @click="viewMode = 'notes'"
              class="text-[11px] font-medium text-[var(--ds-accent)] hover:text-[var(--ds-accent-hover)]"
            >
              单独查看 →
            </button>
          </div>

          <div
            v-if="blindGuessMode"
            class="rounded-2xl border border-dashed border-[var(--ds-accent-border)] bg-[var(--ds-accent-soft)] p-4 text-center text-[11.5px] text-[var(--ds-accent)]"
          >
            盲猜模式已开启，私密备忘已隐藏
          </div>
          <div v-else class="space-y-2">
            <div
              v-for="item in agentNoteEntries"
              :key="item.id"
              class="rounded-2xl border p-3 space-y-1"
              :class="
                item.note
                  ? 'border-[var(--ds-accent-border)] bg-[var(--ds-accent-soft)]'
                  : 'border-black/[0.05] bg-[#FAFAFC]'
              "
            >
              <div class="flex items-center justify-between gap-2">
                <span class="text-[12px] font-semibold text-[#1D1D1F] truncate">{{ item.label }}</span>
                <span class="text-[10.5px] text-slate-400 tabular-nums shrink-0">{{ item.note.length }} 字</span>
              </div>
              <p
                v-if="item.note"
                class="text-[12px] leading-relaxed text-slate-700 whitespace-pre-wrap break-words"
              >
                {{ item.note }}
              </p>
              <p v-else class="text-[11.5px] text-slate-400 italic">暂未记录</p>
            </div>
          </div>
        </div>
      </template>
      </div>
    </div>
  </aside>
</template>
