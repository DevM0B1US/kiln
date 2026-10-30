<script lang="ts">
  /// Tool marks plus collapse. Folds to the 64px rail.
  import Icon from "$lib/components/Icon.svelte";
  import { CHEVRONS_LEFT, CHEVRONS_RIGHT } from "$lib/icons";
  import { store } from "$state/app.svelte";
  import type { Tool } from "$lib/types";

  const tools: Array<{ id: Tool; label: string; mark: string }> = [
    { id: "remove", label: "Remove background", mark: "C" },
    { id: "merge", label: "Template merge", mark: "T" },
  ];
</script>

<nav class="sidebar" class:collapsed={store.collapsed}>
  {#each tools as tool (tool.id)}
    {#if store.collapsed}
      <button
        class="rail-mark mono"
        class:active={store.tool === tool.id}
        onclick={() => store.setTool(tool.id)}
        title={tool.label}
      >
        {tool.mark}
      </button>
    {:else}
      <button
        class="tool"
        class:active={store.tool === tool.id}
        onclick={() => store.setTool(tool.id)}
      >
        {tool.label}
      </button>
    {/if}
  {/each}

  <div class="spacer"></div>
  <button
    class="small fold"
    onclick={() => (store.collapsed = !store.collapsed)}
    title={store.collapsed ? "Expand sidebar" : "Collapse sidebar"}
    aria-label={store.collapsed ? "Expand sidebar" : "Collapse sidebar"}
  >
    <Icon body={store.collapsed ? CHEVRONS_RIGHT : CHEVRONS_LEFT} />
  </button>
</nav>

<style>
  .sidebar {
    width: var(--sidebar-w);
    flex: 0 0 var(--sidebar-w);
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px 8px;
    background: var(--surface);
    border-right: 1px solid var(--border);
  }

  .sidebar.collapsed {
    width: var(--sidebar-rail-w);
    flex-basis: var(--sidebar-rail-w);
    align-items: center;
  }

  .tool {
    height: 28px;
    width: 100%;
    text-align: left;
    color: var(--text-secondary);
    background: transparent;
    border-color: transparent;
    border-radius: 4px;
  }

  .tool.active {
    background: var(--border-strong);
    color: var(--text);
  }

  .rail-mark {
    width: 40px;
    height: 40px;
    display: grid;
    place-items: center;
    background: transparent;
    border-color: transparent;
    border-radius: 4px;
    color: var(--text-secondary);
  }

  .rail-mark.active {
    background: var(--border-strong);
    color: var(--text);
  }

  .spacer {
    flex: 1;
  }
</style>
