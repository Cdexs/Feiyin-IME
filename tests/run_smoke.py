#!/usr/bin/env python3
"""
冒烟测试运行器 - voice-ime

运行所有标记为 @pytest.mark.smoke 的测试，快速验证构建质量。

使用方式：
    python run_smoke.py

等价于：
    pytest -m smoke -v --tb=short
"""

import subprocess
import sys
from pathlib import Path


def main() -> int:
    """运行冒烟测试并返回退出码"""
    tests_dir = Path(__file__).parent

    cmd = [
        sys.executable, "-m", "pytest",
        "-m", "smoke",
        "-v",
        "--tb=short",
        str(tests_dir),
    ]

    print("=" * 60)
    print("🔥 运行冒烟测试（smoke tests）")
    print("=" * 60)
    print(f"命令: {' '.join(cmd)}")
    print("=" * 60)

    result = subprocess.run(cmd, cwd=str(tests_dir))

    print("=" * 60)
    if result.returncode == 0:
        print("✅ 冒烟测试通过")
    else:
        print(f"❌ 冒烟测试失败（退出码: {result.returncode}）")
    print("=" * 60)

    return result.returncode


if __name__ == "__main__":
    sys.exit(main())
