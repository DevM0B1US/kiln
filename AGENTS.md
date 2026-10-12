# AGENTS.md — rules for AI agents (Tauri build)

EmberStudio, Tauri 2 + Svelte 5 + Rust. Read this before writing code.
Details live next to it: `ARCHITECTURE.md` the layout, `FEATURES.md` the
tools, `UI.md` the frozen look, `SECURITY.md` the threat model,
`WORKFLOW.md` setup and gates.

## 0. How to work

1. Do not write code until the user says go.
2. Do not add features nobody asked for. Two tools ship: remove, merge.
3. Plain English. Short replies. Result, not the story.
4. No code comments in new frontend code. Names carry meaning.
5. Never claim untested output works. Run it or say so.

## 1. Ownership

- `src-tauri/ember-core/` owns maths, errors, image IO, model cache, batch
  primitives. It never imports the frontend.
- `src-tauri/src/commands.rs` owns the command surface. Thin: read input,
  blocking work, small data back.
- `src/lib/state.svelte.ts` owns all frontend state and flows. Components
  render state and call flows. No inference or pixel work in JavaScript.
- Components stay small. Reuse `BatchAction` and `Destination`.

## 2. Locked choices

- ONNX Runtime (`ort`) for inference. No other runner.
- RMBG-1.4 for remove only. Weights never in git or installers.
- Queue holds paths, never images. Previews are 1280-edge data URLs.
- Matte stays in Rust. Refine order: feather, threshold, dilate, trim.
- UI look is frozen per `UI.md`. Grey only, danger red the sole accent.

## 3. Never

- Commit secrets, weights, or push without being asked.
- `unwrap`/`expect`/`panic` in backend paths. `Command::new` anywhere.
- Overwrite outputs. Skip confined checks. Trust user files.
- Put hex colours in components. Add settings with one value.
- Swallow errors. One bad file never aborts a batch.

## 4. Flows

- Sequence-guard every async answer. Late responses drop.
- Slider refines debounce at 120ms. No inference in the slider path.
- Stage: scroll viewport, image never moves. Drag scrolls the view,
  wheel/buttons/slider/double-click set shell.zoom directly, view stays
  centered. Original toggle compares, backdrops are checker/dark/light
  for halo inspection. The checkerboard is a bitmap tile clipped to the
  image rect inside a 1px boundary frame.
- Matte loads lazily on the first refine touch; selecting an image shows
  the original instantly with no inference.
- Cancel finishes the current image, then stops.
- Batch summary reads `N ok, M failed · folder`.

## 5. Done means

Gates green (`WORKFLOW.md`), both themes work, all five states reachable,
new behaviour has a test, the user saw real output.
