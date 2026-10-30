<script lang="ts">
  /// Kiln: one window, two tools, three panes. The store owns all state;
  /// this shell wires layout, theme, drag-drop, and the boot sequence.
  import { onMount } from "svelte";
  import { inTauri } from "$lib/bridge/select";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { store } from "$state/app.svelte";
  import TitleBar from "$lib/components/TitleBar.svelte";
  import ModelBanner from "$lib/components/ModelBanner.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Stage from "$lib/components/Stage.svelte";
  import Panel from "$lib/components/Panel.svelte";

  let dropHover = $state(false);

  // The palette lives on the document root; the tokens are defined there.
  $effect(() => {
    const root = document.documentElement;
    root.classList.remove("theme-dark", "theme-light");
    root.classList.add(`theme-${store.theme}`);
  });

  onMount(() => {
    store.boot();
    if (inTauri()) {
      // Window drag-drop carries absolute paths in v2. Non-image drops are
      // skipped silently by the store.
      void getCurrentWebviewWindow().onDragDropEvent((event) => {
        if (event.payload.type === "enter") {
          dropHover = true;
        } else if (event.payload.type === "leave") {
          dropHover = false;
        } else if (event.payload.type === "drop") {
          dropHover = false;
          store.addPaths(event.payload.paths);
        }
      });
    }
  });

  function onDrop(event: DragEvent) {
    event.preventDefault();
    dropHover = false;
    // Browser build: register the dropped files with the mock bridge.
    if (!inTauri()) {
      const files = Array.from(event.dataTransfer?.files ?? []);
      store.addPaths(store.bridge.registerFiles(files));
    }
  }

  function onDragOver(event: DragEvent) {
    if (!inTauri()) {
      event.preventDefault();
      dropHover = true;
    }
  }

  function onDragLeave() {
    dropHover = false;
  }
</script>

<svelte:window oncontextmenu={(event) => event.preventDefault()} />

<div class="kiln">
  <TitleBar />
  {#if store.modelInstalled === false}
    <ModelBanner message="Model file is missing. Put RMBG-1.4 back in models/ next to the app." />
  {/if}
  <div
    class="content"
    role="region"
    aria-label="Kiln workspace"
    ondrop={onDrop}
    ondragover={onDragOver}
    ondragleave={onDragLeave}
  >
    <Sidebar />
    <Stage />
    <Panel />
  </div>
  {#if dropHover}
    <div class="drop-overlay"></div>
  {/if}
</div>

<style>
  .kiln {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }

  .content {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  .drop-overlay {
    position: fixed;
    inset: 0;
    border: 1px solid var(--border-strong);
    background: transparent;
    pointer-events: none;
  }
</style>
