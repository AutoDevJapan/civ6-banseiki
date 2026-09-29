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
WM_KEYDOWN = 0x0100
WM_KEYUP = 0x0101
WM_CLOSE = 0x0010
INPUT_KEYBOARD = 1
# Confirm this mode on Civ VI's loading screen before changing it.
KEY_DELIVERY = "window"
TITLE_PREFIX = "Sid Meier's Civilization VI"
CIV_EXECUTABLES = {"civilizationvi.exe", "civilizationvi_dx12.exe"}
PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
TH32CS_SNAPPROCESS = 0x00000002
ERROR_NO_MORE_FILES = 18


class InputError(RuntimeError):
    """The Civ VI window could not be verified or safely sent input."""


class PROCESSENTRY32W(ctypes.Structure):
    _fields_ = [
        ("dwSize", wintypes.DWORD), ("cntUsage", wintypes.DWORD),
        ("th32ProcessID", wintypes.DWORD), ("th32DefaultHeapID", ctypes.c_size_t),
        ("th32ModuleID", wintypes.DWORD), ("cntThreads", wintypes.DWORD),
        ("th32ParentProcessID", wintypes.DWORD), ("pcPriClassBase", wintypes.LONG),
        ("dwFlags", wintypes.DWORD), ("szExeFile", wintypes.WCHAR * 260),
    ]


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
user32.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
user32.PostMessageW.restype = wintypes.BOOL
user32.MapVirtualKeyW.argtypes = [wintypes.UINT, wintypes.UINT]
user32.MapVirtualKeyW.restype = wintypes.UINT


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [("wVk", wintypes.WORD), ("wScan", wintypes.WORD),
                ("dwFlags", wintypes.DWORD), ("time", wintypes.DWORD),
                ("dwExtraInfo", ctypes.c_size_t)]


class INPUT_UNION(ctypes.Union):
    _fields_ = [("ki", KEYBDINPUT), ("padding", ctypes.c_byte * 32)]


class INPUT(ctypes.Structure):
    _fields_ = [("type", wintypes.DWORD), ("data", INPUT_UNION)]


user32.SendInput.argtypes = [wintypes.UINT, ctypes.POINTER(INPUT), ctypes.c_int]
user32.SendInput.restype = wintypes.UINT
kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
kernel32.OpenProcess.restype = wintypes.HANDLE
kernel32.QueryFullProcessImageNameW.argtypes = [wintypes.HANDLE, wintypes.DWORD,
                                                wintypes.LPWSTR, ctypes.POINTER(wintypes.DWORD)]
kernel32.QueryFullProcessImageNameW.restype = wintypes.BOOL
kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
kernel32.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
kernel32.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
kernel32.Process32FirstW.argtypes = [wintypes.HANDLE, ctypes.POINTER(PROCESSENTRY32W)]
kernel32.Process32FirstW.restype = wintypes.BOOL
kernel32.Process32NextW.argtypes = [wintypes.HANDLE, ctypes.POINTER(PROCESSENTRY32W)]
kernel32.Process32NextW.restype = wintypes.BOOL


def _civ_process_ids() -> set[int]:
    """Include hidden game processes; a visible-window count alone is insufficient."""
    snapshot = kernel32.CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
    if snapshot is None or snapshot == wintypes.HANDLE(-1).value:
        raise InputError("Could not enumerate Civ VI processes")
    try:
        entry = PROCESSENTRY32W()
        entry.dwSize = ctypes.sizeof(entry)
        if not kernel32.Process32FirstW(snapshot, ctypes.byref(entry)):
            raise InputError("Could not read Civ VI process list")
        found = set()
        while True:
            if entry.szExeFile.casefold() in CIV_EXECUTABLES:
                found.add(entry.th32ProcessID)
            if not kernel32.Process32NextW(snapshot, ctypes.byref(entry)):
                if kernel32.GetLastError() != ERROR_NO_MORE_FILES:
                    raise InputError("Civ VI process list became unavailable")
                break
        return found
    finally:
        kernel32.CloseHandle(snapshot)


def require_single_civ_process() -> None:
    count = len(_civ_process_ids())
    if count == 0:
        raise InputError("FireTuner に接続できません（Civ6 が起動していない可能性）")
    if count > 1:
        raise InputError(f"Multiple Civ VI processes found ({count}); driver requires one local game")


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
    require_single_civ_process()
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


def _require_before_deadline(deadline: float | None) -> None:
    if deadline is not None and time.monotonic() >= deadline:
        raise InputError("Step deadline expired before key input")


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


def request_civ_window_close() -> None:
    """Ask the verified game window to close through the normal UI path."""
    hwnd = find_civ_window()
    if find_civ_window() != hwnd or not user32.PostMessageW(hwnd, WM_CLOSE, 0, 0):
        raise InputError("Could not request Civ VI window close")


def press_key(vk: int, *, deadline: float | None = None) -> None:
    """Send a key to the verified Civ VI window, releasing it on every error path."""
    hwnd = find_civ_window()
    if KEY_DELIVERY == "window":
        # PostMessageW targets this HWND even when another app has focus.
        scan = user32.MapVirtualKeyW(vk, 0)
        down_param = 1 | (scan << 16)
        _require_before_deadline(deadline)
        if find_civ_window() != hwnd:
            raise InputError("Civ VI window changed before key input")
        if not user32.PostMessageW(hwnd, WM_KEYDOWN, vk, down_param):
            raise InputError("Could not post Civ VI key down")
        try:
            time.sleep(0.08)
        finally:
            if not user32.PostMessageW(hwnd, WM_KEYUP, vk, down_param | 0xC0000000):
                raise InputError("Could not post Civ VI key up")
    elif KEY_DELIVERY == "sendinput":
        _focus_civ_window(hwnd)
        _require_before_deadline(deadline)

        def send(flags: int) -> None:
            key = INPUT(type=INPUT_KEYBOARD, data=INPUT_UNION(ki=KEYBDINPUT(vk, 0, flags, 0, 0)))
            if flags == 0:
                _require_before_deadline(deadline)
            if user32.SendInput(1, ctypes.byref(key), ctypes.sizeof(key)) != 1:
                raise InputError("Could not send Civ VI key input")

        _require_foreground(hwnd)
        send(0)
        try:
            _require_foreground(hwnd)
            time.sleep(0.08)
        finally:
            send(KEYEVENTF_KEYUP)
    else:
        raise InputError(f"Unknown key delivery mode: {KEY_DELIVERY}")
    if KEY_DELIVERY == "sendinput":
        _require_foreground(hwnd)


if __name__ == "__main__":
    print(click_fraction(float(sys.argv[1]), float(sys.argv[2])))
