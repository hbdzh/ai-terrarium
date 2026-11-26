<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import type { ExperimentDetail } from "../types";
import {
  buildPureScriptDocument,
  buildReplayHandoutSecrets,
  stripMicroCueTagsClient,
  type ReplayHandoutSecrets,
} from "../utils/scriptExport";

type PureScriptDocument = ReturnType<typeof buildPureScriptDocument>;

const props = defineProps<{
  experiment: ExperimentDetail | null;
}>();

const doc = ref<PureScriptDocument | null>(null);
const secrets = ref<ReplayHandoutSecrets | null>(null);
const printMode = ref<"pure" | "replay">("pure");

function refresh(ev?: Event) {
  const custom = ev as CustomEvent<{ mode?: "pure" | "replay" }> | undefined;
  printMode.value = custom?.detail?.mode === "replay" ? "replay" : "pure";
  if (!props.experiment) {
    doc.value = null;
    secrets.value = null;
    return;
  }
  doc.value = buildPureScriptDocument(props.experiment);
  secrets.value =
    printMode.value === "replay" ? buildReplayHandoutSecrets(props.experiment) : null;
}

onMounted(() => {
  window.addEventListener("beforeprint", refresh);
  window.addEventListener("terrarium:prepare-print", refresh);
});

onUnmounted(() => {
  window.removeEventListener("beforeprint", refresh);
  window.removeEventListener("terrarium:prepare-print", refresh);
});

defineExpose({ refresh });
</script>

<template>
  <div id="pure-script-print-root" class="hidden print:block bg-white text-[#1D1D1F] p-4">
    <template v-if="doc">
      <header class="pb-4 mb-5 border-b border-slate-200 space-y-2.5">
        <h1 class="text-2xl font-bold tracking-tight text-[#1D1D1F]">{{ doc.title }}</h1>

        <div v-if="doc.tags.length > 0" class="flex flex-wrap gap-1.5">
          <span
            v-for="tag in doc.tags"
            :key="tag"
            class="px-2.5 py-0.5 rounded-full bg-slate-100 text-[11px] font-medium text-slate-700"
          >
            {{ tag }}
          </span>
        </div>

        <div v-if="doc.cast.length > 0" class="flex flex-wrap items-center gap-4 text-[12px] text-slate-600 pt-1">
          <span
            v-for="actor in doc.cast"
            :key="actor.label"
            class="inline-flex items-center gap-1.5 font-medium"
          >
            <span
              class="w-2.5 h-2.5 rounded-full inline-block"
              :style="{ backgroundColor: actor.colorHex }"
            ></span>
            <span>{{ actor.label }}</span>
          </span>
        </div>
      </header>

      <section
        v-if="doc.boardContent"
        class="mb-6 p-4 rounded-xl bg-slate-50 border border-slate-200 break-inside-avoid"
      >
        <h2 class="text-xs font-semibold text-slate-700 mb-1.5">现场线索与共识</h2>
        <pre class="whitespace-pre-wrap text-[12px] leading-relaxed text-slate-600 font-sans">{{ doc.boardContent }}</pre>
      </section>

      <main class="space-y-4">
        <template v-for="(item, idx) in doc.items" :key="idx">
          <div
            v-if="item.kind === 'room_header'"
            class="px-3.5 py-2 rounded-lg bg-violet-50 border border-violet-200 text-xs font-semibold text-violet-800 break-inside-avoid"
          >
            {{ item.content }}
          </div>

          <div
            v-else-if="item.kind === 'observer_note'"
            class="my-4 p-3.5 rounded-xl bg-slate-50 border border-blue-200 break-inside-avoid"
          >
            <div class="text-[11px] font-semibold text-blue-700 mb-1">
              分析笔记 · {{ item.meta }}
            </div>
            <div class="text-[12.5px] leading-relaxed text-slate-700">{{ item.content }}</div>
          </div>

          <div
            v-else-if="item.kind === 'god_system' || item.kind === 'god_public'"
            class="px-3.5 py-2.5 rounded-xl border break-inside-avoid"
            :class="
              item.kind === 'god_system'
                ? 'bg-slate-900 text-white border-slate-800'
                : 'bg-emerald-50 text-emerald-900 border-emerald-200'
            "
          >
            <div class="text-[11px] font-semibold opacity-80 mb-0.5">
              {{ item.kind === "god_system" ? "场景事件" : "外部插话" }}
            </div>
            <div class="text-[13px] leading-relaxed whitespace-pre-wrap">{{ item.content }}</div>
          </div>

          <div
            v-else-if="item.kind === 'pass'"
            class="text-[11.5px] italic text-slate-400 pl-3"
          >
            — {{ item.content }} —
          </div>

          <article
            v-else
            class="break-inside-avoid pl-3"
            :class="item.roomLabel ? 'py-2 pr-3 rounded-xl bg-violet-50/40' : ''"
          >
            <div class="flex items-center gap-1.5 mb-1">
              <span
                class="w-2 h-2 rounded-full inline-block"
                :style="{ backgroundColor: item.colorHex }"
              ></span>
              <span
                class="text-[13px] font-semibold"
                :style="{ color: item.colorHex }"
              >
                {{ item.speaker }}
              </span>
              <span
                v-if="item.roomLabel"
                class="px-1.5 py-0.5 rounded bg-violet-100 text-violet-700 text-[10px] font-medium"
              >
                {{ item.roomLabel }}
              </span>
            </div>
            <p class="text-[14px] leading-[1.7] text-[#1D1D1F] whitespace-pre-wrap pl-3.5">
              {{ printMode === "replay" ? stripMicroCueTagsClient(item.content) : item.content }}
            </p>
          </article>
        </template>
      </main>

      <section
        v-if="printMode === 'replay' && secrets"
        class="mt-10 pt-8 border-t-2 border-dashed border-slate-300 break-before-page space-y-6"
      >
        <div>
          <h2 class="text-lg font-bold text-[#1D1D1F] mb-3">幕后真相与复盘档案</h2>
          <div class="space-y-3">
            <div
              v-for="ag in secrets.agents"
              :key="ag.label"
              class="p-3.5 rounded-xl bg-slate-50 border border-slate-200 break-inside-avoid space-y-1.5"
            >
              <div class="flex items-center justify-between gap-2">
                <span class="text-[13px] font-bold text-[#1D1D1F]">{{ ag.label }}</span>
                <span
                  v-if="ag.secretGoal"
                  class="px-2 py-0.5 rounded-full text-[10.5px] font-medium"
                  :class="ag.goalAchieved ? 'bg-amber-100 text-amber-800' : 'bg-slate-200 text-slate-600'"
                >
                  {{ ag.goalAchieved ? "🏆 暗线已达成" : "暗线未达成" }}
                </span>
              </div>
              <p class="text-[12px] leading-relaxed text-slate-700 whitespace-pre-wrap">{{ ag.systemPrompt }}</p>
              <p v-if="ag.secretGoal" class="text-[11.5px] text-amber-800">
                暗线目标：{{ ag.secretGoal }}
              </p>
            </div>
          </div>
        </div>

        <div v-if="secrets.privateRooms.length > 0" class="space-y-3">
          <h3 class="text-sm font-bold text-[#1D1D1F]">私密里屋密谈摘要</h3>
          <div
            v-for="room in secrets.privateRooms"
            :key="room.roomTitle"
            class="p-3.5 rounded-xl bg-violet-50/50 border border-violet-200 break-inside-avoid space-y-1.5"
          >
            <div class="text-[12px] font-semibold text-violet-800">{{ room.roomTitle }}</div>
            <div
              v-for="(m, i) in room.messages"
              :key="i"
              class="text-[12px] leading-relaxed text-slate-700"
            >
              <span class="font-semibold">[第{{ m.turnNumber }}轮 {{ m.speaker }}]：</span>{{ m.content }}
            </div>
          </div>
        </div>

        <div class="p-4 rounded-xl bg-slate-50 border border-slate-200 break-inside-avoid">
          <h3 class="text-sm font-bold text-[#1D1D1F] mb-1.5">最终局势分析结论</h3>
          <pre class="whitespace-pre-wrap text-[12px] leading-relaxed text-slate-700 font-sans">{{ secrets.finalConclusion }}</pre>
        </div>
      </section>

      <footer class="mt-8 pt-3 border-t border-slate-200 text-[11px] text-slate-400">
        AI Terrarium · {{ printMode === "replay" ? "剧本杀复盘手卡" : "剧本导出" }}
      </footer>
    </template>
  </div>
</template>
