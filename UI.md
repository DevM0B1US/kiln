# UI — EmberStudio visual contract

The look is frozen. It matches the Iced version pixel for pixel. Do not
redesign. No new visual language, no rounded excess, no accent colour.

## Tokens

Dark is default. Bright surrounds hide shadow detail in a matte.

| Role | Dark | Light |
|---|---|---|
| Background | `#141414` | `#f4f4f4` |
| Surface (panels) | `#1c1c1c` | `#ffffff` |
| Surface raised | `#242424` | `#ebebeb` |
| Border | `#2e2e2e` | `#dcdcdc` |
| Border strong | `#3d3d3d` | `#c2c2c2` |
| Text primary | `#e8e8e8` | `#1a1a1a` |
| Text secondary | `#9a9a9a` | `#6b6b6b` |
| Text disabled | `#5c5c5c` | `#a8a8a8` |
| Danger | `#c0574f` | `#b03a30` |
| Checker | `#1e1e1e` / `#2a2a2a` | `#c4c4c4` / `#d8d8d8` |

Danger red is the only accent. Everything else is grey. Tokens live in
`src/app.css` as custom properties. No hex in component files.

## Type

Inter for text, JetBrains Mono for every number, Lucide for every glyph.
Fonts ship in `static/fonts` and load at boot. Panel title 13px semibold,
label 12px, numbers mono, captions 11px secondary, glyphs 12–14px.

## Spacing and shape

4px base. Panel interior 16px, gap between groups 24px, gap between controls
8–12px. Sidebar 208px, folds to a 64px rail. Right panel fixed 264px.
Checkerboard 16px cells. Corners 4px on cards and buttons, square (0px)
elsewhere. Hairline 1px dividers join panes edge to edge.

## Layout

```
+- title bar: EmberStudio | file name | theme _ [] X --+
| banner (only when the model is missing)              |
| sidebar | stage canvas          | fixed panel, 264px   |
| 208px,  | checkerboard,         | image facts          |
| 64px    | hold-to-compare,      | refine groups / fit  |
|         | zoom bar              | Destination, export  |
+---------+-----------------------+----------------------+
```

No OS chrome. The title bar is drawn by the app: drag the file name to move,
double-click to maximise. Sidebar holds one mark per tool plus collapse.
The panel never collapses. No options bar, no status bar, no bottom queue
strip, no divider lines inside the panel. No transitions; state changes cut.

## States

| State | What shows |
|---|---|
| No model | Calm banner under the title bar. Export disabled. |
| No images | Empty stage. Icon tile, `Drop images here`, Browse + Paste. |
| Ready | Preview, live controls, action reads the live count. |
| Working | `n/total` pill with thin progress bar, bottom-right of stage. |
| Failed | Per-item error in the status line. Batch continues. |

## Interaction

OS file and folder pickers, plus window drag-drop (non-image drops are
skipped). Native right-click is disabled everywhere; right-click on the
image opens a custom Copy / Paste / Clear menu (Clear drops the current
image from the queue). Sliders debounce 120ms. The first slider touch runs
the one inference; every move after that re-refines the cached matte. Stage: locked centered viewer, no pan. Wheel, slider, buttons and double-click zoom about the center with smoothing. The Original toggle pins the source photo for compare. Three backdrops sit in the zoom bar: checkerboard, dark, and white.
Dark exposes light halos, white exposes dark fringes — the checkerboard
alone forgives both, so check edges on solid colours before exporting.
