use std::path::{Path, PathBuf};

/// Result alias used throughout the crate.
pub type ToolResult<T> = Result<T, ToolError>;

/// Everything that can go wrong in a tool.
///
/// The variants stay distinct so the UI can tell the user what happened.
/// `ModelMissing` means the bundled file is not where it should be;
/// there is nothing to download, only a path to fix.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    /// No model file on disk. The app reads the file from a `models/`
    /// directory beside itself; this means it was moved or never unpacked.
    #[error("no model `{0}` is installed")]
    ModelMissing(ModelId),

    /// The model file exists but ONNX Runtime rejected it.
    #[error("model `{model}` failed to load: {source}")]
    ModelLoad {
        /// Which model failed.
        model: ModelId,
        /// The underlying runtime error.
        #[source]
        source: ort::Error,
    },

    /// The forward pass itself failed.
    #[error("inference failed: {0}")]
    Inference(String),

    /// The input file could not be decoded.
    #[error("cannot read `{path}`: {source}")]
    Read {
        /// The file we tried to open.
        path: PathBuf,
        /// What the decoder said.
        #[source]
        source: image::ImageError,
    },

    /// The output file could not be written.
    #[error("cannot write `{path}`: {source}")]
    Write {
        /// The file we tried to write.
        path: PathBuf,
        /// What the filesystem said.
        #[source]
        source: std::io::Error,
    },

    /// The extension is not something we can decode.
    #[error("`{0}` is not a supported image format")]
    UnsupportedFormat(PathBuf),

    /// The user cancelled the job.
    #[error("operation was cancelled")]
    Cancelled,

    /// Bad arguments, or a state the tool cannot continue from.
    #[error("{0}")]
    Invalid(String),
}

impl ToolError {
    /// Wrap a decoder failure with the path that caused it.
    pub fn read(path: impl AsRef<Path>, source: image::ImageError) -> Self {
        Self::Read {
            path: path.as_ref().to_path_buf(),
            source,
        }
    }

    /// Wrap a filesystem failure with the path that caused it.
    pub fn write(path: impl AsRef<Path>, source: std::io::Error) -> Self {
        Self::Write {
            path: path.as_ref().to_path_buf(),
            source,
        }
    }

    /// Whether the failure fixes itself. Nothing does: the model is
    /// expected beside the app, so a missing file needs the user to put it back.
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        false
    }
}

/// Identifier for a model in the on-disk model registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModelId(pub &'static str);

impl ModelId {
    /// Wrap a static string as an id.
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for ModelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
