@echo off
REM Start sync-code in web mode.
REM Requires: Rust / Cargo  https://rustup.rs
REM No npm needed — the pre-built frontend is included in the repository.
REM
REM Usage:  double-click start-web.bat  OR  run in CMD / PowerShell
REM Then open http://localhost:8080 in your browser.

cd /d "%~dp0"
echo Building sync-code web server (first run compiles Rust, takes a few minutes)...
cargo run --manifest-path src-tauri\Cargo.toml --bin web-server --features web
pause
