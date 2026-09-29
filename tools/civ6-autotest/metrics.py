"""Lightweight per-turn Civ VI timing and process-memory samples."""

from __future__ import annotations

import ctypes
from ctypes import wintypes
from datetime import datetime, timezone
import json
from pathlib import Path
import time


PROCESS_QUERY_INFORMATION = 0x0400
PROCESS_VM_READ = 0x0010


class PROCESS_MEMORY_COUNTERS_EX(ctypes.Structure):
    _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD),
                ("PeakWorkingSetSize", ctypes.c_size_t), ("WorkingSetSize", ctypes.c_size_t),
                ("QuotaPeakPagedPoolUsage", ctypes.c_size_t), ("QuotaPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t), ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
                ("PagefileUsage", ctypes.c_size_t), ("PeakPagefileUsage", ctypes.c_size_t),
                ("PrivateUsage", ctypes.c_size_t)]


def process_memory(pid: int) -> dict[str, int]:
    kernel32 = ctypes.windll.kernel32
    psapi = ctypes.windll.psapi
    kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel32.OpenProcess.restype = wintypes.HANDLE
    psapi.GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(PROCESS_MEMORY_COUNTERS_EX),
                                           wintypes.DWORD]
    psapi.GetProcessMemoryInfo.restype = wintypes.BOOL
    handle = kernel32.OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, False, pid)
    if not handle:
        raise OSError(f"Cannot open Civ VI process {pid} for memory sample")
    try:
        counters = PROCESS_MEMORY_COUNTERS_EX()
        counters.cb = ctypes.sizeof(counters)
        if not psapi.GetProcessMemoryInfo(handle, ctypes.byref(counters), counters.cb):
            raise OSError(f"Cannot sample Civ VI process {pid} memory")
        return {"working_set_bytes": counters.WorkingSetSize,
                "private_bytes": counters.PrivateUsage}
    finally:
        kernel32.CloseHandle(handle)


class TurnMetrics:
    """Observed turn changes; missed polling intervals remain explicit nulls."""

    def __init__(self, path: Path, start_turn: int, *, clock=time.monotonic):
        self.path = path
        self.turn = start_turn
        self.clock = clock
        self.last_at = clock()
        self.start_at = self.last_at
        self.observed_turns = 0
        self.missed_turns = 0

    def _write(self, row: dict) -> None:
        with self.path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(row) + "\n")

    def start(self, memory: dict[str, int]) -> None:
        self._write({"phase": "start", "at": datetime.now(timezone.utc).isoformat(),
                     "turn": self.turn, "elapsed_seconds": 0, "memory": memory})

    def sample(self, turn: int, memory: dict[str, int]) -> None:
        if turn < self.turn:
            raise ValueError(f"Turn moved backwards: {self.turn} -> {turn}")
        if turn == self.turn:
            return
        now = self.clock()
        delta = turn - self.turn
        for value in range(self.turn + 1, turn):
            self._write({"phase": "turn", "at": datetime.now(timezone.utc).isoformat(),
                         "turn": value, "observed": False, "duration_seconds": None,
                         "elapsed_seconds": round(now - self.start_at, 3), "memory": None})
            self.missed_turns += 1
        self._write({"phase": "turn", "at": datetime.now(timezone.utc).isoformat(),
                     "turn": turn, "observed": True,
                     "duration_seconds": round(now - self.last_at, 3) if delta == 1 else None,
                     "elapsed_seconds": round(now - self.start_at, 3), "memory": memory})
        self.observed_turns += 1
        self.turn = turn
        self.last_at = now
