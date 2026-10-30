/// Types shared between the bridge implementations and the store.
///
/// The DTO shapes mirror the Rust command layer one-to-one; the batch event
/// tags mirror the serde `kebab-case` tagging on the Rust side.

/** Which tool owns the window. */
export type Tool = "remove" | "merge";

/** How a photo is sized under the template. */
export type Fit = "cover" | "fit" | "stretch";

/** Which palette the window uses. Dark is the default. */
export type ThemeChoice = "dark" | "light";

/** Stage backdrop. The checker forgives both halo kinds. */
export type Backdrop = "checker" | "dark" | "white";

/** One entry in a tool queue: a path and its display name. */
export interface QueueEntry {
  path: string;
  label: string;
}

/** Refine controls for the remove tool. Threshold starts off. */
export interface RemoveOptions {
  threshold: number | null;
  feather: number;
  dilate: number;
  trim: boolean;
}

/** Refine options as the backend expects them. Same shape, explicit. */
export interface MatteOptionsDto {
  threshold: number | null;
  feather: number;
  dilate: number;
  trim: boolean;
}

/** PNG preview payload: base64 plus source and preview dimensions. */
export interface PreviewPayload {
  png: string;
  width: number;
  height: number;
  preview_width: number;
  preview_height: number;
}

/** Where the model files live and whether the file is there. */
export interface ModelStatus {
  installed: boolean;
  root: string;
  file_name: string;
}

/** Progress events for the working pill, mirroring the Rust batch events. */
export type BatchEvent =
  | { type: "batch-started"; total: number }
  | { type: "batch-item"; ok: boolean; message: string }
  | { type: "batch-done"; ok: number; failed: number; folder: string };

/** Extensions accepted by drag-drop and Browse, lowercase without dot. */
export const INPUT_EXTENSIONS = [
  "png",
  "jpg",
  "jpeg",
  "webp",
  "bmp",
  "gif",
  "tiff",
  "tga",
] as const;

/** Whether a path looks like an image we can decode. */
export function isImagePath(path: string): boolean {
  const dot = path.lastIndexOf(".");
  if (dot < 0 || dot === path.length - 1) {
    return false;
  }
  const ext = path.slice(dot + 1).toLowerCase();
  return (INPUT_EXTENSIONS as readonly string[]).includes(ext);
}

/** File name part of a path, for queue labels and the title bar. */
export function pathLabel(path: string): string {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return cut >= 0 ? path.slice(cut + 1) : path;
}

/** Parent directory of a path, or "." when there is none. */
export function pathParent(path: string): string {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return cut > 0 ? path.slice(0, cut) : ".";
}
