#!/usr/bin/env bash
# voice-ime 密钥/隐私扫描共享模式表（被 pre-commit / pre-push 同时 source）
#
# 背景：SECRET-082。原 pre-commit 内联正则，pre-push 建起来后两边各抄一份必然漂移 ——
# 某天给 pre-commit 加了新前缀、pre-push 没加，commit 拦得住的 push 反而放过去，
# 比没有闸门更危险（你以为有）。所以模式表和扫描函数只能有一份，两个钩子都 source 它。
#
# 只许被 source，不许直接执行（没有 main 逻辑）。
# 统一逃生口：SECRET_SCAN_SKIP=1（两个钩子同名同语义，别另起变量名）。

set -u

# ---- 模式表 -----------------------------------------------------------

# 已知密钥前缀（几乎不会误报）
SECRET_PREFIX_PATTERNS='sk-[A-Za-z0-9]{20,}|sk-ant-[A-Za-z0-9_-]{20,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z_-]{35}|xox[baprs]-[A-Za-z0-9-]{10,}|glpat-[A-Za-z0-9_-]{20,}'

# 通用赋值形态（key/token/secret/password = 长随机串）
SECRET_GENERIC_PATTERN='(api[_-]?key|apikey|access[_-]?token|auth[_-]?token|secret[_-]?key|client[_-]?secret|password)["'"'"' ]*[:=]["'"'"' ]*[A-Za-z0-9_/+=-]{20,}'

# 隐私信息：邮箱 / 中国手机号 / 身份证 18 位
PRIVACY_EMAIL_PATTERN='[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}'
# SECRET-104：边界用「非字母数字」而非「非数字」—— Cargo.lock 等文件的十六进制校验和
# 里 1[3-9] 开头的 11 位连号必被 hex 字母包夹（如 …a19749016521d…），旧边界会误报拦截。
# 代价（如实）：紧贴字母/数字的手机号（如 id13812345678）会漏 —— 真实文本不这么写，可接受。
PRIVACY_PHONE_PATTERN='(^|[^0-9A-Za-z])1[3-9][0-9]{9}([^0-9A-Za-z]|$)'
PRIVACY_IDCARD_PATTERN='[1-9][0-9]{5}(19|20)[0-9]{2}(0[1-9]|1[0-2])(0[1-9]|[12][0-9]|3[01])[0-9]{3}[0-9Xx]'

# ---- 白名单（占位符 + 合法邮箱）---------------------------------------

# 占位符 / 示例 / 环境变量引用 —— 出现这些词的行不算真值
SECRET_PLACEHOLDER='REDACTED|PLACEHOLDER|YOUR[_-]?|xxxx|EXAMPLE|CHANGE[_-]?ME|<[^>]*>|\*\*\*|\$\{|%[A-Z_]+%|env\.|getenv|process\.env|std::env'

# 邮箱白名单（这些地址在仓库里合法存在，拦它们等于把每次 push 全拦死）
#  - cdexs@hotmail.com        : Gavin 本人提交作者字段
#  - gavinshare6@outlook.com  : Gavin 本人，git author 字段
#  - noreply / users.noreply  : 第三方依赖里的自动生成地址
#  - *.example.com            : 文档/测试示例域名
SECRET_EMAIL_WHITELIST='cdexs@hotmail\.com|gavinshare6@outlook\.com|noreply@[A-Za-z0-9.-]+|@users\.noreply\.github\.com|@[A-Za-z0-9.-]+\.example\.com'

# ---- 扫描函数 ---------------------------------------------------------

# scan_diff <diff_text> —— 对一段 `git diff -U0 --diff-filter=ACM` 的输出扫全部模式，
# 命中则把分类 + 行号写入 stdout，调用方用 $() 捕获后判非空即拦截。
# 输入：stdin = diff 文本（已 grep ^+ 过滤、已 grep -v ^+++ 去掉文件头）
# 输出：分类块拼接到 stdout，调用方 [ -n ] 判定
scan_diff() {
  local DIFF="$1"
  [ -z "$DIFF" ] && return 0

  local HITS=""

  # 1. 已知密钥前缀（不做白名单，前缀型几乎不误报）
  local P_HIT
  P_HIT=$(printf '%s\n' "$DIFF" | grep -nE "$SECRET_PREFIX_PATTERNS" || true)
  [ -n "$P_HIT" ] && HITS="${HITS}
[已知密钥前缀]
${P_HIT}"

  # 2. 通用赋值型，先匹赋值形态再排占位符
  local G_HIT
  G_HIT=$(printf '%s\n' "$DIFF" | grep -inE "$SECRET_GENERIC_PATTERN" | grep -viE "$SECRET_PLACEHOLDER" || true)
  [ -n "$G_HIT" ] && HITS="${HITS}
[疑似凭证赋值]
${G_HIT}"

  # 3. 隐私组：邮箱（先匹再排白名单）
  local E_HIT
  E_HIT=$(printf '%s\n' "$DIFF" | grep -nE "$PRIVACY_EMAIL_PATTERN" | grep -viE "$SECRET_EMAIL_WHITELIST" || true)
  [ -n "$E_HIT" ] && HITS="${HITS}
[隐私-邮箱]
${E_HIT}"

  # 4. 隐私组：中国手机号
  local PH_HIT
  PH_HIT=$(printf '%s\n' "$DIFF" | grep -nE "$PRIVACY_PHONE_PATTERN" || true)
  [ -n "$PH_HIT" ] && HITS="${HITS}
[隐私-手机号]
${PH_HIT}"

  # 5. 隐私组：身份证 18 位
  local ID_HIT
  ID_HIT=$(printf '%s\n' "$DIFF" | grep -nE "$PRIVACY_IDCARD_PATTERN" || true)
  [ -n "$ID_HIT" ] && HITS="${HITS}
[隐私-身份证]
${ID_HIT}"

  # 去掉开头的空行
  printf '%s' "$HITS" | sed '/^$/N;/^\n$/D'
}

# print_block_guidance —— 拦截后给用户的处理指引（两个钩子共用）
print_secret_guidance() {
  cat >&2 <<MSG

处理方式（按优先级）：
  1. 把明文换成占位符，真值移到环境变量或 git 之外的凭证文件；
  2. 邮箱误报：确认地址在白名单里是否已登记，未登记的合法地址加进
     scripts/git-hooks/secret-patterns.sh 的 SECRET_EMAIL_WHITELIST（注释写清为什么安全）；
  3. 如确认是误报（变量名、示例、测试桩），改写成含 REDACTED/YOUR_KEY/<...> 的形式，
     或用 SECRET_SCAN_SKIP=1 git push 显式放行（会打印警告，留痕）。

🔴 已经 push 出去的密钥，删文件不等于安全 —— 必须去服务商后台吊销。
   参见 collab/troubleshooting.md [SECRET-IN-REPO-001]。
MSG
}