# EmberStudio (Tauri 2 + Svelte 5)

Fast desktop background remover plus template merger. Drop images in, get
transparent PNGs out. Grey-only UI, dark by default.

## Run

```bash
pnpm install
ln -s ../EmberStudio/models models   # dev weights link, gitignored
pnpm tauri dev
```

Needs the RMBG-1.4 weights next to the app in `models/` (personal use only,
BRIA licence — never commit or ship them). Without them the app runs with a
missing-model banner.

## Ship

```bash
pnpm tauri build
```

## Docs

`AGENTS.md` rules, `ARCHITECTURE.md` layout, `FEATURES.md` tools,
`UI.md` frozen look, `SECURITY.md` threat model, `WORKFLOW.md` gates.
