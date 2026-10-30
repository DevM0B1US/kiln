<script lang="ts">
  /// Custom right-click menu on the image: Copy / Paste / Clear.
  /// Clear drops the current image from the queue. Native context menus are
  /// disabled everywhere (see App.svelte).
  import { store } from "$state/app.svelte";

  const at = $derived(store.contextMenu);

  function close() {
    store.contextMenu = null;
  }

  function copy() {
    store.copyCurrent();
    close();
  }

  function paste() {
    store.paste();
    close();
  }

  function clear() {
    store.clearCurrent();
    close();
  }
</script>

{#if at}
  <div
    class="backdrop"
    role="presentation"
    onclick={close}
    onkeydown={(event) => {
      if (event.key === "Escape") {
        close();
      }
    }}
    oncontextmenu={(e) => {
      e.preventDefault();
      close();
    }}
  >
    <div class="menu" style="left:{at.x}px;top:{at.y}px" role="menu">
      <button role="menuitem" onclick={copy}>Copy</button>
      <button role="menuitem" onclick={paste}>Paste</button>
      <button role="menuitem" onclick={clear}>Clear</button>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
  }

  .menu {
    position: fixed;
    display: flex;
    flex-direction: column;
    min-width: 120px;
    padding: 4px;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: 4px;
  }

  .menu button {
    text-align: left;
    background: transparent;
    border-color: transparent;
    border-radius: 0;
    padding: 4px 10px;
  }

  .menu button:hover {
    background: var(--border);
  }
</style>
