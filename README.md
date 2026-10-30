# Kiln

Fast desktop background remover plus template merger. Drop images in, get
transparent PNGs out. Grey-only UI, dark by default.

Two tools, one window, no uploads: **remove background** segments with
RMBG-1.4 and refines live from a cached matte; **template merge** fits photos
under a transparent PNG template. Everything runs locally.

## Stack

- **Backend**: Rust — [`tauri`](https://tauri.app) 2 shell over
  [`crates/kiln-core`](crates/kiln-core), a framework-free pipeline crate
  (`image` + `ort` for ONNX Runtime).
- **Frontend**: TypeScript + [Svelte 5](https://svelte.dev) (runes) +
  [Vite](https://vite.dev), bundled into the webview. The `bridge/` layer is
  the only Tauri touchpoint; a canvas mock runs the whole UI in a plain
  browser tab, which is also how CI exercises the frontend.
- **One binary per platform. No Node at runtime, no webserver, no installer.**

## Develop

```bash
npm install
npm run tauri dev        # desktop window (needs the Rust toolchain)
npm run dev              # browser-only mode: same UI, mock pipeline
```

Linux dev needs webkit2gtk-4.1 and friends — see `.github/workflows/ci.yml`
for the exact package list.

## Test and check

```bash
cargo test --workspace   # core pipeline + command layer
npm test                 # frontend stores and bridges (vitest)
npm run check            # svelte-check
npm run lint             # eslint
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

## Headless

The same pipeline runs without a window, which is also how CI exercises it:

```bash
kiln cut photo.jpg --out ./out --threshold 128 --feather 1.5 --dilate 1 --trim
kiln merge frame.png --out ./out --fit cover a.png b.png
```

## Model

Needs the RMBG-1.4 weights in a `models/` directory next to the binary —
beside the executable, one or two levels up (`target/{debug,release}`), the
working directory, or `KILN_MODELS_DIR`. Personal use only, BRIA licence —
never commit or ship them. Without them the app shows a calm banner and
disables export.

## Ship

```bash
npm run tauri build
```

One binary per platform; bundling is optional (`--no-bundle` gives the bare
executable). CI builds all three desktop platforms on every push.

## Docs

`FEATURES.md` tools, `UI.md` frozen look, `ARCHITECTURE.md` layout,
`SECURITY.md` threat model, `WORKFLOW.md` day-to-day commands.

## On the inference backend

The UI, the pipeline maths, and the command layer are pure Rust + TypeScript.
Inference runs on ONNX Runtime through [`ort`](https://github.com/pykeio/ort)
— a deliberate exception to "pure Rust everywhere": RMBG-1.4 is an IS-Net
graph whose runtime behaviour we pin by SHA-256, every Rust project in this
niche uses `ort`, and the real weights are licence-gated so a pure-Rust
runtime (tract) cannot be validated against the actual model in CI. The
session is loaded once, cached, and verified before every load.

## License

Copyright (C) 2026 DevM0B1US.

This program is free software: you can redistribute it and/or modify it under
the terms of the **GNU Affero General Public License v3.0**
(AGPL-3.0-or-later), as published by the Free Software Foundation.
See [`LICENSE`](./LICENSE). Hosting a modified version as a network service
requires offering users the complete corresponding source — no closed-source
SaaS.

**Model weights are NOT covered by this license.** The RMBG-1.4 weights are
licensed by BRIA for personal, non-commercial use only and are never
committed to or distributed with this repository. Bundled fonts (Inter,
JetBrains Mono) ship under the SIL Open Font License in `src/fonts/`.
