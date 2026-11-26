<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  PopoverContent,
  PopoverPortal,
  PopoverRoot,
  PopoverTrigger,
} from "reka-ui";
import {
  AlertCircle,
  Check,
  ChevronDown,
  Loader2,
  Plus,
  RefreshCw,
  Search,
  Sparkles,
  X,
} from "lucide-vue-next";
import {
  isKeylessFriendlyEndpoint,
  matchProviderByUrl,
} from "../../constants/providers";
import {
  fetchProviderModels,
  getModelCacheEntry,
} from "../../utils/modelCache";

const props = withDefaults(
  defineProps<{
    modelValue: string;
    baseUrl: string;
    apiKey?: string;
    placeholder?: string;
    compact?: boolean;
  }>(),
  {
    apiKey: "",
    placeholder: "选择或拉取模型",
    compact: false,
  }
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
  notify: [message: string];
}>();

const open = ref(false);
const searchQuery = ref("");

const cacheEntry = computed(() =>
  getModelCacheEntry(props.baseUrl || "", props.apiKey || "")
);

const isLoading = computed(() => Boolean(cacheEntry.value?.loading));
const fetchError = computed(() => cacheEntry.value?.error ?? null);
const liveModels = computed(() => cacheEntry.value?.models ?? []);
const matchedProvider = computed(() => matchProviderByUrl(props.baseUrl || ""));
const canAutoFetch = computed(() => {
  const url = (props.baseUrl || "").trim();
  if (!url.startsWith("http://") && !url.startsWith("https://")) return false;
  return Boolean((props.apiKey || "").trim()) || isKeylessFriendlyEndpoint(url);
});

const recommendedModels = computed(() => {
  if (!matchedProvider.value) return [];
  return matchedProvider.value.recommendedModels;
});

const filteredLiveModels = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return liveModels.value;
  return liveModels.value.filter(
    (m) =>
      m.id.toLowerCase().includes(q) ||
      m.ownedBy.toLowerCase().includes(q)
  );
});

const filteredRecommendedModels = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  const list = recommendedModels.value;
  if (!q) return list;
  return list.filter((id) => id.toLowerCase().includes(q));
});

const trimmedQuery = computed(() => searchQuery.value.trim());

const exactMatchExists = computed(() => {
  const q = trimmedQuery.value;
  if (!q) return true;
  if (liveModels.value.some((m) => m.id === q)) return true;
  if (liveModels.value.length === 0 && recommendedModels.value.includes(q)) return true;
  return false;
});

async function triggerFetch(forceRefresh = false) {
  const url = (props.baseUrl || "").trim();
  if (!url) {
    emit("notify", "请先填写接口地址");
    return;
  }
  try {
    const list = await fetchProviderModels(url, props.apiKey || "", forceRefresh);
    if (forceRefresh && list.length > 0) {
      emit("notify", `已获取 ${list.length} 个可用模型`);
    }
  } catch (err: any) {
    if (forceRefresh) {
      const msg =
        typeof err === "string"
          ? err
          : err?.message ?? err?.toString?.() ?? "拉取模型列表失败";
      emit("notify", msg);
    }
  }
}

watch(open, (isOpen) => {
  if (isOpen) {
    searchQuery.value = "";
    const entry = cacheEntry.value;
    if (canAutoFetch.value && (!entry || (entry.models.length === 0 && !entry.loading))) {
      void triggerFetch(false);
    }
  }
});

function selectModel(modelId: string) {
  emit("update:modelValue", modelId);
  open.value = false;
}
</script>

<template>
  <div class="flex items-center gap-1.5 min-w-0">
    <PopoverRoot v-model:open="open">
      <PopoverTrigger as-child>
        <button
          type="button"
          class="flex-1 min-w-0 flex items-center justify-between gap-2 border border-black/[0.08] bg-[#F5F5F7] md:bg-[#F7F7F9] hover:bg-white focus:bg-white text-left outline-none focus:border-black/20 transition data-[state=open]:bg-white data-[state=open]:border-black/25"
          :class="compact ? 'h-8 rounded-lg px-2.5 text-[12px]' : 'h-10 rounded-xl px-3 text-[13px]'"
          :title="modelValue || placeholder"
        >
          <span
            class="truncate font-mono"
            :class="modelValue ? 'text-[#1D1D1F]' : 'text-slate-400 font-sans'"
          >
            {{ modelValue || placeholder }}
          </span>
          <span class="flex items-center gap-1 shrink-0">
            <span
              v-if="liveModels.length > 0"
              class="hidden sm:inline-flex items-center px-1.5 py-[1px] rounded bg-emerald-50 text-emerald-700 text-[10px] font-medium"
            >
              {{ liveModels.length }}
            </span>
            <Loader2 v-if="isLoading" class="w-3.5 h-3.5 text-slate-400 animate-spin" />
            <ChevronDown v-else class="w-3.5 h-3.5 text-slate-400" />
          </span>
        </button>
      </PopoverTrigger>

      <PopoverPortal>
        <PopoverContent
          align="start"
          :side-offset="6"
          :collision-padding="12"
          class="desktop-popover pointer-events-auto z-[120] w-[min(350px,calc(100vw-2rem))] rounded-xl bg-white/95 backdrop-blur-2xl border border-black/[0.08] shadow-[0_14px_38px_rgba(0,0,0,0.16),0_2px_8px_rgba(0,0,0,0.06)] p-2 text-[#1D1D1F] outline-none select-none antialiased"
        >
          <!-- Search + Refresh Row -->
          <div class="flex items-center gap-1.5 mb-2">
            <div class="relative flex-1 flex items-center min-w-0">
              <Search class="w-3.5 h-3.5 text-slate-400 absolute left-2.5 pointer-events-none" />
              <input
                v-model="searchQuery"
                placeholder="搜索模型名称或输入自定义 ID..."
                class="w-full h-8 pl-7 pr-6 rounded-lg bg-[#F4F4F6] focus:bg-white border border-transparent focus:border-black/[0.1] text-[12px] font-mono text-[#1D1D1F] placeholder:font-sans placeholder:text-slate-400 outline-none transition"
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
              type="button"
              @click="triggerFetch(true)"
              :disabled="isLoading"
              class="h-8 px-2.5 flex items-center gap-1 rounded-lg text-[11.5px] font-medium bg-[#F2F2F7] hover:bg-[#E5E5EA] text-[#1D1D1F] transition shrink-0 disabled:opacity-50"
              title="重新从服务端拉取最新模型列表"
            >
              <RefreshCw class="w-3 h-3" :class="{ 'animate-spin': isLoading }" />
              <span>{{ isLoading ? "拉取中" : "刷新" }}</span>
            </button>
          </div>

          <!-- Status Banners -->
          <div
            v-if="isLoading && liveModels.length === 0"
            class="py-6 px-3 flex flex-col items-center justify-center gap-2 text-[12px] text-slate-500"
          >
            <Loader2 class="w-4 h-4 animate-spin text-slate-400" />
            <span>正在获取模型列表...</span>
          </div>

          <div
            v-else-if="fetchError"
            class="mb-2 px-2.5 py-2 rounded-lg bg-rose-50 border border-rose-200/80 text-[11.5px] text-rose-700 flex items-start gap-2"
          >
            <AlertCircle class="w-3.5 h-3.5 shrink-0 mt-0.5" />
            <div class="flex-1 min-w-0 leading-relaxed break-words">
              {{ fetchError }}
            </div>
            <button
              type="button"
              @click="triggerFetch(true)"
              class="shrink-0 font-medium underline hover:opacity-80"
            >
              重试
            </button>
          </div>

          <div
            v-else-if="!canAutoFetch && liveModels.length === 0"
            class="mb-2 px-2.5 py-1.5 rounded-lg bg-[#F5F5F7] text-[11px] text-slate-500 flex items-center justify-between gap-2"
          >
            <span>填写 API Key 后可获取模型列表</span>
            <button
              type="button"
              @click="triggerFetch(true)"
              class="shrink-0 text-[#007AFF] font-medium hover:underline"
            >
              尝试获取
            </button>
          </div>

          <!-- Scrollable List -->
          <div class="max-h-[250px] overflow-y-auto space-y-1 pr-0.5">
            <!-- Live Models from API -->
            <template v-if="liveModels.length > 0">
              <div class="px-2 py-1 flex items-center justify-between text-[10.5px] font-medium text-slate-400">
                <span class="flex items-center gap-1">
                  <span class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
                  <span>可用模型 ({{ filteredLiveModels.length }})</span>
                </span>
                <span v-if="matchedProvider">{{ matchedProvider.name }}</span>
              </div>

              <div
                v-if="filteredLiveModels.length === 0"
                class="py-4 px-2 text-center text-[11.5px] text-slate-400"
              >
                没有匹配「{{ trimmedQuery }}」的在线模型
              </div>

              <button
                v-for="m in filteredLiveModels"
                :key="m.id"
                type="button"
                @click="selectModel(m.id)"
                class="w-full flex items-center justify-between gap-2 px-2.5 py-1.5 rounded-lg text-left transition-colors outline-none"
                :class="
                  modelValue === m.id
                    ? 'bg-black/[0.065] font-medium text-[#1D1D1F]'
                    : 'hover:bg-black/[0.045] text-slate-700'
                "
              >
                <div class="min-w-0 flex-1">
                  <div class="text-[12.5px] font-mono truncate">{{ m.id }}</div>
                  <div v-if="m.ownedBy" class="text-[10px] text-slate-400 truncate">
                    {{ m.ownedBy }}
                  </div>
                </div>
                <Check
                  v-if="modelValue === m.id"
                  class="w-3.5 h-3.5 text-[#007AFF] shrink-0"
                />
              </button>
            </template>

            <!-- Preset Recommended Fallback (before live fetch or if fetch failed) -->
            <template v-else-if="!isLoading && filteredRecommendedModels.length > 0">
              <div class="px-2 py-1 flex items-center justify-between text-[10.5px] font-medium text-slate-400">
                <span class="flex items-center gap-1">
                  <Sparkles class="w-3 h-3" />
                  <span>{{ matchedProvider?.name || "预设" }} 常用最新模型</span>
                </span>
              </div>

              <button
                v-for="recId in filteredRecommendedModels"
                :key="recId"
                type="button"
                @click="selectModel(recId)"
                class="w-full flex items-center justify-between gap-2 px-2.5 py-1.5 rounded-lg text-left transition-colors outline-none"
                :class="
                  modelValue === recId
                    ? 'bg-black/[0.065] font-medium text-[#1D1D1F]'
                    : 'hover:bg-black/[0.045] text-slate-700'
                "
              >
                <span class="text-[12.5px] font-mono truncate">{{ recId }}</span>
                <Check
                  v-if="modelValue === recId"
                  class="w-3.5 h-3.5 text-[#007AFF] shrink-0"
                />
              </button>
            </template>
          </div>

          <!-- Custom Model Name Fallback Action -->
          <div
            v-if="trimmedQuery && !exactMatchExists"
            class="mt-1.5 pt-1.5 border-t border-black/[0.06]"
          >
            <button
              type="button"
              @click="selectModel(trimmedQuery)"
              class="w-full flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-[12px] font-medium text-[#007AFF] hover:bg-blue-50 transition text-left outline-none"
            >
              <Plus class="w-3.5 h-3.5 shrink-0" />
              <span class="truncate">使用自定义模型名：<code class="font-mono">{{ trimmedQuery }}</code></span>
            </button>
          </div>
        </PopoverContent>
      </PopoverPortal>
    </PopoverRoot>

    <!-- Quick External Refresh Button -->
    <button
      type="button"
      @click="triggerFetch(true)"
      :disabled="isLoading"
      class="shrink-0 flex items-center justify-center border border-black/[0.08] bg-[#F5F5F7] md:bg-[#F7F7F9] hover:bg-[#EAEAEF] text-slate-600 hover:text-[#1D1D1F] transition disabled:opacity-50 outline-none"
      :class="compact ? 'w-8 h-8 rounded-lg' : 'w-10 h-10 rounded-xl'"
      title="拉取最新模型列表"
    >
      <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': isLoading }" />
    </button>
  </div>
</template>
