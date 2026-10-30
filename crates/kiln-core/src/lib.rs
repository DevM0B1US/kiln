//! Core contracts for `Kiln` tools.
//!
//! This crate never depends on the GUI. It holds shared errors, image IO,
//! matte and merge maths, model loading, and cancellation. The GUI drives
//! batches through events and owns all window state.
//!
//! The pipeline maths and their load-bearing ordering are carried over from
//! the original `EmberStudio` core, so cutout quality is identical.
#![deny(unsafe_code)]
#![warn(clippy::all, clippy::pedantic, missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// Tests may panic on broken invariants: that is what they are for.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

/// Error types shared by every tool.
pub mod error;
/// Decoding, encoding, and applying a matte to pixels.
pub mod image_io;
/// Running a segmentation model and reading its output.
pub mod inference;
/// Cooperative cancellation for running batches.
pub mod job;
/// Turning raw model output into an alpha channel.
pub mod matte;
/// Template merge maths: fit a photo under a template.
pub mod merge;
/// The model manifest and the load-once session cache.
pub mod models;
/// Safe output names and numbered export paths.
pub mod registry;

pub use error::{ModelId, ToolError, ToolResult};
pub use inference::{INPUT_SIZE, Segmenter};
pub use job::CancelToken;
pub use matte::MatteOptions;
pub use merge::{MergeFit, composite};
pub use models::{ModelHandle, ModelSpec, ModelStore};
pub use registry::confined_path;
