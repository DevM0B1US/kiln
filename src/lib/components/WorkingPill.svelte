<script lang="ts">
  /// `n/total` pill with a thin progress bar, bottom-right of the stage.
  import { store } from "$state/app.svelte";

  const progress = $derived(store.batch);
  const fraction = $derived(
    progress && progress.total > 0 ? Math.min(1, progress.done / progress.total) : 0,
  );
</script>

{#if progress}
  <div class="pill">
    <span class="mono count">{progress.done}/{progress.total}</span>
    <span class="bar">
      <span class="fill" style="width:{fraction * 100}%"></span>
    </span>
    <button class="small" onclick={() => store.cancelExport()}>Cancel</button>
  </div>
{/if}

<style>
  .pill {
    position: absolute;
    right: 16px;
    bottom: 16px;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 7px 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 4px;
  }

  .count {
    color: var(--text);
    font-size: 13px;
  }

  .bar {
    width: 80px;
    height: 3px;
    background: var(--border);
    border-radius: 1.5px;
    overflow: hidden;
    display: block;
  }

  .fill {
    height: 100%;
    background: var(--text-secondary);
    display: block;
  }
</style>
