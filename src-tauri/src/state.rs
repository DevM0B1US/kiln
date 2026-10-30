//! Backend state shared by every command.
//!
//! Queues and window state live in the frontend; the backend caches pixels:
//! downscaled previews and full-res mattes, keyed by path with an mtime
//! guard, exactly like the original caches. Heavy work runs inside
//! `spawn_blocking`; locks are only ever held inside one blocking section.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};

use image::{DynamicImage, GenericImageView, GrayImage, RgbaImage};
use kiln_core::{CancelToken, ModelStore};

/// Longest edge a preview may have. Matches the original preview pipeline;
/// full-res export never goes through this.
pub const PREVIEW_EDGE: u32 = 1280;

/// Longest-edge fit for previews, ported from the original `images.rs`.
#[must_use]
pub fn fit_edge(width: u32, height: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= PREVIEW_EDGE || longest == 0 {
        return (width, height);
    }
    let scale = f64::from(PREVIEW_EDGE) / f64::from(longest);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (fit_width, fit_height) = (
        (f64::from(width) * scale).round() as u32,
        (f64::from(height) * scale).round() as u32,
    );
    (fit_width.max(1), fit_height.max(1))
}

/// Downscale a decoded source to the preview edge, as RGBA pixels.
#[must_use]
pub fn small_rgba(source: &DynamicImage) -> RgbaImage {
    let (width, height) = source.dimensions();
    let (fit_width, fit_height) = fit_edge(width, height);
    if (fit_width, fit_height) == (width, height) {
        return source.to_rgba8();
    }
    source
        .resize_exact(fit_width, fit_height, image::imageops::FilterType::Triangle)
        .to_rgba8()
}

/// mtime of a file, or the epoch when it cannot be read.
#[must_use]
pub fn file_modified(path: &Path) -> SystemTime {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

/// A cached preview source, guarded by the file's mtime at decode time.
pub struct CachedPreview {
    /// File mtime when the preview was produced.
    pub modified: SystemTime,
    /// Downscaled source pixels.
    pub rgba: Arc<RgbaImage>,
}

/// A cached full-res matte, guarded by the file's mtime at segment time.
pub struct CachedMatte {
    /// File mtime when the matte was produced.
    pub modified: SystemTime,
    /// The matte at original image resolution.
    pub matte: Arc<GrayImage>,
}

/// Cache bound. A working set past 20 images is out of scope for v1; this
/// only stops an unbounded map when someone churns through hundreds.
const CACHE_CAP: usize = 64;

/// Drop the oldest entries past the cap. Insertion order is good enough.
fn trim_cache<V>(map: &mut HashMap<PathBuf, V>) {
    while map.len() > CACHE_CAP {
        let oldest = map.keys().next().cloned();
        match oldest {
            Some(key) => {
                map.remove(&key);
            }
            None => break,
        }
    }
}

/// Model store plus the preview and matte caches plus the batch cancel flag.
pub struct AppState {
    /// Load-once ONNX session store.
    pub store: ModelStore,
    /// Downscaled previews keyed by path.
    pub previews: Mutex<HashMap<PathBuf, CachedPreview>>,
    /// Full-res mattes keyed by path.
    pub mattes: Mutex<HashMap<PathBuf, CachedMatte>>,
    /// Cooperative cancel for the running batch.
    pub cancel: Mutex<CancelToken>,
}

impl AppState {
    /// Build the state from a model root.
    #[must_use]
    pub fn new(store: ModelStore) -> Self {
        Self {
            store,
            previews: Mutex::new(HashMap::new()),
            mattes: Mutex::new(HashMap::new()),
            cancel: Mutex::new(CancelToken::new()),
        }
    }

    /// Fetch the cached preview, honouring the mtime guard.
    ///
    /// # Errors
    ///
    /// Errors when the cache is poisoned or no fresh preview exists.
    pub fn preview_for(&self, path: &Path) -> Result<Arc<RgbaImage>, String> {
        let modified = file_modified(path);
        let guard = self
            .previews
            .lock()
            .map_err(|_| "preview cache is poisoned".to_owned())?;
        let entry = guard
            .get(path)
            .ok_or_else(|| "preview expired; reselect the image".to_owned())?;
        if entry.modified != modified {
            return Err("file changed on disk; reselect the image".to_owned());
        }
        Ok(Arc::clone(&entry.rgba))
    }

    /// Fetch the cached matte, honouring the mtime guard.
    ///
    /// # Errors
    ///
    /// Errors when the cache is poisoned, no fresh matte exists, or the
    /// model is missing / fails to load.
    pub fn matte_for(&self, path: &Path, source: &DynamicImage) -> Result<Arc<GrayImage>, String> {
        let modified = file_modified(path);
        if let Ok(guard) = self.mattes.lock()
            && let Some(entry) = guard.get(path)
            && entry.modified == modified
        {
            return Ok(Arc::clone(&entry.matte));
        }

        let handle = self
            .store
            .get(kiln_core::models::RMBG_14.id)
            .map_err(|error| error.to_string())?;
        let matte = kiln_core::Segmenter::new(&handle)
            .matte_for(source, kiln_core::models::RMBG_14.input_size)
            .map_err(|error| error.to_string())?;
        let matte = Arc::new(matte);
        if let Ok(mut guard) = self.mattes.lock() {
            guard.insert(
                path.to_path_buf(),
                CachedMatte {
                    modified,
                    matte: Arc::clone(&matte),
                },
            );
            trim_cache(&mut guard);
        }
        Ok(matte)
    }

    /// Store a preview for a path.
    pub fn cache_preview(&self, path: &Path, rgba: RgbaImage) {
        if let Ok(mut guard) = self.previews.lock() {
            guard.insert(
                path.to_path_buf(),
                CachedPreview {
                    modified: file_modified(path),
                    rgba: Arc::new(rgba),
                },
            );
            trim_cache(&mut guard);
        }
    }

    /// Flip the batch cancel flag.
    pub fn cancel_batch(&self) {
        if let Ok(guard) = self.cancel.lock() {
            guard.cancel();
        }
    }

    /// A fresh cancel token for a new batch, replacing the old one.
    pub fn take_cancel_token(&self) -> CancelToken {
        let mut guard = match self.cancel.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let token = CancelToken::new();
        *guard = token.clone();
        token
    }
}
