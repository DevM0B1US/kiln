<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import {
    shell,
    remove,
    refreshModel,
    addAndPreview,
    isImage,
  } from "$lib/state.svelte";
  import TitleBar from "$lib/components/TitleBar.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Stage from "$lib/components/Stage.svelte";
  import RemovePanel from "$lib/components/RemovePanel.svelte";
  import MergePanel from "$lib/components/MergePanel.svelte";

  $effect(() => {
    document.documentElement.dataset.theme = shell.darkMode ? "dark" : "light";
  });

  onMount(() => {
    void refreshModel();
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "over") {
          shell.dropActive = true;
        } else if (event.payload.type === "drop") {
          shell.dropActive = false;
          shell.contextMenu = false;
          const paths = event.payload.paths.filter(isImage);
          if (paths.length > 0) void addAndPreview(paths);
        } else {
          shell.dropActive = false;
        }
      })
      .then((stop) => (unlisten = stop))
      .catch(() => undefined);
    const key = (e: KeyboardEvent) => (shell.ctrlHeld = e.ctrlKey);
    const clear = () => (shell.ctrlHeld = false);
    window.addEventListener("keydown", key);
    window.addEventListener("keyup", key);
    window.addEventListener("blur", clear);
    return () => {
      unlisten?.();
      window.removeEventListener("keydown", key);
      window.removeEventListener("keyup", key);
      window.removeEventListener("blur", clear);
    };
  });
</script>

<div
  class="flex h-screen w-screen flex-col"
  role="presentation"
  oncontextmenu={(e) => e.preventDefault()}
>
  <TitleBar />
  <div class="rule-h"></div>
  {#if !remove.modelReady}
    <div class="banner text-[12px]">{remove.modelMessage || "Model file is missing."}</div>
    <div class="rule-h"></div>
  {/if}
  <div class="flex min-h-0 flex-1">
    <Sidebar />
    <div class="rule-v"></div>
    <div class="min-w-0 flex-1" style="background: var(--bg);">
      <Stage />
    </div>
    <div class="rule-v"></div>
    <div class="w-[264px] flex-none overflow-y-auto p-4" style="background: var(--surface);">
      {#if shell.section === "remove"}
        <RemovePanel />
      {:else}
        <MergePanel />
      {/if}
    </div>
  </div>
</div>
