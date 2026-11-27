import type { ApiProfile, ExperimentConfig, NodeConfig } from "../types";
import { DEFAULT_BASE_URL, DEFAULT_MODEL_NAME, matchProviderByUrl } from "../constants/providers";
import { normalizeBaseUrl } from "./modelCache";

export function normalizeThinkingLevel(raw: unknown): string {
  const s = typeof raw === "string" ? raw.trim().toLowerCase() : "";
  return s === "off" || s === "low" || s === "medium" || s === "high" || s === "default"
    ? s
    : "low";
}

export function cloneNormalizedConfig(src: ExperimentConfig): ExperimentConfig {
  const cloned: ExperimentConfig = JSON.parse(JSON.stringify(src));
  cloned.globalSystemPrompt = cloned.globalSystemPrompt ?? "";
  cloned.tokenBudget = cloned.tokenBudget ?? 50000;
  cloned.contextWindowTokens = cloned.contextWindowTokens ?? 32000;
  cloned.compactionThresholdPct = cloned.compactionThresholdPct ?? 80;
  cloned.boardApprovalMode = cloned.boardApprovalMode ?? "auto";
  cloned.enableBoardTools = cloned.enableBoardTools ?? true;
  cloned.initialBoard = cloned.initialBoard ?? "";
  cloned.enableCompactionGuard = cloned.enableCompactionGuard ?? true;
  cloned.enablePrivateNotes = cloned.enablePrivateNotes ?? false;
  cloned.enableSmartSelector = cloned.enableSmartSelector ?? true;
  cloned.enableAutoDirector = cloned.enableAutoDirector ?? false;
  cloned.enableNarrativeArc = cloned.enableNarrativeArc ?? false;
  cloned.tags = Array.isArray(cloned.tags) ? [...cloned.tags] : [];
  cloned.targetTotalTurns = cloned.targetTotalTurns ?? 30;
  cloned.allowPassTurn = cloned.allowPassTurn ?? false;
  cloned.randomMoodDice = cloned.randomMoodDice ?? false;
  cloned.intuitiveFastReply = cloned.intuitiveFastReply ?? false;
  cloned.agents = (cloned.agents ?? []).map((a) => ({
    ...a,
    thinkingLevel: normalizeThinkingLevel(a.thinkingLevel),
  }));
  if (cloned.nodeC) {
    cloned.nodeC.thinkingLevel = normalizeThinkingLevel(cloned.nodeC.thinkingLevel);
  }
  return cloned;
}

export function applyApiProfileToNode<T extends NodeConfig>(node: T, profile: ApiProfile): T {
  node.apiProfileId = profile.id;
  node.baseUrl = profile.baseUrl;
  node.apiKey = profile.apiKey;
  node.modelName = profile.modelName;
  return node;
}

export function resolveNodeApiProfileId(
  node: Pick<NodeConfig, "apiProfileId" | "baseUrl" | "apiKey" | "modelName">,
  profiles: ApiProfile[]
): string {
  if (profiles.length === 0) return "";
  if (node.apiProfileId && profiles.some((p) => p.id === node.apiProfileId)) {
    return node.apiProfileId;
  }
  const nUrl = normalizeBaseUrl(node.baseUrl || "");
  const nKey = (node.apiKey || "").trim();
  const nModel = (node.modelName || "").trim();
  const exact = profiles.find(
    (p) =>
      normalizeBaseUrl(p.baseUrl) === nUrl &&
      p.apiKey.trim() === nKey &&
      p.modelName.trim() === nModel
  );
  if (exact) return exact.id;
  const byCreds = profiles.find(
    (p) => normalizeBaseUrl(p.baseUrl) === nUrl && p.apiKey.trim() === nKey && nKey.length > 0
  );
  if (byCreds) return byCreds.id;
  const byUrl = profiles.find((p) => normalizeBaseUrl(p.baseUrl) === nUrl);
  if (byUrl) return byUrl.id;
  return profiles[0].id;
}

export function syncConfigWithApiProfiles(
  config: ExperimentConfig,
  profiles: ApiProfile[]
): ExperimentConfig {
  if (profiles.length === 0) return config;
  const cloned = cloneNormalizedConfig(config);
  const byId = new Map(profiles.map((p) => [p.id, p]));
  const syncNode = (node: NodeConfig) => {
    const resolvedId = resolveNodeApiProfileId(node, profiles);
    const prof = byId.get(resolvedId) ?? profiles[0];
    if (prof) {
      applyApiProfileToNode(node, prof);
    }
  };
  cloned.agents.forEach(syncNode);
  if (cloned.nodeC) {
    syncNode(cloned.nodeC);
  }
  return cloned;
}

export function seedApiProfilesFromConfig(config: ExperimentConfig | null): ApiProfile[] {
  const seen = new Map<string, ApiProfile>();
  const nodes: NodeConfig[] = config
    ? [...(config.agents ?? []), ...(config.nodeC ? [config.nodeC] : [])]
    : [];
  for (const n of nodes) {
    const baseUrl = (n.baseUrl || "").trim() || DEFAULT_BASE_URL;
    const apiKey = (n.apiKey || "").trim();
    const modelName = (n.modelName || "").trim() || DEFAULT_MODEL_NAME;
    const prov = matchProviderByUrl(baseUrl);
    const id = prov ? prov.id : "custom";
    const existing = seen.get(id);
    if (!existing || (!existing.apiKey && apiKey)) {
      seen.set(id, {
        id,
        name: prov ? prov.name : "自定义",
        baseUrl,
        apiKey,
        modelName,
      });
    }
  }
  const list = Array.from(seen.values());
  if (list.length === 0) {
    return [
      {
        id: "deepseek",
        name: "DeepSeek",
        baseUrl: DEFAULT_BASE_URL,
        apiKey: "",
        modelName: DEFAULT_MODEL_NAME,
      },
    ];
  }
  return list;
}
