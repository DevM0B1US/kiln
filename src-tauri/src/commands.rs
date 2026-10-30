//! The Tauri command layer: thin serde-shaped wrappers over `kiln-core`.
//!
//! Every pixel path mirrors the original worker jobs: decode with size caps
//! before allocation, cache previews and mattes by path with an mtime guard,
//! refine at preview scale, export at full resolution. Batches emit progress
//! events and stop between items when cancelled — never mid-image.

use std::{path::Path, path::PathBuf, sync::Arc};

use base64::Engine as _;
use image::{DynamicImage, GenericImageView, ImageFormat};
use kiln_core::{MatteOptions, MergeFit, image_io, matte as matte_math, registry};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::state::{AppState, fit_edge, small_rgba};

/// PNG payload for the stage: base64 plus the source's true dimensions.
#[derive(Debug, Clone, Serialize)]
pub struct PreviewDto {
    /// PNG bytes, base64.
    pub png: String,
    /// Full source width in pixels.
    pub width: u32,
    /// Full source height in pixels.
    pub height: u32,
    /// Preview pixel width (what the payload actually renders).
    pub preview_width: u32,
    /// Preview pixel height.
    pub preview_height: u32,
}

/// One queue entry sent down by the frontend: path plus its display label.
#[derive(Debug, Clone, Deserialize)]
pub struct EntryDto {
    /// Absolute path of the source image.
    pub path: String,
    /// File name shown in the UI.
    pub label: String,
}

/// Where the model files live and whether the file is there.
#[derive(Debug, Clone, Serialize)]
pub struct ModelStatusDto {
    /// Whether the pinned model file exists.
    pub installed: bool,
    /// The directory being used.
    pub root: String,
    /// File name inside that directory.
    pub file_name: String,
}

/// Progress event for the working pill. Mirrors the original batch events.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum BatchEvent {
    /// A batch began.
    BatchStarted {
        /// Items in the batch.
        total: usize,
    },
    /// One item finished, ok or failed.
    BatchItem {
        /// Whether the item succeeded.
        ok: bool,
        /// Output path on success, error text on failure.
        message: String,
    },
    /// The whole batch ended.
    BatchDone {
        /// Items that wrote successfully.
        ok: usize,
        /// Items that failed.
        failed: usize,
        /// Folder the batch wrote into.
        folder: String,
    },
}

const BATCH_EVENT: &str = "kiln://batch";

fn emit_batch(handle: &AppHandle, event: &BatchEvent) {
    if let Err(error) = handle.emit(BATCH_EVENT, event) {
        tracing::error!("could not emit batch event: {error}");
    }
}

/// Encode pixels to a PNG preview payload.
fn preview_payload(
    rgba: &image::RgbaImage,
    source_width: u32,
    source_height: u32,
) -> Result<PreviewDto, String> {
    let mut png = std::io::Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(rgba.clone())
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(PreviewDto {
        png: base64::engine::general_purpose::STANDARD.encode(png.into_inner()),
        width: source_width,
        height: source_height,
        preview_width: rgba.width(),
        preview_height: rgba.height(),
    })
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

/// Where the model lives and whether it is installed.
#[tauri::command]
pub fn model_status(state: State<'_, Arc<AppState>>) -> ModelStatusDto {
    let state = state.inner();
    let spec = kiln_core::models::RMBG_14;
    ModelStatusDto {
        installed: state.store.is_installed(spec.id),
        root: state.store.root().to_string_lossy().into_owned(),
        file_name: spec.file_name.to_owned(),
    }
}

/// Decode + downscale the original preview for a path.
#[tauri::command]
pub async fn preview(path: String, state: State<'_, Arc<AppState>>) -> Result<PreviewDto, String> {
    let state = Arc::clone(state.inner());
    let job = move || -> Result<PreviewDto, String> {
        let path = PathBuf::from(&path);
        let source = image_io::read(&path).map_err(|error| error.to_string())?;
        let (source_width, source_height) = source.dimensions();
        let small = small_rgba(&source);
        state.cache_preview(&path, small.clone());
        preview_payload(&small, source_width, source_height)
    };
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|error| format!("job failed: {error}"))?
}

/// Refine the cached matte at preview scale, segmenting first when needed.
///
/// One command covers the whole slider path: the first call after picking an
/// image pays the one inference; every later call re-refines the cached
/// matte. Semantics match the original `dispatch_refine` + `request_refine`.
#[tauri::command]
pub async fn refine(
    path: String,
    options: MatteOptions,
    state: State<'_, Arc<AppState>>,
) -> Result<PreviewDto, String> {
    let state = Arc::clone(state.inner());
    let job = move || -> Result<PreviewDto, String> {
        let path = PathBuf::from(&path);
        let source = image_io::read(&path).map_err(|error| error.to_string())?;
        let (source_width, source_height) = source.dimensions();
        let matte = state.matte_for(&path, &source)?;
        let small = if let Ok(cached) = state.preview_for(&path) {
            cached
        } else {
            let small = small_rgba(&source);
            state.cache_preview(&path, small.clone());
            Arc::new(small)
        };
        let (width, height) = small.dimensions();
        let shrunk = image::imageops::resize(
            &*matte,
            width,
            height,
            image::imageops::FilterType::Triangle,
        );
        let refined = matte_math::refine(&shrunk, options);
        let mut rgba = image_io::apply_matte(&DynamicImage::ImageRgba8((*small).clone()), &refined)
            .map_err(|error| error.to_string())?;
        if options.trim
            && let Some(cropped) = matte_math::trim(&rgba, &refined)
        {
            rgba = cropped;
        }
        preview_payload(&rgba, source_width, source_height)
    };
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|error| format!("job failed: {error}"))?
}

/// Composite one photo under the template and downscale for display.
#[tauri::command]
pub async fn merge_preview(
    photo: String,
    template: String,
    fit: MergeFit,
) -> Result<PreviewDto, String> {
    let job = move || -> Result<PreviewDto, String> {
        let photo_image = image_io::read(Path::new(&photo)).map_err(|error| error.to_string())?;
        let template_image =
            image_io::read(Path::new(&template)).map_err(|error| error.to_string())?;
        let merged = kiln_core::composite(&photo_image.to_rgba8(), &template_image.to_rgba8(), fit)
            .map_err(|error| error.to_string())?;
        let (source_width, source_height) = merged.dimensions();
        let (fit_width, fit_height) = fit_edge(merged.width(), merged.height());
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
        preview_payload(&small, source_width, source_height)
    };
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|error| format!("job failed: {error}"))?
}

/// Decode + downscale the template for its thumbnail and stage.
#[tauri::command]
pub async fn template_preview(path: String) -> Result<PreviewDto, String> {
    let job = move || -> Result<PreviewDto, String> {
        let image = image_io::read(Path::new(&path)).map_err(|error| error.to_string())?;
        let (source_width, source_height) = image.dimensions();
        let (fit_width, fit_height) = fit_edge(image.width(), image.height());
        let small =
            image.resize_exact(fit_width, fit_height, image::imageops::FilterType::Triangle);
        preview_payload(&small.to_rgba8(), source_width, source_height)
    };
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|error| format!("job failed: {error}"))?
}

/// Export the remove queue: inference (or cached matte) + full-res refine +
/// write, per file, in order. One bad file never stops the batch.
#[tauri::command]
pub async fn export_remove(
    entries: Vec<EntryDto>,
    options: MatteOptions,
    destination: String,
    handle: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let state = Arc::clone(state.inner());
    let cancel = state.take_cancel_token();
    emit_batch(
        &handle,
        &BatchEvent::BatchStarted {
            total: entries.len(),
        },
    );
    let job = move || -> Result<(), String> {
        let output_dir = PathBuf::from(&destination);
        let mut ok = 0;
        let mut failed = 0;
        for entry in &entries {
            if cancel.is_cancelled() {
                break;
            }
            let result = export_one_remove(Path::new(&entry.path), &options, &output_dir, &state);
            match result {
                Ok(path) => {
                    ok += 1;
                    emit_batch(
                        &handle,
                        &BatchEvent::BatchItem {
                            ok: true,
                            message: path,
                        },
                    );
                }
                Err(message) => {
                    failed += 1;
                    emit_batch(&handle, &BatchEvent::BatchItem { ok: false, message });
                }
            }
        }
        emit_batch(
            &handle,
            &BatchEvent::BatchDone {
                ok,
                failed,
                folder: destination,
            },
        );
        Ok(())
    };
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|error| format!("job failed: {error}"))?
}

fn export_one_remove(
    path: &Path,
    options: &MatteOptions,
    output_dir: &Path,
    state: &AppState,
) -> Result<String, String> {
    let source = image_io::read(path).map_err(|error| error.to_string())?;
    let matte = state.matte_for(path, &source)?;
    let refined = matte_math::refine(&matte, *options);
    let mut rgba = image_io::apply_matte(&source, &refined).map_err(|error| error.to_string())?;
    if options.trim
        && let Some(cropped) = matte_math::trim(&rgba, &refined)
    {
        rgba = cropped;
    }
    write_numbered(path, output_dir, &DynamicImage::ImageRgba8(rgba))
}

/// Export the merge queue: composite per file, template size, numbered write.
#[tauri::command]
pub async fn export_merge(
    entries: Vec<EntryDto>,
    template: String,
    fit: MergeFit,
    destination: String,
    handle: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let cancel = state.take_cancel_token();
    emit_batch(
        &handle,
        &BatchEvent::BatchStarted {
            total: entries.len(),
        },
    );
    let job = move || -> Result<(), String> {
        let output_dir = PathBuf::from(&destination);
        let template_pixels = image_io::read(Path::new(&template))
            .map(|image| image.to_rgba8())
            .map_err(|error| error.to_string());
        let mut ok = 0;
        let mut failed = 0;
        for entry in &entries {
            if cancel.is_cancelled() {
                break;
            }
            let result = match &template_pixels {
                Err(message) => Err(message.clone()),
                Ok(template_rgba) => {
                    export_one_merge(Path::new(&entry.path), template_rgba, fit, &output_dir)
                }
            };
            match result {
                Ok(path) => {
                    ok += 1;
                    emit_batch(
                        &handle,
                        &BatchEvent::BatchItem {
                            ok: true,
                            message: path,
                        },
                    );
                }
                Err(message) => {
                    failed += 1;
                    emit_batch(&handle, &BatchEvent::BatchItem { ok: false, message });
                }
            }
        }
        emit_batch(
            &handle,
            &BatchEvent::BatchDone {
                ok,
                failed,
                folder: destination,
            },
        );
        Ok(())
    };
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|error| format!("job failed: {error}"))?
}

fn export_one_merge(
    photo: &Path,
    template_rgba: &image::RgbaImage,
    fit: MergeFit,
    output_dir: &Path,
) -> Result<String, String> {
    let photo_image = image_io::read(photo).map_err(|error| error.to_string())?;
    let merged = kiln_core::composite(&photo_image.to_rgba8(), template_rgba, fit)
        .map_err(|error| error.to_string())?;
    write_numbered(photo, output_dir, &DynamicImage::ImageRgba8(merged))
}

/// Copy the full-res cutout (untrimmed) to the clipboard.
#[tauri::command]
pub async fn copy_cutout(
    path: String,
    options: MatteOptions,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let state = Arc::clone(state.inner());
    let job = move || -> Result<(), String> {
        let source = image_io::read(Path::new(&path)).map_err(|error| error.to_string())?;
        let matte = state.matte_for(Path::new(&path), &source)?;
        let refined = matte_math::refine(&matte, options);
        let rgba = image_io::apply_matte(&source, &refined).map_err(|error| error.to_string())?;
        set_clipboard_rgba(&rgba)
    };
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|error| format!("job failed: {error}"))?
}

/// Copy the raw photo to the clipboard (merge tool).
#[tauri::command]
pub async fn copy_photo(path: String) -> Result<(), String> {
    let job = move || -> Result<(), String> {
        let photo = image_io::read(Path::new(&path)).map_err(|error| error.to_string())?;
        set_clipboard_rgba(&photo.to_rgba8())
    };
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|error| format!("job failed: {error}"))?
}

/// Paste a clipboard image into the queue: it lands on disk as a numbered
/// `pasted.png` inside `dir`, then the frontend queues it like any file.
#[tauri::command]
pub async fn paste_image(dir: String) -> Result<String, String> {
    let job = move || -> Result<String, String> {
        let mut clip = arboard::Clipboard::new().map_err(|error| error.to_string())?;
        let data = clip.get_image().map_err(|error| error.to_string())?;
        let width = u32::try_from(data.width).map_err(|_| "clipboard width overflow".to_owned())?;
        let height =
            u32::try_from(data.height).map_err(|_| "clipboard height overflow".to_owned())?;
        let pixels = image::RgbaImage::from_raw(width, height, data.bytes.into_owned())
            .ok_or_else(|| "Clipboard image is corrupt.".to_owned())?;
        let output_dir = if dir.is_empty() {
            PathBuf::from(".")
        } else {
            PathBuf::from(&dir)
        };
        let numbered = registry::output_path(Path::new("pasted.png"), &output_dir, "png");
        let name = numbered.file_name().map_or_else(
            || "output.png".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        let dest =
            registry::confined_path(&output_dir, &name).map_err(|error| error.to_string())?;
        image_io::write_png(&DynamicImage::ImageRgba8(pixels), &dest)
            .map_err(|error| error.to_string())?;
        Ok(dest.to_string_lossy().into_owned())
    };
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|error| format!("job failed: {error}"))?
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

/// Ask the running batch to stop after the current item.
#[tauri::command]
pub fn cancel_batch(state: State<'_, Arc<AppState>>) {
    state.inner().cancel_batch();
}
