//! Window state and shared caches.
//!
//! Queues hold paths, never pixels (same rule as the original). Full-res
//! mattes live in [`Caches`], keyed by path with an mtime guard. Refine is
//! maths on the cached matte — no second inference.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

use egui::ColorImage;
use image::GenericImageView;
use image::GrayImage;

use crate::app::tasks::Event;
use crate::app::theme::{Backdrop, ThemeChoice};
use kiln::{CancelToken, MergeFit, ModelStore};

/// Longest edge a preview texture may have. Matches the original preview
/// pipeline; the full-res export never goes through this.
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

/// Build a preview texture image from decoded RGBA pixels.
#[must_use]
pub fn preview_image(width: u32, height: u32, rgba: &[u8]) -> ColorImage {
    ColorImage::from_rgba_unmultiplied([width as usize, height as usize], rgba)
}

/// Downscale a decoded source to the preview edge, as RGBA bytes.
#[must_use]
pub fn small_rgba(source: &image::DynamicImage) -> image::RgbaImage {
    let (width, height) = source.dimensions();
    let (fit_width, fit_height) = fit_edge(width, height);
    if (fit_width, fit_height) == (width, height) {
        return source.to_rgba8();
    }
    source
        .resize_exact(fit_width, fit_height, image::imageops::FilterType::Triangle)
        .to_rgba8()
}

/// One entry in a tool queue: a path and its display name.
#[derive(Debug, Clone)]
pub struct QueueEntry {
    /// Absolute path of the source image.
    pub path: PathBuf,
    /// File name shown in the UI.
    pub label: String,
}

impl QueueEntry {
    #[must_use]
    pub fn from_path(path: PathBuf) -> Self {
        let label = path.file_name().map_or_else(
            || "image".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        Self { path, label }
    }
}

/// Extensions accepted by drag-drop and Browse, lowercase without dot.
pub const INPUT_EXTENSIONS: [&str; 8] = ["png", "jpg", "jpeg", "webp", "bmp", "gif", "tiff", "tga"];

/// Whether a path looks like an image we can decode. Non-image drops are
/// skipped silently.
#[must_use]
pub fn is_image_path(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|ext| {
        let ext = ext.to_string_lossy().to_ascii_lowercase();
        INPUT_EXTENSIONS.contains(&ext.as_str())
    })
}

/// A cached full-res matte, guarded by the file's mtime at segment time.
pub struct CachedMatte {
    /// File mtime when the matte was produced.
    pub modified: SystemTime,
    /// The matte at original image resolution.
    pub matte: ArcMatte,
}

/// Shared matte handle across threads.
pub type ArcMatte = std::sync::Arc<GrayImage>;

/// Model store plus the matte and preview caches, shared with workers.
pub struct Caches {
    /// Load-once ONNX session store.
    pub store: ModelStore,
    /// Full-res mattes keyed by path.
    pub mattes: Mutex<HashMap<PathBuf, CachedMatte>>,
    /// Original preview images (1280 edge) keyed by path.
    pub previews: Mutex<HashMap<PathBuf, std::sync::Arc<ColorImage>>>,
}

impl Caches {
    #[must_use]
    pub fn new(store: ModelStore) -> Self {
        Self {
            store,
            mattes: Mutex::new(HashMap::new()),
            previews: Mutex::new(HashMap::new()),
        }
    }
}

/// Refine controls for the remove tool.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RemoveOptions {
    /// Below this the pixel is dropped entirely. `None` keeps the soft edge;
    /// the slider starts off.
    pub threshold: Option<u8>,
    /// Gaussian sigma applied before thresholding.
    pub feather: f32,
    /// Grow the solid region by this many pixels.
    pub dilate: u32,
    /// Crop output to the subject box.
    pub trim: bool,
}

impl RemoveOptions {
    /// Back to factory defaults.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Which tool owns the window right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    /// Remove background.
    #[default]
    Remove,
    /// Template merge.
    Merge,
}

impl Tool {
    /// Sidebar mark text.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Remove => "Remove background",
            Self::Merge => "Template merge",
        }
    }

    /// Short mark for the collapsed rail.
    #[must_use]
    pub fn mark(self) -> &'static str {
        match self {
            Self::Remove => "C",
            Self::Merge => "T",
        }
    }
}

/// Progress through a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchProgress {
    /// Items finished, ok or failed.
    pub done: usize,
    /// Items in the batch.
    pub total: usize,
}

/// Everything the window shows and edits. Owned by the UI thread; workers
/// only receive clones of [`Caches`], the cancel token, and the event sender.
#[allow(clippy::struct_excessive_bools)]
pub struct KilnState {
    /// Active tool.
    pub tool: Tool,
    /// Remove-background queue.
    pub remove_queue: Vec<QueueEntry>,
    /// Template-merge queue.
    pub merge_queue: Vec<QueueEntry>,
    /// Selected index per tool.
    pub selected: [Option<usize>; 2],
    /// Refine controls.
    pub remove_opts: RemoveOptions,
    /// Template fit.
    pub fit: MergeFit,
    /// The one transparent template PNG.
    pub template: Option<PathBuf>,
    /// Export folder; defaults to the first image's parent.
    pub destination: Option<PathBuf>,
    /// Latest status line (per-item error or info).
    pub status: Option<String>,
    /// Batch summary: `N ok, M failed · folder`.
    pub summary: Option<String>,
    /// Live batch progress.
    pub batch: Option<BatchProgress>,
    /// Cooperative cancel for the running batch.
    pub cancel: CancelToken,
    /// Bumped on every preview dispatch; late events compare and drop.
    pub preview_seq: u64,
    /// Bumped on every work dispatch (segment, refine, merge).
    pub work_seq: u64,
    /// Sidebar folded to the 64px rail.
    pub collapsed: bool,
    /// Palette.
    pub theme: ThemeChoice,
    /// Current zoom, animated toward `zoom_target`.
    pub zoom: f32,
    /// Where the zoom is headed.
    pub zoom_target: f32,
    /// Holding the image compares against the original.
    pub compare_hold: bool,
    /// Original toggle pins the source photo for compare.
    pub compare_pin: bool,
    /// Stage backdrop behind the cutout.
    pub backdrop: Backdrop,
    /// Whether the model is installed, once known.
    pub model_installed: Option<bool>,
    /// The queue changed while a batch was running.
    pub queue_changed_during_export: bool,
    /// Which tool's export is running.
    pub exporting_tool: Option<Tool>,
    /// Whether a refine recompute is scheduled (120ms debounce).
    pub refine_pending: Option<std::time::Instant>,
    /// Options the pending refine will use.
    pub refine_opts: kiln::MatteOptions,
    /// Dimensions of the current image, once its preview decodes.
    pub dims: Option<(u32, u32)>,
    /// "Save to…" picked a folder and wants an export right after.
    pub export_after_folder_pick: bool,
    /// A segmentation request is in flight for the current image.
    pub segment_in_flight: bool,
    /// Whether the current selection has a preview dispatched.
    pub preview_in_flight: bool,
    /// Whether a merge preview is wanted for the current (photo, template, fit).
    pub merge_in_flight: bool,
}

impl KilnState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tool: Tool::Remove,
            remove_queue: Vec::new(),
            merge_queue: Vec::new(),
            selected: [None, None],
            remove_opts: RemoveOptions::default(),
            fit: MergeFit::default(),
            template: None,
            destination: None,
            status: None,
            summary: None,
            batch: None,
            cancel: CancelToken::new(),
            preview_seq: 0,
            work_seq: 0,
            collapsed: false,
            theme: ThemeChoice::Dark,
            zoom: 1.0,
            zoom_target: 1.0,
            compare_hold: false,
            compare_pin: false,
            backdrop: Backdrop::Checker,
            model_installed: None,
            queue_changed_during_export: false,
            exporting_tool: None,
            refine_pending: None,
            refine_opts: kiln::MatteOptions::default(),
            dims: None,
            export_after_folder_pick: false,
            segment_in_flight: false,
            preview_in_flight: false,
            merge_in_flight: false,
        }
    }

    /// Queue of the active tool.
    #[must_use]
    pub fn queue(&self) -> &Vec<QueueEntry> {
        match self.tool {
            Tool::Remove => &self.remove_queue,
            Tool::Merge => &self.merge_queue,
        }
    }

    /// Mutable queue of the active tool; flags a running batch.
    pub fn queue_mut(&mut self) -> &mut Vec<QueueEntry> {
        if self.exporting_tool.is_some() {
            self.queue_changed_during_export = true;
        }
        match self.tool {
            Tool::Remove => &mut self.remove_queue,
            Tool::Merge => &mut self.merge_queue,
        }
    }

    /// Selected index of the active tool.
    #[must_use]
    pub fn selected(&self) -> Option<usize> {
        let index = match self.tool {
            Tool::Remove => 0,
            Tool::Merge => 1,
        };
        self.selected[index]
    }

    /// Select an index on the active tool.
    pub fn select(&mut self, index: Option<usize>) {
        let slot = match self.tool {
            Tool::Remove => 0,
            Tool::Merge => 1,
        };
        self.selected[slot] = index;
    }

    /// The selected entry, if any.
    #[must_use]
    pub fn current(&self) -> Option<&QueueEntry> {
        let queue = self.queue();
        self.selected().and_then(|index| queue.get(index))
    }

    /// Switching tools mid-batch is blocked.
    #[must_use]
    pub fn can_switch_tool(&self) -> bool {
        self.exporting_tool.is_none()
    }

    /// Stash one queue, restore the other (queues are fields; this only
    /// moves the window's attention and resets per-image view state).
    pub fn set_tool(&mut self, tool: Tool) {
        if !self.can_switch_tool() {
            self.status = Some("Wait for the batch to finish.".to_owned());
            return;
        }
        if self.tool == tool {
            return;
        }
        self.tool = tool;
        self.status = None;
        self.summary = None;
        self.compare_hold = false;
        self.compare_pin = false;
        self.zoom = 1.0;
        self.zoom_target = 1.0;
        self.refine_pending = None;
    }

    /// Append paths to the active queue, skipping non-images.
    pub fn add_paths(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let was_empty = self.queue().is_empty();
        let mut added = 0;
        for path in paths {
            if !is_image_path(&path) || self.queue().iter().any(|entry| entry.path == path) {
                continue;
            }
            self.queue_mut().push(QueueEntry::from_path(path));
            added += 1;
        }
        if added == 0 {
            return;
        }
        // Destination defaults to the first image's parent.
        if self.destination.is_none()
            && let Some(first) = self.queue().first()
        {
            self.destination = first.path.parent().map(std::path::Path::to_path_buf);
        }
        if was_empty {
            self.select(Some(0));
            self.preview_seq += 1;
        }
    }

    /// Drop the current image from the queue.
    pub fn clear_current(&mut self) {
        let Some(index) = self.selected() else {
            return;
        };
        self.queue_mut().remove(index);
        let len = self.queue().len();
        if len == 0 {
            self.select(None);
        } else {
            self.select(Some(index.min(len - 1)));
        }
        self.preview_seq += 1;
        self.work_seq += 1;
    }

    /// Remove a path from the active queue by path (after a paste or export).
    pub fn note_status(&mut self, message: impl Into<String>) {
        self.status = Some(message.into());
    }

    /// The displayed zoom, clamped.
    #[must_use]
    pub fn zoom_display(&self) -> f32 {
        self.zoom.clamp(0.05, 8.0)
    }
}

impl Default for KilnState {
    fn default() -> Self {
        Self::new()
    }
}

/// UI-side textures for the current image, rebuilt only when bytes change.
#[derive(Default)]
pub struct Textures {
    /// Original preview texture.
    pub original: Option<(PathBuf, egui::TextureHandle)>,
    /// Refined cutout preview texture.
    pub refined: Option<(PathBuf, egui::TextureHandle)>,
    /// Merged preview texture (merge tool).
    pub merged: Option<(PathBuf, egui::TextureHandle)>,
    /// Template preview texture.
    pub template: Option<(PathBuf, egui::TextureHandle)>,
}

impl Textures {
    fn store(
        slot: &mut Option<(PathBuf, egui::TextureHandle)>,
        ctx: &egui::Context,
        path: &std::path::Path,
        image: &ColorImage,
    ) {
        let matches = slot
            .as_ref()
            .is_some_and(|(slot_path, _)| slot_path == path);
        if !matches {
            let handle = ctx.load_texture(
                format!("kiln-{}", path.display()),
                image.clone(),
                egui::TextureOptions::LINEAR,
            );
            *slot = Some((path.to_path_buf(), handle));
        }
    }

    /// Handle an event that carries pixels.
    pub fn absorb(&mut self, ctx: &egui::Context, event: &Event) {
        match event {
            Event::PreviewReady { path, original, .. } => {
                Self::store(&mut self.original, ctx, path, original);
            }
            Event::RefinedReady { path, refined, .. } => {
                Self::store(&mut self.refined, ctx, path, refined);
            }
            Event::MergeReady { path, merged, .. } => {
                Self::store(&mut self.merged, ctx, path, merged);
            }
            Event::TemplateReady { path, image, .. } => {
                Self::store(&mut self.template, ctx, path, image);
            }
            _ => {}
        }
    }

    /// Forget everything that is not the given path.
    pub fn retain_current(&mut self, path: &std::path::Path) {
        for slot in [
            &mut self.original,
            &mut self.refined,
            &mut self.merged,
            &mut self.template,
        ] {
            let keep = slot
                .as_ref()
                .is_some_and(|(slot_path, _)| slot_path == path);
            if !keep {
                *slot = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_edge_scales_down_to_1280() {
        assert_eq!(fit_edge(4000, 3000), (1280, 960));
        assert_eq!(fit_edge(3000, 4000), (960, 1280));
    }

    #[test]
    fn fit_edge_leaves_small_images_alone() {
        assert_eq!(fit_edge(1280, 720), (1280, 720));
        assert_eq!(fit_edge(640, 480), (640, 480));
    }

    #[test]
    fn fit_edge_handles_degenerate_sizes() {
        assert_eq!(fit_edge(0, 0), (0, 0));
        assert_eq!(fit_edge(0, 500), (0, 500));
    }

    #[test]
    fn image_paths_are_filtered() {
        assert!(is_image_path(std::path::Path::new("/a/photo.PNG")));
        assert!(is_image_path(std::path::Path::new("/a/photo.jpeg")));
        assert!(!is_image_path(std::path::Path::new("/a/notes.txt")));
        assert!(!is_image_path(std::path::Path::new("/a/photo")));
    }

    #[test]
    fn tool_switch_blocked_mid_batch_sets_status() {
        let mut state = KilnState::new();
        state.exporting_tool = Some(Tool::Remove);
        state.set_tool(Tool::Merge);
        assert_eq!(state.tool, Tool::Remove);
        assert_eq!(
            state.status.as_deref(),
            Some("Wait for the batch to finish.")
        );
    }

    #[test]
    fn queue_mutation_during_export_is_flagged() {
        let mut state = KilnState::new();
        state.exporting_tool = Some(Tool::Remove);
        state.add_paths(vec![PathBuf::from("/tmp/a.png")]);
        assert!(state.queue_changed_during_export);
    }

    #[test]
    fn add_paths_defaults_destination_to_first_parent() {
        let mut state = KilnState::new();
        state.add_paths(vec![
            PathBuf::from("/photos/one.png"),
            PathBuf::from("/elsewhere/two.png"),
        ]);
        assert_eq!(state.destination, Some(PathBuf::from("/photos")));
        assert_eq!(state.selected(), Some(0));
    }

    #[test]
    fn add_paths_skips_duplicates_and_non_images() {
        let mut state = KilnState::new();
        state.add_paths(vec![PathBuf::from("/tmp/a.png")]);
        state.exporting_tool = None;
        state.add_paths(vec![
            PathBuf::from("/tmp/a.png"),
            PathBuf::from("/tmp/b.txt"),
        ]);
        assert_eq!(state.queue().len(), 1);
    }

    #[test]
    fn clear_current_keeps_selection_in_range() {
        let mut state = KilnState::new();
        state.add_paths(vec![
            PathBuf::from("/tmp/a.png"),
            PathBuf::from("/tmp/b.png"),
        ]);
        state.select(Some(1));
        state.clear_current();
        assert_eq!(state.queue().len(), 1);
        assert_eq!(state.selected(), Some(0));
    }
}
