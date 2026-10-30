<script lang="ts">
  /// The fixed right panel: image facts, refine groups / fit, destination,
  /// export, status line. 264px, never collapses.
  import { store } from "$state/app.svelte";
  import { exportLabel, pluralImages } from "$lib/format";

  const queueCount = $derived(store.queue.length);
  const busy = $derived(store.exportingTool !== null);
  const modelOk = $derived(store.modelInstalled === true);

  const removeExportEnabled = $derived(
    modelOk && !busy && queueCount > 0 && store.destination !== null,
  );
  const mergeExportEnabled = $derived(
    !busy && queueCount > 0 && store.template !== null && store.destination !== null,
  );
  const removeCopyEnabled = $derived(!busy && modelOk && store.current !== null);
  const mergeCopyEnabled = $derived(!busy && store.current !== null);

  const fits: Array<{ id: "cover" | "fit" | "stretch"; label: string; caption: string }> = [
    { id: "cover", label: "Cover", caption: "Fills the template, crops what sticks out." },
    { id: "fit", label: "Fit", caption: "Shows the whole photo, transparency around it." },
    { id: "stretch", label: "Stretch", caption: "Forces the photo to template size." },
  ];

  const activeFit = $derived(fits.find((candidate) => candidate.id === store.fit));

  function onFeather(event: Event) {
    const value = Number((event.target as HTMLInputElement).value);
    store.removeOpts.feather = Number.isFinite(value) ? value : 0;
    store.scheduleRefine();
  }

  function onDilate(event: Event) {
    const value = Number((event.target as HTMLInputElement).value);
    store.removeOpts.dilate = Number.isFinite(value) ? Math.max(0, Math.round(value)) : 0;
    store.scheduleRefine();
  }

  function onThreshold(event: Event) {
    const value = Number((event.target as HTMLInputElement).value);
    if (Number.isFinite(value) && value >= 0 && value <= 255) {
      store.removeOpts.threshold = Math.round(value);
      store.scheduleRefine();
    }
  }

  function onTrim(event: Event) {
    store.removeOpts.trim = (event.target as HTMLInputElement).checked;
    store.scheduleRefine();
  }
</script>

<aside class="panel">
  {#if store.tool === "remove"}
    <section>
      <div class="title">Image</div>
      {#if store.current}
        <div class="truncate">{store.current.label}</div>
        {#if store.dims}
          <div class="mono dim">{store.dims.width} × {store.dims.height}</div>
        {/if}
        {#if queueCount > 1}
          <div class="caption">{queueCount} in queue</div>
        {/if}
      {:else}
        <div class="caption">Nothing selected.</div>
      {/if}
    </section>

    <section class="section">
      <div class="title">Refine</div>
      <div class="row">
        <button
          class="small"
          onclick={() => store.setThresholdEnabled(store.removeOpts.threshold === null)}
        >
          Threshold: {store.removeOpts.threshold !== null ? "on" : "off"}
        </button>
        <input
          type="number"
          min="0"
          max="255"
          value={store.removeOpts.threshold ?? 0}
          disabled={store.removeOpts.threshold === null}
          onchange={onThreshold}
          aria-label="Threshold value"
        />
      </div>
      <div class="caption">Hard cut below this alpha. Off keeps the soft edge.</div>

      <div class="row control">
        <label class="caption label" for="feather">Feather</label>
        <input
          type="range"
          id="feather"
          min="0"
          max="10"
          step="0.1"
          value={store.removeOpts.feather}
          oninput={onFeather}
          style="flex:1"
        />
        <input
          type="number"
          min="0"
          max="10"
          step="0.1"
          value={store.removeOpts.feather}
          onchange={onFeather}
          aria-label="Feather value"
        />
      </div>

      <div class="row control">
        <label class="caption label" for="dilate">Dilate</label>
        <input
          type="range"
          id="dilate"
          min="0"
          max="10"
          step="1"
          value={store.removeOpts.dilate}
          oninput={onDilate}
          style="flex:1"
        />
        <input
          type="number"
          min="0"
          max="10"
          step="1"
          value={store.removeOpts.dilate}
          onchange={onDilate}
          aria-label="Dilate value"
        />
      </div>

      <label class="row control check">
        <input type="checkbox" checked={store.removeOpts.trim} onchange={onTrim} />
        <span>Trim to subject</span>
      </label>

      <button class="small" onclick={() => store.resetOptions()}>Reset all</button>
    </section>

    <section class="section">
      <div class="title">Destination</div>
      <div class="caption truncate">
        {store.destination ?? "same folder as the first image"}
      </div>
      <div class="row">
        <button class="small" onclick={() => void store.pickFolder(false)}>Browse folder</button>
        <button
          class="small"
          onclick={() => void store.pickFolder(true)}
          title="Pick the folder and export in one step"
        >
          Save to…
        </button>
      </div>
    </section>

    <section class="section">
      <div class="title">Export</div>
      <button class="export" disabled={!removeExportEnabled} onclick={() => store.dispatchExport()}>
        {exportLabel(queueCount)}
      </button>
      <button
        class="export"
        disabled={!removeCopyEnabled}
        onclick={() => store.copyCurrent()}
        title="Copy the cutout to the clipboard"
      >
        Copy
      </button>
      {#if !modelOk}
        <div class="caption">Export is disabled until the model is back in place.</div>
      {/if}
    </section>
  {:else}
    <section>
      <div class="title">Template</div>
      {#if store.template}
        <div class="truncate">{store.template.split(/[\\/]/).pop() ?? "template"}</div>
        <button class="small" onclick={() => void store.pickTemplate()}>Change template</button>
      {:else}
        <div class="caption">One transparent PNG. The hole shows the photo.</div>
        <button onclick={() => void store.pickTemplate()}>Browse template</button>
      {/if}
    </section>

    <section class="section">
      <div class="title">Fit</div>
      <div class="row">
        {#each fits as candidate (candidate.id)}
          <button
            class="small"
            class:active={store.fit === candidate.id}
            onclick={() => {
              store.fit = candidate.id;
              store.requestMergePreview();
            }}
          >
            {candidate.label}
          </button>
        {/each}
      </div>
      <div class="caption">{activeFit?.caption}</div>
    </section>

    <section class="section">
      <div class="title">Image</div>
      {#if store.current}
        <div class="truncate">{store.current.label}</div>
        {#if queueCount > 1}
          <div class="caption">{queueCount} in queue</div>
        {/if}
      {:else}
        <div class="caption">Nothing selected.</div>
      {/if}
    </section>

    <section class="section">
      <div class="title">Destination</div>
      <div class="caption truncate">
        {store.destination ?? "same folder as the first image"}
      </div>
      <div class="row">
        <button class="small" onclick={() => void store.pickFolder(false)}>Browse folder</button>
        <button
          class="small"
          onclick={() => void store.pickFolder(true)}
          title="Pick the folder and export in one step"
        >
          Save to…
        </button>
      </div>
    </section>

    <section class="section">
      <div class="title">Export</div>
      <button class="export" disabled={!mergeExportEnabled} onclick={() => store.dispatchExport()}>
        {exportLabel(queueCount)}
      </button>
      <button
        class="export"
        disabled={!mergeCopyEnabled}
        onclick={() => store.copyCurrent()}
        title="Copy the photo to the clipboard"
      >
        Copy
      </button>
      {#if store.template === null}
        <div class="caption">Merge preview needs a template first.</div>
      {/if}
    </section>
  {/if}

  <div class="spacer"></div>

  <footer class="status-line">
    {#if store.summary}
      <div class="caption">{store.summary}</div>
    {/if}
    {#if store.status}
      <div class="danger-line">{store.status}</div>
    {/if}
    {#if store.batch === null && store.summary === null && store.status === null}
      <div class="disabled-line">{pluralImages(queueCount)} · batch bar is 20</div>
    {/if}
  </footer>
</aside>

<style>
  .panel {
    width: var(--panel-w);
    flex: 0 0 var(--panel-w);
    display: flex;
    flex-direction: column;
    padding: 16px 16px 8px;
    background: var(--surface);
    border-left: 1px solid var(--border);
    overflow-y: auto;
  }

  .section {
    margin-top: 12px;
  }

  .section > .title {
    margin-bottom: 4px;
  }

  .dim {
    color: var(--text-secondary);
  }

  .control {
    margin-top: 4px;
  }

  .label {
    width: 52px;
    flex: 0 0 52px;
  }

  .check {
    gap: 6px;
  }

  .export {
    width: 100%;
    margin-bottom: 8px;
  }

  .spacer {
    flex: 1;
  }

  .status-line {
    border-top: 1px solid var(--border);
    padding-top: 8px;
    padding-bottom: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .danger-line {
    color: var(--danger);
    font-size: 11px;
    white-space: normal;
  }

  .disabled-line {
    color: var(--text-disabled);
    font-size: 11px;
  }

  button.active {
    background: var(--border-strong);
  }
</style>
