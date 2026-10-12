# Features — EmberStudio

Two tools. Nothing else.

## Remove background

Drop images in and the original shows instantly. Nothing runs until you
touch a refine slider or hit export. The first slider touch segments once
with RMBG-1.4, caches the matte, and every move after re-refines it live.
Exports run the full pipeline per file in batch.

- Inputs: png jpg jpeg webp bmp gif tiff tga. Past 8192px edge or 50MP is
  refused before decoding. Corrupt files report and continue.
- Refine controls: Threshold (0–255, starts off), Feather (0–10, softens the
  edge before the cut), Dilate (0–10px, grows the kept area), Trim to subject
  (crops output to the subject box), Reset all.
- Sliders re-preview through a 120ms debounce. No inference in the slider
  path. Typing exact values works; bad input is ignored.
- Hold the image to compare with the original.
- Export writes numbered PNGs (`photo.png`, `photo-1.png`, …) into the picked
  folder. Default folder is the first image's parent. Save to… picks the
  folder and exports in one step.
- Copy sends the cutout to the clipboard. Paste reads a clipboard image into
  the queue as `pasted.png` (numbered).

## Template merge

One transparent template PNG plus photos. Each photo is scaled under the
template per the fit, then the template draws over it. Output size always
equals template size. Batch export, same numbering and folder rules.

- Fits: Cover (fills, crops), Fit (shows all, transparency around), Stretch
  (ignores shape).
- Merge preview needs a template first. Copy sends the photo to the
  clipboard.

## Batch rules (both tools)

- 20 images through the window is the v1 bar.
- One bad file never stops the batch. Its error sits in the status line.
- Cancel takes effect after the current image, never mid-image.
- Clearing the queue mid-export ends the batch with
  "Queue changed during export."
- Switching tools mid-batch is blocked with "Wait for the batch to finish."
- Summary reads `N ok, M failed · folder`.
