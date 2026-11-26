<script setup lang="ts">
import { computed } from "vue";
import { useWindowSize } from "@vueuse/core";
import { isMobileOS } from "../../utils/platform";
import {
  DialogContent,
  DialogDescription,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from "reka-ui";
import {
  DrawerContent,
  DrawerDescription,
  DrawerOverlay,
  DrawerPortal,
  DrawerRoot,
  DrawerTitle,
} from "vaul-vue";

const props = withDefaults(
  defineProps<{
    open: boolean;
    title?: string;
    description?: string;
    maxWidthClass?: string;
    contentClass?: string;
    mobilePresentation?: "sheet" | "fullscreen";
  }>(),
  {
    title: "详情弹窗",
    description: "",
    maxWidthClass: "max-w-3xl",
    contentClass: "",
    mobilePresentation: "sheet",
  }
);

const emit = defineEmits<{
  close: [];
  "update:open": [value: boolean];
}>();

const { width: winW } = useWindowSize();
const isDesktop = computed(() => winW.value >= 768);

function handleOpenChange(val: boolean) {
  emit("update:open", val);
  if (!val) {
    emit("close");
  }
}
</script>

<template>
  <!-- Desktop / Tablet (>= 768px): Centered Dialog with Esc & Focus Trap -->
  <DialogRoot v-if="isDesktop" :open="open" @update:open="handleOpenChange">
    <DialogPortal>
      <DialogOverlay
        class="ds-modal-overlay fixed inset-0 z-50 bg-black/30 backdrop-blur-[3px]"
      />
      <DialogContent
        class="ds-modal-content fixed left-1/2 top-1/2 z-50 w-[calc(100vw-2rem-var(--safe-left,0px)-var(--safe-right,0px))] flex flex-col rounded-3xl bg-white/95 backdrop-blur-2xl shadow-[0_24px_64px_rgba(0,0,0,0.22)] border border-black/[0.08] overflow-hidden outline-none"
        :class="[
          isMobileOS
            ? 'max-h-[calc(100dvh-var(--safe-top,0px)-var(--safe-bottom,0px)-24px)]'
            : 'max-h-[88dvh]',
          props.maxWidthClass,
          props.contentClass,
        ]"
      >
        <DialogTitle class="sr-only">{{ props.title }}</DialogTitle>
        <DialogDescription class="sr-only">{{ props.description || props.title }}</DialogDescription>
        <slot name="header" />
        <slot />
        <slot name="footer" />
      </DialogContent>
    </DialogPortal>
  </DialogRoot>

  <!-- Mobile / Compact Portrait (< 768px): Fullscreen Native Page for heavy forms -->
  <DialogRoot
    v-else-if="props.mobilePresentation === 'fullscreen'"
    :open="open"
    @update:open="handleOpenChange"
  >
    <DialogPortal>
      <DialogOverlay class="fixed inset-0 z-50 bg-black/25" />
      <DialogContent
        class="fixed inset-0 z-50 h-dvh w-screen flex flex-col bg-[#F2F2F7] overflow-hidden outline-none"
        :class="props.contentClass"
      >
        <DialogTitle class="sr-only">{{ props.title }}</DialogTitle>
        <DialogDescription class="sr-only">{{ props.description || props.title }}</DialogDescription>
        <div class="shrink-0 bg-white pt-[var(--safe-top,env(safe-area-inset-top))]">
          <slot name="header" />
        </div>
        <slot />
        <div
          v-if="$slots.footer"
          class="shrink-0 bg-white pb-[var(--safe-bottom,env(safe-area-inset-bottom))]"
        >
          <slot name="footer" />
        </div>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>

  <!-- Mobile / Compact Portrait (< 768px): Bottom Swipeable Sheet for lightweight pickers -->
  <DrawerRoot v-else :open="open" @update:open="handleOpenChange">
    <DrawerPortal>
      <DrawerOverlay class="fixed inset-0 z-50 bg-black/35 backdrop-blur-[2px]" />
      <DrawerContent
        class="fixed inset-x-0 bottom-0 z-50 max-h-[86dvh] flex flex-col rounded-t-[26px] bg-white border-t border-black/[0.08] shadow-[0_-16px_48px_rgba(0,0,0,0.18)] overflow-hidden outline-none pb-[var(--safe-bottom,env(safe-area-inset-bottom))]"
        :class="props.contentClass"
      >
        <div class="w-10 h-1 rounded-full bg-slate-300/90 mx-auto mt-2.5 mb-1 shrink-0"></div>
        <DrawerTitle class="sr-only">{{ props.title }}</DrawerTitle>
        <DrawerDescription class="sr-only">{{ props.description || props.title }}</DrawerDescription>
        <slot name="header" />
        <slot />
        <slot name="footer" />
      </DrawerContent>
    </DrawerPortal>
  </DrawerRoot>
</template>
