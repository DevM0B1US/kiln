/// Window state and dispatch, ported from the original `KilnState` and the
/// egui event pump.
///
/// Queues hold paths, never pixels. Sequence counters tag every dispatch and
/// late responses compare and drop, so fast clicking never shows the wrong
/// image — same rule as the original. The matte and preview caches live in
/// the backend; the store only holds display payloads.

import { getBridge } from "$lib/bridge/select";
import { pluralImages } from "$lib/format";
import type { Bridge } from "$lib/bridge/index";
import type {
  Backdrop,
  BatchEvent,
  Fit,
  PreviewPayload,
  QueueEntry,
  RemoveOptions,
  ThemeChoice,
  Tool,
} from "$lib/types";
import { isImagePath, pathLabel, pathParent } from "$lib/types";

/** Refine debounce, matching the original 120ms. */
const REFINE_DEBOUNCE_MS = 120;

/** Zoom bounds, matching the original clamp. */
const ZOOM_MIN = 1;
const ZOOM_MAX = 8;

/** Longest-edge fit for previews, same maths as the Rust `fit_edge`. */
export function fitEdge(width: number, height: number): [number, number] {
  const longest = Math.max(width, height);
  if (longest <= 1280 || longest === 0) {
    return [width, height];
  }
  const scale = 1280 / longest;
  return [Math.max(1, Math.round(width * scale)), Math.max(1, Math.round(height * scale))];
}

/** Data URL for a preview payload. */
export function previewUrl(payload: PreviewPayload): string {
  return `data:image/png;base64,${payload.png}`;
}

/** One displayed texture, tagged with the path it belongs to. */
export interface Texture {
  path: string;
  url: string;
  width: number;
  height: number;
}

function matteOptions(options: RemoveOptions) {
  return {
    threshold: options.threshold,
    feather: options.feather,
    dilate: options.dilate,
    trim: options.trim,
  };
}

export class KilnStore {
  readonly bridge: Bridge;

  tool = $state<Tool>("remove");
  removeQueue = $state<QueueEntry[]>([]);
  mergeQueue = $state<QueueEntry[]>([]);
  selectedRemove = $state<number | null>(null);
  selectedMerge = $state<number | null>(null);
  removeOpts = $state<RemoveOptions>({
    threshold: null,
    feather: 0,
    dilate: 0,
    trim: false,
  });
  fit = $state<Fit>("cover");
  template = $state<string | null>(null);
  destination = $state<string | null>(null);
  status = $state<string | null>(null);
  summary = $state<string | null>(null);
  batch = $state<{ done: number; total: number } | null>(null);
  collapsed = $state(false);
  theme = $state<ThemeChoice>("dark");
  zoom = $state(1);
  zoomTarget = $state(1);
  compareHold = $state(false);
  comparePin = $state(false);
  backdrop = $state<Backdrop>("checker");
  modelInstalled = $state<boolean | null>(null);
  exportingTool = $state<Tool | null>(null);
  dims = $state<{ width: number; height: number } | null>(null);
  queueChangedDuringExport = $state(false);
  contextMenu = $state<{ x: number; y: number } | null>(null);

  originalTex = $state<Texture | null>(null);
  refinedTex = $state<Texture | null>(null);
  mergedTex = $state<Texture | null>(null);
  templateTex = $state<Texture | null>(null);

  private previewSeq = 0;
  private workSeq = 0;
  private refineTimer: ReturnType<typeof setTimeout> | null = null;
  private exportAfterFolderPick = false;
  private mergeInFlight = false;
  private templateInFlight = false;

  constructor(bridge: Bridge = getBridge()) {
    this.bridge = bridge;
  }

  // ---- derived -----------------------------------------------------------

  get queue(): QueueEntry[] {
    return this.tool === "remove" ? this.removeQueue : this.mergeQueue;
  }

  get selected(): number | null {
    return this.tool === "remove" ? this.selectedRemove : this.selectedMerge;
  }

  get current(): QueueEntry | null {
    const index = this.selected;
    return index === null ? null : (this.queue[index] ?? null);
  }

  get canSwitchTool(): boolean {
    return this.exportingTool === null;
  }

  get zoomDisplay(): number {
    return Math.min(ZOOM_MAX, Math.max(0.05, this.zoom));
  }

  // ---- boot --------------------------------------------------------------

  /** One-time boot: model banner state and the batch event stream. */
  boot() {
    this.bridge.modelStatus().then(
      (status) => {
        this.modelInstalled = status.installed;
      },
      () => {
        this.modelInstalled = false;
      },
    );
    this.bridge.onBatchEvent((event) => this.handleBatchEvent(event));
  }

  // ---- tool queues -------------------------------------------------------

  setTool(tool: Tool) {
    if (!this.canSwitchTool) {
      this.noteStatus("Wait for the batch to finish.");
      return;
    }
    if (this.tool === tool) {
      return;
    }
    this.tool = tool;
    this.status = null;
    this.summary = null;
    this.compareHold = false;
    this.comparePin = false;
    this.zoom = 1;
    this.zoomTarget = 1;
    this.clearRefineTimer();
    this.onSelectionChanged();
  }

  addPaths(paths: string[]) {
    if (paths.length === 0) {
      return;
    }
    const wasEmpty = this.queue.length === 0;
    const before = this.current?.path ?? null;
    let added = 0;
    for (const path of paths) {
      if (!isImagePath(path) || this.queue.some((entry) => entry.path === path)) {
        continue;
      }
      this.mutateQueue().push({ path, label: pathLabel(path) });
      added += 1;
    }
    if (added === 0) {
      return;
    }
    if (this.destination === null && this.queue.length > 0) {
      this.destination = pathParent(this.queue[0]!.path);
    }
    if (wasEmpty) {
      this.selectIndex(0);
    }
    const after = this.current?.path ?? null;
    if (before !== after || this.current === null) {
      this.onSelectionChanged();
    }
  }

  /** Mutating the queue during a batch flags the summary, never blocks. */
  private mutateQueue(): QueueEntry[] {
    if (this.exportingTool !== null) {
      this.queueChangedDuringExport = true;
    }
    return this.tool === "remove" ? this.removeQueue : this.mergeQueue;
  }

  selectIndex(index: number | null) {
    if (this.tool === "remove") {
      this.selectedRemove = index;
    } else {
      this.selectedMerge = index;
    }
  }

  select(index: number | null) {
    if (index === this.selected) {
      return;
    }
    this.selectIndex(index);
    this.onSelectionChanged();
  }

  clearCurrent() {
    const index = this.selected;
    if (index === null) {
      return;
    }
    this.mutateQueue().splice(index, 1);
    const len = this.queue.length;
    this.selectIndex(len === 0 ? null : Math.min(index, len - 1));
    this.previewSeq += 1;
    this.workSeq += 1;
    this.onSelectionChanged();
  }

  // ---- selection and previews ---------------------------------------------

  /** Fresh selection: reset view state, dispatch preview (+ merge). */
  onSelectionChanged() {
    this.zoom = 1;
    this.zoomTarget = 1;
    this.compareHold = false;
    this.comparePin = false;
    this.dims = null;
    this.contextMenu = null;
    this.clearRefineTimer();
    const entry = this.current;
    if (!entry) {
      this.retainTextures(null);
      return;
    }
    this.retainTextures(entry.path);
    const seq = ++this.previewSeq;
    const path = entry.path;
    this.bridge
      .preview(path)
      .then((payload) => {
        if (this.previewSeq !== seq) {
          return;
        }
        if (this.current?.path === path) {
          this.dims = { width: payload.width, height: payload.height };
        }
        this.originalTex = {
          path,
          url: previewUrl(payload),
          width: payload.preview_width,
          height: payload.preview_height,
        };
        if (this.tool === "merge") {
          this.requestMergePreview();
        }
      })
      .catch((error: unknown) => {
        if (this.previewSeq === seq) {
          this.noteStatus(errorText(error));
        }
      });
    if (this.tool === "merge") {
      this.requestMergePreview();
    }
  }

  /** Merge preview needs a template and a photo. */
  requestMergePreview() {
    if (this.tool !== "merge" || !this.template || !this.current) {
      return;
    }
    if (this.mergeInFlight) {
      return;
    }
    this.mergeInFlight = true;
    const seq = ++this.workSeq;
    const photo = this.current.path;
    const template = this.template;
    const fit = this.fit;
    this.bridge
      .mergePreview(photo, template, fit)
      .then((payload) => {
        this.mergeInFlight = false;
        if (this.workSeq !== seq) {
          return;
        }
        this.mergedTex = {
          path: photo,
          url: previewUrl(payload),
          width: payload.preview_width,
          height: payload.preview_height,
        };
      })
      .catch((error: unknown) => {
        this.mergeInFlight = false;
        if (this.workSeq === seq) {
          this.noteStatus(errorText(error));
        }
      });
  }

  requestTemplatePreview() {
    const template = this.template;
    if (!template || this.templateInFlight) {
      return;
    }
    this.templateInFlight = true;
    this.bridge
      .templatePreview(template)
      .then((payload) => {
        this.templateInFlight = false;
        this.templateTex = {
          path: template,
          url: previewUrl(payload),
          width: payload.preview_width,
          height: payload.preview_height,
        };
        this.requestMergePreview();
      })
      .catch((error: unknown) => {
        this.templateInFlight = false;
        this.noteStatus(errorText(error));
      });
  }

  async pickTemplate() {
    const picked = await this.bridge.pickTemplate();
    if (picked) {
      this.template = picked;
      this.templateTex = null;
      this.requestTemplatePreview();
    }
  }

  /** Drop every texture that is not the given path. */
  retainTextures(path: string | null) {
    const keep = (texture: Texture | null) =>
      texture !== null && path !== null && texture.path === path;
    if (!keep(this.originalTex)) {
      this.originalTex = null;
    }
    if (!keep(this.refinedTex)) {
      this.refinedTex = null;
    }
    if (!keep(this.mergedTex)) {
      this.mergedTex = null;
    }
    if (!keep(this.templateTex)) {
      this.templateTex = null;
    }
  }

  // ---- refine ------------------------------------------------------------

  /** Schedule a debounced refine after a control change. */
  scheduleRefine() {
    if (this.tool !== "remove") {
      return;
    }
    if (this.refineTimer === null) {
      this.refineTimer = setTimeout(() => {
        this.refineTimer = null;
        this.dispatchRefine();
      }, REFINE_DEBOUNCE_MS);
    }
  }

  private clearRefineTimer() {
    if (this.refineTimer !== null) {
      clearTimeout(this.refineTimer);
      this.refineTimer = null;
    }
  }

  /** Refine the cached matte; the backend segments first when needed. */
  dispatchRefine() {
    const entry = this.current;
    if (!entry || this.tool !== "remove") {
      return;
    }
    const seq = ++this.workSeq;
    const path = entry.path;
    const options = matteOptions(this.removeOpts);
    this.bridge
      .refine(path, options)
      .then((payload) => {
        if (this.workSeq !== seq || this.current?.path !== path) {
          return;
        }
        this.refinedTex = {
          path,
          url: previewUrl(payload),
          width: payload.preview_width,
          height: payload.preview_height,
        };
      })
      .catch((error: unknown) => {
        if (this.workSeq === seq) {
          this.noteStatus(errorText(error));
        }
      });
  }

  setThresholdEnabled(enabled: boolean) {
    this.removeOpts.threshold = enabled ? 128 : null;
    this.scheduleRefine();
  }

  resetOptions() {
    this.removeOpts = { threshold: null, feather: 0, dilate: 0, trim: false };
    this.scheduleRefine();
  }

  // ---- export ------------------------------------------------------------

  /** Start a batch on the active tool. */
  dispatchExport() {
    if (this.exportingTool !== null) {
      return;
    }
    if (this.destination === null) {
      this.noteStatus("Pick a destination folder first.");
      return;
    }
    const entries = [...this.queue];
    if (entries.length === 0) {
      return;
    }
    if (this.tool === "remove" && this.modelInstalled !== true) {
      this.noteStatus("Model file is missing. Put RMBG-1.4 back in models/.");
      return;
    }
    if (this.tool === "merge" && this.template === null) {
      this.noteStatus("Merge needs a template first.");
      return;
    }
    this.exportingTool = this.tool;
    this.summary = null;
    this.queueChangedDuringExport = false;
    const destination = this.destination;
    const options = matteOptions(this.removeOpts);
    const promise =
      this.tool === "remove"
        ? this.bridge.exportRemove(entries, options, destination)
        : this.bridge.exportMerge(entries, this.template ?? "", this.fit, destination);
    promise.catch((error: unknown) => {
      // Transport-level failure before any event arrived: unwind the pill.
      if (this.batch === null && this.exportingTool !== null) {
        this.exportingTool = null;
        this.noteStatus(errorText(error));
      }
    });
  }

  cancelExport() {
    void this.bridge.cancelBatch().catch((error: unknown) => this.noteStatus(errorText(error)));
  }

  /** Batch event sink; also the test seam for batch summaries. */
  handleBatchEvent(event: BatchEvent) {
    if (event.type === "batch-started") {
      this.batch = { done: 0, total: event.total };
      return;
    }
    if (event.type === "batch-item") {
      if (this.batch !== null) {
        this.batch = { done: this.batch.done + 1, total: this.batch.total };
      }
      if (!event.ok) {
        this.noteStatus(event.message);
      }
      return;
    }
    // batch-done
    this.batch = null;
    this.exportingTool = null;
    if (this.queueChangedDuringExport) {
      this.queueChangedDuringExport = false;
      this.summary = "Queue changed during export.";
    } else {
      this.summary = `${event.ok} ok, ${event.failed} failed · ${event.folder}`;
    }
  }

  /** Folder picker; `saveTo` chains straight into an export. */
  async pickFolder(saveTo = false) {
    const folder = await this.bridge.pickFolder().catch(() => null);
    if (folder !== null) {
      this.destination = folder;
    }
    if (saveTo) {
      if (folder !== null) {
        this.dispatchExport();
      }
    }
  }

  // ---- clipboard ---------------------------------------------------------

  copyCurrent() {
    const entry = this.current;
    if (!entry || this.exportingTool !== null) {
      return;
    }
    if (this.tool === "remove") {
      if (this.modelInstalled !== true) {
        this.noteStatus("Model file is missing. Put RMBG-1.4 back in models/.");
        return;
      }
      this.bridge.copyCutout(entry.path, { ...matteOptions(this.removeOpts), trim: false }).then(
        () => this.noteStatus("Copied to clipboard."),
        (error: unknown) => this.noteStatus(errorText(error)),
      );
      return;
    }
    this.bridge.copyPhoto(entry.path).then(
      () => this.noteStatus("Copied to clipboard."),
      (error: unknown) => this.noteStatus(errorText(error)),
    );
  }

  paste() {
    const dir = this.destination ?? ".";
    this.bridge.pasteImage(dir).then(
      (path) => this.addPaths([path]),
      (error: unknown) => this.noteStatus(errorText(error)),
    );
  }

  // ---- view state ----------------------------------------------------------

  toggleTheme() {
    this.theme = this.theme === "dark" ? "light" : "dark";
  }

  zoomBy(factor: number) {
    this.zoomTarget = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, this.zoomTarget * factor));
  }

  setZoomTarget(value: number) {
    this.zoomTarget = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, value));
  }

  /** Wheel zoom about the center, clamped like the original. */
  wheelZoom(scrollY: number) {
    if (scrollY === 0) {
      return;
    }
    const factor = Math.min(1.5, Math.max(0.5, 1 + scrollY * 0.002));
    this.zoomBy(factor);
  }

  doubleClickZoom() {
    this.setZoomTarget(this.zoomTarget > 1.01 ? 1 : 2);
  }

  noteStatus(message: string) {
    this.status = message;
  }

  get idleStatusLine(): string {
    return `${pluralImages(this.queue.length)} · batch bar is 20`;
  }
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** The singleton store. One window, one instance. */
export const store = new KilnStore();
