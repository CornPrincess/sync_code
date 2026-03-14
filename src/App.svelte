<script lang="ts">
  import { onMount } from 'svelte';
  import RepoPanel from './lib/components/RepoPanel.svelte';
  import LogViewer from './lib/components/LogViewer.svelte';
  import StatusBadge from './lib/components/StatusBadge.svelte';
  import { configStore } from './lib/stores/config.svelte.js';
  import { startSync } from './lib/ipc.js';

  type SyncStatus = 'idle' | 'syncing' | 'success' | 'error';

  let status = $state<SyncStatus>('idle');
  let lastSync = $state<string | undefined>(undefined);
  let errorMessage = $state<string | undefined>(undefined);

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

    status = 'syncing';
    try {
      await startSync(configStore.value);
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

  <section class="log-section">
    <LogViewer />
  </section>
</main>

<style>
  .app {
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 24px;
    height: 100vh;
    box-sizing: border-box;
    overflow: hidden;
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

  .repo-grid {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    flex-shrink: 0;
  }

  .arrow {
    font-size: 1.5rem;
    color: var(--text-muted);
    padding-top: 44px;
    flex-shrink: 0;
  }

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

  .log-section {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
</style>
