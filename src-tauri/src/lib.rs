mod commands;
mod images;
mod state;

use ember_core::ModelStore;

use state::{models_root, AppState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let root = models_root();
    let state = AppState::new(ModelStore::new(root));
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::model_status,
            commands::begin_batch,
            commands::cancel_batch,
            commands::drop_matte,
            commands::remove_preview,
            commands::remove_matte,
            commands::refine_preview,
            commands::export_remove,
            commands::merge_preview,
            commands::export_merge,
            commands::copy_remove,
            commands::copy_photo,
            commands::paste_image,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
