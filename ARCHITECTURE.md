# Architecture — Kiln (egui + Rust, no webview)

Two tools, one window: remove background, template merge. Heavy work stays in
worker threads. The UI thread owns state, pumps events, and drops stale ones.
Nothing else.

```
src/
  lib.rs                core contracts: errors, image IO, matte maths,
                        merge maths, ONNX inference, model cache, safe paths.
                        Never imports the GUI. Has its own tests.
  error.rs              ToolError variants the UI can explain
  job.rs                CancelToken: cooperative, between items
  registry.rs           numbered output names, confined paths
  image_io.rs           size caps before decode, atomic temp-file writes,
                        matte application
  matte.rs              min-max normalise, refine (feather > threshold >
                        dilate), trim — order is load bearing
  merge.rs              cover / fit / stretch, source-over blend
  models.rs             ModelSpec + SHA-256 verify + load-once store
  inference.rs          the only ONNX touchpoint: [-1,1] NCHW preprocess,
                        [1,1,H,W] output check
  main.rs               window entry + `kiln cut` / `kiln merge` harness
  app/
    mod.rs              KilnApp: event pump, selection safety, dispatch
    state.rs            tool queues (paths, never pixels), matte cache with
                        mtime guard, preview seq / work seq counters
    tasks.rs            worker threads + Event channel (mpsc)
    theme.rs            exact UI.md tokens, dark + light
    fonts.rs            Inter + JetBrains Mono, embedded
    chrome.rs           title bar, banner, sidebar
    stage.rs            canvas, checkerboard, zoom bar, context menu
    panel.rs            fixed 264px right panel
assets/fonts/           Inter + JetBrains Mono (OFL), embedded via include_bytes
assets/test/            isnet_like.onnx — deterministic 64x64 fixture with
                        RMBG's input/output contract, for CI inference tests
```

## Data flow

Each tool keeps its own queue of paths, never pixels. Switching tools moves
the window's attention; queues stay put. Previews are `ColorImage` textures
at a 1280px edge. The full-res matte stays in `Caches.mattes`, keyed by path
with an mtime guard. Refine is maths on the cached matte. No second
inference.

| Step                    | Worker                | Cost                                         |
| ----------------------- | --------------------- | -------------------------------------------- |
| Pick / drop / paste     | `request_preview`     | decode + downscale, fast                     |
| Segment (once)          | `request_matte`       | ~2.8s inference, caches matte                |
| Slider (120ms debounce) | `request_refine`      | matte maths at preview scale                 |
| Export batch            | `export_remove_batch` | inference or cache + full-res refine + write |
| Merge view              | `request_merge`       | decode + composite at template size          |
| Merge batch             | `export_merge_batch`  | composite + write                            |

Batch loops live in `tasks.rs` on one worker thread, sending
`BatchItem`/`BatchDone` events in order. One bad file records its error and
continues. Cancel sets the `CancelToken`; the current file finishes, the next
item refuses, the loop stops.

## Selection safety

Every dispatch bumps a sequence counter (`preview_seq`, `work_seq`). Late
responses compare and drop. Fast clicking never shows the wrong image. Same
rule as the original, same counters.

## Model

RMBG-1.4, 44 MB, personal use only (BRIA). Loaded once, cached in
`ModelStore`. SHA-256 verified before every load. Weights are never in git
and never in a release artifact. Dev reads them from `models/` or
`KILN_MODELS_DIR`. Missing file means a calm banner and disabled export.

## Cutout order

Feather first, then threshold, then dilate. Trim last. This order is load
bearing for edge quality. See `FEATURES.md`.

## Export fidelity

Preview refine runs at the 1280px preview scale (feather and dilate are
preview pixels); export refines the full-res matte, exactly like the
original. Copy exports the full-res cutout, untrimmed.
