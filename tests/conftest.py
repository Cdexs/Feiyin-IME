"""
pytest 配置文件 - voice-ime 自动化测试框架

提供 fixtures 和全局配置：
- 程序启动/关闭 fixture
- 截图保存 fixture
- 环境变量加载
- 轮询等待工具
"""

# 优化2: pytest-xdist 并行测试说明
# 非 GUI 测试可用 `pytest -m "not gui" -n auto` 并行运行
# GUI 测试不能并行（共享进程和窗口状态），必须串行执行

import os
import sys
import time
import subprocess
from pathlib import Path
from typing import Generator, Optional, Callable

import pytest
from dotenv import load_dotenv

# 加载环境变量
load_dotenv()

# 项目路径配置
PROJECT_ROOT = Path(__file__).parent.parent
TESTS_DIR = Path(__file__).parent
SCREENSHOTS_DIR = TESTS_DIR / "screenshots"

# 默认 exe 路径
DEFAULT_EXE_PATH = PROJECT_ROOT / "target" / "release" / "voice-ime.exe"

# 从环境变量获取配置
EXE_PATH = Path(os.getenv("VOICE_IME_EXE", str(DEFAULT_EXE_PATH)))
DEFAULT_TIMEOUT = int(os.getenv("TEST_TIMEOUT", "30"))
SKIP_AUDIO_TESTS = os.getenv("SKIP_AUDIO_TESTS", "0") == "1"

# ===== 全局 pyautogui 设置 =====
try:
    import pyautogui
    pyautogui.FAILSAFE = True
    pyautogui.PAUSE = 0.1  # 优化5: 降低 PAUSE 从 0.3~0.5 到 0.1
except ImportError:
    pass


# ===== 优化1: 轮询等待工具 =====

def wait_for_condition(
    condition: Callable[[], bool],
    timeout: float = 5.0,
    interval: float = 0.2,
    description: str = "condition"
) -> bool:
    """
    轮询等待条件满足，替代 time.sleep()

    Args:
        condition: 返回 bool 的检测函数
        timeout: 超时秒数
        interval: 检测间隔秒数
        description: 条件描述（用于日志）

    Returns:
        True 如果条件在超时前满足，False 否则
    """
    start = time.time()
    while time.time() - start < timeout:
        if condition():
            return True
        time.sleep(interval)
    return False


def kill_existing_voice_ime() -> None:
    """Kill any running voice-ime.exe instances (for single-instance tests)."""
    subprocess.run(
        ["powershell", "-Command",
         "Get-Process voice-ime -ErrorAction SilentlyContinue | Stop-Process -Force"],
        capture_output=True,
    )
    time.sleep(0.5)


def wait_for_process(pid: int, timeout: float = 10.0) -> bool:
    """
    轮询等待进程存在且未退出

    Args:
        pid: 进程 ID
        timeout: 超时秒数

    Returns:
        True 如果进程在超时期间存活
    """
    import psutil
    start = time.time()
    while time.time() - start < timeout:
        try:
            proc = psutil.Process(pid)
            if proc.status() == psutil.STATUS_ZOMBIE:
                return False
            return True
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            return False
        time.sleep(0.2)
    return False


def wait_for_window_title(
    title_part: str,
    timeout: float = 5.0,
    interval: float = 0.2
) -> Optional[int]:
    """
    轮询等待包含指定标题的窗口出现

    Args:
        title_part: 窗口标题包含的字符串
        timeout: 超时秒数
        interval: 检测间隔

    Returns:
        窗口 HWND 句柄，超时返回 None
    """
    try:
        import pygetwindow as gw
    except ImportError:
        # 如果没有 pygetwindow，用 pywinauto 替代
        try:
            from pywinauto import Desktop
            start = time.time()
            while time.time() - start < timeout:
                try:
                    desktop = Desktop(backend="win32")
                    windows = desktop.windows()
                    for w in windows:
                        try:
                            if title_part in w.window_text():
                                return w.handle
                        except (Exception,):
                            pass
                except (Exception,):
                    pass
                time.sleep(interval)
            return None
        except ImportError:
            return None

    start = time.time()
    while time.time() - start < timeout:
        matching = [w for w in gw.getAllWindows() if title_part in (w.title or "")]
        if matching:
            return matching[0]._hWnd
        time.sleep(interval)
    return None


def pytest_configure(config: pytest.Config) -> None:
    """pytest 全局配置"""
    # 创建截图目录
    SCREENSHOTS_DIR.mkdir(exist_ok=True)

    # 注册标记（优化3）
    config.addinivalue_line("markers", "audio: 需要 microphone 的测试")
    config.addinivalue_line("markers", "slow: 执行时间较长的测试")
    config.addinivalue_line("markers", "gui: 需要 GUI 窗口的测试")

    # FRAMEWORK-001: 新增测试标记
    config.addinivalue_line("markers", "smoke: 冒烟测试（构建后快速验证）")
    config.addinivalue_line("markers", "hardware: 需要硬件设备（麦克风）的测试")
    config.addinivalue_line("markers", "optional: 可选测试（非核心功能）")
    config.addinivalue_line("markers", "regression: 回归测试（验证已修复的 bug）")

    # TEST-SYNC-TAURI-2.0: Tauri v2 升级专项测试标记
    config.addinivalue_line("markers", "tauri_v2: Tauri v2 升级后验证测试")
    config.addinivalue_line("markers", "capabilities: Tauri v2 capabilities 权限测试")
    config.addinivalue_line("markers", "events: Tauri v2 事件系统测试")
    config.addinivalue_line("markers", "commands: Tauri v2 Commands 调用测试")

    # E2E 全流程测试标记
    config.addinivalue_line("markers", "e2e: 端到端全流程测试（需要麦克风）")

    # TEST-FRAMEWORK-PLAYWRIGHT-001: WebView2 UI 测试标记
    config.addinivalue_line("markers", "webview: 需要 WebView2 CDP 连接的 Playwright 测试")
    config.addinivalue_line("markers", "cdp: 需要远程调试端口的测试")

    # TEST-SYNC-MAC-011: macOS 平台测试标记
    config.addinivalue_line("markers", "macos: macOS 平台专属测试")

    # 优化6: pytest-timeout 默认值已在 pytest.ini 中配置，无需在此处添加


def pytest_collection_modifyitems(config: pytest.Config, items: list[pytest.Item]) -> None:
    """测试收集后处理：默认跳过 slow 测试（优化3）"""
    # 如果未指定 --run-slow，跳过 slow 标记测试
    if not config.getoption("-m", default=""):
        skip_slow = pytest.mark.skip(reason="Slow test, use 'pytest -m slow' to run")
        for item in items:
            if "slow" in item.keywords:
                item.add_marker(skip_slow)

    if SKIP_AUDIO_TESTS:
        skip_audio = pytest.mark.skip(reason="SKIP_AUDIO_TESTS=1")
        for item in items:
            if "audio" in item.keywords:
                item.add_marker(skip_audio)


@pytest.fixture(scope="session")
def exe_path() -> Path:
    """返回 voice-ime.exe 路径"""
    if not EXE_PATH.exists():
        pytest.fail(f"voice-ime.exe not found: {EXE_PATH}")
    return EXE_PATH


@pytest.fixture(scope="session")
def screenshots_dir() -> Path:
    """返回截图保存目录"""
    return SCREENSHOTS_DIR


# ===== 优化4: session 级进程复用 fixture =====

@pytest.fixture(scope="session")
def session_voice_ime_process(exe_path: Path) -> Generator[Optional[subprocess.Popen], None, None]:
    """
    session 级 voice-ime.exe 进程 fixture（优化4）

    整个测试会话只启动一次进程，显著减少重复启动开销。
    各测试通过 function 级 wrapper 引用此进程。

    使用方式：
        def test_example(session_voice_ime_process):
            assert session_voice_ime_process.poll() is None
    """
    process = None
    try:
        # 启动进程
        process = subprocess.Popen(
            [str(exe_path)],
            cwd=str(exe_path.parent),
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        )

        # 优化1: 改为轮询等待进程初始化（最多等 10s）
        initialized = wait_for_condition(
            lambda: process.poll() is None,
            timeout=10.0,
            description="process initialization"
        )
        if not initialized or process.poll() is not None:
            pytest.fail(f"voice-ime.exe failed to initialize, exit code: {process.returncode}")

        yield process

    finally:
        # 清理进程
        if process is not None:
            try:
                process.terminate()
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


@pytest.fixture
def voice_ime_process(session_voice_ime_process) -> Generator[Optional[subprocess.Popen], None, None]:
    """
    function 级 voice-ime.exe 进程 wrapper（引用 session 级进程）

    使用方式：
        def test_example(voice_ime_process):
            assert voice_ime_process is not None
            # ... 测试逻辑

    自动清理：测试结束后不关闭进程（由 session fixture 管理）
    """
    yield session_voice_ime_process


# ===== 旧版兼容：如果需要每次测试独立进程，可用此 fixture =====

@pytest.fixture
def isolated_voice_ime_process(exe_path: Path) -> Generator[Optional[subprocess.Popen], None, None]:
    """
    独立进程 fixture（每次测试启动/关闭）

    仅用于需要干净进程状态的测试
    """
    process = None
    try:
        process = subprocess.Popen(
            [str(exe_path)],
            cwd=str(exe_path.parent),
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        )

        # 优化1: 轮询等待初始化
        initialized = wait_for_condition(
            lambda: process.poll() is None,
            timeout=10.0,
            description="process initialization"
        )
        if not initialized or process.poll() is not None:
            pytest.fail(f"voice-ime.exe exited immediately with code: {process.returncode}")

        yield process

    finally:
        if process is not None:
            try:
                process.terminate()
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


@pytest.fixture
def save_screenshot_on_failure(request: pytest.FixtureRequest, screenshots_dir: Path) -> None:
    """
    测试失败时自动截图 fixture

    使用方式：
        def test_example(save_screenshot_on_failure):
            # 如果测试失败，自动截图保存
            ...
    """
    # 测试执行前无操作
    yield

    # 测试失败时截图
    if hasattr(request.node, "rep_call") and request.node.rep_call.failed:
        try:
            import pyautogui
            screenshot_path = screenshots_dir / f"{request.node.name}_{int(time.time())}.png"
            pyautogui.screenshot(str(screenshot_path))
            print(f"\nScreenshot saved: {screenshot_path}")
        except Exception as e:
            print(f"\nFailed to save screenshot: {e}")


@pytest.hookimpl(hookwrapper=True)
def pytest_runtest_makereport(item: pytest.Item, call: pytest.CallInfo) -> None:
    """
    测试结果钩子：为 screenshot fixture 提供失败信息
    """
    outcome = yield
    rep = outcome.get_result()

    # 设置 rep_call 属性供 fixture 使用
    if call.when == "call":
        item.rep_call = rep
    elif call.when == "setup":
        item.rep_setup = rep
    elif call.when == "teardown":
        item.rep_teardown = rep


@pytest.fixture
def app_config() -> dict:
    """
    返回默认测试配置

    测试时可修改配置验证保存/加载
    """
    return {
        "hotkey": {"mode": "toggle", "vk_code": 0x78, "modifiers": 0},  # F9
        "audio": {
            "silence_threshold": 0.01,
            "silence_duration_ms": 1500,
            "max_record_seconds": 30,
        },
        "llm": {"enabled": False},
    }


@pytest.fixture(scope="session")
def test_audio_file() -> Path:
    """
    返回测试音频文件路径

    用于模拟语音输入，播放到扬声器让麦克风捕获
    """
    audio_path = TESTS_DIR / "录音.m4a"
    if not audio_path.exists():
        pytest.skip(f"Test audio file not found: {audio_path}")
    return audio_path


@pytest.fixture
def audio_player(test_audio_file: Path):
    """
    音频播放器 fixture

    使用 pygame 播放测试音频到扬声器

    使用方式：
        def test_with_audio(audio_player):
            audio_player.play()
            time.sleep(5)  # 播放期间触发录音
            audio_player.stop()
    """
    try:
        import pygame
        pygame.mixer.init()
        pygame.mixer.music.load(str(test_audio_file))

        class Player:
            def play(self):
                pygame.mixer.music.play()

            def stop(self):
                pygame.mixer.music.stop()

            def is_playing(self) -> bool:
                return pygame.mixer.music.get_busy()

            def wait_until_done(self, timeout: float = 30.0):
                """等待播放完成"""
                start = time.time()
                while pygame.mixer.music.get_busy():
                    if time.time() - start > timeout:
                        pygame.mixer.music.stop()
                        break
                    time.sleep(0.1)

        yield Player()

        # 清理
        pygame.mixer.music.stop()
        pygame.mixer.quit()

    except ImportError:
        pytest.skip("pygame not installed, run: pip install pygame")


# ===== TEST-FRAMEWORK-PLAYWRIGHT-001: CDP 连接 fixtures =====

CDP_PORT = 9222
CDP_URL = f"http://localhost:{CDP_PORT}"


def _wait_for_cdp_ready(timeout: float = 10.0, interval: float = 0.5) -> bool:
    """
    轮询等待 CDP 端口就绪

    WebView2 启动后 CDP 端口可能延迟就绪，需要重试。
    """
    import urllib.request
    import urllib.error

    start = time.time()
    while time.time() - start < timeout:
        try:
            resp = urllib.request.urlopen(f"{CDP_URL}/json/version", timeout=2)
            if resp.status == 200:
                return True
        except (urllib.error.URLError, OSError, TimeoutError):
            pass
        time.sleep(interval)
    return False


@pytest.fixture(scope="session")
def cdp_browser():
    """
    Session 级 Playwright 浏览器，连接到 WebView2 CDP

    前置条件：启动 voice-ime.exe 前需设置环境变量
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222"

    使用方式：
        def test_web(cdp_browser, main_page):
            main_page.locator("#btn").click()

    注意：使用 browser.disconnect() 而非 close()，避免终止 WebView2 进程。
    """
    from playwright.sync_api import sync_playwright

    pw = sync_playwright().start()

    # 等待 CDP 端口就绪
    if not _wait_for_cdp_ready(timeout=10.0):
        pytest.skip(
            f"CDP port not available at {CDP_URL}. "
            "Ensure WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9222' is set."
        )

    try:
        browser = pw.chromium.connect_over_cdp(CDP_URL)
        yield browser
        browser.disconnect()  # 只断开连接，不关闭 WebView2 进程
    except Exception as e:
        pytest.skip(f"CDP connection failed: {e}")
    finally:
        pw.stop()


def _find_main_page(cdp_browser) -> "Page":
    """
    在 WebView2 的所有页面中查找主窗口页面

    Tauri v1 release 模式 URL: tauri://localhost/index.html 或类似
    Tauri v2 URL: https://tauri.localhost/ 或类似
    
    注意：需要排除 overlay.html 等辅助页面
    """
    primary_pages = []
    fallback_pages = []
    
    for ctx in cdp_browser.contexts:
        for page in ctx.pages:
            url = page.url or ""
            # 排除 about:blank 和 overlay 页面
            if url and url != "about:blank" and "overlay" not in url.lower():
                if "tauri" in url.lower() or "localhost" in url.lower():
                    # 优先匹配根路径页面
                    if url.endswith("/") or url.endswith("/index.html"):
                        primary_pages.append(page)
                    else:
                        fallback_pages.append(page)

    # 优先返回根路径页面
    if primary_pages:
        return primary_pages[0]
    
    # 其次返回其他非 overlay 页面
    if fallback_pages:
        return fallback_pages[0]

    # 降级：返回第一个非空白且非 overlay 页面
    if cdp_browser.contexts:
        pages = cdp_browser.contexts[0].pages
        for page in pages:
            if page.url and page.url != "about:blank" and "overlay" not in page.url.lower():
                return page
        if pages:
            return pages[0]

    pytest.fail("No pages found in WebView2 CDP contexts")


@pytest.fixture
def main_page(cdp_browser):
    """
    获取主窗口 page

    每个测试独立使用，自动清理。
    """
    page = _find_main_page(cdp_browser)
    
    # 等待页面内容加载完成（关键修复：确保 DOM 渲染完成）
    try:
        page.wait_for_load_state("load", timeout=10000)
        # 额外等待确保 React 组件渲染完成
        page.wait_for_selector("body", timeout=5000)
        time.sleep(1)  # 额外等待 React hydration
    except Exception as e:
        print(f"⚠️ Page load wait warning: {e}")
    
    yield page


@pytest.fixture
def all_pages(cdp_browser):
    """
    获取所有 WebView2 页面列表

    用于多窗口场景（主窗口 + overlay 等）。
    """
    pages = []
    for ctx in cdp_browser.contexts:
        pages.extend(ctx.pages)
    return pages