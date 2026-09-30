"""Audit which official rows the Sengoku pack loses to foreign-key cascades.

Builds the Gathering Storm gameplay database (Base + Expansion2Core/Content/Major,
which also carries the XP1 content) from the local Civ6 install, runs the Sengoku
SQL twice with ``PRAGMA foreign_keys=ON`` -- once as shipped and once without the
two official-tree ``DELETE`` statements -- and reports the difference.  Only rows
changed by the tree deletions therefore show up; deliberate deletes (Policies,
Governments, UnitUpgrades) are not counted.  Other DLC and the repo's other mods
are not loaded.  Run ``python tools/sengoku_cascade_audit.py`` for a table.
"""

from __future__ import annotations

import re
import sqlite3
import xml.etree.ElementTree as ET
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GAME = Path("C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI")
BASE_DATA = GAME / "Base/Assets/Gameplay/Data"
XP2 = GAME / "DLC/Expansion2"
SENGOKU = ROOT / "poc/JAC_PoC_SengokuTrees"
TREE_DELETES = ("DELETE FROM Technologies WHERE TechnologyType NOT LIKE 'TECH_JAC_SENGOKU_%';",
                "DELETE FROM Civics WHERE CivicType NOT LIKE 'CIVIC_JAC_SENGOKU_%';")
XP2_COMPONENTS = ("Expansion2Core", "Expansion2CoreContent", "Expansion2MajorContent")


def available() -> bool:
    return (BASE_DATA / "Schema/01_GameplaySchema.sql").is_file() and (XP2 / "Expansion2.modinfo").is_file()


def _value(text: str):
    return {"true": 1, "false": 0}.get(text.lower(), text) if text is not None else None


def _fields(element: ET.Element) -> dict:
    fields = {k: _value(v) for k, v in element.attrib.items()}
    fields.update({child.tag: _value(child.text or "") for child in element})
    return fields


def _where(fields: dict) -> tuple[str, list]:
    if not fields:
        return "1", []
    return " AND ".join(f'"{k}"=?' for k in fields), list(fields.values())


def _load_xml(db: sqlite3.Connection, path: Path, tables: set[str]) -> None:
    root = ET.parse(path).getroot()
    for table in root:
        if table.tag not in tables:
            continue
        for op in table:
            if op.tag in ("Row", "Replace", "InsertOrIgnore"):
                row = _fields(op)
                verb = "INSERT OR REPLACE" if op.tag == "Replace" else "INSERT OR IGNORE"
                db.execute(f'{verb} INTO "{table.tag}" ({",".join(map(repr, row))}) '
                           f'VALUES ({",".join("?" * len(row))})', list(row.values()))
            elif op.tag == "Update":
                where, args = _where(_fields(op.find("Where")))
                sets = _fields(op.find("Set"))
                db.execute(f'UPDATE "{table.tag}" SET {",".join(f"{k!r}=?" for k in sets)} WHERE {where}',
                           list(sets.values()) + args)
            elif op.tag == "Delete":  # the game cascades these deletes (e.g. Expansion2_RemoveData.xml)
                where, args = _where(_fields(op))
                db.commit()
                db.execute("PRAGMA foreign_keys=ON")
                db.execute(f'DELETE FROM "{table.tag}" WHERE {where}', args)
                db.commit()
                db.execute("PRAGMA foreign_keys=OFF")


def _xp2_files() -> list[Path]:
    meta = ET.parse(XP2 / "Expansion2.modinfo").getroot()
    files = []
    for component in XP2_COMPONENTS:
        entries = meta.find(f"./InGameActions/UpdateDatabase[@id='{component}']").findall("File")
        entries = sorted(entries, key=lambda f: -int(f.get("Priority", 0)))  # stable: higher priority first
        files += [XP2 / f.text for f in entries]
    return files


def _register_audit_hash(db: sqlite3.Connection) -> None:
    """Collision-free stand-in for Make_Hash; these are audit IDs, not engine hashes.

    Python's randomized 31-bit hash sometimes collided in Types.Hash and silently
    dropped official rows (INSERT OR IGNORE) or rejected a mod. Preserve IDs when
    copying a database and assign a fresh one to each new type.
    """
    has_types = db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='Types'").fetchone()
    assigned = dict(db.execute("SELECT Type, Hash FROM Types")) if has_types else {}
    next_id = max(assigned.values(), default=0) + 1

    def make_hash(name: str) -> int:
        nonlocal next_id
        if name not in assigned:
            assigned[name] = next_id
            next_id += 1
        return assigned[name]

    db.create_function("Make_Hash", 1, make_hash)


def official_db() -> sqlite3.Connection:
    """In-memory copy of the official GS gameplay data (foreign keys off while loading)."""
    db = sqlite3.connect(":memory:")
    _register_audit_hash(db)
    for sql in ("Schema/01_GameplaySchema.sql", "Schema/02_AddTriggers.sql"):
        db.executescript((BASE_DATA / sql).read_text(encoding="utf-8-sig"))
    xp2 = _xp2_files()
    db.executescript(xp2[0].read_text(encoding="utf-8-sig"))  # Expansion2_Schema.sql
    tables = {r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
    for path in sorted(BASE_DATA.glob("*.xml")):
        _load_xml(db, path, tables)
    for path in xp2[1:]:
        _load_xml(db, path, tables)
    db.commit()
    return db


def _run_sengoku(source: sqlite3.Connection, with_deletes: bool) -> sqlite3.Connection:
    db = sqlite3.connect(":memory:")
    source.backup(db)
    _register_audit_hash(db)
    db.execute("PRAGMA foreign_keys=ON")
    meta = ET.parse(SENGOKU / "JAC_PoC_SengokuTrees.modinfo").getroot()
    for file in meta.findall("./InGameActions/UpdateDatabase/File"):  # modinfo order
        if not file.text.endswith(".sql"):
            continue
        script = (SENGOKU / file.text).read_text(encoding="utf-8")
        if not with_deletes:
            for statement in TREE_DELETES:
                assert statement in script or not file.text.endswith("Unlocks.sql"), statement
                script = script.replace(statement, "")
        # Cascades fire per statement, but the violation check waits for COMMIT: the game reports
        # dangling keys only after a file loads, so rows may reference ones inserted later in the file.
        db.executescript(f"BEGIN; PRAGMA defer_foreign_keys=ON;\n{script}\nCOMMIT;")
    return db


def _rows(db: sqlite3.Connection, table: str) -> dict:
    return {r[0]: r[1:] for r in db.execute(f'SELECT rowid, * FROM "{table}"')}


def audit(source: sqlite3.Connection | None = None) -> dict[str, dict]:
    """{table: {"lost": [row dict], "changed": [(before, after)]}} caused only by the tree deletions."""
    source = source or official_db()
    shipped, kept = _run_sengoku(source, True), _run_sengoku(source, False)
    result: dict[str, dict] = defaultdict(lambda: {"lost": [], "changed": []})
    tables = [r[0] for r in kept.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")]
    for table in tables:
        if table in ("Technologies", "Civics"):
            continue
        cols = [c[1] for c in kept.execute(f'PRAGMA table_info("{table}")')]
        before, after = _rows(kept, table), _rows(shipped, table)
        for rowid, row in before.items():
            if rowid not in after:
                result[table]["lost"].append(dict(zip(cols, row)))
            elif after[rowid] != row:
                result[table]["changed"].append((dict(zip(cols, row)), dict(zip(cols, after[rowid]))))
    return dict(result)


def _key(row: dict) -> str:
    return next((str(v) for k, v in row.items() if k.endswith("Type") and v), str(next(iter(row.values()))))


if __name__ == "__main__":
    for table, diff in sorted(audit().items()):
        lost = sorted(_key(r) for r in diff["lost"])
        changed = sorted(f"{_key(a)}: " + ", ".join(f"{k} {a[k]}->{b[k]}" for k in a if a[k] != b[k])
                         for a, b in diff["changed"])
        print(f"{table}: lost {len(lost)}, changed {len(changed)}")
        for line in lost:
            print("  -", line)
        for line in changed:
            print("  ~", line)
