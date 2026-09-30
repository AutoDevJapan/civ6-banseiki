"""Civ VI process lifecycle and scenario MOD freshness checks (Windows only)."""

from __future__ import annotations

import ctypes
from ctypes import wintypes
import mmap
import os
from pathlib import Path
import time
import xml.etree.ElementTree as ET

from click import (InputError, PROCESS_QUERY_LIMITED_INFORMATION, _civ_process_ids, _process_ids,
                   find_civ_window, kernel32)


DEFAULT_MODS_DIR = (Path.home() / "Documents" / "My Games" /
                    "Sid Meier's Civilization VI" / "Mods")
STEAM_URI = "steam://rungameid/289070"
PROCESS_TERMINATE = 0x0001
EPOCH_DIFFERENCE = 11644473600

kernel32.GetProcessTimes.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.FILETIME),
                                     ctypes.POINTER(wintypes.FILETIME), ctypes.POINTER(wintypes.FILETIME),
                                     ctypes.POINTER(wintypes.FILETIME)]
kernel32.GetProcessTimes.restype = wintypes.BOOL
kernel32.TerminateProcess.argtypes = [wintypes.HANDLE, wintypes.UINT]
kernel32.TerminateProcess.restype = wintypes.BOOL


def enabled_mod_folders(scenario: dict, mods_dir: Path = DEFAULT_MODS_DIR) -> list[Path]:
    """Match enabled UUIDs to installed modinfo files, following Mods junctions."""
    ids = {mod_id.casefold() for step in scenario["steps"] if step["action"] == "enable_mods"
           for mod_id in step["ids"]}
    if not ids:
        return []
    found: dict[str, Path] = {}
    for entry in mods_dir.iterdir():
        if not entry.is_dir():
            continue
        folder = entry.resolve(strict=True)
        for info in folder.glob("*.modinfo"):
            try:
                mod_id = ET.parse(info).getroot().attrib.get("id", "").casefold()
            except ET.ParseError:
                continue
            if mod_id in ids:
                if mod_id in found:
                    raise InputError(f"Duplicate installed mod ID {mod_id}: {found[mod_id]} and {folder}")
                found[mod_id] = folder
    missing = ids - found.keys()
    if missing:
        raise InputError(f"Scenario mod is not installed under {mods_dir}: {', '.join(sorted(missing))}")
    return [found[mod_id] for mod_id in sorted(ids)]


def newest_mod_file(folders: list[Path]) -> tuple[Path, float] | None:
    newest: tuple[Path, float] | None = None

    def on_error(error: OSError) -> None:
        raise error

    for folder in folders:
        for parent, _, files in os.walk(folder, onerror=on_error, followlinks=False):
            for name in files:
                path = Path(parent) / name
                modified = path.stat().st_mtime_ns / 1_000_000_000
                if newest is None or modified > newest[1]:
                    newest = path, modified
    if folders and newest is None:
        raise InputError("Enabled mod folders contain no files")
    return newest


def missing_atlas_textures(folders: list[Path]) -> list[str]:
    """Icon atlas images (IconTextureAtlases Filename) that no cooked package or loose DDS of the same mod holds.

    The icon database rows load even when the BLP is stale, so IconManager:FindIconAtlas still names the sheet, but
    the game then draws the template texture: the official CivilizationIcon (Base/Assets/UI/Instances/
    CivilizationIcon.lua:42-65) showed the "?" of CivSymbols36 for all 22 crests in ages-screens-01, because the
    installed JAC_CustomCivs_Icons.blp predated the crests (#54). The same byte search as
    tools/jac_art/tests/test_frontend_art.py package_textures.
    """
    missing: list[str] = []
    for folder in folders:
        names: set[str] = set()
        for path in folder.rglob("*.xml"):
            try:
                root = ET.parse(path).getroot()
            except ET.ParseError:
                continue
            names.update(Path(row.attrib["Filename"]).stem for row in root.iterfind("./IconTextureAtlases/Row")
                         if row.attrib.get("Filename"))
        names -= {path.stem for path in folder.rglob("*.dds")}
        for package in sorted(folder.rglob("*.blp")):
            if not names or package.stat().st_size == 0:
                continue
            with package.open("rb") as file, mmap.mmap(file.fileno(), 0, access=mmap.ACCESS_READ) as data:
                names = {name for name in names if data.find(name.encode("ascii")) < 0}
        missing += [f"{folder.name}: {name}" for name in sorted(names)]
    return missing


def restart_reason(newest: tuple[Path, float] | None, process_start: float | None) -> str | None:
    """Pure freshness decision; mtimes and process start are Unix UTC seconds."""
    if process_start is None:
        return "Civ6 is not running"
    if newest is not None and newest[1] > process_start:
        path, modified = newest
        return f"mod file newer than Civ6 process: {path} (mtime={modified:.3f}, started={process_start:.3f})"
    return None


def process_start_time(pid: int) -> float:
    process = kernel32.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
    if not process:
        raise InputError(f"Cannot inspect Civ VI process {pid}")
    try:
        created, exited, kernel, user = (wintypes.FILETIME() for _ in range(4))
        if not kernel32.GetProcessTimes(process, ctypes.byref(created), ctypes.byref(exited),
                                        ctypes.byref(kernel), ctypes.byref(user)):
            raise InputError(f"Cannot read Civ VI process start time for {pid}")
        ticks = (created.dwHighDateTime << 32) | created.dwLowDateTime
        return ticks / 10_000_000 - EPOCH_DIFFERENCE
    finally:
        kernel32.CloseHandle(process)


def single_civ_process(settle: float = 30.0) -> tuple[int, float] | None:
    """Return the only Civ VI process. If several exist, wait up to `settle` seconds
    for exiting ones to vanish; never kills anything, still refuses when they persist."""
    pids = _civ_process_ids()
    end = time.monotonic() + settle
    while len(pids) > 1 and time.monotonic() < end:
        time.sleep(min(0.5, max(0.0, end - time.monotonic())))
        pids = _civ_process_ids()
    if len(pids) > 1:
        raise InputError(f"Multiple Civ VI processes found ({len(pids)}); driver requires one local game")
    if not pids:
        return None
    pid = next(iter(pids))
    try:
        return pid, process_start_time(pid)
    except InputError:
        if pid not in _civ_process_ids():
            return None  # It exited between enumeration and the start-time query.
        raise


def wait_for_exit(pid: int, deadline: float) -> bool:
    while time.monotonic() < deadline:
        if pid not in _civ_process_ids():
            return True
        time.sleep(min(0.5, max(0.0, deadline - time.monotonic())))
    return pid not in _civ_process_ids()


def terminate_civ_process(pid: int, expected_start: float) -> None:
    # Recheck both executable name and creation time before forceful termination.
    pids = _civ_process_ids()
    if len(pids) > 1:
        raise InputError(f"Multiple Civ VI processes found ({len(pids)}); refusing termination")
    if pid not in pids:
        return
    if process_start_time(pid) != expected_start:
        raise InputError(f"Civ VI process {pid} changed before termination")
    process = kernel32.OpenProcess(PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
    if not process:
        raise InputError(f"Cannot open Civ VI process {pid} for termination")
    try:
        if not kernel32.TerminateProcess(process, 1):
            raise InputError(f"Could not terminate Civ VI process {pid}")
    finally:
        kernel32.CloseHandle(process)


def close_orphan_crash_reporters() -> int:
    """A Civ6 crash reporter can keep Steam's game session alive after the game exits."""
    if _civ_process_ids():
        return 0
    closed = 0
    for pid in sorted(_process_ids({"firaxisbugreporter.exe"})):
        process = kernel32.OpenProcess(PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
        if not process:
            continue  # It may have exited since enumeration.
        try:
            executable = ctypes.create_unicode_buffer(32768)
            length = wintypes.DWORD(len(executable))
            if not kernel32.QueryFullProcessImageNameW(process, 0, executable, ctypes.byref(length)):
                raise InputError(f"Cannot identify crash reporter {pid}")
            path = Path(executable.value)
            expected = ("sid meier's civilization vi", "base", "binaries", "win64steam", "firaxisbugreporter.exe")
            if tuple(part.casefold() for part in path.parts[-5:]) != expected:
                continue  # Other Firaxis games use the same executable name.
            if _civ_process_ids():
                return closed  # A new game appeared; do not touch its reporter.
            # Verify and terminate the same handle, so PID reuse cannot target another process.
            if not kernel32.TerminateProcess(process, 1):
                raise InputError(f"Could not close Civ VI crash reporter {pid}")
            closed += 1
        finally:
            kernel32.CloseHandle(process)
    return closed


def launch_via_steam() -> None:
    closed = close_orphan_crash_reporters()
    if closed:
        print(f"Closed {closed} orphan Civ VI crash reporter(s)", flush=True)
    os.startfile(STEAM_URI)


def wait_for_civ_window(deadline: float) -> int:
    last = "no Civ VI process yet"
    while time.monotonic() < deadline:
        try:
            process = single_civ_process(settle=0)
        except InputError as exc:
            if not str(exc).startswith("Multiple Civ VI processes found"):
                raise
            last = str(exc)
            process = None  # A short launcher overlap must settle before any input.
        if process is not None:
            try:
                return find_civ_window()
            except InputError as exc:
                last = str(exc)
        time.sleep(min(0.5, max(0.0, deadline - time.monotonic())))
    raise InputError(f"Civ VI window did not appear after Steam launch: {last}")
