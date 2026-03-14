import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

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

export async function startSync(config: AppConfig): Promise<void> {
  return invoke<void>('start_sync', { config });
}

export function onSyncLog(handler: (event: SyncEvent) => void): Promise<UnlistenFn> {
  return listen<SyncEvent>('sync://log', (e) => handler(e.payload));
}
