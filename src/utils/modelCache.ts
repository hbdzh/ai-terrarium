import { invoke } from "@tauri-apps/api/core";
import { reactive } from "vue";

export interface RemoteModelInfo {
  id: string;
  ownedBy: string;
}

export interface ModelCacheEntry {
  models: RemoteModelInfo[];
  loading: boolean;
  error: string | null;
  fetchedAt: number;
}

const cacheState = reactive<Record<string, ModelCacheEntry>>({});
const inflightRequests = new Map<string, Promise<RemoteModelInfo[]>>();

export function normalizeBaseUrl(url: string): string {
  return url.trim().replace(/\/+$/, "");
}

export function makeModelCacheKey(baseUrl: string, apiKey: string): string {
  const normUrl = normalizeBaseUrl(baseUrl);
  const key = apiKey.trim();
  const keyTag = key ? `${key.length}:${key.slice(0, 6)}:${key.slice(-4)}` : "nokey";
  return `${normUrl}::${keyTag}`;
}

export function getModelCacheEntry(baseUrl: string, apiKey: string): ModelCacheEntry | undefined {
  const key = makeModelCacheKey(baseUrl, apiKey);
  return cacheState[key];
}

export async function fetchProviderModels(
  baseUrl: string,
  apiKey: string,
  forceRefresh = false
): Promise<RemoteModelInfo[]> {
  const normUrl = normalizeBaseUrl(baseUrl);
  const cacheKey = makeModelCacheKey(normUrl, apiKey);
  const existing = cacheState[cacheKey];

  if (
    !forceRefresh &&
    existing &&
    existing.models.length > 0 &&
    !existing.error &&
    Date.now() - existing.fetchedAt < 5 * 60 * 1000
  ) {
    return existing.models;
  }

  const pending = inflightRequests.get(cacheKey);
  if (pending && !forceRefresh) {
    return pending;
  }

  cacheState[cacheKey] = {
    models: existing?.models ?? [],
    loading: true,
    error: null,
    fetchedAt: existing?.fetchedAt ?? 0,
  };

  const task = (async () => {
    try {
      const list = await invoke<RemoteModelInfo[]>("fetch_provider_models", {
        baseUrl: normUrl,
        apiKey: apiKey.trim(),
      });
      cacheState[cacheKey] = {
        models: list,
        loading: false,
        error: null,
        fetchedAt: Date.now(),
      };
      return list;
    } catch (err: any) {
      const message =
        typeof err === "string"
          ? err
          : err?.message ?? err?.toString?.() ?? "拉取模型列表失败";
      cacheState[cacheKey] = {
        models: existing?.models ?? [],
        loading: false,
        error: message,
        fetchedAt: existing?.fetchedAt ?? 0,
      };
      throw err;
    } finally {
      inflightRequests.delete(cacheKey);
    }
  })();

  inflightRequests.set(cacheKey, task);
  return task;
}
