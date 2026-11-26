<script setup lang="ts">
import { computed } from "vue";
import { RotateCcw, Swords } from "lucide-vue-next";
import {
  BOUNDARY_TAG_PAIRS,
  MAIN_TONE_TAGS,
  getTagSpec,
} from "../constants/experimentTags";

const props = withDefaults(
  defineProps<{
    tags: string[];
    compact?: boolean;
  }>(),
  {
    compact: false,
  }
);

const emit = defineEmits<{
  toggle: [tagId: string];
  clear: [];
}>();

const selectedSet = computed(() => new Set(props.tags));

const activeHint = computed(() => {
  if (props.tags.length === 0) {
    return "未打标签 · 全开关自由解锁";
  }
  return props.tags
    .map((id) => {
      const spec = getTagSpec(id);
      return spec ? `${spec.label}：${spec.desc}` : id;
    })
    .join(" ｜ ");
});
</script>

<template>
  <div class="space-y-2.5">
    <div class="flex items-center justify-between gap-2">
      <div class="flex items-center gap-2 min-w-0">
        <span class="text-[13px] font-semibold text-[#1D1D1F]">实验定调标签</span>
        <span
          class="px-2 py-0.5 rounded-full text-[11px] font-medium truncate"
          :class="
            tags.length === 0
              ? 'bg-emerald-50 text-emerald-700'
              : 'bg-[#007AFF]/10 text-[#0062CC]'
          "
        >
          {{ tags.length === 0 ? "未打标签 · 全开关自由解锁" : `已选 ${tags.length} 项` }}
        </span>
      </div>

      <button
        v-if="tags.length > 0"
        type="button"
        @click="emit('clear')"
        class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium text-slate-500 hover:text-[#1D1D1F] hover:bg-black/[0.05] transition shrink-0"
        title="清空所有定调标签，恢复全开关自由解锁"
      >
        <RotateCcw class="w-3 h-3" />
        <span>清空标签</span>
      </button>
    </div>

    <!-- Row 1: 8 Main Tone Tags (0 or 1, single-select) -->
    <div class="space-y-1">
      <div class="text-[11px] text-slate-400">主定调（8 选 0 或 1，再次点击取消）</div>
      <div class="flex flex-wrap gap-1.5">
        <button
          v-for="tag in MAIN_TONE_TAGS"
          :key="tag.id"
          type="button"
          @click="emit('toggle', tag.id)"
          :title="tag.desc"
          class="px-2.5 py-1 rounded-full text-[12px] font-medium border transition cursor-pointer select-none"
          :class="
            selectedSet.has(tag.id)
              ? 'bg-[#1D1D1F] border-[#1D1D1F] text-white shadow-2xs'
              : 'bg-[#F7F7FA] border-black/[0.07] text-slate-600 hover:border-black/20 hover:text-[#1D1D1F]'
          "
        >
          {{ tag.label }}
        </button>
      </div>
    </div>

    <!-- Row 2: 3 Boundary Mutually Exclusive Pairs -->
    <div class="space-y-1">
      <div class="text-[11px] text-slate-400">附加行为边界（每组可选 0 或 1 个）</div>
      <div class="flex flex-wrap items-center gap-2">
        <div
          v-for="pair in BOUNDARY_TAG_PAIRS"
          :key="pair.id"
          class="inline-flex items-center gap-1 p-0.5 rounded-full bg-[#F4F4F6] border border-black/[0.06]"
        >
          <button
            type="button"
            @click="emit('toggle', pair.left.id)"
            :title="pair.left.desc"
            class="px-2.5 py-1 rounded-full text-[11.5px] font-medium transition cursor-pointer select-none"
            :class="
              selectedSet.has(pair.left.id)
                ? 'bg-[#007AFF] text-white shadow-2xs'
                : 'text-slate-600 hover:text-[#1D1D1F] hover:bg-white/70'
            "
          >
            {{ pair.left.label }}
          </button>
          <Swords class="w-3 h-3 text-slate-400 shrink-0" />
          <button
            type="button"
            @click="emit('toggle', pair.right.id)"
            :title="pair.right.desc"
            class="px-2.5 py-1 rounded-full text-[11.5px] font-medium transition cursor-pointer select-none"
            :class="
              selectedSet.has(pair.right.id)
                ? 'bg-[#007AFF] text-white shadow-2xs'
                : 'text-slate-600 hover:text-[#1D1D1F] hover:bg-white/70'
            "
          >
            {{ pair.right.label }}
          </button>
        </div>
      </div>
    </div>

    <p v-if="!compact" class="text-[11px] leading-relaxed text-slate-400">
      {{ activeHint }}
    </p>
  </div>
</template>
