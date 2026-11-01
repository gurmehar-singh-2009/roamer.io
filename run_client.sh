#!/usr/bin/env bash
# Builds the WASM packet parser and starts the web client dev server.
# Usage: ./run_client.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Rebuild the parser the client imports from web/parser/pkg.
(cd "$ROOT/web/parser" && wasm-pack build --target web --release)

cd "$ROOT/web"

if command -v bun >/dev/null 2>&1; then
    PM=bun
else
    PM=npm
fi

if [[ ! -d node_modules ]]; then
    "$PM" install
fi

# The server only allows requests from http://localhost:5173 (or
# CLIENT_ORIGIN), so fail instead of silently moving to another port.
exec "$PM" run dev -- --port "${CLIENT_PORT:-5173}" --strictPort
