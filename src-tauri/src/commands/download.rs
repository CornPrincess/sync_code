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

/// Build the archive download URL and auth headers as (name, value) pairs.
/// Returns `(url, headers)`.  `format` is "zip" or "tar.gz".
fn build_archive_url(
    remote_url: &str,
    branch: &str,
    platform: &str,
    token: &str,
    format: &str,
) -> Result<(String, Vec<(String, String)>)> {
    let (scheme, host, path_raw) =
        split_url(remote_url.trim()).ok_or_else(|| {
            AppError::Validation(format!("Invalid remote URL: {remote_url}"))
        })?;

    let project_path = path_raw.trim_end_matches(".git");

    if platform == "github" {
        // GitHub provides separate zipball / tarball endpoints
        let archive_type = if format == "tar.gz" { "tarball" } else { "zipball" };
        let (owner, repo) = project_path.split_once('/').ok_or_else(|| {
            AppError::Validation("GitHub URL must contain owner/repo path".into())
        })?;
        let url = format!(
            "https://api.github.com/repos/{}/{}/{}/{}",
            percent_encode(owner),
            percent_encode(repo),
            archive_type,
            percent_encode(branch),
        );
        Ok((url, vec![("Authorization".into(), format!("Bearer {token}"))]))
    } else if platform == "codeup" {
        // Codeup (Yunxiao/阿里云): send both PRIVATE-TOKEN (GitLab-compatible PAT)
        // and x-yunxiao-token (OAPI PAT) — server honours whichever token type the user has.
        let encoded_path = percent_encode(project_path);
        let url = format!(
            "{scheme}://{host}/api/v4/projects/{encoded_path}/repository/archive\
             ?sha={}&format={}",
            percent_encode(branch),
            percent_encode(format),
        );
        Ok((url, vec![
            ("PRIVATE-TOKEN".into(), token.to_string()),
            ("x-yunxiao-token".into(), token.to_string()),
        ]))
    } else {
        // GitLab: format parameter selects zip or tar.gz
        let encoded_path = percent_encode(project_path);
        let url = format!(
            "{scheme}://{host}/api/v4/projects/{encoded_path}/repository/archive\
             ?sha={}&format={}",
            percent_encode(branch),
            percent_encode(format),
        );
        Ok((url, vec![("PRIVATE-TOKEN".into(), token.to_string())]))
    }
}

/// Download a repository archive from the platform API using `curl`.
/// `format` is "zip" or "tar.gz" (defaults to "zip" when empty).
/// Returns the path to the downloaded archive file in a temp directory.
#[tauri::command]
pub async fn download_repo_zip(
    remote_url: String,
    branch: String,
    token: String,
    platform: String,
    proxy: ProxyConfig,
    format: Option<String>,
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

    let fmt = format.as_deref().unwrap_or("zip");
    let (url, auth_headers) =
        build_archive_url(&remote_url, &branch, &platform, &token, fmt)?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let tmp_dir = std::env::temp_dir().join(format!("sync-code-dl-{ts}"));
    std::fs::create_dir_all(&tmp_dir)
        .map_err(|e| AppError::Git(format!("Cannot create temp dir: {e}")))?;
    let filename = if fmt == "tar.gz" { "repo.tar.gz" } else { "repo.zip" };
    let out_path: PathBuf = tmp_dir.join(filename);

    let mut args: Vec<String> = vec![
        "-L".into(),
        "--fail-with-body".into(),
        "-o".into(),
        out_path.to_string_lossy().into_owned(),
    ];

    for (name, value) in &auth_headers {
        args.push("-H".into());
        args.push(format!("{name}: {value}"));
    }

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

    // Verify magic bytes — server may return a different format than requested
    // (e.g. Codeup returns zip regardless of format param).
    let header = std::fs::read(&out_path)
        .map_err(|e| AppError::Git(format!("Cannot read downloaded file: {e}")))?;

    let is_zip   = header.len() >= 4 && header[0] == 0x50 && header[1] == 0x4B
                                     && header[2] == 0x03 && header[3] == 0x04;
    let is_gzip  = header.len() >= 2 && header[0] == 0x1F && header[1] == 0x8B;

    if !is_zip && !is_gzip {
        let preview = String::from_utf8_lossy(&header[..header.len().min(300)]);
        return Err(AppError::Git(format!(
            "下载失败：服务器返回了非压缩包内容，请检查仓库地址、分支和 Token 是否正确。响应预览: {}",
            preview.trim()
        )));
    }

    // Rename file if the actual format differs from what was requested
    let actual_filename = if is_gzip { "repo.tar.gz" } else { "repo.zip" };
    if actual_filename != filename {
        let corrected = tmp_dir.join(actual_filename);
        std::fs::rename(&out_path, &corrected)
            .map_err(|e| AppError::Git(format!("Cannot rename downloaded file: {e}")))?;
        return Ok(corrected.to_string_lossy().into_owned());
    }

    Ok(out_path.to_string_lossy().into_owned())
}
