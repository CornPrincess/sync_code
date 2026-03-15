<script lang="ts">
  import { tick } from 'svelte';
  import type { SyncEvent } from '../ipc.js';

  let {
    lines = $bindable<SyncEvent[]>([]),
  }: { lines: SyncEvent[] } = $props();

  let container: HTMLElement;

  // Auto-scroll whenever lines array grows
  $effect(() => {
    if (lines.length > 0) {
      tick().then(() => {
        container?.scrollTo({ top: container.scrollHeight, behavior: 'smooth' });
      });
    }
  });

  function clear() {
    lines = [];
  }

  async function copyToClipboard() {
    const text = lines
      .map((l) => `[${l.timestamp}] [${l.level.padEnd(7)}] ${l.message}`)
      .join('\n');
    await navigator.clipboard.writeText(text);
  }
</script>

<div class="log-wrapper">
  <div class="log-header">
    <span class="log-title">Output</span>
    <div class="log-actions">
      <button type="button" onclick={copyToClipboard} class="btn-action" title="Copy to clipboard">
        Copy
      </button>
      <button type="button" onclick={clear} class="btn-action" title="Clear log">Clear</button>
    </div>
  </div>
  <div class="log-body" bind:this={container}>
    {#if lines.length === 0}
      <span class="log-empty">No output yet. Press "Sync Now" to start.</span>
    {:else}
      {#each lines as line, i (i)}
        <div class="log-line log-{line.level}">
          <span class="log-ts">{line.timestamp}</span>
          <span class="log-prefix">[{line.level}]</span>
          <span class="log-msg">{line.message}</span>
        </div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .log-wrapper {
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .log-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 12px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .log-title {
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .log-actions {
    display: flex;
    gap: 8px;
  }

  .btn-action {
    padding: 3px 10px;
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-secondary);
    font-size: 0.75rem;
    cursor: pointer;
    transition: color 0.15s, border-color 0.15s;
  }

  .btn-action:hover {
    color: var(--text-primary);
    border-color: var(--text-secondary);
  }

  .log-body {
    flex: 1;
    overflow-y: auto;
    padding: 12px;
    background: var(--log-bg);
    font-family: var(--font-mono);
    font-size: 0.8rem;
    line-height: 1.6;
  }

  .log-empty {
    color: var(--text-muted);
    font-style: italic;
  }

  .log-line {
    display: flex;
    gap: 8px;
    padding: 1px 0;
  }

  .log-ts {
    color: var(--text-muted);
    flex-shrink: 0;
    user-select: none;
  }

  .log-prefix {
    opacity: 0.5;
    user-select: none;
    flex-shrink: 0;
    min-width: 52px; /* [success] is widest */
  }

  .log-info .log-msg    { color: var(--text-primary); }
  .log-warn .log-msg    { color: var(--color-warn); }
  .log-error .log-msg   { color: var(--color-error); }
  .log-success .log-msg { color: var(--color-success); font-weight: 600; }
</style>
