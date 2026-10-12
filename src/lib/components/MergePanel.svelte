<script lang="ts">
  import { Merge } from "@lucide/svelte";
  import {
    queue,
    merge,
    currentName,
    megapixels,
    fileName,
    pickTemplate,
    setMergeFit,
    beginMerge,
    cancelMerge,
  } from "$lib/state.svelte";
  import Destination from "./Destination.svelte";
  import BatchAction from "./BatchAction.svelte";

  const photoDims = $derived(merge.templateWidth > 0 ? `${merge.templateWidth} x ${merge.templateHeight}` : "-");
  const templateName = $derived(merge.templatePath ? fileName(merge.templatePath) : "Pick a template");
  const count = $derived(queue.paths.length);
  const canExport = $derived(!merge.batch.busy && count > 0 && merge.templatePath != null);
  const fits = ["Cover", "Fit", "Stretch"];
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
      <span class="num text-[12px]">{photoDims}</span>
    </div>
    <div class="flex items-center">
      <span class="secondary-text text-[12px]">Pixels</span>
      <div class="flex-1"></div>
      <span class="num text-[12px]">{megapixels(merge.templateWidth, merge.templateHeight)}</span>
    </div>
    <div class="title-text text-[13px]">Template</div>
    <button class="btn btn-secondary w-full px-3 py-2" onclick={() => void pickTemplate()}>
      <span class="overflow-hidden text-ellipsis">{templateName}</span>
    </button>
    <div class="title-text text-[13px]">Fit</div>
    <div class="flex gap-2">
      {#each fits as fit}
        <button
          class="btn flex-1 px-3 py-2 {merge.mergeFit === fit ? 'btn-action' : 'btn-secondary'}"
          onclick={() => setMergeFit(fit)}
        >
          {fit}
        </button>
      {/each}
    </div>
    <span class="caption">Cover fills and crops. Fit shows all. Stretch ignores shape.</span>
  </div>

  <div class="flex flex-col gap-2">
    <div class="title-text text-[13px]">Output</div>
    <div class="flex items-center">
      <span class="secondary-text text-[12px]">Format</span>
      <div class="flex-1"></div>
      <span class="num text-[12px]">PNG · template size</span>
    </div>
    <Destination />

    <BatchAction
      {count}
      verb="Merge"
      doing="Merging"
      {canExport}
      onExport={() => void beginMerge()}
      batch={merge.batch}
      status={merge.status}
      onCancel={() => void cancelMerge()}
    >
      {#snippet icon()}
        <Merge size={14} />
      {/snippet}
    </BatchAction>
  </div>
</div>
