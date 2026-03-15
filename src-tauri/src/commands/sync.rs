use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tauri::ipc::Channel;
use walkdir::WalkDir;

use crate::commands::git::{emit_log, git_pull, git_push, is_dirty, validate_repo, LogFn};
use crate::error::{AppError, Result};
use crate::models::{AppConfig, SyncEvent};

/// Mirror the contents of `src` into `dst`, excluding the `.git` directory.
fn mirror_files(log: &LogFn, src: &Path, dst: &Path) -> Result<()> {
    emit_log(log, SyncEvent::info(format!("Scanning Repo B: {}", src.display())));

    // Collect all relative file paths from src (excluding .git)
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
        if rel.components().any(|c| c.as_os_str() == ".git") {
            continue;
        }
        src_files.insert(rel.to_path_buf());
    }

    emit_log(
        log,
        SyncEvent::info(format!(
            "Found {} file(s) in Repo B — mirroring into Repo A…",
            src_files.len()
        )),
    );

    // Copy each file from src to dst
    let mut copied = 0u32;
    let mut skipped = 0u32;
    for rel in &src_files {
        let src_file = src.join(rel);
        let dst_file = dst.join(rel);
        if let Some(parent) = dst_file.parent() {
            fs::create_dir_all(parent)?;
        }
        let needs_copy = match (src_file.metadata(), dst_file.metadata()) {
            (Ok(sm), Ok(dm)) => {
                sm.len() != dm.len() || sm.modified().ok() != dm.modified().ok()
            }
            _ => true,
        };
        if needs_copy {
            emit_log(log, SyncEvent::info(format!("  copy  {}", rel.display())));
            fs::copy(&src_file, &dst_file)?;
            copied += 1;
        } else {
            skipped += 1;
        }
    }
    emit_log(
        log,
        SyncEvent::info(format!(
            "Copy complete: {copied} updated, {skipped} unchanged."
        )),
    );

    // Delete stale files in dst not present in src
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
            emit_log(log, SyncEvent::info(format!("  delete {}", rel.display())));
            fs::remove_file(entry.path())?;
            deleted += 1;
        }
    }
    if deleted > 0 {
        emit_log(
            log,
            SyncEvent::info(format!("Removed {deleted} stale file(s) from Repo A.")),
        );
    }

    // Prune empty directories (bottom-up, excluding .git)
    let mut pruned_dirs = 0u32;
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
            continue;
        }
        if rel.components().any(|c| c.as_os_str() == ".git") {
            continue;
        }
        if fs::remove_dir(entry.path()).is_ok() {
            pruned_dirs += 1;
        }
    }
    if pruned_dirs > 0 {
        emit_log(
            log,
            SyncEvent::info(format!("Pruned {pruned_dirs} empty director(ies) from Repo A.")),
        );
    }

    Ok(())
}

#[tauri::command]
pub async fn start_sync(
    _app: AppHandle,
    config: AppConfig,
    sync_lock: tauri::State<'_, Arc<Mutex<bool>>>,
    on_event: Channel<SyncEvent>,
) -> Result<()> {
    // Acquire sync lock — prevent concurrent runs
    {
        let mut locked = sync_lock.lock().map_err(|_| AppError::SyncInProgress)?;
        if *locked {
            return Err(AppError::SyncInProgress);
        }
        *locked = true;
    }

    // Build a LogFn backed by the Channel — guaranteed delivery through IPC
    let log: LogFn = Arc::new(move |event: SyncEvent| {
        let _ = on_event.send(event);
    });

    let result = do_sync(&log, &config).await;

    {
        let mut locked = sync_lock.lock().map_err(|_| AppError::SyncInProgress)?;
        *locked = false;
    }

    result
}

async fn do_sync(log: &LogFn, config: &AppConfig) -> Result<()> {
    let path_a = PathBuf::from(&config.repo_a.local_path);
    let path_b = PathBuf::from(&config.repo_b.local_path);
    let proxy = &config.proxy;

    // ── Step 1: Validate repos ──────────────────────────────────────────────
    emit_log(log, SyncEvent::info("━━ Step 1/5 — Validating repositories ━━"));
    validate_repo(log, &path_a).await?;
    validate_repo(log, &path_b).await?;

    // ── Step 2: Abort if Repo A is dirty ───────────────────────────────────
    emit_log(log, SyncEvent::info("━━ Step 2/5 — Checking Repo A working tree ━━"));
    if is_dirty(&path_a).await? {
        return Err(AppError::Validation(
            "Repo A has uncommitted changes. Please commit or discard them before syncing."
                .to_string(),
        ));
    }
    emit_log(log, SyncEvent::info("  Repo A working tree is clean."));

    // ── Step 3: Pull Repo B ─────────────────────────────────────────────────
    emit_log(
        log,
        SyncEvent::info(format!(
            "━━ Step 3/5 — Pulling Repo B ({}@{}) ━━",
            config.repo_b.local_path, config.repo_b.branch
        )),
    );
    git_pull(log, &config.repo_b, proxy).await?;

    // ── Step 4: Mirror files (CPU/IO-bound — run on blocking thread) ────────
    emit_log(log, SyncEvent::info("━━ Step 4/5 — Mirroring Repo B → Repo A ━━"));
    let log_clone = Arc::clone(log);
    let path_b_clone = path_b.clone();
    let path_a_clone = path_a.clone();
    tauri::async_runtime::spawn_blocking(move || {
        mirror_files(&log_clone, &path_b_clone, &path_a_clone)
    })
    .await
    .map_err(|e| AppError::Git(e.to_string()))??;

    // ── Step 5: Push Repo A ─────────────────────────────────────────────────
    emit_log(
        log,
        SyncEvent::info(format!(
            "━━ Step 5/5 — Pushing Repo A ({}@{}) ━━",
            config.repo_a.local_path, config.repo_a.branch
        )),
    );
    git_push(log, &config.repo_a, proxy).await?;

    emit_log(log, SyncEvent::success("Sync completed successfully!"));
    Ok(())
}
