use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use ember_core::{CancelToken, ModelStore};

pub struct MatteEntry {
    pub matte: image::GrayImage,
    pub modified: Option<SystemTime>,
}

pub struct AppState {
    pub models: Arc<ModelStore>,
    pub mattes: Mutex<HashMap<String, MatteEntry>>,
    pub cancel: Mutex<CancelToken>,
    pub clipboard: Mutex<Option<arboard::Clipboard>>,
}

impl AppState {
    pub fn new(models: ModelStore) -> Self {
        Self {
            models: Arc::new(models),
            mattes: Mutex::new(HashMap::new()),
            cancel: Mutex::new(CancelToken::new()),
            clipboard: Mutex::new(None),
        }
    }

    pub fn fresh_cancel(&self) {
        if let Ok(mut guard) = self.cancel.lock() {
            *guard = CancelToken::new();
        }
    }

    pub fn cancelled(&self) -> bool {
        self.cancel
            .lock()
            .map(|flag| flag.is_cancelled())
            .unwrap_or(false)
    }

    pub fn stop(&self) {
        if let Ok(guard) = self.cancel.lock() {
            guard.cancel();
        }
    }
}

pub fn file_modified(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
}

pub fn models_root() -> PathBuf {
    if let Ok(dir) = std::env::var("EMBER_MODELS_DIR") {
        let root = PathBuf::from(dir);
        if root.is_dir() {
            return root;
        }
    }
    let found = ModelStore::default_root();
    if ModelStore::new(&found).is_installed(ember_core::models::RMBG_14.id) {
        return found;
    }
    let mut anchor = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for _ in 0..5 {
        let candidate = anchor.join("models");
        if candidate
            .join(ember_core::models::RMBG_14.file_name)
            .is_file()
        {
            return candidate;
        }
        if !anchor.pop() {
            break;
        }
    }
    found
}
