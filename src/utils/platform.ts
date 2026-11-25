import { platform as tauriPlatform, type Platform } from "@tauri-apps/plugin-os";

export type RuntimePlatform = Platform | "unknown";

/**
 * Resolve current OS platform via Tauri's native OS plugin (`@tauri-apps/plugin-os`).
 * Falls back to "unknown" only when running in a plain browser outside Tauri.
 */
export function getRuntimePlatform(): RuntimePlatform {
  try {
    if (typeof window !== "undefined" && "__TAURI_OS_PLUGIN_INTERNALS__" in window) {
      return tauriPlatform();
    }
  } catch {
    // Ignore when Tauri runtime internals are unavailable
  }
  return "unknown";
}

export const runtimePlatform: RuntimePlatform = getRuntimePlatform();
export const isMacOS = runtimePlatform === "macos";
export const isWindows = runtimePlatform === "windows";
export const isAndroid = runtimePlatform === "android";
export const isIOS = runtimePlatform === "ios";
export const isMobileOS = isAndroid || isIOS;
export const isDesktopOS = !isMobileOS;
