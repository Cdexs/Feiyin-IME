#!/bin/bash
# macOS 一键构建 + .app 打包（MACOS-P4-BUNDLE-001/002）
#
#   bash scripts/build-macos.sh                 # 完整 release 构建 + .app 打包 + 自签名证书签名
#   bash scripts/build-macos.sh --no-app        # 只构建不打包（老行为）
#   bash scripts/build-macos.sh --copy-models   # 复制 models 进 bundle（默认 symlink，节省磁盘）
#
# 产物：dist/飞音智能语音输入.app/
# 签名：自签名证书（默认证书名 "Feiyin Dev"，可用 CODESIGN_IDENTITY 覆盖），本地调试用，不做公证。
#   ⚠️ 禁止 ad-hoc 签名：ad-hoc 每次重签 cdhash 都变，macOS 会把重建产物当成另一个 App，
#      导致辅助功能/麦克风授权每次构建后都失效，需反复重新授权。自签名证书提供稳定签名身份，
#      TCC 按 designated requirement 记住授权，重建重签后授权依然有效。
#   证书不存在时脚本会明确报错退出并给出创建指引，不会静默退化为 ad-hoc。
set -e

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# ---- 签名身份（BUNDLE-002：自签名证书，禁止 ad-hoc）----
# 默认 "Feiyin Dev"；可环境变量覆盖：CODESIGN_IDENTITY=<证书名> bash scripts/build-macos.sh
CODESIGN_IDENTITY="${CODESIGN_IDENTITY:-Feiyin Dev}"

# ---- 参数解析 ----
PACK_APP=1
COPY_MODELS=0
for arg in "$@"; do
  case "$arg" in
    --no-app) PACK_APP=0 ;;
    --copy-models) COPY_MODELS=1 ;;
    *) echo "未知参数: $arg（支持 --no-app / --copy-models）" >&2; exit 1 ;;
  esac
done

echo "=== voice-ime macOS Build ==="

# 载入 Rust PATH 与 SHERPA_ONNX_LIB_DIR（覆盖 .cargo/config.toml 里的 Windows 路径）
# shellcheck source=./env-macos.sh
source "$REPO_ROOT/scripts/env-macos.sh"

# Kill running instance
pkill -f feiyin-ime || true

# ---- Step 1: 主程序 + crash-reporter ----
echo "[1/4] cargo build --release (main + crash-reporter)..."
cargo build --release

# ---- Step 2: Tauri 设置界面（必须带 custom-protocol，否则 UI 空白页）----
echo "[2/4] npm run build + Tauri UI (custom-protocol)..."
cd ui && npm run build && cd ..
cargo build --release --manifest-path "$REPO_ROOT/src-tauri/Cargo.toml" --features custom-protocol
cp src-tauri/target/release/feiyin-ime-ui target/release/feiyin-ime-ui

# ---- Step 3: 验证 ----
echo "[3/4] Verifying artifacts..."
ls -lh target/release/feiyin-ime
ls -lh target/release/crash-reporter
ls -lh target/release/feiyin-ime-ui
ls -lh ui/dist/

# ---- Step 4: .app 打包 + 签名 ----
if [ "$PACK_APP" = "1" ]; then
  echo "[4/4] Packaging .app bundle..."

  # 证书存在性检查（BUNDLE-002）：自签名证书必须存在，禁止静默退化为 ad-hoc。
  # 注意：不用 -v 过滤——GUI 创建的自签名证书默认 CSSMERR_TP_NOT_TRUSTED，
  # -v 只显示受信任 identity，会把已存在的证书误判为"不存在"。
  if ! security find-identity -p codesigning | grep -q "$CODESIGN_IDENTITY"; then
    cat >&2 <<'HINT'
[错误] 未找到代码签名证书，且本脚本禁止使用 ad-hoc 签名。
原因：ad-hoc 每次重签 cdhash 都变，macOS 会当成另一个 App，
      辅助功能/麦克风授权每次构建后都会失效，需反复重新授权。

请先一次性创建自签名证书：
  钥匙串访问 → 证书助理 → 创建证书
    名称：Feiyin Dev
    身份类型：自签名根证书
    证书类型：代码签名
创建后重跑本脚本；也可用 CODESIGN_IDENTITY=<证书名> 覆盖默认值。
HINT
    exit 1
  fi
  echo "  codesign identity: $CODESIGN_IDENTITY"

  APP_DIR="dist/飞音智能语音输入.app"
  MACOS_DIR="$APP_DIR/Contents/MacOS"
  RES_DIR="$APP_DIR/Contents/Resources"

  rm -rf "$APP_DIR"
  mkdir -p "$MACOS_DIR" "$RES_DIR"

  # 版本号取自 Cargo.toml（单一事实来源），Info.plist 里同步
  VERSION="$(grep '^version' Cargo.toml | head -1 | sed -E 's/.*"([0-9]+\.[0-9]+\.[0-9]+)".*/\1/')"
  if [ -z "$VERSION" ]; then
    echo "  [错误] 无法从 Cargo.toml 提取版本号" >&2
    exit 1
  fi
  echo "  bundle version = $VERSION"

  # 三个可执行文件（spawn_settings_process 从 exe_dir 找 feiyin-ime-ui，
  # crash reporter 用 with_file_name("crash-reporter")，均须与主程序同目录）
  cp target/release/feiyin-ime "$MACOS_DIR/feiyin-ime"
  cp target/release/feiyin-ime-ui "$MACOS_DIR/feiyin-ime-ui"
  cp target/release/crash-reporter "$MACOS_DIR/crash-reporter"

  # 动态库（主程序 @rpath 链接，需与主程序同目录并注入 @loader_path）
  for d in libctranslate2.dylib libctranslate2.4.dylib libctranslate2.4.6.0.dylib \
           libsherpa-onnx-c-api.dylib libsherpa-onnx-cxx-api.dylib \
           libonnxruntime.dylib libonnxruntime.1.24.4.dylib; do
    if [ -f "target/release/$d" ]; then
      cp "target/release/$d" "$MACOS_DIR/$d"
    else
      echo "  [警告] 缺少动态库 target/release/$d" >&2
    fi
  done

  # 主程序注入 @loader_path rpath（@rpath/lib*.dylib → 同目录解析）
  # codesign 需在 install_name_tool 之后执行，否则签名失效
  install_name_tool -add_rpath @loader_path "$MACOS_DIR/feiyin-ime" \
    && echo "  @loader_path rpath injected"

  # 运行时从 exe_dir 加载的数据文件。
  # 注意：必须放 Contents/Resources/（标准布局），并在 MacOS/ 内做相对 symlink——
  # 若直接放 MacOS/ 下，codesign 会把 .toml 当嵌套代码组件，外层签名报
  # "code object is not signed at all"；相对 symlink 目标在 bundle 内，strict 验证也能过。
  cp itn-rules.toml "$RES_DIR/itn-rules.toml"
  cp scene-rules.toml "$RES_DIR/scene-rules.toml"
  ln -s ../Resources/itn-rules.toml "$MACOS_DIR/itn-rules.toml"
  ln -s ../Resources/scene-rules.toml "$MACOS_DIR/scene-rules.toml"

  # models：默认 symlink 指向仓库（本地调试快速，节省 1.8G 磁盘，仅宽松验证）；
  # --copy-models 复制进 bundle 内 Resources 并做相对 symlink（严格验证通过，可分发）
  if [ "$COPY_MODELS" = "1" ]; then
    echo "  copying models ($(du -sh models | awk '{print $1}'))..."
    cp -R models "$RES_DIR/models"
    ln -s ../Resources/models "$MACOS_DIR/models"
    echo "  models -> bundle Resources/models"
  else
    ln -sfn "$REPO_ROOT/models" "$MACOS_DIR/models"
    echo "  models -> symlink $REPO_ROOT/models（本地调试模式）"
  fi

  # 图标
  cp src-tauri/icons/icon.icns "$RES_DIR/icon.icns"

  # Info.plist（版本号同步 Cargo.toml：先在副本上改，避免污染 scripts/Info.plist 模板）
  cp scripts/Info.plist "$APP_DIR/Contents/Info.plist"
  /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" -c "Set :CFBundleVersion $VERSION" "$APP_DIR/Contents/Info.plist"

  # 签名（BUNDLE-002）：先内后外，去掉已废弃的 --deep（Apple 官方不推荐，嵌套代码应各自签名）
  # 顺序：dylib → 外层 app。codesign 必须在 install_name_tool 之后执行（改二进制会使签名失效）。
  echo "  codesign nested dylibs..."
  for f in "$MACOS_DIR"/*.dylib; do
    [ -e "$f" ] || continue
    codesign --force --timestamp=none --sign "$CODESIGN_IDENTITY" "$f"
  done
  echo "  codesign app bundle..."
  codesign --force --timestamp=none --sign "$CODESIGN_IDENTITY" "$APP_DIR"

  # 验证签名：--copy-models 时 models 全部在 bundle 内，可严格验证；
  # 默认 symlink 指向仓库，codesign 严格验证会对 bundle 外 symlink 报
  # "invalid destination for symbolic link in bundle"，此时降级宽松验证（本地调试可接受）。
  if [ "$COPY_MODELS" = "1" ]; then
    codesign --verify --deep --strict "$APP_DIR" \
      && echo "  codesign strict verify OK" || { echo "  [错误] 签名验证失败" >&2; exit 1; }
  else
    if codesign --verify --deep --strict "$APP_DIR" 2>/dev/null; then
      echo "  codesign strict verify OK"
    else
      codesign --verify --deep "$APP_DIR" \
        && echo "  codesign verify OK（宽松；models 为仓库 symlink，strict 对本场景无意义）"
    fi
  fi
  codesign -dv --verbose=4 "$APP_DIR" 2>&1 | grep -E "Signature|Identifier|Authority" || true

  echo "  ✅ 产物: $APP_DIR"
fi

echo "=== Build Complete ==="
