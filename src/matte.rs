use image::{GrayImage, RgbaImage, imageops::FilterType};

/// Output of the network: a low-resolution matte in `[-1, 1]`.
///
/// Rescaling matters. RMBG's raw output regularly exceeds `[-1, 1]`, so the
/// naive `(x + 1) * 0.5` mapping clips and leaves faint background residue — the
/// classic "dirty" cutout. Min-max normalising first is what makes alpha clean.
#[must_use]
pub fn normalize(raw: &[f32]) -> Vec<u8> {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for &value in raw {
        if value.is_nan() {
            continue;
        }
        min = min.min(value);
        max = max.max(value);
    }

    let span = max - min;
    raw.iter()
        .map(|&value| {
            if !span.is_finite() || span <= f32::EPSILON {
                return 0;
            }
            // Clamped to 0..=255 first, so neither cast can lose or flip a sign.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                ((value - min) / span * 255.0).round().clamp(0.0, 255.0) as u8
            }
        })
        .collect()
}

/// Turn network output at model resolution into a matte at image resolution.
#[must_use]
pub fn to_matte(raw: &[f32], model_size: usize, width: u32, height: u32) -> GrayImage {
    let mask = normalize(raw);

    // Only INPUT_SIZE reaches here, far below u32::MAX.
    #[allow(clippy::cast_possible_truncation)]
    let side = model_size as u32;

    let source =
        GrayImage::from_raw(side, side, mask).unwrap_or_else(|| GrayImage::new(side, side));

    image::imageops::resize(&source, width, height, FilterType::Triangle)
}

/// Parameters for turning a raw matte into a final alpha channel.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MatteOptions {
    /// Below this the pixel is dropped entirely. `None` keeps the soft edge.
    pub threshold: Option<u8>,
    /// Gaussian sigma applied before thresholding. Widens the transition band.
    pub feather: f32,
    /// Grow the solid region by this many pixels.
    pub dilate: u32,
    /// Crop to the subject's bounding box.
    pub trim: bool,
}

/// Apply refinement passes. Borrows the matte so slider drags never copy it;
/// each pass returns its own buffer and the input stays usable.
#[must_use]
pub fn refine(matte: &GrayImage, options: MatteOptions) -> GrayImage {
    let mut out = matte.clone();

    if options.feather > 0.0 {
        out = image::imageops::blur(&out, options.feather);
    }

    if let Some(cutoff) = options.threshold {
        let (width, height) = (out.width(), out.height());
        let raw = out.as_raw();
        let cut = raw
            .iter()
            .map(|&alpha| if alpha >= cutoff { u8::MAX } else { 0 })
            .collect();
        out = GrayImage::from_raw(width, height, cut)
            .unwrap_or_else(|| GrayImage::new(width, height));
    }

    if options.dilate > 0 {
        out = dilate(&out, options.dilate);
    }

    out
}

/// Crop to the matte's non-transparent bounding box.
///
/// Returns `None` when the matte is entirely empty, which means there is no
/// subject to crop to and the caller should keep the uncropped image.
#[must_use]
pub fn trim(pixels: &RgbaImage, matte: &GrayImage) -> Option<RgbaImage> {
    let mut min_x = matte.width();
    let mut min_y = matte.height();
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut found = false;

    for (x, y, pixel) in matte.enumerate_pixels() {
        if pixel.0[0] > TRANSPARENCY_CUTOFF {
            found = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }

    if !found {
        return None;
    }

    let width = max_x - min_x + 1;
    let height = max_y - min_y + 1;
    Some(image::imageops::crop_imm(pixels, min_x, min_y, width, height).to_image())
}

/// Alpha below this counts as nothing when finding the subject's bounds.
const TRANSPARENCY_CUTOFF: u8 = 8;

/// Grow the bright region by `radius` pixels.
///
/// A square structuring element applied `radius` times. Repeated 1-pixel
/// dilation is the same as one larger dilation for this shape. Buffers swap
/// instead of allocating per pass, and pixels are read from raw slices so the
/// tight loop pays no bounds check.
fn dilate(matte: &GrayImage, radius: u32) -> GrayImage {
    let (width, height) = matte.dimensions();
    if radius == 0 || width == 0 || height == 0 {
        return matte.clone();
    }

    let stride = width as usize;
    let mut current = matte.as_raw().clone();
    let mut next = vec![0u8; current.len()];

    for _ in 0..radius {
        for (index, slot) in next.iter_mut().enumerate() {
            let x = index % stride;
            let y = index / stride;
            let bright = |px: usize, py: usize| {
                current
                    .get(px + py * stride)
                    .is_some_and(|&alpha| alpha >= DILATE_CUTOFF)
            };
            let set = current[index] >= DILATE_CUTOFF
                || (x > 0 && bright(x - 1, y))
                || bright(x + 1, y)
                || (y > 0 && bright(x, y - 1))
                || bright(x, y + 1);
            *slot = u8::from(set) * u8::MAX;
        }
        std::mem::swap(&mut current, &mut next);
    }

    GrayImage::from_raw(width, height, current).unwrap_or_else(|| GrayImage::new(width, height))
}

/// Anything at or above this is solid enough to grow from.
const DILATE_CUTOFF: u8 = 128;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_stretches_actual_range() {
        // Values outside the nominal [-1, 1] must still reach the full 0..255.
        let raw = [-1.4, -0.5, 0.0, 0.5, 1.2];
        let out = normalize(&raw);

        assert_eq!(out.first(), Some(&0));
        assert_eq!(out.last(), Some(&255));
        // Midpoint of the observed range, not of [-1, 1].
        assert_eq!(out[2], 137);
    }

    #[test]
    fn normalize_handles_flat_input() {
        assert_eq!(normalize(&[0.5; 16]), vec![0; 16]);
    }

    #[test]
    fn normalize_skips_nan() {
        let out = normalize(&[f32::NAN, 0.0, 1.0]);
        assert_eq!(out, vec![0, 0, 255]);
    }

    #[test]
    fn threshold_drops_soft_pixels() {
        let matte = GrayImage::from_fn(4, 1, |x, _| {
            image::Luma([u8::try_from(x * 80).unwrap_or(255)])
        });
        let out = refine(
            &matte,
            MatteOptions {
                threshold: Some(128),
                ..MatteOptions::default()
            },
        );

        assert_eq!(out.get_pixel(0, 0).0[0], 0);
        assert_eq!(out.get_pixel(3, 0).0[0], u8::MAX);
    }

    #[test]
    fn dilate_grows_one_pixel_outward() {
        let matte =
            GrayImage::from_fn(5, 1, |x, _| image::Luma([if x == 2 { u8::MAX } else { 0 }]));
        let out = refine(
            &matte,
            MatteOptions {
                dilate: 1,
                ..MatteOptions::default()
            },
        );

        assert_eq!(out.get_pixel(2, 0).0[0], u8::MAX);
        assert_eq!(out.get_pixel(1, 0).0[0], u8::MAX);
        assert_eq!(out.get_pixel(3, 0).0[0], u8::MAX);
        assert_eq!(out.get_pixel(0, 0).0[0], 0);
        assert_eq!(out.get_pixel(4, 0).0[0], 0);
    }

    #[test]
    fn refine_leaves_input_usable() {
        let matte = GrayImage::from_pixel(2, 2, image::Luma([200]));
        let before = matte.as_raw().clone();

        let _ = refine(
            &matte,
            MatteOptions {
                threshold: Some(128),
                dilate: 1,
                ..MatteOptions::default()
            },
        );

        assert_eq!(matte.as_raw(), &before);
    }

    #[test]
    fn trim_returns_none_for_empty_matte() {
        let pixels = image::RgbaImage::new(8, 8);
        assert!(trim(&pixels, &GrayImage::new(8, 8)).is_none());
    }

    #[test]
    fn trim_crops_to_subject() {
        let matte = GrayImage::from_fn(10, 10, |x, y| {
            image::Luma([if (2..=5).contains(&x) && (3..=6).contains(&y) {
                u8::MAX
            } else {
                0
            }])
        });
        let pixels = image::RgbaImage::new(10, 10);
        let cropped = trim(&pixels, &matte).expect("matte has a subject");

        assert_eq!(cropped.width(), 4);
        assert_eq!(cropped.height(), 4);
    }
}
