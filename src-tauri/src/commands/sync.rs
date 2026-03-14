use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use walkdir::WalkDir;

use crate::commands::git::{emit_log, git_pull, git_push, is_dirty, validate_repo};
use crate::error::{AppError, Result};
use crate::models::{AppConfig, SyncEvent};

/// Mirror the contents of `src` into `dst`, excluding the `.git` directory.
/// Files present in `dst` (but not in `src`) are deleted to achieve an exact mirror.
fn mirror_files(app: &AppHandle, src: &Path, dst: &Path) -> Result<()> {
    // Collect all relative paths in src (excluding .git)
    let mut src_files: HashSet<PathBuf> = HashSet::new();
    for entry in WalkDir::new(src)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let rel = entry
            .path()
            .strip_prefix(src)
            .expect("walkdir entry is always under src");
        // Skip anything inside .git
        if rel.components().any(|c| c.as_os_str() == ".git") {
            continue;
        }
        src_files.insert(rel.to_path_buf());
    }

    emit_log(
        app,
        SyncEvent::info(format!("Copying {} files from Repo B to Repo A...", src_files.len())),
    );

    // Copy each file from src to dst
    for rel in &src_files {
        let src_file = src.join(rel);
        let dst_file = dst.join(rel);
        if let Some(parent) = dst_file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&src_file, &dst_file)?;
    }

    // Delete files in dst that don't exist in src (excluding .git)
    let mut deleted = 0u32;
    for entry in WalkDir::new(dst)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let rel = entry
            .path()
            .strip_prefix(dst)
            .expect("walkdir entry is always under dst");
        if rel.components().any(|c| c.as_os_str() == ".git") {
            continue;
        }
        if !src_files.contains(rel) {
            fs::remove_file(entry.path())?;
            deleted += 1;
        }
    }

    if deleted > 0 {
        emit_log(app, SyncEvent::info(format!("Removed {deleted} stale file(s) from Repo A.")));
    }

    // Remove empty directories (excluding .git), bottom-up
    for entry in WalkDir::new(dst)
        .contents_first(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
    {
        let rel = entry
            .path()
            .strip_prefix(dst)
            .expect("walkdir entry is always under dst");
        if rel.as_os_str().is_empty() {
            continue; // skip root
        }
        if rel.components().any(|c| c.as_os_str() == ".git") {
            continue;
        }
        // Attempt to remove; ignore errors (non-empty dirs will fail silently)
        let _ = fs::remove_dir(entry.path());
    }

    Ok(())
}

#[tauri::command]
pub async fn start_sync(
    app: AppHandle,
    config: AppConfig,
    sync_lock: tauri::State<'_, Arc<Mutex<bool>>>,
) -> Result<()> {
    // Acquire sync lock — prevent concurrent runs
    {
        let mut locked = sync_lock.lock().map_err(|_| AppError::SyncInProgress)?;
        if *locked {
            return Err(AppError::SyncInProgress);
        }
        *locked = true;
    }

    // Run the actual sync, then release the lock regardless of outcome
    let result = do_sync(&app, &config).await;

    {
        let mut locked = sync_lock.lock().map_err(|_| AppError::SyncInProgress)?;
        *locked = false;
    }

    result
}

async fn do_sync(app: &AppHandle, config: &AppConfig) -> Result<()> {
    let path_a = PathBuf::from(&config.repo_a.local_path);
    let path_b = PathBuf::from(&config.repo_b.local_path);

    // Step 1: Validate repos
    emit_log(app, SyncEvent::info("Validating repositories..."));
    validate_repo(&path_a)?;
    validate_repo(&path_b)?;

    // Step 2: Abort if Repo A is dirty
    if is_dirty(&path_a)? {
        return Err(AppError::Validation(
            "Repo A has uncommitted changes. Please commit or discard them before syncing."
                .to_string(),
        ));
    }

    // Step 3: Pull Repo B
    emit_log(app, SyncEvent::info("Pulling latest from Repo B..."));
    git_pull(app, &path_b, &config.repo_b.branch)?;

    // Step 4: Mirror files (blocking file I/O on a thread pool thread)
    let app_clone = app.clone();
    let path_b_clone = path_b.clone();
    let path_a_clone = path_a.clone();
    tauri::async_runtime::spawn_blocking(move || {
        mirror_files(&app_clone, &path_b_clone, &path_a_clone)
    })
    .await
    .map_err(|e| AppError::Git(e.to_string()))??;

    // Step 5: Push Repo A
    emit_log(app, SyncEvent::info("Pushing Repo A to remote..."));
    git_push(app, &path_a, &config.repo_a.branch)?;

    emit_log(app, SyncEvent::success("Sync completed successfully!"));
    Ok(())
}
