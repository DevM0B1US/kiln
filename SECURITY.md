# Security — EmberStudio

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
- Weights live in `models/`, never in git, never in the installer. Dev reads
  them through a gitignored symlink or `EMBER_MODELS_DIR`.

## Writes

- Every export goes through `output_path` (numbered, never overwrites) plus
  `confined_path` (safe name, proves the join stayed inside the output dir).
- Bytes land in a temp sibling, then rename. Readers never see half a PNG.
- Symlink and existing destinations are refused before anything is written.
- Parent dirs are created only inside the picked output folder.

## Deps and shell

- Pinned versions, released crates only, no git deps. The single network
  touch in the build is ORT's prebuilt binary fetch.
- No shell commands anywhere. Dialogs go through the Tauri dialog plugin.
- CSP is locked in `tauri.conf.json`: `img-src` allows only self, asset,
  and data (previews are data URLs). Prototype freeze on.

## Licence

RMBG-1.4 is free for personal use only. Selling anything that uses it needs
a deal with BRIA. Never call the weights open source.
