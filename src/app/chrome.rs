//! Title bar, model banner, and sidebar.
//!
//! No OS chrome: the title bar is drawn by the app — drag it to move,
//! double-click to maximise. The sidebar holds one mark per tool plus
//! collapse; it folds to a 64px rail.

use std::path::PathBuf;

use egui::{
    Align, Align2, Button, Color32, Frame, Layout, RichText, Sense, Stroke, ViewportCommand, pos2,
    vec2,
};

use crate::app::state::{KilnState, Tool};
use crate::app::tasks::{EventTx, paste_image, pick_images};
use crate::app::theme::{Theme, ThemeChoice};

/// Height of the drawn title bar.
pub const TITLE_BAR_HEIGHT: f32 = 40.0;

/// Draw the custom title bar.
pub fn title_bar(ui: &mut egui::Ui, state: &mut KilnState, theme: &Theme) {
    egui::Panel::top("title_bar")
        .exact_size(TITLE_BAR_HEIGHT)
        .resizable(false)
        .frame(
            Frame::default()
                .fill(theme.surface())
                .inner_margin(egui::Margin::symmetric(12, 0))
                .stroke(Stroke::new(1.0, theme.border())),
        )
        .show(ui, |ui| {
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.label(
                    RichText::new("Kiln")
                        .text_style(egui::TextStyle::Name("Title".into()))
                        .color(theme.text()),
                );

                // Drag region carries the current file name.
                let file_name = state
                    .current()
                    .map_or_else(|| "no image".to_owned(), |entry| entry.label.clone());
                let drag_width = (ui.available_width() - 190.0).max(40.0);
                let (drag_rect, drag) =
                    ui.allocate_exact_size(vec2(drag_width, 26.0), Sense::click_and_drag());
                ui.painter().text(
                    pos2(drag_rect.left(), drag_rect.center().y),
                    Align2::LEFT_CENTER,
                    file_name,
                    egui::FontId::proportional(12.0),
                    theme.text_secondary(),
                );
                if drag.drag_started_by(egui::PointerButton::Primary) {
                    ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
                }
                if drag.double_clicked() {
                    let maximized = ui
                        .ctx()
                        .input(|input| input.viewport().maximized)
                        .unwrap_or(false);
                    ui.ctx()
                        .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                }

                // Theme toggle.
                let toggle_label = match state.theme {
                    ThemeChoice::Dark => "\u{25D0} Dark",
                    ThemeChoice::Light => "\u{25D1} Light",
                };
                if ui
                    .add(
                        Button::new(RichText::new(toggle_label).color(theme.text_secondary()))
                            .small(),
                    )
                    .clicked()
                {
                    state.theme = state.theme.toggled();
                }

                window_buttons(ui, theme);
            });
        });
}

fn window_buttons(ui: &mut egui::Ui, theme: &Theme) {
    let maximized = ui
        .ctx()
        .input(|input| input.viewport().maximized)
        .unwrap_or(false);

    if ui
        .add(Button::new(RichText::new("\u{2014}").color(theme.text_secondary())).small())
        .clicked()
    {
        ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
    }
    if ui
        .add(
            Button::new(
                RichText::new(if maximized { "\u{274F}" } else { "\u{25A1}" })
                    .color(theme.text_secondary()),
            )
            .small(),
        )
        .clicked()
    {
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
    let close =
        ui.add(Button::new(RichText::new("\u{2715}").color(theme.text_secondary())).small());
    if close.clicked() {
        ui.ctx().send_viewport_cmd(ViewportCommand::Close);
    } else if close.hovered() {
        // Danger red is the only accent; the close button earns it on hover.
        ui.painter().rect_filled(close.rect, 4.0, theme.danger());
        ui.painter().text(
            close.rect.center(),
            Align2::CENTER_CENTER,
            "\u{2715}",
            egui::FontId::proportional(12.0),
            Color32::WHITE,
        );
    }
}

/// Calm banner under the title bar when the model file is missing.
pub fn model_banner(ui: &mut egui::Ui, theme: &Theme, message: &str) {
    egui::Panel::top("model_banner")
        .resizable(false)
        .frame(
            Frame::default()
                .fill(theme.surface_raised())
                .inner_margin(egui::Margin::symmetric(16, 8))
                .stroke(Stroke::new(1.0, theme.border())),
        )
        .show(ui, |ui| {
            ui.label(
                RichText::new(message)
                    .text_style(egui::TextStyle::Body)
                    .color(theme.text_secondary()),
            );
        });
}

/// Tool marks plus collapse, drawn inside the left panel.
pub fn sidebar(ui: &mut egui::Ui, state: &mut KilnState, theme: &Theme) {
    let collapsed = state.collapsed;
    ui.add_space(12.0);
    for tool in [Tool::Remove, Tool::Merge] {
        let active = state.tool == tool;
        let response = if collapsed {
            let (rect, response) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::click());
            let fill = if active {
                theme.border_strong()
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(rect, 4.0, fill);
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                tool.mark(),
                egui::FontId::monospace(14.0),
                if active {
                    theme.text()
                } else {
                    theme.text_secondary()
                },
            );
            response.on_hover_text(tool.label())
        } else {
            let text = RichText::new(tool.label()).color(if active {
                theme.text()
            } else {
                theme.text_secondary()
            });
            let button = if active {
                Button::new(text).fill(theme.border_strong())
            } else {
                Button::new(text)
            };
            ui.add_sized([ui.available_width(), 28.0], button)
        };
        if response.clicked() {
            state.set_tool(tool);
        }
        ui.add_space(4.0);
    }

    ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
        let fold_label = if collapsed { "\u{00BB}" } else { "\u{00AB}" };
        if ui
            .add(Button::new(RichText::new(fold_label).color(theme.text_secondary())).small())
            .on_hover_text(if collapsed {
                "Expand sidebar"
            } else {
                "Collapse sidebar"
            })
            .clicked()
        {
            state.collapsed = !state.collapsed;
        }
        ui.add_space(12.0);
    });
}

/// Empty-stage tile: icon square, call to action, Browse + Paste.
pub fn empty_stage(ui: &mut egui::Ui, state: &mut KilnState, theme: &Theme, tx: &EventTx) {
    ui.vertical_centered(|ui| {
        ui.add_space(48.0);
        let (tile_rect, _) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::hover());
        ui.painter()
            .rect_filled(tile_rect, 4.0, theme.surface_raised());
        ui.painter().rect_stroke(
            tile_rect,
            4.0,
            Stroke::new(1.0, theme.border_strong()),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            tile_rect.center(),
            Align2::CENTER_CENTER,
            "\u{2B21}",
            egui::FontId::proportional(28.0),
            theme.text_secondary(),
        );
        ui.add_space(12.0);
        ui.label(
            RichText::new("Drop images here")
                .text_style(egui::TextStyle::Body)
                .color(theme.text_secondary()),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.set_width(160.0);
                if ui.button("Paste").clicked() {
                    let dir = state
                        .destination
                        .clone()
                        .unwrap_or_else(|| PathBuf::from("."));
                    paste_image(dir, tx.clone());
                }
                if ui.button("Browse").clicked() {
                    pick_images(tx.clone());
                }
            });
        });
    });
}
