import { invoke } from "@tauri-apps/api/core";
import { isHarmonyOS } from "./platform";

/**
 * Writes text to the system clipboard using native HarmonyOS NDK / Tauri clipboard-manager,
 * falling back to the browser Clipboard API only when running outside Tauri.
 */
export async function writeClipboardText(text: string): Promise<void> {
  if (isHarmonyOS) {
    await invoke("ohos_write_clipboard", { text });
    return;
  }
  try {
    const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
    await writeText(text);
  } catch {
    await navigator.clipboard.writeText(text);
  }
}

export async function readClipboardText(): Promise<string> {
  if (isHarmonyOS) {
    return invoke<string>("ohos_read_clipboard");
  }
  try {
    const { readText } = await import("@tauri-apps/plugin-clipboard-manager");
    return (await readText()) ?? "";
  } catch {
    return (await navigator.clipboard.readText()) ?? "";
  }
}
