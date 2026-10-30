#!/bin/bash
# macOS 构建环境变量 —— 用 `source scripts/env-macos.sh` 载入，或被其他脚本 source。
#
# FLASH-P3-A（DEC-114）：输入法不再带任何本地模型 ⇒ sherpa-onnx 预编译库与 SHERPA_ONNX_LIB_DIR 覆盖已删除
# （仓库也不再有 .cargo/config.toml）。本文件只负责把 Rust 工具链放进 PATH。

# Rust 工具链（rustup 默认不改 PATH）
case ":$PATH:" in
  *":$HOME/.cargo/bin:"*) ;;
  *) export PATH="$HOME/.cargo/bin:$PATH" ;;
esac
