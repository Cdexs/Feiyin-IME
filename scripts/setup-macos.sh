#!/bin/bash
# macOS 开发环境一键初始化（幂等，可重复执行）
#
#   bash scripts/setup-macos.sh
#
# 完成后用 `source scripts/env-macos.sh` 载入环境变量，再 cargo check / build。
set -e

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

echo "=== voice-ime macOS 环境初始化 ==="

# --- 1. Xcode Command Line Tools（提供 clang / ld / 系统头文件）---
echo "[1/5] Xcode CLT..."
if ! xcode-select -p >/dev/null 2>&1; then
  echo "  未安装，触发 xcode-select --install（图形化安装，完成后重跑本脚本）"
  xcode-select --install
  exit 1
fi
echo "  OK: $(xcode-select -p)"

# --- 2. Rust 工具链 ---
echo "[2/5] Rust..."
export PATH="$HOME/.cargo/bin:$PATH"
if ! command -v rustc >/dev/null 2>&1; then
  echo "  安装 rustup（stable，不修改 PATH）..."
  curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile default --no-modify-path
  export PATH="$HOME/.cargo/bin:$PATH"
fi
echo "  OK: $(rustc -V)"

# --- 3. cmake（ctranslate2-sys / sentencepiece-sys 的 C++ 构建）---
echo "[3/5] cmake..."
if ! command -v cmake >/dev/null 2>&1; then
  command -v brew >/dev/null 2>&1 || { echo "  需要 Homebrew：https://brew.sh"; exit 1; }
  brew install cmake pkg-config
fi
echo "  OK: $(cmake --version | head -1)"

# --- 4. sherpa-onnx 预编译动态库（macOS 版，Windows 的 vendor 包用不了）---
echo "[4/5] sherpa-onnx prebuilt..."
# shellcheck source=./env-macos.sh
source "$REPO_ROOT/scripts/env-macos.sh"
if [ -d "$SHERPA_ONNX_LIB_DIR" ]; then
  echo "  OK: 已存在 $SHERPA_ONNX_LIB_DIR"
else
  ARCHIVE="${SHERPA_ONNX_PKG}.tar.bz2"
  URL="https://github.com/k2-fsa/sherpa-onnx/releases/download/v${SHERPA_ONNX_VERSION}/${ARCHIVE}"
  echo "  下载 $URL"
  cd "$REPO_ROOT/vendor/sherpa-onnx"
  curl -fL --progress-bar -o "$ARCHIVE" "$URL"
  tar xjf "$ARCHIVE"
  cd "$REPO_ROOT"
  echo "  OK: 解压到 vendor/sherpa-onnx/${SHERPA_ONNX_PKG}"
fi

# --- 5. 前端依赖（node_modules 含平台相关二进制，从 Windows 迁过来必须重装）---
# 用 npm ci 而非 npm install：按 package-lock.json 精确安装且不修改 lock，
# 避免在 macOS 上把 lock 里的 win32 optional 依赖条目删掉换 darwin 项
# （见 troubleshooting [NPM-LOCK-CROSSPLAT-001]，实测 +39/−462 行破坏 Windows 侧 npm ci）。
# npm ci 自身会先清空 node_modules，故无需手动检测/清理 Windows 版残留。
echo "[5/5] 前端依赖..."
command -v npm >/dev/null 2>&1 || { echo "  需要 Node.js 18+：brew install node"; exit 1; }
if ! (cd ui && npm ci); then
  echo
  echo "  [错误] npm ci 失败。当前 ui/package-lock.json 与 package.json 存在既有失同步" >&2
  echo "  （传递依赖 @emnapi/* 缺失/版本不符），属仓库既有缺陷，非本脚本引入。" >&2
  echo "  见 collab 任务 MACOS-FIX-NPMLOCK-001（修 lock 涉及共享文件、需 Gavin 决策，另立处理）。" >&2
  echo "  绝不回退到 npm install：那会改写 lock、破坏 Windows 侧跨平台构建（DEC-034）。" >&2
  exit 1
fi
echo "  OK: $(node -v) / npm $(npm -v)"

echo
echo "=== 完成 ==="
echo "下一步："
echo "  source scripts/env-macos.sh"
echo "  cargo check                                      # 主程序"
echo "  cargo check --manifest-path src-tauri/Cargo.toml # Tauri 后端"
echo "  cd ui && npm run build                           # 前端"
