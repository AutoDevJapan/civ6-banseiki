"""Summarize first observed research completion turns across major factions."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from statistics import median


def summarize(path: Path, kind: str = "tech") -> list[dict]:
    if kind not in ("tech", "civic"):
        raise ValueError("kind must be tech or civic")
    field = f"completed_{kind}s"
    first: dict[tuple[int, str], int] = {}
    baseline: set[tuple[int, str]] = set()
    with path.open(encoding="utf-8") as handle:
        for number, line in enumerate(handle, 1):
            try:
                row = json.loads(line)
                if row["phase"] not in ("start", "turn") or row["observed"] is not True:
                    raise ValueError("unobserved row")
                turn, slot, completed = row["turn"], row["slot"], row[field]
                if type(turn) is not int or turn < 0 or type(slot) is not int or slot < 0:
                    raise ValueError("invalid turn or slot")
                if not isinstance(completed, list) or any(not isinstance(item, str) for item in completed):
                    raise ValueError("invalid completed IDs")
            except (KeyError, TypeError, ValueError, json.JSONDecodeError) as exc:
                raise ValueError(f"Invalid research row {path}:{number}: {exc}") from exc
            for name in completed:
                key = (slot, name)
                if key not in first:
                    first[key] = turn
                    if row["phase"] == "start":
                        baseline.add(key)
    by_name: dict[str, list[tuple[int, int]]] = {}
    for (slot, name), turn in first.items():
        by_name.setdefault(name, []).append((slot, turn))
    result = []
    for name, faction_turns in by_name.items():
        values = [turn for _, turn in faction_turns]
        result.append({"id": name, "factions": len(values), "median_turn": median(values),
                       "min_turn": min(values), "max_turn": max(values),
                       "spread_turns": max(values) - min(values),
                       "baseline_factions": sum((slot, name) in baseline for slot, _ in faction_turns)})
    return sorted(result, key=lambda row: (row["median_turn"], row["id"]))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("research_jsonl", type=Path)
    parser.add_argument("--kind", choices=("tech", "civic"), default="tech")
    args = parser.parse_args()
    try:
        rows = summarize(args.research_jsonl, args.kind)
    except (OSError, ValueError) as exc:
        parser.exit(2, f"Report error: {exc}\n")
    print("id\tfactions\tmedian_turn\tmin_turn\tmax_turn\tspread_turns\tbaseline_factions")
    for row in rows:
        print("\t".join(str(row[key]) for key in
                        ("id", "factions", "median_turn", "min_turn", "max_turn",
                         "spread_turns", "baseline_factions")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
