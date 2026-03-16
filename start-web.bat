@echo off
REM Start sync-code in web mode (Node.js server).
REM Requires: Node.js 18+  https://nodejs.org
REM No Rust / Cargo needed. The pre-built frontend is included in the repository.
REM
REM Usage:  double-click start-web.bat   OR   run in CMD / PowerShell:
REM   node web-server.js
REM
REM The browser opens automatically at http://localhost:8080

cd /d "%~dp0"
node web-server.js
pause
