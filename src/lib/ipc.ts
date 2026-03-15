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
}

export interface ProxyConfig {
  enabled: boolean;
  http_proxy: string;
  https_proxy: string;
  no_proxy: string;
}

export interface AppConfig {
  repo_a: RepoConfig;
  repo_b: RepoConfig;
  proxy: ProxyConfig;
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
  return { local_path: '', remote_url: '', branch: '', auth: defaultAuthConfig() };
}

export function defaultProxyConfig(): ProxyConfig {
  return { enabled: false, http_proxy: '', https_proxy: '', no_proxy: '' };
}

export function defaultAppConfig(): AppConfig {
  return {
    repo_a: defaultRepoConfig(),
    repo_b: defaultRepoConfig(),
    proxy: defaultProxyConfig(),
  };
}

export async function loadConfig(): Promise<AppConfig> {
  return invoke<AppConfig>('load_config');
}

export async function saveConfig(config: AppConfig): Promise<void> {
  return invoke<void>('save_config', { config });
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
  const channel = new Channel<SyncEvent>();
  channel.onmessage = onEvent;
  return invoke<FileChange[]>('start_sync', { config, onEvent: channel });
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
  const channel = new Channel<SyncEvent>();
  channel.onmessage = onEvent;
  return invoke<void>('commit_and_push', { config, commitMessage, pathsToStage, onEvent: channel });
}

/** Discard all staged / unstaged changes in Repo A (undoes the mirror). */
export async function discardSync(config: AppConfig): Promise<void> {
  return invoke<void>('discard_sync', { config });
}
