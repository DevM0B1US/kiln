<script lang="ts">
  import { Download, Save } from "@lucide/svelte";
  import {
    queue,
    remove,
    currentName,
    megapixels,
    setThreshold,
    typeThreshold,
    setFeather,
    typeFeather,
    setDilate,
    typeDilate,
    toggleTrim,
    resetOpts,
    beginRemove,
    cancelRemove,
    saveRemoveTo,
  } from "$lib/state.svelte";
  import Destination from "./Destination.svelte";
  import BatchAction from "./BatchAction.svelte";

  const dims = $derived(remove.imageWidth > 0 ? `${remove.imageWidth} x ${remove.imageHeight}` : "-");
  const thresholdText = $derived(remove.thresholdOn ? `${remove.threshold}` : "off");
  const thresholdFill = $derived((remove.threshold / 255) * 100);
  const featherFill = $derived((remove.feather / 10) * 100);
  const dilateFill = $derived((remove.dilate / 10) * 100);
  const count = $derived(queue.paths.length);
  const canExport = $derived(!remove.batch.busy && count > 0);
</script>

<div class="flex flex-col gap-6">
  <div class="flex flex-col gap-2">
    <div class="title-text text-[13px]">Image</div>
    <div class="flex items-center">
      <span class="secondary-text text-[12px]">File</span>
      <div class="flex-1"></div>
      <span class="num text-[12px]">{currentName()}</span>
    </div>
    <div class="flex items-center">
      <span class="secondary-text text-[12px]">Size</span>
      <div class="flex-1"></div>
      <span class="num text-[12px]">{dims}</span>
    </div>
    <div class="flex items-center">
      <span class="secondary-text text-[12px]">Pixels</span>
      <div class="flex-1"></div>
      <span class="num text-[12px]">{megapixels(remove.imageWidth, remove.imageHeight)}</span>
    </div>
    <div class="title-text text-[13px]">Refine Edge</div>
    <div class="group-label">Cutoff</div>
    <div class="flex items-center gap-3">
      <span class="w-[72px] flex-none text-[12px]">Threshold</span>
      <input
        type="range"
        class="ember-slider"
        min={0}
        max={255}
        step={1}
        value={remove.threshold}
        style="--fill: {thresholdFill}%"
        oninput={(e) => setThreshold(Number((e.target as HTMLInputElement).value))}
        aria-label="Threshold"
      />
      <input
        type="text"
        class="ember-input"
        value={thresholdText}
        onchange={(e) => typeThreshold((e.target as HTMLInputElement).value)}
        aria-label="Threshold value"
      />
    </div>
    <span class="caption">Pixels below this go fully transparent.</span>
    <div class="group-label">Edge</div>
    <div class="flex items-center gap-3">
      <span class="w-[72px] flex-none text-[12px]">Feather</span>
      <input
        type="range"
        class="ember-slider"
        min={0}
        max={10}
        step={0.5}
        value={remove.feather}
        style="--fill: {featherFill}%"
        oninput={(e) => setFeather(Number((e.target as HTMLInputElement).value))}
        aria-label="Feather"
      />
      <input
        type="text"
        class="ember-input"
        value={remove.feather.toFixed(1)}
        onchange={(e) => typeFeather((e.target as HTMLInputElement).value)}
        aria-label="Feather value"
      />
    </div>
    <span class="caption">Softens the edge before the cut.</span>
    <div class="flex items-center gap-3">
      <span class="w-[72px] flex-none text-[12px]">Dilate</span>
      <input
        type="range"
        class="ember-slider"
        min={0}
        max={10}
        step={1}
        value={remove.dilate}
        style="--fill: {dilateFill}%"
        oninput={(e) => setDilate(Number((e.target as HTMLInputElement).value))}
        aria-label="Dilate"
      />
      <input
        type="text"
        class="ember-input"
        value={`${remove.dilate}`}
        onchange={(e) => typeDilate((e.target as HTMLInputElement).value)}
        aria-label="Dilate value"
      />
    </div>
    <span class="caption">Grows the kept area outward.</span>
    <div class="group-label">Frame</div>
    <label class="flex cursor-pointer items-center gap-2 text-[12px]">
      <input
        type="checkbox"
        class="ember-checkbox"
        checked={remove.trim}
        onchange={(e) => toggleTrim((e.target as HTMLInputElement).checked)}
      />
      <span>Trim to subject</span>
    </label>
    <span class="caption">Crops the output to the subject box.</span>
    <button class="btn btn-ghost self-start px-2 py-1" onclick={resetOpts}>Reset all</button>
  </div>

  <div class="flex flex-col gap-2">
    <div class="title-text text-[13px]">Output</div>
    <div class="flex items-center">
      <span class="secondary-text text-[12px]">Format</span>
      <div class="flex-1"></div>
      <span class="num text-[12px]">PNG · transparent</span>
    </div>
    <Destination />

    <BatchAction
      {count}
      verb="Remove background"
      doing="Removing"
      {canExport}
      onExport={() => void beginRemove()}
      batch={remove.batch}
      status={remove.status}
      onCancel={() => void cancelRemove()}
    >
      {#snippet icon()}
        <Download size={14} />
      {/snippet}
    </BatchAction>
    <button class="btn btn-secondary w-full px-3 py-2" disabled={!canExport} onclick={() => void saveRemoveTo()}>
      <Save size={14} />
      <span>Save to…</span>
    </button>
  </div>
</div>
