mod commands;
mod types;

use commands::export::export_transcript;
use commands::models::{delete_model, download_model, get_models_dir, list_models};
use commands::transcribe::transcribe;
use commands::translate::translate_transcript;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            transcribe,
            export_transcript,
            get_models_dir,
            list_models,
            download_model,
            delete_model,
            translate_transcript
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
