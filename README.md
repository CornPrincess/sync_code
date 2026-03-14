# sync_code

A simple cross-platform desktop app that syncs code from one git repository into another.

## What it does

Given two repositories **A** (target) and **B** (source):

1. Pulls the latest code from Repo B's configured branch
2. Mirrors Repo B's files into Repo A (overwriting, handling deletions)
3. Commits and pushes the result to Repo A's remote branch

## Download (pre-built)

Go to the [Releases](../../releases) page and download the installer for your platform:

| Platform | File | Notes |
|---|---|---|
| macOS | `.dmg` | Universal binary (Apple Silicon + Intel) |
| Windows | `.msi` | Recommended |
| Linux | `.AppImage` | No install needed, just run |
| Linux | `.deb` | Debian / Ubuntu |

> **macOS first run**: Right-click → Open to bypass Gatekeeper.

---

## Build from source

### Prerequisites

| Tool | Install |
|---|---|
| Rust (stable) | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Node.js 20+ | [nodejs.org](https://nodejs.org/) or `brew install node` |
| git | Pre-installed on most systems |
| **Linux only** — GTK3 / WebKit | See below |

**Linux system dependencies:**
```bash
sudo apt-get install libgtk-3-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

**After installing Rust**, reload your shell or run:
```bash
source ~/.cargo/env   # macOS / Linux
# Windows: restart terminal
```

### Development

```bash
# 1. Install frontend dependencies
npm install

# 2. Run in dev mode (hot reload)
npm run tauri dev

# Type-check only
npm run check

# Run tests
npm test
```

### Build release installer

```bash
# Builds installer for the current platform
npm run tauri build
```

Output is placed in `src-tauri/target/release/bundle/`:
- macOS: `macos/*.dmg`, `macos/*.app`
- Windows: `msi/*.msi`, `nsis/*-setup.exe`
- Linux: `deb/*.deb`, `appimage/*.AppImage`

---

## Usage

1. Fill in **Repo A** (target): local path, remote URL, branch
2. Fill in **Repo B** (source): local path, remote URL, branch
3. Click **Sync Now**

Configuration is saved automatically:
- **macOS**: `~/Library/Application Support/com.cornprincess.synccode/config.json`
- **Linux**: `~/.config/com.cornprincess.synccode/config.json`
- **Windows**: `%APPDATA%\com.cornprincess.synccode\config.json`

## Notes

- Repo A must have **no uncommitted changes** before syncing — the app aborts with a clear error if dirty
- The `.git/` directory in Repo A is never modified
- SSH authentication works automatically via your existing SSH agent — launch the app from a terminal that has the agent loaded
- Git credential helpers (macOS Keychain, Windows Credential Manager) work automatically

## Releasing a new version

```bash
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions will automatically build installers for all three platforms and publish them as a GitHub Release.

## License

MIT — see [LICENSE](LICENSE)
