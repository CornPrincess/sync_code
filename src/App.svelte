<script lang="ts">
  import { onMount } from 'svelte';
  import RepoPanel from './lib/components/RepoPanel.svelte';
  import LogViewer from './lib/components/LogViewer.svelte';
  import StatusBadge from './lib/components/StatusBadge.svelte';
  import { configStore } from './lib/stores/config.svelte.js';
  import { startSync, type SyncEvent } from './lib/ipc.js';

  type SyncStatus = 'idle' | 'syncing' | 'success' | 'error';

  let status = $state<SyncStatus>('idle');
  let lastSync = $state<string | undefined>(undefined);
  let errorMessage = $state<string | undefined>(undefined);
  let lines = $state<SyncEvent[]>([]);

  onMount(() => {
    configStore.load();
  });

  function validate(): string | null {
    const a = configStore.value.repo_a;
    const b = configStore.value.repo_b;
    if (!a.local_path.trim()) return 'Repo A: local path is required.';
    if (!a.branch.trim()) return 'Repo A: branch is required.';
    if (!b.local_path.trim()) return 'Repo B: local path is required.';
    if (!b.branch.trim()) return 'Repo B: branch is required.';
    return null;
  }

  async function handleSync() {
    errorMessage = undefined;
    const err = validate();
    if (err) {
      errorMessage = err;
      return;
    }

    lines = [];
    status = 'syncing';
    try {
      await startSync(configStore.value, (event) => {
        lines = [...lines, event];
      });
      status = 'success';
      lastSync = new Date().toLocaleTimeString();
    } catch (e: unknown) {
      status = 'error';
      errorMessage = typeof e === 'string' ? e : 'An unexpected error occurred.';
    }
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
        onchange={onConfigChange}
      />
      <div class="arrow" aria-hidden="true">←</div>
      <RepoPanel
        label="Repo B (source)"
        bind:config={configStore.value.repo_b}
        onchange={onConfigChange}
      />
    </section>

    <!-- Network proxy settings -->
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
                <input
                  type="text"
                  bind:value={configStore.value.proxy.http_proxy}
                  onchange={onConfigChange}
                  placeholder="http://proxy.example.com:8080"
                  class="input"
                />
              </label>
              <label class="field">
                <span class="field-label">HTTPS Proxy</span>
                <input
                  type="text"
                  bind:value={configStore.value.proxy.https_proxy}
                  onchange={onConfigChange}
                  placeholder="http://proxy.example.com:8080"
                  class="input"
                />
              </label>
              <label class="field">
                <span class="field-label">No Proxy (comma-separated hosts)</span>
                <input
                  type="text"
                  bind:value={configStore.value.proxy.no_proxy}
                  onchange={onConfigChange}
                  placeholder="localhost,127.0.0.1,.internal.example.com"
                  class="input"
                />
              </label>
            </div>
          {/if}
        </div>
      </details>
    </section>
  </div>

  <!-- Actions -->
  <section class="actions">
    {#if errorMessage}
      <div class="error-banner" role="alert">{errorMessage}</div>
    {/if}
    <div class="actions-row">
      <StatusBadge {status} {lastSync} />
      <button
        class="btn-sync"
        onclick={handleSync}
        disabled={status === 'syncing'}
      >
        {status === 'syncing' ? 'Syncing…' : 'Sync Now'}
      </button>
    </div>
  </section>

  <!-- Log output -->
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

  .app-header {
    flex-shrink: 0;
  }

  .app-title {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .app-subtitle {
    margin: 4px 0 0 0;
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  /* Config area: natural height, no overflow clipping */
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

  .proxy-summary::-webkit-details-marker {
    display: none;
  }

  .proxy-summary::before {
    content: '▶';
    font-size: 0.6rem;
    transition: transform 0.15s;
    color: var(--text-muted);
  }

  .proxy-details[open] .proxy-summary::before {
    transform: rotate(90deg);
  }

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

  .proxy-fields .field:last-child {
    grid-column: 1 / -1;
  }

  .checkbox-option {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.875rem;
    color: var(--text-primary);
    cursor: pointer;
  }

  .checkbox-option input[type='checkbox'] {
    accent-color: var(--accent);
    cursor: pointer;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

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

  .input:focus {
    outline: none;
    border-color: var(--accent);
  }

  /* Actions */
  .actions {
    flex-shrink: 0;
  }

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

  .btn-sync:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .btn-sync:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Log: fixed height with internal scroll */
  .log-section {
    height: 380px;
    display: flex;
    flex-direction: column;
  }
</style>
