# CLAUDE.md

This file provides guidance to AI assistants working with this repository.

## Project Overview

**sync_code** — A cross-platform desktop application that syncs code from one git repository (source) into another (target), then pushes the result.

**Sync workflow:**
1. Pull latest from Repo B (source) branch
2. Mirror Repo B's files into Repo A (target) — copy new/changed, delete removed, skip `.git/`
3. Commit and push Repo A to its remote

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop framework | Tauri v2 (Rust backend + system WebView) |
| Frontend | Svelte 5 + TypeScript |
| Build tool | Vite 8 |
| Git operations | Shell `git` binary via subprocess (inherits user SSH/credential setup) |
| Config persistence | JSON file in `app_config_dir` (Tauri managed path) |
| Rust dependencies | `walkdir`, `serde`/`serde_json`, `chrono`, `thiserror`, Tauri plugins |

## Repository Structure

```
sync_code/
├── src/                              # Svelte 5 frontend
│   ├── lib/
│   │   ├── components/
│   │   │   ├── RepoPanel.svelte      # Repo config form (local path, remote URL, branch)
│   │   │   ├── LogViewer.svelte      # Auto-scrolling log panel (sync://log events)
│   │   │   └── StatusBadge.svelte    # Sync status indicator
│   │   ├── stores/
│   │   │   └── config.svelte.ts      # Svelte 5 $state() reactive config store
│   │   └── ipc.ts                    # All Tauri invoke() calls — ONLY file touching IPC
│   ├── App.svelte                    # Root layout
│   ├── app.css                       # Global CSS custom properties (dark theme)
│   └── main.ts                       # Svelte mount entry
├── src-tauri/
│   ├── src/
│   │   ├── main.rs                   # Binary entry point (calls lib::run)
│   │   ├── lib.rs                    # Tauri builder, plugin registration, command registration
│   │   ├── models.rs                 # Shared data types: RepoConfig, AppConfig, SyncEvent
│   │   ├── error.rs                  # AppError enum (serializable for Tauri IPC)
│   │   └── commands/
│   │       ├── mod.rs
│   │       ├── config.rs             # load_config / save_config Tauri commands
│   │       ├── git.rs                # git_pull, git_push, validate_repo, is_dirty
│   │       └── sync.rs               # start_sync Tauri command (orchestrates full workflow)
│   ├── Cargo.toml
│   ├── build.rs
│   └── tauri.conf.json
├── .github/workflows/ci.yml          # CI: cargo clippy + svelte-check + vitest
├── .gitignore
├── package.json
├── vite.config.ts
├── tsconfig.json
├── svelte.config.js
├── index.html
├── CLAUDE.md
└── README.md
```

## Key Architecture Decisions

### IPC Boundary
All `invoke()` calls live exclusively in `src/lib/ipc.ts`. No component calls `invoke()` directly. This makes the frontend/backend contract explicit and easy to refactor.

### Event-driven logging
Progress is reported via Tauri events (`sync://log`), not return values. The Rust backend emits `SyncEvent` objects; the frontend `LogViewer` subscribes and renders them.

### Sync lock
A `Mutex<bool>` in Tauri state (`Arc<Mutex<bool>>`) prevents concurrent sync runs. The frontend also disables the button during sync, but the Rust guard is authoritative.

### File mirroring
`commands/sync.rs` uses `walkdir` to traverse both repos. Files are copied from B to A, then stale files in A (not present in B) are deleted. The `.git/` directory is unconditionally excluded at every step.

## Development Commands

```bash
# Install frontend dependencies
npm install

# Dev mode (hot reload — Vite + Tauri together)
npm run tauri dev

# Type-check frontend
npm run check

# Run Rust checks
cd src-tauri && cargo check
cd src-tauri && cargo clippy --all-targets

# Run tests
npm test
```

## Key Conventions

- **Rust error handling**: Use `AppError` from `error.rs`. All command functions return `Result<T>` (the local type alias). Never panic in commands.
- **Path handling**: Always use `std::path::PathBuf` — never string-concatenate paths.
- **Async Rust**: File I/O in `sync.rs` runs in `spawn_blocking` to avoid blocking the async runtime.
- **Svelte reactivity**: Use Svelte 5 runes (`$state`, `$props`, `$bindable`). No Svelte stores.
- **CSS**: Design tokens live in `:root` in `app.css`. Components reference `var(--token-name)` — no hard-coded colors.

## Git Workflow

- Default branch: `main` (remote)
- AI task branches: `claude/<description>-<session-id>`
- Push with: `git push -u origin <branch-name>`
- Commit style: imperative, e.g. `Add confirmation dialog before sync`

## Adding New Tauri Commands

1. Add the handler function to the appropriate file in `src-tauri/src/commands/`
2. Register it in `src-tauri/src/lib.rs` `tauri::generate_handler![...]`
3. Add a typed wrapper in `src/lib/ipc.ts`
4. Update types in `src-tauri/src/models.rs` and `src/lib/ipc.ts` together

## Known Limitations / Future Work

- No "Test Connection" button per repo (to validate remote URL + auth before syncing)
- No support for cloning Repo B if it doesn't exist locally (currently errors with a clear message)
- No selective file exclusion (e.g. `.env` files that shouldn't be synced)
- Icons are placeholder solid-color PNGs — replace with real icons before distributing
