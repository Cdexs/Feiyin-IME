"""
voice-ime 状态检测工具

基于 Win32 API 实现 overlay 窗口状态检测：
- 通过窗口类名查找 overlay 窗口
- 通过可见性 + 窗口尺寸判断当前状态
- 支持截图 + OCR（可选）

窗口尺寸对应状态（2026-05-02 RECORDING-OVERLAY-REDESIGN-001 更新）：
- 所有状态统一尺寸：480x52
"""

import ctypes
import time
from ctypes import wintypes
from enum import Enum
from dataclasses import dataclass
from typing import Optional, Tuple

import pytest


# Win32 API 常量和函数
user32 = ctypes.windll.user32

WNDENUMPROC = ctypes.WINFUNCTYPE(
    ctypes.c_bool,
    wintypes.HWND,
    wintypes.LPARAM
)

# 窗口类名
OVERLAY_CLASS_NAME = "voice-ime-overlay-window"
SETTINGS_WINDOW_TITLE = "飞音语音输入"  # Tauri v2 窗口标题（productName）

# 状态对应窗口尺寸 (width, height)
# RECORDING-OVERLAY-REDESIGN-001: recording/processing 更新为 480x52
# focuslost 保持原尺寸 320x110（coder-2 未修改）
STATE_SIZES = {
    "recording": (480, 52),
    "processing": (480, 52),
    "focuslost": (320, 110),
}


class OverlayState(Enum):
    """Overlay 窗口状态"""
    HIDDEN = "hidden"          # 窗口不存在
    RECORDING = "recording"    # 录音中
    PROCESSING = "processing"  # 处理中
    FOCUSLOST = "focuslost"    # 失焦预览
    UNKNOWN = "unknown"        # 未知状态


@dataclass
class WindowInfo:
    """窗口信息"""
    hwnd: int
    rect: Tuple[int, int, int, int]  # (left, top, right, bottom)
    width: int
    height: int
    state: OverlayState


def find_overlay_window() -> Optional[int]:
    """
    查找 overlay 窗口句柄

    Returns:
        窗口句柄（HWND），如果不存在返回 None
    """
    hwnd = user32.FindWindowW(OVERLAY_CLASS_NAME, None)
    return hwnd if hwnd else None


def find_settings_window() -> Optional[int]:
    """
    查找配置窗口句柄

    Tauri v2 配置窗口特征：
    - 窗口标题：'飞音语音输入'
    - 窗口尺寸较大（约 1179x720），区别于 Controller（隐藏）和 Overlay（320x100）
    
    Returns:
        窗口句柄（HWND），如果不存在返回 None
    """
    # 遍历所有窗口，查找标题匹配且尺寸符合配置窗口特征的
    target_title = SETTINGS_WINDOW_TITLE
    
    def enum_proc(hwnd, lParam):
        if user32.IsWindowVisible(hwnd):
            length = user32.GetWindowTextLengthW(hwnd)
            if length > 0:
                buf = ctypes.create_unicode_buffer(length + 1)
                user32.GetWindowTextW(hwnd, buf, length + 1)
                title = buf.value
                if title == target_title:
                    # 检查窗口尺寸（配置窗口约 1179x720）
                    rect = wintypes.RECT()
                    if user32.GetWindowRect(hwnd, ctypes.byref(rect)):
                        width = rect.right - rect.left
                        height = rect.bottom - rect.top
                        # 配置窗口应该宽度 > 800，高度 > 500
                        if width > 800 and height > 500:
                            # 存储结果
                            ctypes.cast(lParam, ctypes.POINTER(ctypes.c_void_p))[0] = hwnd
                            return False  # 停止枚举
        return True
    
    result = ctypes.c_void_p(0)
    ENUMPROC = ctypes.WINFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)
    user32.EnumWindows(ENUMPROC(enum_proc), ctypes.byref(result))
    
    return result.value if result.value else None


def get_window_rect(hwnd: int) -> Optional[Tuple[int, int, int, int]]:
    """
    获取窗口矩形区域

    Args:
        hwnd: 窗口句柄

    Returns:
        (left, top, right, bottom) 矩形坐标，失败返回 None
    """
    rect = wintypes.RECT()
    result = user32.GetWindowRect(hwnd, ctypes.byref(rect))
    if result:
        return (rect.left, rect.top, rect.right, rect.bottom)
    return None


def detect_overlay_state(hwnd: Optional[int] = None) -> OverlayState:
    """
    检测 overlay 窗口状态

    RECORDING-OVERLAY-REDESIGN-001 更新：
    - recording/processing: 480x52（新设计）
    - focuslost: 320x110（未修改）

    Args:
        hwnd: 窗口句柄（可选，不传则自动查找）

    Returns:
        OverlayState 状态枚举
    """
    if hwnd is None:
        hwnd = find_overlay_window()

    if not hwnd or not is_window_visible(hwnd):
        return OverlayState.HIDDEN

    rect = get_window_rect(hwnd)
    if not rect:
        return OverlayState.HIDDEN

    width = rect[2] - rect[0]
    height = rect[3] - rect[1]

    # 根据尺寸判断状态（允许 ±5px 误差）
    for state_name, (expected_w, expected_h) in STATE_SIZES.items():
        if abs(width - expected_w) <= 5 and abs(height - expected_h) <= 5:
            return OverlayState(state_name)

    return OverlayState.UNKNOWN


def get_overlay_window_info() -> Optional[WindowInfo]:
    """
    获取 overlay 窗口完整信息

    Returns:
        WindowInfo 对象，窗口不存在返回 None
    """
    hwnd = find_overlay_window()
    if not hwnd:
        return None

    rect = get_window_rect(hwnd)
    if not rect:
        return None

    width = rect[2] - rect[0]
    height = rect[3] - rect[1]
    state = detect_overlay_state(hwnd)

    return WindowInfo(
        hwnd=hwnd,
        rect=rect,
        width=width,
        height=height,
        state=state
    )


def get_settings_window_info() -> Optional[WindowInfo]:
    """
    获取配置窗口完整信息

    Returns:
        WindowInfo 对象，窗口不存在返回 None
    """
    hwnd = find_settings_window()
    if not hwnd:
        return None

    rect = get_window_rect(hwnd)
    if not rect:
        return None

    width = rect[2] - rect[0]
    height = rect[3] - rect[1]

    return WindowInfo(
        hwnd=hwnd,
        rect=rect,
        width=width,
        height=height,
        state=OverlayState.UNKNOWN  # 配置窗口无状态概念
    )


def wait_for_overlay_state(
    expected_state: OverlayState,
    timeout: float = 10.0,
    poll_interval: float = 0.5
) -> bool:
    """
    等待 overlay 达到预期状态

    Args:
        expected_state: 预期状态
        timeout: 超时时间（秒）
        poll_interval: 检测间隔（秒）

    Returns:
        True 如果在超时前达到预期状态，False 否则
    """
    start_time = time.time()
    while time.time() - start_time < timeout:
        current_state = detect_overlay_state()
        if current_state == expected_state:
            return True
        time.sleep(poll_interval)
    return False


def is_window_visible(hwnd: int) -> bool:
    """
    检测窗口是否可见

    Args:
        hwnd: 窗口句柄

    Returns:
        True 如果窗口可见
    """
    if not hwnd:
        return False
    return user32.IsWindowVisible(hwnd) != 0


def bring_window_to_front(hwnd: int) -> bool:
    """
    将窗口带到前台

    Args:
        hwnd: 窗口句柄

    Returns:
        True 如果成功
    """
    result = user32.SetForegroundWindow(hwnd)
    return result != 0


# ==================== pytest fixtures ====================

@pytest.fixture
def overlay_detector():
    """
    Overlay 状态检测 fixture

    使用方式：
        def test_recording_state(overlay_detector):
            state = overlay_detector.detect()
            assert state == OverlayState.RECORDING
    """
    return OverlayDetector()


@pytest.fixture
def settings_window(exe_path):
    """
    配置窗口 fixture

    启动配置窗口进程，测试结束后关闭

    使用方式：
        def test_settings_tab(settings_window):
            # settings_window 是进程句柄
            ...
    """
    import subprocess
    process = None
    try:
        process = subprocess.Popen(
            [str(exe_path), "--settings-ui"],
            cwd=str(exe_path.parent),
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        )
        time.sleep(2)  # 等待窗口初始化
        yield process
    finally:
        if process is not None:
            process.terminate()
            process.wait(timeout=5)


class OverlayDetector:
    """
    Overlay 状态检测器类

    提供便捷的状态检测方法
    """

    def detect(self) -> OverlayState:
        """检测当前状态"""
        return detect_overlay_state()

    def wait_for(self, state: OverlayState, timeout: float = 10.0) -> bool:
        """等待指定状态"""
        return wait_for_overlay_state(state, timeout)

    def get_info(self) -> Optional[WindowInfo]:
        """获取窗口信息"""
        return get_overlay_window_info()

    def is_recording(self) -> bool:
        """是否在录音"""
        return self.detect() == OverlayState.RECORDING

    def is_processing(self) -> bool:
        """是否在处理"""
        return self.detect() == OverlayState.PROCESSING

    def is_focuslost(self) -> bool:
        """是否显示失焦窗口"""
        return self.detect() == OverlayState.FOCUSLOST

    def is_hidden(self) -> bool:
        """是否隐藏"""
        return self.detect() == OverlayState.HIDDEN


# ==================== OCR 相关（可选） ====================

def capture_window_screenshot(hwnd: int) -> Optional[bytes]:
    """
    截取窗口截图（需要 Pillow）

    Args:
        hwnd: 窗口句柄

    Returns:
        PNG 格式截图数据，失败返回 None
    """
    try:
        from PIL import Image
        import io

        rect = get_window_rect(hwnd)
        if not rect:
            return None

        width = rect[2] - rect[0]
        height = rect[3] - rect[1]

        # 使用 BitBlt 截图
        hdc_window = user32.GetDC(hwnd)
        hdc_mem = user32.CreateCompatibleDC(hdc_window)
        hbitmap = user32.CreateCompatibleBitmap(hdc_window, width, height)

        user32.SelectObject(hdc_mem, hbitmap)
        user32.BitBlt(hdc_mem, 0, 0, width, height, hdc_window, 0, 0, 0x00CC0020)  # SRCCOPY

        # 获取位图数据
        # 注意：这里简化处理，实际需要更复杂的位图转换
        # 建议使用 pyautogui.screenshot() 替代

        user32.DeleteObject(hbitmap)
        user32.DeleteDC(hdc_mem)
        user32.ReleaseDC(hwnd, hdc_window)

        return None  # 需要额外实现位图转换

    except ImportError:
        pytest.skip("Pillow not installed for screenshot capture")


def ocr_detect_state(image_data: bytes) -> OverlayState:
    """
    OCR 检测状态（需要 pytesseract）

    Args:
        image_data: 截图数据

    Returns:
        检测到的状态
    """
    try:
        import pytesseract
        from PIL import Image
        import io

        image = Image.open(io.BytesIO(image_data))
        text = pytesseract.image_to_string(image, lang='chi_sim+eng')

        if "录音中" in text or "Recording" in text:
            return OverlayState.RECORDING
        if "转录中" in text or "处理中" in text or "Transcribing" in text:
            return OverlayState.PROCESSING
        if "复制" in text or "Copy" in text:
            return OverlayState.FOCUSLOST

        return OverlayState.UNKNOWN

    except ImportError:
        pytest.skip("pytesseract not installed for OCR detection")
