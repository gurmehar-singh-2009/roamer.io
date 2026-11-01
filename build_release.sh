#!/usr/bin/env bash
# Builds everything and puts a ready-to-run copy of the game in release/:
#   release/bin/roamer-server   the game server (static Linux x86-64 binary)
#   release/bin/roamer-client   serves the web client (static binary)
#   release/web/                the built web client
#   release/config/config.toml  game settings
#   release/roamer.sh           sets everything up and starts both
#
# Usage: ./build_release.sh
# Needs: Rust (nightly), wasm-pack, bun or npm, clang (for the static build).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$ROOT/release"
TARGET=x86_64-unknown-linux-musl

cd "$ROOT"

echo "==> Rust target $TARGET"
rustup target add "$TARGET" >/dev/null

echo "==> Building the server and the client host (static)"
# The C parts (ring, mimalloc) are built with clang for musl.
CC_x86_64_unknown_linux_musl="${CC_x86_64_unknown_linux_musl:-clang}" \
CFLAGS_x86_64_unknown_linux_musl="${CFLAGS_x86_64_unknown_linux_musl:---target=$TARGET}" \
    cargo build --release --target "$TARGET" -p server -p roamer-client

echo "==> Building the packet parser (WebAssembly)"
(cd web/parser && wasm-pack build --target web --release)

echo "==> Building the web client"
cd web
if command -v bun >/dev/null 2>&1; then PM=bun; else PM=npm; fi
[[ -d node_modules ]] || "$PM" install
"$PM" run build -- --outDir dist --emptyOutDir
cd "$ROOT"

echo "==> Assembling $OUT"
mkdir -p "$OUT/bin" "$OUT/config"
rm -rf "$OUT/web"
cp "target/$TARGET/release/server" "$OUT/bin/roamer-server"
cp "target/$TARGET/release/roamer-client" "$OUT/bin/roamer-client"
cp -r web/dist "$OUT/web"
cp server/src/config/config.toml "$OUT/config/config.toml"
chmod +x "$OUT/roamer.sh" "$OUT/bin/"*

echo
echo "Done. Start it with:"
echo "  $OUT/roamer.sh"
