<script lang="ts">
  import { onMount } from 'svelte';
  import RepoPanel from './lib/components/RepoPanel.svelte';
  import LogViewer from './lib/components/LogViewer.svelte';
  import StatusBadge from './lib/components/StatusBadge.svelte';
  import { configStore } from './lib/stores/config.svelte.js';
  import FileTree from './lib/components/FileTree.svelte';
  import {
    startSync,
    commitAndPush,
    discardSync,
    type AppConfig,
    type SyncEvent,
    type FileChange,
  } from './lib/ipc.js';

  type SyncStatus = 'idle' | 'syncing' | 'review' | 'pushing' | 'success' | 'error';

  let status = $state<SyncStatus>('idle');
  let lastSync = $state<string | undefined>(undefined);
  let errorMessage = $state<string | undefined>(undefined);
  let lines = $state<SyncEvent[]>([]);

  // Snapshot of the config that was used for the most recent sync.
  // commit_and_push must use this exact snapshot so that branch/auth/proxy
  // cannot drift if the user edits settings while reviewing changes.
  let syncedConfig = $state<AppConfig | null>(null);

  // Review state
  let pendingFiles = $state<FileChange[]>([]);
  let selectedPaths = $state(new Set<string>());
  let commitMessage = $state('');

  onMount(() => {
    configStore.load();
  });

  function defaultCommitMessage(): string {
    return `sync: ${new Date().toLocaleString('sv').replace('T', ' ')}`;
  }

  function validate(): string | null {
    const a = configStore.value.repo_a;
    const b = configStore.value.repo_b;
    if (!a.local_path.trim()) return 'Repo A: local path is required.';
    if (!a.branch.trim()) return 'Repo A: branch is required.';
    if (b.use_zip) {
      // In API download mode the path is filled automatically after clicking download.
      // Don't validate zip_path here — the backend will surface a clear error if needed.
      if (b.zip_source_mode !== 'api' && !b.zip_path?.trim()) return 'Repo B: ZIP 文件路径不能为空。';
    } else {
      if (!b.local_path.trim()) return 'Repo B: local path is required.';
      if (!b.branch.trim()) return 'Repo B: branch is required.';
    }
    return null;
  }

  async function handleSync() {
    errorMessage = undefined;
    const err = validate();
    if (err) { errorMessage = err; return; }

    // Snapshot config so commit_and_push uses the same branch/auth/proxy even if
    // the user edits settings while reviewing changes in the review panel.
    syncedConfig = JSON.parse(JSON.stringify(configStore.value)) as AppConfig;

    lines = [];
    status = 'syncing';
    try {
      const files = await startSync(syncedConfig, (event) => {
        lines = [...lines, event];
      });
      pendingFiles = files;
      selectedPaths = new Set(files.map((f) => f.path)); // select all by default
      commitMessage = defaultCommitMessage();
      status = 'review';
    } catch (e: unknown) {
      syncedConfig = null;
      status = 'error';
      errorMessage = e instanceof Error ? e.message : typeof e === 'string' ? e : 'An unexpected error occurred.';
    }
  }

  async function handlePush() {
    errorMessage = undefined;
    status = 'pushing';

    // For renamed files include both new + old path so git stages the deletion too
    const pathsToStage = [...selectedPaths].flatMap((path) => {
      const f = pendingFiles.find((x) => x.path === path);
      return f?.old_path ? [path, f.old_path] : [path];
    });

    // Use the snapshotted config to guarantee the same branch that was synced.
    const cfg = syncedConfig ?? configStore.value;
    try {
      await commitAndPush(cfg, commitMessage, pathsToStage, (event) => {
        lines = [...lines, event];
      });
      status = 'success';
      lastSync = new Date().toLocaleTimeString();
      pendingFiles = [];
      selectedPaths = new Set();
      syncedConfig = null;
    } catch (e: unknown) {
      status = 'error';
      errorMessage = e instanceof Error ? e.message : typeof e === 'string' ? e : 'Push failed.';
    }
  }

  async function handleDiscard() {
    const cfg = syncedConfig ?? configStore.value;
    try { await discardSync(cfg); } catch { /* best effort */ }
    pendingFiles = [];
    selectedPaths = new Set();
    syncedConfig = null;
    status = 'idle';
  }

  function onConfigChange() {
    configStore.save().catch(console.error);
  }
</script>

<main class="app">
  <header class="app-header">
    <h1 class="app-title">sync_code</h1>
    <p class="app-subtitle">Sync code from Repo B into Repo A, then push.</p>
  </header>

  <!-- Repo configuration + proxy -->
  <div class="config-area">
    <section class="repo-grid">
      <RepoPanel
        label="Repo A (target)"
        bind:config={configStore.value.repo_a}
        bind:presets={configStore.value.repo_a_presets}
        proxy={configStore.value.proxy}
        onchange={onConfigChange}
      />
      <div class="arrow" aria-hidden="true">←</div>
      <RepoPanel
        label="Repo B (source)"
        bind:config={configStore.value.repo_b}
        bind:presets={configStore.value.repo_b_presets}
        proxy={configStore.value.proxy}
        onchange={onConfigChange}
        showZipOption={true}
      />
    </section>

    <section class="proxy-section">
      <details class="proxy-details">
        <summary class="proxy-summary">
          Network Proxy
          {#if configStore.value.proxy.enabled}
            <span class="proxy-badge">Enabled</span>
          {/if}
        </summary>
        <div class="proxy-body">
          <label class="checkbox-option">
            <input
              type="checkbox"
              bind:checked={configStore.value.proxy.enabled}
              onchange={onConfigChange}
            />
            <span>Enable proxy for all git operations</span>
          </label>
          {#if configStore.value.proxy.enabled}
            <div class="proxy-fields">
              <label class="field">
                <span class="field-label">HTTP Proxy</span>
                <input type="text" bind:value={configStore.value.proxy.http_proxy} onchange={onConfigChange} placeholder="http://proxy.example.com:8080" class="input" />
              </label>
              <label class="field">
                <span class="field-label">HTTPS Proxy</span>
                <input type="text" bind:value={configStore.value.proxy.https_proxy} onchange={onConfigChange} placeholder="http://proxy.example.com:8080" class="input" />
              </label>
              <label class="field">
                <span class="field-label">No Proxy (comma-separated)</span>
                <input type="text" bind:value={configStore.value.proxy.no_proxy} onchange={onConfigChange} placeholder="localhost,127.0.0.1" class="input" />
              </label>
            </div>
          {/if}
        </div>
      </details>
    </section>
  </div>

  <!-- Action bar -->
  <section class="actions">
    {#if errorMessage}
      <div class="error-banner" role="alert">{errorMessage}</div>
    {/if}
    <div class="actions-row">
      <StatusBadge {status} {lastSync} />
      <button
        class="btn-sync"
        onclick={handleSync}
        disabled={status === 'syncing' || status === 'review' || status === 'pushing'}
      >
        {status === 'syncing' ? 'Syncing…' : 'Sync Now'}
      </button>
    </div>
  </section>

  <!-- ─── Review panel (appears after sync completes) ──────────────────── -->
  {#if status === 'review' || status === 'pushing'}
    <section class="review-panel">
      <div class="review-header">
        <span class="review-title">
          {#if pendingFiles.length === 0}
            ✔ Nothing to push — Repo A is already up to date
          {:else}
            Review Changes
          {/if}
        </span>
        {#if pendingFiles.length > 0}
          <span class="review-count">{pendingFiles.length} file{pendingFiles.length !== 1 ? 's' : ''}</span>
        {/if}
      </div>

      {#if pendingFiles.length > 0}
        <div class="review-body">
          <!-- Left: hierarchical file tree with checkboxes -->
          <div class="file-tree-wrap">
            <FileTree files={pendingFiles} bind:selected={selectedPaths} />
          </div>

          <!-- Right: commit message + buttons -->
          <div class="commit-pane">
            <label class="field">
              <span class="field-label">Commit Message</span>
              <textarea
                class="commit-msg"
                bind:value={commitMessage}
                rows="5"
                placeholder="Describe what changed…"
                disabled={status === 'pushing'}
              ></textarea>
            </label>
            <p class="commit-hint">Leave blank to use the default timestamp message.</p>

            <div class="commit-actions">
              <button class="btn-discard" onclick={handleDiscard} disabled={status === 'pushing'}>
                Discard
              </button>
              <button
                class="btn-push"
                onclick={handlePush}
                disabled={status === 'pushing' || selectedPaths.size === 0}
                title={selectedPaths.size === 0 ? 'Select at least one file' : ''}
              >
                {status === 'pushing' ? 'Pushing…' : `Commit & Push (${selectedPaths.size}) →`}
              </button>
            </div>
          </div>
        </div>
      {:else}
        <!-- Nothing to push -->
        <div class="review-empty-actions">
          <button class="btn-discard" onclick={handleDiscard}>Done</button>
        </div>
      {/if}
    </section>
  {/if}

  <!-- Output log -->
  <section class="log-section">
    <LogViewer bind:lines />
  </section>
</main>

<style>
  .app {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 24px;
    min-height: 100vh;
    box-sizing: border-box;
  }

  .app-header { flex-shrink: 0; }

  .app-title {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .app-subtitle {
    margin: 4px 0 0;
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  /* Config area */
  .config-area {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .repo-grid {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }

  .arrow {
    font-size: 1.5rem;
    color: var(--text-muted);
    padding-top: 44px;
    flex-shrink: 0;
  }

  /* Proxy section */
  .proxy-details {
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
  }

  .proxy-summary {
    padding: 8px 14px;
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

  .proxy-summary::-webkit-details-marker { display: none; }

  .proxy-summary::before {
    content: '▶';
    font-size: 0.6rem;
    transition: transform 0.15s;
    color: var(--text-muted);
  }

  .proxy-details[open] .proxy-summary::before { transform: rotate(90deg); }

  .proxy-badge {
    font-size: 0.7rem;
    background: var(--color-warn);
    color: #000;
    padding: 1px 6px;
    border-radius: 10px;
    font-weight: 500;
    text-transform: none;
    letter-spacing: 0;
  }

  .proxy-body {
    padding: 12px 14px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .proxy-fields {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px 16px;
  }

  .proxy-fields .field:last-child { grid-column: 1 / -1; }

  .checkbox-option {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.875rem;
    color: var(--text-primary);
    cursor: pointer;
  }

  .checkbox-option input[type='checkbox'] { accent-color: var(--accent); cursor: pointer; }

  .field { display: flex; flex-direction: column; gap: 6px; }

  .field-label {
    font-size: 0.8rem;
    font-weight: 500;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
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

  .input:focus { outline: none; border-color: var(--accent); }

  /* Action bar */
  .actions { flex-shrink: 0; }

  .actions-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .error-banner {
    background: #450a0a55;
    border: 1px solid #dc262644;
    color: var(--color-error);
    border-radius: 6px;
    padding: 8px 12px;
    font-size: 0.875rem;
    margin-bottom: 10px;
  }

  .btn-sync {
    padding: 10px 28px;
    background: var(--accent);
    border: none;
    border-radius: 6px;
    color: #fff;
    font-size: 0.95rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s, opacity 0.15s;
  }

  .btn-sync:hover:not(:disabled) { background: var(--accent-hover); }
  .btn-sync:disabled { opacity: 0.5; cursor: not-allowed; }

  /* ─── Review panel ──────────────────────────────────────────────────── */
  .review-panel {
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    overflow: hidden;
  }

  .review-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 16px;
    background: #1c2333;
    border-bottom: 1px solid var(--border);
  }

  .review-title {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text-primary);
    flex: 1;
  }

  .review-count {
    font-size: 0.75rem;
    background: var(--accent);
    color: #fff;
    padding: 2px 8px;
    border-radius: 10px;
    font-weight: 600;
  }

  .review-body {
    display: flex;
    gap: 0;
    min-height: 220px;
    max-height: 340px;
  }

  /* File tree — left column */
  .file-tree-wrap {
    flex: 1 1 0;
    overflow: hidden;
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
  }

  /* Commit pane — right column */
  .commit-pane {
    flex: 0 0 300px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px;
  }

  .commit-msg {
    width: 100%;
    background: var(--input-bg);
    border: 1px solid var(--border);
    border-radius: 5px;
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 0.82rem;
    line-height: 1.5;
    padding: 8px 10px;
    resize: vertical;
    box-sizing: border-box;
    transition: border-color 0.15s;
  }

  .commit-msg:focus { outline: none; border-color: var(--accent); }
  .commit-msg:disabled { opacity: 0.5; }

  .commit-hint {
    margin: 0;
    font-size: 0.72rem;
    color: var(--text-muted);
    font-style: italic;
  }

  .commit-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    margin-top: auto;
  }

  .btn-discard {
    padding: 8px 16px;
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text-secondary);
    font-size: 0.875rem;
    cursor: pointer;
    transition: border-color 0.15s, color 0.15s;
  }

  .btn-discard:hover:not(:disabled) { border-color: var(--color-error); color: var(--color-error); }
  .btn-discard:disabled { opacity: 0.5; cursor: not-allowed; }

  .btn-push {
    padding: 8px 18px;
    background: var(--accent);
    border: none;
    border-radius: 6px;
    color: #fff;
    font-size: 0.875rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s, opacity 0.15s;
  }

  .btn-push:hover:not(:disabled) { background: var(--accent-hover); }
  .btn-push:disabled { opacity: 0.5; cursor: not-allowed; }

  .review-empty-actions {
    padding: 14px 16px;
    display: flex;
    justify-content: flex-end;
  }

  /* Log */
  .log-section {
    height: 380px;
    display: flex;
    flex-direction: column;
  }
</style>
