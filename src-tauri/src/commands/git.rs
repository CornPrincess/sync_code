use std::path::Path;
use tauri::{AppHandle, Emitter};
use tokio::process::Command;

use crate::error::{AppError, Result};
use crate::models::{AuthConfig, ProxyConfig, RepoConfig, SyncEvent};

/// Emit a log event to the frontend.
pub fn emit_log(app: &AppHandle, event: SyncEvent) {
    let _ = app.emit("sync://log", event);
}

// ---------------------------------------------------------------------------
// Credential / proxy helpers
// ---------------------------------------------------------------------------

/// Build an authenticated HTTP(S) URL by embedding user:pass.
/// Returns None if auth type is not "userpass" or URL is not HTTP(S).
fn auth_url(remote_url: &str, auth: &AuthConfig) -> Option<String> {
    if auth.auth_type != "userpass" || auth.username.is_empty() {
        return None;
    }
    let scheme_end = remote_url.find("://")? + 3;
    let scheme = &remote_url[..scheme_end];
    let rest = &remote_url[scheme_end..];
    let u = percent_encode(&auth.username);
    let p = percent_encode(&auth.password);
    Some(format!("{scheme}{u}:{p}@{rest}"))
}

/// Minimal percent-encoding for URL credentials (RFC 3986 unreserved chars pass through).
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Build the GIT_SSH_COMMAND value when an SSH key is configured.
fn ssh_command(auth: &AuthConfig) -> Option<String> {
    if auth.auth_type != "ssh" || auth.ssh_key_path.is_empty() {
        return None;
    }
    Some(format!(
        "ssh -i \"{}\" -o StrictHostKeyChecking=accept-new -o BatchMode=yes",
        auth.ssh_key_path
    ))
}

// ---------------------------------------------------------------------------
// Core git runner (fully async — no blocking of Tokio worker threads)
// ---------------------------------------------------------------------------

/// Run a git command in `dir` asynchronously.
///
/// * `display` – if `Some`, this string is logged instead of the raw args
///   (use it to hide embedded credentials).
/// * Each non-empty output line is emitted as an info log event.
async fn run_git(
    app: &AppHandle,
    dir: &Path,
    args: &[&str],
    display: Option<&str>,
    auth: Option<&AuthConfig>,
    proxy: Option<&ProxyConfig>,
) -> Result<String> {
    let joined = args.join(" ");
    let logged = display.unwrap_or(&joined);
    emit_log(app, SyncEvent::info(format!("$ git {logged}")));

    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(dir);

    // Disable terminal prompts only when we are actually supplying credentials.
    // The parent process (npm / tauri-cli) may have already set
    // GIT_TERMINAL_PROMPT=0, so explicitly remove it when we want the system
    // credential helper (osxkeychain, git-credential-manager, …) to work.
    if let Some(auth) = auth {
        let disabling_prompts = match auth.auth_type.as_str() {
            "userpass" => !auth.username.is_empty(),
            "ssh" => !auth.ssh_key_path.is_empty(),
            _ => false,
        };
        if disabling_prompts {
            cmd.env("GIT_TERMINAL_PROMPT", "0");
        } else {
            cmd.env_remove("GIT_TERMINAL_PROMPT");
        }
        if let Some(ssh_cmd) = ssh_command(auth) {
            cmd.env("GIT_SSH_COMMAND", ssh_cmd);
        }
    } else {
        cmd.env_remove("GIT_TERMINAL_PROMPT");
    }

    // Proxy
    if let Some(proxy) = proxy {
        if proxy.enabled {
            if !proxy.http_proxy.is_empty() {
                cmd.env("http_proxy", &proxy.http_proxy);
                cmd.env("HTTP_PROXY", &proxy.http_proxy);
            }
            if !proxy.https_proxy.is_empty() {
                cmd.env("https_proxy", &proxy.https_proxy);
                cmd.env("HTTPS_PROXY", &proxy.https_proxy);
            }
            if !proxy.no_proxy.is_empty() {
                cmd.env("no_proxy", &proxy.no_proxy);
                cmd.env("NO_PROXY", &proxy.no_proxy);
            }
        }
    }

    let output = cmd.output().await?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    for line in stdout.lines().chain(stderr.lines()) {
        let t = line.trim();
        if !t.is_empty() {
            emit_log(app, SyncEvent::info(format!("  {t}")));
        }
    }

    if output.status.success() {
        Ok(stdout)
    } else {
        let msg = if stderr.trim().is_empty() { stdout } else { stderr };
        Err(AppError::Git(msg.trim().to_string()))
    }
}

// ---------------------------------------------------------------------------
// Public API (all async)
// ---------------------------------------------------------------------------

/// Verify that `path` exists and contains a `.git` directory.
pub async fn validate_repo(app: &AppHandle, path: &Path) -> Result<()> {
    emit_log(app, SyncEvent::info(format!("Checking {}", path.display())));
    if !path.exists() {
        return Err(AppError::Validation(format!(
            "Path does not exist: {}",
            path.display()
        )));
    }
    if !path.join(".git").exists() {
        return Err(AppError::Validation(format!(
            "Not a git repository (no .git found): {}",
            path.display()
        )));
    }
    emit_log(app, SyncEvent::info(format!("  OK — git repo found at {}", path.display())));
    Ok(())
}

/// Fetch + hard-reset to `origin/<branch>`.
///
/// When HTTP userpass auth is configured the remote URL is temporarily
/// overridden via `-c remote.origin.url=<auth_url>` so credentials are
/// never written to disk.
pub async fn git_pull(app: &AppHandle, repo: &RepoConfig, proxy: &ProxyConfig) -> Result<()> {
    let path = Path::new(&repo.local_path);
    let branch = &repo.branch;

    emit_log(
        app,
        SyncEvent::info(format!(
            "Fetching from remote (branch: {branch}, auth: {})…",
            repo.auth.auth_type
        )),
    );

    if let Some(aurl) = auth_url(&repo.remote_url, &repo.auth) {
        let url_cfg = format!("remote.origin.url={aurl}");
        run_git(
            app,
            path,
            &["-c", &url_cfg, "fetch", "--progress", "origin"],
            Some("fetch --progress origin  [credentials injected]"),
            Some(&repo.auth),
            Some(proxy),
        )
        .await?;
    } else {
        run_git(
            app,
            path,
            &["fetch", "--progress", "origin"],
            None,
            Some(&repo.auth),
            Some(proxy),
        )
        .await?;
    }

    let target = format!("origin/{branch}");
    run_git(
        app,
        path,
        &["reset", "--hard", &target],
        None,
        Some(&repo.auth),
        Some(proxy),
    )
    .await?;
    emit_log(app, SyncEvent::info(format!("  Repo reset to {target}")));
    Ok(())
}

/// Stage everything, commit (if dirty), and push to `origin/<branch>`.
pub async fn git_push(app: &AppHandle, repo: &RepoConfig, proxy: &ProxyConfig) -> Result<()> {
    let path = Path::new(&repo.local_path);
    let branch = &repo.branch;

    run_git(app, path, &["add", "-A"], None, Some(&repo.auth), Some(proxy)).await?;

    // Check if there is anything to commit (async)
    let status_out = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(path)
        .output()
        .await?;
    let dirty = !status_out.stdout.is_empty();

    if dirty {
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let msg = format!("sync: {ts}");
        emit_log(app, SyncEvent::info(format!("Committing with message: \"{msg}\"")));
        run_git(
            app,
            path,
            &["commit", "-m", &msg],
            None,
            Some(&repo.auth),
            Some(proxy),
        )
        .await?;
    } else {
        emit_log(app, SyncEvent::info("Working tree is clean — nothing to commit."));
    }

    emit_log(
        app,
        SyncEvent::info(format!(
            "Pushing to remote (branch: {branch}, auth: {})…",
            repo.auth.auth_type
        )),
    );

    if let Some(aurl) = auth_url(&repo.remote_url, &repo.auth) {
        let url_cfg = format!("remote.origin.url={aurl}");
        run_git(
            app,
            path,
            &["-c", &url_cfg, "push", "--progress", "origin", branch],
            Some(&format!("push --progress origin {branch}  [credentials injected]")),
            Some(&repo.auth),
            Some(proxy),
        )
        .await?;
    } else {
        run_git(
            app,
            path,
            &["push", "--progress", "origin", branch],
            None,
            Some(&repo.auth),
            Some(proxy),
        )
        .await?;
    }
    Ok(())
}

/// Return true if the repo has uncommitted changes.
pub async fn is_dirty(path: &Path) -> Result<bool> {
    let out = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(path)
        .output()
        .await?;
    Ok(!out.stdout.is_empty())
}
