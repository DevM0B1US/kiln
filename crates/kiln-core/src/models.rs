use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};

use ort::session::Session;
use sha2::{Digest, Sha256};

use crate::{ModelId, ToolError, ToolResult};

/// Static metadata about a bundled model.
#[derive(Debug, Clone, Copy)]
pub struct ModelSpec {
    /// Key used to request this model.
    pub id: ModelId,
    /// Name shown in the UI.
    pub display_name: &'static str,
    /// File name inside the model directory.
    pub file_name: &'static str,
    /// Expected SHA-256 of the file. Checked before every load.
    pub sha256: &'static str,
    /// Square input resolution the model expects.
    pub input_size: usize,
    /// Licence string, shown so constraints are visible.
    pub license: &'static str,
}

/// The segmentation model `Kiln` runs on.
pub const RMBG_14: ModelSpec = ModelSpec {
    id: ModelId::new("rmbg-1.4"),
    display_name: "RMBG-1.4",
    file_name: "rmbg-1.4.onnx",
    sha256: "a6648479275dfd0ede0f3a8abc20aa5c437b394681b05e5af6d268250aaf40f3",
    input_size: 1024,
    license: "non-commercial (bria-rmbg-1.4)",
};

/// Every model this build knows about.
#[must_use]
pub fn known_models() -> &'static [ModelSpec] {
    &[RMBG_14]
}

/// Look up a spec by id.
#[must_use]
pub fn spec(id: ModelId) -> Option<&'static ModelSpec> {
    known_models().iter().find(|spec| spec.id == id)
}

/// A loaded network, shared by every job that needs it.
///
/// ONNX Runtime's `run` needs `&mut self`, so the session sits behind a mutex.
/// One lock per inference, released before post-processing, which is the part
/// worth parallelising anyway.
pub struct ModelHandle {
    session: Mutex<Session>,
    input_size: usize,
}

impl ModelHandle {
    /// Wrap an already-built session. A testing seam and an escape hatch for
    /// callers that manage their own model files.
    #[must_use]
    pub fn from_session(session: Session, input_size: usize) -> Self {
        Self {
            session: Mutex::new(session),
            input_size,
        }
    }

    /// Lock the session for one inference. Released before post-processing.
    pub fn session(&self) -> MutexGuard<'_, Session> {
        self.session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[must_use]
    /// Square resolution this model expects.
    pub fn input_size(&self) -> usize {
        self.input_size
    }
}

/// Process-wide model cache. Loading costs seconds, so it happens at most once.
pub struct ModelStore {
    root: PathBuf,
    loaded: Mutex<HashMap<ModelId, Arc<ModelHandle>>>,
}

impl ModelStore {
    #[must_use]
    /// Cache backed by a model directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            loaded: Mutex::new(HashMap::new()),
        }
    }

    /// Where the bundled model lives.
    ///
    /// `KILN_MODELS_DIR` wins when set. Otherwise the first candidate holding
    /// the model file wins: beside the executable, one and two levels up for
    /// `target/{debug,release}` layouts, then the working directory. Matching
    /// on the file rather than the directory matters — `cargo run` leaves an
    /// empty `target/debug/models/` behind, and that would otherwise shadow
    /// the real one. Shipping the binary and a `models/` folder together is
    /// the supported layout.
    #[must_use]
    pub fn default_root() -> PathBuf {
        if let Some(from_env) = std::env::var_os("KILN_MODELS_DIR") {
            return PathBuf::from(from_env);
        }

        // A path with no parent only happens for a bare filename, and on Linux
        // the executable path is absolute anyway. Fall through to `models/`.
        let beside = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."));

        let candidates = [
            beside.join("models"),
            beside.join("../models"),
            beside.join("../../models"),
            PathBuf::from("models"),
        ];

        let holds_model = |dir: &Path| {
            known_models()
                .iter()
                .any(|spec| dir.join(spec.file_name).is_file())
        };

        candidates
            .iter()
            .find(|candidate| holds_model(candidate))
            .map_or_else(|| candidates[0].clone(), PathBuf::clone)
    }

    #[must_use]
    /// Directory holding the model files.
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    /// Where this model lives on disk.
    pub fn path_for(&self, spec: &ModelSpec) -> PathBuf {
        self.root.join(spec.file_name)
    }

    #[must_use]
    /// Whether the file exists, without loading it.
    pub fn is_installed(&self, id: ModelId) -> bool {
        spec(id).is_some_and(|spec| self.path_for(spec).exists())
    }

    /// Fetch the session, loading it on first use.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::ModelMissing`] when the model is not in the
    /// registry or not on disk, [`ToolError::Invalid`] when the file on disk
    /// does not match the pinned hash, and [`ToolError::ModelLoad`] when ONNX
    /// Runtime rejects the file.
    pub fn get(&self, id: ModelId) -> ToolResult<Arc<ModelHandle>> {
        let cache = self.locked();
        if let Some(handle) = cache.get(&id) {
            return Ok(Arc::clone(handle));
        }
        drop(cache);

        let model_spec = spec(id).ok_or(ToolError::ModelMissing(id))?;
        self.get_spec(model_spec)
    }

    /// Fetch the session for an explicit spec, loading it on first use.
    ///
    /// Lets tests and advanced callers run a spec that is not in
    /// [`known_models`] — the hash guard still applies.
    ///
    /// # Errors
    ///
    /// Same contract as [`ModelStore::get`].
    pub fn get_spec(&self, model_spec: &ModelSpec) -> ToolResult<Arc<ModelHandle>> {
        let cache = self.locked();
        if let Some(handle) = cache.get(&model_spec.id) {
            return Ok(Arc::clone(handle));
        }
        drop(cache);

        let path = self.path_for(model_spec);
        if !path.exists() {
            return Err(ToolError::ModelMissing(model_spec.id));
        }
        verify(&path, model_spec)?;

        let session = Session::builder()
            .and_then(|mut builder| builder.commit_from_file(&path))
            .map_err(|source| ToolError::ModelLoad {
                model: model_spec.id,
                source,
            })?;

        let handle = Arc::new(ModelHandle {
            session: Mutex::new(session),
            input_size: model_spec.input_size,
        });
        let mut cache = self.locked();
        // Another thread may have won the race. Either handle works.
        Ok(Arc::clone(cache.entry(model_spec.id).or_insert(handle)))
    }

    fn locked(&self) -> MutexGuard<'_, HashMap<ModelId, Arc<ModelHandle>>> {
        self.loaded
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Hash chunk. Small enough to stream a 44 MB file without noticing.
const CHUNK: usize = 1 << 20;

/// Reject a bundled file that is not the bytes we expect.
///
/// A corrupt model does not fail loudly in ONNX Runtime; it produces garbage
/// mattes. The hash check turns that into an error instead.
fn verify(path: &Path, model_spec: &ModelSpec) -> ToolResult<()> {
    let digest = file_digest(path)?;
    if digest.eq_ignore_ascii_case(model_spec.sha256) {
        return Ok(());
    }
    Err(ToolError::Invalid(format!(
        "model `{}` is corrupt: expected sha256 {}, got {digest}",
        model_spec.id, model_spec.sha256
    )))
}

/// Hash a file already on disk.
///
/// # Errors
///
/// Returns [`ToolError::Write`] when the file cannot be opened or read.
pub fn file_digest(path: &Path) -> ToolResult<String> {
    let mut file = std::fs::File::open(path).map_err(|source| ToolError::write(path, source))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; CHUNK];

    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| ToolError::write(path, source))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(digest: &[u8]) -> String {
    use std::fmt::Write as _;
    digest
        .iter()
        .fold(String::with_capacity(digest.len() * 2), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_lookup_finds_the_shipped_model() {
        assert_eq!(
            spec(RMBG_14.id).map(|found| found.file_name),
            Some("rmbg-1.4.onnx")
        );
        assert!(spec(ModelId::new("not-a-model")).is_none());
    }

    #[test]
    fn installed_checks_the_file_not_just_the_directory() {
        let empty = ModelStore::new(std::env::temp_dir().join("kiln-no-such-models"));
        assert!(!empty.is_installed(RMBG_14.id));
    }

    #[test]
    fn missing_model_is_not_recoverable() {
        let empty = ModelStore::new(std::env::temp_dir().join("kiln-no-such-models"));

        assert!(matches!(empty.get(RMBG_14.id), Err(error) if !error.is_recoverable()));
    }

    #[test]
    fn unknown_model_is_not_installed() {
        let store = ModelStore::new(std::env::temp_dir());
        assert!(!store.is_installed(ModelId::new("not-a-model")));
    }

    #[test]
    fn digest_is_stable_hex_and_content_sensitive() {
        let dir = std::env::temp_dir().join(format!("kiln-digest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir is writable");
        let first = dir.join("a.bin");
        let second = dir.join("b.bin");
        std::fs::write(&first, b"kiln").expect("temp dir is writable");
        std::fs::write(&second, b"kiln!").expect("temp dir is writable");

        let digest = file_digest(&first).expect("just wrote it");
        let again = file_digest(&first).expect("just wrote it");
        let other = file_digest(&second).expect("just wrote it");

        assert_eq!(digest.len(), 64);
        assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(digest, again);
        assert_ne!(digest, other);

        std::fs::remove_file(&first).expect("temp dir is writable");
        std::fs::remove_file(&second).expect("temp dir is writable");
    }

    #[test]
    fn corrupt_file_fails_verification() {
        let dir = std::env::temp_dir().join(format!("kiln-verify-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir is writable");
        let path = dir.join(RMBG_14.file_name);
        std::fs::write(&path, b"not the model").expect("temp dir is writable");

        assert!(verify(&path, &RMBG_14).is_err());

        std::fs::remove_file(&path).expect("temp dir is writable");
    }
}
