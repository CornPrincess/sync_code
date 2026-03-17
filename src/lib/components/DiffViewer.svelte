<script lang="ts">
  import type { FileChange } from '../ipc.js';

  let {
    file,
    diff = '',
    loading = false,
  }: {
    file: FileChange | null;
    diff: string;
    loading: boolean;
  } = $props();

  // ── Diff parsing ──────────────────────────────────────────────────────────
  type LineKind = 'add' | 'del' | 'ctx' | 'hunk' | 'file-header' | 'no-newline';

  interface DiffLine {
    kind: LineKind;
    text: string;
    oldNo: number | null;
    newNo: number | null;
  }

  function parseDiff(raw: string): DiffLine[] {
    if (!raw.trim()) return [];
    const lines = raw.split('\n');
    const result: DiffLine[] = [];
    let oldLine = 0;
    let newLine = 0;

    for (const line of lines) {
      if (line.startsWith('diff --git') || line.startsWith('index ') ||
          line.startsWith('--- ') || line.startsWith('+++ ')) {
        result.push({ kind: 'file-header', text: line, oldNo: null, newNo: null });
      } else if (line.startsWith('@@')) {
        // Extract starting line numbers from @@ -a,b +c,d @@
        const m = line.match(/^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
        if (m) {
          oldLine = parseInt(m[1], 10);
          newLine = parseInt(m[2], 10);
        }
        result.push({ kind: 'hunk', text: line, oldNo: null, newNo: null });
      } else if (line.startsWith('+')) {
        result.push({ kind: 'add', text: line.slice(1), oldNo: null, newNo: newLine++ });
      } else if (line.startsWith('-')) {
        result.push({ kind: 'del', text: line.slice(1), oldNo: oldLine++, newNo: null });
      } else if (line.startsWith('\\ No newline')) {
        result.push({ kind: 'no-newline', text: line, oldNo: null, newNo: null });
      } else {
        // context line
        result.push({ kind: 'ctx', text: line.slice(1), oldNo: oldLine++, newNo: newLine++ });
      }
    }
    return result;
  }

  let parsedLines = $derived(parseDiff(diff));

  // Is the diff non-empty but contains only file header lines (binary file)?
  let isBinary = $derived(
    diff.includes('Binary files') ||
    (diff.trim().length > 0 && parsedLines.every(l => l.kind === 'file-header'))
  );
</script>

<div class="diff-viewer">
  {#if !file}
    <div class="diff-empty">
      <span class="diff-empty-icon">≡</span>
      <span>Select a file to view its diff</span>
    </div>
  {:else if loading}
    <div class="diff-empty">
      <span class="diff-loading">Loading…</span>
    </div>
  {:else if !diff.trim()}
    {#if file.status === 'added'}
      <div class="diff-empty diff-notice">
        <span class="diff-status-badge badge-added">+ added</span>
        <span>New file — no diff (file will be added as-is)</span>
      </div>
    {:else if file.status === 'deleted'}
      <div class="diff-empty diff-notice">
        <span class="diff-status-badge badge-deleted">− deleted</span>
        <span>File will be removed</span>
      </div>
    {:else}
      <div class="diff-empty">No diff available</div>
    {/if}
  {:else if isBinary}
    <div class="diff-empty diff-notice">
      <span class="diff-status-badge badge-modified">~ binary</span>
      <span>Binary file changed</span>
    </div>
  {:else}
    <div class="diff-content" role="region" aria-label="Diff">
      <table class="diff-table">
        <tbody>
          {#each parsedLines as line (line)}
            {#if line.kind === 'file-header'}
              <!-- skip file header meta lines — shown in outer panel header -->
            {:else if line.kind === 'hunk'}
              <tr class="hunk-row">
                <td class="gutter" colspan="2"></td>
                <td class="hunk-header">{line.text}</td>
              </tr>
            {:else if line.kind === 'no-newline'}
              <tr class="ctx-row no-newline-row">
                <td class="gutter"></td>
                <td class="gutter"></td>
                <td class="code-cell no-newline-text">{line.text}</td>
              </tr>
            {:else}
              <tr class="diff-row diff-row-{line.kind}">
                <td class="gutter gutter-old">{line.oldNo ?? ''}</td>
                <td class="gutter gutter-new">{line.newNo ?? ''}</td>
                <td class="code-cell">
                  <span class="line-sign">{line.kind === 'add' ? '+' : line.kind === 'del' ? '-' : ' '}</span><span class="line-text">{line.text}</span>
                </td>
              </tr>
            {/if}
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  .diff-viewer {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    background: var(--bg);
    font-family: var(--font-mono);
    font-size: 12px;
  }

  /* ── Empty / notice states ── */
  .diff-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 8px;
    color: var(--text-muted);
    font-size: 0.8rem;
    font-family: var(--font-sans);
    user-select: none;
  }

  .diff-empty-icon {
    font-size: 2rem;
    opacity: 0.3;
  }

  .diff-loading { color: var(--text-secondary); }

  .diff-notice { gap: 6px; }

  .diff-status-badge {
    font-size: 0.72rem;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 10px;
    font-family: var(--font-mono);
  }

  .badge-added   { background: rgba(63,185,80,0.15); color: var(--color-success); }
  .badge-deleted { background: rgba(248,81,73,0.15);  color: var(--color-error); }
  .badge-modified{ background: rgba(121,192,255,0.15); color: #79c0ff; }

  /* ── Scrollable content ── */
  .diff-content {
    flex: 1;
    overflow: auto;
  }

  /* ── Table layout ── */
  .diff-table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }

  /* Gutters (line numbers) */
  .gutter {
    width: 44px;
    min-width: 44px;
    text-align: right;
    padding: 0 6px;
    color: var(--text-muted);
    background: #0d1117;
    border-right: 1px solid var(--border);
    user-select: none;
    font-size: 11px;
    white-space: nowrap;
    vertical-align: top;
    line-height: 1.6;
  }

  .gutter-old { border-right: none; }
  .gutter-new { border-right: 1px solid var(--border); }

  /* Code cell */
  .code-cell {
    padding: 0 8px;
    white-space: pre;
    overflow: visible;
    line-height: 1.6;
    vertical-align: top;
  }

  /* Sign character (+/-/ ) */
  .line-sign {
    display: inline-block;
    width: 12px;
    user-select: none;
    font-weight: 700;
  }

  /* ── Row colours (IDEA-style) ── */
  .diff-row-add {
    background: rgba(63, 185, 80, 0.10);
  }
  .diff-row-add .gutter {
    background: rgba(63, 185, 80, 0.14);
  }
  .diff-row-add .line-sign { color: #3fb950; }
  .diff-row-add .line-text { color: #b6f4c2; }

  .diff-row-del {
    background: rgba(248, 81, 73, 0.10);
  }
  .diff-row-del .gutter {
    background: rgba(248, 81, 73, 0.14);
  }
  .diff-row-del .line-sign { color: #f85149; }
  .diff-row-del .line-text { color: #ffd0ce; }

  .diff-row-ctx .line-sign { color: transparent; }
  .diff-row-ctx .line-text { color: var(--text-secondary); }

  /* ── Hunk header ── */
  .hunk-row {
    background: #1c2333;
  }

  .hunk-header {
    padding: 2px 8px;
    color: #58a6ff;
    font-size: 11px;
    font-style: italic;
    white-space: pre;
    line-height: 1.6;
  }

  /* ── No-newline indicator ── */
  .no-newline-row { background: transparent; }
  .no-newline-text {
    color: var(--text-muted);
    font-style: italic;
    padding-left: 20px;
  }
</style>
