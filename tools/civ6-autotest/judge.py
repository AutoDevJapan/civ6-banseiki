"""Judge only log bytes produced after a captured run baseline."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re


LOG_NAMES = ("Lua.log", "Database.log", "Modding.log")
BUNDLE0_ID = "dfa0717e-4d0b-4370-b693-27fc6758571b"
GROWTH_ID = "7696e937-2044-4f3c-8e13-4e27632e2710"
TURN = re.compile(r"\bturn=(\d+)\s+firstTurn=(\d+)\s+turnsSeen=(\d+)")
RESTORED = re.compile(r"\brestored firstTurn=(\d+) lastTurn=(\d+) turnsSeen=(\d+)")
GROWTH_RECORD = re.compile(
    r"\b(?P<kind>assigned |restored |)unit=(?P<unit>\d+) level=(?P<level>\d+) xp=(?P<xp>\d+) "
    r"personality=(?P<personality>\S+) trait=(?P<trait>\S+) equipment=(?P<equipment>\S+) "
    r"artifact=(?P<artifact>\S+) player=(?P<player>\d+)(?: turn=(?P<turn>\d+))?"
)


def default_log_dir() -> Path:
    local = os.environ.get("LOCALAPPDATA")
    if not local:
        raise RuntimeError("LOCALAPPDATA is unavailable; pass --logs")
    return Path(local) / "Firaxis Games" / "Sid Meier's Civilization VI" / "Logs"


def _prefix_hash(path: Path, length: int) -> str:
    with path.open("rb") as handle:
        return hashlib.sha256(handle.read(length)).hexdigest()


def snapshot_logs(log_dir: Path) -> dict:
    files = {}
    for name in LOG_NAMES:
        path = log_dir / name
        if path.exists():
            stat = path.stat()
            prefix_length = min(256, stat.st_size)
            files[name] = {"offset": stat.st_size, "inode": stat.st_ino,
                           "prefix_length": prefix_length,
                           "prefix_sha256": _prefix_hash(path, prefix_length)}
        else:
            files[name] = {"offset": 0, "inode": None, "prefix_length": 0,
                           "prefix_sha256": None}
    return {"created_at": datetime.now(timezone.utc).isoformat(), "log_dir": str(log_dir), "files": files}


def slice_log_bytes(contents: bytes, mark: dict, inode: int) -> str:
    """Return bytes after a marker, or all bytes if Civ recreated/truncated the log."""
    old_offset = mark["offset"]
    current_prefix = hashlib.sha256(contents[:mark.get("prefix_length", min(256, old_offset))]).hexdigest()
    replaced = (
        old_offset > len(contents)
        or (mark["inode"] is not None and mark["inode"] != inode)
        or (mark["prefix_sha256"] is not None and mark["prefix_sha256"] != current_prefix)
    )
    return contents[0 if replaced else old_offset:].decode("utf-8-sig", "replace")


def read_since(log_dir: Path, baseline: dict) -> dict[str, str]:
    result = {}
    for name in LOG_NAMES:
        path = log_dir / name
        if not path.is_file():
            raise FileNotFoundError(f"Required Civ6 log is missing: {path}")
        result[name] = slice_log_bytes(path.read_bytes(), baseline["files"][name], path.stat().st_ino)
    return result


@dataclass(frozen=True)
class Check:
    name: str
    passed: bool
    detail: str


def _jac_error_line(line: str) -> bool:
    if "[JAC_POC]" not in line:
        return False
    body = line.split("[JAC_POC]", 1)[1]
    return bool(
        re.search(r"\b(?:api_error|failed|failure|exception|runtime error)\b", body, re.I)
        or re.search(r"\berror\s*=\s*(?!none\b|nil\b)[^\s]+", body, re.I)
        or re.search(r"\berror\b(?!\s*=\s*(?:none|nil)\b)", body, re.I)
    )


def _runtime_errors(lua: str) -> list[str]:
    lines = lua.splitlines()
    found = []
    for index, line in enumerate(lines):
        if "Runtime Error" in line and re.search(r"\bJAC[_\\/.-]|JAC_PoC", "\n".join(lines[index:index + 9]), re.I):
            found.append(line)
    return found


def _database_errors(database: str) -> list[str]:
    lines = database.splitlines()
    found = []
    for index, line in enumerate(lines):
        if "ERROR" not in line.upper():
            continue
        context = "\n".join(lines[index:index + 7])
        if re.search(r"JAC[_\\/.-]|JAC_PoC", context, re.I):
            found.append(line)
    return found


def _bundle0_checks(logs: dict[str, str]) -> list[Check]:
    lua = logs["Lua.log"]
    modding = logs["Modding.log"]
    database = logs["Database.log"]
    own = [line for line in lua.splitlines() if line.startswith("JAC_PoC_Bundle0:")]
    loaded = [i for i, line in enumerate(own) if "[JAC_POC] loaded version=1" in line]
    new = [i for i, line in enumerate(own) if "[JAC_POC] no saved state (new game)" in line]
    restored = [(i, RESTORED.search(line)) for i, line in enumerate(own) if RESTORED.search(line)]
    turns = [(i, TURN.search(line)) for i, line in enumerate(own) if TURN.search(line)]
    checks = [
        Check("Bundle0: mod registered", BUNDLE0_ID.lower() in modding.lower(), "Modding.log contains mod ID"),
        Check("Bundle0: database action", "Applying Component - JAC_PoC_Bundle0_Gameplay (UpdateDatabase)" in modding, "UpdateDatabase applied"),
        Check("Bundle0: script action", "Applying Component - JAC_PoC_Bundle0_Scripts (AddGameplayScripts)" in modding, "AddGameplayScripts applied"),
        Check("Bundle0: database errors", not _database_errors(database), "JAC-related Database.log errors: " + str(len(_database_errors(database)))),
        Check("Bundle0: initial load", bool(loaded), "loaded version=1"),
        Check("Bundle0: new game", bool(new and loaded and new[0] > loaded[0]), "no saved state after initial load"),
    ]
    before_and_after = bool(restored and turns)
    if before_and_after:
        restore_index, match = restored[0]
        before = [m for i, m in turns if i < restore_index]
        after = [m for i, m in turns if i > restore_index]
        before_and_after = bool(before and after and match)
    if before_and_after:
        first, last, count = map(int, match.groups())
        previous = before[-1]
        following = after[0]
        saved_state_ok = (
            first == int(previous[2])
            and last == int(previous[1])
            and count == int(previous[3])
        )
        next_turn_ok = (
            int(following[1]) == last + 1
            and int(following[2]) == first
            and int(following[3]) == count + 1
        )
        counts = [int(item[3]) for item in before]
        progression_ok = len(counts) >= 2 and all(b == a + 1 for a, b in zip(counts, counts[1:]))
    else:
        saved_state_ok = next_turn_ok = progression_ok = False
    checks.extend([
        Check("Bundle0: turns progress", progression_ok, "turnsSeen increases by one before save"),
        Check("Bundle0: restored values", saved_state_ok, "restored values match last pre-save turn"),
        Check("Bundle0: post-load turn", next_turn_ok, "turn and turnsSeen both increase by one"),
    ])
    return checks


def _growth_checks(logs: dict[str, str]) -> list[Check]:
    lua, modding, database = (logs[name] for name in ("Lua.log", "Modding.log", "Database.log"))
    own = [line for line in lua.splitlines() if line.startswith("JAC_PoC_GrowthUI_Gameplay:")]
    parsed = [(i, match) for i, line in enumerate(own) if (match := GROWTH_RECORD.search(line))]
    restores = [(i, m) for i, m in parsed if m["kind"] == "restored "]
    restore_at = restores[0][0] if restores else len(own)
    assigned = [m for i, m in parsed if m["kind"] == "assigned " and i < restore_at]
    before = [m for i, m in parsed if m["kind"] == "" and i < restore_at]
    after = [m for i, m in parsed if m["kind"] == "" and i > restore_at]
    initial_count = re.search(r"\binitialized count=(\d+)", "\n".join(own[:restore_at]))
    restored_count = re.search(r"\brestored count=(\d+)", "\n".join(own[restore_at:]))
    components = ("Text", "Gameplay", "InGameUI", "PartialScreenHooks", "UIFiles")
    checks = [
        Check("Growth: mod registered", GROWTH_ID in modding.lower(), "GrowthUI mod ID in Modding.log"),
        Check("Growth: five actions", all(f"Applying Component - JAC_GrowthUI_{part} " in modding for part in components),
              "UpdateText, AddGameplayScripts, AddUserInterfaces, ReplaceUIScript, ImportFiles"),
        Check("Growth: database errors", not _database_errors(database),
              f"JAC-related errors: {len(_database_errors(database))}"),
        Check("Growth: button attached", "[JAC_POC] notebooks button attached parent=ButtonStack" in lua,
              "parent=ButtonStack"),
        Check("Growth: partial-screen path", "[JAC_POC] notebooks path lookup found=true" in lua
              and "[JAC_POC] notebooks path lookup found=false" not in lua, "lookup true, never false"),
        Check("Growth: open and journal", lua.count("[JAC_POC] notebooks opened") >= 3
              and lua.count("[JAC_POC] character journal shown count=") >= 2,
              f"opened={lua.count('[JAC_POC] notebooks opened')}, journal={lua.count('[JAC_POC] character journal shown count=')}"),
        Check("Growth: partial-screen errors", "UI.DataError" not in lua
              and "Cannot determine if partial screen" not in lua and "Runtime Error" not in lua,
              "no UI.DataError or Runtime Error in run log"),
        Check("Growth: new-game assignment", "[JAC_POC] no saved state (new game)" in "\n".join(own[:restore_at])
              and initial_count is not None and 1 <= len(assigned) <= 3
              and int(initial_count[1]) == len(assigned)
              and all(m["level"] == "1" and m["xp"] == "0" for m in assigned)
              and (not assigned or assigned[0]["equipment"] == "JAC_ITEM_POC_BLADE,JAC_ITEM_POC_CLOAK"
                   and assigned[0]["artifact"] == "JAC_ARTIFACT_POC_RELIC")
              and (len(assigned) < 2 or assigned[1]["equipment"] == "JAC_ITEM_POC_BLADE"
                   and assigned[1]["artifact"] == "none"),
              f"assigned={len(assigned)}, initialized={initial_count[1] if initial_count else '?'}"),
    ]

    # Every surviving record must gain exactly 20 XP on each successive human turn.
    by_unit: dict[tuple[str, str], list[re.Match]] = {}
    for match in before:
        by_unit.setdefault((match["player"], match["unit"]), []).append(match)
    growth_ok = bool(by_unit and len({int(m["turn"]) for m in before}) >= 4)
    level_two_seen = False
    for records in by_unit.values():
        level, xp = 1, 0
        previous_turn = None
        for match in records:
            turn = int(match["turn"])
            if previous_turn is not None and turn != previous_turn + 1:
                growth_ok = False
            xp += 20
            while level < 30:
                needed = int(30 * level ** 1.5 + 0.5)
                if xp < needed:
                    break
                xp -= needed
                level += 1
            if (int(match["level"]), int(match["xp"])) != (level, xp):
                growth_ok = False
            if level == 2 and xp == 10:
                level_two_seen = True
            previous_turn = turn
    checks.append(Check("Growth: 20 XP each turn", growth_ok and level_two_seen,
                        f"turns={len({m['turn'] for m in before})}, Lv2/10={level_two_seen}"))

    last_turn = max((int(m["turn"]) for m in before), default=-1)
    last_rows = {(m["player"], m["unit"]): m for m in before if int(m["turn"]) == last_turn}
    restored_rows = {(m["player"], m["unit"]): m for _, m in restores}
    fields = ("level", "xp", "personality", "trait", "equipment", "artifact")
    same = bool(last_rows and last_rows.keys() == restored_rows.keys()
                and all(all(last_rows[key][field] == restored_rows[key][field] for field in fields)
                        for key in last_rows))
    journal_entries = [(m.start(), int(m[1])) for m in re.finditer(
        r"JAC_Notebooks: \[JAC_POC\] character journal shown count=(\d+)", lua)]
    restore_position = lua.find("JAC_PoC_GrowthUI_Gameplay: [JAC_POC] restored unit=")
    before_journal = [count for position, count in journal_entries if position < restore_position]
    after_journal = [count for position, count in journal_entries if position > restore_position]
    counts_ok = bool(restored_count and int(restored_count[1]) == len(restored_rows)
                     and before_journal and after_journal
                     and before_journal[-1] == after_journal[0] == len(restored_rows))
    checks.append(Check("Growth: restored rows and journal", same and counts_ok,
                        f"pre-save={len(last_rows)}, restored={len(restored_rows)}, "
                        f"journal={before_journal[-1:]}/{after_journal[:1]}"))
    next_rows = {(m["player"], m["unit"]): m for m in after if int(m["turn"]) == last_turn + 1}
    post_ok = bool(same and next_rows.keys() == restored_rows.keys()
                   and all((int(next_rows[key]["level"]), int(next_rows[key]["xp"])) ==
                           _growth_next(int(restored_rows[key]["level"]), int(restored_rows[key]["xp"]))
                           for key in next_rows)
                   and not any(i > restore_at and m["kind"] == "assigned " for i, m in parsed))
    checks.append(Check("Growth: one post-load turn", post_ok,
                        f"turn={last_turn + 1}, rows={len(next_rows)}, no reassignment"))
    return checks


def _growth_next(level: int, xp: int) -> tuple[int, int]:
    xp += 20
    while level < 30:
        needed = int(30 * level ** 1.5 + 0.5)
        if xp < needed:
            break
        xp -= needed
        level += 1
    return level, xp


def judge(logs: dict[str, str], profile: str = "bundle0") -> list[Check]:
    for name in LOG_NAMES:
        if name not in logs:
            raise ValueError(f"Missing log: {name}")
    jac_errors = [line for line in logs["Lua.log"].splitlines() if _jac_error_line(line)]
    runtime_errors = _runtime_errors(logs["Lua.log"])
    checks = [
        Check("Generic: [JAC_POC] errors", not jac_errors, f"{len(jac_errors)} matching lines"),
        Check("Generic: JAC Runtime Error", not runtime_errors, f"{len(runtime_errors)} matching stack traces"),
    ]
    if profile == "bundle0":
        checks.extend(_bundle0_checks(logs))
    elif profile == "growth":
        checks.extend(_growth_checks(logs))
    elif profile != "generic":
        raise ValueError(f"Unknown judge profile: {profile}")
    return checks


def print_table(checks: list[Check]) -> None:
    width = max(len(check.name) for check in checks)
    print(f"{'Result':<6}  {'Check':<{width}}  Detail")
    for check in checks:
        print(f"{'PASS' if check.passed else 'FAIL':<6}  {check.name:<{width}}  {check.detail}")


def main() -> int:
    parser = argparse.ArgumentParser(description="Judge Civ VI PoC logs from a byte-offset baseline")
    parser.add_argument("--logs", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--snapshot", type=Path, help="Write baseline before a manual run, then exit")
    parser.add_argument("--profile", choices=("bundle0", "generic", "growth"), default="bundle0")
    args = parser.parse_args()
    log_dir = args.logs or default_log_dir()
    if args.snapshot:
        args.snapshot.parent.mkdir(parents=True, exist_ok=True)
        args.snapshot.write_text(json.dumps(snapshot_logs(log_dir), indent=2), encoding="utf-8")
        print(args.snapshot)
        return 0
    if not args.baseline:
        parser.error("--baseline is required when judging")
    try:
        baseline = json.loads(args.baseline.read_text(encoding="utf-8"))
        checks = judge(read_since(log_dir, baseline), args.profile)
    except (OSError, KeyError, ValueError) as exc:
        parser.exit(2, f"Cannot judge logs: {exc}\n")
    print_table(checks)
    return 0 if all(check.passed for check in checks) else 1


if __name__ == "__main__":
    raise SystemExit(main())
