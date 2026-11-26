<script setup lang="ts">
import { SplitterResizeHandle } from "reka-ui";
</script>

<template>
  <SplitterResizeHandle
    class="group relative w-[6px] ml-0 -mr-[5px] shrink-0 bg-transparent transition-colors flex items-center justify-start cursor-col-resize select-none z-10"
    title="拖拽调节左右面板宽度"
  >
    <div
      class="w-px h-full bg-black/[0.06] group-hover:w-[2px] group-hover:bg-[#007AFF]/60 group-active:bg-[#007AFF] transition-all"
    ></div>
    <div
      class="opacity-0 group-hover:opacity-100 absolute left-0 -translate-x-1/2 w-1 h-8 rounded-full bg-[#007AFF] shadow-xs transition-opacity"
    ></div>
  </SplitterResizeHandle>
</template>
