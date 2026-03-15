<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { listBranches, type RepoConfig } from '../ipc.js';

  let {
    label,
    config = $bindable(),
    onchange
  }: {
    label: string;
    config: RepoConfig;
    onchange?: () => void;
  } = $props();

  // Unique datalist id per panel instance
  const listId = $derived(`branches-${label.replace(/\s+/g, '-').toLowerCase()}`);

  let branches = $state<string[]>([]);
  let loadingBranches = $state(false);

  async function fetchBranches(path: string) {
    if (!path) { branches = []; return; }
    loadingBranches = true;
    try {
      branches = await listBranches(path);
    } catch {
      branches = [];
    } finally {
      loadingBranches = false;
    }
  }

  // Refresh branch list whenever local_path changes
  $effect(() => { fetchBranches(config.local_path); });

  async function browseFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === 'string') {
      config.local_path = selected;
      onchange?.();
    }
  }

  async function browseSSHKey() {
    const selected = await open({ directory: false, multiple: false });
    if (typeof selected === 'string') {
      config.auth.ssh_key_path = selected;
      onchange?.();
    }
  }
</script>

<div class="repo-panel">
  <h2 class="panel-title">{label}</h2>

  <!-- Basic repo fields -->
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
    <span class="field-label">
      Branch
      {#if loadingBranches}
        <span class="branch-loading">…</span>
      {:else if branches.length > 0}
        <span class="branch-count">{branches.length}</span>
      {/if}
    </span>
    <input
      type="text"
      bind:value={config.branch}
      onchange={onchange}
      placeholder="main"
      list={listId}
      class="input branch-input"
      autocomplete="off"
    />
    <datalist id={listId}>
      {#each branches as b}
        <option value={b}></option>
      {/each}
    </datalist>
  </label>

  <!-- Authentication section -->
  <details class="auth-details">
    <summary class="auth-summary">
      Authentication
      {#if config.auth.auth_type !== 'none'}
        <span class="auth-badge">
          {config.auth.auth_type === 'userpass' ? 'User/Password'
            : config.auth.auth_type === 'token' ? 'Access Token'
            : 'SSH Key'}
        </span>
      {/if}
    </summary>

    <div class="auth-body">
      <!-- Auth type selector -->
      <label class="field">
        <span class="field-label">Auth Type</span>
        <select class="input select" bind:value={config.auth.auth_type} onchange={onchange}>
          <option value="none">None (system credential helper)</option>
          <option value="userpass">Username / Password</option>
          <option value="token">Access Token — GitHub / GitLab / Codeup / Gitea</option>
          <option value="ssh">SSH Key</option>
        </select>
      </label>

      {#if config.auth.auth_type === 'userpass'}
        <label class="field">
          <span class="field-label">Username</span>
          <input
            type="text"
            bind:value={config.auth.username}
            onchange={onchange}
            placeholder="your git username"
            class="input"
            autocomplete="off"
          />
        </label>
        <label class="field">
          <span class="field-label">Password</span>
          <input
            type="password"
            bind:value={config.auth.password}
            onchange={onchange}
            placeholder="account password"
            class="input"
            autocomplete="off"
          />
        </label>
        <p class="auth-note">⚠ Stored in plain text. For GitHub/GitLab use "Access Token" mode instead.</p>
      {/if}

      {#if config.auth.auth_type === 'token'}
        <label class="field">
          <span class="field-label">Access Token</span>
          <input
            type="password"
            bind:value={config.auth.token}
            onchange={onchange}
            placeholder="ghp_xxx / glpat-xxx / your-token"
            class="input"
            autocomplete="off"
          />
        </label>
        <p class="auth-note">
          Sent as <code>oauth2:&lt;token&gt;</code> — works with GitHub, GitLab, Codeup, Gitea.
          ⚠ Stored in plain text.
        </p>
      {/if}

      {#if config.auth.auth_type === 'ssh'}
        <label class="field">
          <span class="field-label">SSH Private Key Path</span>
          <div class="path-row">
            <input
              type="text"
              bind:value={config.auth.ssh_key_path}
              onchange={onchange}
              placeholder="~/.ssh/id_ed25519"
              class="input"
            />
            <button type="button" class="btn-browse" onclick={browseSSHKey}>Browse</button>
          </div>
        </label>
        <p class="auth-note">Uses GIT_SSH_COMMAND with StrictHostKeyChecking=accept-new.</p>
      {/if}
    </div>
  </details>
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
    max-width: 240px;
  }

  .branch-loading {
    font-size: 0.75rem;
    color: var(--text-muted);
    font-weight: 400;
    font-style: italic;
    letter-spacing: 0;
    text-transform: none;
  }

  .branch-count {
    font-size: 0.68rem;
    font-weight: 500;
    color: var(--text-muted);
    background: var(--btn-secondary-bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0 5px;
    letter-spacing: 0;
    text-transform: none;
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

  /* Authentication section */
  .auth-details {
    border: 1px solid var(--border);
    border-radius: 6px;
    margin-top: 4px;
  }

  .auth-summary {
    padding: 8px 12px;
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    cursor: pointer;
    user-select: none;
    display: flex;
    align-items: center;
    gap: 8px;
    list-style: none;
  }

  .auth-summary::-webkit-details-marker {
    display: none;
  }

  .auth-summary::before {
    content: '▶';
    font-size: 0.6rem;
    transition: transform 0.15s;
    color: var(--text-muted);
  }

  .auth-details[open] .auth-summary::before {
    transform: rotate(90deg);
  }

  .auth-badge {
    font-size: 0.7rem;
    background: var(--accent);
    color: #fff;
    padding: 1px 6px;
    border-radius: 10px;
    font-weight: 500;
    text-transform: none;
    letter-spacing: 0;
  }

  .auth-body {
    padding: 12px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .select {
    appearance: auto;
    cursor: pointer;
    width: 100%;
  }

  .auth-note {
    margin: 0;
    font-size: 0.75rem;
    color: var(--text-muted);
    font-style: italic;
  }

  .auth-note code {
    font-family: var(--font-mono);
    background: #0d1117;
    padding: 1px 4px;
    border-radius: 3px;
    font-size: 0.72rem;
    font-style: normal;
  }
</style>
