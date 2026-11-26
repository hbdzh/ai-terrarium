<script setup lang="ts">
import { computed } from "vue";
import { ChevronDown, Check } from "lucide-vue-next";
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuTrigger,
} from "reka-ui";

export interface SelectOption {
  value: string;
  label: string;
}

const props = withDefaults(
  defineProps<{
    modelValue: string;
    options: SelectOption[];
    placeholder?: string;
    triggerClass?: string;
  }>(),
  {
    placeholder: "请选择",
    triggerClass: "",
  }
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

const selectedLabel = computed(() => {
  const found = props.options.find((o) => o.value === props.modelValue);
  return found ? found.label : props.placeholder;
});
</script>

<template>
  <DropdownMenuRoot>
    <DropdownMenuTrigger as-child>
      <button
        type="button"
        class="w-full h-10 rounded-[9px] border border-[#E5E5EA] bg-[#F5F5F7] hover:bg-[#EFEFF2] data-[state=open]:bg-white data-[state=open]:border-[#007AFF] px-3 text-[13px] text-[#25262A] flex items-center justify-between gap-2 transition outline-none text-left"
        :class="triggerClass"
      >
        <span class="truncate">{{ selectedLabel }}</span>
        <ChevronDown class="w-4 h-4 text-[#858A95] shrink-0" />
      </button>
    </DropdownMenuTrigger>
    <DropdownMenuPortal>
      <DropdownMenuContent
        align="start"
        :side-offset="6"
        class="animate-popover z-[220] min-w-[var(--reka-dropdown-menu-trigger-width)] max-w-[320px] max-h-64 overflow-y-auto rounded-2xl bg-white/95 backdrop-blur-xl border border-black/[0.08] shadow-[0_14px_36px_rgba(0,0,0,0.14)] p-1.5 space-y-0.5 text-[12.5px] text-[#1D1D1F] outline-none select-none antialiased"
      >
        <DropdownMenuItem
          v-for="opt in options"
          :key="opt.value"
          @select="emit('update:modelValue', opt.value)"
          class="flex items-center justify-between gap-2 px-3 py-2 rounded-xl cursor-pointer outline-none transition-colors hover:bg-black/[0.05] data-[highlighted]:bg-black/[0.05]"
          :class="opt.value === modelValue ? 'font-medium text-[#1D1D1F]' : 'text-slate-700'"
        >
          <span class="truncate">{{ opt.label }}</span>
          <Check
            v-if="opt.value === modelValue"
            class="w-3.5 h-3.5 text-[#007AFF] shrink-0"
          />
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
</template>
