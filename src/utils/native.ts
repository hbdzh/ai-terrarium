import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isHarmonyOS } from "./platform";

export type HapticKind = "light" | "medium" | "confirm" | "error";

interface MobileNativeBridge {
  haptic?: (kind: string) => void;
  getInsetsJson?: () => string;
  syncInsets?: () => void;
  setStatusBarLightIcons?: (lightIcons: boolean) => void;
}

interface InsetsPayload {
  top?: number;
  right?: number;
  bottom?: number;
  left?: number;
  imeBottom?: number;
}

let ohosInsetsListening = false;

function getBridge(): MobileNativeBridge | null {
  if (typeof window === "undefined") return null;
  const w = window as unknown as {
    AndroidNative?: MobileNativeBridge;
    HarmonyNative?: MobileNativeBridge;
    OHOSNative?: MobileNativeBridge;
  };
  return w.AndroidNative ?? w.HarmonyNative ?? w.OHOSNative ?? null;
}

function applyInsetsJson(raw: string | null | undefined): void {
  if (!raw || typeof document === "undefined") return;
  try {
    const parsed = JSON.parse(raw) as InsetsPayload;
    const root = document.documentElement;
    if (typeof parsed.top === "number") root.style.setProperty("--safe-top", `${parsed.top}px`);
    if (typeof parsed.right === "number") root.style.setProperty("--safe-right", `${parsed.right}px`);
    if (typeof parsed.bottom === "number") root.style.setProperty("--safe-bottom", `${parsed.bottom}px`);
    if (typeof parsed.left === "number") root.style.setProperty("--safe-left", `${parsed.left}px`);
    if (typeof parsed.imeBottom === "number") {
      root.style.setProperty("--ime-bottom", `${parsed.imeBottom}px`);
      root.classList.toggle("ime-open", parsed.imeBottom > 1);
    }
  } catch {
    // ignore malformed insets payload
  }
}

export function triggerHaptic(kind: HapticKind = "light") {
  if (isHarmonyOS) {
    invoke("ohos_trigger_haptic", { kind }).catch(() => {});
    return;
  }
  try {
    getBridge()?.haptic?.(kind);
  } catch {
    // ignore outside mobile native bridge
  }
}

export function applyNativeInsetsFromBridge() {
  if (isHarmonyOS) {
    invoke<string | null>("ohos_get_insets")
      .then((raw) => applyInsetsJson(raw))
      .catch(() => {});
    if (!ohosInsetsListening) {
      ohosInsetsListening = true;
      void listen<string>("terrarium://native-insets", (event) => {
        applyInsetsJson(event.payload);
      });
    }
    return;
  }
  const bridge = getBridge();
  if (!bridge) return;
  try {
    applyInsetsJson(bridge.getInsetsJson?.());
    bridge.syncInsets?.();
  } catch {
    // ignore
  }
}

export function setNativeStatusBarLightIcons(lightIcons: boolean) {
  if (isHarmonyOS) {
    invoke("ohos_set_status_bar_light_icons", { lightIcons }).catch(() => {});
    return;
  }
  try {
    getBridge()?.setStatusBarLightIcons?.(lightIcons);
  } catch {
    // ignore
  }
}
