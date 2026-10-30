//! Template merge maths.
//!
//! A template is one PNG with transparency, usually a frame with a clear
//! hole. Each photo is scaled to the template size, then the template is
//! drawn over it. Output size always equals template size.

use image::RgbaImage;

use crate::{ToolError, ToolResult};

/// How a photo is sized to the template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "lowercase")
)]
pub enum MergeFit {
    /// Fill the template, cropping what sticks out.
    #[default]
    Cover,
    /// Show the whole photo, leaving transparency around it.
    Fit,
    /// Force the photo to template size, ignoring shape.
    Stretch,
}

impl MergeFit {
    /// Short label shown on the fit buttons.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Cover => "Cover",
            Self::Fit => "Fit",
            Self::Stretch => "Stretch",
        }
    }

    /// Every fit, in button order.
    #[must_use]
    pub fn all() -> [Self; 3] {
        [Self::Cover, Self::Fit, Self::Stretch]
    }
}

/// Draw a photo under a template.
///
/// The photo is scaled per `fit`, centred, then the template is blended over
/// it. Transparent template areas show the photo through.
///
/// # Errors
///
/// Returns [`ToolError::Invalid`] when either image is empty.
pub fn composite(photo: &RgbaImage, template: &RgbaImage, fit: MergeFit) -> ToolResult<RgbaImage> {
    let (template_width, template_height) = template.dimensions();
    let (photo_width, photo_height) = photo.dimensions();
    if template_width == 0 || template_height == 0 {
        return Err(ToolError::Invalid("template has no pixels".to_owned()));
    }
    if photo_width == 0 || photo_height == 0 {
        return Err(ToolError::Invalid("photo has no pixels".to_owned()));
    }
    let (scaled_width, scaled_height) = scaled_size(
        photo_width,
        photo_height,
        template_width,
        template_height,
        fit,
    );
    let sized = image::imageops::resize(
        photo,
        scaled_width,
        scaled_height,
        image::imageops::FilterType::Triangle,
    );
    let shift_x = i64::from(template_width) - i64::from(scaled_width);
    let shift_y = i64::from(template_height) - i64::from(scaled_height);
    let photo_raw = sized.as_raw();
    let template_raw = template.as_raw();
    let mut out = Vec::with_capacity(photo_raw.len().max(template_raw.len()));
    for row in 0..template_height {
        for col in 0..template_width {
            let sample_x = i64::from(col) - shift_x / 2;
            let sample_y = i64::from(row) - shift_y / 2;
            let under = sample_at(photo_raw, scaled_width, scaled_height, sample_x, sample_y);
            let over_pixel = template_pixel(template_raw, template_width, col, row);
            let [r, g, b, a] = blend_over(over_pixel, under);
            out.extend_from_slice(&[r, g, b, a]);
        }
    }
    RgbaImage::from_raw(template_width, template_height, out)
        .ok_or_else(|| ToolError::Invalid("merged pixels do not match template size".to_owned()))
}

/// Photo size after fitting to the template.
fn scaled_size(
    photo_width: u32,
    photo_height: u32,
    template_width: u32,
    template_height: u32,
    fit: MergeFit,
) -> (u32, u32) {
    if matches!(fit, MergeFit::Stretch) {
        return (template_width.max(1), template_height.max(1));
    }
    let scale = {
        let wide = f64::from(template_width) / f64::from(photo_width.max(1));
        let tall = f64::from(template_height) / f64::from(photo_height.max(1));
        if matches!(fit, MergeFit::Cover) {
            wide.max(tall)
        } else {
            wide.min(tall)
        }
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let width = (f64::from(photo_width) * scale).round().clamp(1.0, 32768.0) as u32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let height = (f64::from(photo_height) * scale)
        .round()
        .clamp(1.0, 32768.0) as u32;
    (width, height)
}

/// One photo pixel, or transparency when the sample falls outside.
fn sample_at(raw: &[u8], width: u32, height: u32, x: i64, y: i64) -> [u8; 4] {
    if x < 0 || y < 0 {
        return [0, 0, 0, 0];
    }
    #[allow(clippy::cast_sign_loss)]
    let (x, y) = (x as u64, y as u64);
    if x >= u64::from(width) || y >= u64::from(height) {
        return [0, 0, 0, 0];
    }
    let at = (y * u64::from(width) + x) * 4;
    #[allow(clippy::cast_possible_truncation)]
    let at = at as usize;
    [raw[at], raw[at + 1], raw[at + 2], raw[at + 3]]
}

/// One template pixel by column and row.
fn template_pixel(raw: &[u8], stride: u32, col: u32, row: u32) -> [u8; 4] {
    let at = (u64::from(row) * u64::from(stride) + u64::from(col)) * 4;
    #[allow(clippy::cast_possible_truncation)]
    let at = at as usize;
    [raw[at], raw[at + 1], raw[at + 2], raw[at + 3]]
}

/// Standard source-over blend, in integer maths.
fn blend_over(foreground: [u8; 4], background: [u8; 4]) -> [u8; 4] {
    let alpha = u32::from(foreground[3]);
    if alpha == 255 {
        return foreground;
    }
    if alpha == 0 {
        return background;
    }
    let back_alpha = u32::from(background[3]);
    let out_alpha = alpha + back_alpha * (255 - alpha) / 255;
    if out_alpha == 0 {
        return [0, 0, 0, 0];
    }
    let mix = |top: u8, bottom: u8| {
        (u32::from(top) * alpha + u32::from(bottom) * back_alpha * (255 - alpha) / 255) / out_alpha
    };
    #[allow(clippy::cast_possible_truncation)]
    let (r, g, b) = (
        mix(foreground[0], background[0]) as u8,
        mix(foreground[1], background[1]) as u8,
        mix(foreground[2], background[2]) as u8,
    );
    #[allow(clippy::cast_possible_truncation)]
    let a = out_alpha as u8;
    [r, g, b, a]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_fills_and_crops() {
        assert_eq!(
            scaled_size(800, 600, 1000, 1000, MergeFit::Cover),
            (1333, 1000)
        );
    }

    #[test]
    fn fit_shows_all() {
        assert_eq!(
            scaled_size(800, 600, 1000, 1000, MergeFit::Fit),
            (1000, 750)
        );
    }

    #[test]
    fn stretch_ignores_shape() {
        assert_eq!(
            scaled_size(800, 600, 1000, 1000, MergeFit::Stretch),
            (1000, 1000)
        );
    }

    #[test]
    fn opaque_template_covers_photo() {
        let photo = RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
        let template = RgbaImage::from_pixel(4, 4, image::Rgba([0, 255, 0, 255]));

        let out = composite(&photo, &template, MergeFit::Cover).expect("valid sizes");

        assert_eq!(out.dimensions(), (4, 4));
        assert_eq!(out.get_pixel(0, 0).0, [0, 255, 0, 255]);
        assert_eq!(out.get_pixel(3, 3).0, [0, 255, 0, 255]);
    }

    #[test]
    fn clear_template_shows_photo() {
        let photo = RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]));
        let template = RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 0, 0]));

        let out = composite(&photo, &template, MergeFit::Stretch).expect("valid sizes");

        assert_eq!(out.get_pixel(1, 2).0, [255, 0, 0, 255]);
    }

    #[test]
    fn empty_template_is_rejected() {
        let photo = RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));

        assert!(composite(&photo, &RgbaImage::new(0, 0), MergeFit::Cover).is_err());
    }
}
