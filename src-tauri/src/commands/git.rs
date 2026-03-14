use std::path::Path;
use std::process::{Command, Stdio};
use tauri::{AppHandle, Emitter};

use crate::error::{AppError, Result};
use crate::models::SyncEvent;

/// Emit a log event to the frontend.
pub fn emit_log(app: &AppHandle, event: SyncEvent) {
    let _ = app.emit("sync://log", event);
}

/// Run a git command in `dir`, stream each output line as a log event.
/// Returns combined stdout+stderr as a string on success, AppError::Git on failure.
fn run_git(app: &AppHandle, dir: &Path, args: &[&str]) -> Result<String> {
    emit_log(app, SyncEvent::info(format!("$ git {}", args.join(" "))));

    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    for line in stdout.lines().chain(stderr.lines()) {
        if !line.trim().is_empty() {
            emit_log(app, SyncEvent::info(format!("  {line}")));
        }
    }

    if output.status.success() {
        Ok(stdout)
    } else {
        let msg = if stderr.trim().is_empty() { stdout } else { stderr };
        Err(AppError::Git(msg.trim().to_string()))
    }
}

/// Verify that `path` exists and is a git repository.
pub fn validate_repo(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(AppError::Validation(format!(
            "Path does not exist: {}",
            path.display()
        )));
    }
    if !path.join(".git").exists() {
        return Err(AppError::Validation(format!(
            "Not a git repository: {}",
            path.display()
        )));
    }
    Ok(())
}

/// Pull latest from remote for the given branch.
/// Uses fetch + reset --hard to ensure a clean, deterministic state.
pub fn git_pull(app: &AppHandle, path: &Path, branch: &str) -> Result<()> {
    run_git(app, path, &["fetch", "origin"])?;
    run_git(
        app,
        path,
        &["reset", "--hard", &format!("origin/{branch}")],
    )?;
    Ok(())
}

/// Stage all changes, commit, and push to the remote branch.
/// Skips commit if there is nothing to commit.
pub fn git_push(app: &AppHandle, path: &Path, branch: &str) -> Result<()> {
    run_git(app, path, &["add", "-A"])?;

    // Check if there is anything to commit
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(path)
        .output()?;
    let is_dirty = !status_output.stdout.is_empty();

    if is_dirty {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let msg = format!("sync: {timestamp}");
        run_git(app, path, &["commit", "-m", &msg])?;
    } else {
        emit_log(app, SyncEvent::info("Nothing to commit, skipping commit step."));
    }

    run_git(app, path, &["push", "origin", branch])?;
    Ok(())
}

/// Check if the repo has uncommitted changes.
/// Returns Ok(true) if dirty, Ok(false) if clean.
pub fn is_dirty(path: &Path) -> Result<bool> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(path)
        .output()?;
    Ok(!output.stdout.is_empty())
}
