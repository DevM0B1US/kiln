<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { Moon, Sun, Minus, Square, X } from "@lucide/svelte";
  import { shell, currentName } from "$lib/state.svelte";

  const win = getCurrentWindow();

  function drag(e: MouseEvent) {
    if (e.button === 0 && e.detail < 2) void win.startDragging();
  }
</script>

<div class="flex h-9 flex-none items-center gap-1" style="background: var(--surface);">
  <div class="title-text flex-none px-2 py-2">EmberStudio</div>
  <div
    class="flex flex-1 cursor-default items-center justify-center self-stretch"
    role="presentation"
    onmousedown={drag}
    ondblclick={() => void win.toggleMaximize()}
  >
    <span class="secondary-text text-[12px]">{currentName()}</span>
  </div>
  <button
    class="btn btn-ghost h-full px-3"
    onclick={() => (shell.darkMode = !shell.darkMode)}
    aria-label="Toggle theme"
  >
    {#if shell.darkMode}
      <Sun size={14} />
    {:else}
      <Moon size={14} />
    {/if}
  </button>
  <button class="btn btn-ghost h-full w-11" onclick={() => void win.minimize()} aria-label="Minimize">
    <Minus size={12} />
  </button>
  <button class="btn btn-ghost h-full w-11" onclick={() => void win.toggleMaximize()} aria-label="Maximize">
    <Square size={12} />
  </button>
  <button
    class="btn btn-ghost btn-frame-close h-full w-11"
    onclick={() => void win.close()}
    aria-label="Close"
  >
    <X size={12} />
  </button>
</div>
