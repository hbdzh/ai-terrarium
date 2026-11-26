<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    modelValue?: boolean;
    disabled?: boolean;
  }>(),
  {
    modelValue: false,
    disabled: false,
  }
);

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
}>();

function toggle() {
  if (props.disabled) return;
  emit("update:modelValue", !props.modelValue);
}
</script>

<template>
  <button
    type="button"
    role="switch"
    :aria-checked="modelValue"
    :disabled="disabled"
    @click="toggle"
    class="relative inline-flex h-[22px] w-[40px] shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-150 ease-in-out focus-visible:outline-2 focus-visible:outline-[#7965CC] focus-visible:outline-offset-2 disabled:opacity-45 disabled:cursor-not-allowed"
    :class="modelValue ? 'bg-[#7965CC] dark:bg-[#9583E0]' : 'bg-[#D6D8DE] dark:bg-white/20'"
  >
    <span
      class="pointer-events-none inline-block h-[18px] w-[18px] transform rounded-full bg-white shadow-[0_1px_3px_rgba(0,0,0,0.22)] transition duration-150 ease-in-out"
      :class="modelValue ? 'translate-x-[18px]' : 'translate-x-0'"
    ></span>
  </button>
</template>
