use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tauri::ipc::Channel;
use walkdir::WalkDir;

use crate::commands::git::{
    discard_changes, emit_log, get_staged_files, get_unpushed_commits, git_add_all, git_commit,
    git_pull, git_push_only, is_dirty, revert_remaining, stage_selected, validate_repo, LogFn,
};
use crate::error::{AppError, Result};
use crate::models::{AppConfig, FileChange, SyncEvent};

// ---------------------------------------------------------------------------
// File mirroring (unchanged from before)
// ---------------------------------------------------------------------------

fn mirror_files(log: &LogFn, src: &Path, dst: &Path) -> Result<()> {
    emit_log(log, SyncEvent::info(format!("Scanning Repo B: {}", src.display())));

    let mut src_files: HashSet<PathBuf> = HashSet::new();
    for entry in WalkDir::new(src)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let rel = entry.path().strip_prefix(src).expect("walkdir under src");
        if rel.components().any(|c| c.as_os_str() == ".git") {
            continue;
        }
        src_files.insert(rel.to_path_buf());
    }

    emit_log(
        log,
        SyncEvent::info(format!("Found {} file(s) in Repo B — mirroring…", src_files.len())),
    );

    let mut copied = 0u32;
    let mut skipped = 0u32;
    for rel in &src_files {
        let src_file = src.join(rel);
        let dst_file = dst.join(rel);
        if let Some(parent) = dst_file.parent() {
            fs::create_dir_all(parent)?;
        }
        let needs_copy = match (src_file.metadata(), dst_file.metadata()) {
            (Ok(sm), Ok(dm)) => sm.len() != dm.len() || sm.modified().ok() != dm.modified().ok(),
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
        SyncEvent::info(format!("Copy complete: {copied} updated, {skipped} unchanged.")),
    );

    let mut deleted = 0u32;
    for entry in WalkDir::new(dst)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let rel = entry.path().strip_prefix(dst).expect("walkdir under dst");
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
        emit_log(log, SyncEvent::info(format!("Removed {deleted} stale file(s).")));
    }

    let mut pruned = 0u32;
    for entry in WalkDir::new(dst)
        .contents_first(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
    {
        let rel = entry.path().strip_prefix(dst).expect("walkdir under dst");
        if rel.as_os_str().is_empty() { continue; }
        if rel.components().any(|c| c.as_os_str() == ".git") { continue; }
        if fs::remove_dir(entry.path()).is_ok() { pruned += 1; }
    }
    if pruned > 0 {
        emit_log(log, SyncEvent::info(format!("Pruned {pruned} empty dir(s).")));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// start_sync — steps 1-6; returns the list of staged changes for review
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn start_sync(
    _app: AppHandle,
    config: AppConfig,
    sync_lock: tauri::State<'_, Arc<Mutex<bool>>>,
    on_event: Channel<SyncEvent>,
) -> Result<Vec<FileChange>> {
    {
        let mut locked = sync_lock.lock().map_err(|_| AppError::SyncInProgress)?;
        if *locked {
            return Err(AppError::SyncInProgress);
        }
        *locked = true;
    }

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

pub async fn do_sync(log: &LogFn, config: &AppConfig) -> Result<Vec<FileChange>> {
    let path_a = PathBuf::from(&config.repo_a.local_path);
    let path_b = PathBuf::from(&config.repo_b.local_path);
    let proxy = &config.proxy;

    // ── Step 1: Validate ────────────────────────────────────────────────────
    emit_log(log, SyncEvent::info("━━ Step 1/6 — Validating repositories ━━"));
    validate_repo(log, &path_a).await?;
    validate_repo(log, &path_b).await?;

    // ── Step 2: Check Repo A is clean ───────────────────────────────────────
    emit_log(log, SyncEvent::info("━━ Step 2/6 — Checking Repo A working tree ━━"));
    if is_dirty(&path_a).await? {
        return Err(AppError::Validation(
            "Repo A has uncommitted changes. Please commit or discard them before syncing."
                .into(),
        ));
    }
    let unpushed = get_unpushed_commits(&path_a, &config.repo_a.branch).await;
    if !unpushed.is_empty() {
        emit_log(
            log,
            SyncEvent::error(format!(
                "✗ Repo A branch '{}' has {} unpushed commit(s) that would be overwritten by reset --hard:",
                config.repo_a.branch,
                unpushed.len()
            )),
        );
        for commit in &unpushed {
            emit_log(log, SyncEvent::error(format!("    {commit}")));
        }
        return Err(AppError::Validation(format!(
            "Repo A branch '{}' has {} unpushed commit(s). Please push or discard them before syncing.",
            config.repo_a.branch,
            unpushed.len()
        )));
    }
    emit_log(log, SyncEvent::info("  Repo A working tree is clean."));

    // ── Step 3: Pull Repo A ─────────────────────────────────────────────────
    emit_log(
        log,
        SyncEvent::info(format!(
            "━━ Step 3/6 — Pulling Repo A ({}@{}) ━━",
            config.repo_a.local_path, config.repo_a.branch
        )),
    );
    if config.repo_a.remote_url.trim().is_empty() {
        emit_log(log, SyncEvent::info("  No remote URL configured for Repo A — skipping pull."));
    } else {
        git_pull(log, &config.repo_a, proxy).await?;
    }

    // ── Step 4: Pull Repo B ─────────────────────────────────────────────────
    emit_log(
        log,
        SyncEvent::info(format!(
            "━━ Step 4/6 — Pulling Repo B ({}@{}) ━━",
            config.repo_b.local_path, config.repo_b.branch
        )),
    );
    git_pull(log, &config.repo_b, proxy).await?;

    // ── Step 5: Mirror B → A ────────────────────────────────────────────────
    emit_log(log, SyncEvent::info("━━ Step 5/6 — Mirroring Repo B → Repo A ━━"));
    let log_clone = Arc::clone(log);
    let path_b_clone = path_b.clone();
    let path_a_clone = path_a.clone();
    tokio::task::spawn_blocking(move || {
        mirror_files(&log_clone, &path_b_clone, &path_a_clone)
    })
    .await
    .map_err(|e| AppError::Git(e.to_string()))??;

    // ── Step 6: Stage all changes, collect file list ─────────────────────────
    emit_log(log, SyncEvent::info("━━ Step 6/6 — Staging changes in Repo A ━━"));
    git_add_all(log, &path_a, Some(&config.repo_a.auth), Some(proxy)).await?;
    let files = get_staged_files(&path_a).await?;

    if files.is_empty() {
        emit_log(log, SyncEvent::info("  Nothing changed — Repo A is already up to date."));
    } else {
        emit_log(
            log,
            SyncEvent::info(format!("  {} file(s) staged and ready to review.", files.len())),
        );
    }

    Ok(files)
}

// ---------------------------------------------------------------------------
// commit_and_push — user-triggered after selecting files in the review panel
// ---------------------------------------------------------------------------

/// `paths_to_stage`: flat list of file paths to commit.
/// For renamed files the caller must include BOTH the new path and the old path
/// so that the deletion of the old name is staged alongside the new file.
#[tauri::command]
pub async fn commit_and_push(
    config: AppConfig,
    commit_message: String,
    paths_to_stage: Vec<String>,
    on_event: Channel<SyncEvent>,
) -> Result<()> {
    let log: LogFn = Arc::new(move |event: SyncEvent| {
        let _ = on_event.send(event);
    });
    run_commit_and_push(&log, &config, commit_message, paths_to_stage).await
}

pub async fn run_commit_and_push(
    log: &LogFn,
    config: &AppConfig,
    commit_message: String,
    paths_to_stage: Vec<String>,
) -> Result<()> {
    let path_a = PathBuf::from(&config.repo_a.local_path);
    let proxy = &config.proxy;

    if paths_to_stage.is_empty() {
        return Err(AppError::Validation("No files selected to commit.".into()));
    }

    // 1. Unstage all, then re-stage only the selected paths
    emit_log(log, SyncEvent::info("━━ Staging selected files ━━"));
    stage_selected(log, &path_a, &paths_to_stage, Some(&config.repo_a.auth), Some(proxy)).await?;

    // 2. Commit (skips automatically if nothing is staged)
    emit_log(log, SyncEvent::info("━━ Committing ━━"));
    let msg = if commit_message.trim().is_empty() {
        format!("sync: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"))
    } else {
        commit_message
    };
    git_commit(log, &path_a, &msg, Some(&config.repo_a.auth), Some(proxy)).await?;

    // 3. Push
    emit_log(log, SyncEvent::info("━━ Pushing Repo A ━━"));
    git_push_only(log, &config.repo_a, proxy).await?;

    // 4. Revert any remaining (unselected) working-tree changes
    emit_log(log, SyncEvent::info("Reverting unselected changes in Repo A…"));
    revert_remaining(&path_a).await?;

    emit_log(log, SyncEvent::success("Push completed successfully!"));
    Ok(())
}

// ---------------------------------------------------------------------------
// discard_sync — revert Repo A to HEAD (undo the mirror)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn discard_sync(config: AppConfig) -> Result<()> {
    let path_a = PathBuf::from(&config.repo_a.local_path);
    discard_changes(&path_a).await
}
