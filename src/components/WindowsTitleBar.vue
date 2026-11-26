<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { Minus, Square, Copy, X } from "lucide-vue-next";

defineProps<{
  showBrand?: boolean;
}>();

const appWindow = getCurrentWindow();
const isMaximized = ref(false);
const maxHovered = ref(false);
let unlistenResize: (() => void) | null = null;
let unlistenMaxHover: (() => void) | null = null;

async function syncMaximized() {
  try {
    isMaximized.value = await appWindow.isMaximized();
  } catch {
    // ignore outside Tauri
  }
}

onMounted(async () => {
  await syncMaximized();
  try {
    unlistenResize = await appWindow.onResized(() => {
      syncMaximized();
    });
  } catch {
    // ignore outside Tauri
  }
  try {
    unlistenMaxHover = await listen<boolean>("win-max-hover", (event) => {
      maxHovered.value = Boolean(event.payload);
    });
  } catch {
    // ignore outside Tauri
  }
});

onUnmounted(() => {
  unlistenResize?.();
  unlistenMaxHover?.();
});

function minimize() {
  appWindow.minimize().catch(() => {});
}

async function toggleMaximize() {
  try {
    await appWindow.toggleMaximize();
    await syncMaximized();
  } catch {
    // ignore
  }
}

function closeWindow() {
  appWindow.close().catch(() => {});
}
</script>

<template>
  <!-- Borderless transparent top drag strip sitting on Windows 11 Acrylic backdrop (exact 36px height matching win_chrome.rs) -->
  <div
    class="h-9 shrink-0 flex items-center justify-between bg-transparent select-none antialiased text-[12px] text-slate-600"
  >
    <!-- Left: Brand only shown here when LeftSidebar is collapsed -->
    <div
      v-if="showBrand"
      data-tauri-drag-region
      @dblclick="toggleMaximize"
      class="flex items-center gap-2 pl-3.5 pr-4 h-full shrink-0 cursor-default"
    >
      <span class="w-4 h-4 rounded-[5px] bg-[#1D1D1F] text-white text-[9px] font-bold flex items-center justify-center shrink-0">
        T
      </span>
      <span class="font-medium text-[#1D1D1F]/80 dark:text-white/85 tracking-tight">AI Terrarium</span>
    </div>

    <!-- Center: Native Draggable Region (no bottom border line!) -->
    <div
      data-tauri-drag-region
      @dblclick="toggleMaximize"
      class="flex-1 h-full"
    ></div>

    <!-- Right: Native-matched Windows Caption Buttons (exact 46px x 36px matching win_chrome.rs HTMAXBUTTON rect) -->
    <div class="flex items-center h-full shrink-0">
      <button
        type="button"
        @click="minimize"
        class="w-[46px] h-[36px] flex items-center justify-center text-slate-500 hover:bg-black/[0.06] hover:text-[#1D1D1F] transition-colors outline-none"
        aria-label="最小化"
      >
        <Minus class="w-3.5 h-3.5 stroke-[1.6]" />
      </button>
      <button
        type="button"
        @click="toggleMaximize"
        class="w-[46px] h-[36px] flex items-center justify-center transition-colors outline-none"
        :class="maxHovered ? 'bg-black/[0.06] text-[#1D1D1F]' : 'text-slate-500 hover:bg-black/[0.06] hover:text-[#1D1D1F]'"
        :aria-label="isMaximized ? '还原' : '最大化'"
      >
        <Copy v-if="isMaximized" class="w-3 h-3 stroke-[1.6] rotate-90" />
        <Square v-else class="w-3 h-3 stroke-[1.6]" />
      </button>
      <button
        type="button"
        @click="closeWindow"
        class="w-[46px] h-[36px] flex items-center justify-center text-slate-500 hover:bg-[#C42B1C] hover:text-white transition-colors outline-none"
        aria-label="关闭"
      >
        <X class="w-3.5 h-3.5 stroke-[1.75]" />
      </button>
    </div>
  </div>
</template>
