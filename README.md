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

## License

Copyright (C) 2026 DevM0B1US.

This program is free software: you can redistribute it and/or modify it under
the terms of the **GNU Affero General Public License v3.0** (AGPL-3.0-or-later),
as published by the Free Software Foundation. See [`LICENSE`](./LICENSE).

What this means in practice:

- You may use, study, modify, and redistribute this code.
- If you modify it and let users interact with it over a network (e.g. host it
  as a web service or SaaS), you **must** offer those users the complete,
  corresponding source code under AGPL-3.0 — including your modifications.
  You cannot turn EmberStudio into a closed-source service.
- Derivative works must likewise be released under AGPL-3.0.

**Model weights are NOT covered by this license.** The RMBG-1.4 segmentation
weights (`models/`, gitignored) are licensed by BRIA for personal, non-
commercial use only and are never committed to or distributed with this
repository. Obtaining and using those weights is subject to BRIA's own terms,
which are separate from and unaffected by the AGPL-3.0 grant on this code.
