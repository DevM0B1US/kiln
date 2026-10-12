<script lang="ts">
  import { queue, pickOutputDir, folderName } from "$lib/state.svelte";

  const picked = $derived(queue.outputDir ? folderName(queue.outputDir) : "Pick a folder");
</script>

<div class="flex flex-col gap-2">
  <div class="title-text text-[13px]">Destination</div>
  <button class="btn btn-secondary w-full px-3 py-2" onclick={() => void pickOutputDir()}>
    <span class="overflow-hidden text-ellipsis">{picked}</span>
  </button>
  {#if queue.outputDir}
    <span class="num secondary-text text-[11px]">{queue.outputDir}</span>
  {/if}
</div>
