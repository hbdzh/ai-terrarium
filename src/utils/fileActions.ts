import { invoke } from "@tauri-apps/api/core";
import { isHarmonyOS } from "./platform";

export interface FileFilter {
  name: string;
  extensions: string[];
}

function collectExtensions(filters?: FileFilter[]): string[] {
  if (!filters) return [];
  return filters.flatMap((f) => f.extensions).filter(Boolean);
}

/**
 * Prompt user with native save dialog and write UTF-8 text to chosen destination.
 * Returns true if saved, false if cancelled.
 */
export async function saveTextWithDialog(opts: {
  defaultPath: string;
  filters?: FileFilter[];
  content: string;
}): Promise<boolean> {
  if (isHarmonyOS) {
    return invoke<boolean>("ohos_pick_and_save_text", {
      defaultName: opts.defaultPath,
      extensions: collectExtensions(opts.filters),
      content: opts.content,
    });
  }
  const { save } = await import("@tauri-apps/plugin-dialog");
  const filePath = await save({
    defaultPath: opts.defaultPath,
    filters: opts.filters,
  });
  if (!filePath) return false;
  await invoke("save_markdown_to_file", { path: filePath, content: opts.content });
  return true;
}

/**
 * Prompt user with native save dialog and write binary bytes to chosen destination.
 * Returns true if saved, false if cancelled.
 */
export async function saveBinaryWithDialog(opts: {
  defaultPath: string;
  filters?: FileFilter[];
  data: Uint8Array | number[];
}): Promise<boolean> {
  const bytes = Array.isArray(opts.data) ? opts.data : Array.from(opts.data);
  if (isHarmonyOS) {
    return invoke<boolean>("ohos_pick_and_save_binary", {
      defaultName: opts.defaultPath,
      extensions: collectExtensions(opts.filters),
      data: bytes,
    });
  }
  const { save } = await import("@tauri-apps/plugin-dialog");
  const filePath = await save({
    defaultPath: opts.defaultPath,
    filters: opts.filters,
  });
  if (!filePath) return false;
  await invoke("save_binary_to_file", { path: filePath, data: bytes });
  return true;
}

/**
 * Prompt user with native open dialog and read selected UTF-8 text file.
 * Returns `{ name, content }` or `null` if cancelled.
 */
export async function openTextWithDialog(opts: {
  filters?: FileFilter[];
}): Promise<{ name: string; content: string } | null> {
  if (isHarmonyOS) {
    const raw = await invoke<string | null>("ohos_pick_and_open_text", {
      extensions: collectExtensions(opts.filters),
    });
    if (!raw) return null;
    const parsed = JSON.parse(raw) as { name?: string; content?: string };
    return {
      name: parsed.name || "imported",
      content: parsed.content ?? "",
    };
  }
  const { open: openDialog } = await import("@tauri-apps/plugin-dialog");
  const selected = await openDialog({
    multiple: false,
    filters: opts.filters,
  });
  if (!selected || Array.isArray(selected)) return null;
  const content = await invoke<string>("read_text_file", { path: selected });
  const name = selected.split(/[\\/]/).pop() || "imported";
  return { name, content };
}
