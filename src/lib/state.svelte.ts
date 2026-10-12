import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

export type Section = "remove" | "merge";

export interface MatteOpts {
  threshold_on: boolean;
  threshold: number;
  feather: number;
  dilate: number;
  trim: boolean;
}

export interface Batch {
  busy: boolean;
  index: number;
  total: number;
  ok: number;
  failed: number;
  progress: number;
  startedAt: number;
  elapsedMs: number;
}

export const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "bmp", "gif", "tiff", "tga"];
export const DEFAULT_THRESHOLD = 128;
export const DEBOUNCE_MS = 120;

export const ZOOM_MIN = 5;
export const ZOOM_MAX = 3200;
export const ZOOM_FIT = 50;
export const ZOOM_ACTUAL = 100;
const BASE_PREVIEW = 640;
const PANEL_WIDTH = 264;
const SIDEBAR_WIDTH = 208;
const RAIL_WIDTH = 64;
const STAGE_MARGIN = 32;
const TITLE_HEIGHT = 48;
const DOCK_HEIGHT = 96;

export const shell = $state({
  darkMode: true as boolean,
  section: "remove" as Section,
  zoom: ZOOM_FIT as number,
  panX: 0 as number,
  panY: 0 as number,
  sidebarCollapsed: false as boolean,
  dropActive: false as boolean,
  ctrlHeld: false as boolean,
  contextMenu: false as boolean,
  backdrop: "checker" as "checker" | "dark" | "light",
});

export const queue = $state({
  paths: [] as string[],
  selected: null as number | null,
  sourcePath: null as string | null,
  outputDir: null as string | null,
});

interface StoredQueue {
  paths: string[];
  selected: number | null;
  sourcePath: string | null;
  outputDir: string | null;
}

const stored: Record<Section, StoredQueue> = {
  remove: { paths: [], selected: null, sourcePath: null, outputDir: null },
  merge: { paths: [], selected: null, sourcePath: null, outputDir: null },
};

function stashQueue(section: Section) {
  stored[section] = {
    paths: [...queue.paths],
    selected: queue.selected,
    sourcePath: queue.sourcePath,
    outputDir: queue.outputDir,
  };
}

function restoreQueue(section: Section) {
  const saved = stored[section];
  queue.paths = [...saved.paths];
  queue.selected = saved.selected;
  queue.sourcePath = saved.sourcePath;
  queue.outputDir = saved.outputDir;
}

function freshBatch(): Batch {
  return { busy: false, index: 0, total: 0, ok: 0, failed: 0, progress: 0, startedAt: 0, elapsedMs: 0 };
}

export const remove = $state({
  modelReady: false as boolean,
  modelMessage: "" as string,
  imageWidth: 0 as number,
  imageHeight: 0 as number,
  original: null as string | null,
  refined: null as string | null,
  showOriginal: false as boolean,
  thresholdOn: false as boolean,
  threshold: DEFAULT_THRESHOLD as number,
  feather: 0 as number,
  dilate: 0 as number,
  trim: false as boolean,
  previewSeq: 0 as number,
  workSeq: 0 as number,
  matteBusy: false as boolean,
  refineBusy: false as boolean,
  pendingRefine: false as boolean,
  matteKey: null as string | null,
  cancelled: false as boolean,
  batch: freshBatch() as Batch,
  status: "" as string,
});

export const merge = $state({
  templatePath: null as string | null,
  templateWidth: 0 as number,
  templateHeight: 0 as number,
  original: null as string | null,
  merged: null as string | null,
  showOriginal: false as boolean,
  mergeFit: "Cover" as string,
  seq: 0 as number,
  mergeBusy: false as boolean,
  cancelled: false as boolean,
  batch: freshBatch() as Batch,
  status: "" as string,
});

let refineTimer: ReturnType<typeof setTimeout> | null = null;
let creepTimer: ReturnType<typeof setInterval> | null = null;

export function fileName(path: string): string {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return cut >= 0 ? path.slice(cut + 1) : path;
}

export function folderName(path: string): string {
  return fileName(path);
}

export function currentName(): string {
  if (queue.selected == null) return "";
  const path = queue.paths[queue.selected];
  return path ? fileName(path) : "";
}

export function megapixels(width: number, height: number): string {
  if (width === 0 || height === 0) return "-";
  return `${((width * height) / 1_000_000).toFixed(1)} MP`;
}

export function elapsedText(ms: number): string {
  return `${Math.floor(ms / 1000)}s`;
}

export function isImage(path: string): boolean {
  const lower = path.toLowerCase();
  return IMAGE_EXTENSIONS.some((ext) => lower.endsWith(`.${ext}`));
}

export function fitZoom(imageWidth: number, imageHeight: number): number {
  if (imageWidth === 0 || imageHeight === 0) return ZOOM_FIT;
  const sidebar = shell.sidebarCollapsed ? RAIL_WIDTH : SIDEBAR_WIDTH;
  const stageWidth = window.innerWidth - sidebar - PANEL_WIDTH - 2 - STAGE_MARGIN;
  const stageHeight = window.innerHeight - TITLE_HEIGHT - DOCK_HEIGHT - STAGE_MARGIN;
  const byWidth = (stageWidth / BASE_PREVIEW) * 100;
  const byHeight = ((stageHeight / BASE_PREVIEW) * imageWidth * 100) / imageHeight;
  return Math.min(byWidth, byHeight);
}

export function clampZoom(value: number): number {
  if (Number.isNaN(value)) return ZOOM_FIT;
  return Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, value));
}

function matteOpts(): MatteOpts {
  return {
    threshold_on: remove.thresholdOn,
    threshold: remove.threshold,
    feather: remove.feather,
    dilate: remove.dilate,
    trim: remove.trim,
  };
}

function startCreep(batch: Batch) {
  stopCreep();
  creepTimer = setInterval(() => {
    batch.elapsedMs = Date.now() - batch.startedAt;
    if (batch.busy) {
      const total = Math.max(1, batch.total);
      const cap = (batch.index + 0.75) / total;
      if (batch.progress < cap) batch.progress = Math.min(cap, batch.progress + 0.008);
    }
  }, 100);
}

function stopCreep() {
  if (creepTimer != null) {
    clearInterval(creepTimer);
    creepTimer = null;
  }
}

function batchSummary(batch: Batch, outputDir: string | null): string {
  const folder = outputDir ? ` · ${folderName(outputDir)}` : "";
  return `${batch.ok} ok, ${batch.failed} failed${folder}`;
}

export async function refreshModel(): Promise<void> {
  try {
    const status = await invoke<{ installed: boolean; message: string }>("model_status");
    remove.modelReady = status.installed;
    remove.modelMessage = status.message;
  } catch (error) {
    remove.modelReady = false;
    remove.modelMessage = String(error);
  }
}

export function batchesBusy(): boolean {
  return remove.batch.busy || merge.batch.busy;
}

function activeStatus(message: string) {
  if (shell.section === "remove") remove.status = message;
  else merge.status = message;
}

export function previewWarm(): boolean {
  if (shell.section === "remove") return remove.original != null;
  return merge.original != null || merge.merged != null;
}

export async function previewAt(index: number): Promise<void> {
  queue.selected = index;
  const photo = queue.paths[index];
  if (!photo) return;
  if (queue.sourcePath === photo && previewWarm()) return;
  shell.panX = 0;
  shell.panY = 0;
  const old = queue.sourcePath;
  queue.sourcePath = photo;
  remove.matteKey = null;
  remove.pendingRefine = false;
  remove.matteBusy = false;
  remove.refineBusy = false;
  remove.workSeq += 1;
  merge.seq += 1;
  if (shell.section === "remove" && old && old !== photo)
    void invoke("drop_matte", { path: old }).catch(() => undefined);
  if (shell.section === "remove") {
    void fetchRemovePreview(photo);
  } else {
    void startMergePreview(index);
  }
}

async function fetchRemovePreview(photo: string): Promise<void> {
  remove.original = null;
  remove.refined = null;
  remove.imageWidth = 0;
  remove.imageHeight = 0;
  remove.previewSeq += 1;
  const seq = remove.previewSeq;
  remove.status = "Loading preview…";
  try {
    const loaded = await invoke<{ width: number; height: number; original_url: string }>("remove_preview", {
      path: photo,
    });
    if (seq !== remove.previewSeq) return;
    remove.imageWidth = loaded.width;
    remove.imageHeight = loaded.height;
    remove.original = loaded.original_url;
    shell.zoom = fitZoom(loaded.width, loaded.height);
    remove.status = remove.matteKey === photo ? "" : "Move a slider to preview the cutout.";
  } catch (error) {
    if (seq !== remove.previewSeq) return;
    remove.status = String(error);
  }
}

async function loadMatte(photo: string): Promise<void> {
  if (remove.matteBusy || remove.batch.busy) return;
  if (!remove.modelReady) {
    remove.status = "Model file is missing. Put it back in models/.";
    return;
  }
  remove.matteBusy = true;
  remove.workSeq += 1;
  const work = remove.workSeq;
  remove.status = "Working on cutout…";
  try {
    const out = await invoke<{
      width: number;
      height: number;
      original_url: string;
      refined_url: string;
    }>("remove_matte", { path: photo });
    if (work !== remove.workSeq) return;
    remove.imageWidth = out.width;
    remove.imageHeight = out.height;
    remove.original = out.original_url;
    remove.refined = out.refined_url;
    shell.zoom = fitZoom(out.width, out.height);
    remove.matteBusy = false;
    remove.matteKey = photo;
    remove.status = "";
    if (remove.pendingRefine) {
      remove.pendingRefine = false;
      void runRefine();
    }
  } catch (error) {
    if (work !== remove.workSeq) return;
    remove.matteBusy = false;
    remove.status = String(error);
  }
}

async function startMergePreview(index: number): Promise<void> {
  const photo = queue.paths[index];
  if (!photo) return;
  if (!merge.templatePath) {
    merge.merged = null;
    merge.status = "Pick a template first.";
    return;
  }
  merge.original = null;
  merge.merged = null;
  merge.mergeBusy = true;
  merge.seq += 1;
  const seq = merge.seq;
  merge.status = "Merging preview…";
  try {
    const loaded = await invoke<{
      width: number;
      height: number;
      original_url: string;
      merged_url: string;
    }>("merge_preview", { photo, template: merge.templatePath, fit: merge.mergeFit });
    if (seq !== merge.seq) return;
    merge.mergeBusy = false;
    merge.templateWidth = loaded.width;
    merge.templateHeight = loaded.height;
    merge.original = loaded.original_url;
    merge.merged = loaded.merged_url;
    shell.zoom = fitZoom(loaded.width, loaded.height);
    merge.status = "";
  } catch (error) {
    if (seq !== merge.seq) return;
    merge.mergeBusy = false;
    merge.status = String(error);
  }
}

export function addAndPreview(paths: string[]): Promise<void> | null {
  shell.contextMenu = false;
  let firstNew: number | null = null;
  for (const path of paths) {
    if (!isImage(path)) continue;
    if (!queue.paths.includes(path)) {
      queue.paths.push(path);
      if (firstNew == null) firstNew = queue.paths.length - 1;
    }
  }
  if (firstNew == null) return null;
  return previewAt(firstNew);
}

export async function browseFiles(): Promise<void> {
  const picked = await open({ multiple: true, filters: [{ name: "Images", extensions: [...IMAGE_EXTENSIONS] }] });
  if (!picked) return;
  const paths = (Array.isArray(picked) ? picked : [picked]).filter((entry) => typeof entry === "string") as string[];
  await addAndPreview(paths);
}

export async function pickOutputDir(): Promise<boolean> {
  const picked = await open({ directory: true });
  if (typeof picked === "string") {
    queue.outputDir = picked;
    return true;
  }
  return false;
}

export async function pickTemplate(): Promise<void> {
  const picked = await open({ multiple: false, filters: [{ name: "Images", extensions: [...IMAGE_EXTENSIONS] }] });
  if (typeof picked === "string") {
    merge.templatePath = picked;
    if (queue.selected != null) await startMergePreview(queue.selected);
  }
}

export function clearQueue() {
  const old = queue.sourcePath;
  queue.paths = [];
  queue.selected = null;
  queue.sourcePath = null;
  shell.panX = 0;
  shell.panY = 0;
  remove.original = null;
  remove.refined = null;
  remove.imageWidth = 0;
  remove.imageHeight = 0;
  remove.matteKey = null;
  remove.pendingRefine = false;
  remove.matteBusy = false;
  remove.refineBusy = false;
  remove.workSeq += 1;
  remove.status = "";
  merge.seq += 1;
  merge.original = null;
  merge.merged = null;
  merge.templateWidth = 0;
  merge.templateHeight = 0;
  merge.status = "";
  if (shell.section === "remove" && old) void invoke("drop_matte", { path: old }).catch(() => undefined);
}

export function removeCurrent() {
  const index = queue.selected;
  if (index == null || !queue.paths[index]) return;
  shell.contextMenu = false;
  const [gone] = queue.paths.splice(index, 1);
  if (shell.section === "remove" && gone) void invoke("drop_matte", { path: gone }).catch(() => undefined);
  if (queue.paths.length === 0) {
    clearQueue();
    return;
  }
  queue.sourcePath = null;
  remove.matteKey = null;
  remove.pendingRefine = false;
  remove.matteBusy = false;
  remove.refineBusy = false;
  remove.workSeq += 1;
  merge.seq += 1;
  void previewAt(Math.min(index, queue.paths.length - 1));
}

export function selectTool(section: Section) {
  if (batchesBusy()) {
    activeStatus("Wait for the batch to finish.");
    return;
  }
  if (shell.section !== section) {
    stashQueue(shell.section);
    shell.section = section;
    restoreQueue(section);
  }
  if (queue.selected != null) void previewAt(queue.selected);
}

export function requestRefine() {
  if (refineTimer != null) clearTimeout(refineTimer);
  refineTimer = setTimeout(() => {
    refineTimer = null;
    void runRefine();
  }, DEBOUNCE_MS);
}

async function runRefine(): Promise<void> {
  const photo = queue.sourcePath;
  if (!photo || remove.batch.busy) return;
  if (!remove.modelReady) {
    remove.status = "Model file is missing. Put it back in models/.";
    return;
  }
  if (remove.matteKey !== photo) {
    remove.pendingRefine = true;
    void loadMatte(photo);
    return;
  }
  remove.workSeq += 1;
  const seq = remove.workSeq;
  remove.refineBusy = true;
  try {
    const out = await invoke<{ width: number; height: number; refined_url: string }>("refine_preview", {
      path: photo,
      options: matteOpts(),
    });
    if (seq !== remove.workSeq) return;
    remove.refineBusy = false;
    remove.refined = out.refined_url;
    remove.status = "";
  } catch (error) {
    if (seq !== remove.workSeq) return;
    remove.refineBusy = false;
    if (String(error).includes("file changed on disk")) remove.matteKey = null;
    remove.status = String(error);
  }
}

export function setThreshold(value: number) {
  if (remove.batch.busy) return;
  remove.threshold = Math.round(Math.min(255, Math.max(0, value)));
  remove.thresholdOn = true;
  requestRefine();
}

export function typeThreshold(text: string) {
  if (remove.batch.busy) return;
  const value = Number.parseFloat(text.trim());
  if (Number.isNaN(value)) return;
  setThreshold(value);
}

export function setFeather(value: number) {
  if (remove.batch.busy) return;
  remove.feather = Math.min(10, Math.max(0, value));
  requestRefine();
}

export function typeFeather(text: string) {
  if (remove.batch.busy) return;
  const value = Number.parseFloat(text.trim());
  if (Number.isNaN(value)) return;
  setFeather(value);
}

export function setDilate(value: number) {
  if (remove.batch.busy) return;
  remove.dilate = Math.round(Math.min(10, Math.max(0, value)));
  requestRefine();
}

export function typeDilate(text: string) {
  if (remove.batch.busy) return;
  const value = Number.parseFloat(text.trim());
  if (Number.isNaN(value)) return;
  setDilate(value);
}

export function toggleTrim(value: boolean) {
  if (remove.batch.busy) return;
  remove.trim = value;
  requestRefine();
}

export function resetOpts() {
  if (remove.batch.busy) return;
  remove.thresholdOn = false;
  remove.threshold = DEFAULT_THRESHOLD;
  remove.feather = 0;
  remove.dilate = 0;
  remove.trim = false;
  remove.pendingRefine = false;
  remove.matteBusy = false;
  remove.refineBusy = false;
  remove.workSeq += 1;
  remove.refined = null;
  const photo = queue.sourcePath;
  remove.status = remove.matteKey === photo && photo != null ? "" : "Move a slider to preview the cutout.";
}

export function setMergeFit(fit: string) {
  merge.mergeFit = fit;
  if (queue.selected != null) void startMergePreview(queue.selected);
}

function ensureOutput(): string | null {
  if (queue.paths.length === 0) return null;
  if (!queue.outputDir) {
    const first = queue.paths[0];
    const cut = Math.max(first.lastIndexOf("/"), first.lastIndexOf("\\"));
    queue.outputDir = cut >= 0 ? first.slice(0, cut) : null;
  }
  return queue.outputDir;
}

export async function beginRemove(): Promise<void> {
  if (queue.paths.length === 0 || remove.batch.busy || merge.batch.busy) return;
  if (!remove.modelReady) {
    remove.status = "Model file is missing. Put it back in models/.";
    return;
  }
  const output = ensureOutput();
  if (!output) {
    remove.status = "Pick a folder first.";
    return;
  }
  await invoke("begin_batch").catch(() => undefined);
  void runRefine();
  const batch = remove.batch;
  batch.busy = true;
  batch.index = 0;
  batch.total = queue.paths.length;
  batch.ok = 0;
  batch.failed = 0;
  batch.progress = 0;
  batch.startedAt = Date.now();
  batch.elapsedMs = 0;
  startCreep(batch);
  const snapshot = [...queue.paths];
  const options = matteOpts();
  let lastError = "";
  for (let index = 0; index < snapshot.length; index += 1) {
    if (queue.paths[index] !== snapshot[index]) {
      finishRemove("Queue changed during export.");
      return;
    }
    batch.index = index;
    remove.status = `Removing ${index + 1}/${batch.total}`;
    try {
      await invoke("export_remove", { path: snapshot[index], outputDir: output, options });
      batch.ok += 1;
    } catch (error) {
      batch.failed += 1;
      lastError = String(error);
      remove.status = lastError;
    }
    batch.index = index + 1;
    batch.progress = batch.index / Math.max(1, batch.total);
    if (remove.cancelled) {
      finishRemove("Cancel takes effect after this image.");
      return;
    }
  }
  finishRemove(batch.failed > 0 ? `${batchSummary(batch, output)} — ${lastError}` : batchSummary(batch, output));
}

export async function beginMerge(): Promise<void> {
  if (queue.paths.length === 0 || remove.batch.busy || merge.batch.busy) return;
  if (!merge.templatePath) {
    merge.status = "Pick a template first.";
    return;
  }
  const output = ensureOutput();
  if (!output) {
    merge.status = "Pick a folder first.";
    return;
  }
  await invoke("begin_batch").catch(() => undefined);
  const batch = merge.batch;
  batch.busy = true;
  batch.index = 0;
  batch.total = queue.paths.length;
  batch.ok = 0;
  batch.failed = 0;
  batch.progress = 0;
  batch.startedAt = Date.now();
  batch.elapsedMs = 0;
  startCreep(batch);
  const snapshot = [...queue.paths];
  const template = merge.templatePath;
  const fit = merge.mergeFit;
  let lastError = "";
  for (let index = 0; index < snapshot.length; index += 1) {
    if (queue.paths[index] !== snapshot[index]) {
      finishMerge("Queue changed during export.");
      return;
    }
    batch.index = index;
    merge.status = `Merging ${index + 1}/${batch.total}`;
    try {
      await invoke("export_merge", { photo: snapshot[index], template, fit, outputDir: output });
      batch.ok += 1;
    } catch (error) {
      batch.failed += 1;
      lastError = String(error);
      merge.status = lastError;
    }
    batch.index = index + 1;
    batch.progress = batch.index / Math.max(1, batch.total);
    if (merge.cancelled) {
      finishMerge("Cancel takes effect after this image.");
      return;
    }
  }
  finishMerge(batch.failed > 0 ? `${batchSummary(batch, output)} — ${lastError}` : batchSummary(batch, output));
}

function finishRemove(message: string) {
  remove.batch.busy = false;
  remove.batch.progress = 1;
  remove.matteBusy = false;
  remove.refineBusy = false;
  remove.cancelled = false;
  remove.status = message;
  stopCreep();
}

function finishMerge(message: string) {
  merge.batch.busy = false;
  merge.batch.progress = 1;
  merge.mergeBusy = false;
  merge.cancelled = false;
  merge.status = message;
  stopCreep();
}

export async function saveRemoveTo(): Promise<void> {
  if (queue.paths.length === 0 || remove.batch.busy || merge.batch.busy) return;
  const picked = await pickOutputDir();
  if (!picked) return;
  await beginRemove();
}

export async function cancelRemove(): Promise<void> {
  remove.cancelled = true;
  await invoke("cancel_batch").catch(() => undefined);
  remove.status = "Cancel takes effect after this image.";
}

export async function cancelMerge(): Promise<void> {
  merge.cancelled = true;
  await invoke("cancel_batch").catch(() => undefined);
  merge.status = "Cancel takes effect after this image.";
}

export async function copyCurrent(): Promise<void> {
  try {
    if (shell.section === "remove") {
      const photo = queue.sourcePath;
      if (!photo) return;
      if (remove.matteKey !== photo) {
        remove.status = "Move a slider to preview the cutout first.";
        return;
      }
      remove.status = await invoke<string>("copy_remove", { path: photo, options: matteOpts() });
    } else if (shell.section === "merge") {
      const photo = queue.sourcePath;
      if (!photo) return;
      merge.status = await invoke<string>("copy_photo", { photo });
    }
  } catch (error) {
    activeStatus(String(error));
  }
}

export async function pasteClipboard(): Promise<void> {
  const dir = queue.outputDir ?? queue.sourcePath?.slice(0, Math.max(queue.sourcePath.lastIndexOf("/"), queue.sourcePath.lastIndexOf("\\"))) ?? null;
  if (!dir) {
    activeStatus("Pick a folder first.");
    return;
  }
  try {
    const out = await invoke<{ output: string }>("paste_image", { outputDir: dir });
    await addAndPreview([out.output]);
  } catch (error) {
    activeStatus(String(error));
  }
}
