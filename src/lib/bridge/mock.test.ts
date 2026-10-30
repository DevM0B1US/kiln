/// Browser-bridge contract tests that run without a canvas: everything the
/// store can hit in a plain node test environment.

import { describe, expect, it, vi } from "vitest";
import { BrowserBridge } from "$lib/bridge/mock";

describe("BrowserBridge", () => {
  it("reports the model as not installed with a calm banner", async () => {
    const bridge = new BrowserBridge();
    const status = await bridge.modelStatus();
    expect(status.installed).toBe(false);
    expect(status.file_name).toBe("rmbg-1.4.onnx");
  });

  it("exports into the virtual downloads folder", async () => {
    const bridge = new BrowserBridge();
    expect(await bridge.pickFolder()).toBe("browser://downloads");
  });

  it("rejects clipboard access with an explanatory error", async () => {
    const bridge = new BrowserBridge();
    await expect(bridge.pasteImage(".")).rejects.toThrow(/browser preview/);
    await expect(bridge.copyPhoto("mock://0/x.png")).rejects.toThrow(/browser preview/);
  });

  it("delivers batch events to listeners", async () => {
    const bridge = new BrowserBridge();
    const events: string[] = [];
    bridge.onBatchEvent((event) => events.push(event.type));
    await bridge.exportRemove(
      [{ path: "mock://missing.png", label: "missing.png" }],
      { threshold: null, feather: 0, dilate: 0, trim: false },
      "browser://downloads",
    );
    // One failed item + one done event; no canvas in node, so the item fails.
    expect(events).toEqual(["batch-item", "batch-done"]);
  });

  it("cancelBatch unwinds the pill with a done event", async () => {
    const bridge = new BrowserBridge();
    const listener = vi.fn();
    bridge.onBatchEvent(listener);
    await bridge.cancelBatch();
    expect(listener).toHaveBeenCalledWith(expect.objectContaining({ type: "batch-done" }));
  });
});
