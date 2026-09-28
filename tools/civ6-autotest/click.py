"""Mouse/key input for the one visible Civ VI window, with foreground verification."""

from __future__ import annotations

import ctypes
from ctypes import wintypes
from pathlib import Path
import sys
import time

user32 = ctypes.windll.user32
kernel32 = ctypes.windll.kernel32
user32.SetProcessDPIAware()

MOUSEEVENTF_LEFTDOWN = 0x0002
MOUSEEVENTF_LEFTUP = 0x0004
KEYEVENTF_KEYUP = 0x0002
VK_ESCAPE = 0x1B
TITLE_PREFIX = "Sid Meier's Civilization VI"
CIV_EXECUTABLES = {"civilizationvi.exe", "civilizationvi_dx12.exe"}
PROCESS_QUERY_LIMITED_INFORMATION = 0x1000


class InputError(RuntimeError):
    """The Civ VI window could not be verified or safely sent input."""


_enum_callback = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
user32.EnumWindows.argtypes = [_enum_callback, wintypes.LPARAM]
user32.EnumWindows.restype = wintypes.BOOL
user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
user32.GetWindowThreadProcessId.restype = wintypes.DWORD
user32.GetForegroundWindow.restype = wintypes.HWND
user32.SetForegroundWindow.argtypes = [wintypes.HWND]
user32.SetForegroundWindow.restype = wintypes.BOOL
user32.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
user32.GetWindowRect.restype = wintypes.BOOL
kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
kernel32.OpenProcess.restype = wintypes.HANDLE
kernel32.QueryFullProcessImageNameW.argtypes = [wintypes.HANDLE, wintypes.DWORD,
                                                wintypes.LPWSTR, ctypes.POINTER(wintypes.DWORD)]
kernel32.QueryFullProcessImageNameW.restype = wintypes.BOOL
kernel32.CloseHandle.argtypes = [wintypes.HANDLE]


def _process_name(hwnd: int) -> str:
    pid = wintypes.DWORD()
    if not user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid)) or not pid.value:
        raise InputError("Cannot identify Civ VI window process")
    process = kernel32.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, False, pid.value)
    if not process:
        raise InputError("Cannot open Civ VI window process")
    try:
        executable = ctypes.create_unicode_buffer(32768)
        length = wintypes.DWORD(len(executable))
        if not kernel32.QueryFullProcessImageNameW(process, 0, executable, ctypes.byref(length)):
            raise InputError("Cannot read Civ VI window executable")
        return Path(executable.value).name.casefold()
    finally:
        kernel32.CloseHandle(process)


def find_civ_window() -> int:
    found: list[int] = []
    errors: list[Exception] = []

    @_enum_callback
    def enum(hwnd: int, _param: int) -> bool:
        try:
            if not user32.IsWindowVisible(hwnd):
                return True
            length = user32.GetWindowTextLengthW(hwnd)
            if not length:
                return True
            title = ctypes.create_unicode_buffer(length + 1)
            user32.GetWindowTextW(hwnd, title, len(title))
            if not title.value.startswith(TITLE_PREFIX):
                return True
            name = _process_name(hwnd)
            if name in CIV_EXECUTABLES:
                found.append(hwnd)
            return True
        except Exception as exc:
            errors.append(exc)
            return False

    enumerated = user32.EnumWindows(enum, 0)
    if errors:
        raise InputError("Could not verify Civ VI windows") from errors[0]
    if not enumerated or len(found) != 1:
        raise InputError(f"Expected one visible Civ VI executable window; found {len(found)}")
    return found[0]


def _require_foreground(hwnd: int) -> None:
    if find_civ_window() != hwnd or user32.GetForegroundWindow() != hwnd:
        raise InputError("Civ VI window is not the verified foreground window; input aborted")


def _focus_civ_window(hwnd: int) -> None:
    user32.ShowWindow(hwnd, 9)  # SW_RESTORE
    if not user32.SetForegroundWindow(hwnd):
        raise InputError("Could not bring Civ VI window to foreground; input aborted")
    time.sleep(0.4)
    _require_foreground(hwnd)


def click_fraction(fx: float, fy: float) -> tuple[int, int]:
    if not (0.0 <= fx <= 1.0 and 0.0 <= fy <= 1.0):
        raise ValueError("fractions must be within 0..1")
    hwnd = find_civ_window()
    _focus_civ_window(hwnd)
    rect = wintypes.RECT()
    if not user32.GetWindowRect(hwnd, ctypes.byref(rect)):
        raise InputError("GetWindowRect failed")
    if rect.right <= rect.left or rect.bottom <= rect.top:
        raise InputError("Civ VI window has no visible area")
    x = rect.left + int((rect.right - rect.left) * fx)
    y = rect.top + int((rect.bottom - rect.top) * fy)
    _require_foreground(hwnd)
    if not user32.SetCursorPos(x, y):
        raise InputError("SetCursorPos failed")
    time.sleep(0.2)
    _require_foreground(hwnd)
    try:
        user32.mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0)
        time.sleep(0.08)
        _require_foreground(hwnd)
    finally:
        user32.mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0)
    return x, y


def press_key(vk: int) -> None:
    """Send one key press only while the unique Civ VI window has focus."""
    hwnd = find_civ_window()
    _focus_civ_window(hwnd)
    _require_foreground(hwnd)
    try:
        user32.keybd_event(vk, 0, 0, 0)
        time.sleep(0.08)
        _require_foreground(hwnd)
    finally:
        user32.keybd_event(vk, 0, KEYEVENTF_KEYUP, 0)


if __name__ == "__main__":
    print(click_fraction(float(sys.argv[1]), float(sys.argv[2])))
