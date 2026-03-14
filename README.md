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
| Windows | `.msi` | Recommended installer |
| Windows | `_portable.zip` | No install needed, unzip and run |
| Linux | `.AppImage` | No install needed, just run |
| Linux | `.deb` | Debian / Ubuntu |

> **macOS first run**: Right-click → Open to bypass Gatekeeper.

---

## Build from source

### macOS

<details>
<summary>Expand macOS instructions</summary>

#### 1. Install Xcode Command Line Tools

```bash
xcode-select --install
```

#### 2. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

For a universal binary (Apple Silicon + Intel), add both targets:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

#### 3. Install Node.js

```bash
# via Homebrew (recommended)
brew install node

# or download from https://nodejs.org/
```

#### 4. Clone and install dependencies

```bash
git clone https://github.com/CornPrincess/sync_code.git
cd sync_code
npm install
```

#### 5. Run in development mode

```bash
npm run tauri dev
```

#### 6. Build release `.dmg`

```bash
# Single-arch (current machine only)
npm run tauri build

# Universal binary (runs on both Apple Silicon and Intel)
npm run tauri build -- --target universal-apple-darwin
```

Output: `src-tauri/target/release/bundle/macos/`

#### Troubleshooting

**`error while running tauri application: PluginInitialization("shell", ...)`**

This error means an outdated or invalid `plugins.shell.commands` block is present in `src-tauri/tauri.conf.json`. That field does not exist in Tauri v2 — the app uses `std::process::Command` directly to run `git`. Remove the `commands` array from `tauri.conf.json` (keep only `"plugins": {}`), and remove `tauri-plugin-shell` from `Cargo.toml` and `lib.rs` if it is present.

**`failed to run 'cargo metadata'` / `No such file or directory`**

Rust is not installed or not on `PATH`. Install via rustup (step 2 above), then run `source ~/.cargo/env` before retrying.

**`could not find Cargo.toml in ... or any parent directory`**

`Cargo.toml` lives inside `src-tauri/`, not the project root. Always run Cargo commands from that subdirectory:

```bash
cd src-tauri && cargo clean && cd ..
```

Or use the manifest flag from the project root:

```bash
cargo clean --manifest-path src-tauri/Cargo.toml
```

</details>

---

### Windows

<details>
<summary>Expand Windows instructions</summary>

#### 1. Install Rust

Download and run the installer from [rustup.rs](https://rustup.rs/).
Restart your terminal after installation.

Verify:
```powershell
rustc --version
cargo --version
```

#### 2. Install Visual Studio C++ Build Tools

Rust on Windows requires the MSVC linker. Install one of:

- **Option A** — [Visual Studio 2022](https://visualstudio.microsoft.com/) with the **Desktop development with C++** workload
- **Option B** — [Build Tools for Visual Studio 2022](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (lighter, no IDE)

During install, make sure **MSVC v143** and **Windows SDK** are selected.

#### 3. Install Node.js

Download the LTS installer from [nodejs.org](https://nodejs.org/) and run it.

Verify:
```powershell
node --version
npm --version
```

#### 4. Install WebView2 (usually pre-installed)

Tauri uses the system WebView2 runtime, which ships with Windows 10 (1803+) and Windows 11.
If missing, download from [Microsoft WebView2](https://developer.microsoft.com/microsoft-edge/webview2/).

#### 5. Clone and install dependencies

```powershell
git clone https://github.com/CornPrincess/sync_code.git
cd sync_code
npm install
```

#### 6. Run in development mode

```powershell
npm run tauri dev
```

#### 7. Build release installer

```powershell
npm run tauri build
```

Output: `src-tauri\target\release\bundle\`
- `msi\sync-code_x.x.x_x64_en-US.msi` — MSI installer
- `nsis\sync-code_x.x.x_x64-setup.exe` — NSIS installer
- `release\sync-code.exe` — portable executable (zip manually or use CI)

</details>

---

### Linux

<details>
<summary>Expand Linux instructions</summary>

#### 1. Install system dependencies

**Debian / Ubuntu:**
```bash
sudo apt-get update
sudo apt-get install -y \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev \
  patchelf \
  build-essential \
  curl \
  wget \
  file \
  libssl-dev \
  libayatana-appindicator3-dev
```

**Fedora / RHEL:**
```bash
sudo dnf install -y \
  gtk3-devel \
  webkit2gtk4.1-devel \
  libappindicator-gtk3-devel \
  librsvg2-devel \
  patchelf \
  openssl-devel \
  curl \
  wget \
  file
```

**Arch Linux:**
```bash
sudo pacman -S --needed \
  gtk3 \
  webkit2gtk-4.1 \
  libappindicator-gtk3 \
  librsvg \
  patchelf \
  openssl \
  curl \
  wget \
  file \
  base-devel
```

#### 2. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

#### 3. Install Node.js

```bash
# via nvm (recommended)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash
source ~/.bashrc   # or ~/.zshrc
nvm install 20
nvm use 20

# or via system package manager (Ubuntu)
sudo apt-get install -y nodejs npm
```

#### 4. Clone and install dependencies

```bash
git clone https://github.com/CornPrincess/sync_code.git
cd sync_code
npm install
```

#### 5. Run in development mode

```bash
npm run tauri dev
```

> **Headless / SSH sessions**: A display server is required. Use `Xvfb` or connect with X11 forwarding (`ssh -X`).

#### 6. Build release packages

```bash
npm run tauri build
```

Output: `src-tauri/target/release/bundle/`
- `deb/sync-code_x.x.x_amd64.deb` — Debian package
- `appimage/sync-code_x.x.x_amd64.AppImage` — portable AppImage

**Install the `.deb`:**
```bash
sudo dpkg -i src-tauri/target/release/bundle/deb/sync-code_*.deb
```

**Run the `.AppImage` directly:**
```bash
chmod +x sync-code_*.AppImage
./sync-code_*.AppImage
```

</details>

---

## Development commands

```bash
npm run tauri dev   # dev mode with hot reload
npm run check       # TypeScript + Svelte type check
npm test            # run unit tests
cd src-tauri && cargo clippy --all-targets   # Rust lints
```

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

GitHub Actions automatically builds installers for all three platforms and publishes them as a GitHub Release.

## License

MIT — see [LICENSE](LICENSE)
