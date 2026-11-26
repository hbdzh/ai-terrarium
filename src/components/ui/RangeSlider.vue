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
      :class="dark ? 'bg-white/20' : 'bg-[#E5E5EA] dark:bg-white/15'"
    >
      <div
        class="h-full rounded-full transition-[width] duration-75"
        :class="dark ? 'bg-[#9583E0]' : 'bg-[#7965CC] dark:bg-[#9583E0]'"
        :style="{ width: `${pct}%` }"
      ></div>
    </div>
    <div
      class="pointer-events-none absolute top-1/2 -translate-y-1/2 -translate-x-1/2 w-[18px] h-[18px] rounded-full bg-white border border-black/[0.1] shadow-[0_1px_4px_rgba(0,0,0,0.22)] transition-[left] duration-75"
      :style="{ left: `${pct}%` }"
    ></div>
    <input
      type="range"
      :min="min"
      :max="max"
      :step="step"
      :value="modelValue"
      @input="onInput"
      class="absolute inset-0 w-full h-full opacity-0 cursor-pointer m-0 focus-visible:outline-none"
    />
  </div>
</template>
