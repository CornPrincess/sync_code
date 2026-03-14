import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export interface RepoConfig {
  local_path: string;
  remote_url: string;
  branch: string;
}

export interface AppConfig {
  repo_a: RepoConfig;
  repo_b: RepoConfig;
}

export interface SyncEvent {
  level: 'info' | 'warn' | 'error' | 'success';
  message: string;
}

export function defaultRepoConfig(): RepoConfig {
  return { local_path: '', remote_url: '', branch: '' };
}

export function defaultAppConfig(): AppConfig {
  return { repo_a: defaultRepoConfig(), repo_b: defaultRepoConfig() };
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
