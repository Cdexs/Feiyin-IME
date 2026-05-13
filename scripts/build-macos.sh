#!/bin/bash
set -e

echo "=== voice-ime macOS Build ==="

# Kill running instance
pkill -f voice-ime || true

# Build main binary
echo "[1/3] cargo build --release..."
cargo build --release

# Build Tauri UI
echo "[2/3] npm run build..."
cd ui && npm run build && cd ..

# Verify
echo "[3/3] Verifying artifacts..."
ls -lh target/release/voice-ime
ls -lh ui/dist/

echo "=== Build Complete ==="