use std::path::Path;
use std::sync::Arc;
use tokio::process::Command;

use crate::error::{AppError, Result};
use crate::models::{AuthConfig, FileChange, ProxyConfig, RepoConfig, SyncEvent};

/// Shared logger type: cheaply clone-able and safe to send across threads.
pub type LogFn = Arc<dyn Fn(SyncEvent) + Send + Sync>;

/// Call the logger with a sync event.
pub fn emit_log(log: &LogFn, event: SyncEvent) {
    log(event);
}

// ---------------------------------------------------------------------------
// Credential / proxy helpers
// ---------------------------------------------------------------------------

/// Build an authenticated remote URL for HTTPS-based git operations.
/// Each platform has a different token URL format:
///   GitHub  — oauth2:<token>@github.com/...   (or x-access-token, both work)
///   GitLab  — oauth2:<token>@gitlab.com/...   (documented by GitLab)
///   Codeup  — <username>:<token>@codeup.aliyun.com/...  (basic HTTP auth)
fn auth_url(remote_url: &str, auth: &AuthConfig, platform: &str) -> Option<String> {
    let scheme_end = remote_url.find("://")? + 3;
    let scheme = &remote_url[..scheme_end];
    let rest = &remote_url[scheme_end..];
    match auth.auth_type.as_str() {
        "userpass" if !auth.username.is_empty() => {
            let u = percent_encode(&auth.username);
            let p = percent_encode(&auth.password);
            Some(format!("{scheme}{u}:{p}@{rest}"))
        }
        "token" if !auth.token.is_empty() => {
            let t = percent_encode(&auth.token);
            match platform {
                "codeup" => {
                    // Codeup requires real username + token as password
                    let u = if auth.username.is_empty() { "git".to_string() } else { percent_encode(&auth.username) };
                    Some(format!("{scheme}{u}:{t}@{rest}"))
                }
                _ => {
                    // GitHub and GitLab both accept "oauth2" as the username
                    Some(format!("{scheme}oauth2:{t}@{rest}"))
                }
            }
        }
        _ => None,
    }
}

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
// Core git runner
// ---------------------------------------------------------------------------

async fn run_git(
    log: &LogFn,
    dir: &Path,
    args: &[&str],
    display: Option<&str>,
    auth: Option<&AuthConfig>,
    proxy: Option<&ProxyConfig>,
) -> Result<String> {
    let joined = args.join(" ");
    let logged = display.unwrap_or(&joined);
    emit_log(log, SyncEvent::info(format!("$ git {logged}")));

    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(dir);

    if let Some(auth) = auth {
        let disabling_prompts = match auth.auth_type.as_str() {
            "userpass" => !auth.username.is_empty(),
            "token" => !auth.token.is_empty(),
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
            emit_log(log, SyncEvent::info(format!("  {t}")));
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
// Public API
// ---------------------------------------------------------------------------

/// Verify that `path` exists and contains a `.git` directory.
pub async fn validate_repo(log: &LogFn, path: &Path) -> Result<()> {
    emit_log(log, SyncEvent::info(format!("Checking {}", path.display())));
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
    emit_log(log, SyncEvent::info(format!("  OK — {}", path.display())));
    Ok(())
}

/// Return commits on the current HEAD that are not yet pushed to `origin/<branch>`.
/// Returns an empty Vec when the remote ref doesn't exist or git fails.
async fn get_unpushed_commits(path: &Path, branch: &str) -> Vec<String> {
    let range = format!("origin/{branch}..HEAD");
    let Ok(out) = Command::new("git")
        .args(["log", &range, "--oneline"])
        .current_dir(path)
        .output()
        .await
    else {
        return vec![];
    };
    if !out.status.success() {
        return vec![];
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.to_string())
        .collect()
}

/// Fetch + hard-reset to `origin/<branch>`.
pub async fn git_pull(log: &LogFn, repo: &RepoConfig, proxy: &ProxyConfig) -> Result<()> {
    let path = Path::new(&repo.local_path);
    let branch = &repo.branch;

    // Warn about local commits that will be overwritten by reset --hard.
    let unpushed = get_unpushed_commits(path, branch).await;
    if !unpushed.is_empty() {
        emit_log(
            log,
            SyncEvent::warn(format!(
                "⚠ Branch '{branch}' has {} unpushed commit(s) — these will be overwritten by reset --hard:",
                unpushed.len()
            )),
        );
        for commit in &unpushed {
            emit_log(log, SyncEvent::warn(format!("    {commit}")));
        }
    }

    emit_log(
        log,
        SyncEvent::info(format!(
            "Fetching (branch: {branch}, auth: {})…",
            repo.auth.auth_type
        )),
    );

    if let Some(aurl) = auth_url(&repo.remote_url, &repo.auth, &repo.platform) {
        // Specify the branch refspec explicitly so git fetches the right branch
        // (not just the remote's default HEAD) and maps it to origin/<branch>.
        let refspec = format!("refs/heads/{branch}:refs/remotes/origin/{branch}");
        run_git(
            log,
            path,
            &["-c", "credential.helper=", "fetch", "--progress", &aurl, &refspec],
            Some(&format!("fetch --progress <auth-url> {} (auth: {})", branch, repo.auth.auth_type)),
            Some(&repo.auth),
            Some(proxy),
        )
        .await?;
        let target = format!("origin/{branch}");
        run_git(log, path, &["reset", "--hard", &target], None, Some(&repo.auth), Some(proxy))
            .await?;
        emit_log(log, SyncEvent::info(format!("  Reset to {target}")));
    } else {
        run_git(
            log,
            path,
            &["fetch", "--progress", "origin"],
            None,
            Some(&repo.auth),
            Some(proxy),
        )
        .await?;
        let target = format!("origin/{branch}");
        run_git(log, path, &["reset", "--hard", &target], None, Some(&repo.auth), Some(proxy))
            .await?;
        emit_log(log, SyncEvent::info(format!("  Reset to {target}")));
    }
    Ok(())
}

/// Stage all changes in `path` (`git add -A`).
pub async fn git_add_all(log: &LogFn, path: &Path, auth: Option<&AuthConfig>, proxy: Option<&ProxyConfig>) -> Result<()> {
    run_git(log, path, &["add", "-A"], None, auth, proxy).await?;
    Ok(())
}

/// Return the list of staged changes (`git diff --cached --name-status`).
pub async fn get_staged_files(path: &Path) -> Result<Vec<FileChange>> {
    let out = Command::new("git")
        .args(["diff", "--cached", "--name-status"])
        .current_dir(path)
        .output()
        .await?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    Ok(parse_name_status(&stdout))
}

/// Commit staged changes. No-op (returns Ok) if nothing is staged.
pub async fn git_commit(
    log: &LogFn,
    path: &Path,
    message: &str,
    auth: Option<&AuthConfig>,
    proxy: Option<&ProxyConfig>,
) -> Result<()> {
    // Check whether there is anything staged before committing.
    let check = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(path)
        .output()
        .await?;
    if check.status.success() {
        emit_log(log, SyncEvent::info("Nothing staged — skipping commit."));
        return Ok(());
    }
    run_git(log, path, &["commit", "-m", message], None, auth, proxy).await?;
    Ok(())
}

/// Push the current branch to its remote.
pub async fn git_push_only(log: &LogFn, repo: &RepoConfig, proxy: &ProxyConfig) -> Result<()> {
    let path = Path::new(&repo.local_path);
    let branch = &repo.branch;

    emit_log(
        log,
        SyncEvent::info(format!(
            "Pushing (branch: {branch}, auth: {})…",
            repo.auth.auth_type
        )),
    );

    // Use HEAD:refs/heads/<branch> so the push targets the correct remote branch
    // regardless of what the local branch name is.  git_pull uses `git reset --hard
    // origin/<branch>` which moves the *current* local branch pointer to the desired
    // commit without renaming it, so we cannot rely on the local branch name matching.
    let refspec = format!("HEAD:refs/heads/{branch}");
    if let Some(aurl) = auth_url(&repo.remote_url, &repo.auth, &repo.platform) {
        run_git(
            log,
            path,
            &["-c", "credential.helper=", "push", "--progress", &aurl, &refspec],
            Some(&format!("push --progress <auth-url> {branch} (auth: {})", repo.auth.auth_type)),
            Some(&repo.auth),
            Some(proxy),
        )
        .await?;
    } else {
        run_git(
            log,
            path,
            &["push", "--progress", "origin", &refspec],
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

/// Discard all staged and unstaged changes in `path`, and remove untracked files.
pub async fn discard_changes(path: &Path) -> Result<()> {
    Command::new("git").args(["reset", "HEAD", "."]).current_dir(path).output().await?;
    Command::new("git").args(["checkout", "--", "."]).current_dir(path).output().await?;
    Command::new("git").args(["clean", "-fd"]).current_dir(path).output().await?;
    Ok(())
}

/// Unstage everything, then re-stage only the given paths.
/// `paths_to_add` should already include old paths for renames so that
/// the deletion of the old name is staged alongside the new file.
pub async fn stage_selected(
    log: &LogFn,
    path: &Path,
    paths_to_add: &[String],
    auth: Option<&AuthConfig>,
    proxy: Option<&ProxyConfig>,
) -> Result<()> {
    // Unstage all
    run_git(log, path, &["reset", "HEAD", "."], None, auth, proxy).await?;

    if paths_to_add.is_empty() {
        emit_log(log, SyncEvent::info("  No paths to stage."));
        return Ok(());
    }

    // Re-stage only selected paths
    let mut args = vec!["add", "--"];
    for p in paths_to_add {
        args.push(p.as_str());
    }
    run_git(log, path, &args, None, auth, proxy).await?;
    Ok(())
}

/// After commit+push, revert any remaining working-tree changes so that
/// unselected mirror changes don't linger in Repo A.
pub async fn revert_remaining(path: &Path) -> Result<()> {
    Command::new("git").args(["checkout", "--", "."]).current_dir(path).output().await?;
    Command::new("git").args(["clean", "-fd"]).current_dir(path).output().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Env-var helpers (proxy / auth) for bare Command invocations
// ---------------------------------------------------------------------------

fn apply_proxy_env(cmd: &mut Command, proxy: &ProxyConfig) {
    if !proxy.enabled { return; }
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

fn apply_auth_env(cmd: &mut Command, auth: &AuthConfig) {
    if let Some(ssh_cmd) = ssh_command(auth) {
        cmd.env("GIT_SSH_COMMAND", ssh_cmd);
    }
    let suppress_prompt = match auth.auth_type.as_str() {
        "userpass" => !auth.username.is_empty(),
        "token"    => !auth.token.is_empty(),
        "ssh"      => !auth.ssh_key_path.is_empty(),
        _          => false,
    };
    if suppress_prompt {
        cmd.env("GIT_TERMINAL_PROMPT", "0");
    }
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Structured branch list returned to the frontend.
#[derive(serde::Serialize)]
pub struct BranchList {
    pub local: Vec<String>,
    pub remote: Vec<String>,
}

/// Return local and remote branch names from cached refs — no network call.
/// Returns empty lists if the path doesn't exist or isn't a git repo.
#[tauri::command]
pub async fn list_branches(local_path: String) -> BranchList {
    let path = std::path::Path::new(&local_path);
    if local_path.is_empty() || !path.exists() {
        return BranchList { local: vec![], remote: vec![] };
    }
    branches_from_refs(path).await
}

/// Run `git fetch --prune` (with proxy) then return the updated branch list.
/// Use this when the user explicitly requests a refresh.
#[tauri::command]
pub async fn refresh_branches(local_path: String, proxy: Option<crate::models::ProxyConfig>) -> BranchList {
    let path = std::path::Path::new(&local_path);
    if local_path.is_empty() || !path.exists() {
        return BranchList { local: vec![], remote: vec![] };
    }
    let mut cmd = Command::new("git");
    // Explicit wildcard refspec: works even for single-branch clones
    cmd.args(["fetch", "origin", "--prune", "+refs/heads/*:refs/remotes/origin/*"])
        .current_dir(path);
    if let Some(ref p) = proxy { apply_proxy_env(&mut cmd, p); }
    let _ = cmd.output().await;
    branches_from_refs(path).await
}

/// Read local + remote branch names from the cached git refs (no network).
/// Uses `git for-each-ref` with full %(refname) for deterministic prefix stripping,
/// avoiding %(refname:short) ambiguity that depends on local branch names.
async fn branches_from_refs(path: &std::path::Path) -> BranchList {
    // Local: refs/heads/main → "main"
    let raw_local = branch_list(
        path,
        &["for-each-ref", "--format=%(refname)", "refs/heads/"],
    ).await;
    let local: Vec<String> = raw_local
        .into_iter()
        .filter_map(|b| b.strip_prefix("refs/heads/").map(|s| s.to_string()))
        .collect();

    // Remote: refs/remotes/origin/main → "main"
    let raw_remote = branch_list(
        path,
        &["for-each-ref", "--format=%(refname)", "refs/remotes/origin/"],
    ).await;
    let mut remote: Vec<String> = raw_remote
        .into_iter()
        .filter_map(|b| {
            let name = b.strip_prefix("refs/remotes/origin/")?.to_string();
            if name == "HEAD" || name.is_empty() { None } else { Some(name) }
        })
        .collect();
    remote.sort();
    remote.dedup();
    BranchList { local, remote }
}

async fn branch_list(path: &std::path::Path, args: &[&str]) -> Vec<String> {
    let Ok(out) = Command::new("git").args(args).current_dir(path).output().await else {
        return vec![];
    };
    if !out.status.success() { return vec![]; }
    let mut v: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    v.sort();
    v
}

/// Checkout the given branch in the repo at `local_path`.
/// If the branch exists only on the remote, creates a local tracking branch automatically.
#[tauri::command]
pub async fn checkout_branch(local_path: String, branch: String) -> Result<()> {
    let path = std::path::Path::new(&local_path);
    if local_path.is_empty() || !path.exists() {
        return Err(AppError::Validation("Local path does not exist".into()));
    }
    if branch.is_empty() {
        return Err(AppError::Validation("Branch name is empty".into()));
    }
    // First try a plain checkout (works for local branches and remote branches that
    // git can auto-track).
    let out = Command::new("git")
        .args(["checkout", &branch])
        .current_dir(path)
        .output()
        .await?;
    if out.status.success() {
        return Ok(());
    }
    // If that failed, try creating a local tracking branch from origin/<branch>.
    let out2 = Command::new("git")
        .args(["checkout", "--track", &format!("origin/{branch}")])
        .current_dir(path)
        .output()
        .await?;
    if out2.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out2.stderr);
    Err(AppError::Git(stderr.trim().to_string()))
}

/// Checkout a branch then pull latest code (fetch + reset to origin/<branch>).
/// Accepts auth and proxy so credentials work the same as the main sync flow.
#[tauri::command]
pub async fn checkout_and_pull(
    local_path: String,
    branch: String,
    remote_url: String,
    auth: crate::models::AuthConfig,
    proxy: crate::models::ProxyConfig,
    platform: String,
) -> Result<()> {
    let path = Path::new(&local_path);
    if local_path.is_empty() || !path.exists() {
        return Err(AppError::Validation("Local path does not exist".into()));
    }
    if branch.is_empty() {
        return Err(AppError::Validation("Branch name is empty".into()));
    }

    // 1. Checkout (create local tracking branch if needed)
    let co = Command::new("git")
        .args(["checkout", &branch])
        .current_dir(path)
        .output()
        .await?;
    if !co.status.success() {
        let co2 = Command::new("git")
            .args(["checkout", "--track", &format!("origin/{branch}")])
            .current_dir(path)
            .output()
            .await?;
        if !co2.status.success() {
            let stderr = String::from_utf8_lossy(&co2.stderr);
            return Err(AppError::Git(stderr.trim().to_string()));
        }
    }

    // 2. Pull latest code — use `git pull origin <branch>` with explicit remote
    //    and branch so it works on single-branch clones too.
    //    For userpass/token auth embed credentials in the URL.
    let mut pull_cmd = Command::new("git");
    if let Some(aurl) = auth_url(&remote_url, &auth, &platform) {
        pull_cmd.args(["-c", "credential.helper=", "pull", &aurl, &branch]);
    } else {
        pull_cmd.args(["pull", "origin", &branch]);
    }
    pull_cmd.current_dir(path);
    apply_proxy_env(&mut pull_cmd, &proxy);
    apply_auth_env(&mut pull_cmd, &auth);

    let pull_out = pull_cmd.output().await?;
    if !pull_out.status.success() {
        let stderr = String::from_utf8_lossy(&pull_out.stderr);
        return Err(AppError::Git(format!("pull: {}", stderr.trim())));
    }

    Ok(())
}

/// Parse `git diff --cached --name-status` output into `FileChange` objects.
fn parse_name_status(output: &str) -> Vec<FileChange> {
    let mut files = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        if parts.is_empty() {
            continue;
        }
        // Status code may have a similarity score suffix (e.g. "R90"), strip it.
        let code = parts[0].chars().next().unwrap_or('?');
        let (status, path, old_path) = match code {
            'A' => (
                "added",
                parts.get(1).unwrap_or(&"").to_string(),
                None,
            ),
            'M' => (
                "modified",
                parts.get(1).unwrap_or(&"").to_string(),
                None,
            ),
            'D' => (
                "deleted",
                parts.get(1).unwrap_or(&"").to_string(),
                None,
            ),
            'R' => (
                "renamed",
                parts.get(2).unwrap_or(&"").to_string(),
                Some(parts.get(1).unwrap_or(&"").to_string()),
            ),
            'C' => (
                "copied",
                parts.get(2).unwrap_or(&"").to_string(),
                Some(parts.get(1).unwrap_or(&"").to_string()),
            ),
            _ => (
                "unknown",
                parts.get(1).unwrap_or(&"").to_string(),
                None,
            ),
        };
        if !path.is_empty() {
            files.push(FileChange { status: status.to_string(), path, old_path });
        }
    }
    files
}
