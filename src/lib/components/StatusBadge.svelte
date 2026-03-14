<script lang="ts">
  let {
    status,
    lastSync
  }: {
    status: 'idle' | 'syncing' | 'success' | 'error';
    lastSync?: string;
  } = $props();

  const labels: Record<typeof status, string> = {
    idle: 'Ready',
    syncing: 'Syncing…',
    success: 'Sync OK',
    error: 'Sync Failed'
  };
</script>

<div class="status-bar">
  <span class="badge badge-{status}">{labels[status]}</span>
  {#if lastSync}
    <span class="last-sync">Last sync: {lastSync}</span>
  {/if}
</div>

<style>
  .status-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 0;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px;
    border-radius: 99px;
    font-size: 0.75rem;
    font-weight: 600;
  }

  .badge-idle {
    background: var(--surface);
    color: var(--text-secondary);
    border: 1px solid var(--border);
  }

  .badge-syncing {
    background: #1e3a5f;
    color: #60a5fa;
    border: 1px solid #2563eb44;
    animation: pulse 1.5s ease-in-out infinite;
  }

  .badge-success {
    background: #14532d44;
    color: var(--color-success);
    border: 1px solid #16a34a44;
  }

  .badge-error {
    background: #450a0a44;
    color: var(--color-error);
    border: 1px solid #dc262644;
  }

  .last-sync {
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.6; }
  }
</style>
