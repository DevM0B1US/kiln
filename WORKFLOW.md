# Workflow — EmberStudio

## Setup

```bash
pnpm install
ln -s ../EmberStudio/models models   # dev weights link, gitignored
pnpm tauri dev
```

`EMBER_MODELS_DIR` overrides the model folder. Without weights the app runs
with the missing-model banner; everything except inference still works.

## Daily gates

```bash
pnpm check                  # Svelte + TypeScript
cargo fmt --check -p emberstudio-tauri   # from src-tauri
cargo clippy --lib --all-targets         # from src-tauri, zero warnings
cargo test -p ember-core                 # 36 maths tests, offline
cargo test --test smoke                  # real-model pipeline, needs models/
pnpm build                  # frontend bundle
```

## Adding a tool

1. Maths plus tests in `ember-core` first.
2. One command per step in `src-tauri/src/commands.rs`. Blocking work goes
   in `spawn_blocking`. Small data back: paths, dims, status, data URLs.
3. State plus flow in `src/lib/state.svelte.ts`. Sequence-guard every async
   answer. Reuse the batch loop shape, never re-implement it.
4. Panel plus rail mark last. Numbers in mono, no hex in components.

## Rules

- Do not write code until the user says go.
- Do not add features nobody asked for.
- New crate or npm package needs the user's okay first.
- Never commit or push without the user saying so. Never `git add -A`.
- Never put model weights in git or the installer.
- Behaviour change needs a test. If you did not run it, it is not done.
