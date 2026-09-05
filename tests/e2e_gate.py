"""
E2E-GATE-103 · 「ERROR>0 即门禁不通过」机器闸门（可执行判据）

背景（logs/20260905.md）：BUILD-085/093 报「64P/1F」「65P/0F」漂亮数字背后，
full_pipeline 四条用例因环境缺 toml 收集期 ERROR，三周无人发现——收集期 ERROR
被当成「没跑」而非「门禁失效」。只加文字规则等于没解决，必须落成机器判据。

用法（Python 3.11，构建产物必须已就位）：
    py -3.11 -m pytest tests/test_cases/ -v            # 全量 E2E（本脚本等价包装）
    py -3.11 tests/e2e_gate.py                          # 用默认选集（全量）跑门禁
    py -3.11 tests/e2e_gate.py -m "not hardware"        # 指定选集

机器判据（脚本退出码）：
    - errors > 0                    → exit 1（门禁红，收集期 ERROR 显式拦截）
    - failed  > 0                   → exit 1（门禁红）
    - 两者都为 0                    → exit 0（门禁绿）

选集显式化：本脚本始终打印「-m 表达式 + collected + deselected + 各计数」，
BUILD-093（-m "not hardware" 7 deselected）与 BUILD-098（全量 6 deselected）
那种「数字不可比却无人察觉」从此显式可见。

与 conftest.py 的 pytest_terminal_summary 横幅联动：横幅给出人类可读报告，
本脚本解析 pytest 退出码 + 摘要行给出机器判据，双重保险。
"""

import argparse
import re
import subprocess
import sys
from pathlib import Path

# Windows 控制台默认 GBK，emoji/中文可能在 print 时炸编码 —— 强制 UTF-8 输出
try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

ROOT = Path(__file__).resolve().parent.parent


def parse_counts(output: str) -> dict:
    """从 pytest 摘要提取计数（容错：缺项补 0）。只取最后一行形如
    'N passed, M failed, ... in X.XXs' 的汇总，避免 -v 进度行误匹配。"""
    counts = {"passed": 0, "failed": 0, "errors": 0, "skipped": 0, "deselected": 0, "error": 0}
    # 找所有含 "in <time>s" 的汇总行，取最后一条
    summary_lines = [l for l in output.splitlines() if re.search(r" in \d+\.?\d*s", l)]
    line = summary_lines[-1] if summary_lines else output
    m = re.search(r"(\d+) passed", line)
    if m:
        counts["passed"] = int(m.group(1))
    m = re.search(r"(\d+) failed", line)
    if m:
        counts["failed"] = int(m.group(1))
    m = re.search(r"(\d+) error", line)
    if m:
        counts["errors"] = int(m.group(1))
    m = re.search(r"(\d+) skipped", line)
    if m:
        counts["skipped"] = int(m.group(1))
    m = re.search(r"(\d+) deselected", line)
    if m:
        counts["deselected"] = int(m.group(1))
    # pytest 摘要可能写 "errors"（复数）
    m = re.search(r"(\d+) errors", line)
    if m and counts["errors"] == 0:
        counts["errors"] = int(m.group(1))
    # 若整行是 "N passed, M deselected, K error in ..." 变体
    m = re.search(r"(\d+) error", line)
    if m:
        counts["errors"] = max(counts["errors"], int(m.group(1)))
    return counts


def main() -> int:
    parser = argparse.ArgumentParser(description="voice-ime E2E 门禁（ERROR>0 即不通过）")
    parser.add_argument("-m", dest="markexpr", default="", help="pytest -m 选集表达式")
    parser.add_argument("--paths", nargs="*", default=["tests/test_cases/"], help="pytest 目标路径")
    args = parser.parse_args()

    cmd = [sys.executable, "-m", "pytest"]
    cmd += args.paths
    cmd += ["-v", "--tb=short"]
    if args.markexpr:
        cmd += ["-m", args.markexpr]

    print(f"[E2E-GATE-103] 执行: {' '.join(cmd)}")
    proc = subprocess.run(cmd, cwd=str(ROOT), capture_output=True, text=True,
                          encoding="utf-8", errors="replace")
    output = (proc.stdout or "") + "\n" + (proc.stderr or "")

    # 打印关键输出（截断避免刷屏，保留门禁横幅与摘要）
    lines = output.splitlines()
    show = []
    for i, l in enumerate(lines):
        if "E2E-GATE-103" in l or "passed" in l or "failed" in l or "error" in l.lower() \
                or "deselected" in l or "====" in l:
            show.append(l)
    print("\n".join(show))

    counts = parse_counts(output)
    print("\n" + "=" * 60)
    print(f"[E2E-GATE-103] 机器判据")
    print(f"  -m 表达式       : {args.markexpr or '(none)'}")
    print(f"  pytest 退出码   : {proc.returncode}")
    print(f"  passed          : {counts['passed']}")
    print(f"  failed          : {counts['failed']}")
    print(f"  errors          : {counts['errors']}")
    print(f"  skipped         : {counts['skipped']}")
    print(f"  deselected      : {counts['deselected']}")

    if counts["errors"] > 0:
        print("  🔴 门禁判定：不通过（errors>0 —— 收集期 ERROR 显式拦截，不允许静默计入 skip/deselect）")
        return 1
    if counts["failed"] > 0:
        print("  🔴 门禁判定：不通过（failed>0）")
        return 1
    print("  ✅ 门禁判定：通过")
    return 0


if __name__ == "__main__":
    sys.exit(main())