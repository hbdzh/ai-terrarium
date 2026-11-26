<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  DialogContent,
  DialogDescription,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from "reka-ui";
import { triggerHaptic } from "../../utils/native";

const props = withDefaults(
  defineProps<{
    open: boolean;
    title?: string;
    description?: string;
    widthClass?: string;
    side?: "left" | "right";
  }>(),
  {
    title: "侧边面板",
    description: "",
    widthClass: "w-[360px] max-w-[88vw]",
    side: "right",
  }
);

const emit = defineEmits<{
  close: [];
  "update:open": [value: boolean];
}>();

const dragging = ref(false);
const dragOffsetX = ref(0);
let startX = 0;
let startY = 0;
let startTime = 0;
let axisLocked: "none" | "horizontal" | "vertical" = "none";

watch(
  () => props.open,
  (isOpen) => {
    dragOffsetX.value = 0;
    dragging.value = false;
    axisLocked = "none";
    if (isOpen) {
      triggerHaptic("light");
    }
  }
);

function handleOpenChange(val: boolean) {
  emit("update:open", val);
  if (!val) {
    emit("close");
  }
}

function onTouchStart(e: TouchEvent) {
  if (e.touches.length !== 1) return;
  const t = e.touches[0];
  startX = t.clientX;
  startY = t.clientY;
  startTime = performance.now();
  axisLocked = "none";
  dragOffsetX.value = 0;
}

function onTouchMove(e: TouchEvent) {
  if (e.touches.length !== 1) return;
  const t = e.touches[0];
  const dx = t.clientX - startX;
  const dy = t.clientY - startY;

  if (axisLocked === "none") {
    if (Math.abs(dx) < 8 && Math.abs(dy) < 8) return;
    const towardDismiss =
      (props.side === "left" && dx < 0) || (props.side === "right" && dx > 0);
    if (towardDismiss && Math.abs(dx) > Math.abs(dy) * 1.15) {
      axisLocked = "horizontal";
      dragging.value = true;
    } else {
      axisLocked = "vertical";
      return;
    }
  }

  if (axisLocked !== "horizontal") return;

  if (props.side === "left") {
    dragOffsetX.value = Math.min(0, dx);
  } else {
    dragOffsetX.value = Math.max(0, dx);
  }
}

function onTouchEnd() {
  if (axisLocked !== "horizontal") {
    dragging.value = false;
    dragOffsetX.value = 0;
    axisLocked = "none";
    return;
  }

  const elapsed = Math.max(1, performance.now() - startTime);
  const absOffset = Math.abs(dragOffsetX.value);
  const velocity = absOffset / elapsed;

  dragging.value = false;
  axisLocked = "none";

  if (absOffset > 72 || velocity > 0.38) {
    dragOffsetX.value = 0;
    triggerHaptic("light");
    handleOpenChange(false);
  } else {
    dragOffsetX.value = 0;
  }
}

const contentDragStyle = computed(() => {
  if (dragOffsetX.value === 0 && !dragging.value) return undefined;
  return {
    transform: `translate3d(${dragOffsetX.value}px, 0, 0)`,
    transition: dragging.value
      ? "none"
      : "transform 220ms cubic-bezier(0.22, 1, 0.36, 1)",
  };
});

const overlayDragStyle = computed(() => {
  if (!dragging.value || dragOffsetX.value === 0) return undefined;
  const ratio = Math.max(0.15, 1 - Math.abs(dragOffsetX.value) / 260);
  return {
    opacity: String(ratio),
    transition: "none",
  };
});
</script>

<template>
  <DialogRoot :open="open" @update:open="handleOpenChange">
    <DialogPortal>
      <DialogOverlay
        :style="overlayDragStyle"
        class="sheet-overlay fixed inset-0 z-40 bg-black/30 backdrop-blur-[1.5px]"
      />
      <DialogContent
        :style="contentDragStyle"
        @touchstart.passive="onTouchStart"
        @touchmove.passive="onTouchMove"
        @touchend.passive="onTouchEnd"
        @touchcancel.passive="onTouchEnd"
        class="fixed inset-y-0 z-50 h-dvh flex flex-col outline-none overflow-hidden antialiased pt-[var(--safe-top,env(safe-area-inset-top))] pb-[var(--safe-bottom,env(safe-area-inset-bottom))] will-change-transform"
        :class="[
          props.side === 'left'
            ? 'sheet-content-left left-0 bg-[#F7F7FA] border-r border-black/[0.06] rounded-r-[24px] shadow-[16px_0_48px_rgba(0,0,0,0.16)]'
            : 'sheet-content-right right-0 bg-white border-l border-black/[0.06] rounded-l-[24px] shadow-[-16px_0_48px_rgba(0,0,0,0.16)]',
          props.widthClass,
        ]"
      >
        <DialogTitle class="sr-only">{{ props.title }}</DialogTitle>
        <DialogDescription class="sr-only">{{ props.description || props.title }}</DialogDescription>
        <slot />
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
