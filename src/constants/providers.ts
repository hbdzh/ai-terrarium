export interface ProviderPreset {
  id: string;
  name: string;
  url: string;
  model: string;
  recommendedModels: string[];
}

export const DEFAULT_BASE_URL = "https://api.deepseek.com";
export const DEFAULT_MODEL_NAME = "deepseek-flash";

export const PROVIDER_PRESETS: ProviderPreset[] = [
  {
    id: "deepseek",
    name: "DeepSeek",
    url: "https://api.deepseek.com",
    model: "deepseek-flash",
    recommendedModels: ["deepseek-flash", "deepseek-v4-pro"],
  },
  {
    id: "dashscope",
    name: "阿里云百炼 (Qwen)",
    url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    model: "qwen3.6-plus",
    recommendedModels: ["qwen3.6-plus", "qwen3.8-max", "qwen3.8-flash"],
  },
  {
    id: "moonshot",
    name: "月之暗面 (Kimi)",
    url: "https://api.moonshot.cn/v1",
    model: "kimi-k3.1-pro",
    recommendedModels: ["kimi-k3.1-pro", "kimi-k3", "kimi-k2.5"],
  },
  {
    id: "volces",
    name: "火山引擎 (豆包)",
    url: "https://ark.cn-beijing.volces.com/api/v3",
    model: "doubao-seed-2-1-pro-260915",
    recommendedModels: [
      "doubao-seed-2-1-pro-260915",
      "doubao-seed-2-1-lite-260915",
      "doubao-seed-evolving",
    ],
  },
  {
    id: "openai",
    name: "OpenAI",
    url: "https://api.openai.com/v1",
    model: "gpt-6.2-sol",
    recommendedModels: ["gpt-6.2-sol", "gpt-6.2-astra", "gpt-6-luna"],
  },
  {
    id: "gemini",
    name: "Google Gemini",
    url: "https://generativelanguage.googleapis.com/v1beta/openai",
    model: "gemini-4-hydrogen",
    recommendedModels: [
      "gemini-4-hydrogen",
      "gemini-4-neon",
      "gemini-4-carbon",
      "gemini-3.8-flash",
    ],
  },
  {
    id: "zhipu",
    name: "智谱清言 (GLM)",
    url: "https://open.bigmodel.cn/api/paas/v4",
    model: "glm-5.3",
    recommendedModels: ["glm-5.3", "glm-5.3-flash", "glm-5.2"],
  },
  {
    id: "minimax",
    name: "MiniMax",
    url: "https://api.minimaxi.com/v1",
    model: "MiniMax-M3.1-Flash",
    recommendedModels: ["MiniMax-M3.1-Flash", "MiniMax-M3"],
  },
  {
    id: "openrouter",
    name: "OpenRouter",
    url: "https://openrouter.ai/api/v1",
    model: "openai/gpt-6.2-sol",
    recommendedModels: [
      "openai/gpt-6.2-sol",
      "anthropic/claude-sonnet-5-5",
      "google/gemini-4-hydrogen",
      "deepseek/deepseek-flash",
    ],
  },
];

export function matchProviderByUrl(baseUrl: string): ProviderPreset | undefined {
  const lower = baseUrl.trim().toLowerCase();
  if (!lower) return undefined;
  return PROVIDER_PRESETS.find((p) => {
    try {
      const host = new URL(p.url).hostname;
      return lower.includes(host);
    } catch {
      return false;
    }
  });
}

export function isKeylessFriendlyEndpoint(baseUrl: string): boolean {
  const lower = baseUrl.trim().toLowerCase();
  return (
    lower.includes("localhost") ||
    lower.includes("127.0.0.1") ||
    lower.includes("openrouter.ai")
  );
}
