<script lang="ts">
  /// Zoom slider, backdrop swatches, and the Original toggle, as one bar.
  import { store } from "$state/app.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { CHECKER, FILLED_DARK, MINUS, PLUS, SQUARE_WHITE } from "$lib/icons";
  import type { Backdrop } from "$lib/types";

  const backdrops: Array<{ id: Backdrop; label: string; title: string; icon: string }> = [
    { id: "checker", label: "▦", title: "Checkerboard", icon: CHECKER },
    { id: "dark", label: "■", title: "Dark backdrop exposes light halos", icon: FILLED_DARK },
    { id: "white", label: "□", title: "White backdrop exposes dark fringes", icon: SQUARE_WHITE },
  ];

  // Logarithmic slider: 0 → 1x, 1 → 8x, same curve as the original.
  const sliderValue = $derived(Math.log(store.zoomTarget) / Math.log(8));

  function onSlider(event: Event) {
    const value = Number((event.target as HTMLInputElement).value);
    store.setZoomTarget(Math.pow(8, value));
  }

  const percent = $derived(`${Math.round(store.zoomDisplay * 100)}%`);
</script>

<div class="zoom-bar" role="toolbar" aria-label="Zoom">
  <button class="small" onclick={() => store.zoomBy(1 / 1.25)} aria-label="Zoom out">
    <Icon body={MINUS} />
  </button>
  <input
    type="range"
    min="0"
    max="1"
    step="0.001"
    value={sliderValue}
    oninput={onSlider}
    style="width:140px"
    aria-label="Zoom level"
  />
  <button class="small" onclick={() => store.zoomBy(1.25)} aria-label="Zoom in">
    <Icon body={PLUS} />
  </button>
  <span class="mono percent">{percent}</span>

  <span class="divider"></span>

  {#each backdrops as backdrop (backdrop.id)}
    <button
      class="small swatch"
      class:active={store.backdrop === backdrop.id}
      onclick={() => (store.backdrop = backdrop.id)}
      title={backdrop.title}
      aria-label={backdrop.title}
    >
      <Icon body={backdrop.icon} />
    </button>
  {/each}

  <span class="divider"></span>

  <button
    class="small"
    class:active={store.comparePin}
    onclick={() => (store.comparePin = !store.comparePin)}
    title="Pin the source photo for compare"
  >
    Original
  </button>
</div>

<style>
  .zoom-bar {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    bottom: 12px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 10px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 4px;
    max-width: calc(100% - 16px);
  }

  .percent {
    color: var(--text-secondary);
    min-width: 44px;
    text-align: right;
  }

  .divider {
    width: 1px;
    height: 18px;
    background: var(--border);
  }

  .swatch {
    color: var(--text-secondary);
    padding: 2px 6px;
  }

  .swatch.active {
    background: var(--border-strong);
    color: var(--text);
  }
</style>
