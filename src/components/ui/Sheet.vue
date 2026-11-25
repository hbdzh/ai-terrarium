<script setup lang="ts">
import {
  DialogContent,
  DialogDescription,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from "reka-ui";

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
    widthClass: "w-[400px] max-w-[90vw]",
    side: "right",
  }
);

const emit = defineEmits<{
  close: [];
  "update:open": [value: boolean];
}>();

function handleOpenChange(val: boolean) {
  emit("update:open", val);
  if (!val) {
    emit("close");
  }
}
</script>

<template>
  <DialogRoot :open="open" @update:open="handleOpenChange">
    <DialogPortal>
      <DialogOverlay class="fixed inset-0 z-40 bg-black/20 backdrop-blur-[2px]" />
      <DialogContent
        class="fixed inset-y-0 z-50 h-dvh flex flex-col bg-[#FAFAFC] border-black/[0.08] outline-none overflow-hidden antialiased"
        :class="[
          props.side === 'left'
            ? 'left-0 border-r shadow-[16px_0_48px_rgba(0,0,0,0.14)]'
            : 'right-0 border-l shadow-[-16px_0_48px_rgba(0,0,0,0.14)]',
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
