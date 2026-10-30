//! The only place that talks to ONNX Runtime.

use image::{DynamicImage, GenericImageView, imageops::FilterType};
use ndarray::Array4;
use ort::{inputs, value::TensorRef};

use crate::{ModelHandle, ToolError, ToolResult};

/// Input resolution. RMBG-1.4 is exported with a static 1024x1024 input, so
/// this is the model's shape, not a tuning knob — ORT rejects anything else.
pub const INPUT_SIZE: usize = 1024;

/// Runs a segmentation model and returns a matte at the original image size.
pub struct Segmenter<'a> {
    handle: &'a ModelHandle,
}

impl<'a> Segmenter<'a> {
    #[must_use]
    /// Wrap a loaded model handle.
    pub fn new(handle: &'a ModelHandle) -> Self {
        Self { handle }
    }

    /// Produce a matte the size of `source`.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::Inference`] when the forward pass fails or the
    /// model returns an output shape we do not recognise.
    pub fn matte_for(
        &self,
        source: &DynamicImage,
        input_size: usize,
    ) -> ToolResult<image::GrayImage> {
        let (width, height) = source.dimensions();
        let raw = self.run(source, input_size)?;
        Ok(crate::matte::to_matte(&raw, input_size, width, height))
    }

    /// Raw network output at model resolution, as one flat channel.
    fn run(&self, source: &DynamicImage, input_size: usize) -> ToolResult<Vec<f32>> {
        let tensor = preprocess(source, input_size);

        let mut session = self.handle.session();
        let outputs = session
            .run(inputs![
                TensorRef::from_array_view(&tensor).map_err(|error| to_inference(&error))?
            ])
            .map_err(|error| to_inference(&error))?;

        let (shape, data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|error| to_inference(&error))?;

        check_output_shape(shape)?;
        Ok(data.to_vec())
    }
}

/// Resize to the model's square input and scale to `[-1, 1]` in NCHW order.
///
/// One pass over the resized pixels fills all three planes, so the buffer is
/// touched once instead of copied through an intermediate RGB image.
fn preprocess(source: &DynamicImage, size: usize) -> Array4<f32> {
    // The only size that reaches here is INPUT_SIZE, far below u32::MAX, so the
    // narrowing cannot lose anything.
    #[allow(clippy::cast_possible_truncation)]
    let edge = size as u32;

    let resized = image::imageops::resize(source, edge, edge, FilterType::Triangle);
    let raw = resized.into_raw();

    let plane = size * size;
    let mut data = vec![0.0f32; plane * 3];
    for (index, chunk) in raw.as_chunks::<4>().0.iter().enumerate() {
        data[index] = f32::from(chunk[0]) / 255.0 * 2.0 - 1.0;
        data[plane + index] = f32::from(chunk[1]) / 255.0 * 2.0 - 1.0;
        data[plane * 2 + index] = f32::from(chunk[2]) / 255.0 * 2.0 - 1.0;
    }

    Array4::from_shape_vec((1, 3, size, size), data)
        .unwrap_or_else(|_| Array4::zeros((1, 3, size, size)))
}

/// Expect `[1, 1, H, W]`. Anything else means the model is not what we think.
fn check_output_shape(shape: &[i64]) -> ToolResult<()> {
    let expected = [1, 1];
    let actual = [
        shape.first().copied().unwrap_or(0),
        shape.get(1).copied().unwrap_or(0),
    ];

    if actual != expected {
        return Err(ToolError::Inference(format!(
            "unexpected output shape {shape:?}"
        )));
    }
    Ok(())
}

fn to_inference(source: &ort::Error) -> ToolError {
    ToolError::Inference(source.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preprocess_produces_normalised_nchw() {
        let source = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            8,
            8,
            image::Rgb([255, 255, 255]),
        ));
        let tensor = preprocess(&source, 4);

        assert_eq!(tensor.shape(), &[1, 3, 4, 4]);
        assert!((tensor[[0, 0, 0, 0]] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn preprocess_maps_black_and_white_extremes() {
        let source =
            DynamicImage::ImageRgb8(image::RgbImage::from_pixel(4, 4, image::Rgb([0, 255, 0])));
        let tensor = preprocess(&source, 2);

        assert!((tensor[[0, 0, 0, 0]] - (-1.0)).abs() < f32::EPSILON);
        assert!((tensor[[0, 1, 0, 0]] - 1.0).abs() < f32::EPSILON);
        assert!((tensor[[0, 2, 0, 0]] - (-1.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn check_output_shape_rejects_wrong_rank() {
        assert!(check_output_shape(&[1, 1, 8, 8]).is_ok());
        assert!(check_output_shape(&[1, 3, 8, 8]).is_err());
        assert!(check_output_shape(&[8, 8]).is_err());
    }
}

/// End-to-end inference through a real ONNX graph.
///
/// The RMBG-1.4 weights are licence-gated and never committed, so the fixture
/// (`tests/isnet_like.onnx`, 64x64, IS-Net's input/output contract)
/// stands in for the network. Its output is deterministic: channel 0 of the
/// preprocessed tensor scaled back outside `[-1, 1]`, which exercises the
/// min-max matte normalisation exactly the way real RMBG output does.
#[cfg(test)]
mod fixture_tests {
    use std::path::Path;

    use super::*;
    use crate::matte::MatteOptions;
    use crate::{ModelId, image_io::apply_matte, models::ModelSpec, registry::output_path};

    fn fixture_spec() -> ModelSpec {
        ModelSpec {
            id: ModelId::new("isnet-like-test"),
            display_name: "IS-Net-like fixture",
            file_name: "isnet_like.onnx",
            // Computed from the generated fixture; guards against accidental
            // edits changing what this test proves.
            sha256: "cfae3dda1f3f78289ed45f13c6cda78dae7b8cca28e0208563df7d7e54e28769",
            input_size: 64,
            license: "test-only",
        }
    }

    #[test]
    fn end_to_end_matte_through_onnx_fixture() {
        let store = crate::models::ModelStore::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests"));
        let handle = store.get_spec(&fixture_spec()).expect("fixture loads");
        assert_eq!(handle.input_size(), 64);

        // A horizontal gradient subject on a flat background, 64x64 so the
        // model-size resize is an identity.
        let source = DynamicImage::ImageRgb8(image::RgbImage::from_fn(64, 64, |x, y| {
            if y < 32 {
                image::Rgb([u8::try_from(x * 4).unwrap_or(255), 0, 0])
            } else {
                image::Rgb([0, 0, 0])
            }
        }));

        let matte = Segmenter::new(&handle)
            .matte_for(&source, 64)
            .expect("fixture inference succeeds");
        assert_eq!(matte.dimensions(), (64, 64));

        // The fixture outputs channel 0 * 2 - 1: bottom half is flat -1, so
        // after min-max normalisation the top row must be the brightest and
        // the bottom row the darkest.
        let top = matte.get_pixel(63, 0).0[0];
        let bottom = matte.get_pixel(63, 63).0[0];
        assert_eq!(top, 255, "brightest source column tops the matte");
        assert_eq!(bottom, 0, "flat background normalises to zero");
        // Left edge of the gradient is near-black (0*2-1 = -1 = the floor),
        // same as the flat region.
        assert_eq!(matte.get_pixel(0, 0).0[0], 0);

        // Refine + apply must produce an RGBA image of the same size.
        let refined = crate::matte::refine(
            &matte,
            MatteOptions {
                threshold: Some(128),
                ..MatteOptions::default()
            },
        );
        let cutout = apply_matte(&source, &refined).expect("sizes match");
        assert_eq!(cutout.dimensions(), (64, 64));
        assert_eq!(cutout.get_pixel(63, 0).0[3], 255);
        assert_eq!(cutout.get_pixel(0, 63).0[3], 0);
    }

    #[test]
    fn fixture_writes_a_numbered_png_export() {
        let store = crate::models::ModelStore::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests"));
        let handle = store.get_spec(&fixture_spec()).expect("fixture loads");

        let source = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            64,
            64,
            image::Rgb([128, 128, 128]),
        ));
        let matte = Segmenter::new(&handle)
            .matte_for(&source, 64)
            .expect("inference works");
        let cutout = apply_matte(&source, &matte).expect("sizes match");

        let dir = std::env::temp_dir().join(format!("kiln-e2e-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir is writable");
        let dest = output_path(Path::new("/in/photo.jpg"), &dir, "png");
        crate::image_io::write_png(&DynamicImage::ImageRgba8(cutout), &dest)
            .expect("png export works");

        let decoded = image::open(&dest).expect("round trip decodes");
        assert_eq!(decoded.dimensions(), (64, 64));

        std::fs::remove_file(&dest).expect("temp dir is writable");
        std::fs::remove_dir(&dir).expect("temp dir is writable");
    }
}
