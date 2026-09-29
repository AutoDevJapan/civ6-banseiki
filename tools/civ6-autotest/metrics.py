"""Lightweight per-turn Civ VI timing and process-memory samples."""

from __future__ import annotations

import ctypes
from ctypes import wintypes
from datetime import datetime, timezone
import json
from pathlib import Path
import re
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
