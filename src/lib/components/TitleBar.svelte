<script lang="ts">
  /// Custom title bar: drag to move, double-click to maximise, theme toggle,
  /// window controls. No OS chrome — this bar is the app's.
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { inTauri } from "$lib/bridge/select";
  import Icon from "$lib/components/Icon.svelte";
  import { CLOSE, MINUS, RESTORE, SQUARE } from "$lib/icons";
  import { store } from "$state/app.svelte";

  // The Window handle only exists inside the webview; the browser build
  // keeps the bar inert instead of crashing.
  const win = inTauri() ? getCurrentWindow() : null;

  let maximized = $state(false);

  async function toggleMaximize() {
    if (!win) {
      return;
    }
    await win.toggleMaximize();
    maximized = await win.isMaximized();
  }

  async function minimize() {
    await win?.minimize();
  }

  async function close() {
    await win?.close();
  }

  function startDrag(event: MouseEvent) {
    if (event.button === 0) {
      void win?.startDragging();
    }
  }
</script>

<header class="title-bar">
  <span class="title brand">Kiln</span>
  <div
    class="drag-region"
    onmousedown={startDrag}
    ondblclick={toggleMaximize}
    role="button"
    tabindex="-1"
    aria-label="Window drag handle"
  >
    <span class="file-name truncate">{store.current?.label ?? "no image"}</span>
  </div>
  <button class="small theme-toggle" onclick={() => store.toggleTheme()}>
    {store.theme === "dark" ? "◐ Dark" : "◑ Light"}
  </button>
  {#if inTauri()}
    <div class="window-buttons">
      <button class="small ghost" onclick={minimize} aria-label="Minimise">
        <Icon body={MINUS} />
      </button>
      <button class="small ghost" onclick={toggleMaximize} aria-label="Maximise">
        <Icon body={maximized ? RESTORE : SQUARE} />
      </button>
      <button class="small ghost close" onclick={close} aria-label="Close">
        <Icon body={CLOSE} />
      </button>
    </div>
  {/if}
</header>

<style>
  .title-bar {
    height: var(--title-bar-h);
    flex: 0 0 var(--title-bar-h);
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 12px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .brand {
    color: var(--text);
  }

  .drag-region {
    flex: 1;
    height: 26px;
    display: flex;
    align-items: center;
    min-width: 40px;
  }

  .file-name {
    font-size: 12px;
    color: var(--text-secondary);
  }

  .theme-toggle {
    color: var(--text-secondary);
    white-space: nowrap;
  }

  .window-buttons {
    display: flex;
    gap: 4px;
  }

  .ghost {
    background: transparent;
    border-color: transparent;
    color: var(--text-secondary);
    padding: 4px 6px;
  }

  .ghost:hover:not(:disabled) {
    background: var(--border);
    color: var(--text);
  }

  .close:hover:not(:disabled) {
    background: var(--danger);
    border-color: var(--danger);
    color: #ffffff;
  }
</style>
