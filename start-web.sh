#!/bin/sh
# Start sync-code in web mode (Node.js server).
# Requires: Node.js 18+  https://nodejs.org
# No Rust / Cargo needed. The pre-built frontend is included in the repository.
#
# Usage:  ./start-web.sh
# The browser opens automatically at http://localhost:8080

set -e
cd "$(dirname "$0")"
node web-server.js
