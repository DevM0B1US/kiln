# Architecture — Kiln (Tauri 2 + Svelte 5 + Rust)

Two tools, one window: remove background, template merge. Three layers with
one rule each: the frontend owns queues and window state, the backend owns
pixels and the model, and the bridge between them is exactly the command set
below. Heavy work never runs on the UI thread on either side.

```
crates/kiln-core/      pure pipeline crate. Never imports Tauri. Own tests.
  error.rs             ToolError variants the UI can explain
  job.rs               CancelToken: cooperative, between items
  registry.rs          numbered output names, confined paths
  image_io.rs          size caps before decode, atomic temp-file writes,
                       matte application
  matte.rs             min-max normalise, refine (feather > threshold >
                       dilate), trim — order is load bearing
  merge.rs             cover / fit / stretch, source-over blend
  models.rs            ModelSpec + SHA-256 verify + load-once store
  inference.rs         the only ONNX touchpoint: [-1,1] NCHW preprocess,
                       [1,1,H,W] output check
src-tauri/             Tauri shell. Thin by design.
  src/state.rs         AppState: preview + matte caches keyed by path with
                       mtime guards, batch cancel token
  src/commands.rs      the whole IPC surface (serde DTOs over kiln-core);
                       batches emit progress events; blocking work runs in
                       spawn_blocking
  src/cli.rs           `kiln cut` / `kiln merge` headless harness
  src/lib.rs           builder: plugins, state, command registration
  capabilities/        least-privilege permission set (see SECURITY.md)
src/                   Svelte 5 + TypeScript frontend (runes mode).
  app.css              exact UI.md tokens, dark + light, @font-face
  App.svelte           layout, theme root class, drag-drop wiring, boot
  state/app.svelte.ts  KilnStore: queues, selection, seq counters, batch
                       rules — ported from the original state machine
  lib/bridge/          the ONLY Tauri touchpoint. tauri.ts = real commands;
                       mock.ts = canvas pipeline so the UI runs in a plain
                       browser tab; select.ts picks per environment
  lib/components/      TitleBar, Sidebar, Stage, Panel, ZoomBar,
                       WorkingPill, ContextMenu, EmptyStage, Icon
  lib/icons.ts         inline Lucide marks
  lib/types.ts         DTO shapes mirroring the Rust serde types
src/fonts/             Inter + JetBrains Mono (OFL)
tools/                 gen_test_model.py, gen_icon.py
assets (tests)         crates/kiln-core/tests/isnet_like.onnx —
                       deterministic 64x64 fixture with RMBG's I/O contract
```

## Data flow

Queues hold paths, never pixels. The backend caches downscaled previews and
full-res mattes keyed by path with an mtime guard; refine is maths on the
cached matte, so slider drags never re-run inference.

| Step                              | Command               | Cost                                       |
| --------------------------------- | --------------------- | ------------------------------------------ |
| Pick / drop / paste               | `preview`             | decode + downscale, fast                   |
| First slider touch                | `refine`              | the one inference, matte cached            |
| Every later move (120ms debounce) | `refine`              | matte maths at preview scale               |
| Export batch                      | `export_remove`       | full-res refine + numbered write, per item |
| Merge view                        | `merge_preview`       | decode + composite at template size        |
| Merge batch                       | `export_merge`        | composite + write                          |
| Batch progress                    | `kiln://batch` events | started / item / done                      |

Each command runs inside `spawn_blocking`; locks are held only inside one
blocking section. Batches check the shared `CancelToken` between items,
never mid-image. One bad file records its error and the batch continues.

## Sequence safety

Every preview and work dispatch bumps a counter (`previewSeq`, `workSeq`).
Late responses compare and drop. Fast clicking never shows the wrong image —
same rule as the original, same counters, now in the Svelte store.

## Model

RMBG-1.4, 44 MB, personal use only (BRIA). Loaded once, cached in
`ModelStore`. SHA-256 verified before every load. Weights are never in git
and never in a release artifact. Dev reads them from `models/` or
`KILN_MODELS_DIR`. Missing file means a calm banner and disabled export.

## Browser mode

`mock.ts` implements the same `Bridge` interface with canvas maths and
virtual files: luminance-to-alpha segmentation, approximate refine passes,
virtual export folder. The store, components, and batch rules are identical
in both environments — that is what makes the frontend unit-testable in
node and screenshot-verifiable in a plain tab.

## Cutout order

Feather first, then threshold, then dilate. Trim last. This order is load
bearing for edge quality. See `FEATURES.md`.

## Export fidelity

Preview refine runs at the 1280px preview scale (feather and dilate are
preview pixels); export refines the full-res matte, exactly like the
original. Copy exports the full-res cutout, untrimmed.

## Lint policy

`kiln-core` runs the full pedantic clippy set. `src-tauri` runs
correctness lints only: the command layer is I/O glue whose shapes are
dictated by Tauri's macros and extractors.
