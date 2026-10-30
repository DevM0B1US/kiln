/// Real bridge: Tauri commands, plugins, and window events.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { Bridge } from "$lib/bridge/index";
import type { BatchEvent, Fit, MatteOptionsDto, PreviewPayload, QueueEntry } from "$lib/types";

const IMAGE_FILTER = {
  name: "Images",
  extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif", "tiff", "tga"],
};

function paths(value: string | string[] | null): string[] {
  if (value === null) {
    return [];
  }
  return Array.isArray(value) ? value : [value];
}

export const tauriBridge: Bridge = {
  kind: "tauri",

  modelStatus() {
    return invoke("model_status");
  },

  preview(path: string) {
    return invoke<PreviewPayload>("preview", { path });
  },

  refine(path: string, options: MatteOptionsDto) {
    return invoke<PreviewPayload>("refine", { path, options });
  },

  mergePreview(photo: string, template: string, fit: Fit) {
    return invoke<PreviewPayload>("merge_preview", { photo, template, fit });
  },

  templatePreview(path: string) {
    return invoke<PreviewPayload>("template_preview", { path });
  },

  async exportRemove(entries: QueueEntry[], options: MatteOptionsDto, destination: string) {
    await invoke("export_remove", { entries, options, destination });
  },

  async exportMerge(entries: QueueEntry[], template: string, fit: Fit, destination: string) {
    await invoke("export_merge", { entries, template, fit, destination });
  },

  cancelBatch() {
    return invoke("cancel_batch").then(() => undefined);
  },

  copyCutout(path: string, options: MatteOptionsDto) {
    return invoke("copy_cutout", { path, options }).then(() => undefined);
  },

  copyPhoto(path: string) {
    return invoke("copy_photo", { path }).then(() => undefined);
  },

  pasteImage(dir: string) {
    return invoke<string>("paste_image", { dir });
  },

  async pickImages() {
    return paths(await open({ multiple: true, filters: [IMAGE_FILTER] }));
  },

  async pickTemplate() {
    return await open({
      multiple: false,
      filters: [{ name: "Template PNG", extensions: ["png"] }],
    });
  },

  async pickFolder() {
    return await open({ directory: true });
  },

  registerFiles() {
    // Native drag-drop delivers absolute paths through window events; this
    // method exists for the browser build only.
    return [];
  },

  onBatchEvent(cb: (event: BatchEvent) => void) {
    void listen<BatchEvent>("kiln://batch", (event) => cb(event.payload));
  },
};
