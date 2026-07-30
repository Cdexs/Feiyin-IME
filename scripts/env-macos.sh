#!/bin/bash
# macOS 构建环境变量 —— 用 `source scripts/env-macos.sh` 载入，或被其他脚本 source。
#
# 为什么需要它：.cargo/config.toml 的 [env] 段把 SHERPA_ONNX_LIB_DIR 写死成 Windows 路径
# (D:\Workspace\...)，在 macOS 上会让 sherpa-onnx-sys 的 build.rs 直接 panic。
# cargo 的 [env] 默认 force = false —— shell 里已导出的值优先，所以这里覆盖即可，
# 不必改动 config.toml（改了会破坏 Windows 侧构建）。

SHERPA_ONNX_VERSION="1.12.38"

_env_macos_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"

# Rust 工具链（rustup 默认不改 PATH）
case ":$PATH:" in
  *":$HOME/.cargo/bin:"*) ;;
  *) export PATH="$HOME/.cargo/bin:$PATH" ;;
esac

case "$(uname -m)" in
  arm64) SHERPA_ONNX_ARCH="arm64" ;;
  *)     SHERPA_ONNX_ARCH="x64" ;;
esac

export SHERPA_ONNX_PKG="sherpa-onnx-v${SHERPA_ONNX_VERSION}-osx-${SHERPA_ONNX_ARCH}-shared-lib"
export SHERPA_ONNX_LIB_DIR="${_env_macos_root}/vendor/sherpa-onnx/${SHERPA_ONNX_PKG}/lib"

unset _env_macos_root
