mod commands;
mod error;
mod models;

use std::sync::{Arc, Mutex};

use commands::{
    config::{load_config, save_config},
    sync::start_sync,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(Arc::new(Mutex::new(false))) // sync lock
        .invoke_handler(tauri::generate_handler![load_config, save_config, start_sync])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
