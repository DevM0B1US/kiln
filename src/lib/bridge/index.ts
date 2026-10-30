/// The bridge is the ONLY place that talks to Tauri.
///
/// The store consumes this interface; the browser build injects the mock so
/// the whole UI runs and is testable in a plain tab without the desktop
/// shell. This mirrors the core rule on the Rust side, where `inference.rs`
/// was the only ONNX touchpoint.

import type {
  BatchEvent,
  Fit,
  MatteOptionsDto,
  ModelStatus,
  PreviewPayload,
  QueueEntry,
} from "$lib/types";

export interface Bridge {
  /** Which implementation is live. */
  readonly kind: "tauri" | "browser";

  modelStatus(): Promise<ModelStatus>;
  preview(path: string): Promise<PreviewPayload>;
  refine(path: string, options: MatteOptionsDto): Promise<PreviewPayload>;
  mergePreview(photo: string, template: string, fit: Fit): Promise<PreviewPayload>;
  templatePreview(path: string): Promise<PreviewPayload>;
  exportRemove(entries: QueueEntry[], options: MatteOptionsDto, destination: string): Promise<void>;
  exportMerge(
    entries: QueueEntry[],
    template: string,
    fit: Fit,
    destination: string,
  ): Promise<void>;
  cancelBatch(): Promise<void>;
  copyCutout(path: string, options: MatteOptionsDto): Promise<void>;
  copyPhoto(path: string): Promise<void>;
  pasteImage(dir: string): Promise<string>;
  pickImages(): Promise<string[]>;
  pickTemplate(): Promise<string | null>;
  pickFolder(): Promise<string | null>;
  /** Register dropped files with the backend; browser build only. */
  registerFiles(files: File[]): string[];
  onBatchEvent(cb: (event: BatchEvent) => void): void;
}
