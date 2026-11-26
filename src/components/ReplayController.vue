<script setup lang="ts">
import { Play, Pause, SkipBack, SkipForward, X, Film } from "lucide-vue-next";
import RangeSlider from "./ui/RangeSlider.vue";

defineProps<{
  playing: boolean;
  speed: 1 | 2 | 5;
  currentIndex: number;
  totalCount: number;
  currentTurn: number;
  totalTurns: number;
}>();

const emit = defineEmits<{
  togglePlay: [];
  setSpeed: [speed: 1 | 2 | 5];
  seek: [index: number];
  stepPrev: [];
  stepNext: [];
  exit: [];
}>();
</script>

<template>
  <div class="shrink-0 px-3 sm:px-6 md:px-10 pb-[calc(0.75rem+env(safe-area-inset-bottom))] pt-1.5 z-20">
    <div
      class="terrarium-stay-dark max-w-3xl mx-auto rounded-3xl md:rounded-2xl bg-[#1D1D1F]/95 backdrop-blur-xl text-white border border-white/10 shadow-[0_14px_40px_rgba(0,0,0,0.25)] px-4 py-3.5 md:py-3 space-y-2.5"
    >
      <div class="flex flex-wrap items-center justify-between gap-2">
        <div class="flex items-center gap-2 min-w-0">
          <span class="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full bg-[#007AFF]/25 text-blue-300 text-[11.5px] md:text-[11px] font-medium">
            <Film class="w-3.5 h-3.5 md:w-3 md:h-3" />
            回放
          </span>
          <span class="text-[12.5px] md:text-xs text-slate-300 tabular-nums">
            第 {{ currentTurn }}/{{ totalTurns }} 轮 · {{ currentIndex }}/{{ totalCount }}
          </span>
        </div>

        <div class="flex items-center gap-1.5">
          <div class="flex items-center rounded-full bg-white/10 p-0.5">
            <button
              v-for="sp in ([1, 2, 5] as const)"
              :key="sp"
              type="button"
              @click="emit('setSpeed', sp)"
              class="px-2.5 py-1 md:px-2 md:py-0.5 rounded-full text-[11.5px] md:text-[11px] font-mono font-medium transition"
              :class="speed === sp ? 'bg-white text-[#1D1D1F]' : 'text-slate-300 hover:text-white'"
            >
              {{ sp }}x
            </button>
          </div>
          <button
            type="button"
            @click="emit('exit')"
            class="w-8 h-8 md:w-auto md:h-auto md:p-1.5 flex items-center justify-center rounded-full hover:bg-white/15 text-slate-300 hover:text-white transition"
            title="退出回放"
          >
            <X class="w-4 h-4" />
          </button>
        </div>
      </div>

      <div class="flex items-center gap-3">
        <div class="flex items-center gap-1 shrink-0">
          <button
            type="button"
            @click="emit('stepPrev')"
            :disabled="currentIndex <= 0"
            class="w-9 h-9 md:w-auto md:h-auto md:p-1.5 flex items-center justify-center rounded-full hover:bg-white/15 text-slate-200 disabled:opacity-35 transition"
            title="上一条"
          >
            <SkipBack class="w-4 h-4" />
          </button>
          <button
            type="button"
            @click="emit('togglePlay')"
            class="terrarium-stay-white w-10 h-10 md:w-8 md:h-8 rounded-full bg-white text-[#1D1D1F] hover:bg-slate-200 flex items-center justify-center transition active:scale-95"
            :title="playing ? '暂停回放' : '继续回放'"
          >
            <Pause v-if="playing" class="w-4 h-4 fill-current" />
            <Play v-else class="w-4 h-4 fill-current ml-0.5" />
          </button>
          <button
            type="button"
            @click="emit('stepNext')"
            :disabled="currentIndex >= totalCount"
            class="w-9 h-9 md:w-auto md:h-auto md:p-1.5 flex items-center justify-center rounded-full hover:bg-white/15 text-slate-200 disabled:opacity-35 transition"
            title="下一条"
          >
            <SkipForward class="w-4 h-4" />
          </button>
        </div>

        <div class="flex-1 min-w-0">
          <RangeSlider
            :model-value="currentIndex"
            :min="0"
            :max="Math.max(1, totalCount)"
            :step="1"
            :dark="true"
            @update:model-value="emit('seek', $event)"
          />
        </div>
      </div>
    </div>
  </div>
</template>
