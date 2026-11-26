<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  PopoverContent,
  PopoverPortal,
  PopoverRoot,
  PopoverTrigger,
} from "reka-ui";
import { BookUser, Check, Search, Sparkles, Trash2, X } from "lucide-vue-next";
import type { CharacterCard } from "../types";

const props = withDefaults(
  defineProps<{
    cards?: CharacterCard[];
    inheritMemory: boolean;
    allowDelete?: boolean;
    showCount?: boolean;
    triggerLabel?: string;
    iconOnly?: boolean;
    compact?: boolean;
  }>(),
  {
    cards: () => [],
    allowDelete: false,
    showCount: true,
    triggerLabel: "角色库",
    iconOnly: false,
    compact: false,
  }
);

const emit = defineEmits<{
  select: [card: CharacterCard];
  delete: [cardId: string];
  "update:inheritMemory": [value: boolean];
}>();

const open = ref(false);
const searchQuery = ref("");

watch(open, (isOpen) => {
  if (!isOpen) {
    searchQuery.value = "";
  }
});

const hasMemories = computed(() =>
  props.cards.some(
    (c) =>
      c.behaviorTraits.trim().length > 0 ||
      (c.memoryImprints && c.memoryImprints.length > 0)
  )
);

const filteredCards = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return props.cards;
  return props.cards.filter(
    (c) =>
      c.name.toLowerCase().includes(q) ||
      c.behaviorTraits.toLowerCase().includes(q) ||
      (c.memoryImprints ?? []).some((m) => m.toLowerCase().includes(q))
  );
});

function choose(card: CharacterCard) {
  emit("select", card);
  open.value = false;
}
</script>

<template>
  <PopoverRoot v-model:open="open">
    <PopoverTrigger as-child>
      <button
        type="button"
        class="flex items-center justify-center gap-1.5 text-[12px] font-medium bg-[#F5F5F7] hover:bg-[#EAEAEF] text-slate-700 border border-black/[0.08] transition-colors outline-none data-[state=open]:bg-[#EAEAEF] data-[state=open]:border-black/[0.15]"
        :class="[
          compact ? 'rounded-lg' : 'rounded-xl',
          iconOnly ? (compact ? 'w-8 h-8' : 'w-9 h-9') : (compact ? 'h-8 px-2.5' : 'h-9 px-3'),
        ]"
        :title="triggerLabel"
      >
        <BookUser class="w-3.5 h-3.5 text-slate-500 shrink-0" />
        <span v-if="!iconOnly">{{ triggerLabel }}<template v-if="showCount"> ({{ cards.length }})</template></span>
      </button>
    </PopoverTrigger>

    <PopoverPortal>
      <PopoverContent
        align="end"
        :side-offset="6"
        :collision-padding="12"
        class="desktop-popover pointer-events-auto z-[110] w-[288px] rounded-xl bg-white/95 backdrop-blur-2xl border border-black/[0.08] shadow-[0_14px_38px_rgba(0,0,0,0.16),0_2px_8px_rgba(0,0,0,0.06)] p-1.5 text-[#1D1D1F] outline-none select-none antialiased"
      >
        <!-- Quick Name Search -->
        <div class="relative flex items-center mb-1">
          <Search class="w-3.5 h-3.5 text-slate-400 absolute left-2.5 pointer-events-none" />
          <input
            v-model="searchQuery"
            placeholder="搜索角色名称或特质..."
            class="w-full pl-7 pr-6 py-1.5 rounded-lg bg-[#F4F4F6] focus:bg-white border border-transparent focus:border-black/[0.1] text-[12px] text-[#1D1D1F] placeholder:text-slate-400 outline-none transition"
          />
          <button
            v-if="searchQuery"
            type="button"
            @click="searchQuery = ''"
            class="absolute right-1.5 p-0.5 rounded-full text-slate-400 hover:text-slate-600"
          >
            <X class="w-3 h-3" />
          </button>
        </div>

        <button
          v-if="hasMemories"
          type="button"
          @click="emit('update:inheritMemory', !inheritMemory)"
          class="w-full flex items-center justify-between gap-2 px-2.5 py-1.5 rounded-lg hover:bg-black/[0.05] transition-colors text-left outline-none"
        >
          <span class="text-[12px] font-medium text-slate-700">附带经历记忆</span>
          <span
            class="w-4 h-4 rounded-[5px] flex items-center justify-center transition-colors shrink-0"
            :class="
              inheritMemory
                ? 'bg-[#1D1D1F] text-white'
                : 'border border-black/20 bg-white'
            "
          >
            <Check v-if="inheritMemory" class="w-3 h-3 stroke-[2.5]" />
          </span>
        </button>

        <div v-if="hasMemories" class="my-1 h-px bg-black/[0.06]"></div>

        <div
          v-if="filteredCards.length === 0"
          class="py-5 px-3 text-center text-[11.5px] text-slate-400 leading-relaxed"
        >
          {{ cards.length === 0 ? "角色库暂无存档，可在复盘报告中保存角色。" : "未找到匹配的角色" }}
        </div>

        <div v-else class="max-h-[256px] overflow-y-auto space-y-0.5 pr-0.5">
          <div
            v-for="card in filteredCards"
            :key="card.id"
            class="group flex items-center gap-1 rounded-lg hover:bg-black/[0.055] transition-colors"
          >
            <button
              type="button"
              @click="choose(card)"
              class="flex-1 min-w-0 px-2.5 py-1.5 text-left outline-none"
            >
              <div class="flex items-center gap-1.5 min-w-0">
                <span class="text-[12.5px] font-medium text-[#1D1D1F] truncate">
                  {{ card.name }}
                </span>
                <span
                  v-if="card.isBuiltin"
                  class="shrink-0 px-1.5 py-[1px] rounded bg-black/[0.05] text-[10px] font-medium text-slate-500"
                >
                  内置
                </span>
                <span
                  v-if="card.memoryImprints && card.memoryImprints.length > 0"
                  class="shrink-0 inline-flex items-center gap-0.5 px-1.5 py-[1px] rounded bg-indigo-50 text-[10px] font-medium text-indigo-600"
                  :title="card.memoryImprints.join('\n')"
                >
                  <Sparkles class="w-2.5 h-2.5" />
                  {{ card.memoryImprints.length }}
                </span>
              </div>
              <p
                v-if="card.behaviorTraits"
                class="text-[11px] text-slate-400 truncate mt-0.5"
              >
                {{ card.behaviorTraits }}
              </p>
            </button>

            <button
              v-if="allowDelete && !card.isBuiltin"
              type="button"
              @click.stop="emit('delete', card.id)"
              class="mr-1.5 p-1 rounded-md text-slate-400 opacity-0 group-hover:opacity-100 hover:text-rose-600 hover:bg-rose-50 transition shrink-0 outline-none"
              title="从角色库移除"
            >
              <Trash2 class="w-3.5 h-3.5" />
            </button>
          </div>
        </div>
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>
