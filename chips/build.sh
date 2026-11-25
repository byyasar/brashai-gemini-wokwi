#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MANIFEST="$SCRIPT_DIR/Cargo.toml"

echo "[1/2] Wokwi custom chip'ler wasm32-unknown-unknown hedefi icin derleniyor..."
RUSTFLAGS="-Clink-arg=--export-table" cargo build --manifest-path "$MANIFEST" --target wasm32-unknown-unknown --release

RELEASE_DIR="$SCRIPT_DIR/target/wasm32-unknown-unknown/release"

cp "$RELEASE_DIR/vs1053.wasm" "$SCRIPT_DIR/vs1053.chip.wasm"
cp "$RELEASE_DIR/sd_board.wasm" "$SCRIPT_DIR/sd-board.chip.wasm"
cp "$RELEASE_DIR/speaker.wasm" "$SCRIPT_DIR/speaker.chip.wasm"
cp "$RELEASE_DIR/fan.wasm" "$SCRIPT_DIR/fan.chip.wasm"

# Wokwi VS Code eklentisinin bulabilmesi icin proje kokune de kopyala
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cp -f "$SCRIPT_DIR"/*.chip.* "$ROOT_DIR/"

echo "[2/2] Uretilen .chip.wasm dosyalari:"
ls -lh "$SCRIPT_DIR"/*.chip.wasm
