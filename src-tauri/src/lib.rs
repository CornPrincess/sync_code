mod commands;
mod error;
mod models;

use std::sync::{Arc, Mutex};

use commands::{
    config::{load_config, save_config},
    git::{checkout_and_pull, checkout_branch, list_branches, refresh_branches},
    sync::{commit_and_push, discard_sync, start_sync},
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(Mutex::new(false))) // sync lock
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            list_branches,
            refresh_branches,
            checkout_branch,
            checkout_and_pull,
            start_sync,
            commit_and_push,
            discard_sync,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
