"""Stamp/check the initial-city marker in the checked-in historical maps.

The Cities table in each Civ6Map is the castle registry. This command can
update maps already generated before the marker was added to generate.py.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sqlite3


ROOT = Path(__file__).resolve().parents[2]
MAPS = ROOT / "poc/JAC_PoC_JapanMapScale/Maps"
MARKER = "BUILDING_JAC_REGISTERED_CASTLE"


def castle_plots(path: Path) -> set[int]:
    with sqlite3.connect(path) as db:
        rows = db.execute("SELECT Plot FROM Cities").fetchall()
        plots = {row[0] for row in rows}
        if not rows or len(rows) != len(plots):
            raise ValueError(f"Missing or duplicate map castles: {path}")
        return plots


def verify_or_stamp(stamp: bool = False) -> int:
    manifest_path = MAPS / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    counts = set()
    manifest_changed = False
    for item in manifest:
        path = MAPS / item["file"]
        plots = castle_plots(path)
        counts.add(len(plots))
        with sqlite3.connect(path) as db:
            before = {row[0] for row in db.execute(
                "SELECT Plot FROM Buildings WHERE BuildingType=?", (MARKER,))}
            if stamp and before != plots:
                db.execute("DELETE FROM Buildings WHERE BuildingType=?", (MARKER,))
                db.executemany("INSERT INTO Buildings VALUES (?, ?)",
                               ((MARKER, plot) for plot in sorted(plots)))
                db.commit()
                db.execute("VACUUM")
            after = {row[0] for row in db.execute(
                "SELECT Plot FROM Buildings WHERE BuildingType=?", (MARKER,))}
            if after != plots:
                raise ValueError(f"Castle markers do not match Cities: {path}")
            if db.execute("PRAGMA integrity_check").fetchone()[0] != "ok":
                raise ValueError(f"Invalid map database: {path}")
        size = path.stat().st_size
        if item["bytes"] != size:
            item["bytes"] = size
            manifest_changed = True
        settler_path = MAPS / item["settler_file"]
        with sqlite3.connect(settler_path) as db:
            if db.execute("SELECT COUNT(*) FROM Cities").fetchone()[0] or db.execute(
                "SELECT COUNT(*) FROM Buildings WHERE BuildingType=?", (MARKER,)).fetchone()[0]:
                raise ValueError(f"Settler map has registered castles: {settler_path}")
    if len(counts) != 1:
        raise ValueError(f"Historical maps have different castle counts: {counts}")
    if stamp and manifest_changed:
        manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
                                 encoding="utf-8")
    return counts.pop()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stamp", action="store_true")
    args = parser.parse_args()
    print(f"registered castles: {verify_or_stamp(args.stamp)}")
