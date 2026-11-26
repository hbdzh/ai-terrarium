<script setup lang="ts">
import { ref } from "vue";
import { SplitterPanel } from "reka-ui";

const props = defineProps<{
  defaultSize?: number;
  minSize?: number;
  maxSize?: number;
  collapsible?: boolean;
  collapsedSize?: number;
}>();

const emit = defineEmits<{
  collapse: [];
  expand: [];
}>();

const panelRef = ref<InstanceType<typeof SplitterPanel> | null>(null);

defineExpose({
  collapse: () => panelRef.value?.collapse(),
  expand: () => panelRef.value?.expand(),
  resize: (size: number) => panelRef.value?.resize(size),
  get isCollapsed() {
    return panelRef.value?.isCollapsed ?? false;
  },
});
</script>

<template>
  <SplitterPanel
    ref="panelRef"
    :default-size="props.defaultSize"
    :min-size="props.minSize"
    :max-size="props.maxSize"
    :collapsible="props.collapsible"
    :collapsed-size="props.collapsedSize"
    class="min-w-0 min-h-0 flex flex-col overflow-hidden"
    @collapse="emit('collapse')"
    @expand="emit('expand')"
  >
    <slot />
  </SplitterPanel>
</template>
