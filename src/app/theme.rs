//! The visual contract from `UI.md`, expressed as egui style.
//!
//! Dark is default. Bright surrounds hide shadow detail in a matte. Danger
//! red is the only accent; everything else is grey. 4px corners on buttons
//! and cards, square elsewhere. No transitions; state changes cut.

use egui::{Color32, CornerRadius, FontId, Style, Visuals};

/// Which palette the window uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeChoice {
    /// Dark by default.
    #[default]
    Dark,
    /// Light, for bright rooms.
    Light,
}

impl ThemeChoice {
    /// The other palette.
    #[must_use]
    pub fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}

/// Stage backdrops. The checkerboard forgives both halo kinds; solid
/// backdrops expose them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Backdrop {
    /// 16px checker cells.
    #[default]
    Checker,
    /// Solid dark — exposes light halos.
    Dark,
    /// Solid white — exposes dark fringes.
    White,
}

impl Backdrop {
    /// Every backdrop, in swatch order.
    #[must_use]
    pub fn all() -> [Self; 3] {
        [Self::Checker, Self::Dark, Self::White]
    }
}

/// One palette's exact tokens.
struct Tokens {
    background: Color32,
    surface: Color32,
    surface_raised: Color32,
    border: Color32,
    border_strong: Color32,
    text_primary: Color32,
    text_secondary: Color32,
    text_disabled: Color32,
    danger: Color32,
    checker_a: Color32,
    checker_b: Color32,
}

const DARK: Tokens = Tokens {
    background: Color32::from_rgb(0x14, 0x14, 0x14),
    surface: Color32::from_rgb(0x1c, 0x1c, 0x1c),
    surface_raised: Color32::from_rgb(0x24, 0x24, 0x24),
    border: Color32::from_rgb(0x2e, 0x2e, 0x2e),
    border_strong: Color32::from_rgb(0x3d, 0x3d, 0x3d),
    text_primary: Color32::from_rgb(0xe8, 0xe8, 0xe8),
    text_secondary: Color32::from_rgb(0x9a, 0x9a, 0x9a),
    text_disabled: Color32::from_rgb(0x5c, 0x5c, 0x5c),
    danger: Color32::from_rgb(0xc0, 0x57, 0x4f),
    checker_a: Color32::from_rgb(0x1e, 0x1e, 0x1e),
    checker_b: Color32::from_rgb(0x2a, 0x2a, 0x2a),
};

const LIGHT: Tokens = Tokens {
    background: Color32::from_rgb(0xf4, 0xf4, 0xf4),
    surface: Color32::from_rgb(0xff, 0xff, 0xff),
    surface_raised: Color32::from_rgb(0xeb, 0xeb, 0xeb),
    border: Color32::from_rgb(0xdc, 0xdc, 0xdc),
    border_strong: Color32::from_rgb(0xc2, 0xc2, 0xc2),
    text_primary: Color32::from_rgb(0x1a, 0x1a, 0x1a),
    text_secondary: Color32::from_rgb(0x6b, 0x6b, 0x6b),
    text_disabled: Color32::from_rgb(0xa8, 0xa8, 0xa8),
    danger: Color32::from_rgb(0xb0, 0x3a, 0x30),
    checker_a: Color32::from_rgb(0xc4, 0xc4, 0xc4),
    checker_b: Color32::from_rgb(0xd8, 0xd8, 0xd8),
};

/// Palette + font styles resolved for painting and layout.
pub struct Theme {
    tokens: Tokens,
    /// Base text style, 12px.
    pub label: FontId,
    /// Captions, 11px secondary.
    pub caption: FontId,
    /// Numbers, 12px mono.
    pub mono: FontId,
    /// Big mono for the working pill, 13px.
    pub mono_strong: FontId,
}

impl Theme {
    #[must_use]
    pub fn new(choice: ThemeChoice) -> Self {
        let tokens = match choice {
            ThemeChoice::Dark => DARK,
            ThemeChoice::Light => LIGHT,
        };
        let family = egui::FontFamily::Proportional;
        let mono = egui::FontFamily::Monospace;
        Self {
            tokens,
            label: FontId::new(12.0, family.clone()),
            caption: FontId::new(11.0, family.clone()),
            mono: FontId::new(12.0, mono.clone()),
            mono_strong: FontId::new(13.0, mono),
        }
    }

    #[must_use]
    pub fn background(&self) -> Color32 {
        self.tokens.background
    }

    #[must_use]
    pub fn surface(&self) -> Color32 {
        self.tokens.surface
    }

    #[must_use]
    pub fn surface_raised(&self) -> Color32 {
        self.tokens.surface_raised
    }

    #[must_use]
    pub fn border(&self) -> Color32 {
        self.tokens.border
    }

    #[must_use]
    pub fn border_strong(&self) -> Color32 {
        self.tokens.border_strong
    }

    #[must_use]
    pub fn text(&self) -> Color32 {
        self.tokens.text_primary
    }

    #[must_use]
    pub fn text_secondary(&self) -> Color32 {
        self.tokens.text_secondary
    }

    #[must_use]
    pub fn text_disabled(&self) -> Color32 {
        self.tokens.text_disabled
    }

    #[must_use]
    pub fn danger(&self) -> Color32 {
        self.tokens.danger
    }

    #[must_use]
    pub fn checker_a(&self) -> Color32 {
        self.tokens.checker_a
    }

    #[must_use]
    pub fn checker_b(&self) -> Color32 {
        self.tokens.checker_b
    }

    /// Solid stage backdrop colour, independent of the active palette: a
    /// dark backdrop must stay dark in light mode to expose light halos.
    #[must_use]
    pub fn backdrop_solid(&self, backdrop: Backdrop) -> Color32 {
        match backdrop {
            Backdrop::Dark => DARK.background,
            Backdrop::White => LIGHT.background,
            Backdrop::Checker => self.tokens.background,
        }
    }

    /// Apply the palette to egui's style: greys, square corners, 4px on
    /// widgets, 4px base spacing, exact type ramp.
    pub fn apply(&self, style: &mut Style) {
        style.visuals = self.visuals();
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.menu_margin = egui::Margin::same(4);
        style.spacing.interact_size = egui::vec2(64.0, 24.0);
        let mut text_styles = style.text_styles.clone();
        text_styles.insert(egui::TextStyle::Body, self.label.clone());
        text_styles.insert(egui::TextStyle::Button, self.label.clone());
        text_styles.insert(egui::TextStyle::Small, self.caption.clone());
        text_styles.insert(
            egui::TextStyle::Name("Title".into()),
            FontId::new(13.0, FontFamilySemi::family()),
        );
        text_styles.insert(egui::TextStyle::Name("Mono".into()), self.mono.clone());
        text_styles.insert(
            egui::TextStyle::Name("MonoStrong".into()),
            self.mono_strong.clone(),
        );
        style.text_styles = text_styles;
    }

    fn visuals(&self) -> Visuals {
        let t = &self.tokens;
        let stroke = |color: Color32| egui::Stroke::new(1.0, color);
        let is_light = self.tokens.background == LIGHT.background;
        let mut visuals = if is_light {
            Visuals::light()
        } else {
            Visuals::dark()
        };
        visuals.override_text_color = Some(t.text_primary);
        visuals.panel_fill = t.surface;
        visuals.window_fill = t.surface;
        visuals.extreme_bg_color = t.surface_raised;
        visuals.faint_bg_color = t.background;
        visuals.window_stroke = stroke(t.border);
        visuals.widgets.noninteractive.bg_fill = t.surface;
        visuals.widgets.noninteractive.weak_bg_fill = t.surface;
        visuals.widgets.noninteractive.fg_stroke = stroke(t.text_secondary);
        visuals.widgets.noninteractive.bg_stroke = stroke(t.border);
        visuals.widgets.inactive.bg_fill = t.surface_raised;
        visuals.widgets.inactive.weak_bg_fill = t.surface_raised;
        visuals.widgets.inactive.fg_stroke = stroke(t.text_primary);
        visuals.widgets.inactive.bg_stroke = stroke(t.border);
        visuals.widgets.hovered.bg_fill = t.border;
        visuals.widgets.hovered.weak_bg_fill = t.border;
        visuals.widgets.hovered.fg_stroke = stroke(t.text_primary);
        visuals.widgets.hovered.bg_stroke = stroke(t.border_strong);
        visuals.widgets.active.bg_fill = t.border_strong;
        visuals.widgets.active.weak_bg_fill = t.border_strong;
        visuals.widgets.active.fg_stroke = stroke(t.text_primary);
        visuals.widgets.active.bg_stroke = stroke(t.border_strong);
        visuals.widgets.open.fg_stroke = stroke(t.text_disabled);
        visuals.widgets.open.bg_stroke = stroke(t.border);
        visuals.selection.bg_fill = t.border_strong;
        visuals.selection.stroke = stroke(t.text_primary);
        // 4px on buttons and cards, square elsewhere.
        visuals.widgets.inactive.corner_radius = CornerRadius::same(4);
        visuals.widgets.hovered.corner_radius = CornerRadius::same(4);
        visuals.widgets.active.corner_radius = CornerRadius::same(4);
        visuals.widgets.noninteractive.corner_radius = CornerRadius::same(0);
        visuals.widgets.open.corner_radius = CornerRadius::same(4);
        visuals.menu_corner_radius = CornerRadius::same(4);
        visuals.popup_shadow = egui::epaint::Shadow::NONE;
        visuals.window_shadow = egui::epaint::Shadow::NONE;
        visuals
    }
}

/// Custom font family carrying the semibold face, used by panel titles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontFamilySemi;

impl FontFamilySemi {
    /// The registered family name; matches `fonts.rs`.
    #[must_use]
    pub fn family() -> egui::FontFamily {
        egui::FontFamily::Name("SemiBold".into())
    }
}
