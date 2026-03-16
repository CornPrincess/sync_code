// web-server.js — sync-code web server (Node.js, zero external dependencies)
// Requires: Node.js 18+
// Usage:    node web-server.js
//
// Opens http://localhost:8080 in your browser automatically.
// Config is stored in the OS user-config directory (same location as the desktop app).

import http from 'node:http';
import https from 'node:https';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PORT = 8080;

// ─── CONFIG ────────────────────────────────────────────────────────────────

function configDir() {
  const plat = process.platform;
  if (plat === 'win32') {
    return path.join(process.env.APPDATA || os.homedir(), 'sync-code');
  }
  if (plat === 'darwin') {
    return path.join(os.homedir(), 'Library', 'Application Support', 'sync-code');
  }
  return path.join(process.env.XDG_CONFIG_HOME || path.join(os.homedir(), '.config'), 'sync-code');
}

const DEFAULT_AUTH   = { auth_type: 'none', username: '', password: '', token: '', ssh_key_path: '' };
const DEFAULT_REPO   = { local_path: '', remote_url: '', branch: '', auth: { ...DEFAULT_AUTH }, platform: 'github', use_zip: false, zip_path: '', zip_source_mode: 'local' };
const DEFAULT_PROXY  = { enabled: false, http_proxy: '', https_proxy: '', no_proxy: '' };

function makeDefaultConfig() {
  return {
    repo_a: { ...DEFAULT_REPO, auth: { ...DEFAULT_AUTH } },
    repo_b: { ...DEFAULT_REPO, auth: { ...DEFAULT_AUTH } },
    proxy: { ...DEFAULT_PROXY },
    repo_a_presets: [],
    repo_b_presets: [],
  };
}

function loadConfig() {
  const f = path.join(configDir(), 'config.json');
  if (!fs.existsSync(f)) return makeDefaultConfig();
  try {
    const raw = JSON.parse(fs.readFileSync(f, 'utf8'));
    return {
      repo_a: { ...DEFAULT_REPO, ...raw.repo_a, auth: { ...DEFAULT_AUTH, ...(raw.repo_a?.auth ?? {}) } },
      repo_b: { ...DEFAULT_REPO, ...raw.repo_b, auth: { ...DEFAULT_AUTH, ...(raw.repo_b?.auth ?? {}) } },
      proxy:  { ...DEFAULT_PROXY, ...(raw.proxy ?? {}) },
      repo_a_presets: raw.repo_a_presets ?? [],
      repo_b_presets: raw.repo_b_presets ?? [],
    };
  } catch {
    return makeDefaultConfig();
  }
}

function saveConfig(config) {
  const dir = configDir();
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, 'config.json'), JSON.stringify(config, null, 2), 'utf8');
}

// ─── SSE / LOGGING ─────────────────────────────────────────────────────────

function nowTs() {
  return new Date().toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

const SSE_HEADERS = {
  'Content-Type': 'text/event-stream',
  'Cache-Control': 'no-cache',
  'X-Accel-Buffering': 'no',
  'Access-Control-Allow-Origin': '*',
};

/** Send a log event line over an open SSE response. */
function sendLog(res, level, message) {
  res.write(`data: ${JSON.stringify({ type: 'log', payload: { level, message, timestamp: nowTs() } })}\n\n`);
}

/** Send the final result and close the SSE stream. */
function sendResult(res, payload) {
  res.write(`data: ${JSON.stringify({ type: 'result', payload })}\n\n`);
  res.end();
}

/** Send an error and close the SSE stream. */
function sendError(res, message) {
  res.write(`data: ${JSON.stringify({ type: 'error', payload: message })}\n\n`);
  res.end();
}

// ─── AUTH HELPERS ──────────────────────────────────────────────────────────

/** Percent-encode a credential string (keep alphanumeric and -._~). */
function percentEncode(str) {
  return String(str).split('').map(c => /[A-Za-z0-9\-._~]/.test(c) ? c : encodeURIComponent(c)).join('');
}

/**
 * Build a remote URL with embedded credentials for non-interactive git.
 * Returns null when no credentials are configured or URL is not HTTP(S).
 */
function authUrl(remoteUrl, auth, platform) {
  if (!remoteUrl || !auth) return null;
  const t = auth.auth_type;
  let user = '', pass = '';

  if (t === 'token' && auth.token) {
    if (platform === 'codeup') {
      user = auth.username ? percentEncode(auth.username) : 'git';
    } else {
      user = 'oauth2';
    }
    pass = percentEncode(auth.token);
  } else if (t === 'userpass' && auth.username && auth.password) {
    user = percentEncode(auth.username);
    pass = percentEncode(auth.password);
  } else {
    return null;
  }

  try {
    const u = new URL(remoteUrl);
    u.username = user;
    u.password = pass;
    return u.toString();
  } catch {
    return null;
  }
}

/**
 * Build the process environment for running git.
 * Handles GIT_TERMINAL_PROMPT, GIT_SSH_COMMAND, and proxy variables.
 */
function buildGitEnv(auth, proxy) {
  const env = { ...process.env };

  const hasCredentials = auth && auth.auth_type !== 'none' &&
    ((auth.auth_type === 'token'   && auth.token) ||
     (auth.auth_type === 'userpass' && auth.username && auth.password) ||
     (auth.auth_type === 'ssh'     && auth.ssh_key_path));

  if (hasCredentials) {
    env.GIT_TERMINAL_PROMPT = '0';
  } else {
    delete env.GIT_TERMINAL_PROMPT;
  }

  if (auth && auth.auth_type === 'ssh' && auth.ssh_key_path) {
    env.GIT_SSH_COMMAND = `ssh -i "${auth.ssh_key_path}" -o StrictHostKeyChecking=accept-new -o BatchMode=yes`;
  }

  if (proxy && proxy.enabled) {
    if (proxy.http_proxy)  { env.http_proxy  = proxy.http_proxy;  env.HTTP_PROXY  = proxy.http_proxy;  }
    if (proxy.https_proxy) { env.https_proxy = proxy.https_proxy; env.HTTPS_PROXY = proxy.https_proxy; }
    if (proxy.no_proxy)    { env.no_proxy    = proxy.no_proxy;    env.NO_PROXY    = proxy.no_proxy;    }
  }

  return env;
}

// ─── GIT RUNNER ────────────────────────────────────────────────────────────

/**
 * Spawn `git <args>` in `dir`.
 * - Streams stdout/stderr lines to `logFn(level, line)` if provided.
 * - `displayArgs` replaces `args` in log output (for hiding credentials).
 * - Resolves with stdout string on exit code 0; rejects with Error otherwise.
 */
function runGit(dir, args, { env = process.env, logFn = null, displayArgs = null } = {}) {
  return new Promise((resolve, reject) => {
    const show = displayArgs ?? args;
    if (logFn) logFn('info', `$ git ${show.join(' ')}`);

    const proc = spawn('git', args, { cwd: dir, env, shell: false });
    // Collect raw Buffers so multibyte UTF-8 sequences split across chunk
    // boundaries are decoded correctly (chunk.toString() would corrupt them).
    const stdoutChunks = [];
    const stderrChunks = [];

    proc.stdout.on('data', chunk => {
      stdoutChunks.push(chunk);
      if (logFn) {
        chunk.toString('utf8').split('\n').filter(l => l.trim()).forEach(l => logFn('info', l));
      }
    });
    proc.stderr.on('data', chunk => {
      stderrChunks.push(chunk);
      if (logFn) {
        chunk.toString('utf8').split('\n').filter(l => l.trim()).forEach(l => logFn('info', l));
      }
    });
    proc.on('error', err => reject(new Error(`Failed to spawn git: ${err.message}`)));
    proc.on('close', code => {
      const stdout = Buffer.concat(stdoutChunks).toString('utf8');
      const stderr = Buffer.concat(stderrChunks).toString('utf8');
      if (code === 0) {
        resolve(stdout);
      } else {
        reject(new Error(stderr.trim() || stdout.trim() || `git exited with code ${code}`));
      }
    });
  });
}

// ─── GIT OPERATIONS ────────────────────────────────────────────────────────

function validateRepo(localPath) {
  if (!localPath) throw new Error('Repository path is empty');
  if (!fs.existsSync(localPath)) throw new Error(`Path does not exist: ${localPath}`);
  if (!fs.existsSync(path.join(localPath, '.git'))) throw new Error(`Not a git repository: ${localPath}`);
}

async function gitPull(repo, proxy, logFn) {
  const env = buildGitEnv(repo.auth, proxy);
  const url = authUrl(repo.remote_url, repo.auth, repo.platform);
  if (url) {
    const displayFetch = ['-c', 'credential.helper=', 'fetch', '--progress', '<redacted>',
      `refs/heads/${repo.branch}:refs/remotes/origin/${repo.branch}`];
    await runGit(repo.local_path,
      ['-c', 'credential.helper=', 'fetch', '--progress', url,
        `refs/heads/${repo.branch}:refs/remotes/origin/${repo.branch}`],
      { env, logFn, displayArgs: displayFetch });
  } else {
    await runGit(repo.local_path, ['fetch', '--progress', 'origin'], { env, logFn });
  }
  await runGit(repo.local_path, ['reset', '--hard', `origin/${repo.branch}`], { env, logFn });
}

async function gitAddAll(localPath, logFn) {
  await runGit(localPath, ['add', '-A'], { logFn });
}

/**
 * Decode a git-quoted path back to a UTF-8 string.
 * When core.quotePath=true (git default), non-ASCII bytes in filenames are
 * wrapped in double-quotes and escaped as octal sequences, e.g.:
 *   "src/\344\270\255\346\226\207.txt"
 * This function decodes both quoted (octal) and unquoted (plain UTF-8) forms.
 */
function decodeGitPath(s) {
  if (!s || !s.startsWith('"') || !s.endsWith('"')) return s;
  const inner = s.slice(1, -1);
  const bytes = [];
  let i = 0;
  while (i < inner.length) {
    if (inner[i] === '\\' && i + 1 < inner.length) {
      // Octal escape: \XYZ (three octal digits)
      if (i + 3 < inner.length &&
          inner[i+1] >= '0' && inner[i+1] <= '7' &&
          inner[i+2] >= '0' && inner[i+2] <= '7' &&
          inner[i+3] >= '0' && inner[i+3] <= '7') {
        bytes.push(parseInt(inner.slice(i + 1, i + 4), 8));
        i += 4;
      } else {
        // Other C-style escapes: \n \t \\ \"
        switch (inner[i + 1]) {
          case 'n':  bytes.push(0x0A); break;
          case 't':  bytes.push(0x09); break;
          case '\\': bytes.push(0x5C); break;
          case '"':  bytes.push(0x22); break;
          default:   bytes.push(inner.charCodeAt(i)); i -= 1; break;
        }
        i += 2;
      }
    } else {
      bytes.push(inner.charCodeAt(i));
      i += 1;
    }
  }
  return Buffer.from(bytes).toString('utf8');
}

/** Parse `git diff --cached --name-status` output into FileChange objects. */
function parseNameStatus(output) {
  return output.split('\n').filter(l => l.trim()).map(line => {
    const parts = line.split('\t');
    const status = parts[0] ?? '';
    if (status.startsWith('R')) return { status: 'renamed', path: decodeGitPath(parts[2] ?? ''), old_path: decodeGitPath(parts[1] ?? '') };
    if (status.startsWith('C')) return { status: 'copied',  path: decodeGitPath(parts[2] ?? ''), old_path: decodeGitPath(parts[1] ?? '') };
    if (status === 'A') return { status: 'added',    path: decodeGitPath(parts[1] ?? ''), old_path: null };
    if (status === 'M') return { status: 'modified', path: decodeGitPath(parts[1] ?? ''), old_path: null };
    if (status === 'D') return { status: 'deleted',  path: decodeGitPath(parts[1] ?? ''), old_path: null };
    return { status: 'unknown', path: decodeGitPath(parts[1] ?? ''), old_path: null };
  });
}

async function getStagedFiles(localPath) {
  const out = await runGit(localPath, ['-c', 'core.quotePath=false', 'diff', '--cached', '--name-status']);
  return parseNameStatus(out);
}

async function gitCommit(localPath, message, logFn) {
  // `diff --cached --quiet` exits 0 (resolve) when nothing staged → skip commit.
  try {
    await runGit(localPath, ['diff', '--cached', '--quiet']);
    if (logFn) logFn('info', 'Nothing staged, skipping commit.');
    return;
  } catch {
    // Exit code 1 → there are staged changes, proceed.
  }
  const msg = message && message.trim()
    ? message.trim()
    : `sync: ${new Date().toISOString().replace('T', ' ').slice(0, 19)}`;
  await runGit(localPath, ['commit', '-m', msg], { logFn });
}

async function gitPushOnly(repo, proxy, logFn) {
  const env = buildGitEnv(repo.auth, proxy);
  const url = authUrl(repo.remote_url, repo.auth, repo.platform);
  if (url) {
    const displayPush = ['-c', 'credential.helper=', 'push', '--progress', '<redacted>',
      `HEAD:refs/heads/${repo.branch}`];
    await runGit(repo.local_path,
      ['-c', 'credential.helper=', 'push', '--progress', url, `HEAD:refs/heads/${repo.branch}`],
      { env, logFn, displayArgs: displayPush });
  } else {
    await runGit(repo.local_path,
      ['push', '--progress', 'origin', `HEAD:refs/heads/${repo.branch}`],
      { env, logFn });
  }
}

async function isDirty(localPath) {
  const out = await runGit(localPath, ['status', '--porcelain']);
  return out.trim().length > 0;
}

async function discardChanges(localPath) {
  await runGit(localPath, ['reset', 'HEAD', '.']);
  await runGit(localPath, ['checkout', '--', '.']);
  await runGit(localPath, ['clean', '-fd']);
}

async function stageSelected(localPath, paths, logFn) {
  await runGit(localPath, ['reset', 'HEAD', '.'], { logFn });
  if (paths.length > 0) {
    await runGit(localPath, ['add', '--', ...paths], { logFn });
  }
}

async function revertRemaining(localPath) {
  await runGit(localPath, ['checkout', '--', '.']);
  await runGit(localPath, ['clean', '-fd']);
}

async function listBranchesFromRefs(localPath) {
  const localOut  = await runGit(localPath, ['for-each-ref', '--format=%(refname)', 'refs/heads/']);
  const remoteOut = await runGit(localPath, ['for-each-ref', '--format=%(refname)', 'refs/remotes/origin/']);
  const local  = localOut .split('\n').filter(l => l.trim()).map(l => l.replace('refs/heads/', ''));
  const remote = remoteOut.split('\n').filter(l => l.trim())
    .map(l => l.replace('refs/remotes/origin/', '')).filter(b => b !== 'HEAD');
  return { local, remote };
}

async function refreshBranches(localPath, proxy, auth) {
  const env = buildGitEnv(auth || { auth_type: 'none' }, proxy);
  await runGit(localPath,
    ['fetch', 'origin', '--prune', '+refs/heads/*:refs/remotes/origin/*'],
    { env });
  return listBranchesFromRefs(localPath);
}

async function checkoutBranch(localPath, branch) {
  try {
    await runGit(localPath, ['checkout', branch]);
  } catch {
    await runGit(localPath, ['checkout', '--track', `origin/${branch}`]);
  }
}

async function checkoutAndPull(localPath, branch, remoteUrl, auth, proxy, platform, logFn) {
  await checkoutBranch(localPath, branch);
  const env = buildGitEnv(auth, proxy);
  const url = authUrl(remoteUrl, auth, platform);
  if (url) {
    await runGit(localPath,
      ['-c', 'credential.helper=', 'pull', url, branch],
      { env, logFn, displayArgs: ['-c', 'credential.helper=', 'pull', '<redacted>', branch] });
  } else {
    await runGit(localPath, ['pull', 'origin', branch], { env, logFn });
  }
}

// ─── ZIP EXTRACTION ────────────────────────────────────────────────────────

/**
 * Extract a zip archive to a fresh temp directory using system tools.
 * - Windows: PowerShell Expand-Archive (built-in on Windows 10+)
 * - Linux/macOS: unzip
 * Returns the temp directory path.
 */
function extractZip(zipPath, logFn) {
  const tmpDir = path.join(os.tmpdir(), `sync-code-zip-${Date.now()}`);
  fs.mkdirSync(tmpDir, { recursive: true });
  if (logFn) {
    logFn('info', `Extracting ZIP: ${zipPath}`);
    logFn('info', `Temp dir: ${tmpDir}`);
  }

  return new Promise((resolve, reject) => {
    let proc;
    if (process.platform === 'win32') {
      // PowerShell is available on all supported Windows versions
      proc = spawn('powershell', [
        '-NoProfile', '-NonInteractive', '-Command',
        `Expand-Archive -LiteralPath '${zipPath.replace(/'/g, "''")}' -DestinationPath '${tmpDir.replace(/'/g, "''")}' -Force`,
      ], { stdio: 'pipe' });
    } else {
      proc = spawn('unzip', ['-o', zipPath, '-d', tmpDir], { stdio: 'pipe' });
    }

    let errOut = '';
    proc.stderr?.on('data', chunk => { errOut += chunk.toString(); });
    proc.stdout?.on('data', chunk => { errOut += chunk.toString(); }); // unzip uses stdout for errors too

    proc.on('error', err => {
      try { fs.rmSync(tmpDir, { recursive: true, force: true }); } catch { /* ok */ }
      reject(new Error(
        process.platform === 'win32'
          ? `PowerShell Expand-Archive failed: ${err.message}`
          : `unzip not found: ${err.message}. Please install unzip.`,
      ));
    });

    proc.on('close', code => {
      // unzip exits 1 for warnings (still OK); PowerShell exits 0 on success
      const ok = process.platform === 'win32' ? code === 0 : (code === 0 || code === 1);
      if (ok) {
        if (logFn) logFn('info', 'ZIP extracted successfully.');
        resolve(tmpDir);
      } else {
        try { fs.rmSync(tmpDir, { recursive: true, force: true }); } catch { /* ok */ }
        reject(new Error(`ZIP extraction failed (exit ${code}): ${errOut.trim()}`));
      }
    });
  });
}

/**
 * Extract a tar.gz (or .tgz) archive to a temp directory.
 *
 * On Windows, the built-in tar.exe (bsdtar) decodes non-ASCII filenames
 * using the system ANSI code page (ACP, e.g. CP1252) rather than UTF-8,
 * which garbles Chinese filenames.  We work around this by:
 *   1. Trying Python 3's tarfile module first — it always uses UTF-8 and
 *      writes filenames via the Windows Unicode API, so Chinese names are
 *      preserved correctly.
 *   2. Falling back to system tar with --hdrcharset=UTF-8 if Python is
 *      not installed.
 *
 * On Linux/macOS, the locale is UTF-8 and system tar works correctly.
 * Returns the temp directory path.
 */
function extractTarGz(archivePath, logFn) {
  const tmpDir = path.join(os.tmpdir(), `sync-code-tar-${Date.now()}`);
  fs.mkdirSync(tmpDir, { recursive: true });
  if (logFn) {
    logFn('info', `Extracting tar.gz: ${archivePath}`);
    logFn('info', `Temp dir: ${tmpDir}`);
  }

  // Python one-liner: open the archive and extract to tmpDir.
  // Uses tarfile which decodes header bytes as UTF-8 by default.
  const PYTHON_SCRIPT =
    'import tarfile,sys; tarfile.open(sys.argv[1],"r:gz").extractall(sys.argv[2])';

  return new Promise((resolve, reject) => {
    const cleanup = () => {
      try { fs.rmSync(tmpDir, { recursive: true, force: true }); } catch { /* ok */ }
    };

    function runProc(cmd, args) {
      const proc = spawn(cmd, args, { stdio: 'pipe', shell: false });
      let errOut = '';
      proc.stderr?.on('data', chunk => { errOut += chunk.toString('utf8'); });
      proc.stdout?.on('data', chunk => { errOut += chunk.toString('utf8'); });
      return { proc, getErr: () => errOut };
    }

    if (process.platform === 'win32') {
      // ── Attempt 1: Python 3 ──────────────────────────────────────────────
      const { proc: pyProc, getErr: getPyErr } = runProc('python', [
        '-c', PYTHON_SCRIPT, archivePath, tmpDir,
      ]);

      pyProc.on('error', () => {
        // Python not found — fall back to system tar with explicit charset.
        if (logFn) logFn('info', 'Python not found, falling back to tar --hdrcharset=UTF-8');
        const { proc: tarProc, getErr: getTarErr } = runProc('tar', [
          '--hdrcharset=UTF-8', '-xzf', archivePath, '-C', tmpDir,
        ]);
        tarProc.on('error', err => {
          cleanup();
          reject(new Error(`tar not found: ${err.message}. Install tar (Windows 10+ includes it) or Python 3.`));
        });
        tarProc.on('close', code => {
          if (code === 0) { if (logFn) logFn('info', 'tar.gz extracted successfully.'); resolve(tmpDir); }
          else { cleanup(); reject(new Error(`tar extraction failed (exit ${code}): ${getTarErr().trim()}`)); }
        });
      });

      pyProc.on('close', code => {
        if (code === 0) { if (logFn) logFn('info', 'tar.gz extracted successfully.'); resolve(tmpDir); }
        else { cleanup(); reject(new Error(`tar.gz extraction failed (exit ${code}): ${getPyErr().trim()}`)); }
      });
    } else {
      // ── Linux / macOS: system tar with UTF-8 locale ─────────────────────
      const { proc, getErr } = runProc('tar', ['-xzf', archivePath, '-C', tmpDir]);
      proc.on('error', err => {
        cleanup();
        reject(new Error(`tar not found: ${err.message}. Please install tar.`));
      });
      proc.on('close', code => {
        if (code === 0) { if (logFn) logFn('info', 'tar.gz extracted successfully.'); resolve(tmpDir); }
        else { cleanup(); reject(new Error(`tar extraction failed (exit ${code}): ${getErr().trim()}`)); }
      });
    }
  });
}

/**
 * Extract an archive file, auto-detecting format from extension.
 * Supports .zip and .tar.gz / .tgz.
 */
function extractArchive(archivePath, logFn) {
  if (archivePath.endsWith('.tar.gz') || archivePath.endsWith('.tgz')) {
    return extractTarGz(archivePath, logFn);
  }
  return extractZip(archivePath, logFn);
}

/**
 * If `dir` contains exactly one subdirectory (and nothing else),
 * return that subdirectory — handles GitHub's wrapper dir (e.g. `repo-main/`).
 */
function findSingleTopDir(dir) {
  try {
    const entries = fs.readdirSync(dir, { withFileTypes: true });
    if (entries.length === 1 && entries[0].isDirectory()) {
      return path.join(dir, entries[0].name);
    }
  } catch { /* ok */ }
  return null;
}

// ─── FILE MIRRORING ────────────────────────────────────────────────────────

/** Recursively list all files under `dir`, relative to `dir`. Skips `.git/`. */
function walkDir(dir) {
  const results = [];
  function walk(current) {
    let entries;
    try { entries = fs.readdirSync(current, { withFileTypes: true }); } catch { return; }
    for (const e of entries) {
      if (e.name === '.git') continue;
      const full = path.join(current, e.name);
      if (e.isDirectory()) {
        walk(full);
      } else {
        results.push(path.relative(dir, full));
      }
    }
  }
  walk(dir);
  return results;
}

/**
 * Mirror files from `src` into `dst`:
 * 1. Copy new / changed files (compared by size + mtime).
 * 2. Delete stale files in dst not present in src.
 * 3. Prune empty directories in dst (bottom-up).
 * Unconditionally skips `.git/` in both trees.
 */
async function mirrorFiles(src, dst, logFn) {
  const srcFiles = new Set(walkDir(src));
  let copied = 0;
  let deleted = 0;

  for (const rel of srcFiles) {
    const srcPath = path.join(src, rel);
    const dstPath = path.join(dst, rel);

    let needsCopy = true;
    if (fs.existsSync(dstPath)) {
      const ss = fs.statSync(srcPath);
      const ds = fs.statSync(dstPath);
      needsCopy = ss.size !== ds.size || Math.abs(ss.mtimeMs - ds.mtimeMs) > 1;
    }
    if (needsCopy) {
      fs.mkdirSync(path.dirname(dstPath), { recursive: true });
      fs.copyFileSync(srcPath, dstPath);
      copied++;
      if (logFn) logFn('info', `  copy: ${rel}`);
    }
  }

  for (const rel of walkDir(dst)) {
    if (!srcFiles.has(rel)) {
      try { fs.unlinkSync(path.join(dst, rel)); deleted++; if (logFn) logFn('info', `  delete: ${rel}`); } catch { /* ok */ }
    }
  }

  function pruneEmptyDirs(dir) {
    let entries;
    try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch { return; }
    for (const e of entries) {
      if (e.name === '.git') continue;
      if (e.isDirectory()) {
        const full = path.join(dir, e.name);
        pruneEmptyDirs(full);
        try { if (fs.readdirSync(full).length === 0) fs.rmdirSync(full); } catch { /* ok */ }
      }
    }
  }
  pruneEmptyDirs(dst);

  if (logFn) logFn('info', `Mirror done: ${copied} copied, ${deleted} deleted.`);
}

// ─── SYNC OPERATIONS ───────────────────────────────────────────────────────

async function doSync(config, logFn) {
  const { repo_a, repo_b, proxy } = config;
  const useZip = !!repo_b.use_zip;

  // ── Step 1: Validate ──────────────────────────────────────────────────────
  logFn('info', '━━ Step 1/6 — Validating repositories ━━');
  validateRepo(repo_a.local_path);
  if (useZip) {
    const zip = repo_b.zip_path?.trim();
    if (!zip) throw new Error('Repo B: ZIP 文件路径不能为空。');
    if (!fs.existsSync(zip)) throw new Error(`ZIP file not found: ${zip}`);
    logFn('info', `  ZIP source: ${zip}`);
  } else {
    validateRepo(repo_b.local_path);
  }

  // ── Step 2: Check Repo A clean ────────────────────────────────────────────
  logFn('info', '━━ Step 2/6 — Checking Repo A working tree ━━');
  if (await isDirty(repo_a.local_path)) {
    throw new Error('Repo A has uncommitted changes. Please commit or discard them before syncing.');
  }
  logFn('info', '  Repo A working tree is clean.');

  // ── Step 3: Pull Repo A ───────────────────────────────────────────────────
  logFn('info', `━━ Step 3/6 — Pulling Repo A (${repo_a.local_path}@${repo_a.branch}) ━━`);
  if (repo_a.remote_url) {
    await gitPull(repo_a, proxy, logFn);
  } else {
    logFn('info', '  No remote URL for Repo A — skipping pull.');
  }

  // ── Step 4: Pull Repo B  OR  extract zip ─────────────────────────────────
  let srcPath;
  let tmpDir = null;
  if (useZip) {
    logFn('info', '━━ Step 4/6 — Extracting Repo B ZIP ━━');
    tmpDir = await extractArchive(repo_b.zip_path.trim(), logFn);
    // Handle GitHub/GitLab wrapper directory (e.g. repo-main/ inside the zip)
    const singleTop = findSingleTopDir(tmpDir);
    srcPath = singleTop || tmpDir;
    logFn('info', `  Using source directory: ${srcPath}`);
  } else {
    logFn('info', `━━ Step 4/6 — Pulling Repo B (${repo_b.local_path}@${repo_b.branch}) ━━`);
    await gitPull(repo_b, proxy, logFn);
    srcPath = repo_b.local_path;
  }

  // ── Step 5: Mirror B → A ─────────────────────────────────────────────────
  logFn('info', '━━ Step 5/6 — Mirroring Repo B → Repo A ━━');
  try {
    await mirrorFiles(srcPath, repo_a.local_path, logFn);
  } finally {
    if (tmpDir) {
      try { fs.rmSync(tmpDir, { recursive: true, force: true }); } catch { /* ok */ }
    }
  }

  // ── Step 6: Stage ─────────────────────────────────────────────────────────
  logFn('info', '━━ Step 6/6 — Staging changes in Repo A ━━');
  await gitAddAll(repo_a.local_path, logFn);
  const staged = await getStagedFiles(repo_a.local_path);
  logFn('success', `Ready for review: ${staged.length} file change(s) staged.`);
  return staged;
}

async function runCommitAndPush(config, commitMessage, pathsToStage, logFn) {
  const { repo_a, proxy } = config;
  logFn('info', 'Staging selected files…');
  await stageSelected(repo_a.local_path, pathsToStage, logFn);
  logFn('info', 'Committing…');
  await gitCommit(repo_a.local_path, commitMessage, logFn);
  logFn('info', 'Pushing Repo A…');
  await gitPushOnly(repo_a, proxy, logFn);
  logFn('info', 'Reverting unselected changes…');
  await revertRemaining(repo_a.local_path);
  logFn('success', 'Done! Changes committed and pushed.');
}

// ─── HTTP UTILITIES ────────────────────────────────────────────────────────

const MIME_TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js':   'application/javascript',
  '.mjs':  'application/javascript',
  '.css':  'text/css',
  '.json': 'application/json',
  '.png':  'image/png',
  '.jpg':  'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif':  'image/gif',
  '.svg':  'image/svg+xml',
  '.ico':  'image/x-icon',
  '.woff': 'font/woff',
  '.woff2':'font/woff2',
  '.ttf':  'font/ttf',
  '.webp': 'image/webp',
};

function serveStatic(res, filePath) {
  if (!fs.existsSync(filePath)) return false;
  const mime = MIME_TYPES[path.extname(filePath).toLowerCase()] ?? 'application/octet-stream';
  const size = fs.statSync(filePath).size;
  res.writeHead(200, { 'Content-Type': mime, 'Content-Length': size });
  fs.createReadStream(filePath).pipe(res);
  return true;
}

function readJsonBody(req) {
  return new Promise((resolve, reject) => {
    let body = '';
    req.on('data', chunk => { body += chunk; });
    req.on('end', () => {
      try { resolve(JSON.parse(body || '{}')); } catch (e) { reject(new Error('Invalid JSON')); }
    });
    req.on('error', reject);
  });
}

function addCorsHeaders(res) {
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type');
}

function sendJson(res, data, status = 200) {
  const body = JSON.stringify(data);
  res.writeHead(status, { 'Content-Type': 'application/json', 'Content-Length': Buffer.byteLength(body) });
  res.end(body);
}

function sendPlainError(res, message, code = 500) {
  res.writeHead(code, { 'Content-Type': 'text/plain' });
  res.end(message);
}

// ─── ROUTE HANDLERS ────────────────────────────────────────────────────────

let syncLocked = false;

// ─── PLATFORM API DOWNLOAD ─────────────────────────────────────────────────

/**
 * Build archive download URL and auth header details for a given platform.
 * @param {string} format  "zip" or "tar.gz" (default "zip")
 */
function buildArchiveApiUrl(remoteUrl, branch, platform, format = 'zip') {
  const u = new URL(remoteUrl);
  const projectPath = u.pathname.replace(/^\//, '').replace(/\.git$/, '');
  if (platform === 'github') {
    const [owner, repo] = projectPath.split('/');
    // GitHub uses separate endpoints: zipball vs tarball
    const archiveType = format === 'tar.gz' ? 'tarball' : 'zipball';
    return {
      url: `https://api.github.com/repos/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}/${archiveType}/${encodeURIComponent(branch)}`,
      headerName: 'Authorization',
      headerValue: '',  // filled in by caller
      isGitHub: true,
    };
  } else {
    // Codeup / GitLab: use format query parameter
    const encodedPath = encodeURIComponent(projectPath);
    return {
      url: `${u.protocol}//${u.host}/api/v4/projects/${encodedPath}/repository/archive?sha=${encodeURIComponent(branch)}&format=${encodeURIComponent(format)}`,
      headerName: 'PRIVATE-TOKEN',
      headerValue: '',  // filled in by caller
      isGitHub: false,
    };
  }
}

/** Download a URL to a Buffer, following up to 10 redirects. */
function downloadBuffer(url, reqHeaders, redirectCount = 0) {
  if (redirectCount > 10) return Promise.reject(new Error('Too many redirects'));
  return new Promise((resolve, reject) => {
    const parsed = new URL(url);
    const mod = parsed.protocol === 'https:' ? https : http;
    const opts = {
      hostname: parsed.hostname,
      port: parsed.port || (parsed.protocol === 'https:' ? 443 : 80),
      path: parsed.pathname + parsed.search,
      method: 'GET',
      headers: reqHeaders,
    };
    const req = mod.request(opts, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        res.resume();
        resolve(downloadBuffer(res.headers.location, reqHeaders, redirectCount + 1));
        return;
      }
      if (res.statusCode < 200 || res.statusCode >= 300) {
        const chunks = [];
        res.on('data', c => chunks.push(c));
        res.on('end', () => reject(new Error(`HTTP ${res.statusCode}: ${Buffer.concat(chunks).toString().slice(0, 300)}`)));
        return;
      }
      const chunks = [];
      res.on('data', c => chunks.push(c));
      res.on('end', () => resolve(Buffer.concat(chunks)));
      res.on('error', reject);
    });
    req.on('error', reject);
    req.end();
  });
}

/**
 * Dispatch API routes. Returns false when no route matched (→ fall through to static).
 * Returning anything else (including undefined) means the route was handled.
 */
async function handleApiRoute(req, res, parsedUrl) {
  const { pathname } = parsedUrl;
  const method = req.method;

  // ── Config ──────────────────────────────────────────────────────────────
  if (pathname === '/api/config' && method === 'GET') {
    return sendJson(res, loadConfig());
  }
  if (pathname === '/api/config' && method === 'POST') {
    saveConfig(await readJsonBody(req));
    res.writeHead(204); res.end();
    return;
  }

  // ── ZIP upload (web mode: browser can't reveal full path, so we receive the file) ──
  if (pathname === '/api/upload/zip' && method === 'POST') {
    const rawName = req.headers['x-filename'];
    const safeName = (rawName ? decodeURIComponent(rawName) : 'upload.zip')
      .replace(/[^a-zA-Z0-9._\-]/g, '_');
    const uploadDir = path.join(os.tmpdir(), 'sync-code-uploads');
    fs.mkdirSync(uploadDir, { recursive: true });
    const savePath = path.join(uploadDir, `${Date.now()}-${safeName}`);

    await new Promise((resolve, reject) => {
      const out = fs.createWriteStream(savePath);
      req.pipe(out);
      out.on('finish', resolve);
      out.on('error', reject);
      req.on('error', reject);
    });

    return sendJson(res, { path: savePath });
  }

  // ── Download repo ZIP from platform API ──────────────────────────────────
  if (pathname === '/api/download/repo-zip' && method === 'POST') {
    const { remote_url, branch, token, platform, format } = await readJsonBody(req);
    if (!remote_url || !branch || !token) {
      return sendPlainError(res, 'remote_url, branch, and token are required');
    }
    const fmt = format === 'tar.gz' ? 'tar.gz' : 'zip';
    let apiInfo;
    try {
      apiInfo = buildArchiveApiUrl(remote_url.trim(), branch.trim(), platform || 'github', fmt);
    } catch (e) {
      return sendPlainError(res, `Invalid remote URL: ${e.message}`);
    }
    const reqHeaders = {};
    if (apiInfo.isGitHub) {
      reqHeaders['Authorization'] = `Bearer ${token}`;
      reqHeaders['Accept'] = 'application/vnd.github+json';
      reqHeaders['X-GitHub-Api-Version'] = '2022-11-28';
      reqHeaders['User-Agent'] = 'sync-code/1.0';
    } else {
      reqHeaders['PRIVATE-TOKEN'] = token;
    }
    try {
      const buf = await downloadBuffer(apiInfo.url, reqHeaders);
      const ts = Date.now();
      const tmpDir = path.join(os.tmpdir(), `sync-code-dl-${ts}`);
      fs.mkdirSync(tmpDir, { recursive: true });
      const filename = fmt === 'tar.gz' ? 'repo.tar.gz' : 'repo.zip';
      const archivePath = path.join(tmpDir, filename);
      fs.writeFileSync(archivePath, buf);
      return sendJson(res, archivePath);
    } catch (e) {
      return sendPlainError(res, e.message);
    }
  }

  // ── Sync: start (SSE stream) ─────────────────────────────────────────────
  if (pathname === '/api/sync/start' && method === 'POST') {
    if (syncLocked) return sendPlainError(res, 'Sync already in progress', 409);
    const config = await readJsonBody(req);
    syncLocked = true;
    res.writeHead(200, SSE_HEADERS);
    try {
      const files = await doSync(config, (level, msg) => sendLog(res, level, msg));
      sendResult(res, files);
    } catch (e) {
      sendError(res, e.message);
    } finally {
      syncLocked = false;
    }
    return;
  }

  // ── Sync: commit (SSE stream) ────────────────────────────────────────────
  if (pathname === '/api/sync/commit' && method === 'POST') {
    if (syncLocked) return sendPlainError(res, 'Sync already in progress', 409);
    const { config, commit_message, paths_to_stage } = await readJsonBody(req);
    syncLocked = true;
    res.writeHead(200, SSE_HEADERS);
    try {
      await runCommitAndPush(config, commit_message || '', paths_to_stage || [],
        (level, msg) => sendLog(res, level, msg));
      sendResult(res, null);
    } catch (e) {
      sendError(res, e.message);
    } finally {
      syncLocked = false;
    }
    return;
  }

  // ── Sync: discard ────────────────────────────────────────────────────────
  if (pathname === '/api/sync/discard' && method === 'POST') {
    const config = await readJsonBody(req);
    try {
      await discardChanges(config.repo_a.local_path);
      res.writeHead(204); res.end();
    } catch (e) {
      sendPlainError(res, e.message);
    }
    return;
  }

  // ── Branches: list ───────────────────────────────────────────────────────
  if (pathname === '/api/branches' && method === 'GET') {
    const localPath = parsedUrl.searchParams.get('path') || '';
    if (!localPath) return sendJson(res, { local: [], remote: [] });
    try {
      validateRepo(localPath);
      return sendJson(res, await listBranchesFromRefs(localPath));
    } catch {
      return sendJson(res, { local: [], remote: [] });
    }
  }

  // ── Branches: refresh ────────────────────────────────────────────────────
  if (pathname === '/api/branches/refresh' && method === 'POST') {
    const { local_path, proxy } = await readJsonBody(req);
    try {
      validateRepo(local_path);
      return sendJson(res, await refreshBranches(local_path, proxy));
    } catch (e) {
      return sendPlainError(res, e.message);
    }
  }

  // ── Branches: checkout ───────────────────────────────────────────────────
  if (pathname === '/api/branches/checkout' && method === 'POST') {
    const { local_path, branch } = await readJsonBody(req);
    try {
      await checkoutBranch(local_path, branch);
      res.writeHead(204); res.end();
    } catch (e) {
      sendPlainError(res, e.message);
    }
    return;
  }

  // ── Branches: checkout-pull ──────────────────────────────────────────────
  if (pathname === '/api/branches/checkout-pull' && method === 'POST') {
    const { local_path, branch, remote_url, auth, proxy, platform } = await readJsonBody(req);
    try {
      await checkoutAndPull(local_path, branch, remote_url, auth, proxy, platform, null);
      res.writeHead(204); res.end();
    } catch (e) {
      sendPlainError(res, e.message);
    }
    return;
  }

  return false; // no route matched
}

// ─── STATIC FILE SERVING ───────────────────────────────────────────────────

function findDistDir() {
  for (const candidate of [
    path.join(__dirname, 'dist'),
    path.join(__dirname, '..', 'dist'),
  ]) {
    if (fs.existsSync(candidate) && fs.existsSync(path.join(candidate, 'index.html'))) {
      return candidate;
    }
  }
  return null;
}

// ─── MAIN REQUEST HANDLER ──────────────────────────────────────────────────

async function requestHandler(req, res) {
  const parsedUrl = new URL(req.url, `http://localhost:${PORT}`);

  addCorsHeaders(res);

  // CORS preflight
  if (req.method === 'OPTIONS') {
    res.writeHead(204); res.end();
    return;
  }

  // API routes
  if (parsedUrl.pathname.startsWith('/api/')) {
    try {
      const handled = await handleApiRoute(req, res, parsedUrl);
      if (handled !== false) return;
    } catch (e) {
      console.error('[API error]', e);
      if (!res.headersSent) sendPlainError(res, e.message);
      return;
    }
    // Unknown /api/* path
    if (!res.headersSent) sendPlainError(res, 'Not found', 404);
    return;
  }

  // Static files from dist/
  const distDir = findDistDir();
  if (!distDir) {
    res.writeHead(503, { 'Content-Type': 'text/plain' });
    res.end('dist/ not found. Run: npm run build');
    return;
  }

  let reqPath = parsedUrl.pathname;
  if (reqPath === '/') reqPath = '/index.html';

  // Guard against path traversal
  const absPath = path.normalize(path.join(distDir, reqPath));
  if (!absPath.startsWith(path.normalize(distDir))) {
    res.writeHead(403); res.end('Forbidden');
    return;
  }

  // Try exact path, then SPA fallback to index.html
  if (!serveStatic(res, absPath)) {
    serveStatic(res, path.join(distDir, 'index.html'));
  }
}

// ─── BROWSER LAUNCHER ──────────────────────────────────────────────────────

function openBrowser(url) {
  try {
    const plat = process.platform;
    if (plat === 'win32') {
      spawn('cmd', ['/c', 'start', '', url], { detached: true, stdio: 'ignore' });
    } else if (plat === 'darwin') {
      spawn('open', [url], { detached: true, stdio: 'ignore' });
    } else {
      spawn('xdg-open', [url], { detached: true, stdio: 'ignore' });
    }
  } catch { /* ignore */ }
}

// ─── ENTRY POINT ───────────────────────────────────────────────────────────

const distDir = findDistDir();
if (!distDir) {
  console.warn('[warn] dist/ directory not found. Static files will not be served.');
  console.warn('       Run `npm run build` first to generate the frontend.');
}

const server = http.createServer(requestHandler);
server.listen(PORT, '127.0.0.1', () => {
  const url = `http://localhost:${PORT}`;
  console.log(`\nsync-code web server listening on ${url}`);
  console.log('Press Ctrl+C to stop.\n');
  openBrowser(url);
});
