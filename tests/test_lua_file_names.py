"""Every Lua file in the mods has a unique file name.

Civ6 resolves Lua by bare file name in the VFS: an imported file shadows any other file of the same name
(official: DLC/Expansion2/Expansion2.modinfo imports UI/Replacements/CitySupport.lua to replace
Base/Assets/UI/CitySupport.lua). When the court-arbitration UI and gameplay scripts shared the name
JAC_CourtArbitration.lua, the gameplay context loaded the UI file and failed on InstanceManager
(artifacts/court-arbitration-01/Lua.log).
"""

import unittest
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class LuaFileNameTests(unittest.TestCase):
    def test_lua_file_names_are_unique_across_all_mods(self):
        paths = defaultdict(list)
        for path in (ROOT / "poc").rglob("*.lua"):
            paths[path.name.lower()].append(path.relative_to(ROOT).as_posix())
        self.assertEqual({name: found for name, found in paths.items() if len(found) > 1}, {})


if __name__ == "__main__":
    unittest.main()
