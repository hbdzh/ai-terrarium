<script setup lang="ts">
import { SplitterGroup } from "reka-ui";

const props = withDefaults(
  defineProps<{
    direction?: "horizontal" | "vertical";
    autoSaveId?: string;
  }>(),
  {
    direction: "horizontal",
  }
);

const emit = defineEmits<{
  layout: [sizes: number[]];
}>();
</script>

<template>
  <SplitterGroup
    :direction="props.direction"
    :auto-save-id="props.autoSaveId"
    class="flex h-full w-full min-h-0 min-w-0"
    @layout="emit('layout', $event)"
  >
    <slot />
  </SplitterGroup>
</template>
