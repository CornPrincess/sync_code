#!/bin/sh
# Start sync-code in web mode.
# Requires: Rust / Cargo (https://rustup.rs)
# No npm needed — the pre-built frontend is included in the repository.
#
# Usage:  ./start-web.sh
# Then open http://localhost:8080 in your browser.

set -e
cd "$(dirname "$0")"

echo "Building sync-code web server (first run compiles Rust, takes a few minutes)..."
cargo run --manifest-path src-tauri/Cargo.toml --bin web-server --features web
