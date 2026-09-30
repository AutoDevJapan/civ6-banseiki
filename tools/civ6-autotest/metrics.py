"""Lightweight per-turn Civ VI timing and process-memory samples."""

from __future__ import annotations

import ctypes
from ctypes import wintypes
import csv
from datetime import datetime, timezone
import json
import math
from pathlib import Path
import re
import statistics
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
    kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel32.CloseHandle.restype = wintypes.BOOL
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

    COLUMNS = ("phase", "at", "turn", "observed", "duration_seconds", "elapsed_seconds",
               "working_set_bytes", "private_bytes")

    def __init__(self, path: Path, start_turn: int, *, clock=time.monotonic,
                 started_at: float | None = None):
        self.path = path
        self.csv_path = path.with_suffix(".csv")
        self.start_turn = start_turn
        self.turn = start_turn
        self.clock = clock
        self.last_at = clock() if started_at is None else started_at
        self.start_at = self.last_at
        self.observed_turns = 0
        self.missed_turns = 0
        self.rows: list[dict] = []

    def _write(self, row: dict) -> None:
        with self.path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(row) + "\n")
        new_file = not self.csv_path.exists()
        with self.csv_path.open("a", encoding="utf-8", newline="") as handle:
            writer = csv.DictWriter(handle, fieldnames=self.COLUMNS)
            if new_file:
                writer.writeheader()
            flat = {key: row.get(key) for key in self.COLUMNS}
            flat.update(row.get("memory") or {})
            writer.writerow(flat)
        self.rows.append(row)

    def start(self, memory: dict[str, int]) -> None:
        self._write({"phase": "start", "at": datetime.now(timezone.utc).isoformat(),
                     "turn": self.turn, "observed": True, "elapsed_seconds": 0, "memory": memory})

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

    def summary(self) -> dict:
        return {"jsonl": self.path.name, "csv": self.csv_path.name,
                "start_turn": self.start_turn, "last_turn": self.turn,
                **summarize_turns(self.rows)}


def _statistics(values: list[float | int]) -> dict:
    if not values:
        return {"count": 0, "min": None, "median": None, "p95": None, "max": None}
    ordered = sorted(values)
    return {"count": len(values), "min": ordered[0], "median": statistics.median(ordered),
            "p95": ordered[math.ceil(len(ordered) * 0.95) - 1], "max": ordered[-1]}


def summarize_turns(rows: list[dict]) -> dict:
    turns = [row for row in rows if row["phase"] == "turn"]
    durations = [row["duration_seconds"] for row in turns if row["duration_seconds"] is not None]
    summary = {"observed_turns": sum(row["observed"] for row in turns),
               "missed_turns": sum(not row["observed"] for row in turns),
               "duration_seconds": _statistics(durations)}
    for key in ("working_set_bytes", "private_bytes"):
        values = [row["memory"][key] for row in rows if row.get("memory") is not None]
        summary[key] = {**_statistics(values), "first": values[0] if values else None,
                        "last": values[-1] if values else None,
                        "growth": values[-1] - values[0] if values else None}
    return summary


class SaveLoadMetrics:
    COLUMNS = ("at", "action", "save_name", "turn", "save_bytes", "duration_seconds")

    def __init__(self, path: Path):
        self.path = path
        self.rows: list[dict] = []

    def record(self, action: str, name: str, turn: int, size: int, seconds: float) -> dict:
        row = {"at": datetime.now(timezone.utc).isoformat(), "action": action,
               "save_name": name, "turn": turn, "save_bytes": size,
               "duration_seconds": round(seconds, 3)}
        new_file = not self.path.exists()
        with self.path.open("a", encoding="utf-8", newline="") as handle:
            writer = csv.DictWriter(handle, fieldnames=self.COLUMNS)
            if new_file:
                writer.writeheader()
            writer.writerow(row)
        self.rows.append(row)
        return row

    def summary(self) -> dict:
        saves = [row["save_bytes"] for row in self.rows if row["action"] == "save"]
        loads = [row["duration_seconds"] for row in self.rows if row["action"] == "load"]
        return {"csv": self.path.name if self.rows else None, "samples": self.rows,
                "save_bytes": {**_statistics(saves), "growth": saves[-1] - saves[0] if saves else None},
                "load_seconds": _statistics(loads)}


# The official research and civic panels read these same player APIs. The payload
# uses only numeric fields and game type IDs, which cannot contain our separators.
RESEARCH_SAMPLE_CODE = """
local rows = {}
local configured = nil
local function current(info, index, key)
  if index == nil or index < 0 then return '' end
  local row = info[index]
  return row and row[key] or ''
end
for _, id in ipairs(configured or GameConfiguration.GetParticipatingPlayerIDs()) do
  local player = Players[id]
  if player and (configured or player:IsMajor()) then
    local techs, civics = {}, {}
    local research, culture = player:GetTechs(), player:GetCulture()
    for tech in GameInfo.Technologies() do
      if research:HasTech(tech.Index) then techs[#techs + 1] = tech.TechnologyType end
    end
    for civic in GameInfo.Civics() do
      if culture:HasCivic(civic.Index) then civics[#civics + 1] = civic.CivicType end
    end
    table.sort(techs)
    table.sort(civics)
    rows[#rows + 1] = table.concat({id,
      PlayerConfigurations[id]:GetLeaderTypeName(), tostring(player:IsAlive()),
      #techs, #civics,
      current(GameInfo.Technologies, research:GetResearchingTech(), 'TechnologyType'),
      current(GameInfo.Civics, culture:GetProgressingCivic(), 'CivicType'),
      table.concat(techs, ','), table.concat(civics, ',')}, '~')
  end
end
return tostring(Game.GetCurrentGameTurn()) .. '|' .. table.concat(rows, ';')
"""


def research_sample_code(slots: set[int]) -> str:
    """Keep the original major slots even if participation changes after defeat."""
    if not slots or any(type(slot) is not int or slot < 0 for slot in slots):
        raise ValueError("Expected nonempty major slot IDs")
    literal = ",".join(str(slot) for slot in sorted(slots))
    return RESEARCH_SAMPLE_CODE.replace("local configured = nil", f"local configured = {{{literal}}}", 1)


_TYPE_ID = re.compile(r"^[A-Z][A-Z0-9_]+$")


def parse_research_sample(payload: str) -> tuple[int, list[dict]]:
    try:
        turn_text, entries = payload.split("|", 1)
        turn = int(turn_text)
        if turn < 0 or not entries:
            raise ValueError("empty or negative turn")
        rows = []
        seen = set()
        for entry in entries.split(";"):
            slot_text, leader, alive, tech_text, civic_text, current_tech, current_civic, techs, civics = entry.split("~")
            slot, tech_count, civic_count = int(slot_text), int(tech_text), int(civic_text)
            completed_techs = techs.split(",") if techs else []
            completed_civics = civics.split(",") if civics else []
            if (slot < 0 or slot in seen or alive not in ("true", "false")
                    or tech_count != len(completed_techs) or civic_count != len(completed_civics)
                    or any(not _TYPE_ID.fullmatch(value) for value in
                           [leader, *completed_techs, *completed_civics]
                           + ([current_tech] if current_tech else [])
                           + ([current_civic] if current_civic else []))):
                raise ValueError("invalid major research row")
            seen.add(slot)
            rows.append({"slot": slot, "leader": leader, "alive": alive == "true",
                         "tech_count": tech_count, "civic_count": civic_count,
                         "current_tech": current_tech or None, "current_civic": current_civic or None,
                         "completed_techs": completed_techs, "completed_civics": completed_civics})
        return turn, rows
    except (ValueError, TypeError) as exc:
        raise ValueError(f"Invalid research sample: {payload[:120]!r}") from exc


class ResearchMetrics:
    """One row per major at each observed turn; never invent skipped turn data."""

    def __init__(self, path: Path):
        self.path = path
        self.last_turn: int | None = None
        self.slots: set[int] | None = None
        self.rows: list[dict] = []

    def sample(self, payload: str) -> int:
        turn, rows = parse_research_sample(payload)
        slots = {row["slot"] for row in rows}
        if not slots:
            raise ValueError("No major slots in research sample")
        if self.slots is None:
            self.slots = slots
        if slots != self.slots:
            raise ValueError(f"Major slots changed during research sampling: {self.slots} -> {slots}")
        if self.last_turn is not None:
            if turn < self.last_turn:
                raise ValueError(f"Research turn moved backwards: {self.last_turn} -> {turn}")
            if turn == self.last_turn:
                return turn
        phase = "start" if self.last_turn is None else "turn"
        with self.path.open("a", encoding="utf-8") as handle:
            for row in rows:
                handle.write(json.dumps({"phase": phase, "turn": turn, "observed": True, **row}) + "\n")
        self.last_turn = turn
        self.rows = rows
        return turn
