//! The Kiln window: one app, two tools, three panes.
//!
//! The UI thread owns all window state, pumps worker events, and drops
//! stale ones by sequence. Heavy work never touches this thread.

mod chrome;
pub mod fonts;
mod panel;
mod stage;
mod state;
mod tasks;
mod theme;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use egui::Frame;

use state::BatchProgress;
pub use state::{Caches, KilnState, QueueEntry, Textures, Tool};
use tasks::{Event, EventTx};

use kiln::{CancelToken, MatteOptions, ModelStore};

/// The eframe app.
pub struct KilnApp {
    state: KilnState,
    caches: Arc<Caches>,
    textures: Textures,
    checker: stage::Checker,
    events: std::sync::mpsc::Receiver<Event>,
    tx: EventTx,
    applied_theme: Option<theme::ThemeChoice>,
    last_selection: Option<(Tool, PathBuf)>,
    theme: theme::Theme,
}

impl KilnApp {
    #[must_use]
    pub fn new(models_root: ModelStore) -> Self {
        let (tx, events) = std::sync::mpsc::channel();
        let caches = Arc::new(Caches::new(models_root));
        let mut state = KilnState::new();
        state.model_installed = Some(caches.store.is_installed(kiln::models::RMBG_14.id));
        let theme = theme::Theme::new(state.theme);
        Self {
            state,
            caches,
            textures: Textures::default(),
            checker: stage::Checker::default(),
            events,
            tx,
            applied_theme: None,
            last_selection: None,
            theme,
        }
    }

    fn apply_theme_if_changed(&mut self, ctx: &egui::Context) {
        if self.applied_theme != Some(self.state.theme) {
            let theme = theme::Theme::new(self.state.theme);
            ctx.global_style_mut(|style| theme.apply(style));
            self.theme = theme;
            self.checker.invalidate();
            self.applied_theme = Some(self.state.theme);
        }
    }

    /// Drop events the UI has moved past; apply the live ones.
    fn pump_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.events.try_recv() {
            self.textures.absorb(ctx, &event);
            match event {
                Event::PreviewReady {
                    path,
                    width,
                    height,
                    ..
                } => {
                    self.state.preview_in_flight = false;
                    if self.is_current(&path) {
                        self.state.dims = Some((width, height));
                    }
                }
                Event::MatteReady { path } => {
                    self.state.segment_in_flight = false;
                    if self.is_current(&path) && self.state.tool == Tool::Remove {
                        // Matte landed: refine right away.
                        self.state.refine_pending = Some(Instant::now());
                    }
                }
                Event::RefinedReady { path, .. } => {
                    self.state.work_seq = self.state.work_seq.wrapping_add(1);
                    let _ = path;
                }
                Event::MergeReady { path, .. } => {
                    self.state.merge_in_flight = false;
                    let _ = path;
                }
                Event::TemplateReady { .. } => {
                    // Template thumbnail ready; a photo may now merge.
                    self.request_merge_preview();
                }
                Event::ModelStatus { installed } => {
                    self.state.model_installed = Some(installed);
                    if !installed {
                        self.state
                            .note_status("Model file is missing. Put RMBG-1.4 back in models/.");
                    }
                }
                Event::BatchStarted { total } => {
                    self.state.batch = Some(BatchProgress { done: 0, total });
                }
                Event::BatchItem { result } => {
                    if let Some(progress) = self.state.batch.as_mut() {
                        progress.done += 1;
                    }
                    if let Err(message) = result {
                        self.state.note_status(message);
                    }
                }
                Event::BatchDone { ok, failed, folder } => {
                    self.state.batch = None;
                    self.state.exporting_tool = None;
                    self.state.cancel = CancelToken::new();
                    self.state.summary = if self.state.queue_changed_during_export {
                        self.state.queue_changed_during_export = false;
                        Some("Queue changed during export.".to_owned())
                    } else {
                        Some(format!(
                            "{ok} ok, {failed} failed \u{00B7} {}",
                            folder.display()
                        ))
                    };
                }
                Event::FolderPicked(folder) => {
                    if let Some(dir) = folder.clone() {
                        self.state.destination = Some(dir);
                    }
                    if self.state.export_after_folder_pick {
                        self.state.export_after_folder_pick = false;
                        if folder.is_some() {
                            self.dispatch_export();
                        }
                    }
                }
                Event::ImagesPicked(paths) => {
                    self.add_paths(paths);
                }
                Event::TemplatePicked(path) => {
                    if let Some(template) = path {
                        self.state.template = Some(template.clone());
                        tasks::request_template(template, self.tx.clone());
                    }
                }
                Event::Pasted(result) => match result {
                    Ok(path) => {
                        self.add_paths(vec![path]);
                    }
                    Err(message) => self.state.note_status(message),
                },
                Event::Copied(result) => match result {
                    Ok(()) => self.state.note_status("Copied to clipboard."),
                    Err(message) => self.state.note_status(message),
                },
            }
        }
    }

    fn is_current(&self, path: &std::path::Path) -> bool {
        self.state.current().is_some_and(|entry| entry.path == path)
    }

    /// Add image paths to the active queue and refresh previews.
    fn add_paths(&mut self, paths: Vec<PathBuf>) {
        let before = self.state.current().map(|entry| entry.path.clone());
        self.state.add_paths(paths);
        let after = self.state.current().map(|entry| entry.path.clone());
        if before != after || self.state.current().is_none() {
            self.on_selection_changed();
        }
    }

    /// Fresh selection: reset view state, dispatch preview (+ merge).
    fn on_selection_changed(&mut self) {
        self.state.zoom = 1.0;
        self.state.zoom_target = 1.0;
        self.state.compare_hold = false;
        self.state.compare_pin = false;
        self.state.dims = None;
        self.state.refine_pending = None;
        self.state.segment_in_flight = false;
        self.state.merge_in_flight = false;
        self.state.preview_in_flight = false;
        match self.state.current().cloned() {
            Some(entry) => {
                self.textures.retain_current(&entry.path);
                self.last_selection = Some((self.state.tool, entry.path.clone()));
                self.state.preview_in_flight = true;
                self.state.preview_seq += 1;
                tasks::request_preview(
                    entry.path.clone(),
                    Arc::clone(&self.caches),
                    self.tx.clone(),
                );
                if self.state.tool == Tool::Merge {
                    self.request_merge_preview();
                }
            }
            None => {
                self.last_selection = None;
            }
        }
    }

    /// Merge preview needs a template and a photo.
    fn request_merge_preview(&mut self) {
        if self.state.tool != Tool::Merge {
            return;
        }
        let (Some(template), Some(entry)) =
            (self.state.template.clone(), self.state.current().cloned())
        else {
            return;
        };
        if self.state.merge_in_flight {
            return;
        }
        self.state.merge_in_flight = true;
        self.state.work_seq = self.state.work_seq.wrapping_add(1);
        tasks::request_merge(
            entry.path.clone(),
            template,
            self.state.fit,
            self.tx.clone(),
        );
    }

    /// Detect selection changes made through direct state edits.
    fn check_selection(&mut self) {
        let now = self
            .state
            .current()
            .map(|entry| (self.state.tool, entry.path.clone()));
        if now != self.last_selection {
            self.on_selection_changed();
        }
    }

    /// Fire the debounced refine when its 120ms window closes.
    fn check_debounce(&mut self) {
        if let Some(deadline) = self.state.refine_pending
            && Instant::now() >= deadline
        {
            self.state.refine_pending = None;
            self.dispatch_refine();
        }
    }

    /// Refine the cached matte, segmenting first when needed.
    fn dispatch_refine(&mut self) {
        let Some(entry) = self.state.current().cloned() else {
            return;
        };
        if self.state.tool != Tool::Remove {
            return;
        }
        let options = self.state.refine_opts;
        let has_matte = self.caches.mattes.lock().is_ok_and(|guard| {
            guard
                .get(&entry.path)
                .is_some_and(|cached| cached.modified == tasks::file_modified(&entry.path))
        });
        if has_matte {
            tasks::request_refine(
                entry.path.clone(),
                options,
                Arc::clone(&self.caches),
                self.tx.clone(),
            );
        } else if !self.state.segment_in_flight {
            self.state.segment_in_flight = true;
            self.state.work_seq = self.state.work_seq.wrapping_add(1);
            tasks::request_matte(
                entry.path.clone(),
                Arc::clone(&self.caches),
                self.tx.clone(),
            );
        }
    }

    /// Start a batch on the active tool.
    fn dispatch_export(&mut self) {
        dispatch_export(&mut self.state, &self.tx, &self.caches);
    }

    /// Window drag-drop: non-image paths are skipped silently.
    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect()
        });
        if !dropped.is_empty() {
            self.add_paths(dropped);
        }
    }
}

impl eframe::App for KilnApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.apply_theme_if_changed(&ctx);
        self.pump_events(&ctx);
        self.handle_drops(&ctx);
        self.check_selection();
        self.check_debounce();

        chrome::title_bar(ui, &mut self.state, &self.theme);
        if self.state.model_installed == Some(false) {
            chrome::model_banner(
                ui,
                &self.theme,
                "Model file is missing. Put RMBG-1.4 back in models/ next to the app.",
            );
        }

        let sidebar_width = if self.state.collapsed { 64.0 } else { 208.0 };
        egui::Panel::left("sidebar")
            .exact_size(sidebar_width)
            .resizable(false)
            .frame(
                Frame::default()
                    .fill(self.theme.surface())
                    .stroke(egui::Stroke::new(1.0, self.theme.border())),
            )
            .show(ui, |ui| {
                chrome::sidebar(ui, &mut self.state, &self.theme);
            });

        let caches = Arc::clone(&self.caches);
        let tx = self.tx.clone();
        let dims = self.state.dims;
        egui::Panel::right("panel")
            .exact_size(panel::PANEL_WIDTH)
            .resizable(false)
            .frame(
                Frame::default()
                    .fill(self.theme.surface())
                    .stroke(egui::Stroke::new(1.0, self.theme.border())),
            )
            .show(ui, |ui| {
                panel::panel(ui, &mut self.state, &self.theme, &tx, &caches, dims);
            });

        stage::stage(
            ui,
            &mut self.state,
            &self.textures,
            &mut self.checker,
            &self.theme,
            &self.tx,
            &self.caches,
        );
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let background = self.theme.background();
        [
            f32::from(background.r()) / 255.0,
            f32::from(background.g()) / 255.0,
            f32::from(background.b()) / 255.0,
            1.0,
        ]
    }
}

/// Start a batch on the active tool. Shared by the Export button and the
/// "Save to…" flow.
pub fn dispatch_export(state: &mut KilnState, tx: &EventTx, caches: &Arc<Caches>) {
    if state.exporting_tool.is_some() {
        return;
    }
    let Some(destination) = state.destination.clone() else {
        state.note_status("Pick a destination folder first.");
        return;
    };
    let entries: Vec<QueueEntry> = state.queue().clone();
    if entries.is_empty() {
        return;
    }
    state.cancel = CancelToken::new();
    state.exporting_tool = Some(state.tool);
    state.summary = None;
    match state.tool {
        Tool::Remove => {
            if !state.model_installed.unwrap_or(false) {
                state.exporting_tool = None;
                state.note_status("Model file is missing. Put RMBG-1.4 back in models/.");
                return;
            }
            let options = MatteOptions {
                threshold: state.remove_opts.threshold,
                feather: state.remove_opts.feather,
                dilate: state.remove_opts.dilate,
                trim: state.remove_opts.trim,
            };
            tasks::export_remove_batch(
                entries,
                options,
                destination,
                Arc::clone(caches),
                state.cancel.clone(),
                tx.clone(),
            );
        }
        Tool::Merge => {
            let Some(template) = state.template.clone() else {
                state.exporting_tool = None;
                state.note_status("Merge needs a template first.");
                return;
            };
            let fit = state.fit;
            tasks::export_merge_batch(
                entries,
                template,
                fit,
                destination,
                state.cancel.clone(),
                tx.clone(),
            );
        }
    }
}

/// Copy dispatch shared by the stage context menu.
pub fn copy_dispatch(state: &mut KilnState, tx: &EventTx, caches: &Arc<Caches>) {
    let Some(entry) = state.current().cloned() else {
        return;
    };
    match state.tool {
        Tool::Remove => tasks::copy_remove(
            entry.path.clone(),
            MatteOptions {
                threshold: state.remove_opts.threshold,
                feather: state.remove_opts.feather,
                dilate: state.remove_opts.dilate,
                trim: false,
            },
            Arc::clone(caches),
            tx.clone(),
        ),
        Tool::Merge => tasks::copy_photo(entry.path.clone(), tx.clone()),
    }
}

/// Schedule a debounced refine after a control change.
pub fn schedule_refine(state: &mut KilnState) {
    if state.tool != Tool::Remove {
        return;
    }
    if state.refine_pending.is_none() {
        state.refine_pending = Some(Instant::now() + Duration::from_millis(120));
    }
}
