<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { isTauri, listBranches, refreshBranches, checkoutAndPull, type BranchList, type RepoConfig, type ProxyConfig } from '../ipc.js';

  const isTauriCtx = isTauri();

  let {
    label,
    config = $bindable(),
    proxy,
    onchange,
    showZipOption = false,
  }: {
    label: string;
    config: RepoConfig;
    proxy: ProxyConfig;
    onchange?: () => void;
    /** If true, show a "Use ZIP file" toggle for this repo panel (Repo B only). */
    showZipOption?: boolean;
  } = $props();

  // ── Branch combobox state ──────────────────────────────────────────────────
  let branchList = $state<BranchList>({ local: [], remote: [] });
  let loadingBranches = $state(false);
  let refreshingBranches = $state(false);
  let checkingOut = $state(false);
  let branchError = $state('');
  let dropdownOpen = $state(false);
  let activeIdx = $state(-1);   // flat index across local + remote

  // Only filter while the user is actively typing — not on initial open.
  // Without this, opening the dropdown with branch="main" would hide all
  // remote branches that don't contain "main".
  let userTyping = $state(false);
  const q = $derived(dropdownOpen && userTyping ? config.branch.toLowerCase() : '');
  const filteredLocal  = $derived(q ? branchList.local.filter(b  => b.toLowerCase().includes(q)) : branchList.local);
  const filteredRemote = $derived(q ? branchList.remote.filter(b => b.toLowerCase().includes(q)) : branchList.remote);
  const totalCount     = $derived(filteredLocal.length + filteredRemote.length);

  // Flat list for keyboard navigation
  const flatBranches = $derived([...filteredLocal, ...filteredRemote]);

  // Fast load from cached git refs — no network call
  async function fetchBranches(path: string) {
    if (!path) { branchList = { local: [], remote: [] }; return; }
    loadingBranches = true;
    try {
      branchList = await listBranches(path);
    } catch {
      branchList = { local: [], remote: [] };
    } finally {
      loadingBranches = false;
    }
  }

  // Network refresh — runs git fetch with proxy then re-reads refs
  async function doRefreshBranches() {
    if (!config.local_path || refreshingBranches) return;
    refreshingBranches = true;
    try {
      branchList = await refreshBranches(config.local_path, proxy);
    } catch {
      // keep existing list on failure
    } finally {
      refreshingBranches = false;
    }
  }

  // Load cached refs whenever the repo path changes (fast, no network)
  $effect(() => { fetchBranches(config.local_path); });

  // Track what the branch value was when the input gained focus, so we can
  // detect manual edits on blur and trigger a checkout.
  let focusedBranch = '';

  function openDropdown() {
    dropdownOpen = true;
    userTyping = false; // show all branches unfiltered on open
    activeIdx = -1;
    branchError = '';
  }

  function onBranchFocus() {
    focusedBranch = config.branch;
    openDropdown();
  }

  function onBranchInput() {
    dropdownOpen = true;
    userTyping = true; // user started typing — enable filter
    activeIdx = -1;
    branchError = '';
  }

  function onBranchKeydown(e: KeyboardEvent) {
    if (!dropdownOpen) { if (e.key === 'ArrowDown') openDropdown(); return; }
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      activeIdx = Math.min(activeIdx + 1, totalCount - 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      activeIdx = Math.max(activeIdx - 1, 0);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (activeIdx >= 0) {
        selectBranch(flatBranches[activeIdx]);
      } else {
        // User pressed Enter on manually-typed text → treat as branch selection
        const typed = config.branch.trim();
        if (typed) selectBranch(typed);
        else dropdownOpen = false;
      }
    } else if (e.key === 'Escape') {
      dropdownOpen = false;
    }
  }

  function onBranchBlur() {
    setTimeout(() => {
      dropdownOpen = false;
      // If the user manually edited the branch name (didn't pick from dropdown),
      // trigger a checkout. selectBranch() updates focusedBranch so a prior
      // dropdown click won't fire a second checkout here.
      const typed = config.branch.trim();
      if (typed && typed !== focusedBranch) {
        selectBranch(typed);
      }
    }, 150);
  }

  function toggleDropdown(e: MouseEvent) {
    e.preventDefault(); // keep input focused
    if (dropdownOpen) { dropdownOpen = false; }
    else openDropdown();
  }

  async function selectBranch(branch: string) {
    dropdownOpen = false;
    userTyping = false;
    config.branch = branch;
    focusedBranch = branch; // prevent onBranchBlur from triggering a duplicate checkout
    branchError = '';
    if (!config.local_path) { onchange?.(); return; }
    checkingOut = true;
    try {
      await checkoutAndPull(config.local_path, branch, config.remote_url, config.auth, proxy, config.platform ?? 'github');
      onchange?.();
    } catch (e) {
      branchError = String(e).replace(/^(Git error:|error:)\s*/i, '').trim();
    } finally {
      checkingOut = false;
    }
  }

  // ── File pickers ───────────────────────────────────────────────────────────
  async function browseFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === 'string') { config.local_path = selected; onchange?.(); }
  }

  async function browseSSHKey() {
    const selected = await open({ directory: false, multiple: false });
    if (typeof selected === 'string') { config.auth.ssh_key_path = selected; onchange?.(); }
  }

  async function browseZipFile() {
    const selected = await open({ directory: false, multiple: false, filters: [{ name: 'ZIP Archive', extensions: ['zip'] }] });
    if (typeof selected === 'string') { config.zip_path = selected; onchange?.(); }
  }
</script>

<div class="repo-panel">
  <h2 class="panel-title">{label}</h2>

  <!-- ZIP source toggle (Repo B only) -->
  {#if showZipOption}
    <label class="field zip-toggle-field">
      <input
        type="checkbox"
        class="zip-checkbox"
        bind:checked={config.use_zip}
        onchange={onchange}
      />
      <span class="zip-toggle-label">使用 ZIP 包作为源（无法通过 git 获取时）</span>
    </label>
  {/if}

  {#if showZipOption && config.use_zip}
    <!-- ZIP mode: show only the zip file path -->
    <label class="field">
      <span class="field-label">ZIP 文件路径</span>
      <div class="path-row">
        <input
          type="text"
          bind:value={config.zip_path}
          onchange={onchange}
          placeholder="/path/to/repo.zip"
          class="input"
        />
        {#if isTauriCtx}
          <button type="button" class="btn-browse" onclick={browseZipFile}>Browse</button>
        {/if}
      </div>
    </label>
    <p class="zip-note">
      下载仓库的 ZIP 包（如 GitHub → Code → Download ZIP），选择后自动解压并同步到 Repo A。
      解压时自动处理顶层包裹目录（如 <code>repo-main/</code>）。
    </p>
  {:else}

  <!-- Local Path -->
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
      {#if isTauriCtx}<button type="button" class="btn-browse" onclick={browseFolder}>Browse</button>{/if}
    </div>
  </label>

  <!-- Platform -->
  <label class="field">
    <span class="field-label">Platform</span>
    <div class="platform-row">
      {#each [
        { value: 'github',  label: 'GitHub',        logo: 'github'  },
        { value: 'gitlab',  label: 'GitLab',        logo: 'gitlab'  },
        { value: 'codeup',  label: 'Codeup (阿里云)', logo: 'codeup'  },
      ] as p}
        <button
          type="button"
          class="platform-btn"
          class:selected={config.platform === p.value}
          onclick={() => { config.platform = p.value; onchange?.(); }}
        >
          {#if p.logo === 'github'}
            <svg class="platform-icon" viewBox="0 0 16 16" fill="currentColor">
              <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/>
            </svg>
          {:else if p.logo === 'gitlab'}
            <svg class="platform-icon" viewBox="0 0 16 16" fill="currentColor">
              <path d="M15.97 9.058l-.895-2.756L13.3.842a.37.37 0 00-.702 0L10.821 6.3H5.18L3.403.842a.37.37 0 00-.702 0L.925 6.302.03 9.058a.693.693 0 00.252.775L8 15.233l7.718-5.4a.693.693 0 00.252-.775"/>
            </svg>
          {:else}
            <svg class="platform-icon" viewBox="0 0 16 16" fill="currentColor">
              <path d="M8 1a7 7 0 100 14A7 7 0 008 1zM0 8a8 8 0 1116 0A8 8 0 010 8z"/>
              <path d="M5.5 6.5A1.5 1.5 0 017 5h2a1.5 1.5 0 011.5 1.5v.5H7.5V6.5a.5.5 0 00-.5-.5H7a.5.5 0 00-.5.5V7H5.5v-.5zM5 8h6v1.5A1.5 1.5 0 019.5 11h-3A1.5 1.5 0 015 9.5V8z"/>
            </svg>
          {/if}
          <span>{p.label}</span>
        </button>
      {/each}
    </div>
  </label>

  <!-- Remote URL -->
  <label class="field">
    <span class="field-label">Remote URL</span>
    <input
      type="text"
      bind:value={config.remote_url}
      onchange={onchange}
      placeholder={config.platform === 'gitlab'
        ? 'https://gitlab.com/user/repo.git'
        : config.platform === 'codeup'
        ? 'https://codeup.aliyun.com/org/repo.git'
        : 'https://github.com/user/repo.git'}
      class="input"
    />
  </label>

  <!-- Branch combobox -->
  <div class="field">
    <span class="field-label">
      Branch
      {#if loadingBranches}
        <span class="branch-hint">loading…</span>
      {:else if refreshingBranches}
        <span class="branch-hint">refreshing…</span>
      {:else if checkingOut}
        <span class="branch-hint">checking out…</span>
      {:else if branchList.local.length + branchList.remote.length > 0}
        <span class="branch-count">
          {branchList.local.length}L · {branchList.remote.length}R
        </span>
      {/if}
    </span>

    <div class="branch-wrap">
      <!-- Text input -->
      <input
        type="text"
        bind:value={config.branch}
        onfocus={onBranchFocus}
        oninput={onBranchInput}
        onkeydown={onBranchKeydown}
        onblur={onBranchBlur}
        placeholder="main"
        class="input branch-input"
        class:input-error={!!branchError}
        autocomplete="off"
        spellcheck="false"
      />
      <!-- Arrow toggle button -->
      <button
        type="button"
        class="branch-arrow"
        class:open={dropdownOpen}
        onmousedown={toggleDropdown}
        tabindex="-1"
        aria-label="Toggle branch list"
      >
        <svg width="10" height="6" viewBox="0 0 10 6" fill="none">
          <path d="M1 1L5 5L9 1" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      </button>

      <!-- Dropdown -->
      {#if dropdownOpen}
        <div class="branch-dropdown">
          <!-- Refresh toolbar -->
          <div class="branch-toolbar">
            <button
              type="button"
              class="branch-refresh-btn"
              class:spinning={refreshingBranches}
              onmousedown={(e) => { e.preventDefault(); doRefreshBranches(); }}
              disabled={refreshingBranches}
              title="Fetch remote branches"
            >
              <svg width="11" height="11" viewBox="0 0 16 16" fill="currentColor">
                <path d="M8 3a5 5 0 1 0 4.546 2.914.5.5 0 0 1 .908-.417A6 6 0 1 1 8 2v1z"/>
                <path d="M8 4.466V.534a.25.25 0 0 1 .41-.192l2.36 1.966c.12.1.12.284 0 .384L8.41 4.658A.25.25 0 0 1 8 4.466z"/>
              </svg>
              {refreshingBranches ? 'Refreshing…' : 'Refresh remote'}
            </button>
          </div>
          {#if loadingBranches}
            <div class="branch-loading-row">Loading…</div>
          {:else if totalCount === 0}
            <div class="branch-empty">No branches found — click Refresh to fetch from remote</div>
          {:else}
            <!-- LOCAL section -->
            {#if filteredLocal.length > 0}
              <div class="branch-section-header">
                <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor">
                  <path d="M11.75 2.5a.75.75 0 1 0 1.5 0 .75.75 0 0 0-1.5 0zm.75 2.25a2.25 2.25 0 1 1-1.5-2.122V6A2.5 2.5 0 0 1 8.5 8.5H5.06a2.25 2.25 0 1 1 0-1.5H8.5A1 1 0 0 0 9.5 6V4.628A2.25 2.25 0 0 1 12.5 4.75zM4.25 13.5a.75.75 0 1 0 1.5 0 .75.75 0 0 0-1.5 0zm.75-2.25a2.25 2.25 0 1 1 0 4.5 2.25 2.25 0 0 1 0-4.5z"/>
                </svg>
                LOCAL BRANCHES
              </div>
              {#each filteredLocal as b, i}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div
                  class="branch-option"
                  class:active={i === activeIdx}
                  class:current={b === config.branch}
                  onmousedown={() => selectBranch(b)}
                >
                  <span class="branch-icon">⎇</span>
                  <span class="branch-name">{b}</span>
                  {#if b === config.branch}
                    <span class="branch-current-mark">✓</span>
                  {/if}
                </div>
              {/each}
            {/if}

            <!-- REMOTE section -->
            {#if filteredRemote.length > 0}
              {#if filteredLocal.length > 0}
                <div class="branch-divider"></div>
              {/if}
              <div class="branch-section-header remote-header">
                <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor">
                  <path d="M8 0C3.58 0 0 3.58 0 8c0 4.42 3.58 8 8 8s8-3.58 8-8c0-4.42-3.58-8-8-8zm3.67 10.17c-.28.28-.66.44-1.06.44H9.5v1.25a.75.75 0 0 1-1.5 0V10.5H5.39a1.5 1.5 0 0 1-1.06-2.56L7.47 4.8a.75.75 0 0 1 1.06 0l3.14 3.14c.28.29.44.67.44 1.07 0 .4-.16.78-.44 1.06z"/>
                </svg>
                REMOTE BRANCHES
              </div>
              {#each filteredRemote as b, i}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div
                  class="branch-option remote"
                  class:active={filteredLocal.length + i === activeIdx}
                  onmousedown={() => selectBranch(b)}
                >
                  <span class="branch-icon remote-icon">⇅</span>
                  <span class="branch-name">{b}</span>
                </div>
              {/each}
            {/if}
          {/if}
        </div>
      {/if}
    </div>

    {#if branchError}
      <span class="branch-error">{branchError}</span>
    {/if}
  </div>

  <!-- Authentication (git mode only) -->
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
      <label class="field">
        <span class="field-label">Auth Type</span>
        <select class="input select" bind:value={config.auth.auth_type} onchange={onchange}>
          <option value="none">None (system credential helper)</option>
          <option value="userpass">Username / Password</option>
          <option value="token">Access Token</option>
          <option value="ssh">SSH Key</option>
        </select>
      </label>

      {#if config.auth.auth_type === 'userpass'}
        <label class="field">
          <span class="field-label">Username</span>
          <input type="text" bind:value={config.auth.username} onchange={onchange}
            placeholder="your git username" class="input" autocomplete="off" />
        </label>
        <label class="field">
          <span class="field-label">Password</span>
          <input type="password" bind:value={config.auth.password} onchange={onchange}
            placeholder="account password" class="input" autocomplete="off" />
        </label>
        <p class="auth-note">⚠ Stored in plain text. Use "Access Token" mode for better security.</p>
      {/if}

      {#if config.auth.auth_type === 'token'}
        {#if config.platform === 'codeup'}
          <!-- Codeup requires username + token (basic auth format: username:token) -->
          <label class="field">
            <span class="field-label">Username</span>
            <input type="text" bind:value={config.auth.username} onchange={onchange}
              placeholder="your Codeup username" class="input" autocomplete="off" />
          </label>
          <label class="field">
            <span class="field-label">Personal Access Token</span>
            <input type="password" bind:value={config.auth.token} onchange={onchange}
              placeholder="your Codeup personal access token" class="input" autocomplete="off" />
          </label>
          <p class="auth-note">
            Yunxiao → 个人中心 → 个人访问令牌。
            格式：<code>&lt;username&gt;:&lt;token&gt;</code>。⚠ 明文存储。
          </p>
        {:else if config.platform === 'gitlab'}
          <label class="field">
            <span class="field-label">Personal Access Token</span>
            <input type="password" bind:value={config.auth.token} onchange={onchange}
              placeholder="glpat-xxxxxxxxxxxxxxxxxxxx" class="input" autocomplete="off" />
          </label>
          <p class="auth-note">
            GitLab → User Settings → Access Tokens（需要 <code>read_repository</code> + <code>write_repository</code> 权限）。
            格式：<code>oauth2:&lt;token&gt;</code>。⚠ 明文存储。
          </p>
        {:else}
          <!-- GitHub -->
          <label class="field">
            <span class="field-label">Personal Access Token</span>
            <input type="password" bind:value={config.auth.token} onchange={onchange}
              placeholder="ghp_xxxxxxxxxxxxxxxxxxxx" class="input" autocomplete="off" />
          </label>
          <p class="auth-note">
            GitHub → Settings → Developer settings → Personal access tokens（需要 <code>repo</code> 权限）。
            格式：<code>oauth2:&lt;token&gt;</code>。⚠ 明文存储。
          </p>
        {/if}
      {/if}

      {#if config.auth.auth_type === 'ssh'}
        <label class="field">
          <span class="field-label">SSH Private Key Path</span>
          <div class="path-row">
            <input type="text" bind:value={config.auth.ssh_key_path} onchange={onchange}
              placeholder="~/.ssh/id_ed25519" class="input" />
            {#if isTauriCtx}<button type="button" class="btn-browse" onclick={browseSSHKey}>Browse</button>{/if}
          </div>
        </label>
        <p class="auth-note">Uses GIT_SSH_COMMAND with StrictHostKeyChecking=accept-new.</p>
      {/if}
    </div>
  </details>

  {/if}<!-- end zip/git conditional -->
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
    display: flex;
    align-items: center;
    gap: 6px;
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

  .path-row .input { flex: 1; }

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

  .input-error { border-color: var(--color-error) !important; }

  /* ── Branch status labels ────────────────────────────────────────────────── */
  .branch-hint {
    font-size: 0.72rem;
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
    padding: 0 6px;
    letter-spacing: 0;
    text-transform: none;
  }

  /* ── Platform selector ───────────────────────────────────────────────────── */
  .platform-row {
    display: flex;
    gap: 6px;
  }

  .platform-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    background: var(--input-bg);
    border: 1px solid var(--border);
    border-radius: 5px;
    color: var(--text-secondary);
    font-size: 0.78rem;
    cursor: pointer;
    flex: 1;
    justify-content: center;
    transition: border-color 0.15s, color 0.15s, background 0.15s;
  }

  .platform-btn:hover {
    border-color: var(--accent);
    color: var(--text-primary);
  }

  .platform-btn.selected {
    border-color: var(--accent);
    background: rgba(35, 134, 54, 0.12);
    color: var(--accent-hover);
    font-weight: 600;
  }

  .platform-icon {
    width: 13px;
    height: 13px;
    flex-shrink: 0;
  }

  /* ── Branch combobox container ───────────────────────────────────────────── */
  .branch-wrap {
    position: relative;
    max-width: 280px;
    display: flex;
    align-items: stretch;
  }

  .branch-input {
    flex: 1;
    border-radius: 5px 0 0 5px;
    border-right: none;
    min-width: 0;
  }

  /* Arrow button — attached to the right of the input */
  .branch-arrow {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    flex-shrink: 0;
    background: var(--btn-secondary-bg);
    border: 1px solid var(--border);
    border-left: none;
    border-radius: 0 5px 5px 0;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0;
    transition: background 0.12s, color 0.12s;
  }

  .branch-arrow:hover { background: var(--btn-secondary-hover); color: var(--text-primary); }

  .branch-arrow svg {
    transition: transform 0.15s;
  }
  .branch-arrow.open svg { transform: rotate(180deg); }

  /* Shared focus glow: when input is focused, also highlight the arrow border */
  .branch-wrap:focus-within .branch-arrow {
    border-color: var(--accent);
  }
  .branch-wrap:focus-within .branch-input {
    border-color: var(--accent);
  }

  /* ── Dropdown panel ──────────────────────────────────────────────────────── */
  .branch-dropdown {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
    max-height: 220px;
    overflow-y: auto;
    z-index: 200;
  }

  /* Section headers (LOCAL / REMOTE) */
  .branch-section-header {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 6px 10px 4px;
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
    user-select: none;
    position: sticky;
    top: 0;
    background: var(--surface);
  }

  .remote-header { color: #79c0ff; }
  .remote-header svg { fill: #79c0ff; }

  .branch-divider {
    height: 1px;
    background: var(--border);
    margin: 2px 0;
  }

  /* Branch rows */
  .branch-option {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px 5px 14px;
    font-size: 0.82rem;
    font-family: var(--font-mono);
    color: var(--text-primary);
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: background 0.07s;
  }

  .branch-option:hover,
  .branch-option.active {
    background: rgba(35, 134, 54, 0.15);
  }

  .branch-option.current {
    color: var(--accent-hover);
    font-weight: 600;
  }

  .branch-option.remote .branch-name { color: #79c0ff; }
  .branch-option.remote:hover .branch-name,
  .branch-option.remote.active .branch-name { color: #a5d6ff; }

  .branch-icon {
    font-size: 0.75rem;
    color: var(--text-muted);
    flex-shrink: 0;
    width: 14px;
    text-align: center;
  }

  .remote-icon { color: #4d9cf8; }

  .branch-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .branch-current-mark {
    font-size: 0.75rem;
    color: var(--accent-hover);
    flex-shrink: 0;
  }

  /* Refresh toolbar at top of dropdown */
  .branch-toolbar {
    display: flex;
    align-items: center;
    padding: 4px 6px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    position: sticky;
    top: 0;
    z-index: 1;
  }

  .branch-refresh-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 3px 8px;
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-muted);
    font-size: 0.72rem;
    cursor: pointer;
    transition: color 0.12s, border-color 0.12s, background 0.12s;
  }

  .branch-refresh-btn:hover:not(:disabled) {
    color: var(--text-primary);
    border-color: var(--accent);
    background: rgba(35, 134, 54, 0.1);
  }

  .branch-refresh-btn:disabled { opacity: 0.5; cursor: not-allowed; }

  .branch-refresh-btn.spinning svg {
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  .branch-loading-row,
  .branch-empty {
    padding: 10px 14px;
    font-size: 0.8rem;
    color: var(--text-muted);
    font-style: italic;
  }

  .branch-error {
    font-size: 0.75rem;
    color: var(--color-error);
    line-height: 1.4;
  }

  /* ── Buttons ─────────────────────────────────────────────────────────────── */
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

  .btn-browse:hover { background: var(--btn-secondary-hover); }

  /* ── Authentication section ──────────────────────────────────────────────── */
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

  .auth-summary::-webkit-details-marker { display: none; }

  .auth-summary::before {
    content: '▶';
    font-size: 0.6rem;
    transition: transform 0.15s;
    color: var(--text-muted);
  }

  .auth-details[open] .auth-summary::before { transform: rotate(90deg); }

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

  /* ── ZIP source toggle ───────────────────────────────────────────────────── */
  .zip-toggle-field {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    background: rgba(121, 192, 255, 0.06);
    border: 1px solid rgba(121, 192, 255, 0.25);
    border-radius: 6px;
    cursor: pointer;
  }

  .zip-checkbox {
    width: auto;
    margin: 0;
    accent-color: var(--accent);
    cursor: pointer;
  }

  .zip-toggle-label {
    font-size: 0.82rem;
    color: var(--text-primary);
    cursor: pointer;
    user-select: none;
  }

  .zip-note {
    margin: 0;
    font-size: 0.75rem;
    color: var(--text-muted);
    font-style: italic;
    line-height: 1.5;
  }

  .zip-note code {
    font-family: var(--font-mono);
    background: #0d1117;
    padding: 1px 4px;
    border-radius: 3px;
    font-size: 0.72rem;
    font-style: normal;
  }
</style>
