use image::{DynamicImage, GenericImageView, RgbaImage};

pub const PREVIEW_EDGE: u32 = 1280;

pub fn fit_edge(width: u32, height: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= PREVIEW_EDGE || longest == 0 {
        return (width, height);
    }
    let scale = f64::from(PREVIEW_EDGE) / f64::from(longest);
    let scaled_width = (f64::from(width) * scale).round().max(1.0) as u32;
    let scaled_height = (f64::from(height) * scale).round().max(1.0) as u32;
    (scaled_width, scaled_height)
}

pub fn encode_png_url(pixels: &[u8], width: u32, height: u32) -> Result<String, String> {
    let mut raw = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new_with_quality(
        &mut raw,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::Sub,
    );
    use image::ImageEncoder as _;
    encoder
        .write_image(pixels, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|error| format!("cannot encode preview: {error}"))?;
    use base64::Engine as _;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&raw)
    ))
}

pub fn preview_url(source: &DynamicImage) -> Result<String, String> {
    let (width, height) = source.dimensions();
    let (fit_width, fit_height) = fit_edge(width, height);
    if (fit_width, fit_height) == (width, height) {
        let rgba = source.to_rgba8();
        return encode_png_url(rgba.as_raw(), width, height);
    }
    let small = source.resize(fit_width, fit_height, image::imageops::FilterType::Triangle);
    let rgba = small.to_rgba8();
    encode_png_url(rgba.as_raw(), fit_width, fit_height)
}

pub fn small_rgba(source: &DynamicImage) -> RgbaImage {
    let (width, height) = source.dimensions();
    let (fit_width, fit_height) = fit_edge(width, height);
    if (fit_width, fit_height) == (width, height) {
        return source.to_rgba8();
    }
    source
        .resize(fit_width, fit_height, image::imageops::FilterType::Triangle)
        .to_rgba8()
}

pub fn small_matte(matte: &image::GrayImage, width: u32, height: u32) -> image::GrayImage {
    let (fit_width, fit_height) = fit_edge(width, height);
    if (fit_width, fit_height) == (width, height) {
        return matte.clone();
    }
    image::imageops::resize(
        matte,
        fit_width,
        fit_height,
        image::imageops::FilterType::Triangle,
    )
}
