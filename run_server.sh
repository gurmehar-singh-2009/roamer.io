#!/usr/bin/env bash
# Builds and runs the game server.
# Usage: ./run_server.sh [--debug]
# Builds optimized by default; --debug builds faster but runs slower.
# Logs go to server/server.log, not the terminal.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

PROFILE=release
FLAGS=(--release)
if [[ "${1:-}" == "--debug" ]]; then
    PROFILE=debug
    FLAGS=()
fi

# Build from the repo root: running cargo inside server/ picks up
# server/.cargo/config.toml, whose `-unroll-loops` flag current nightly rejects.
(cd "$ROOT" && cargo build -p server ${FLAGS[@]+"${FLAGS[@]}"})

# The server loads src/config/config.toml, cert.pem and key.pem relative to
# its working directory.
cd "$ROOT/server"
echo "Server running (game port ${GAME_PORT:-6767}, proof-of-work port ${POW_PORT:-3001}). Logs: server/server.log"
exec "$ROOT/target/$PROFILE/server"
