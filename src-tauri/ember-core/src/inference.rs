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
    for (index, chunk) in raw.chunks_exact(4).enumerate() {
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
    fn check_output_shape_rejects_wrong_rank() {
        assert!(check_output_shape(&[1, 1, 8, 8]).is_ok());
        assert!(check_output_shape(&[1, 3, 8, 8]).is_err());
        assert!(check_output_shape(&[8, 8]).is_err());
    }
}
