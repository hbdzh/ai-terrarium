import { invoke } from "@tauri-apps/api/core";
import { platform as tauriPlatform, type Platform } from "@tauri-apps/plugin-os";

export type RuntimePlatform = Platform | "openharmony" | "ohos" | "unknown";
export type HarmonyDeviceType =
  | "phone"
  | "tablet"
  | "2in1"
  | "pc"
  | "desktop"
  | "wearable"
  | "tv"
  | "car"
  | "unknown"
  | (string & {});

interface HarmonyBridgeHint {
  getDeviceType?: () => string;
}

function getRawTauriPlatform(): RuntimePlatform {
  try {
    if (typeof window !== "undefined" && "__TAURI_OS_PLUGIN_INTERNALS__" in window) {
      return (tauriPlatform() as RuntimePlatform) || "unknown";
    }
  } catch {
    // Ignore when Tauri runtime internals are unavailable
  }
  return "unknown";
}

function detectHarmonyFallback(raw: RuntimePlatform): boolean {
  if (raw === "openharmony" || raw === "ohos") return true;
  if (raw !== "unknown" && raw !== "linux") return false;
  const w = typeof window !== "undefined" ? (window as unknown as Record<string, unknown>) : null;
  const hasOhosBridge = Boolean(w?.HarmonyNative || w?.OHOSNative);
  const ua = typeof navigator !== "undefined" ? navigator.userAgent : "";
  const hasOhosUa = /OpenHarmony|HarmonyOS|ArkWeb/i.test(ua);
  const isTouchFirstLinux =
    raw === "linux" &&
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(pointer: coarse)").matches &&
    typeof navigator !== "undefined" &&
    navigator.maxTouchPoints > 0;
  return hasOhosBridge || hasOhosUa || isTouchFirstLinux;
}

function detectHarmonyDesktopFromWeb(): boolean {
  if (typeof window === "undefined") return false;
  const w = window as unknown as {
    HarmonyNative?: HarmonyBridgeHint;
    OHOSNative?: HarmonyBridgeHint;
  };
  const bridge = w.HarmonyNative ?? w.OHOSNative;
  try {
    const devType = bridge?.getDeviceType?.()?.toLowerCase();
    if (devType) {
      return devType === "2in1" || devType === "pc" || devType === "desktop";
    }
  } catch {
    // Ignore bridge lookup errors
  }
  if (typeof navigator !== "undefined" && /Phone|Tablet|Mobile/i.test(navigator.userAgent)) {
    return false;
  }
  return (
    typeof window.matchMedia === "function" &&
    window.matchMedia("(any-pointer: fine)").matches &&
    (typeof navigator === "undefined" || navigator.maxTouchPoints === 0)
  );
}

export let runtimePlatform: RuntimePlatform = (() => {
  const raw = getRawTauriPlatform();
  return detectHarmonyFallback(raw) ? "openharmony" : raw;
})();
export let harmonyDeviceType: HarmonyDeviceType | null = null;
export let isMacOS = runtimePlatform === "macos";
export let isWindows = runtimePlatform === "windows";
export let isAndroid = runtimePlatform === "android";
export let isIOS = runtimePlatform === "ios";
export let isHarmonyOS = runtimePlatform === "openharmony" || runtimePlatform === "ohos";
export let isHarmonyDesktop = isHarmonyOS && detectHarmonyDesktopFromWeb();
export let isMobileOS = isAndroid || isIOS || (isHarmonyOS && !isHarmonyDesktop);
export let isDesktopOS = !isMobileOS;

/**
 * Resolve native HarmonyOS device type via Rust (`OH_GetDeviceType` from `libdeviceinfo_ndk.z.so`).
 * Returns immediately with zero IPC overhead on Windows, macOS, Android, and iOS.
 */
export async function initPlatformDetection(): Promise<void> {
  const raw = getRawTauriPlatform();
  if (raw !== "linux" && raw !== "unknown" && raw !== "openharmony" && raw !== "ohos") {
    return;
  }
  try {
    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      const nativeDevType = await invoke<string | null>("get_ohos_device_type");
      if (nativeDevType) {
        const normalized = nativeDevType.trim().toLowerCase();
        runtimePlatform = "openharmony";
        harmonyDeviceType = normalized;
        isHarmonyOS = true;
        isHarmonyDesktop =
          normalized === "2in1" || normalized === "pc" || normalized === "desktop";
        isMobileOS = !isHarmonyDesktop;
        isDesktopOS = isHarmonyDesktop;
      }
    }
  } catch {
    // Keep fallback detection outside Tauri or if command is unavailable
  }
}
