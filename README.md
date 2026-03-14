# sync_code

A simple cross-platform desktop app that syncs code from one git repository into another.

## What it does

Given two repositories **A** (target) and **B** (source):

1. Pulls the latest code from Repo B's configured branch
2. Mirrors Repo B's files into Repo A (overwriting, handling deletions)
3. Commits and pushes the result to Repo A's remote branch

## Tech Stack

- **Desktop**: [Tauri v2](https://tauri.app/) (Rust backend + system WebView)
- **Frontend**: [Svelte 5](https://svelte.dev/) + TypeScript
- **Build tool**: Vite 8
- **Git operations**: Shell `git` commands (inherits your existing SSH/credential setup)

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- [Node.js](https://nodejs.org/) 20+
- `git` available in your PATH
- **Linux only**: GTK3 and WebKit2GTK system libraries
  ```bash
  sudo apt-get install libgtk-3-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
  ```

## Development

```bash
# Install frontend dependencies
npm install

# Run in development mode (hot reload)
npm run tauri dev

# Type-check the frontend
npm run check

# Run tests
npm test
```

## Build

```bash
# Build release installer for the current platform
npm run tauri build
```

Installers are placed in `src-tauri/target/release/bundle/`.

## Usage

1. Fill in **Repo A** (target): local path, remote URL, and branch
2. Fill in **Repo B** (source): local path, remote URL, and branch
3. Click **Sync Now**

Config is saved automatically to your system's application config directory:
- **macOS**: `~/Library/Application Support/com.cornprincess.synccode/config.json`
- **Linux**: `~/.config/com.cornprincess.synccode/config.json`
- **Windows**: `%APPDATA%\com.cornprincess.synccode\config.json`

## Notes

- Repo A must have **no uncommitted changes** before syncing — the app will abort with a clear error if it detects a dirty state
- The `.git/` directory in Repo A is never touched
- SSH authentication uses your existing SSH agent — launch the app from a terminal that has your agent loaded
- Git credential helpers (macOS Keychain, Windows Credential Manager, etc.) work automatically

## License

MIT — see [LICENSE](LICENSE)
