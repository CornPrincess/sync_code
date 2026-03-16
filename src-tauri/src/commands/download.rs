use std::path::PathBuf;
use tokio::process::Command;

use crate::error::{AppError, Result};
use crate::models::ProxyConfig;

/// Percent-encode a string (keep alphanumeric and -._~).
fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for byte in s.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char);
            }
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}

/// Parse `scheme://host/rest` — returns (scheme, host, rest-without-leading-slash).
fn split_url(url: &str) -> Option<(&str, &str, &str)> {
    let (scheme, after) = url.split_once("://")?;
    let slash = after.find('/')?;
    Some((scheme, &after[..slash], &after[slash + 1..]))
}

/// Build the archive download URL and the auth header (name, value).
fn build_archive_url(
    remote_url: &str,
    branch: &str,
    platform: &str,
    token: &str,
) -> Result<(String, String, String)> {
    let (scheme, host, path_raw) =
        split_url(remote_url.trim()).ok_or_else(|| {
            AppError::Validation(format!("Invalid remote URL: {remote_url}"))
        })?;

    let project_path = path_raw.trim_end_matches(".git");

    if platform == "github" {
        // https://api.github.com/repos/{owner}/{repo}/zipball/{branch}
        let (owner, repo) = project_path.split_once('/').ok_or_else(|| {
            AppError::Validation("GitHub URL must contain owner/repo path".into())
        })?;
        let url = format!(
            "https://api.github.com/repos/{}/{}/zipball/{}",
            percent_encode(owner),
            percent_encode(repo),
            percent_encode(branch),
        );
        Ok((url, "Authorization".into(), format!("Bearer {token}")))
    } else {
        // Codeup / GitLab: https://{host}/api/v4/projects/{encoded}/repository/archive
        let encoded_path = percent_encode(project_path);
        let url = format!(
            "{scheme}://{host}/api/v4/projects/{encoded_path}/repository/archive\
             ?sha={}&format=zip",
            percent_encode(branch),
        );
        Ok((url, "PRIVATE-TOKEN".into(), token.to_string()))
    }
}

/// Download a repository ZIP archive from the platform API using `curl`.
/// Returns the path to the downloaded zip file in a temp directory.
#[tauri::command]
pub async fn download_repo_zip(
    remote_url: String,
    branch: String,
    token: String,
    platform: String,
    proxy: ProxyConfig,
) -> Result<String> {
    if remote_url.trim().is_empty() {
        return Err(AppError::Validation("Remote URL is required".into()));
    }
    if branch.trim().is_empty() {
        return Err(AppError::Validation("Branch name is required".into()));
    }
    if token.trim().is_empty() {
        return Err(AppError::Validation("Access token is required".into()));
    }

    let (url, header_name, header_value) =
        build_archive_url(&remote_url, &branch, &platform, &token)?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let tmp_dir = std::env::temp_dir().join(format!("sync-code-dl-{ts}"));
    std::fs::create_dir_all(&tmp_dir)
        .map_err(|e| AppError::Git(format!("Cannot create temp dir: {e}")))?;
    let out_path: PathBuf = tmp_dir.join("repo.zip");

    let mut args: Vec<String> = vec![
        "-L".into(),
        "--fail-with-body".into(),
        "-o".into(),
        out_path.to_string_lossy().into_owned(),
        "-H".into(),
        format!("{header_name}: {header_value}"),
    ];

    // GitHub requires Accept and User-Agent headers
    if platform == "github" {
        args.push("-H".into());
        args.push("Accept: application/vnd.github+json".into());
        args.push("-H".into());
        args.push("X-GitHub-Api-Version: 2022-11-28".into());
        args.push("-A".into());
        args.push("sync-code/1.0".into());
    }

    // Proxy
    if proxy.enabled {
        let p = if !proxy.https_proxy.is_empty() {
            &proxy.https_proxy
        } else {
            &proxy.http_proxy
        };
        if !p.is_empty() {
            args.push("--proxy".into());
            args.push(p.clone());
        }
        if !proxy.no_proxy.is_empty() {
            args.push("--noproxy".into());
            args.push(proxy.no_proxy.clone());
        }
    }

    args.push(url);

    let output = Command::new("curl")
        .args(&args)
        .output()
        .await
        .map_err(|e| AppError::Git(format!("curl not found: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(AppError::Git(format!(
            "curl failed ({}): {}{}",
            output.status,
            stderr.trim(),
            if stdout.trim().is_empty() { String::new() } else { format!(" — {}", stdout.trim()) },
        )));
    }

    Ok(out_path.to_string_lossy().into_owned())
}
