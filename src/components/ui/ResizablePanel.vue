<script setup lang="ts">
import { SplitterPanel } from "reka-ui";

const props = defineProps<{
  defaultSize?: number;
  minSize?: number;
  maxSize?: number;
  collapsible?: boolean;
  collapsedSize?: number;
}>();
</script>

<template>
  <SplitterPanel
    :default-size="props.defaultSize"
    :min-size="props.minSize"
    :max-size="props.maxSize"
    :collapsible="props.collapsible"
    :collapsed-size="props.collapsedSize"
    class="min-w-0 min-h-0 flex flex-col"
  >
    <slot />
  </SplitterPanel>
</template>
