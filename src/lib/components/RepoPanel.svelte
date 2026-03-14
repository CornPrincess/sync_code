<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import type { RepoConfig } from '../ipc.js';

  let {
    label,
    config = $bindable(),
    onchange
  }: {
    label: string;
    config: RepoConfig;
    onchange?: () => void;
  } = $props();

  async function browseFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === 'string') {
      config.local_path = selected;
      onchange?.();
    }
  }
</script>

<div class="repo-panel">
  <h2 class="panel-title">{label}</h2>

  <label class="field">
    <span class="field-label">Local Path</span>
    <div class="path-row">
      <input
        type="text"
        bind:value={config.local_path}
        onchange={onchange}
        placeholder="/home/user/my-repo"
        class="input"
      />
      <button type="button" class="btn-browse" onclick={browseFolder}>Browse</button>
    </div>
  </label>

  <label class="field">
    <span class="field-label">Remote URL</span>
    <input
      type="text"
      bind:value={config.remote_url}
      onchange={onchange}
      placeholder="git@github.com:user/repo.git"
      class="input"
    />
  </label>

  <label class="field">
    <span class="field-label">Branch</span>
    <input
      type="text"
      bind:value={config.branch}
      onchange={onchange}
      placeholder="main"
      class="input branch-input"
    />
  </label>
</div>

<style>
  .repo-panel {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 20px;
    flex: 1;
    min-width: 0;
  }

  .panel-title {
    margin: 0 0 16px 0;
    font-size: 1rem;
    font-weight: 600;
    color: var(--text-primary);
    letter-spacing: 0.02em;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 14px;
  }

  .field-label {
    font-size: 0.8rem;
    font-weight: 500;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .path-row {
    display: flex;
    gap: 8px;
  }

  .path-row .input {
    flex: 1;
  }

  .input {
    width: 100%;
    padding: 8px 10px;
    background: var(--input-bg);
    border: 1px solid var(--border);
    border-radius: 5px;
    color: var(--text-primary);
    font-size: 0.875rem;
    font-family: var(--font-mono);
    box-sizing: border-box;
    transition: border-color 0.15s;
  }

  .input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .branch-input {
    max-width: 200px;
  }

  .btn-browse {
    padding: 8px 14px;
    background: var(--btn-secondary-bg);
    border: 1px solid var(--border);
    border-radius: 5px;
    color: var(--text-primary);
    font-size: 0.875rem;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.15s;
  }

  .btn-browse:hover {
    background: var(--btn-secondary-hover);
  }
</style>
