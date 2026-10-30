<script lang="ts">
  /// The stage: locked centered viewer, checkerboard, hold-to-compare, zoom.
  ///
  /// Wheel, buttons and double-click zoom about the center with smoothing
  /// (rAF lerp, same 0.25 factor as the original). No pan. Holding the image
  /// compares with the original; the Original toggle pins it. Right-click on
  /// the image opens the custom Copy / Paste / Clear menu.
  import { store, fitEdge } from "$state/app.svelte";
  import EmptyStage from "$lib/components/EmptyStage.svelte";
  import ZoomBar from "$lib/components/ZoomBar.svelte";
  import WorkingPill from "$lib/components/WorkingPill.svelte";
  import ContextMenu from "$lib/components/ContextMenu.svelte";

  let stageWidth = $state(0);
  let stageHeight = $state(0);

  // Animated zoom: approach the target, then stop ticking.
  $effect(() => {
    let frame: number | null = null;
    const tick = () => {
      const diff = store.zoomTarget - store.zoom;
      if (Math.abs(diff) > 0.001) {
        store.zoom += diff * 0.25;
        frame = requestAnimationFrame(tick);
      } else {
        store.zoom = store.zoomTarget;
        frame = null;
      }
    };
    frame = requestAnimationFrame(tick);
    return () => {
      if (frame !== null) {
        cancelAnimationFrame(frame);
      }
    };
  });

  const texture = $derived.by(() => {
    const path = store.current?.path ?? null;
    if (path === null) {
      return null;
    }
    const matches = (texture: { path: string } | null) => texture?.path === path;
    const showOriginal = store.compareHold || store.comparePin;
    if (!showOriginal) {
      if (store.tool === "merge" && matches(store.mergedTex)) {
        return store.mergedTex;
      }
      if (store.tool === "remove" && matches(store.refinedTex)) {
        return store.refinedTex;
      }
    }
    if (matches(store.originalTex)) {
      return store.originalTex;
    }
    if (store.tool === "merge" && matches(store.templateTex)) {
      return store.templateTex;
    }
    return null;
  });

  const imageRect = $derived.by(() => {
    if (!texture || stageWidth <= 0 || stageHeight <= 0) {
      return null;
    }
    const [fitWidth, fitHeight] = fitEdge(texture.width, texture.height);
    let width = fitWidth * store.zoomDisplay;
    let height = fitHeight * store.zoomDisplay;
    if (store.zoomDisplay <= 1.01) {
      // Fit inside the stage at rest.
      const scale = Math.min(stageWidth / width, stageHeight / height, 1);
      width *= scale;
      height *= scale;
    }
    const maxSize = Math.min(stageWidth * 8, stageHeight * 8);
    const clamped = Math.min(width, height, maxSize > 0 ? maxSize : width);
    if (clamped !== width && width > 0) {
      height = (height * clamped) / width;
      width = clamped;
    }
    return { width, height };
  });

  function onWheel(event: WheelEvent) {
    event.preventDefault();
    store.wheelZoom(-event.deltaY);
  }

  function onHoldDown(event: MouseEvent) {
    if (event.button === 0) {
      store.compareHold = true;
    }
  }

  function onHoldUp() {
    store.compareHold = false;
  }

  function onContextMenu(event: MouseEvent) {
    event.preventDefault();
    store.contextMenu = { x: event.clientX, y: event.clientY };
  }
</script>

<main class="stage" bind:clientWidth={stageWidth} bind:clientHeight={stageHeight} onwheel={onWheel}>
  {#if !store.current}
    <EmptyStage />
  {:else if !texture}
    <div class="loading caption">Loading preview…</div>
  {:else if imageRect}
    <div
      class="image-holder"
      style="width:{imageRect.width}px;height:{imageRect.height}px"
      class:checker={store.backdrop === "checker"}
      class:solid-dark={store.backdrop === "dark"}
      class:solid-white={store.backdrop === "white"}
      role="button"
      aria-label={store.current?.label ?? "preview"}
      tabindex="-1"
      onmousedown={onHoldDown}
      onmouseup={onHoldUp}
      onmouseleave={onHoldUp}
      ondblclick={() => store.doubleClickZoom()}
      oncontextmenu={onContextMenu}
    >
      <img
        class="pixels"
        src={texture.url}
        alt={store.current?.label ?? "preview"}
        draggable="false"
      />
    </div>
    <ZoomBar />
    {#if store.batch}
      <WorkingPill />
    {/if}
  {/if}
</main>

{#if store.contextMenu}
  <ContextMenu />
{/if}

<style>
  .stage {
    position: relative;
    flex: 1;
    overflow: hidden;
    background: var(--bg);
  }

  .loading {
    height: 100%;
    display: grid;
    place-items: center;
  }

  .image-holder {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    overflow: hidden;
  }

  .solid-dark {
    /* A dark backdrop must stay dark in light mode: it exposes halos. */
    background: #141414;
  }

  .solid-white {
    background: #f4f4f4;
  }

  .pixels {
    width: 100%;
    height: 100%;
    display: block;
    image-rendering: auto;
    cursor: zoom-in;
  }
</style>
