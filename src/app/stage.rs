//! The stage: locked centered viewer, checkerboard, hold-to-compare, zoom.
//!
//! Wheel, slider, buttons and double-click zoom about the center with
//! smoothing. No pan. Holding the image compares with the original; the
//! Original toggle pins it. Right-click on the image opens the custom
//! Copy / Paste / Clear menu.

use std::sync::Arc;

use egui::{
    Align, Button, Color32, ColorImage, Frame, Layout, RichText, Sense, Stroke, TextureHandle,
    TextureOptions, Vec2, pos2, vec2,
};

use crate::app::chrome::empty_stage;
use crate::app::state::{BatchProgress, Caches, KilnState, Textures, Tool, fit_edge};
use crate::app::tasks::EventTx;
use crate::app::theme::{Backdrop, Theme};
use crate::app::{copy_dispatch, tasks};

/// Checkerboard texture cache, so the stage does not redraw thousands of
/// tiles every frame.
#[derive(Default)]
pub struct Checker {
    texture: Option<TextureHandle>,
}

impl Checker {
    /// One 512px tile of 16px cells; drawn stretched over the image rect.
    fn texture(&mut self, ctx: &egui::Context, theme: &Theme) -> TextureHandle {
        if let Some(texture) = &self.texture {
            return texture.clone();
        }
        let cells = 16;
        let cell = 16;
        let side = cells * cell;
        let mut rgba = Vec::with_capacity(side * side * 4);
        for y in 0..side {
            for x in 0..side {
                let on = ((x / cell) + (y / cell)) % 2 == 0;
                let color = if on {
                    theme.checker_a()
                } else {
                    theme.checker_b()
                };
                rgba.extend_from_slice(&color.to_array());
            }
        }
        let image = ColorImage::from_rgba_unmultiplied([side, side], &rgba);
        let handle = ctx.load_texture("kiln-checker", image, TextureOptions::NEAREST);
        self.texture = Some(handle.clone());
        handle
    }

    /// Forget the cached texture when the palette changes.
    pub fn invalidate(&mut self) {
        self.texture = None;
    }
}

/// Draw the stage for the active tool.
#[allow(clippy::too_many_lines)]
pub fn stage(
    ui: &mut egui::Ui,
    state: &mut KilnState,
    textures: &Textures,
    checker: &mut Checker,
    theme: &Theme,
    tx: &EventTx,
    caches: &Arc<Caches>,
) {
    // Animated zoom: approach the target, then stop repainting.
    let diff = state.zoom_target - state.zoom;
    if diff.abs() > 0.001 {
        state.zoom += diff * 0.25;
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(8));
    } else {
        state.zoom = state.zoom_target;
    }

    egui::CentralPanel::default()
        .frame(Frame::default().fill(theme.background()))
        .show(ui, |ui| {
            let Some(entry) = state.current().cloned() else {
                empty_stage(ui, state, theme, tx);
                return;
            };

            let (texture, pixel_size) = stage_texture(state, textures, entry.path.as_path());
            let Some(texture) = texture else {
                // Preview still decoding.
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("Loading preview\u{2026}")
                            .text_style(egui::TextStyle::Body)
                            .color(theme.text_secondary()),
                    );
                });
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(50));
                return;
            };

            let stage_rect = ui.available_rect_before_wrap();
            let image_rect = centered_rect(stage_rect, pixel_size, state.zoom_display());
            let image_response = ui
                .allocate_rect(image_rect, Sense::click_and_drag())
                .on_hover_cursor(egui::CursorIcon::ZoomIn);

            // Backdrop, then image, clipped to the stage.
            let painter = ui.painter().with_clip_rect(stage_rect);
            match state.backdrop {
                Backdrop::Checker => {
                    let checker_texture = checker.texture(ui.ctx(), theme);
                    painter.image(
                        checker_texture.id(),
                        image_rect,
                        egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                Backdrop::Dark | Backdrop::White => {
                    painter.rect_filled(image_rect, 0.0, theme.backdrop_solid(state.backdrop));
                }
            }
            painter.image(
                texture.id(),
                image_rect,
                egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );

            // Hold the image to compare with the original.
            if image_response.dragged_by(egui::PointerButton::Primary) {
                state.compare_hold = true;
            }
            if image_response.drag_stopped_by(egui::PointerButton::Primary)
                || image_response.secondary_clicked()
            {
                state.compare_hold = false;
            }

            // Wheel zoom about the center.
            let scroll = ui.ctx().input(|input| input.smooth_scroll_delta.y);
            if image_response.hovered() && scroll.abs() > 0.0 {
                let factor: f32 = (1.0_f32 + scroll * 0.002).clamp(0.5, 1.5);
                state.zoom_target = (state.zoom_target * factor).clamp(1.0, 8.0);
            }
            if image_response.double_clicked() {
                state.zoom_target = if state.zoom_target > 1.01 { 1.0 } else { 2.0 };
            }

            // Right-click on the image opens the custom menu.
            if image_response.secondary_clicked() {
                image_response.context_menu(|ui| {
                    if ui.button("Copy").clicked() {
                        ui.close();
                        copy_dispatch(state, tx, caches);
                    }
                    if ui.button("Paste").clicked() {
                        ui.close();
                        let dir = state
                            .destination
                            .clone()
                            .unwrap_or_else(|| std::path::PathBuf::from("."));
                        tasks::paste_image(dir, tx.clone());
                    }
                    if ui.button("Clear").clicked() {
                        ui.close();
                        state.clear_current();
                    }
                });
            }

            // Zoom bar, bottom-center overlay.
            zoom_bar(ui, state, theme, image_rect);

            // Working pill, bottom-right of the stage.
            if let Some(progress) = state.batch {
                working_pill(ui, state, theme, progress, stage_rect);
            }
        });
}

/// Pick the texture the stage should show right now.
fn stage_texture<'a>(
    state: &KilnState,
    textures: &'a Textures,
    path: &std::path::Path,
) -> (Option<&'a TextureHandle>, Vec2) {
    let current = |slot: &'a Option<(std::path::PathBuf, TextureHandle)>| {
        slot.as_ref()
            .filter(|(slot_path, _)| slot_path == path)
            .map(|(_, handle)| handle)
    };
    let show_original = state.compare_hold || state.compare_pin;
    let is_merge = state.tool == Tool::Merge;
    if !show_original {
        if is_merge {
            if let Some(handle) = current(&textures.merged) {
                let size = handle.size_vec2();
                return (Some(handle), size);
            }
        } else if let Some(handle) = current(&textures.refined) {
            let size = handle.size_vec2();
            return (Some(handle), size);
        }
    }
    if let Some(handle) = current(&textures.original) {
        let size = handle.size_vec2();
        return (Some(handle), size);
    }
    if is_merge && let Some(handle) = current(&textures.template) {
        let size = handle.size_vec2();
        return (Some(handle), size);
    }
    (None, Vec2::ZERO)
}

/// f32 to i64 without the clippy noise; caller guarantees positivity.
#[allow(clippy::cast_possible_truncation)]
fn f2i(value: f32) -> i64 {
    value as i64
}

/// f32 to i32, same deal; zoom percents are small positives.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn f2i32(value: f32) -> i32 {
    value as i32
}

/// Centered display rect for `pixel_size` at `zoom`, fitted to `stage`.
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
fn centered_rect(stage: egui::Rect, pixel_size: Vec2, zoom: f32) -> egui::Rect {
    if pixel_size.x <= 0.0 || pixel_size.y <= 0.0 {
        return egui::Rect::from_center_size(stage.center(), vec2(1.0, 1.0));
    }
    let fit = fit_edge(
        u32::try_from(f2i(pixel_size.x.max(1.0))).unwrap_or(1),
        u32::try_from(f2i(pixel_size.y.max(1.0))).unwrap_or(1),
    );
    let base = vec2(fit.0 as f32, fit.1 as f32);
    let mut size = base * zoom;
    if zoom <= 1.01 {
        // Fit inside the stage at rest.
        let scale = (stage.width() / size.x)
            .min(stage.height() / size.y)
            .min(1.0);
        size *= scale;
    }
    let max_size = stage.size() * 8.0;
    size = size.min(max_size);
    egui::Rect::from_center_size(stage.center(), size)
}

/// Zoom slider, backdrop swatches, and the Original toggle, as one bar.
#[allow(clippy::too_many_lines)]
fn zoom_bar(ui: &mut egui::Ui, state: &mut KilnState, theme: &Theme, image_rect: egui::Rect) {
    let bar_size = vec2(380.0, 36.0);
    let bar_pos = pos2(
        image_rect.center().x - bar_size.x / 2.0,
        image_rect.bottom() + 12.0,
    );
    // Keep the bar on screen when zoomed in.
    let screen = ui.max_rect();
    let bar_pos = pos2(
        bar_pos.x.clamp(
            screen.left() + 8.0,
            (screen.right() - bar_size.x - 8.0).max(screen.left() + 8.0),
        ),
        bar_pos.y.min(screen.bottom() - bar_size.y - 8.0),
    );

    #[allow(deprecated)]
    egui::Area::new(egui::Id::new("kiln-zoom-bar"))
        .fixed_pos(bar_pos)
        .order(egui::Order::Middle)
        .show(ui.ctx(), |ui| {
            Frame::default()
                .fill(theme.surface())
                .stroke(Stroke::new(1.0, theme.border()))
                .corner_radius(4.0)
                .inner_margin(egui::Margin::symmetric(10, 5))
                .show(ui, |ui| {
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        if ui
                            .add(Button::new(RichText::new("\u{2212}").color(theme.text())).small())
                            .clicked()
                        {
                            state.zoom_target = (state.zoom_target / 1.25).clamp(1.0, 8.0);
                        }
                        let slider = egui::Slider::new(&mut state.zoom_target, 1.0..=8.0)
                            .logarithmic(true)
                            .show_value(false)
                            .text("");
                        ui.add_sized([140.0, 20.0], slider);
                        if ui
                            .add(Button::new(RichText::new("\u{FF0B}").color(theme.text())).small())
                            .clicked()
                        {
                            state.zoom_target = (state.zoom_target * 1.25).clamp(1.0, 8.0);
                        }
                        let percent = format!(
                            "{}%",
                            i32::from(state.zoom_display() * 100.0 >= 0.0)
                                * f2i32((state.zoom_display() * 100.0).round())
                        );
                        ui.label(
                            RichText::new(percent)
                                .text_style(egui::TextStyle::Name("Mono".into()))
                                .color(theme.text_secondary()),
                        );

                        ui.separator();

                        for backdrop in Backdrop::all() {
                            let active = state.backdrop == backdrop;
                            let label = match backdrop {
                                Backdrop::Checker => "\u{25A6}",
                                Backdrop::Dark => "\u{25A0}",
                                Backdrop::White => "\u{25A1}",
                            };
                            let mut button = Button::new(RichText::new(label).color(if active {
                                theme.text()
                            } else {
                                theme.text_secondary()
                            }))
                            .small();
                            if active {
                                button = button.fill(theme.border_strong());
                            }
                            if ui
                                .add(button)
                                .on_hover_text(match backdrop {
                                    Backdrop::Checker => "Checkerboard",
                                    Backdrop::Dark => "Dark backdrop exposes light halos",
                                    Backdrop::White => "White backdrop exposes dark fringes",
                                })
                                .clicked()
                            {
                                state.backdrop = backdrop;
                            }
                        }

                        ui.separator();

                        let pinned = state.compare_pin;
                        let original = Button::new(RichText::new("Original").color(if pinned {
                            theme.text()
                        } else {
                            theme.text_secondary()
                        }))
                        .small();
                        let original = if pinned {
                            original.fill(theme.border_strong())
                        } else {
                            original
                        };
                        if ui
                            .add(original)
                            .on_hover_text("Pin the source photo for compare")
                            .clicked()
                        {
                            state.compare_pin = !state.compare_pin;
                        }
                    });
                });
        });
}

/// `n/total` pill with a thin progress bar, bottom-right of the stage.
fn working_pill(
    ui: &mut egui::Ui,
    state: &mut KilnState,
    theme: &Theme,
    progress: BatchProgress,
    stage_rect: egui::Rect,
) {
    let pill_size = vec2(220.0, 38.0);
    let pos = pos2(
        stage_rect.right() - pill_size.x - 16.0,
        stage_rect.bottom() - pill_size.y - 16.0,
    );
    #[allow(deprecated)]
    egui::Area::new(egui::Id::new("kiln-working-pill"))
        .fixed_pos(pos)
        .order(egui::Order::Middle)
        .show(ui.ctx(), |ui| {
            Frame::default()
                .fill(theme.surface())
                .stroke(Stroke::new(1.0, theme.border()))
                .corner_radius(4.0)
                .inner_margin(egui::Margin::symmetric(12, 7))
                .show(ui, |ui| {
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{}/{}", progress.done, progress.total))
                                .text_style(egui::TextStyle::Name("MonoStrong".into()))
                                .color(theme.text()),
                        );
                        let fraction = if progress.total == 0 {
                            0.0
                        } else {
                            f32::from(u16::try_from(progress.done).unwrap_or(u16::MAX))
                                / f32::from(u16::try_from(progress.total).unwrap_or(u16::MAX))
                        };
                        let (bar_rect, _) = ui.allocate_exact_size(vec2(80.0, 3.0), Sense::hover());
                        ui.painter().rect_filled(bar_rect, 1.5, theme.border());
                        let mut fill = bar_rect;
                        fill.set_width(bar_rect.width() * fraction.clamp(0.0, 1.0));
                        ui.painter().rect_filled(fill, 1.5, theme.text_secondary());
                        if ui
                            .add(
                                Button::new(RichText::new("Cancel").color(theme.text_secondary()))
                                    .small(),
                            )
                            .clicked()
                        {
                            state.cancel.cancel();
                        }
                    });
                });
        });
}
