//! Kiln library entry: Tauri builder, plugins, managed state, commands.
//!
//! The frontend owns queues and window state; the backend owns pixels and
//! the model. The bridge between them is exactly the command set below.

pub mod cli;
pub mod commands;
pub mod state;

use std::sync::Arc;

use kiln_core::ModelStore;
use state::AppState;

/// Model directory resolution is the core's job; the window just shows the
/// calm banner when the weights are not there.
fn models_root() -> ModelStore {
    ModelStore::new(ModelStore::default_root())
}

/// Build and run the window.
///
/// A failure to start the window is a broken install, not a user error: the
/// process exits non-zero with the reason on stderr.
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(AppState::new(models_root())))
        .invoke_handler(tauri::generate_handler![
            commands::model_status,
            commands::preview,
            commands::refine,
            commands::merge_preview,
            commands::template_preview,
            commands::export_remove,
            commands::export_merge,
            commands::copy_cutout,
            commands::copy_photo,
            commands::paste_image,
            commands::cancel_batch,
        ])
        .run(tauri::generate_context!());

    if let Err(error) = result {
        eprintln!("kiln: {error}");
        std::process::exit(1);
    }
}
