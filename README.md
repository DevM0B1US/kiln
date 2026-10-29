# Kiln

Fast desktop background remover plus template merger. Drop images in, get
transparent PNGs out. Grey-only UI, dark by default.

Kiln is the pure-Rust rewrite of EmberStudio (Tauri 2 + Svelte 5): one
language end to end, no webview, no Node, one binary. The UI is
[egui](https://github.com/emilk/egui) 0.37; the pipeline maths and the frozen
visual contract are carried over unchanged, so cutout quality is identical.

## Run

```bash
cargo run --release
```

Needs the RMBG-1.4 weights in a `models/` directory next to the binary —
beside the executable, one or two levels up (`target/{debug,release}`), the
working directory, or `KILN_MODELS_DIR`. Personal use only, BRIA licence —
never commit or ship them. Without them the app shows a calm banner and
disables export.

## Headless

The same pipeline runs without a window, which is also how CI exercises it:

```bash
kiln cut photo.jpg --out ./out --threshold 128 --feather 1.5 --dilate 1 --trim
kiln merge frame.png --out ./out --fit cover a.png b.png
```

## Ship

```bash
cargo build --release
```

One binary per platform. No installer steps, no bundler.

## Features

Two tools. Nothing else. See `FEATURES.md`.

- **Remove background** — RMBG-1.4 segmentation with refine sliders
  (threshold, feather, dilate, trim), live re-refinement from a cached
  matte, batch export, clipboard in and out.
- **Template merge** — one transparent PNG template, photos fitted under it
  (cover / fit / stretch), batch export at template size.

## Docs

`FEATURES.md` tools, `UI.md` frozen look, `ARCHITECTURE.md` layout,
`SECURITY.md` threat model.

## On the inference backend

The image pipeline, matte maths, merge maths, and the whole UI are pure Rust
(`image`, `egui`). Inference runs on ONNX Runtime through
[`ort`](https://github.com/pykeio/ort) — a deliberate exception to "pure
Rust everywhere": RMBG-1.4 is an IS-Net graph whose runtime behaviour we pin
by SHA-256, every Rust project in this niche uses `ort`, and the real weights
are licence-gated so a pure-Rust runtime (tract) cannot be validated against
the actual model in CI. The session is loaded once, cached, and verified
before every load.

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
JetBrains Mono) ship under the SIL Open Font License in `assets/fonts/`.
