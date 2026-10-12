<script lang="ts">
  import { tick } from "svelte";
  import { ImagePlus, ClipboardPaste, Minus, Plus, Eye } from "@lucide/svelte";
  import {
    shell,
    queue,
    remove,
    merge,
    clampZoom,
    fitZoom,
    ZOOM_MIN,
    ZOOM_MAX,
    ZOOM_FIT,
    ZOOM_ACTUAL,
    browseFiles,
    copyCurrent,
    pasteClipboard,
    removeCurrent,
  } from "$lib/state.svelte";

  const BASE_PREVIEW = 640;

  const mergeMode = $derived(shell.section === "merge");
  const imageWidth = $derived(mergeMode ? merge.templateWidth : remove.imageWidth);
  const imageHeight = $derived(mergeMode ? merge.templateHeight : remove.imageHeight);
  const original = $derived(mergeMode ? merge.original : remove.original);
  const result = $derived(mergeMode ? merge.merged : remove.refined);
  const showOriginal = $derived(mergeMode ? merge.showOriginal : remove.showOriginal);
  const exportBusy = $derived(remove.batch.busy || merge.batch.busy);
  const shown = $derived(exportBusy || showOriginal ? original : (result ?? original));

  const operation = $derived.by(() => {
    if (remove.batch.busy) {
      const batch = remove.batch;
      return { label: `Removing ${batch.index + 1}/${batch.total}`, progress: batch.progress };
    }
    if (merge.batch.busy) {
      const batch = merge.batch;
      return { label: `Merging ${batch.index + 1}/${batch.total}`, progress: batch.progress };
    }
    if (shell.section === "remove" && (remove.matteBusy || remove.refineBusy))
      return { label: "Working on cutout…", progress: null };
    if (shell.section === "merge" && merge.mergeBusy) return { label: "Working on preview…", progress: null };
    return null;
  });

  let viewport = $state<HTMLDivElement | null>(null);
  let fittedKey: string | null = null;
  let tile = $state("");

  let lastX = 0;
  let lastY = 0;

  function makeTile(): string {
    const dark = shell.darkMode;
    const a = dark ? "#1e1e1e" : "#c4c4c4";
    const b = dark ? "#2a2a2a" : "#d8d8d8";
    const canvas = document.createElement("canvas");
    canvas.width = 32;
    canvas.height = 32;
    const g = canvas.getContext("2d");
    if (!g) return "";
    g.fillStyle = a;
    g.fillRect(0, 0, 32, 32);
    g.fillStyle = b;
    g.fillRect(0, 0, 16, 16);
    g.fillRect(16, 16, 16, 16);
    return canvas.toDataURL();
  }

  $effect(() => {
    tile = makeTile();
  });

  function centerScroll() {
    const el = viewport;
    if (!el) return;
    el.scrollLeft = Math.max(0, (el.scrollWidth - el.clientWidth) / 2);
    el.scrollTop = Math.max(0, (el.scrollHeight - el.clientHeight) / 2);
  }

  function setZoom(value: number) {
    shell.zoom = clampZoom(value);
  }

  function centeredZoom(value: number) {
    setZoom(value);
    void tick().then(centerScroll);
  }

  async function onWheel(e: WheelEvent) {
    e.preventDefault();
    const el = viewport;
    if (!el) return;
    const raw = e.deltaMode === 1 ? e.deltaY * 33 : e.deltaY;
    const clamped = Math.max(-120, Math.min(120, raw));
    if (clamped === 0) return;
    const nz = clampZoom(shell.zoom * Math.pow(1.15, -clamped / 100));
    if (nz === shell.zoom) return;
    const rect = el.getBoundingClientRect();
    const px = e.clientX - rect.left;
    const py = e.clientY - rect.top;
    const oldW = Math.max(1, el.scrollWidth);
    const oldH = Math.max(1, el.scrollHeight);
    const oldSL = el.scrollLeft;
    const oldST = el.scrollTop;
    setZoom(nz);
    await tick();
    el.scrollLeft = ((oldSL + px) * el.scrollWidth) / oldW - px;
    el.scrollTop = ((oldST + py) * el.scrollHeight) / oldH - py;
  }

  function fitView() {
    centeredZoom(fitZoom(imageWidth, imageHeight));
  }

  function resetView() {
    centeredZoom(shell.zoom === ZOOM_FIT ? ZOOM_ACTUAL : ZOOM_FIT);
  }

  let dragging = false;

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0 && e.button !== 1) return;
    e.preventDefault();
    dragging = true;
    lastX = e.clientX;
    lastY = e.clientY;
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", onPointerUp);
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragging) return;
    const el = viewport;
    if (!el) return;
    el.scrollLeft -= e.clientX - lastX;
    el.scrollTop -= e.clientY - lastY;
    lastX = e.clientX;
    lastY = e.clientY;
  }

  function onPointerUp() {
    if (!dragging) return;
    dragging = false;
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    window.removeEventListener("pointercancel", onPointerUp);
  }

  $effect(() => {
    const key = shown == null ? null : `${queue.sourcePath}|${imageWidth}x${imageHeight}`;
    if (key == null) {
      fittedKey = null;
      return;
    }
    if (fittedKey !== key) {
      fittedKey = key;
      fitView();
    }
  });

  $effect(() => {
    const el = viewport;
    if (!el) return;
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  });

  function toggleCompare() {
    if (mergeMode) merge.showOriginal = !merge.showOriginal;
    else remove.showOriginal = !remove.showOriginal;
  }

  function onContextMenu(e: MouseEvent) {
    e.preventDefault();
    shell.contextMenu = true;
  }

  const zoomLabel = $derived(`${Math.round(shell.zoom)}%`);
  const dims = $derived(imageWidth > 0 ? `${imageWidth} x ${imageHeight}` : "");
  const dw = $derived((BASE_PREVIEW * shell.zoom) / 100);
  const dh = $derived(imageWidth === 0 ? dw : (dw * imageHeight) / imageWidth);
  const checkerBg = $derived(
    shell.backdrop === "checker" && tile !== "" ? `background-image: url('${tile}'); background-size: 32px 32px;` : "",
  );
  const backdropStyle = $derived(
    shell.backdrop === "checker"
      ? ""
      : shell.backdrop === "dark"
        ? "background: #0b0b0b;"
        : "background: #ffffff;",
  );
</script>

{#if queue.paths.length === 0 || (mergeMode && merge.templatePath == null && !merge.mergeBusy)}
  <div class="flex h-full w-full items-center justify-center">
    <div class="card flex w-[360px] flex-col items-center gap-3 p-8">
      <div class="raised-card flex h-12 w-12 items-center justify-center">
        <ImagePlus size={20} />
      </div>
      <div class="title-text text-[14px]">Drop images here</div>
      <div class="secondary-text text-center text-[12px]">
        {shell.dropActive ? "release to add" : "Transparent PNGs out, no questions asked"}
      </div>
      <div class="flex items-center gap-2 pt-1">
        <button class="btn btn-action px-5 py-2" onclick={() => void browseFiles()}>
          <ImagePlus size={14} />
          <span>Browse files</span>
        </button>
        <button class="btn btn-secondary px-4 py-2" onclick={() => void pasteClipboard()}>
          <ClipboardPaste size={14} />
          <span>Paste</span>
        </button>
      </div>
    </div>
  </div>
{:else if shown == null}
  <div class="flex h-full w-full items-center justify-center">
    <div class="card num secondary-text p-6 text-[12px]">
      {shell.section === "merge" ? "Working on the preview…" : "Working on the first cutout…"}
    </div>
  </div>
{:else}
  <div class="relative flex h-full w-full flex-col">
    <div class="relative min-h-0 flex-1">
      <div
        class="stage-scroll absolute inset-0 cursor-grab overflow-auto active:cursor-grabbing"
        style="touch-action: none; {backdropStyle}"
        role="presentation"
        bind:this={viewport}
        oncontextmenu={onContextMenu}
      >
        <div
          class="flex min-h-full min-w-full w-fit p-8"
          role="presentation"
          onpointerdown={onPointerDown}
          ondblclick={resetView}
        >
          <div
            class="relative m-auto flex-none overflow-hidden"
            style="width: {dw}px; height: {dh}px; outline: 1px solid var(--border-strong); outline-offset: -1px; {checkerBg}"
            role="presentation"
          >
            <img
              src={shown}
              alt="preview"
              draggable={false}
              class="pointer-events-none h-full w-full"
            />
            {#if operation}
              <div class="pointer-events-none absolute inset-0 overflow-hidden">
                <div class="ember-sweep"></div>
              </div>
            {/if}
          </div>
        </div>
      </div>
      {#if shell.contextMenu}
        <div
          class="absolute inset-0 z-20 flex items-center justify-center"
          style="background: rgba(0,0,0,0.6);"
          role="presentation"
          onpointerdown={() => (shell.contextMenu = false)}
          oncontextmenu={(e) => {
            e.preventDefault();
            shell.contextMenu = false;
          }}
        >
          <div class="raised-card flex w-40 flex-col gap-1 p-2" role="presentation" onpointerdown={(e) => e.stopPropagation()}>
            <button
              class="btn btn-ghost w-full justify-start px-3 py-2"
              onclick={() => {
                shell.contextMenu = false;
                void copyCurrent();
              }}>Copy</button
            >
            <button
              class="btn btn-ghost w-full justify-start px-3 py-2"
              onclick={() => {
                shell.contextMenu = false;
                void pasteClipboard();
              }}>Paste</button
            >
            <button
              class="btn btn-ghost w-full justify-start px-3 py-2"
              onclick={() => removeCurrent()}>Clear</button
            >
          </div>
        </div>
      {/if}
      {#if operation}
        <div class="pointer-events-none absolute inset-0 z-10 flex items-end justify-end p-4">
          <div class="raised-card flex w-[220px] flex-col gap-2 p-3">
            <span class="num secondary-text text-[12px]">{operation.label}</span>
            {#if operation.progress != null}
              <div class="thin-track"><div class="thin-fill" style="width: {Math.round(operation.progress * 100)}%"></div></div>
            {/if}
          </div>
        </div>
      {/if}
    </div>
    <div class="px-3 py-2">
      <input
        type="range"
        class="ember-slider w-full"
        min={ZOOM_MIN}
        max={ZOOM_MAX}
        step={5}
        value={shell.zoom}
        style="--fill: {((shell.zoom - ZOOM_MIN) / (ZOOM_MAX - ZOOM_MIN)) * 100}%"
        oninput={(e) => {
          centeredZoom(Number((e.target as HTMLInputElement).value));
        }}
        aria-label="Zoom"
      />
    </div>
    <div class="rule-h"></div>
    <div class="flex flex-none items-center gap-2 p-2" style="background: var(--surface);">
      <button class="btn btn-ghost px-3 py-1" onclick={() => centeredZoom(shell.zoom - 10)} aria-label="Zoom out">
        <Minus size={12} />
      </button>
      <button class="btn btn-ghost px-3 py-1" onclick={() => centeredZoom(shell.zoom + 10)} aria-label="Zoom in">
        <Plus size={12} />
      </button>
      <span class="num text-[12px]">{zoomLabel}</span>
      <button
        class="btn btn-ghost px-3 py-1"
        onclick={() => centeredZoom(ZOOM_FIT)}>Fit</button
      >
      <button
        class="btn btn-ghost px-3 py-1"
        onclick={() => centeredZoom(ZOOM_ACTUAL)}>1:1</button
      >
      <button
        class="btn px-3 py-1 {showOriginal ? 'btn-action' : 'btn-ghost'}"
        onclick={toggleCompare}
        aria-label="Compare with original"
        title="Show original"
      >
        <Eye size={12} />
        <span class="text-[12px]">Original</span>
      </button>
      <div class="flex items-center gap-1 pl-1" role="group" aria-label="Backdrop">
        <button
          class="btn h-4 w-4 flex-none p-0 {shell.backdrop === 'checker' ? 'ring-1' : ''}"
          style="background: repeating-conic-gradient(var(--checker-b) 0% 25%, var(--checker-a) 0% 50%) 0 0 / 8px 8px; {shell.backdrop === 'checker' ? 'outline: 1px solid var(--text);' : 'outline: 1px solid var(--border-strong);'}"
          onclick={() => (shell.backdrop = "checker")}
          aria-label="Checkerboard backdrop"
          title="Checkerboard"
        ></button>
        <button
          class="btn h-4 w-4 flex-none p-0"
          style="background: #0b0b0b; {shell.backdrop === 'dark' ? 'outline: 1px solid var(--text);' : 'outline: 1px solid var(--border-strong);'}"
          onclick={() => (shell.backdrop = "dark")}
          aria-label="Dark backdrop"
          title="Dark (exposes light halos)"
        ></button>
        <button
          class="btn h-4 w-4 flex-none p-0"
          style="background: #ffffff; {shell.backdrop === 'light' ? 'outline: 1px solid var(--text);' : 'outline: 1px solid var(--border-strong);'}"
          onclick={() => (shell.backdrop = "light")}
          aria-label="White backdrop"
          title="White (exposes dark fringes)"
        ></button>
      </div>
      <div class="flex-1"></div>
      <span class="num secondary-text text-[11px]">{dims}</span>
    </div>
  </div>
{/if}
