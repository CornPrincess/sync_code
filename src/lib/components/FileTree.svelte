<script lang="ts">
  import type { FileChange } from '../ipc.js';

  // ── Types ──────────────────────────────────────────────────────────────────
  interface TreeNode {
    name: string;
    fullPath: string; // '' = root dir; 'src/components' = dir path; 'src/App.svelte' = file path
    isDir: boolean;
    children: TreeNode[];
    file?: FileChange;
  }

  // ── Props ──────────────────────────────────────────────────────────────────
  let {
    files = [],
    selected = $bindable(new Set<string>()),
    activeFile = null,
    onfileclick,
  }: {
    files: FileChange[];
    selected: Set<string>;
    activeFile?: string | null;
    onfileclick?: (file: FileChange) => void;
  } = $props();

  // ── Tree construction ──────────────────────────────────────────────────────
  function buildTree(files: FileChange[]): TreeNode[] {
    const root: TreeNode[] = [];
    // Sort so dirs appear before files and siblings are alphabetical
    const sorted = [...files].sort((a, b) => {
      const aDepth = a.path.split('/').length;
      const bDepth = b.path.split('/').length;
      if (aDepth !== bDepth) return bDepth - aDepth; // deeper first so dirs get created
      return a.path.localeCompare(b.path);
    });

    for (const file of sorted) {
      const parts = file.path.split('/');
      let nodes = root;
      let cumPath = '';

      // Create/find directory nodes
      for (let i = 0; i < parts.length - 1; i++) {
        cumPath = cumPath ? `${cumPath}/${parts[i]}` : parts[i];
        let dir = nodes.find((n) => n.isDir && n.name === parts[i]);
        if (!dir) {
          dir = { name: parts[i], fullPath: cumPath, isDir: true, children: [] };
          nodes.push(dir);
        }
        nodes = dir.children;
      }

      // Insert file node
      nodes.push({
        name: parts[parts.length - 1],
        fullPath: file.path,
        isDir: false,
        children: [],
        file,
      });
    }

    // Sort each level: dirs first, then files, each alphabetically
    function sortLevel(nodes: TreeNode[]) {
      nodes.sort((a, b) => {
        if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
        return a.name.localeCompare(b.name);
      });
      for (const n of nodes) if (n.isDir) sortLevel(n.children);
    }
    sortLevel(root);

    return root;
  }

  function collectDirPaths(nodes: TreeNode[]): string[] {
    const out: string[] = [];
    for (const n of nodes) {
      if (n.isDir) { out.push(n.fullPath); out.push(...collectDirPaths(n.children)); }
    }
    return out;
  }

  function getFilePaths(node: TreeNode): string[] {
    if (!node.isDir) return node.file ? [node.fullPath] : [];
    return node.children.flatMap(getFilePaths);
  }

  // ── Reactive state ─────────────────────────────────────────────────────────
  let tree = $derived(buildTree(files));

  // Open all dirs whenever files change
  let openDirs = $state(new Set<string>());
  $effect(() => { openDirs = new Set(collectDirPaths(tree)); });

  // Header checkbox state
  let allSelected = $derived(files.length > 0 && files.every((f) => selected.has(f.path)));
  let someSelected = $derived(!allSelected && files.some((f) => selected.has(f.path)));

  // ── Interactions ───────────────────────────────────────────────────────────
  function toggleAll() {
    selected = allSelected ? new Set() : new Set(files.map((f) => f.path));
  }

  function getDirState(node: TreeNode): 'all' | 'some' | 'none' {
    const paths = getFilePaths(node);
    if (!paths.length) return 'none';
    const n = paths.filter((p) => selected.has(p)).length;
    return n === 0 ? 'none' : n === paths.length ? 'all' : 'some';
  }

  function toggleDir(node: TreeNode) {
    const paths = getFilePaths(node);
    if (getDirState(node) === 'all') {
      selected = new Set([...selected].filter((p) => !paths.includes(p)));
    } else {
      selected = new Set([...selected, ...paths]);
    }
  }

  function toggleFile(path: string) {
    if (selected.has(path)) {
      selected = new Set([...selected].filter((p) => p !== path));
    } else {
      selected = new Set([...selected, path]);
    }
  }

  function toggleDirOpen(fullPath: string) {
    if (openDirs.has(fullPath)) {
      openDirs = new Set([...openDirs].filter((p) => p !== fullPath));
    } else {
      openDirs = new Set([...openDirs, fullPath]);
    }
  }

  // ── Helpers ────────────────────────────────────────────────────────────────
  /** Svelte action: keeps `input.indeterminate` in sync */
  function indeterminate(el: HTMLInputElement, value: boolean) {
    el.indeterminate = value;
    return { update: (v: boolean) => { el.indeterminate = v; } };
  }

  const STATUS_ICON: Record<string, string> = {
    added: '+', modified: '~', deleted: '−', renamed: '→', copied: '⊕', unknown: '?',
  };
</script>

<!-- ── Template ──────────────────────────────────────────────────────────── -->
<div class="file-tree">
  <!-- Header: select-all checkbox + summary -->
  <div class="tree-header">
    <label class="cb-wrap">
      <input
        type="checkbox"
        checked={allSelected}
        use:indeterminate={someSelected}
        onchange={toggleAll}
        class="cb"
      />
    </label>
    <span class="header-label">
      {files.length} file{files.length !== 1 ? 's' : ''}
      <span class="header-sel">
        {#if selected.size === 0}
          — none selected
        {:else if selected.size === files.length}
          — all selected
        {:else}
          — {selected.size} selected
        {/if}
      </span>
    </span>
  </div>

  <!-- Tree body -->
  <div class="tree-body">
    {#each tree as node}
      {@render renderNode(node, 0)}
    {/each}
  </div>
</div>

<!-- ── Recursive snippet ──────────────────────────────────────────────────── -->
{#snippet renderNode(node: TreeNode, depth: number)}
  {#if node.isDir}
    {@const ds = getDirState(node)}
    <div class="tree-row dir-row" style:padding-left="{depth * 18 + 6}px">
      <label class="cb-wrap">
        <input
          type="checkbox"
          checked={ds === 'all'}
          use:indeterminate={ds === 'some'}
          onchange={() => toggleDir(node)}
          class="cb"
        />
      </label>
      <button class="dir-btn" onclick={() => toggleDirOpen(node.fullPath)}>
        <span class="dir-arrow">{openDirs.has(node.fullPath) ? '▾' : '▸'}</span>
        <span class="dir-name">{node.name}/</span>
      </button>
    </div>
    {#if openDirs.has(node.fullPath)}
      {#each node.children as child}
        {@render renderNode(child, depth + 1)}
      {/each}
    {/if}
  {:else if node.file}
    {@const isActive = activeFile === node.fullPath}
    <div
      class="tree-row file-row file-{node.file.status}"
      class:file-active={isActive}
      style:padding-left="{depth * 18 + 6}px"
      role="button"
      tabindex="0"
      onclick={() => onfileclick?.(node.file!)}
      onkeydown={(e) => e.key === 'Enter' && onfileclick?.(node.file!)}
    >
      <label class="cb-wrap">
        <input
          type="checkbox"
          checked={selected.has(node.fullPath)}
          onchange={() => toggleFile(node.fullPath)}
          onclick={(e) => e.stopPropagation()}
          class="cb"
        />
      </label>
      <span class="file-icon" title={node.file.status}>
        {STATUS_ICON[node.file.status] ?? '?'}
      </span>
      <span class="file-name">{node.name}</span>
      {#if node.file.old_path}
        <span class="file-old">← {node.file.old_path.split('/').pop()}</span>
      {/if}
    </div>
  {/if}
{/snippet}

<style>
  .file-tree {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  /* Header */
  .tree-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px 6px 6px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    background: rgba(255,255,255,0.02);
  }

  .header-label {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--text-secondary);
    user-select: none;
  }

  .header-sel {
    font-weight: 400;
    color: var(--text-muted);
  }

  /* Tree body */
  .tree-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px 0;
  }

  /* Rows */
  .tree-row {
    display: flex;
    align-items: center;
    gap: 4px;
    min-height: 24px;
    padding-top: 1px;
    padding-bottom: 1px;
    transition: background 0.08s;
  }

  .tree-row:hover { background: rgba(255,255,255,0.04); }
  .file-row { cursor: pointer; }
  .file-active { background: rgba(35, 134, 54, 0.15) !important; }
  .file-active:hover { background: rgba(35, 134, 54, 0.22) !important; }

  /* Checkbox — hide native, draw custom */
  .cb-wrap {
    display: flex;
    align-items: center;
    flex-shrink: 0;
    cursor: pointer;
  }

  .cb {
    /* hide the native checkbox but keep it in the a11y tree */
    appearance: none;
    -webkit-appearance: none;
    position: relative;
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    cursor: pointer;

    /* box */
    background: var(--input-bg);
    border: 1px solid var(--border);
    border-radius: 3px;
    transition: border-color 0.12s, background 0.12s, box-shadow 0.12s;
    vertical-align: middle;
  }

  /* hover */
  .cb:hover {
    border-color: var(--accent-hover);
  }

  /* checked — filled green */
  .cb:checked {
    background: var(--accent);
    border-color: var(--accent);
  }

  /* checkmark via pseudo-element */
  .cb:checked::after {
    content: '';
    position: absolute;
    left: 3px;
    top: 1px;
    width: 5px;
    height: 8px;
    border: 2px solid #fff;
    border-top: none;
    border-left: none;
    transform: rotate(45deg);
  }

  /* indeterminate — dash */
  .cb:indeterminate {
    background: var(--surface);
    border-color: var(--accent);
  }

  .cb:indeterminate::after {
    content: '';
    position: absolute;
    left: 3px;
    top: 5px;
    width: 6px;
    height: 2px;
    background: var(--accent);
    border-radius: 1px;
  }

  /* focus ring */
  .cb:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px rgba(35, 134, 54, 0.4);
  }

  /* Directory rows */
  .dir-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 0.8rem;
  }

  .dir-btn:hover { color: var(--text-primary); }

  .dir-arrow {
    font-size: 0.65rem;
    width: 10px;
    text-align: center;
    flex-shrink: 0;
    color: var(--text-muted);
  }

  .dir-name { font-weight: 500; }

  /* File rows */
  .file-icon {
    font-size: 0.8rem;
    font-weight: 700;
    width: 14px;
    text-align: center;
    flex-shrink: 0;
    font-family: var(--font-mono);
    user-select: none;
  }

  .file-name {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-old {
    font-family: var(--font-mono);
    font-size: 0.7rem;
    color: var(--text-muted);
    flex-shrink: 0;
    white-space: nowrap;
  }

  /* Status colours */
  .file-added   .file-icon, .file-added   .file-name { color: var(--color-success); }
  .file-modified .file-icon                           { color: #79c0ff; }
  .file-modified .file-name                           { color: var(--text-primary); }
  .file-deleted .file-icon, .file-deleted .file-name  { color: var(--color-error); text-decoration: line-through; }
  .file-renamed .file-icon                            { color: var(--color-warn); }
  .file-renamed .file-name                            { color: var(--color-warn); }
  .file-copied  .file-icon, .file-copied  .file-name  { color: #a371f7; }
  .file-unknown .file-icon, .file-unknown .file-name  { color: var(--text-muted); }
</style>
