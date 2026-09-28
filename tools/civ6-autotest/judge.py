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
TURN = re.compile(r"\bturn=(\d+)\s+firstTurn=(\d+)\s+turnsSeen=(\d+)")
RESTORED = re.compile(r"\brestored firstTurn=(\d+) lastTurn=(\d+) turnsSeen=(\d+)")


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
    parser.add_argument("--profile", choices=("bundle0", "generic"), default="bundle0")
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
