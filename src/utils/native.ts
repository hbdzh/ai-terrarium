export type HapticKind = "light" | "medium" | "confirm" | "error";

interface AndroidNativeInterface {
  haptic?: (kind: string) => void;
  getInsetsJson?: () => string;
  syncInsets?: () => void;
  setStatusBarLightIcons?: (lightIcons: boolean) => void;
}

function getBridge(): AndroidNativeInterface | null {
  if (typeof window === "undefined") return null;
  const w = window as unknown as { AndroidNative?: AndroidNativeInterface };
  return w.AndroidNative ?? null;
}

export function triggerHaptic(kind: HapticKind = "light") {
  try {
    getBridge()?.haptic?.(kind);
  } catch {
    // ignore outside Android
  }
}

export function applyAndroidInsetsFromBridge() {
  const bridge = getBridge();
  if (!bridge) return;
  try {
    const raw = bridge.getInsetsJson?.();
    if (raw) {
      const parsed = JSON.parse(raw) as {
        top?: number;
        right?: number;
        bottom?: number;
        left?: number;
        imeBottom?: number;
      };
      const root = document.documentElement;
      if (typeof parsed.top === "number") root.style.setProperty("--safe-top", `${parsed.top}px`);
      if (typeof parsed.right === "number") root.style.setProperty("--safe-right", `${parsed.right}px`);
      if (typeof parsed.bottom === "number") root.style.setProperty("--safe-bottom", `${parsed.bottom}px`);
      if (typeof parsed.left === "number") root.style.setProperty("--safe-left", `${parsed.left}px`);
      if (typeof parsed.imeBottom === "number") {
        root.style.setProperty("--ime-bottom", `${parsed.imeBottom}px`);
        root.classList.toggle("ime-open", parsed.imeBottom > 1);
      }
    }
    bridge.syncInsets?.();
  } catch {
    // ignore
  }
}

export function setAndroidStatusBarLightIcons(lightIcons: boolean) {
  try {
    getBridge()?.setStatusBarLightIcons?.(lightIcons);
  } catch {
    // ignore
  }
}
