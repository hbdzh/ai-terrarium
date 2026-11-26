/**
 * Writes text to the system clipboard using Tauri's native clipboard-manager plugin,
 * falling back to the browser Clipboard API only when running outside Tauri.
 */
export async function writeClipboardText(text: string): Promise<void> {
  try {
    const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
    await writeText(text);
  } catch {
    await navigator.clipboard.writeText(text);
  }
}

export async function readClipboardText(): Promise<string> {
  try {
    const { readText } = await import("@tauri-apps/plugin-clipboard-manager");
    return (await readText()) ?? "";
  } catch {
    return (await navigator.clipboard.readText()) ?? "";
  }
}
