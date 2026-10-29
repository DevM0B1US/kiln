//! Bundled fonts. `Inter` for text, `JetBrains Mono` for every number.
//!
//! Both ship under the SIL Open Font License (see `assets/fonts/`).

use egui::FontDefinitions;

const INTER_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-SemiBold.ttf");
const JETBRAINS_MONO: &[u8] = include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf");

/// Register the exact type ramp: `Inter` proportional, `JetBrains Mono` for
/// numbers, and a semibold family for panel titles.
pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "inter_regular".into(),
        egui::FontData::from_static(INTER_REGULAR).into(),
    );
    fonts.font_data.insert(
        "inter_semibold".into(),
        egui::FontData::from_static(INTER_SEMIBOLD).into(),
    );
    fonts.font_data.insert(
        "jetbrains_mono".into(),
        egui::FontData::from_static(JETBRAINS_MONO).into(),
    );

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "inter_regular".into());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "jetbrains_mono".into());
    fonts
        .families
        .entry(egui::FontFamily::Name("SemiBold".into()))
        .or_default()
        .push("inter_semibold".into());

    ctx.set_fonts(fonts);
}
