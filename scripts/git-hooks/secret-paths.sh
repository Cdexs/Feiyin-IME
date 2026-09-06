#!/usr/bin/env bash
# voice-ime 路径闸门共享判定（SECRET-126，被 pre-commit / pre-push 同时 source）
#
# 背景：Gavin 2026-09-06 指令「确保本地配置文件和代码不会提交任何 key、和其他隐私敏感信息」。
# 主控审计确认当前没有正在泄露的东西，本文件补的是 latent 洞：仓库根目录一旦出现
# /config.toml（真实 api_key）、/.env、/debug.log（口述转写原文 + app_exe），
# `git add -A` 会直接抓进去 —— 而 .gitignore 挡不住 `git add -f`，也挡不住有人删规则。
#
# 为什么判据是路径不是内容：[GATE-MATCHES-SHAPE-NOT-CONTENT-001] 已写死教训 ——
# 口述语料 / 实验 dump 这类「文件类别本身就危险」的东西是内容正则的永久盲区，
# 唯一可靠的判据是路径。与 secret-patterns.sh（内容闸门）是两层，职责分开、互不掺和。
#
# 为什么单独成文件而不是塞进 secret-patterns.sh：SECRET-082 的原始教训就是
# 「两份拷贝必然漂移」，pre-commit / pre-push 必须共用同一份路径判定；
# 同时不碰 secret-patterns.sh 的现有模式表，保持两层边界清晰。
#
# 只许被 source，不许直接执行（没有 main 逻辑）。
# 统一逃生口：SECRET_SCAN_SKIP=1（与内容闸门同名同语义，别另起变量名）。
#
# 调用约定：
#   - 输入：stdin = 以换行分隔的路径（git 原生输出，仓库相对路径、正斜杠）
#   - 调用方必须用 `-c core.quotePath=false` 取路径，否则中文路径被 C 转义成引号串无法匹配
#   - 输出：stdout = 每行「路径 — 危险原因」，调用方 [ -n ] 判定后原样打印给用户

set -u

# scan_paths —— 逐行分类路径，命中即输出。
# 匹配统一转小写：Windows 文件系统大小写不敏感，CONFIG.TOML 和 config.toml 是同一个文件，
# 闸门若按大小写敏感匹配，换种写法就能绕过。
scan_paths() {
  local p dir base lbase ldir reason
  while IFS= read -r p; do
    [ -z "$p" ] && continue
    # git index 路径恒为正斜杠，直接拆目录与文件名
    dir="${p%/*}"
    [ "$dir" = "$p" ] && dir=""
    base="${p##*/}"
    lbase="$(printf '%s' "$base" | tr '[:upper:]' '[:lower:]')"
    ldir="$(printf '%s' "$dir" | tr '[:upper:]' '[:lower:]')"
    reason=""
    # config.toml 在**任何目录**都拦（不止根目录）—— 真正装着 api_key 的是
    # target/release/ 与 Publish/ 那两份运行时副本，它们此前只靠 .gitignore 挡，
    # git add -f 即可绕过（2026-09-06 Gavin 追问配置入库问题时主控发现的缺口）。
    # 唯一例外：*/.cargo/config.toml 是 Cargo 构建配置，不含凭证，且已入库。
    if [ "$lbase" = "config.toml" ] && [ "${ldir##*/}" != ".cargo" ]; then
      reason="运行时配置（根目录/target/Publish 副本），含真实 api_key，只许本地留存"
    elif case "$lbase" in .env|*.env|.env.*) true ;; *) false ;; esac; then
      reason="环境变量文件，凭证专用载体"
    elif case "$lbase" in *.log) true ;; *) false ;; esac; then
      # logs/*.md 的改动日志不受影响 —— .log 只匹配日志扩展名，不匹配 .md
      reason="运行日志，载荷含口述转写原文等隐私（内容正则拦不住，只能靠路径拦）"
    elif case "$lbase" in *.sqlite|*.sqlite-shm|*.sqlite-wal|*.db|*.db-shm|*.db-wal) true ;; *) false ;; esac; then
      reason="用户数据库（词库等隐私数据），只许本地留存"
    elif [ "$lbase" = "version_check.json" ]; then
      reason="运行时产物，无入库价值"
    elif case "$ldir" in collab/evidence|collab/evidence/*|collab/research|collab/research/*) true ;; *) false ;; esac; then
      # 事故目录（[SECRET-IN-REPO-001]）：口述语料 / LLM 实验 dump，即使被 -f 强加也拒绝
      reason="collab 事故目录（口述语料 / 实验 dump，内容正则的永久盲区）"
    fi
    [ -n "$reason" ] && printf '%s — %s\n' "$p" "$reason"
  done
}

# print_path_guidance —— 路径闸门拦截后给用户的处理指引（两个钩子共用）
print_path_guidance() {
  cat >&2 <<MSG

处理方式（按优先级）：
  1. 这些文件按「路径」即判危险，放本地目录即可，不要入库；
  2. 确需入库的配置请写脱敏模板（参照 assets/default-config.toml，api_key 留空占位）；
  3. 如确认是误报，用 SECRET_SCAN_SKIP=1 显式放行（会打印警告，留痕）。
MSG
}
