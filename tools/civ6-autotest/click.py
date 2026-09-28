"""Minimal mouse helper for the few Civ VI screens that Tuner Lua cannot drive (e.g. the loading
screen's Begin Game button, before the in-game Tuner states exist).

Coordinates are given as fractions of the Civ VI window rectangle (title bar included), measured
from a window screenshot taken by capture.py, so they survive window moves and size changes.
"""

from __future__ import annotations

import ctypes
from ctypes import wintypes
import sys
import time

user32 = ctypes.windll.user32
user32.SetProcessDPIAware()

MOUSEEVENTF_LEFTDOWN = 0x0002
MOUSEEVENTF_LEFTUP = 0x0004
TITLE_PREFIX = "Sid Meier's Civilization VI"


def find_civ_window() -> int:
    found: list[int] = []

    @ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def enum(hwnd: int, _param: int) -> bool:
        length = user32.GetWindowTextLengthW(hwnd)
        if length and user32.IsWindowVisible(hwnd):
            buf = ctypes.create_unicode_buffer(length + 1)
            user32.GetWindowTextW(hwnd, buf, length + 1)
            if buf.value.startswith(TITLE_PREFIX):
                found.append(hwnd)
        return True

    user32.EnumWindows(enum, 0)
    if not found:
        raise RuntimeError("Civ VI window not found")
    return found[0]


def click_fraction(fx: float, fy: float) -> tuple[int, int]:
    if not (0.0 <= fx <= 1.0 and 0.0 <= fy <= 1.0):
        raise ValueError("fractions must be within 0..1")
    hwnd = find_civ_window()
    rect = wintypes.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(rect))
    x = rect.left + int((rect.right - rect.left) * fx)
    y = rect.top + int((rect.bottom - rect.top) * fy)
    user32.ShowWindow(hwnd, 9)  # SW_RESTORE
    user32.SetForegroundWindow(hwnd)
    time.sleep(0.4)
    user32.SetCursorPos(x, y)
    time.sleep(0.2)
    user32.mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0)
    time.sleep(0.08)
    user32.mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0)
    return x, y


if __name__ == "__main__":
    print(click_fraction(float(sys.argv[1]), float(sys.argv[2])))


KEYEVENTF_KEYUP = 0x0002
VK_ESCAPE = 0x1B


def press_key(vk: int) -> None:
    """Bring Civ VI to the foreground and send one key press (used for the loading screen: Esc = Begin Game)."""
    hwnd = find_civ_window()
    user32.ShowWindow(hwnd, 9)
    # Foreground lock workaround: a synthetic Alt press lets SetForegroundWindow succeed.
    user32.keybd_event(0x12, 0, 0, 0)
    user32.SetForegroundWindow(hwnd)
    user32.keybd_event(0x12, 0, KEYEVENTF_KEYUP, 0)
    time.sleep(0.5)
    user32.keybd_event(vk, 0, 0, 0)
    time.sleep(0.08)
    user32.keybd_event(vk, 0, KEYEVENTF_KEYUP, 0)
