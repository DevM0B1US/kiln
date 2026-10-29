//! The fixed right panel: image facts, refine groups / fit, destination,
//! export. 264px, never collapses.

use std::sync::Arc;

use egui::{Align, Button, Layout, RichText};

use kiln::MergeFit;

use crate::app::state::{Caches, KilnState, Tool};
use crate::app::tasks::{EventTx, copy_photo, copy_remove, pick_folder, pick_template};
use crate::app::theme::Theme;

/// Panel width, fixed.
pub const PANEL_WIDTH: f32 = 264.0;

/// Draw the panel for the active tool.
pub fn panel(
    ui: &mut egui::Ui,
    state: &mut KilnState,
    theme: &Theme,
    tx: &EventTx,
    caches: &Arc<Caches>,
    dims: Option<(u32, u32)>,
) {
    ui.add_space(16.0);
    match state.tool {
        Tool::Remove => remove_panel(ui, state, theme, tx, caches, dims),
        Tool::Merge => merge_panel(ui, state, theme, tx, caches),
    }
    ui.add_space(8.0);
    status_line(ui, state, theme);
    ui.add_space(8.0);
}

fn section_title(ui: &mut egui::Ui, theme: &Theme, title: &str) {
    ui.label(
        RichText::new(title)
            .text_style(egui::TextStyle::Name("Title".into()))
            .color(theme.text()),
    );
    ui.add_space(4.0);
}

fn caption(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.label(
        RichText::new(text)
            .text_style(egui::TextStyle::Small)
            .color(theme.text_secondary()),
    );
}

fn remove_panel(
    ui: &mut egui::Ui,
    state: &mut KilnState,
    theme: &Theme,
    tx: &EventTx,
    caches: &Arc<Caches>,
    dims: Option<(u32, u32)>,
) {
    image_facts(ui, state, theme, dims);

    ui.add_space(12.0);
    section_title(ui, theme, "Refine");
    refine_controls(ui, state, theme);

    ui.add_space(12.0);
    destination_group(ui, state, theme, tx);

    ui.add_space(12.0);
    section_title(ui, theme, "Export");
    let model_ok = state.model_installed.unwrap_or(false);
    let busy = state.exporting_tool.is_some();
    let count = state.queue().len();
    let export_label = format!(
        "Export {} image{}",
        count,
        if count == 1 { "" } else { "s" }
    );

    ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
        let export = ui.add_enabled(
            model_ok && !busy && count > 0 && state.destination.is_some(),
            Button::new(RichText::new(&export_label).color(theme.text())),
        );
        if export.clicked() {
            crate::app::dispatch_export(state, tx, caches);
        }
        let copy_enabled = !busy && model_ok && state.current().is_some();
        if ui
            .add_enabled(
                copy_enabled,
                Button::new(RichText::new("Copy").color(theme.text())),
            )
            .on_hover_text("Copy the cutout to the clipboard")
            .clicked()
            && let Some(entry) = state.current().cloned()
        {
            copy_remove(
                entry.path,
                kiln::MatteOptions {
                    threshold: state.remove_opts.threshold,
                    feather: state.remove_opts.feather,
                    dilate: state.remove_opts.dilate,
                    trim: false,
                },
                Arc::clone(caches),
                tx.clone(),
            );
        }
    });
    if !model_ok {
        ui.add_space(4.0);
        caption(
            ui,
            theme,
            "Export is disabled until the model is back in place.",
        );
    }
}

fn refine_controls(ui: &mut egui::Ui, state: &mut KilnState, theme: &Theme) {
    // Threshold: starts off; switching it on gives the hard cut.
    let threshold_on = state.remove_opts.threshold.is_some();
    let toggle = ui.add(
        Button::new(
            RichText::new(if threshold_on {
                "Threshold: on"
            } else {
                "Threshold: off"
            })
            .color(theme.text()),
        )
        .small(),
    );
    if toggle.clicked() {
        state.remove_opts.threshold = if threshold_on { None } else { Some(128) };
        crate::app::schedule_refine(state);
    }
    let mut threshold = state.remove_opts.threshold.unwrap_or(0);
    let mut changed = false;
    ui.add_enabled_ui(threshold_on, |ui| {
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            let slider = ui.add_sized(
                [140.0, 20.0],
                egui::Slider::new(&mut threshold, 0..=255).text(""),
            );
            let typed = ui.add(
                egui::DragValue::new(&mut threshold)
                    .range(0..=255)
                    .speed(1.0),
            );
            if slider.changed() || typed.changed() {
                changed = true;
            }
        });
    });
    if threshold_on {
        state.remove_opts.threshold = Some(threshold);
        if changed {
            crate::app::schedule_refine(state);
        }
    }
    caption(
        ui,
        theme,
        "Hard cut below this alpha. Off keeps the soft edge.",
    );

    ui.add_space(4.0);
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        ui.label(RichText::new("Feather").color(theme.text_secondary()));
        let slider = ui.add_sized(
            [110.0, 20.0],
            egui::Slider::new(&mut state.remove_opts.feather, 0.0..=10.0).text(""),
        );
        let typed = ui.add(
            egui::DragValue::new(&mut state.remove_opts.feather)
                .speed(0.1)
                .suffix(" px"),
        );
        if slider.changed() || typed.changed() {
            crate::app::schedule_refine(state);
        }
    });
    ui.add_space(4.0);
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        ui.label(RichText::new("Dilate").color(theme.text_secondary()));
        let mut dilate = i32::try_from(state.remove_opts.dilate).unwrap_or(0);
        let slider = ui.add_sized(
            [110.0, 20.0],
            egui::Slider::new(&mut dilate, 0..=10).text(""),
        );
        let typed = ui.add(
            egui::DragValue::new(&mut dilate)
                .range(0..=10)
                .suffix(" px"),
        );
        if slider.changed() || typed.changed() {
            state.remove_opts.dilate = u32::try_from(dilate).unwrap_or(0);
            crate::app::schedule_refine(state);
        } else {
            state.remove_opts.dilate = u32::try_from(dilate).unwrap_or(0);
        }
    });
    ui.add_space(4.0);
    let trim = ui.checkbox(&mut state.remove_opts.trim, "Trim to subject");
    if trim.changed() {
        crate::app::schedule_refine(state);
    }
    ui.add_space(4.0);
    if ui
        .add(Button::new(RichText::new("Reset all").color(theme.text_secondary())).small())
        .clicked()
    {
        state.remove_opts.reset();
        crate::app::schedule_refine(state);
    }
}

fn image_facts(ui: &mut egui::Ui, state: &mut KilnState, theme: &Theme, dims: Option<(u32, u32)>) {
    section_title(ui, theme, "Image");
    let Some(entry) = state.current() else {
        caption(ui, theme, "Nothing selected.");
        return;
    };
    ui.label(
        RichText::new(&entry.label)
            .text_style(egui::TextStyle::Body)
            .color(theme.text()),
    );
    if let Some((width, height)) = dims {
        ui.label(
            RichText::new(format!("{width} \u{00D7} {height}"))
                .text_style(egui::TextStyle::Name("Mono".into()))
                .color(theme.text_secondary()),
        );
    }
    if let Ok(meta) = std::fs::metadata(&entry.path) {
        ui.label(
            RichText::new(format_bytes(meta.len()))
                .text_style(egui::TextStyle::Name("Mono".into()))
                .color(theme.text_secondary()),
        );
    }
    let queue = state.queue().len();
    if queue > 1 {
        caption(ui, theme, &format!("{queue} in queue"));
    }
}

fn merge_panel(
    ui: &mut egui::Ui,
    state: &mut KilnState,
    theme: &Theme,
    tx: &EventTx,
    caches: &Arc<Caches>,
) {
    section_title(ui, theme, "Template");
    if let Some(template) = state.template.clone() {
        let name = template.file_name().map_or_else(
            || "template".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        ui.label(
            RichText::new(name)
                .text_style(egui::TextStyle::Body)
                .color(theme.text()),
        );
        if ui
            .add(Button::new(RichText::new("Change template").color(theme.text())).small())
            .clicked()
        {
            pick_template(tx.clone());
        }
    } else {
        caption(ui, theme, "One transparent PNG. The hole shows the photo.");
        if ui
            .add(Button::new(
                RichText::new("Browse template").color(theme.text()),
            ))
            .clicked()
        {
            pick_template(tx.clone());
        }
    }

    ui.add_space(12.0);
    section_title(ui, theme, "Fit");
    ui.horizontal(|ui| {
        for fit in MergeFit::all() {
            let active = state.fit == fit;
            let mut button = Button::new(RichText::new(fit.label()).color(if active {
                theme.text()
            } else {
                theme.text_secondary()
            }))
            .small();
            if active {
                button = button.fill(theme.border_strong());
            }
            if ui.add(button).clicked() {
                state.fit = fit;
            }
        }
    });
    caption(
        ui,
        theme,
        match state.fit {
            MergeFit::Cover => "Fills the template, crops what sticks out.",
            MergeFit::Fit => "Shows the whole photo, transparency around it.",
            MergeFit::Stretch => "Forces the photo to template size.",
        },
    );

    ui.add_space(12.0);
    image_facts(ui, state, theme, None);

    ui.add_space(12.0);
    destination_group(ui, state, theme, tx);

    ui.add_space(12.0);
    section_title(ui, theme, "Export");
    let busy = state.exporting_tool.is_some();
    let count = state.queue().len();
    let export_label = format!(
        "Export {} image{}",
        count,
        if count == 1 { "" } else { "s" }
    );
    ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
        let export = ui.add_enabled(
            !busy && count > 0 && state.template.is_some() && state.destination.is_some(),
            Button::new(RichText::new(&export_label).color(theme.text())),
        );
        if export.clicked() {
            crate::app::dispatch_export(state, tx, caches);
        }
        let copy_enabled = !busy && state.current().is_some();
        if ui
            .add_enabled(
                copy_enabled,
                Button::new(RichText::new("Copy").color(theme.text())),
            )
            .on_hover_text("Copy the photo to the clipboard")
            .clicked()
            && let Some(entry) = state.current().cloned()
        {
            copy_photo(entry.path, tx.clone());
        }
    });
    if state.template.is_none() {
        ui.add_space(4.0);
        caption(ui, theme, "Merge preview needs a template first.");
    }
}

fn destination_group(ui: &mut egui::Ui, state: &mut KilnState, theme: &Theme, tx: &EventTx) {
    section_title(ui, theme, "Destination");
    let shown = state.destination.clone().map_or_else(
        || "same folder as the first image".to_owned(),
        |dir| dir.to_string_lossy().into_owned(),
    );
    ui.add(
        egui::Label::new(
            RichText::new(shown)
                .text_style(egui::TextStyle::Small)
                .color(theme.text_secondary()),
        )
        .truncate(),
    );
    ui.horizontal(|ui| {
        if ui
            .add(Button::new(RichText::new("Browse folder").color(theme.text())).small())
            .clicked()
        {
            pick_folder(tx.clone());
        }
        if ui
            .add(Button::new(RichText::new("Save to\u{2026}").color(theme.text())).small())
            .on_hover_text("Pick the folder and export in one step")
            .clicked()
        {
            state.export_after_folder_pick = true;
            pick_folder(tx.clone());
        }
    });
}

fn status_line(ui: &mut egui::Ui, state: &mut KilnState, theme: &Theme) {
    ui.separator();
    if let Some(summary) = &state.summary {
        ui.label(
            RichText::new(summary)
                .text_style(egui::TextStyle::Small)
                .color(theme.text_secondary()),
        );
    }
    if let Some(status) = &state.status {
        ui.add(
            egui::Label::new(
                RichText::new(status)
                    .text_style(egui::TextStyle::Small)
                    .color(theme.danger()),
            )
            .wrap(),
        );
    }
    if state.batch.is_none() && state.summary.is_none() && state.status.is_none() {
        ui.label(
            RichText::new(format!(
                "{} image{} \u{00B7} batch bar is 20",
                state.queue().len(),
                if state.queue().len() == 1 { "" } else { "s" }
            ))
            .text_style(egui::TextStyle::Small)
            .color(theme.text_disabled()),
        );
    }
}

/// Bytes into a human shape.
#[must_use]
pub fn format_bytes(len: u64) -> String {
    let units = ["B", "KB", "MB", "GB"];
    let mut value = f64::from(u32::try_from(len.min(u64::from(u32::MAX))).unwrap_or(u32::MAX));
    let mut unit = 0;
    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{len} B")
    } else {
        format!("{value:.1} {}", units[unit])
    }
}
