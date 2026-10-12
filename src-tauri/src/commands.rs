use std::path::{Path, PathBuf};

use ember_core::models::RMBG_14;
use ember_core::{MatteOptions, MergeFit, Segmenter};
use image::{DynamicImage, GenericImageView};
use tauri::State;

use crate::images::{encode_png_url, preview_url, small_matte, small_rgba};
use crate::state::{file_modified, AppState, MatteEntry};

#[derive(Debug, serde::Deserialize)]
pub struct MatteOptsDto {
    pub threshold_on: bool,
    pub threshold: u8,
    pub feather: f32,
    pub dilate: u32,
    pub trim: bool,
}

impl MatteOptsDto {
    fn baked(&self) -> MatteOptions {
        MatteOptions {
            threshold: self.threshold_on.then_some(self.threshold),
            feather: self.feather.clamp(0.0, 10.0),
            dilate: self.dilate.min(10),
            trim: self.trim,
        }
    }
}

#[derive(serde::Serialize)]
pub struct ModelStatus {
    pub installed: bool,
    pub message: String,
}

#[derive(serde::Serialize)]
pub struct PreviewOut {
    pub width: u32,
    pub height: u32,
    pub original_url: String,
}

#[derive(serde::Serialize)]
pub struct MatteOut {
    pub width: u32,
    pub height: u32,
    pub original_url: String,
    pub refined_url: String,
}

#[derive(serde::Serialize)]
pub struct RefineOut {
    pub width: u32,
    pub height: u32,
    pub refined_url: String,
}

#[derive(serde::Serialize)]
pub struct MergeOut {
    pub width: u32,
    pub height: u32,
    pub original_url: String,
    pub merged_url: String,
}

#[derive(serde::Serialize)]
pub struct ExportOut {
    pub output: String,
}

fn fit_of_label(fit: &str) -> Result<MergeFit, String> {
    for option in MergeFit::all() {
        if option.label().eq_ignore_ascii_case(fit) {
            return Ok(option);
        }
    }
    Err(format!("`{fit}` is not a fit; use Cover, Fit, or Stretch"))
}

fn refined_preview(
    source: &DynamicImage,
    matte: &image::GrayImage,
    options: MatteOptions,
) -> Result<(String, u32, u32), String> {
    let (width, height) = source.dimensions();
    let small_source = small_rgba(source);
    let small_dynamic = DynamicImage::ImageRgba8(small_source);
    let shrunk = small_matte(matte, width, height);
    let refined = ember_core::matte::refine(&shrunk, options);
    let mut rgba = ember_core::image_io::apply_matte(&small_dynamic, &refined)
        .map_err(|error| error.to_string())?;
    if options.trim {
        if let Some(cropped) = ember_core::matte::trim(&rgba, &refined) {
            rgba = cropped;
        }
    }
    let (out_width, out_height) = rgba.dimensions();
    let url = encode_png_url(rgba.as_raw(), out_width, out_height)?;
    Ok((url, out_width, out_height))
}

fn write_cutout(
    input: &Path,
    output_dir: &Path,
    matte: &image::GrayImage,
    options: MatteOptions,
) -> Result<String, String> {
    let source = ember_core::image_io::read(input).map_err(|error| error.to_string())?;
    let refined = ember_core::matte::refine(matte, options);
    let mut rgba =
        ember_core::image_io::apply_matte(&source, &refined).map_err(|error| error.to_string())?;
    if options.trim {
        if let Some(cropped) = ember_core::matte::trim(&rgba, &refined) {
            rgba = cropped;
        }
    }
    write_numbered(input, output_dir, &DynamicImage::ImageRgba8(rgba))
}

fn write_merged(
    photo: &Path,
    template: &Path,
    fit: MergeFit,
    output_dir: &Path,
) -> Result<String, String> {
    let photo_image = ember_core::image_io::read(photo).map_err(|error| error.to_string())?;
    let template_image = ember_core::image_io::read(template).map_err(|error| error.to_string())?;
    let merged = ember_core::composite(&photo_image.to_rgba8(), &template_image.to_rgba8(), fit)
        .map_err(|error| error.to_string())?;
    write_numbered(photo, output_dir, &DynamicImage::ImageRgba8(merged))
}

fn write_numbered(input: &Path, output_dir: &Path, image: &DynamicImage) -> Result<String, String> {
    let numbered = ember_core::registry::output_path(input, output_dir, "png");
    let name = numbered.file_name().map_or_else(
        || "output.png".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let dest = ember_core::registry::confined_path(output_dir, &name)
        .map_err(|error| error.to_string())?;
    ember_core::image_io::write_png(image, &dest).map_err(|error| error.to_string())?;
    Ok(dest.to_string_lossy().into_owned())
}

struct MatteEntryParts {
    matte: image::GrayImage,
}

fn peek_matte(state: &AppState, path: &str) -> Result<MatteEntryParts, String> {
    let modified = file_modified(Path::new(path));
    let guard = state
        .mattes
        .lock()
        .map_err(|_| "matte cache is poisoned".to_owned())?;
    let entry = guard
        .get(path)
        .ok_or_else(|| "preview expired; reselect the image".to_owned())?;
    if entry.modified != modified {
        return Err("file changed on disk; reselect the image".to_owned());
    }
    Ok(MatteEntryParts {
        matte: entry.matte.clone(),
    })
}

#[tauri::command]
pub fn model_status(state: State<'_, AppState>) -> ModelStatus {
    let installed = state.models.is_installed(RMBG_14.id);
    let message = if installed {
        String::new()
    } else {
        "Model file is missing. Put RMBG-1.4 back in models/.".to_owned()
    };
    ModelStatus { installed, message }
}

#[tauri::command]
pub async fn begin_batch(state: State<'_, AppState>) -> Result<(), String> {
    state.fresh_cancel();
    Ok(())
}

#[tauri::command]
pub async fn cancel_batch(state: State<'_, AppState>) -> Result<(), String> {
    state.stop();
    Ok(())
}

#[tauri::command]
pub fn drop_matte(state: State<'_, AppState>, path: String) {
    if let Ok(mut guard) = state.mattes.lock() {
        guard.remove(&path);
    }
}

#[tauri::command]
pub async fn remove_preview(path: String) -> Result<PreviewOut, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<PreviewOut, String> {
        let source =
            ember_core::image_io::read(Path::new(&path)).map_err(|error| error.to_string())?;
        let (width, height) = source.dimensions();
        let original_url = preview_url(&source)?;
        Ok(PreviewOut {
            width,
            height,
            original_url,
        })
    })
    .await
    .map_err(|_| "background task stopped".to_owned())?
}

#[tauri::command]
pub async fn remove_matte(state: State<'_, AppState>, path: String) -> Result<MatteOut, String> {
    let models = state.models.clone();
    let cache_key = path.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || -> Result<_, String> {
        let source =
            ember_core::image_io::read(Path::new(&path)).map_err(|error| error.to_string())?;
        let (width, height) = source.dimensions();
        let original_url = preview_url(&source)?;
        let handle = models.get(RMBG_14.id).map_err(|error| error.to_string())?;
        let matte = Segmenter::new(&handle)
            .matte_for(&source, RMBG_14.input_size)
            .map_err(|error| error.to_string())?;
        let defaults = MatteOptions {
            threshold: None,
            feather: 0.0,
            dilate: 0,
            trim: false,
        };
        let (refined_url, _, _) = refined_preview(&source, &matte, defaults)?;
        Ok::<_, String>((width, height, original_url, refined_url, matte))
    })
    .await
    .map_err(|_| "background task stopped".to_owned())??;
    let (width, height, original_url, refined_url, matte) = outcome;
    let modified = file_modified(Path::new(&cache_key));
    if let Ok(mut guard) = state.mattes.lock() {
        guard.insert(cache_key, MatteEntry { matte, modified });
    }
    Ok(MatteOut {
        width,
        height,
        original_url,
        refined_url,
    })
}

#[tauri::command]
pub async fn refine_preview(
    state: State<'_, AppState>,
    path: String,
    options: MatteOptsDto,
) -> Result<RefineOut, String> {
    let entry = peek_matte(&state, &path)?;
    let baked = options.baked();
    tauri::async_runtime::spawn_blocking(move || -> Result<RefineOut, String> {
        let source =
            ember_core::image_io::read(Path::new(&path)).map_err(|error| error.to_string())?;
        let (refined_url, width, height) = refined_preview(&source, &entry.matte, baked)?;
        Ok(RefineOut {
            width,
            height,
            refined_url,
        })
    })
    .await
    .map_err(|_| "background task stopped".to_owned())?
}

#[tauri::command]
pub async fn export_remove(
    state: State<'_, AppState>,
    path: String,
    output_dir: String,
    options: MatteOptsDto,
) -> Result<ExportOut, String> {
    if state.cancelled() {
        return Err("operation was cancelled".to_owned());
    }
    let models = state.models.clone();
    let baked = options.baked();
    tauri::async_runtime::spawn_blocking(move || -> Result<ExportOut, String> {
        let source =
            ember_core::image_io::read(Path::new(&path)).map_err(|error| error.to_string())?;
        let handle = models.get(RMBG_14.id).map_err(|error| error.to_string())?;
        let matte = Segmenter::new(&handle)
            .matte_for(&source, RMBG_14.input_size)
            .map_err(|error| error.to_string())?;
        let output = write_cutout(Path::new(&path), Path::new(&output_dir), &matte, baked)?;
        Ok(ExportOut { output })
    })
    .await
    .map_err(|_| "background task stopped".to_owned())?
}

#[tauri::command]
pub async fn merge_preview(
    photo: String,
    template: String,
    fit: String,
) -> Result<MergeOut, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<MergeOut, String> {
        let fit = fit_of_label(&fit)?;
        let photo_image =
            ember_core::image_io::read(Path::new(&photo)).map_err(|error| error.to_string())?;
        let template_image =
            ember_core::image_io::read(Path::new(&template)).map_err(|error| error.to_string())?;
        let template_rgba = template_image.to_rgba8();
        let (width, height) = (template_rgba.width(), template_rgba.height());
        let merged = ember_core::composite(&photo_image.to_rgba8(), &template_rgba, fit)
            .map_err(|error| error.to_string())?;
        let original_url = preview_url(&photo_image)?;
        let merged_url = preview_url(&DynamicImage::ImageRgba8(merged))?;
        Ok(MergeOut {
            width,
            height,
            original_url,
            merged_url,
        })
    })
    .await
    .map_err(|_| "background task stopped".to_owned())?
}

#[tauri::command]
pub async fn export_merge(
    state: State<'_, AppState>,
    photo: String,
    template: String,
    fit: String,
    output_dir: String,
) -> Result<ExportOut, String> {
    if state.cancelled() {
        return Err("operation was cancelled".to_owned());
    }
    tauri::async_runtime::spawn_blocking(move || -> Result<ExportOut, String> {
        let fit = fit_of_label(&fit)?;
        let output = write_merged(
            Path::new(&photo),
            Path::new(&template),
            fit,
            Path::new(&output_dir),
        )?;
        Ok(ExportOut { output })
    })
    .await
    .map_err(|_| "background task stopped".to_owned())?
}

#[tauri::command]
pub async fn copy_remove(
    state: State<'_, AppState>,
    path: String,
    options: MatteOptsDto,
) -> Result<String, String> {
    let entry = peek_matte(&state, &path)?;
    let baked = options.baked();
    let rgba = tauri::async_runtime::spawn_blocking(move || -> Result<image::RgbaImage, String> {
        let source =
            ember_core::image_io::read(Path::new(&path)).map_err(|error| error.to_string())?;
        let refined = ember_core::matte::refine(&entry.matte, baked);
        ember_core::image_io::apply_matte(&source, &refined).map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| "background task stopped".to_owned())??;
    copy_rgba(&state, &rgba)?;
    Ok("Copied to clipboard.".to_owned())
}

#[tauri::command]
pub async fn copy_photo(state: State<'_, AppState>, photo: String) -> Result<String, String> {
    let rgba = tauri::async_runtime::spawn_blocking(move || {
        let photo_image =
            ember_core::image_io::read(Path::new(&photo)).map_err(|error| error.to_string())?;
        Ok::<image::RgbaImage, String>(photo_image.to_rgba8())
    })
    .await
    .map_err(|_| "background task stopped".to_owned())??;
    copy_rgba(&state, &rgba)?;
    Ok("Copied to clipboard.".to_owned())
}

fn copy_rgba(state: &AppState, rgba: &image::RgbaImage) -> Result<(), String> {
    let mut guard = state
        .clipboard
        .lock()
        .map_err(|_| "clipboard is busy".to_owned())?;
    if guard.is_none() {
        *guard = arboard::Clipboard::new()
            .map_err(|error| error.to_string())
            .ok();
    }
    match guard.as_mut() {
        Some(clip) => {
            let data = arboard::ImageData {
                width: rgba.width() as usize,
                height: rgba.height() as usize,
                bytes: rgba.as_raw().clone().into(),
            };
            clip.set_image(data).map_err(|error| error.to_string())
        }
        None => Err("Clipboard is not available.".to_owned()),
    }
}

#[tauri::command]
pub async fn paste_image(output_dir: String) -> Result<ExportOut, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<ExportOut, String> {
        let mut clip = arboard::Clipboard::new().map_err(|error| error.to_string())?;
        let data = clip.get_image().map_err(|error| error.to_string())?;
        let width = data.width as u32;
        let height = data.height as u32;
        let pixels = image::RgbaImage::from_raw(width, height, data.bytes.into_owned())
            .ok_or_else(|| "Clipboard image is corrupt.".to_owned())?;
        let dir = PathBuf::from(&output_dir);
        let numbered = ember_core::registry::output_path(Path::new("pasted.png"), &dir, "png");
        let name = numbered.file_name().map_or_else(
            || "output.png".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        let dest =
            ember_core::registry::confined_path(&dir, &name).map_err(|error| error.to_string())?;
        ember_core::image_io::write_png(&DynamicImage::ImageRgba8(pixels), &dest)
            .map_err(|error| error.to_string())?;
        Ok(ExportOut {
            output: dest.to_string_lossy().into_owned(),
        })
    })
    .await
    .map_err(|_| "background task stopped".to_owned())?
}
