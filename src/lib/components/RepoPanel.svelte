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
    <span class="field-label">Branch</span>
    <input
      type="text"
      bind:value={config.branch}
      onchange={onchange}
      placeholder="main"
      class="input branch-input"
    />
  </label>

  <!-- Authentication section -->
  <details class="auth-details">
    <summary class="auth-summary">
      Authentication
      {#if config.auth.auth_type !== 'none'}
        <span class="auth-badge">{config.auth.auth_type === 'userpass' ? 'User/Password' : 'SSH Key'}</span>
      {/if}
    </summary>

    <div class="auth-body">
      <!-- Auth type selector -->
      <div class="radio-group">
        <label class="radio-option">
          <input
            type="radio"
            bind:group={config.auth.auth_type}
            value="none"
            onchange={onchange}
          />
          <span>None</span>
        </label>
        <label class="radio-option">
          <input
            type="radio"
            bind:group={config.auth.auth_type}
            value="userpass"
            onchange={onchange}
          />
          <span>Username / Password</span>
        </label>
        <label class="radio-option">
          <input
            type="radio"
            bind:group={config.auth.auth_type}
            value="ssh"
            onchange={onchange}
          />
          <span>SSH Key</span>
        </label>
      </div>

      {#if config.auth.auth_type === 'userpass'}
        <div class="auth-tip">
          <strong>GitHub / GitLab / Gitea</strong> no longer accept account passwords for Git
          operations. Use a <strong>Personal Access Token (PAT)</strong> as the password:
          <ul>
            <li>GitHub: Settings → Developer settings → Personal access tokens → Generate new token (scope: <code>repo</code>)</li>
            <li>GitLab: User Settings → Access Tokens (scope: <code>read_repository</code> + <code>write_repository</code>)</li>
          </ul>
        </div>
        <label class="field">
          <span class="field-label">Username</span>
          <input
            type="text"
            bind:value={config.auth.username}
            onchange={onchange}
            placeholder="your git username (e.g. octocat)"
            class="input"
            autocomplete="off"
          />
        </label>
        <label class="field">
          <span class="field-label">Personal Access Token</span>
          <input
            type="password"
            bind:value={config.auth.password}
            onchange={onchange}
            placeholder="ghp_xxxxxxxxxxxx  (NOT your account password)"
            class="input"
            autocomplete="off"
          />
        </label>
        <p class="auth-note">⚠ Stored in plain text in the app config file. Use a PAT with minimal scopes.</p>
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

  .radio-group {
    display: flex;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 4px;
  }

  .radio-option {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.875rem;
    color: var(--text-primary);
    cursor: pointer;
  }

  .radio-option input[type='radio'] {
    accent-color: var(--accent);
    cursor: pointer;
  }

  .auth-tip {
    font-size: 0.78rem;
    color: var(--text-secondary);
    background: #1c2128;
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 8px 10px;
    line-height: 1.5;
  }

  .auth-tip strong {
    color: var(--color-warn);
  }

  .auth-tip ul {
    margin: 4px 0 0 0;
    padding-left: 16px;
  }

  .auth-tip li {
    margin: 2px 0;
  }

  .auth-tip code {
    font-family: var(--font-mono);
    background: #0d1117;
    padding: 1px 4px;
    border-radius: 3px;
    font-size: 0.75rem;
  }

  .auth-note {
    margin: 0;
    font-size: 0.75rem;
    color: var(--text-muted);
    font-style: italic;
  }
</style>
