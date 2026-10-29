//! Background jobs and the event channel back to the UI.
//!
//! Every heavy step — decode, inference, refine, composite, export — runs on
//! a worker thread. The UI thread stays at frame rate and applies events
//! tagged with the sequence it dispatched, dropping stale ones. Cancel takes
//! effect between items, never mid-image.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use egui::ColorImage;
use image::{DynamicImage, GenericImageView};

use kiln::{
    CancelToken, MatteOptions, MergeFit, Segmenter, image_io, matte as matte_math, registry,
};

use crate::app::state::{Caches, QueueEntry, preview_image, small_rgba};

/// One-way mail from workers to the UI thread.
pub enum Event {
    /// Decoded + downscaled original preview for a path.
    PreviewReady {
        path: PathBuf,
        original: Arc<ColorImage>,
        width: u32,
        height: u32,
    },
    /// The one inference for a path finished; matte is cached.
    MatteReady { path: PathBuf },
    /// Refined cutout preview for a path.
    RefinedReady {
        path: PathBuf,
        refined: Arc<ColorImage>,
    },
    /// Composite of one photo under the template.
    MergeReady {
        path: PathBuf,
        merged: Arc<ColorImage>,
    },
    /// Decoded template preview.
    TemplateReady {
        path: PathBuf,
        image: Arc<ColorImage>,
    },
    /// Model presence changed or was first checked.
    ModelStatus { installed: bool },
    /// A batch began.
    BatchStarted { total: usize },
    /// One item of the batch finished.
    BatchItem { result: Result<String, String> },
    /// The whole batch ended.
    BatchDone {
        ok: usize,
        failed: usize,
        folder: PathBuf,
    },
    /// A folder picker returned.
    FolderPicked(Option<PathBuf>),
    /// An image picker returned.
    ImagesPicked(Vec<PathBuf>),
    /// The template picker returned.
    TemplatePicked(Option<PathBuf>),
    /// A pasted clipboard image landed on disk.
    Pasted(Result<PathBuf, String>),
    /// A clipboard write finished.
    Copied(Result<(), String>),
}

/// Sender half shared with workers.
pub type EventTx = std::sync::mpsc::Sender<Event>;

/// mtime of a file, or the epoch when it cannot be read.
#[must_use]
pub fn file_modified(path: &Path) -> SystemTime {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

/// Run `f` on a worker thread. A spawn failure is a broken process, not a
/// user error; it is logged and the UI simply stays idle.
pub fn spawn_work(name: &str, job: impl FnOnce() + Send + 'static) {
    let result = std::thread::Builder::new().name(name.to_owned()).spawn(job);
    if let Err(error) = result {
        tracing::error!("could not start {name}: {error}");
    }
}

/// Read an image, enforcing the size caps before a single pixel allocates.
fn read_capped(path: &Path) -> Result<DynamicImage, String> {
    image_io::read(path).map_err(|error| error.to_string())
}

/// Cache the matte for `path`, or fetch the fresh one.
///
/// A stale mtime means the file changed on disk; the old matte is dropped
/// and re-segmented rather than shown.
fn matte_for(
    path: &Path,
    source: &DynamicImage,
    caches: &Caches,
    tx: &EventTx,
) -> Result<Arc<image::GrayImage>, String> {
    let modified = file_modified(path);
    if let Ok(guard) = caches.mattes.lock()
        && let Some(entry) = guard.get(path)
        && entry.modified == modified
    {
        return Ok(Arc::clone(&entry.matte));
    }

    let handle = caches
        .store
        .get(kiln::models::RMBG_14.id)
        .map_err(|error| report_model_status(&error.to_string(), tx))?;
    let matte = Segmenter::new(&handle)
        .matte_for(source, kiln::models::RMBG_14.input_size)
        .map_err(|error| error.to_string())?;
    let matte = Arc::new(matte);
    if let Ok(mut guard) = caches.mattes.lock() {
        guard.insert(
            path.to_path_buf(),
            crate::app::state::CachedMatte {
                modified,
                matte: Arc::clone(&matte),
            },
        );
    }
    Ok(matte)
}

fn report_model_status(error: &str, tx: &EventTx) -> String {
    if error.contains("no model") {
        let _ = tx.send(Event::ModelStatus { installed: false });
    }
    error.to_owned()
}

/// Decode + downscale the original preview for a path.
pub fn request_preview(path: PathBuf, caches: Arc<Caches>, tx: EventTx) {
    spawn_work("kiln-preview", move || {
        let job = || -> Result<(Arc<ColorImage>, u32, u32), String> {
            let source = read_capped(&path)?;
            let (width, height) = source.dimensions();
            let small = small_rgba(&source);
            let image = Arc::new(preview_image(small.width(), small.height(), small.as_raw()));
            if let Ok(mut guard) = caches.previews.lock() {
                guard.insert(path.clone(), Arc::clone(&image));
            }
            Ok((image, width, height))
        };
        match job() {
            Ok((original, width, height)) => {
                let _ = tx.send(Event::PreviewReady {
                    path,
                    original,
                    width,
                    height,
                });
            }
            Err(message) => {
                let _ = tx.send(Event::BatchItem {
                    result: Err(message),
                });
            }
        }
    });
}

/// Segment once for a path and cache the matte.
pub fn request_matte(path: PathBuf, caches: Arc<Caches>, tx: EventTx) {
    spawn_work("kiln-segment", move || {
        let job = || -> Result<Arc<image::GrayImage>, String> {
            let source = read_capped(&path)?;
            matte_for(&path, &source, &caches, &tx)
        };
        match job() {
            Ok(_matte) => {
                let _ = tx.send(Event::MatteReady { path });
            }
            Err(message) => {
                let _ = tx.send(Event::BatchItem {
                    result: Err(message),
                });
            }
        }
    });
}

/// Refine the cached matte at preview scale and apply it to the preview.
///
/// Mirrors the original `refined_preview`: matte shrunk to the preview fit,
/// feather, then threshold, then dilate, applied to the downscaled source,
/// trimmed last. No inference in this path.
pub fn request_refine(path: PathBuf, options: MatteOptions, caches: Arc<Caches>, tx: EventTx) {
    spawn_work("kiln-refine", move || {
        let job = || -> Result<Arc<ColorImage>, String> {
            let modified = file_modified(&path);
            let matte = {
                let guard = caches
                    .mattes
                    .lock()
                    .map_err(|_| "matte cache is poisoned".to_owned())?;
                let entry = guard.get(&path).ok_or_else(|| {
                    "no cached matte; touch a refine slider to segment".to_owned()
                })?;
                if entry.modified != modified {
                    return Err("file changed on disk; reselect the image".to_owned());
                }
                Arc::clone(&entry.matte)
            };
            let preview = {
                let guard = caches
                    .previews
                    .lock()
                    .map_err(|_| "preview cache is poisoned".to_owned())?;
                guard
                    .get(&path)
                    .cloned()
                    .ok_or_else(|| "preview expired; reselect the image".to_owned())?
            };
            let small = rgba_of_color_image(&preview)?;
            let (width, height) = small.dimensions();
            let shrunk = image::imageops::resize(
                &*matte,
                width,
                height,
                image::imageops::FilterType::Triangle,
            );
            let refined = matte_math::refine(&shrunk, options);
            let mut rgba = image_io::apply_matte(&DynamicImage::ImageRgba8(small), &refined)
                .map_err(|error| error.to_string())?;
            if options.trim
                && let Some(cropped) = matte_math::trim(&rgba, &refined)
            {
                rgba = cropped;
            }
            Ok(Arc::new(preview_image(
                rgba.width(),
                rgba.height(),
                rgba.as_raw(),
            )))
        };
        match job() {
            Ok(refined) => {
                let _ = tx.send(Event::RefinedReady { path, refined });
            }
            Err(message) => {
                let _ = tx.send(Event::BatchItem {
                    result: Err(message),
                });
            }
        }
    });
}

/// Composite one photo under the template and downscale for display.
pub fn request_merge(photo: PathBuf, template: PathBuf, fit: MergeFit, tx: EventTx) {
    spawn_work("kiln-merge-preview", move || {
        let job = || -> Result<Arc<ColorImage>, String> {
            let photo_image = read_capped(&photo)?;
            let template_image = read_capped(&template)?;
            let merged = kiln::composite(&photo_image.to_rgba8(), &template_image.to_rgba8(), fit)
                .map_err(|error| error.to_string())?;
            let (fit_width, fit_height) =
                crate::app::state::fit_edge(merged.width(), merged.height());
            let small = if (fit_width, fit_height) == (merged.width(), merged.height()) {
                merged
            } else {
                image::imageops::resize(
                    &merged,
                    fit_width,
                    fit_height,
                    image::imageops::FilterType::Triangle,
                )
            };
            Ok(Arc::new(preview_image(
                small.width(),
                small.height(),
                small.as_raw(),
            )))
        };
        match job() {
            Ok(merged) => {
                let _ = tx.send(Event::MergeReady {
                    path: photo,
                    merged,
                });
            }
            Err(message) => {
                let _ = tx.send(Event::BatchItem {
                    result: Err(message),
                });
            }
        }
    });
}

/// Decode + downscale the template for its panel thumbnail and stage.
pub fn request_template(template: PathBuf, tx: EventTx) {
    spawn_work("kiln-template", move || {
        let job = || -> Result<Arc<ColorImage>, String> {
            let image = read_capped(&template)?;
            let (fit_width, fit_height) =
                crate::app::state::fit_edge(image.width(), image.height());
            let small =
                image.resize_exact(fit_width, fit_height, image::imageops::FilterType::Triangle);
            Ok(Arc::new(preview_image(
                small.width(),
                small.height(),
                small.to_rgba8().as_raw(),
            )))
        };
        match job() {
            Ok(image) => {
                let _ = tx.send(Event::TemplateReady {
                    path: template,
                    image,
                });
            }
            Err(message) => {
                let _ = tx.send(Event::BatchItem {
                    result: Err(message),
                });
            }
        }
    });
}

/// Numbered PNG writer, ported from the original `write_numbered`.
fn write_numbered(input: &Path, output_dir: &Path, image: &DynamicImage) -> Result<String, String> {
    let numbered = registry::output_path(input, output_dir, "png");
    let name = numbered.file_name().map_or_else(
        || "output.png".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let dest = registry::confined_path(output_dir, &name).map_err(|error| error.to_string())?;
    image_io::write_png(image, &dest).map_err(|error| error.to_string())?;
    Ok(dest.to_string_lossy().into_owned())
}

/// Export the remove queue: inference (or cached matte) + full-res refine +
/// write, per file, in order. One bad file never stops the batch.
pub fn export_remove_batch(
    entries: Vec<QueueEntry>,
    options: MatteOptions,
    output_dir: PathBuf,
    caches: Arc<Caches>,
    cancel: CancelToken,
    tx: EventTx,
) {
    let total = entries.len();
    let _ = tx.send(Event::BatchStarted { total });
    spawn_work("kiln-export-remove", move || {
        let mut ok = 0;
        let mut failed = 0;
        for entry in &entries {
            if cancel.is_cancelled() {
                break;
            }
            let result = export_one_remove(entry, &options, &output_dir, &caches, &tx);
            match result {
                Ok(path) => {
                    ok += 1;
                    let _ = tx.send(Event::BatchItem { result: Ok(path) });
                }
                Err(message) => {
                    failed += 1;
                    let _ = tx.send(Event::BatchItem {
                        result: Err(message),
                    });
                }
            }
        }
        let _ = tx.send(Event::BatchDone {
            ok,
            failed,
            folder: output_dir,
        });
    });
}

fn export_one_remove(
    entry: &QueueEntry,
    options: &MatteOptions,
    output_dir: &Path,
    caches: &Caches,
    tx: &EventTx,
) -> Result<String, String> {
    let source = read_capped(&entry.path)?;
    let matte = matte_for(&entry.path, &source, caches, tx)?;
    let refined = matte_math::refine(&matte, *options);
    let mut rgba = image_io::apply_matte(&source, &refined).map_err(|error| error.to_string())?;
    if options.trim
        && let Some(cropped) = matte_math::trim(&rgba, &refined)
    {
        rgba = cropped;
    }
    write_numbered(&entry.path, output_dir, &DynamicImage::ImageRgba8(rgba))
}

/// Export the merge queue: composite per file, template size, numbered write.
pub fn export_merge_batch(
    entries: Vec<QueueEntry>,
    template: PathBuf,
    fit: MergeFit,
    output_dir: PathBuf,
    cancel: CancelToken,
    tx: EventTx,
) {
    let total = entries.len();
    let _ = tx.send(Event::BatchStarted { total });
    spawn_work("kiln-export-merge", move || {
        let mut ok = 0;
        let mut failed = 0;
        let template_rgba = read_capped(&template).map(|image| image.to_rgba8());
        for entry in &entries {
            if cancel.is_cancelled() {
                break;
            }
            let result = match &template_rgba {
                Err(message) => Err(message.clone()),
                Ok(template_pixels) => export_one_merge(entry, template_pixels, fit, &output_dir),
            };
            match result {
                Ok(path) => {
                    ok += 1;
                    let _ = tx.send(Event::BatchItem { result: Ok(path) });
                }
                Err(message) => {
                    failed += 1;
                    let _ = tx.send(Event::BatchItem {
                        result: Err(message),
                    });
                }
            }
        }
        let _ = tx.send(Event::BatchDone {
            ok,
            failed,
            folder: output_dir,
        });
    });
}

fn export_one_merge(
    entry: &QueueEntry,
    template_rgba: &image::RgbaImage,
    fit: MergeFit,
    output_dir: &Path,
) -> Result<String, String> {
    let photo_image = read_capped(&entry.path)?;
    let merged = kiln::composite(&photo_image.to_rgba8(), template_rgba, fit)
        .map_err(|error| error.to_string())?;
    write_numbered(&entry.path, output_dir, &DynamicImage::ImageRgba8(merged))
}

/// Copy the full-res cutout (untrimmed) for the current photo.
pub fn copy_remove(path: PathBuf, options: MatteOptions, caches: Arc<Caches>, tx: EventTx) {
    spawn_work("kiln-copy-remove", move || {
        let job = || -> Result<(), String> {
            let source = read_capped(&path)?;
            let matte = matte_for(&path, &source, &caches, &tx)?;
            let refined = matte_math::refine(&matte, options);
            let rgba =
                image_io::apply_matte(&source, &refined).map_err(|error| error.to_string())?;
            set_clipboard_rgba(&rgba)
        };
        let _ = tx.send(Event::Copied(job()));
    });
}

/// Copy the raw photo for the merge tool.
pub fn copy_photo(path: PathBuf, tx: EventTx) {
    spawn_work("kiln-copy-photo", move || {
        let job = || -> Result<(), String> {
            let photo = read_capped(&path)?;
            let rgba = photo.to_rgba8();
            set_clipboard_rgba(&rgba)
        };
        let _ = tx.send(Event::Copied(job()));
    });
}

fn set_clipboard_rgba(rgba: &image::RgbaImage) -> Result<(), String> {
    let mut clip = arboard::Clipboard::new().map_err(|error| error.to_string())?;
    let data = arboard::ImageData {
        width: rgba.width() as usize,
        height: rgba.height() as usize,
        bytes: rgba.as_raw().clone().into(),
    };
    clip.set_image(data).map_err(|error| error.to_string())
}

/// Paste a clipboard image into the queue: it lands on disk as a numbered
/// `pasted.png` inside `dir`, then the UI queues it like any file.
pub fn paste_image(dir: PathBuf, tx: EventTx) {
    spawn_work("kiln-paste", move || {
        let job = || -> Result<PathBuf, String> {
            let mut clip = arboard::Clipboard::new().map_err(|error| error.to_string())?;
            let data = clip.get_image().map_err(|error| error.to_string())?;
            let width = usize_to_u32(data.width);
            let height = usize_to_u32(data.height);
            let pixels = image::RgbaImage::from_raw(width, height, data.bytes.into_owned())
                .ok_or_else(|| "Clipboard image is corrupt.".to_owned())?;
            let numbered = registry::output_path(Path::new("pasted.png"), &dir, "png");
            let name = numbered.file_name().map_or_else(
                || "output.png".to_owned(),
                |name| name.to_string_lossy().into_owned(),
            );
            let dest = registry::confined_path(&dir, &name).map_err(|error| error.to_string())?;
            image_io::write_png(&DynamicImage::ImageRgba8(pixels), &dest)
                .map_err(|error| error.to_string())?;
            Ok(dest)
        };
        let _ = tx.send(Event::Pasted(job()));
    });
}

/// Native pickers on a worker thread (GTK blocking rules).
pub fn pick_images(tx: EventTx) {
    spawn_work("kiln-pick-images", move || {
        let files = rfd::FileDialog::new()
            .add_filter(
                "Images",
                &["png", "jpg", "jpeg", "webp", "bmp", "gif", "tiff", "tga"],
            )
            .pick_files();
        let _ = tx.send(Event::ImagesPicked(files.unwrap_or_default()));
    });
}

pub fn pick_template(tx: EventTx) {
    spawn_work("kiln-pick-template", move || {
        let file = rfd::FileDialog::new()
            .add_filter("Template PNG", &["png"])
            .pick_file();
        let _ = tx.send(Event::TemplatePicked(file));
    });
}

pub fn pick_folder(tx: EventTx) {
    spawn_work("kiln-pick-folder", move || {
        let folder = rfd::FileDialog::new().pick_folder();
        let _ = tx.send(Event::FolderPicked(folder));
    });
}

/// `usize` pixel dims from arboard, clamped to the u32 image crate max.
#[must_use]
pub fn usize_to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// Convert a cached preview into RGBA pixels for maths.
fn rgba_of_color_image(image: &ColorImage) -> Result<image::RgbaImage, String> {
    let [width, height] = image.size;
    let width = u32::try_from(width).map_err(|_| "preview width overflow".to_owned())?;
    let height = u32::try_from(height).map_err(|_| "preview height overflow".to_owned())?;
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for pixel in &image.pixels {
        rgba.extend_from_slice(&pixel.to_array());
    }
    image::RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| "preview buffer does not match its size".to_owned())
}
