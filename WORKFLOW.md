# Workflow — Kiln

Day-to-day commands. The scripts live in `package.json`; everything else is
plain cargo.

## Frontend

```bash
npm run dev          # browser mode, mock pipeline — fastest loop
npm run check        # svelte-check
npm run lint         # eslint
npm test             # vitest (stores, bridge, formatting)
npm run format       # prettier
npm run build        # production bundle into dist/
```

## Rust

```bash
cargo test --workspace        # core + shell tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all               # format (CI checks with --check)
cargo build                   # debug desktop binary
```

## Desktop

```bash
npm run tauri dev             # dev window with hot reload
npm run tauri build           # release build + bundles
npm run tauri build -- --no-bundle   # bare executable only
```

## Headless pipeline

```bash
kiln cut photo.jpg --out ./out --threshold 128 --trim
kiln merge frame.png --out ./out --fit cover a.png b.png
kiln --help
```

`KILN_MODELS_DIR` or a `models/` folder next to the binary supplies the
RMBG-1.4 weights. Without them, `cut` fails with a clear message; the
window shows the calm banner.

## Tools

```bash
python3 tools/gen_test_model.py   # regenerate the 64x64 CI fixture
python3 tools/gen_icon.py         # regenerate src/app-icon.png
npm run icons                     # derive src-tauri/icons/ from it
```

## CI

GitHub Actions (`.github/workflows/ci.yml`) runs on every push and PR:

1. **frontend** — install, lint, format check, svelte-check, vitest, vite build.
2. **core** — rustfmt check, clippy `-D warnings`, workspace tests. No
   system deps needed: the core is framework-free.
3. **desktop** — tauri-action build on Linux, Windows, macOS with the bare
   executables uploaded as artifacts.

Release bundles (installers, dmgs) build on tags via `release.yml`.
