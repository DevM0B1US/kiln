# Security — Kiln

This app opens hostile files and runs a bundled model. Nothing phones home.

## Images

- Dimensions are probed before decoding. Past 8192px edge or 50MP is refused
  before a single pixel is allocated. Both tools ride on `image_io::read`,
  so both are covered.
- Only 8 extensions are accepted in pickers and drops. Decode errors never
  crash; they become status strings, and batches continue.
- Clipboard paste rejects corrupt pixels.

## Model

- SHA-256 is verified against the pinned hash before every load. A swapped
  file is an error, never garbage mattes.
- Weights live in `models/`, never in git, never in a release artifact. Dev
  reads them from `models/` or `KILN_MODELS_DIR`.

## Writes

- Every export goes through `output_path` (numbered, never overwrites) plus
  `confined_path` (safe name, proves the join stayed inside the output dir).
- Bytes land in a temp sibling, then rename. Readers never see half a PNG.
- Symlink and existing destinations are refused before anything is written.
- Parent dirs are created only inside the picked output folder.

## Tauri surface

- Capabilities are least-privilege (`src-tauri/capabilities/default.json`):
  window controls for the custom title bar, `dialog:open` for pickers, and
  nothing else — no shell, no fs plugin, no http, no updater.
- The CSP forbids remote code: `default-src 'self'`, images limited to
  `self data: blob:` (previews), styles self + inline (Svelte), fonts self.
- Only the eleven commands registered in `lib.rs` exist. Each one validates
  paths through the core's size caps and confined writes.
- Batch progress rides one event channel (`kiln://batch`); nothing inbound
  executes as code.

## Deps and shell

- Pinned versions, released crates and npm packages only, no git deps. The
  single network touch in the build is ORT's prebuilt binary fetch. The app
  itself makes zero network requests — no telemetry, no update checks,
  nothing.
- No shell commands anywhere. Dialogs go through the dialog plugin; the
  clipboard through `arboard` (image bytes only, both directions).
- All window state lives in one process; commands run on blocking workers
  and report through events, and late responses are dropped by sequence
  counters in the store.

## Licence

RMBG-1.4 is free for personal use only. Selling anything that uses it needs
a deal with BRIA. Never call the weights open source.
