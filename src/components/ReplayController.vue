<script setup lang="ts">
import { Play, Pause, SkipBack, SkipForward, X, Film } from "lucide-vue-next";

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
  <div class="shrink-0 px-3 sm:px-6 md:px-10 pb-3 pt-1.5 z-20">
    <div
      class="max-w-3xl mx-auto rounded-2xl bg-[#1D1D1F]/95 backdrop-blur-xl text-white border border-white/10 shadow-[0_14px_40px_rgba(0,0,0,0.25)] px-3.5 sm:px-4 py-3 space-y-2.5"
    >
      <div class="flex flex-wrap items-center justify-between gap-2">
        <div class="flex items-center gap-2 min-w-0">
          <span class="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full bg-[#007AFF]/25 text-blue-300 text-[11px] font-medium">
            <Film class="w-3 h-3" />
            剧场回放模式
          </span>
          <span class="text-xs text-slate-300 tabular-nums">
            第 {{ currentTurn }} / {{ totalTurns }} 轮 · 消息 {{ currentIndex }} / {{ totalCount }}
          </span>
        </div>

        <div class="flex items-center gap-1.5">
          <div class="flex items-center rounded-full bg-white/10 p-0.5">
            <button
              v-for="sp in ([1, 2, 5] as const)"
              :key="sp"
              type="button"
              @click="emit('setSpeed', sp)"
              class="px-2 py-0.5 rounded-full text-[11px] font-mono font-medium transition"
              :class="speed === sp ? 'bg-white text-[#1D1D1F]' : 'text-slate-300 hover:text-white'"
            >
              {{ sp }}x
            </button>
          </div>
          <button
            type="button"
            @click="emit('exit')"
            class="p-1.5 rounded-full hover:bg-white/15 text-slate-300 hover:text-white transition"
            title="退出剧场回放"
          >
            <X class="w-4 h-4" />
          </button>
        </div>
      </div>

      <div class="flex items-center gap-3">
        <div class="flex items-center gap-1">
          <button
            type="button"
            @click="emit('stepPrev')"
            :disabled="currentIndex <= 0"
            class="p-1.5 rounded-full hover:bg-white/15 text-slate-200 disabled:opacity-35 transition"
            title="上一条"
          >
            <SkipBack class="w-4 h-4" />
          </button>
          <button
            type="button"
            @click="emit('togglePlay')"
            class="w-8 h-8 rounded-full bg-white text-[#1D1D1F] hover:bg-slate-200 flex items-center justify-center transition active:scale-95"
            :title="playing ? '暂停回放' : '继续回放'"
          >
            <Pause v-if="playing" class="w-4 h-4 fill-current" />
            <Play v-else class="w-4 h-4 fill-current ml-0.5" />
          </button>
          <button
            type="button"
            @click="emit('stepNext')"
            :disabled="currentIndex >= totalCount"
            class="p-1.5 rounded-full hover:bg-white/15 text-slate-200 disabled:opacity-35 transition"
            title="下一条"
          >
            <SkipForward class="w-4 h-4" />
          </button>
        </div>

        <input
          type="range"
          min="0"
          :max="totalCount"
          :value="currentIndex"
          @input="emit('seek', Number(($event.target as HTMLInputElement).value))"
          class="flex-1 accent-[#007AFF] cursor-pointer"
        />
      </div>
    </div>
  </div>
</template>
