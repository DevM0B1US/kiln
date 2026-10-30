/// Browser bridge: the whole UI runs in a plain tab against canvas mocks.
///
/// The fake pipeline is deterministic and cheap: "segmentation" turns
/// luminance into alpha (dark backdrop drops out), refine applies the same
/// passes in spirit. It exists so the frontend can be developed, screenshotted,
/// and unit-tested without the desktop shell or the model weights.

import type { Bridge } from "$lib/bridge/index";
import type { BatchEvent, Fit, MatteOptionsDto, PreviewPayload, QueueEntry } from "$lib/types";

const PREVIEW_EDGE = 1280;

/** Longest-edge fit, same maths as the Rust `fit_edge`. */
function fitEdge(width: number, height: number): [number, number] {
  const longest = Math.max(width, height);
  if (longest <= PREVIEW_EDGE || longest === 0) {
    return [width, height];
  }
  const scale = PREVIEW_EDGE / longest;
  return [Math.max(1, Math.round(width * scale)), Math.max(1, Math.round(height * scale))];
}

function toPng(canvas: HTMLCanvasElement): PreviewPayload {
  return {
    png: canvas.toDataURL("image/png").split(",")[1] ?? "",
    width: 0,
    height: 0,
    preview_width: canvas.width,
    preview_height: canvas.height,
  };
}

async function loadImage(source: Blob): Promise<ImageBitmap> {
  return await createImageBitmap(source);
}

/** Luminance-to-alpha matte: a stand-in for RMBG on product-style shots. */
function fakeMatte(bitmap: ImageBitmap, size: [number, number]): ImageData {
  const [width, height] = size;
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    throw new Error("canvas unavailable in the browser preview");
  }
  ctx.drawImage(bitmap, 0, 0, width, height);
  const frame = ctx.getImageData(0, 0, width, height);
  const pixels = frame.data;
  for (let i = 0; i < pixels.length; i += 4) {
    const lum = 0.2126 * pixels[i]! + 0.7152 * pixels[i + 1]! + 0.0722 * pixels[i + 2]!;
    // Dark surroundings go transparent, bright subjects stay, soft ramp
    // between — enough to exercise every refine control.
    const alpha = Math.max(0, Math.min(255, Math.round((lum - 48) * 2.2)));
    pixels[i + 3] = alpha;
  }
  return frame;
}

function drawImageData(frame: ImageData): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = frame.width;
  canvas.height = frame.height;
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    throw new Error("canvas unavailable in the browser preview");
  }
  ctx.putImageData(frame, 0, 0);
  return canvas;
}

export class BrowserBridge implements Bridge {
  readonly kind = "browser" as const;

  private files = new Map<string, Blob>();
  private nextId = 0;
  private listeners: Array<(event: BatchEvent) => void> = [];

  /** Register picked/dropped files under mock paths, as the pickers do. */
  registerFiles(files: File[]): string[] {
    return files.map((file) => {
      const id = this.nextId;
      this.nextId += 1;
      const path = `mock://${id}/${file.name}`;
      this.files.set(path, file);
      return path;
    });
  }

  async modelStatus() {
    // Calm-banner path stays reachable in the browser.
    return { installed: false, root: "(browser preview)", file_name: "rmbg-1.4.onnx" };
  }

  async preview(path: string): Promise<PreviewPayload> {
    const file = this.files.get(path);
    if (!file) {
      throw new Error(`mock: no file for ${path}`);
    }
    const bitmap = await loadImage(file);
    const [width, height] = fitEdge(bitmap.width, bitmap.height);
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext("2d");
    if (!ctx) {
      throw new Error("canvas unavailable in the browser preview");
    }
    ctx.drawImage(bitmap, 0, 0, width, height);
    const payload = toPng(canvas);
    payload.width = bitmap.width;
    payload.height = bitmap.height;
    bitmap.close();
    return payload;
  }

  async refine(path: string, options: MatteOptionsDto): Promise<PreviewPayload> {
    const file = this.files.get(path);
    if (!file) {
      throw new Error(`mock: no file for ${path}`);
    }
    const bitmap = await loadImage(file);
    const [width, height] = fitEdge(bitmap.width, bitmap.height);
    const frame = fakeMatte(bitmap, [width, height]);
    bitmap.close();

    // Feather: canvas blur on the alpha; threshold: hard step; dilate:
    // slight upscale of the matte. Order mirrors the core (feather, then
    // threshold, then dilate) even though each pass is approximate.
    const pixels = frame.data;
    for (let i = 0; i < pixels.length; i += 4) {
      let alpha = pixels[i + 3]!;
      if (options.threshold !== null) {
        alpha = alpha >= options.threshold ? 255 : 0;
      }
      if (options.dilate > 0 && alpha > 64) {
        alpha = 255;
      }
      pixels[i + 3] = alpha;
    }
    let canvas = drawImageData(frame);
    if (options.feather > 0) {
      const soft = document.createElement("canvas");
      soft.width = width;
      soft.height = height;
      const ctx = soft.getContext("2d");
      if (ctx) {
        ctx.filter = `blur(${options.feather}px)`;
        ctx.drawImage(canvas, 0, 0);
      }
      canvas = soft;
    }
    await new Promise((resolve) => setTimeout(resolve, 40));
    return toPng(canvas);
  }

  async mergePreview(photo: string, template: string, fit: Fit): Promise<PreviewPayload> {
    const photoFile = this.files.get(photo);
    const templateFile = this.files.get(template);
    if (!photoFile || !templateFile) {
      throw new Error("mock: photo or template missing");
    }
    const photoBitmap = await loadImage(photoFile);
    const templateBitmap = await loadImage(templateFile);
    const canvas = document.createElement("canvas");
    canvas.width = templateBitmap.width;
    canvas.height = templateBitmap.height;
    const ctx = canvas.getContext("2d");
    if (!ctx) {
      throw new Error("canvas unavailable in the browser preview");
    }
    const [tw, th] = [canvas.width, canvas.height];
    const scale =
      fit === "stretch"
        ? [tw / photoBitmap.width, th / photoBitmap.height]
        : fit === "fit"
          ? [
              Math.min(tw / photoBitmap.width, th / photoBitmap.height),
              Math.min(tw / photoBitmap.width, th / photoBitmap.height),
            ]
          : [
              Math.max(tw / photoBitmap.width, th / photoBitmap.height),
              Math.max(tw / photoBitmap.width, th / photoBitmap.height),
            ];
    const drawWidth = photoBitmap.width * scale[0]!;
    const drawHeight = photoBitmap.height * scale[1]!;
    ctx.drawImage(photoBitmap, (tw - drawWidth) / 2, (th - drawHeight) / 2, drawWidth, drawHeight);
    ctx.drawImage(templateBitmap, 0, 0);
    photoBitmap.close();
    templateBitmap.close();
    const [width, height] = fitEdge(tw, th);
    const small = document.createElement("canvas");
    small.width = width;
    small.height = height;
    small.getContext("2d")?.drawImage(canvas, 0, 0, width, height);
    const payload = toPng(small);
    payload.width = tw;
    payload.height = th;
    return payload;
  }

  async templatePreview(path: string): Promise<PreviewPayload> {
    return await this.preview(path);
  }

  async exportRemove(
    entries: QueueEntry[],
    options: MatteOptionsDto,
    destination: string,
  ): Promise<void> {
    for (const entry of entries) {
      try {
        await this.refine(entry.path, options);
        this.emit({ type: "batch-item", ok: true, message: `${destination}/${entry.label}` });
      } catch (error) {
        this.emit({ type: "batch-item", ok: false, message: String(error) });
      }
    }
    this.emit({ type: "batch-done", ok: entries.length, failed: 0, folder: destination });
  }

  async exportMerge(
    entries: QueueEntry[],
    template: string,
    fit: Fit,
    destination: string,
  ): Promise<void> {
    for (const entry of entries) {
      try {
        await this.mergePreview(entry.path, template, fit);
        this.emit({ type: "batch-item", ok: true, message: `${destination}/${entry.label}` });
      } catch (error) {
        this.emit({ type: "batch-item", ok: false, message: String(error) });
      }
    }
    this.emit({ type: "batch-done", ok: entries.length, failed: 0, folder: destination });
  }

  async cancelBatch() {
    this.emit({ type: "batch-done", ok: 0, failed: 0, folder: "(cancelled)" });
  }

  async copyCutout(_path: string, _options: MatteOptionsDto): Promise<void> {
    throw new Error("Clipboard is unavailable in the browser preview.");
  }

  async copyPhoto(_path: string): Promise<void> {
    throw new Error("Clipboard is unavailable in the browser preview.");
  }

  async pasteImage(_dir: string): Promise<string> {
    throw new Error("Clipboard images are unavailable in the browser preview.");
  }

  async pickImages(): Promise<string[]> {
    return await new Promise((resolve) => {
      const input = document.createElement("input");
      input.type = "file";
      input.multiple = true;
      input.accept = "image/png,image/jpeg,image/webp,image/bmp,image/gif,image/tiff,image/tga";
      input.onchange = () => {
        resolve(this.registerFiles(Array.from(input.files ?? [])));
      };
      input.click();
    });
  }

  async pickTemplate(): Promise<string | null> {
    return await new Promise((resolve) => {
      const input = document.createElement("input");
      input.type = "file";
      input.accept = "image/png";
      input.onchange = () => {
        const registered = this.registerFiles(Array.from(input.files ?? []));
        resolve(registered[0] ?? null);
      };
      input.click();
    });
  }

  /** The browser build exports into a virtual folder. */
  async pickFolder(): Promise<string | null> {
    return "browser://downloads";
  }

  onBatchEvent(cb: (event: BatchEvent) => void) {
    this.listeners.push(cb);
  }

  private emit(event: BatchEvent) {
    for (const listener of this.listeners) {
      listener(event);
    }
  }
}
