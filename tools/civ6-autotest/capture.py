"""Capture the Civ VI window to PNG using Win32 only (no input automation)."""

from __future__ import annotations

import ctypes
from ctypes import wintypes
from pathlib import Path
import struct
import sys
import zlib


class CaptureError(RuntimeError):
    pass


class BITMAPINFOHEADER(ctypes.Structure):
    _fields_ = [
        ("biSize", wintypes.DWORD), ("biWidth", wintypes.LONG),
        ("biHeight", wintypes.LONG), ("biPlanes", wintypes.WORD),
        ("biBitCount", wintypes.WORD), ("biCompression", wintypes.DWORD),
        ("biSizeImage", wintypes.DWORD), ("biXPelsPerMeter", wintypes.LONG),
        ("biYPelsPerMeter", wintypes.LONG), ("biClrUsed", wintypes.DWORD),
        ("biClrImportant", wintypes.DWORD),
    ]


class BITMAPINFO(ctypes.Structure):
    _fields_ = [("bmiHeader", BITMAPINFOHEADER), ("bmiColors", wintypes.DWORD * 1)]


def _png_chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


def _write_png(path: Path, width: int, height: int, bgra: bytes) -> None:
    rows = bytearray()
    stride = width * 4
    for y in range(height):
        rows.append(0)
        row = bgra[y * stride:(y + 1) * stride]
        for x in range(0, stride, 4):
            rows.extend((row[x + 2], row[x + 1], row[x]))
    png = b"\x89PNG\r\n\x1a\n"
    png += _png_chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
    png += _png_chunk(b"IDAT", zlib.compress(rows, 6))
    png += _png_chunk(b"IEND", b"")
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        raise CaptureError(f"Screenshot already exists: {path}")
    path.write_bytes(png)


def capture_civ_window(path: Path, crop: list[float] | None = None) -> None:
    if sys.platform != "win32":
        raise CaptureError("Window capture requires Windows")
    user32 = ctypes.WinDLL("user32", use_last_error=True)
    gdi32 = ctypes.WinDLL("gdi32", use_last_error=True)
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)

    user32.EnumWindows.argtypes = [ctypes.c_void_p, wintypes.LPARAM]
    user32.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    user32.GetWindowTextW.restype = ctypes.c_int
    user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user32.IsWindowVisible.argtypes = [wintypes.HWND]
    user32.IsIconic.argtypes = [wintypes.HWND]
    user32.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
    user32.GetWindowDC.argtypes = [wintypes.HWND]
    user32.GetWindowDC.restype = wintypes.HDC
    user32.ReleaseDC.argtypes = [wintypes.HWND, wintypes.HDC]
    user32.PrintWindow.argtypes = [wintypes.HWND, wintypes.HDC, wintypes.UINT]
    gdi32.CreateCompatibleDC.argtypes = [wintypes.HDC]
    gdi32.CreateCompatibleDC.restype = wintypes.HDC
    gdi32.CreateCompatibleBitmap.argtypes = [wintypes.HDC, ctypes.c_int, ctypes.c_int]
    gdi32.CreateCompatibleBitmap.restype = wintypes.HBITMAP
    gdi32.SelectObject.argtypes = [wintypes.HDC, wintypes.HGDIOBJ]
    gdi32.SelectObject.restype = wintypes.HGDIOBJ
    gdi32.BitBlt.argtypes = [wintypes.HDC, ctypes.c_int, ctypes.c_int, ctypes.c_int,
                            ctypes.c_int, wintypes.HDC, ctypes.c_int, ctypes.c_int, wintypes.DWORD]
    gdi32.GetDIBits.argtypes = [wintypes.HDC, wintypes.HBITMAP, wintypes.UINT, wintypes.UINT,
                               ctypes.c_void_p, ctypes.POINTER(BITMAPINFO), wintypes.UINT]
    gdi32.DeleteObject.argtypes = [wintypes.HGDIOBJ]
    gdi32.DeleteDC.argtypes = [wintypes.HDC]
    kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel32.OpenProcess.restype = wintypes.HANDLE
    kernel32.QueryFullProcessImageNameW.argtypes = [wintypes.HANDLE, wintypes.DWORD,
                                                    wintypes.LPWSTR, ctypes.POINTER(wintypes.DWORD)]
    kernel32.CloseHandle.argtypes = [wintypes.HANDLE]

    windows: list[int] = []
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

    def inspect(hwnd: int, _param: int) -> bool:
        if not user32.IsWindowVisible(hwnd):
            return True
        title = ctypes.create_unicode_buffer(256)
        user32.GetWindowTextW(hwnd, title, len(title))
        if "civilization vi" not in title.value.casefold():
            return True
        pid = wintypes.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        process = kernel32.OpenProcess(0x1000, False, pid.value)
        if not process:
            return True
        try:
            executable = ctypes.create_unicode_buffer(1024)
            length = wintypes.DWORD(len(executable))
            if kernel32.QueryFullProcessImageNameW(process, 0, executable, ctypes.byref(length)):
                if Path(executable.value).name.casefold() in ("civilizationvi.exe", "civilizationvi_dx12.exe"):
                    windows.append(hwnd)
        finally:
            kernel32.CloseHandle(process)
        return True

    callback = callback_type(inspect)
    if not user32.EnumWindows(callback, 0) or len(windows) != 1:
        raise CaptureError(f"Expected one visible Civ VI window; found {len(windows)}")
    hwnd = windows[0]
    if user32.IsIconic(hwnd):
        raise CaptureError("Civ VI window is minimized; restore it before capture")
    rect = wintypes.RECT()
    if not user32.GetWindowRect(hwnd, ctypes.byref(rect)):
        raise CaptureError("GetWindowRect failed")
    width, height = rect.right - rect.left, rect.bottom - rect.top
    if width <= 0 or height <= 0:
        raise CaptureError("Civ VI window has no visible area")

    source = user32.GetWindowDC(hwnd)
    if not source:
        raise CaptureError("GetWindowDC failed")
    memory = gdi32.CreateCompatibleDC(source)
    bitmap = gdi32.CreateCompatibleBitmap(source, width, height) if memory else None
    previous = gdi32.SelectObject(memory, bitmap) if bitmap else None
    try:
        if not bitmap or not previous:
            raise CaptureError("Could not allocate capture bitmap")
        info = BITMAPINFO()
        info.bmiHeader.biSize = ctypes.sizeof(BITMAPINFOHEADER)
        info.bmiHeader.biWidth = width
        info.bmiHeader.biHeight = -height
        info.bmiHeader.biPlanes = 1
        info.bmiHeader.biBitCount = 32
        pixels = (ctypes.c_ubyte * (width * height * 4))()

        def read_pixels() -> bytes:
            # GetDIBits requires the bitmap to be deselected from its memory DC.
            gdi32.SelectObject(memory, previous)
            try:
                copied = gdi32.GetDIBits(source, bitmap, 0, height, pixels, ctypes.byref(info), 0)
            finally:
                gdi32.SelectObject(memory, bitmap)
            if copied != height:
                raise CaptureError(f"GetDIBits returned {copied}/{height} rows")
            return bytes(pixels)

        printed = bool(user32.PrintWindow(hwnd, memory, 2))
        raw = read_pixels() if printed else b""
        if not printed or not any(raw):
            if not gdi32.BitBlt(memory, 0, 0, width, height, source, 0, 0, 0x00CC0020):
                raise CaptureError("PrintWindow and BitBlt both failed")
            raw = read_pixels()
        if not any(raw):
            raise CaptureError("Capture is blank; make the Civ VI window visible and unobscured")
        if crop is not None:
            left, top, right, bottom = (int(crop[0] * width), int(crop[1] * height),
                                        int(crop[2] * width), int(crop[3] * height))
            if right <= left or bottom <= top:
                raise CaptureError("Crop produced an empty image")
            raw = b"".join(raw[y * width * 4 + left * 4:y * width * 4 + right * 4]
                           for y in range(top, bottom))
            width, height = right - left, bottom - top
        _write_png(path, width, height, raw)
    finally:
        if previous:
            gdi32.SelectObject(memory, previous)
        if bitmap:
            gdi32.DeleteObject(bitmap)
        if memory:
            gdi32.DeleteDC(memory)
        user32.ReleaseDC(hwnd, source)
