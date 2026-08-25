#!/usr/bin/env bash
# Live-reload dev loop for the DeepMate desktop app (Tauri + React).
#
# The desktop app moved from Slint to Tauri 2: the React frontend runs under
# Vite (which hot-reloads .tsx/.ts/.css edits in place), and the Rust backend
# lives in src-tauri as its own workspace. `npm run tauri dev` wires the two
# together and restarts the Rust process when src-tauri sources change.
#
# Usage:
#   scripts/dev-watch.sh   # start the Tauri dev loop
#
# Requirements: Node.js 20+ and a Rust toolchain. First run installs frontend
# deps with `npm install`.

set -euo pipefail

cd "$(dirname "$0")/../apps/desktop"

if [[ ! -d node_modules ]]; then
    echo "node_modules missing — running npm install first…"
    npm install
fi

# `tauri dev` runs the Vite dev server (HMR) and rebuilds/restarts the Rust
# backend on change. It is the replacement for the old watchexec + cargo run
# loop used in the Slint era.
exec npm run tauri dev
