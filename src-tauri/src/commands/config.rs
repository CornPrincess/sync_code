use std::fs;
use tauri::Manager;

use crate::{
    error::{AppError, Result},
    models::AppConfig,
};

fn config_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Config(e.to_string()))?;
    Ok(dir.join("config.json"))
}

#[tauri::command]
pub fn load_config(app: tauri::AppHandle) -> Result<AppConfig> {
    let path = config_path(&app)?;
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let data = fs::read_to_string(&path)?;
    let config: AppConfig = serde_json::from_str(&data)?;
    Ok(config)
}

#[tauri::command]
pub fn save_config(app: tauri::AppHandle, config: AppConfig) -> Result<()> {
    let path = config_path(&app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(&config)?;
    fs::write(&path, data)?;
    Ok(())
}
