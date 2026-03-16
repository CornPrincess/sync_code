import { invoke, Channel } from '@tauri-apps/api/core';

export interface AuthConfig {
  /** "none" | "userpass" | "token" | "ssh" */
  auth_type: string;
  username: string;
  password: string;
  /** Personal / OAuth2 access token */
  token: string;
  ssh_key_path: string;
}

export interface RepoConfig {
  local_path: string;
  remote_url: string;
  branch: string;
  auth: AuthConfig;
  /** "github" | "gitlab" | "codeup" */
  platform: string;
  /** When true, use zip_path as source instead of git pull (Repo B only) */
  use_zip: boolean;
  /** Absolute path to the zip file when use_zip is true */
  zip_path: string;
  /** "local" | "api" — which ZIP source tab is active (persisted for UX) */
  zip_source_mode: string;
}

export interface ProxyConfig {
  enabled: boolean;
  http_proxy: string;
  https_proxy: string;
  no_proxy: string;
}

export interface RepoPreset {
  id: string;
  name: string;
  config: RepoConfig;
}

export interface AppConfig {
  repo_a: RepoConfig;
  repo_b: RepoConfig;
  proxy: ProxyConfig;
  repo_a_presets: RepoPreset[];
  repo_b_presets: RepoPreset[];
}

export interface SyncEvent {
  level: 'info' | 'warn' | 'error' | 'success';
  message: string;
  timestamp: string; // "HH:MM:SS"
}

export interface FileChange {
  /** "added" | "modified" | "deleted" | "renamed" | "copied" | "unknown" */
  status: string;
  path: string;
  old_path: string | null;
}

export function defaultAuthConfig(): AuthConfig {
  return { auth_type: 'none', username: '', password: '', token: '', ssh_key_path: '' };
}

export function defaultRepoConfig(): RepoConfig {
  return { local_path: '', remote_url: '', branch: '', auth: defaultAuthConfig(), platform: 'github', use_zip: false, zip_path: '', zip_source_mode: 'local' };
}

export function defaultProxyConfig(): ProxyConfig {
  return { enabled: false, http_proxy: '', https_proxy: '', no_proxy: '' };
}

export function defaultAppConfig(): AppConfig {
  return {
    repo_a: defaultRepoConfig(),
    repo_b: defaultRepoConfig(),
    proxy: defaultProxyConfig(),
    repo_a_presets: [],
    repo_b_presets: [],
  };
}

// ---------------------------------------------------------------------------
// Runtime environment detection
// ---------------------------------------------------------------------------

/** Returns true when running inside a Tauri desktop window. */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && !!(window as Record<string, unknown>).__TAURI_INTERNALS__;
}

// ---------------------------------------------------------------------------
// HTTP helpers used in web mode
// ---------------------------------------------------------------------------

async function webGet<T>(path: string, params?: Record<string, string>): Promise<T> {
  const url = new URL(`/api${path}`, window.location.origin);
  if (params) Object.entries(params).forEach(([k, v]) => url.searchParams.set(k, v));
  const res = await fetch(url.toString());
  if (!res.ok) throw new Error(await res.text());
  return res.json() as Promise<T>;
}

async function webPost<T>(path: string, body?: unknown): Promise<T> {
  const res = await fetch(`/api${path}`, {
    method: 'POST',
    headers: body !== undefined ? { 'Content-Type': 'application/json' } : {},
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  if (!res.ok) throw new Error(await res.text());
  // 204 No Content → return undefined cast to T
  if (res.status === 204) return undefined as unknown as T;
  return res.json() as Promise<T>;
}

/**
 * POST a streaming endpoint and forward SSE-style lines to onEvent.
 * Each line: `data: {"type":"log"|"result"|"error","payload":...}\n\n`
 * Resolves with the result payload on success, rejects with Error on failure.
 */
async function webStream<T>(
  path: string,
  body: unknown,
  onEvent: (e: SyncEvent) => void,
): Promise<T> {
  const res = await fetch(`/api${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(await res.text());
  if (!res.body) throw new Error('No response body');

  const reader = res.body.getReader();
  const dec = new TextDecoder();
  let buf = '';

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    buf += dec.decode(value, { stream: true });
    const lines = buf.split('\n');
    buf = lines.pop()!; // keep incomplete last line
    for (const line of lines) {
      if (!line.startsWith('data: ')) continue;
      const msg = JSON.parse(line.slice(6)) as { type: string; payload: unknown };
      if (msg.type === 'log') {
        onEvent(msg.payload as SyncEvent);
      } else if (msg.type === 'result') {
        return msg.payload as T;
      } else if (msg.type === 'error') {
        throw new Error(msg.payload as string);
      }
    }
  }
  throw new Error('Stream ended without a result');
}

// ---------------------------------------------------------------------------
// Public API — each function works in both Tauri and web mode
// ---------------------------------------------------------------------------

export async function loadConfig(): Promise<AppConfig> {
  if (isTauri()) return invoke<AppConfig>('load_config');
  return webGet<AppConfig>('/config');
}

export async function saveConfig(config: AppConfig): Promise<void> {
  if (isTauri()) return invoke<void>('save_config', { config });
  return webPost<void>('/config', config);
}

/**
 * Start the sync and stream log events via a Tauri v2 Channel.
 * The Channel shares the same IPC pipe as invoke responses, so events
 * are guaranteed to arrive while the command runs (no race with emit/listen).
 */
/** Run sync steps 1-6, returning staged file changes for review. */
export async function startSync(
  config: AppConfig,
  onEvent: (event: SyncEvent) => void,
): Promise<FileChange[]> {
  if (isTauri()) {
    const channel = new Channel<SyncEvent>();
    channel.onmessage = onEvent;
    return invoke<FileChange[]>('start_sync', { config, onEvent: channel });
  }
  return webStream<FileChange[]>('/sync/start', config, onEvent);
}

/**
 * Commit only the selected files and push Repo A.
 * `pathsToStage`: list of file paths to `git add`. For renamed files,
 * include both the new path and the old path so the deletion is staged too.
 */
export async function commitAndPush(
  config: AppConfig,
  commitMessage: string,
  pathsToStage: string[],
  onEvent: (event: SyncEvent) => void,
): Promise<void> {
  if (isTauri()) {
    const channel = new Channel<SyncEvent>();
    channel.onmessage = onEvent;
    return invoke<void>('commit_and_push', { config, commitMessage, pathsToStage, onEvent: channel });
  }
  return webStream<void>(
    '/sync/commit',
    { config, commit_message: commitMessage, paths_to_stage: pathsToStage },
    onEvent,
  );
}

/** Discard all staged / unstaged changes in Repo A (undoes the mirror). */
export async function discardSync(config: AppConfig): Promise<void> {
  if (isTauri()) return invoke<void>('discard_sync', { config });
  return webPost<void>('/sync/discard', config);
}

export interface BranchList {
  local: string[];
  remote: string[];
}

/**
 * List local and remote branch names from cached git refs — no network call.
 * Returns empty lists if the path is empty, doesn't exist, or isn't a git repo.
 */
export async function listBranches(localPath: string): Promise<BranchList> {
  if (isTauri()) return invoke<BranchList>('list_branches', { localPath });
  return webGet<BranchList>('/branches', { path: localPath });
}

/**
 * Run `git fetch --prune` (with proxy) then return the updated branch list.
 * Call this when the user explicitly requests a refresh.
 */
export async function refreshBranches(localPath: string, proxy: ProxyConfig): Promise<BranchList> {
  if (isTauri()) return invoke<BranchList>('refresh_branches', { localPath, proxy });
  return webPost<BranchList>('/branches/refresh', { local_path: localPath, proxy });
}

/** Run `git checkout <branch>` in the given repo. Throws on failure. */
export async function checkoutBranch(localPath: string, branch: string): Promise<void> {
  if (isTauri()) return invoke<void>('checkout_branch', { localPath, branch });
  return webPost<void>('/branches/checkout', { local_path: localPath, branch });
}

/**
 * Download a repository archive from the platform API (Codeup / GitLab / GitHub).
 * @param format  "zip" or "tar.gz" (default "zip")
 * Returns the server-side path to the downloaded archive file.
 */
export async function downloadZipFromRepo(
  remoteUrl: string,
  branch: string,
  token: string,
  platform: string,
  proxy: ProxyConfig,
  format: string = 'zip',
): Promise<string> {
  if (isTauri()) {
    return invoke<string>('download_repo_zip', { remoteUrl, branch, token, platform, proxy, format });
  }
  return webPost<string>('/download/repo-zip', { remote_url: remoteUrl, branch, token, platform, proxy, format });
}

/**
 * Checkout a branch then pull latest code (fetch + reset to origin/<branch>).
 * Passes auth and proxy so credentials match the main sync flow.
 */
export async function checkoutAndPull(
  localPath: string,
  branch: string,
  remoteUrl: string,
  auth: AuthConfig,
  proxy: ProxyConfig,
  platform: string,
): Promise<void> {
  if (isTauri()) {
    return invoke<void>('checkout_and_pull', { localPath, branch, remoteUrl, auth, proxy, platform });
  }
  return webPost<void>('/branches/checkout-pull', {
    local_path: localPath,
    branch,
    remote_url: remoteUrl,
    auth,
    proxy,
    platform,
  });
}
