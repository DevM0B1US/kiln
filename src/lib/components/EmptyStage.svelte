<script lang="ts">
  /// Empty-stage tile: icon square, call to action, Browse + Paste.
  import Icon from "$lib/components/Icon.svelte";
  import { HEXAGON } from "$lib/icons";
  import { store } from "$state/app.svelte";
</script>

<div class="empty">
  <div class="tile"><Icon body={HEXAGON} size={28} /></div>
  <div class="caption">Drop images here</div>
  <div class="row">
    <button class="small" onclick={() => store.paste()}>Paste</button>
    <button
      class="small"
      onclick={() => void store.bridge.pickImages().then((p) => store.addPaths(p))}
    >
      Browse
    </button>
  </div>
</div>

<style>
  .empty {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
  }

  .tile {
    width: 64px;
    height: 64px;
    display: grid;
    place-items: center;
    background: var(--surface-raised);
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    color: var(--text-secondary);
  }

  .tile :global(svg) {
    width: 28px;
    height: 28px;
  }
</style>
