import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { ThemePreference } from "../types";
import { isDesktopOS, isMobileOS } from "./platform";
import { setAndroidStatusBarLightIcons } from "./native";

const THEME_STORAGE_KEY = "ai-terrarium-theme";

function normalizeThemePref(raw: unknown): ThemePreference {
  if (raw === "light" || raw === "dark" || raw === "system") {
    return raw;
  }
  return "system";
}

function readStoredTheme(): ThemePreference {
  try {
    if (typeof localStorage !== "undefined") {
      return normalizeThemePref(localStorage.getItem(THEME_STORAGE_KEY));
    }
  } catch {
    // ignore storage errors
  }
  return "system";
}

const mediaQuery =
  typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia("(prefers-color-scheme: dark)")
    : null;

const systemPrefersDark = ref<boolean>(Boolean(mediaQuery?.matches));
export const themePreference = ref<ThemePreference>(readStoredTheme());
export const resolvedTheme = computed<"light" | "dark">(() => {
  if (themePreference.value === "dark") return "dark";
  if (themePreference.value === "light") return "light";
  return systemPrefersDark.value ? "dark" : "light";
});

export function syncThemeVisuals(syncNativeWindow = true): void {
  const isDark = resolvedTheme.value === "dark";
  if (typeof document !== "undefined") {
    const root = document.documentElement;
    root.classList.toggle("dark", isDark);
    root.style.colorScheme = isDark ? "dark" : "light";
  }
  if (isMobileOS) {
    setAndroidStatusBarLightIcons(!isDark);
  }
  if (isDesktopOS && syncNativeWindow) {
    invoke("set_window_theme", { theme: themePreference.value }).catch(() => {});
  }
}

export function setThemePreference(
  pref: ThemePreference | string,
  syncNativeWindow = true
): void {
  const normalized = normalizeThemePref(pref);
  themePreference.value = normalized;
  try {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(THEME_STORAGE_KEY, normalized);
    }
  } catch {
    // ignore storage errors
  }
  syncThemeVisuals(syncNativeWindow);
}

if (mediaQuery) {
  const onChange = (e: MediaQueryListEvent) => {
    systemPrefersDark.value = e.matches;
    if (themePreference.value === "system") {
      syncThemeVisuals(false);
    }
  };
  if (typeof mediaQuery.addEventListener === "function") {
    mediaQuery.addEventListener("change", onChange);
  } else if (typeof mediaQuery.addListener === "function") {
    mediaQuery.addListener(onChange);
  }
}
