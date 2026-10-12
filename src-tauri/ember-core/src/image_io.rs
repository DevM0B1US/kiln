use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use image::{DynamicImage, GenericImageView, RgbaImage};

use crate::{ToolError, ToolResult};

/// Longest edge accepted before decoding. Past this, buffers get absurd.
pub const MAX_IMAGE_EDGE: u32 = 8192;

/// Pixel count accepted before decoding. 24 MP works, bombs do not.
pub const MAX_IMAGE_PIXELS: u64 = 50_000_000;

/// Reject absurd sizes before a single pixel is allocated.
///
/// # Errors
///
/// Returns [`ToolError::Invalid`] when the image is past the edge or pixel cap.
pub fn check_dimensions(width: u32, height: u32) -> ToolResult<()> {
    if width > MAX_IMAGE_EDGE || height > MAX_IMAGE_EDGE {
        return Err(ToolError::Invalid(format!(
            "image is {width}x{height}, past the {MAX_IMAGE_EDGE} px edge"
        )));
    }
    if u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS {
        return Err(ToolError::Invalid(format!(
            "image is {width}x{height}, past the 50 megapixel cap"
        )));
    }
    Ok(())
}

/// Decode any format `image` supports, preserving an existing alpha channel.
///
/// Dimensions are probed first, so a hostile header cannot trigger a huge
/// allocation. Oversized files are rejected before decoding.
///
/// # Errors
///
/// Returns [`ToolError::UnsupportedFormat`] when `image` has no decoder for the
/// extension, [`ToolError::Invalid`] when the image is past the size caps, and
/// [`ToolError::Read`] for anything else, including a file that
/// is missing or corrupt.
pub fn read(path: &Path) -> ToolResult<DynamicImage> {
    let (width, height) = image::image_dimensions(path).map_err(|source| match source {
        image::ImageError::Unsupported(_) => ToolError::UnsupportedFormat(path.to_path_buf()),
        other => ToolError::read(path, other),
    })?;
    check_dimensions(width, height)?;
    image::open(path).map_err(|source| match source {
        image::ImageError::Unsupported(_) => ToolError::UnsupportedFormat(path.to_path_buf()),
        other => ToolError::read(path, other),
    })
}

/// Write as PNG. PNG is the only lossless option that carries alpha reliably,
/// so it is the default target for cutouts.
///
/// # Errors
///
/// Returns [`ToolError::Write`] if the parent directory cannot be created or
/// the encoder cannot write the file.
pub fn write_png(image: &DynamicImage, path: &Path) -> ToolResult<()> {
    write_with_format(image, path, image::ImageFormat::Png)
}

/// Write in a named format, inferred from the extension.
///
/// The bytes land in a temp file beside the destination, then rename.
/// Readers never see half a PNG. Symlink and existing destinations are
/// refused before anything is written.
///
/// # Errors
///
/// Returns [`ToolError::Write`] if the parent directory cannot be created or
/// the encoder cannot write the file, and [`ToolError::Invalid`] when the
/// destination is a symlink or already exists.
pub fn write_with_format(
    image: &DynamicImage,
    path: &Path,
    format: image::ImageFormat,
) -> ToolResult<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|source| ToolError::write(parent, source))?;
    }
    guard_destination(path)?;
    let tmp = temp_sibling(path);
    if let Err(error) = image
        .save_with_format(&tmp, format)
        .map_err(|source| ToolError::write(path, source_io(source)))
    {
        let _ = std::fs::remove_file(&tmp);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&tmp, path).map_err(|source| ToolError::write(path, source))
    {
        let _ = std::fs::remove_file(&tmp);
        return Err(error);
    }
    Ok(())
}

/// Monotonic suffix so concurrent exports never share a temp name.
static WRITE_SERIAL: AtomicU64 = AtomicU64::new(0);

/// Temp file in the same directory, so the rename stays atomic.
fn temp_sibling(path: &Path) -> PathBuf {
    let stem = path.file_name().map_or_else(
        || "output".to_owned(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    let serial = WRITE_SERIAL.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!("{stem}.tmp-{}-{serial}", std::process::id()))
}

/// Refuse destinations that would overwrite or escape through a link.
///
/// # Errors
///
/// Returns [`ToolError::Invalid`] when the destination is a symlink or
/// already exists, and [`ToolError::Write`] when its state cannot be read.
fn guard_destination(path: &Path) -> ToolResult<()> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(ToolError::write(path, error)),
        Ok(meta) if meta.file_type().is_symlink() => Err(ToolError::Invalid(format!(
            "`{}` is a symlink; refusing to write",
            path.display()
        ))),
        Ok(_) => Err(ToolError::Invalid(format!(
            "`{}` already exists; refusing to overwrite",
            path.display()
        ))),
    }
}

/// `image::ImageError`'s IO variant wraps an `io::Error`, which is what
/// `ToolError::Write` needs. Anything else becomes a plain invalid-format error.
#[allow(clippy::needless_pass_by_value)]
fn source_io(source: image::ImageError) -> std::io::Error {
    match source {
        image::ImageError::IoError(io) => io,
        other => std::io::Error::other(other.to_string()),
    }
}

/// Apply a single-channel matte to an image, producing RGBA.
///
/// Colour channels pass through untouched and are left unpremultiplied, so
/// compositing the result naively does not darken edges. An image that is
/// already RGBA is read in place; anything else is converted once.
///
/// # Errors
///
/// Returns [`ToolError::Invalid`] when the matte is not the same size as the
/// image. A size mismatch means the two came from different sources.
pub fn apply_matte(source: &DynamicImage, matte: &image::GrayImage) -> ToolResult<RgbaImage> {
    let (width, height) = source.dimensions();
    if matte.dimensions() != (width, height) {
        return Err(ToolError::Invalid(format!(
            "matte is {}x{} but image is {width}x{height}",
            matte.width(),
            matte.height()
        )));
    }

    // The decode path hands us RGBA already, so borrowing it skips a full
    // 96 MB copy on a 24 MP image. Anything else converts once, here.
    let borrowed;
    let source_rgba = if let DynamicImage::ImageRgba8(pixels) = source {
        pixels
    } else {
        borrowed = source.to_rgba8();
        &borrowed
    };
    let mut out = RgbaImage::new(width, height);

    for (x, y, pixel) in out.enumerate_pixels_mut() {
        let [r, g, b, a] = source_rgba.get_pixel(x, y).0;
        *pixel = image::Rgba([r, g, b, a.min(matte.get_pixel(x, y).0[0])]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_matte_writes_alpha_only() {
        let source = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            4,
            1,
            image::Rgba([10, 20, 30, 255]),
        ));
        let matte = image::GrayImage::from_pixel(4, 1, image::Luma([128]));

        let out = apply_matte(&source, &matte).expect("matte matches image size");

        assert_eq!(out.get_pixel(0, 0).0, [10, 20, 30, 128]);
    }

    #[test]
    fn apply_matte_rejects_mismatched_size() {
        let source = DynamicImage::ImageRgba8(image::RgbaImage::new(4, 4));
        let matte = image::GrayImage::new(8, 8);

        assert!(apply_matte(&source, &matte).is_err());
    }

    #[test]
    fn sane_sizes_pass_the_cap() {
        assert!(check_dimensions(6000, 4000).is_ok());
        assert!(check_dimensions(8192, 8192).is_err());
    }

    #[test]
    fn past_the_edge_is_rejected() {
        assert!(check_dimensions(9000, 100).is_err());
        assert!(check_dimensions(100, 9000).is_err());
    }

    #[test]
    fn tiny_headers_cannot_claim_huge_pixels() {
        assert!(check_dimensions(u32::MAX, u32::MAX).is_err());
        assert!(check_dimensions(0, 0).is_ok());
    }

    #[test]
    fn existing_destination_is_not_overwritten() {
        let dir = std::env::temp_dir().join(format!("ember-guard-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir is writable");
        let dest = dir.join("kept.png");
        std::fs::write(&dest, b"original").expect("temp dir is writable");
        let source = DynamicImage::ImageRgba8(image::RgbaImage::new(2, 2));

        assert!(write_png(&source, &dest).is_err());
        assert_eq!(std::fs::read(&dest).expect("kept file stays"), b"original");

        std::fs::remove_file(&dest).expect("temp dir is writable");
    }

    #[test]
    fn symlink_destination_is_refused() {
        let dir = std::env::temp_dir().join(format!("ember-link-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir is writable");
        let target = dir.join("target.png");
        let link = dir.join("link.png");
        std::fs::write(&target, b"target").expect("temp dir is writable");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).expect("temp dir allows links");
        let source = DynamicImage::ImageRgba8(image::RgbaImage::new(2, 2));

        #[cfg(unix)]
        {
            assert!(write_png(&source, &link).is_err());
            assert_eq!(std::fs::read(&target).expect("target untouched"), b"target");
            std::fs::remove_file(&link).expect("temp dir is writable");
        }
        #[cfg(not(unix))]
        {
            let _ = (source, link);
        }

        std::fs::remove_file(&target).expect("temp dir is writable");
    }

    #[test]
    fn successful_write_leaves_no_temp_files() {
        let dir = std::env::temp_dir().join(format!("ember-atomic-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir is writable");
        let dest = dir.join("clean.png");
        let source = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([1, 2, 3, 255]),
        ));

        write_png(&source, &dest).expect("fresh destination writes");

        let leftovers = std::fs::read_dir(&dir)
            .expect("temp dir is readable")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
            .count();
        assert_eq!(leftovers, 0);

        std::fs::remove_file(&dest).expect("temp dir is writable");
    }
}
