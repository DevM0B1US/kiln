<script lang="ts">
  import { Eraser, Merge, PanelLeftClose, PanelLeftOpen } from "@lucide/svelte";
  import { shell, selectTool } from "$lib/state.svelte";

  const expanded = $derived(!shell.sidebarCollapsed);
</script>

<div
  class="flex h-full flex-none flex-col gap-2 p-2"
  style="width: {expanded ? 208 : 64}px; background: var(--surface);"
>
  {#if expanded}
    <span class="secondary-text px-2 pt-1 text-[11px]">Tools</span>
  {/if}
  <button
    class="btn w-full p-2 {shell.section === 'remove' ? 'btn-action' : 'btn-secondary'}"
    onclick={() => selectTool("remove")}
  >
    <Eraser size={14} />
    {#if expanded}<span>Remove</span>{/if}
  </button>
  <button
    class="btn w-full p-2 {shell.section === 'merge' ? 'btn-action' : 'btn-secondary'}"
    onclick={() => selectTool("merge")}
  >
    <Merge size={14} />
    {#if expanded}<span>Merge</span>{/if}
  </button>
  <div class="flex-1"></div>
  <button class="btn btn-ghost w-full p-2" onclick={() => (shell.sidebarCollapsed = !shell.sidebarCollapsed)}>
    {#if expanded}
      <PanelLeftClose size={14} />
      <span>Collapse</span>
    {:else}
      <PanelLeftOpen size={14} />
    {/if}
  </button>
</div>
