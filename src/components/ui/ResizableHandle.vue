<script setup lang="ts">
import { SplitterResizeHandle } from "reka-ui";
</script>

<template>
  <SplitterResizeHandle
    class="group relative w-1.5 shrink-0 bg-transparent hover:bg-[#007AFF]/15 active:bg-[#007AFF]/25 transition-colors flex items-center justify-center cursor-col-resize select-none"
    title="拖拽调节左右面板宽度"
  >
    <div
      class="w-px h-full bg-black/[0.06] group-hover:bg-[#007AFF]/50 group-active:bg-[#007AFF] transition-colors"
    ></div>
    <div
      class="absolute w-1 h-8 rounded-full bg-slate-300/80 group-hover:bg-[#007AFF] transition-colors"
    ></div>
  </SplitterResizeHandle>
</template>
