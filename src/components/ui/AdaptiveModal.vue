<script setup lang="ts">
import { useMediaQuery } from "@vueuse/core";
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
  }>(),
  {
    title: "详情弹窗",
    description: "",
    maxWidthClass: "max-w-3xl",
    contentClass: "",
  }
);

const emit = defineEmits<{
  close: [];
  "update:open": [value: boolean];
}>();

const isDesktop = useMediaQuery("(min-width: 768px)");

function handleOpenChange(val: boolean) {
  emit("update:open", val);
  if (!val) {
    emit("close");
  }
}
</script>

<template>
  <!-- Desktop (>= 768px): Centered Dialog with Esc & Focus Trap -->
  <DialogRoot v-if="isDesktop" :open="open" @update:open="handleOpenChange">
    <DialogPortal>
      <DialogOverlay
        class="fixed inset-0 z-50 bg-black/30 backdrop-blur-[3px] data-[state=open]:animate-in data-[state=closed]:animate-out"
      />
      <DialogContent
        class="fixed left-1/2 top-1/2 z-50 -translate-x-1/2 -translate-y-1/2 w-[calc(100vw-2rem)] max-h-[88vh] flex flex-col rounded-3xl bg-white/95 backdrop-blur-2xl shadow-[0_24px_64px_rgba(0,0,0,0.22)] border border-black/[0.08] overflow-hidden outline-none"
        :class="[props.maxWidthClass, props.contentClass]"
      >
        <DialogTitle class="sr-only">{{ props.title }}</DialogTitle>
        <DialogDescription class="sr-only">{{ props.description || props.title }}</DialogDescription>
        <slot name="header" />
        <slot />
        <slot name="footer" />
      </DialogContent>
    </DialogPortal>
  </DialogRoot>

  <!-- Mobile (< 768px): Bottom Swipeable Drawer -->
  <DrawerRoot v-else :open="open" @update:open="handleOpenChange">
    <DrawerPortal>
      <DrawerOverlay class="fixed inset-0 z-50 bg-black/35 backdrop-blur-[2px]" />
      <DrawerContent
        class="fixed inset-x-0 bottom-0 z-50 max-h-[92dvh] flex flex-col rounded-t-3xl bg-white border-t border-black/[0.08] shadow-[0_-16px_48px_rgba(0,0,0,0.18)] overflow-hidden outline-none pb-[env(safe-area-inset-bottom)]"
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
