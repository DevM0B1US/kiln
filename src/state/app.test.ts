/// Store behaviour tests, ported from the original `state.rs` and `mod.rs`
/// test suites. The bridge is a stub, so every path is synchronous and
/// deterministic; the debounce uses fake timers.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { KilnStore, fitEdge } from "$state/app.svelte";
import type { Bridge } from "$lib/bridge/index";
import type { BatchEvent, Fit, MatteOptionsDto, PreviewPayload, QueueEntry } from "$lib/types";

function payload(width = 640, height = 480): PreviewPayload {
  return { png: "", width, height, preview_width: width, preview_height: height };
}

class StubBridge implements Bridge {
  readonly kind = "browser" as const;
  events: BatchEvent[] = [];
  listeners: Array<(event: BatchEvent) => void> = [];
  refineCalls: Array<{ path: string; options: MatteOptionsDto }> = [];

  modelStatus() {
    return Promise.resolve({ installed: true, root: "/models", file_name: "rmbg-1.4.onnx" });
  }
  preview(_path: string) {
    return Promise.resolve(payload());
  }
  refine(path: string, options: MatteOptionsDto) {
    this.refineCalls.push({ path, options });
    return Promise.resolve(payload());
  }
  mergePreview(_photo: string, _template: string, _fit: Fit) {
    return Promise.resolve(payload());
  }
  templatePreview(_path: string) {
    return Promise.resolve(payload());
  }
  exportRemove(entries: QueueEntry[], _options: MatteOptionsDto, destination: string) {
    for (const entry of entries) {
      this.emit({ type: "batch-item", ok: true, message: `${destination}/${entry.label}` });
    }
    this.emit({ type: "batch-done", ok: entries.length, failed: 0, folder: destination });
    return Promise.resolve();
  }
  exportMerge(
    entries: QueueEntry[],
    _template: string,
    _fit: Fit,
    destination: string,
  ): Promise<void> {
    return this.exportRemove(
      entries,
      { threshold: null, feather: 0, dilate: 0, trim: false },
      destination,
    );
  }
  cancelBatch() {
    return Promise.resolve();
  }
  copyCutout(_path: string, _options: MatteOptionsDto) {
    return Promise.resolve();
  }
  copyPhoto(_path: string) {
    return Promise.resolve();
  }
  pasteImage(_dir: string) {
    return Promise.resolve("/pasted.png");
  }
  pickImages() {
    return Promise.resolve([]);
  }
  pickTemplate() {
    return Promise.resolve(null);
  }
  pickFolder() {
    return Promise.resolve(null);
  }
  registerFiles() {
    return [];
  }
  onBatchEvent(cb: (event: BatchEvent) => void) {
    this.listeners.push(cb);
  }
  private emit(event: BatchEvent) {
    this.events.push(event);
    for (const listener of this.listeners) {
      listener(event);
    }
  }
}

function makeStore(): { store: KilnStore; bridge: StubBridge } {
  const bridge = new StubBridge();
  const store = new KilnStore(bridge);
  store.boot(); // wires the batch event listener, like the app shell does
  return { store, bridge };
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("fitEdge", () => {
  it("scales down to 1280", () => {
    expect(fitEdge(4000, 3000)).toEqual([1280, 960]);
    expect(fitEdge(3000, 4000)).toEqual([960, 1280]);
  });

  it("leaves small images alone", () => {
    expect(fitEdge(1280, 720)).toEqual([1280, 720]);
    expect(fitEdge(640, 480)).toEqual([640, 480]);
  });

  it("handles degenerate sizes", () => {
    expect(fitEdge(0, 0)).toEqual([0, 0]);
    expect(fitEdge(0, 500)).toEqual([0, 500]);
  });
});

describe("queues", () => {
  it("filters non-images and skips duplicates", () => {
    const { store } = makeStore();
    store.addPaths(["/tmp/a.png"]);
    store.addPaths(["/tmp/a.png", "/tmp/b.txt", "/tmp/photo"]);
    expect(store.queue).toHaveLength(1);
  });

  it("defaults destination to the first image's parent", () => {
    const { store } = makeStore();
    store.addPaths(["/photos/one.png", "/elsewhere/two.png"]);
    expect(store.destination).toBe("/photos");
    expect(store.selected).toBe(0);
  });

  it("clear keeps the selection in range", () => {
    const { store } = makeStore();
    store.addPaths(["/tmp/a.png", "/tmp/b.png"]);
    store.select(1);
    store.clearCurrent();
    expect(store.queue).toHaveLength(1);
    expect(store.selected).toBe(0);
  });
});

describe("tools", () => {
  it("switching tools mid-batch is blocked with a status line", () => {
    const { store } = makeStore();
    store.exportingTool = "remove";
    store.setTool("merge");
    expect(store.tool).toBe("remove");
    expect(store.status).toBe("Wait for the batch to finish.");
  });

  it("mutating the queue during export is flagged", () => {
    const { store } = makeStore();
    store.exportingTool = "remove";
    store.addPaths(["/tmp/a.png"]);
    expect(store.queueChangedDuringExport).toBe(true);
  });
});

describe("refine", () => {
  it("debounces a slider burst into one dispatch", () => {
    const { store, bridge } = makeStore();
    store.addPaths(["/tmp/a.png"]);
    store.removeOpts.threshold = 128;
    store.scheduleRefine();
    store.removeOpts.feather = 2;
    store.scheduleRefine();
    expect(bridge.refineCalls).toHaveLength(0);
    vi.advanceTimersByTime(121);
    expect(bridge.refineCalls).toHaveLength(1);
    expect(bridge.refineCalls[0]?.options.threshold).toBe(128);
    expect(bridge.refineCalls[0]?.options.feather).toBe(2);
  });

  it("late refine responses are dropped after reselect", async () => {
    makeStore();
    let release!: (value: PreviewPayload) => void;
    const gate = new Promise<PreviewPayload>((resolve) => {
      release = resolve;
    });
    const bridge = new StubBridge();
    bridge.refine = (path, options) => {
      bridge.refineCalls.push({ path, options });
      return gate;
    };
    const slow = new KilnStore(bridge);
    slow.boot();
    slow.addPaths(["/tmp/a.png"]);
    slow.removeOpts.threshold = 128;
    slow.scheduleRefine();
    await vi.advanceTimersByTimeAsync(121);
    expect(bridge.refineCalls).toHaveLength(1);
    // Reselect a different path, then the stale refine resolves: dropped.
    slow.clearCurrent();
    slow.addPaths(["/tmp/other.png"]);
    release(payload(320, 240));
    await gate;
    await Promise.resolve();
    expect(slow.refinedTex).toBeNull();
  });
});

describe("batches", () => {
  it("summary counts ok and failed", async () => {
    const { store } = makeStore();
    store.addPaths(["/tmp/a.png"]);
    // Let boot()'s model status land before dispatching.
    await vi.advanceTimersByTimeAsync(0);
    store.dispatchExport();
    // The stub completes the batch synchronously: events land inside the call.
    expect(store.summary).toBe("1 ok, 0 failed · /tmp");
    expect(store.batch).toBeNull();
    expect(store.exportingTool).toBeNull();
  });

  it("queue change during export replaces the summary", () => {
    const { store } = makeStore();
    store.addPaths(["/tmp/a.png"]);
    store.destination = "/out";
    store.dispatchExport();
    // A queue change arriving before the batch ends wins the summary.
    store.exportingTool = "remove";
    store.queueChangedDuringExport = true;
    store.handleBatchEvent({ type: "batch-done", ok: 1, failed: 0, folder: "/out" });
    expect(store.summary).toBe("Queue changed during export.");
  });

  it("export without a destination asks for one", () => {
    const { store } = makeStore();
    store.addPaths(["/tmp/a.png"]);
    store.destination = null;
    store.dispatchExport();
    expect(store.status).toBe("Pick a destination folder first.");
    expect(store.exportingTool).toBeNull();
  });

  it("remove export is blocked while the model is missing", () => {
    const { store } = makeStore();
    store.modelInstalled = false;
    store.addPaths(["/tmp/a.png"]);
    store.destination = "/out";
    store.dispatchExport();
    expect(store.status).toBe("Model file is missing. Put RMBG-1.4 back in models/.");
    expect(store.exportingTool).toBeNull();
  });

  it("merge export needs a template", () => {
    const { store } = makeStore();
    store.setTool("merge");
    store.addPaths(["/tmp/a.png"]);
    store.destination = "/out";
    store.dispatchExport();
    expect(store.status).toBe("Merge needs a template first.");
    expect(store.exportingTool).toBeNull();
  });
});

describe("zoom", () => {
  it("clamps to 1x..8x", () => {
    const { store } = makeStore();
    store.setZoomTarget(100);
    expect(store.zoomTarget).toBe(8);
    store.setZoomTarget(0.1);
    expect(store.zoomTarget).toBe(1);
  });

  it("wheel zoom clamps the factor", () => {
    const { store } = makeStore();
    store.wheelZoom(-4000);
    expect(store.zoomTarget).toBeLessThanOrEqual(8);
    expect(store.zoomTarget).toBeGreaterThanOrEqual(1);
  });
});
