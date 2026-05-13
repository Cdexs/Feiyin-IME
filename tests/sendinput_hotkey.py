"""
Windows SendInput helper for voice-ime hotkey tests.

This stays in tests/ on purpose: it is test-only infrastructure and must not be
linked into production code.
"""

from __future__ import annotations

import ctypes
import time
from ctypes import wintypes
from typing import Iterable


if ctypes.sizeof(ctypes.c_void_p) == ctypes.sizeof(ctypes.c_ulonglong):
    ULONG_PTR = ctypes.c_ulonglong
else:
    ULONG_PTR = ctypes.c_ulong


INPUT_KEYBOARD = 1

KEYEVENTF_EXTENDEDKEY = 0x0001
KEYEVENTF_KEYUP = 0x0002

VK_ESCAPE = 0x1B
VK_SPACE = 0x20
VK_F9 = 0x78
VK_CONTROL = 0x11
VK_MENU = 0x12
VK_OEM_3 = 0xC0


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [
        ("wVk", wintypes.WORD),
        ("wScan", wintypes.WORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ULONG_PTR),
    ]


class MOUSEINPUT(ctypes.Structure):
    _fields_ = [
        ("dx", wintypes.LONG),
        ("dy", wintypes.LONG),
        ("mouseData", wintypes.DWORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ULONG_PTR),
    ]


class HARDWAREINPUT(ctypes.Structure):
    _fields_ = [
        ("uMsg", wintypes.DWORD),
        ("wParamL", wintypes.WORD),
        ("wParamH", wintypes.WORD),
    ]


class _INPUTUNION(ctypes.Union):
    _fields_ = [
        ("ki", KEYBDINPUT),
        ("mi", MOUSEINPUT),
        ("hi", HARDWAREINPUT),
    ]


class INPUT(ctypes.Structure):
    _anonymous_ = ("u",)
    _fields_ = [
        ("type", wintypes.DWORD),
        ("u", _INPUTUNION),
    ]


user32 = ctypes.WinDLL("user32", use_last_error=True)
SendInput = user32.SendInput
SendInput.argtypes = (wintypes.UINT, ctypes.POINTER(INPUT), ctypes.c_int)
SendInput.restype = wintypes.UINT


def _keyboard_input(vk_code: int, *, key_up: bool = False, extended: bool = False) -> INPUT:
    flags = 0
    if extended:
        flags |= KEYEVENTF_EXTENDEDKEY
    if key_up:
        flags |= KEYEVENTF_KEYUP

    return INPUT(
        type=INPUT_KEYBOARD,
        ki=KEYBDINPUT(
            wVk=vk_code,
            wScan=0,
            dwFlags=flags,
            time=0,
            dwExtraInfo=0,
        ),
    )


def send_inputs(inputs: Iterable[INPUT]) -> int:
    input_array = tuple(inputs)
    if not input_array:
        return 0

    raw = (INPUT * len(input_array))(*input_array)
    sent = SendInput(len(raw), raw, ctypes.sizeof(INPUT))
    if sent != len(raw):
        raise ctypes.WinError(ctypes.get_last_error())
    return sent


def key_down(vk_code: int, *, extended: bool = False) -> None:
    send_inputs([_keyboard_input(vk_code, extended=extended)])


def key_up(vk_code: int, *, extended: bool = False) -> None:
    send_inputs([_keyboard_input(vk_code, key_up=True, extended=extended)])


def tap_key(vk_code: int, *, hold_seconds: float = 0.05, extended: bool = False) -> None:
    key_down(vk_code, extended=extended)
    time.sleep(hold_seconds)
    key_up(vk_code, extended=extended)


def hold_key(vk_code: int, duration: float, *, extended: bool = False) -> None:
    key_down(vk_code, extended=extended)
    time.sleep(duration)
    key_up(vk_code, extended=extended)


def press_hotkey(vk_code: int, modifiers: Iterable[int], *, hold_seconds: float = 0.05) -> None:
    modifiers = tuple(modifiers)
    for modifier in modifiers:
        key_down(modifier)

    try:
        tap_key(vk_code, hold_seconds=hold_seconds)
    finally:
        for modifier in reversed(modifiers):
            key_up(modifier)


def tap_f9(*, hold_seconds: float = 0.05) -> None:
    tap_key(VK_F9, hold_seconds=hold_seconds)


def hold_f9(duration: float) -> None:
    hold_key(VK_F9, duration)


def press_escape(*, hold_seconds: float = 0.03) -> None:
    tap_key(VK_ESCAPE, hold_seconds=hold_seconds)


def press_ctrl_space(*, hold_seconds: float = 0.05) -> None:
    press_hotkey(VK_SPACE, [VK_CONTROL], hold_seconds=hold_seconds)


def press_alt_grave(*, hold_seconds: float = 0.05) -> None:
    press_hotkey(VK_OEM_3, [VK_MENU], hold_seconds=hold_seconds)
