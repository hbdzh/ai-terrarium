<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { Minus, Square, Copy, X, PanelLeft, SquarePen } from "lucide-vue-next";

defineProps<{
  showSidebarToggle?: boolean;
  sidebarOpen?: boolean;
  compact?: boolean;
}>();

const emit = defineEmits<{
  toggleSidebar: [];
  openNewExperiment: [];
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

function handleTitlebarDblClick(e: MouseEvent) {
  const target = e.target as HTMLElement | null;
  if (target?.closest("button, input, textarea, select, a, [role='button'], [role='menuitem']")) {
    return;
  }
  void toggleMaximize();
}

function closeWindow() {
  appWindow.close().catch(() => {});
}
</script>

<template>
  <!-- Borderless transparent top drag strip sitting on Windows 11 Mica backdrop -->
  <div
    class="shrink-0 flex items-center justify-between bg-transparent select-none antialiased text-[12px] text-slate-600 transition-[height] duration-150"
    :class="compact ? 'h-[42px]' : 'h-9'"
  >
    <!-- Left: Persistent Fixed Toggle + New Experiment (ChatGPT Desktop style) -->
    <div
      v-if="showSidebarToggle"
      class="pl-2.5 h-full shrink-0 flex items-center gap-1"
    >
      <button
        type="button"
        @click="emit('toggleSidebar')"
        class="w-7 h-7 flex items-center justify-center rounded-lg transition-all duration-150 hover:scale-[1.06] active:scale-[0.95] outline-none"
        :class="
          sidebarOpen
            ? 'text-[#1D1D1F] hover:bg-black/[0.06]'
            : 'text-slate-500 hover:text-[#1D1D1F] hover:bg-black/[0.06]'
        "
        title="展开/收起推演记录栏 (Ctrl+B)"
      >
        <PanelLeft class="w-4 h-4 stroke-[1.75]" />
      </button>
      <button
        type="button"
        @click="emit('openNewExperiment')"
        class="w-7 h-7 flex items-center justify-center rounded-lg text-slate-500 hover:text-[#1D1D1F] hover:bg-black/[0.06] hover:scale-[1.06] active:scale-[0.95] transition-all duration-150 outline-none"
        title="新建推演 (Ctrl+N)"
      >
        <SquarePen class="w-[15px] h-[15px] stroke-[1.75]" />
      </button>
    </div>

    <!-- Center: Integrated Controls or Draggable Region -->
    <div
      data-tauri-drag-region
      @dblclick="handleTitlebarDblClick"
      class="flex-1 min-w-0 h-full flex items-center"
    >
      <slot />
    </div>

    <!-- Right: Native-matched Windows Caption Buttons -->
    <div class="flex items-center h-full shrink-0">
      <button
        type="button"
        @click="minimize"
        class="w-[46px] flex items-center justify-center text-slate-500 hover:bg-black/[0.06] hover:text-[#1D1D1F] transition-colors outline-none"
        :class="compact ? 'h-[42px]' : 'h-[36px]'"
        aria-label="最小化"
      >
        <Minus class="w-3.5 h-3.5 stroke-[1.6]" />
      </button>
      <button
        type="button"
        @click="toggleMaximize"
        class="w-[46px] flex items-center justify-center transition-colors outline-none"
        :class="[
          compact ? 'h-[42px]' : 'h-[36px]',
          maxHovered ? 'bg-black/[0.06] text-[#1D1D1F]' : 'text-slate-500 hover:bg-black/[0.06] hover:text-[#1D1D1F]',
        ]"
        :aria-label="isMaximized ? '还原' : '最大化'"
      >
        <Copy v-if="isMaximized" class="w-3 h-3 stroke-[1.6] rotate-90" />
        <Square v-else class="w-3 h-3 stroke-[1.6]" />
      </button>
      <button
        type="button"
        @click="closeWindow"
        class="w-[46px] flex items-center justify-center text-slate-500 hover:bg-[#C42B1C] hover:text-white transition-colors outline-none"
        :class="compact ? 'h-[42px]' : 'h-[36px]'"
        aria-label="关闭"
      >
        <X class="w-3.5 h-3.5 stroke-[1.75]" />
      </button>
    </div>
  </div>
</template>
