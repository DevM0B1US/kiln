<script lang="ts">
  import type { Snippet } from "svelte";
  import { elapsedText, type Batch } from "$lib/state.svelte";

  interface Props {
    count: number;
    verb: string;
    doing: string;
    icon: Snippet;
    canExport: boolean;
    onExport: () => void;
    batch: Batch;
    status: string;
    onCancel: () => void;
  }

  const { count, verb, doing, icon, canExport, onExport, batch, status, onCancel }: Props = $props();
  const noun = $derived(count === 1 ? "image" : "images");
  const pct = $derived(Math.round(batch.progress * 100));
  const label = $derived(
    batch.busy
      ? `${doing} ${batch.index}/${batch.total} · ${pct}% · ${elapsedText(batch.elapsedMs)}`
      : `${verb} (${count} ${noun})`,
  );
</script>

<div class="flex flex-col gap-2">
  <span class="num secondary-text text-[11px]">{count} {noun} in queue</span>
  <button
    class="btn btn-action relative w-full overflow-hidden px-3 py-2"
    disabled={!canExport && !batch.busy}
    onclick={() => {
      if (canExport) onExport();
    }}
  >
    {#if batch.busy}
      <span
        class="pointer-events-none absolute inset-y-0 left-0"
        style="width: {pct}%; background: var(--bg); opacity: 0.25;"
      ></span>
    {/if}
    <span class="relative flex items-center gap-2">
      {@render icon()}
      <span>{label}</span>
    </span>
  </button>
  {#if batch.busy}
    <button class="btn btn-danger-text self-start px-2 py-1" onclick={onCancel}>Cancel</button>
  {/if}
  {#if status !== ""}
    <span class="secondary-text text-[12px]">{status}</span>
  {/if}
</div>
