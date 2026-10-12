# Architecture — EmberStudio (Tauri 2 + Svelte 5)

Two tools, one window: remove background, template merge. Heavy work stays in
Rust. The frontend calls commands and renders state. Nothing else.

```
src/                        Svelte 5 + TypeScript + Tailwind 4
  routes/+page.svelte       frame: title bar, banner, rail, stage, panel
  routes/+layout.svelte     css import only
  app.css                   exact grey tokens, fonts, controls
  lib/state.svelte.ts       all state, all flows, batch loops (runes)
  lib/components/           TitleBar Sidebar Stage RemovePanel MergePanel
                            Destination BatchAction
src-tauri/
  ember-core/               pure Rust: matte maths, merge maths, image IO,
                            ONNX inference, model cache, confined paths.
                            Never imports the frontend. Has its own tests.
  src/lib.rs                Tauri setup, state, command list
  src/state.rs              AppState: model store, matte cache, cancel flag
  src/commands.rs           one command per flow step, blocking work inside
  src/images.rs             preview downscale (1280 edge), PNG data URLs
  tests/smoke.rs            real-model pipeline test (needs models/ link)
static/fonts/               Inter + JetBrains Mono (bundled, no web fonts)
models -> ../EmberStudio/models   dev-only symlink, gitignored, never shipped
```

## Data flow

Each tool keeps its own queue of paths, never pixels. Switching tools stashes
one queue and restores the other. Previews travel as PNG data URLs at a 1280px
edge. The full-res matte stays in Rust (`AppState.mattes`, keyed by path with
an mtime guard). Refine is maths on the cached matte. No second inference.

| Step | Command | Cost |
|---|---|---|
| Pick/list | `remove_preview` | decode + downscale, fast |
| Segment | `remove_matte` | ~2.8s inference, caches matte |
| Slider | `refine_preview` | matte maths, debounced 120ms |
| Export one | `export_remove` | inference + full-res refine + write |
| Merge view | `merge_preview` | decode + composite at template size |
| Export one | `export_merge` | composite + write |

Batch loops live in `state.svelte.ts` and call one-file commands in order.
One bad file records its error and continues. Cancel sets the backend flag;
the current file finishes, the next command refuses, the loop stops.

## Selection safety

Every preview dispatch bumps a sequence counter (`previewSeq`, `workSeq`,
`seq`). Late responses compare and drop. Fast clicking never shows the wrong
image. Same rule as the Iced version, same counters.

## Model

RMBG-1.4, 44 MB, personal use only (BRIA). Loaded once, cached in
`ModelStore`. SHA-256 verified before every load. Weights are never in git
and never in the installer. Dev reads them through the `models/` symlink or
`EMBER_MODELS_DIR`. Missing file means a calm banner and disabled export.

## Cutout order

Feather first, then threshold, then dilate. Trim last. This order is load
bearing for edge quality. See `FEATURES.md`.
