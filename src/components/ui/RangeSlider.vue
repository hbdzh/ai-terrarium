<script setup lang="ts">
import { computed } from "vue";

const props = withDefaults(
  defineProps<{
    modelValue: number;
    min?: number;
    max?: number;
    step?: number;
    dark?: boolean;
  }>(),
  {
    min: 0,
    max: 100,
    step: 1,
    dark: false,
  }
);

const emit = defineEmits<{
  "update:modelValue": [value: number];
}>();

const pct = computed(() => {
  const span = props.max - props.min;
  if (span <= 0) return 0;
  const clamped = Math.max(props.min, Math.min(props.max, props.modelValue));
  return ((clamped - props.min) / span) * 100;
});

function onInput(e: Event) {
  const val = Number((e.target as HTMLInputElement).value);
  emit("update:modelValue", val);
}
</script>

<template>
  <div class="relative h-7 flex items-center select-none touch-none">
    <div
      class="w-full h-1.5 rounded-full overflow-hidden"
      :class="dark ? 'bg-white/20' : 'bg-slate-200/90'"
    >
      <div
        class="h-full rounded-full transition-[width] duration-75"
        :class="dark ? 'bg-[#0A84FF]' : 'bg-[#007AFF]'"
        :style="{ width: `${pct}%` }"
      ></div>
    </div>
    <div
      class="pointer-events-none absolute top-1/2 -translate-y-1/2 -translate-x-1/2 w-5 h-5 rounded-full bg-white border border-black/[0.08] shadow-[0_2px_6px_rgba(0,0,0,0.2)] transition-[left] duration-75"
      :style="{ left: `${pct}%` }"
    ></div>
    <input
      type="range"
      :min="min"
      :max="max"
      :step="step"
      :value="modelValue"
      @input="onInput"
      class="absolute inset-0 w-full h-full opacity-0 cursor-pointer m-0"
    />
  </div>
</template>
