use std::path::{Path, PathBuf};

use crate::{ToolError, ToolResult};

/// Build an output path that keeps the input stem, forces a known extension,
/// and never overwrites an existing file.
#[must_use]
pub fn output_path(input: &Path, output_dir: &Path, extension: &str) -> PathBuf {
    let stem = input.file_stem().map_or_else(
        || "output".to_owned(),
        |stem| stem.to_string_lossy().into_owned(),
    );

    let mut candidate = output_dir.join(format!("{stem}.{extension}"));
    let mut suffix = 1;
    while candidate.exists() {
        candidate = output_dir.join(format!("{stem}-{suffix}.{extension}"));
        suffix += 1;
    }
    candidate
}

/// Reject names that would escape the output directory.
///
/// # Errors
///
/// Returns [`ToolError::Invalid`] for an empty name or one containing `..`, a
/// path separator, or a backslash.
pub fn ensure_safe_name(name: &str) -> ToolResult<()> {
    if name.is_empty() || name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err(ToolError::Invalid(format!(
            "`{name}` is not a usable file name"
        )));
    }
    Ok(())
}

/// Join a file name onto the output directory and prove it stayed inside.
///
/// `output_path` builds names from input stems, which cannot hold separators,
/// but the check is cheap and it is the last line before a write.
///
/// # Errors
///
/// Returns [`ToolError::Invalid`] when the name is unsafe or the joined path
/// is not inside `output_dir`.
pub fn confined_path(output_dir: &Path, file_name: &str) -> ToolResult<PathBuf> {
    ensure_safe_name(file_name)?;
    let joined = output_dir.join(file_name);
    if joined.starts_with(output_dir) {
        return Ok(joined);
    }
    Err(ToolError::Invalid(format!(
        "`{file_name}` escapes the output folder"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_path_keeps_stem() {
        let dir = std::env::temp_dir().join(format!("ember-test-{}", std::process::id()));
        let path = output_path(Path::new("/in/photo.jpg"), &dir, "png");

        assert_eq!(
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .as_deref(),
            Some("photo.png")
        );
    }

    #[test]
    fn ensure_safe_name_rejects_traversal() {
        assert!(ensure_safe_name("../escape").is_err());
        assert!(ensure_safe_name("nested/name").is_err());
        assert!(ensure_safe_name("").is_err());
        assert!(ensure_safe_name("photo.png").is_ok());
    }

    #[test]
    fn confined_path_stays_inside_the_output_dir() {
        let dir = Path::new("/out");
        assert_eq!(
            confined_path(dir, "photo.png").expect("safe name"),
            Path::new("/out/photo.png")
        );
        assert!(confined_path(dir, "../escape.png").is_err());
        assert!(confined_path(dir, "").is_err());
    }
}
