#!/bin/bash
set -e

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

echo "=== voice-ime macOS Build ==="

# 载入 Rust PATH 与 SHERPA_ONNX_LIB_DIR（覆盖 .cargo/config.toml 里的 Windows 路径）
# shellcheck source=./env-macos.sh
source "$REPO_ROOT/scripts/env-macos.sh"

# Kill running instance
pkill -f feiyin-ime || true

# Build main binary
echo "[1/3] cargo build --release..."
cargo build --release

# Build Tauri UI
echo "[2/3] npm run build..."
cd ui && npm run build && cd ..

# Verify
echo "[3/3] Verifying artifacts..."
ls -lh target/release/feiyin-ime
ls -lh ui/dist/

echo "=== Build Complete ==="
