import { ref } from "vue";

export type TitlebarLayoutMode = "standard" | "compact";

const STORAGE_KEY = "ai-terrarium-titlebar-layout";

function readStoredLayout(): TitlebarLayoutMode {
  try {
    if (typeof localStorage !== "undefined") {
      const val = localStorage.getItem(STORAGE_KEY);
      if (val === "standard" || val === "compact") {
        return val;
      }
    }
  } catch {
    // ignore storage errors
  }
  return "compact";
}

export const titlebarLayoutMode = ref<TitlebarLayoutMode>(readStoredLayout());

export function setTitlebarLayoutMode(mode: TitlebarLayoutMode): void {
  titlebarLayoutMode.value = mode;
  try {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(STORAGE_KEY, mode);
    }
  } catch {
    // ignore storage errors
  }
}
