"""Execute the driver's actual FrontEnd Lua against a dependency-enforcing mod graph."""
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

from lupa import LuaError, LuaRuntime

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scenario import Runner, ScenarioError, _enable_mods_code, validate_scenario


BASE, MIDDLE, LEAF, SIBLING, KEEP, OFF, MISSING = [
    f"abcdef{n:02x}-1234-1234-1234-123456789abc" for n in range(1, 8)
]

MODDING_STUB = """
local definitions = ...
MODS, INSTALLED, DISABLED, ENABLED = {}, {}, {}, {}
UPDATES = 0
for _, definition in ipairs(definitions) do
    local id = string.lower(definition.Id)
    local info = {Id = definition.Id, Handle = id, Enabled = definition.Enabled,
                  Dependencies = definition.Dependencies}
    MODS[id] = info
    INSTALLED[#INSTALLED + 1] = info
end
UI = {IsInFrontEnd = function() return true end}
GameConfiguration = {UpdateEnabledMods = function() UPDATES = UPDATES + 1 end}
Modding = {
    GetInstalledMods = function() return INSTALLED end,
    GetModHandle = function(id) local key = string.lower(id); return MODS[key] and key end,
    GetModInfo = function(handle) return assert(MODS[handle]) end,
    GetModAssociations = function(handle)
        local dependencies = {}
        for _, id in ipairs(MODS[handle].Dependencies) do
            dependencies[#dependencies + 1] = {Id = id}
        end
        return dependencies, {}, {}
    end,
    CanDisableMod = function(handle)
        local sources = {}
        for _, info in ipairs(INSTALLED) do
            if info.Enabled then
                for _, dependency in ipairs(info.Dependencies) do
                    if string.lower(dependency) == handle then sources[#sources + 1] = {Id = info.Id} end
                end
            end
        end
        if #sources > 0 then return 'MissingDependencies', {}, sources end
        return 'OK'
    end,
    CanEnableMod = function() return 'OK' end,
}
Modding.DisableMod = function(handle)
    -- Civ6 refuses to disable a prerequisite of an enabled mod.
    if Modding.CanDisableMod(handle) ~= 'OK' or REFUSE_DISABLE == handle then return end
    MODS[handle].Enabled = false
    DISABLED[#DISABLED + 1] = handle
end
Modding.EnableMod = function(handle, dependencies)
    if dependencies then
        for _, id in ipairs(MODS[handle].Dependencies) do
            local dependency = Modding.GetModHandle(id)
            if not MODS[dependency].Enabled then Modding.EnableMod(dependency, true) end
        end
    end
    MODS[handle].Enabled = true
    ENABLED[#ENABLED + 1] = handle
end
"""


class EnableModsTests(unittest.TestCase):
    def graph(self, edges, disabled=(), uppercase=False):
        lua = LuaRuntime(unpack_returned_tuples=True)
        definitions = [{"Id": mod_id.upper() if uppercase else mod_id,
                        "Enabled": mod_id not in disabled,
                        "Dependencies": [d.upper() if uppercase else d for d in dependencies]}
                       for mod_id, dependencies in edges.items()]
        lua.execute(MODDING_STUB, lua.table_from(definitions, recursive=True))
        return lua

    def test_transitive_dependents_are_disabled_first_once_and_reported(self):
        lua = self.graph({BASE: [], MIDDLE: [BASE], LEAF: [MIDDLE, SIBLING],
                          SIBLING: [BASE], KEEP: [], OFF: [BASE]}, disabled=[KEEP, OFF], uppercase=True)
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        step = {"action": "enable_mods", "ids": [KEEP], "disable_ids": [BASE, SIBLING, MISSING]}
        with patch.object(runner, "_lua", side_effect=lambda state, code, timeout: lua.execute(code)) as execute:
            detail = runner._run_step(step)
        self.assertEqual(list(lua.globals().DISABLED.values()), [LEAF, MIDDLE, SIBLING, BASE])
        self.assertEqual(detail, f"enabled/verified {KEEP}; disabled {LEAF}, {MIDDLE}, {SIBLING}, {BASE}")
        self.assertTrue(lua.globals().MODS[KEEP].Enabled)
        self.assertFalse(lua.globals().MODS[OFF].Enabled)
        self.assertEqual(lua.globals().UPDATES, 1)
        self.assertEqual(execute.call_args.args[0], "FrontEnd")

    def test_traversal_includes_enabled_descendants_of_a_disabled_intermediate(self):
        lua = self.graph({BASE: [], MIDDLE: [BASE], LEAF: [MIDDLE], KEEP: [], OFF: []}, disabled=[MIDDLE, OFF])
        lua.execute(_enable_mods_code([KEEP], [BASE]))
        self.assertEqual(list(lua.globals().DISABLED.values()), [LEAF, BASE])
        self.assertTrue(lua.globals().MODS[KEEP].Enabled)
        self.assertFalse(lua.globals().MODS[OFF].Enabled)

    def test_requested_dependents_are_preserved_and_conflicts_fail_before_mutations(self):
        for disabled in ([], [LEAF]):
            with self.subTest(disabled=disabled):
                lua = self.graph({BASE: [], MIDDLE: [BASE], LEAF: [MIDDLE], OFF: []}, disabled=disabled)
                with self.assertRaisesRegex(LuaError, "scenario enables a dependent mod"):
                    lua.execute(_enable_mods_code([LEAF.upper()], [OFF, BASE]))
                self.assertEqual(list(lua.globals().DISABLED.values()), [])
                self.assertEqual(list(lua.globals().ENABLED.values()), [])
                self.assertTrue(lua.globals().MODS[BASE].Enabled)
                self.assertEqual(lua.globals().MODS[LEAF].Enabled, LEAF not in disabled)

    def test_dependency_cycle_fails_before_mutations(self):
        lua = self.graph({BASE: [MIDDLE], MIDDLE: [BASE], KEEP: []})
        with self.assertRaisesRegex(LuaError, "mod dependency cycle"):
            lua.execute(_enable_mods_code([KEEP], [BASE]))
        self.assertEqual(list(lua.globals().DISABLED.values()), [])

    def test_disable_verifies_the_engine_state_and_names_the_failed_mod(self):
        lua = self.graph({BASE: [], KEEP: []})
        lua.globals().REFUSE_DISABLE = BASE
        with self.assertRaisesRegex(LuaError, "mod stayed enabled: " + BASE):
            lua.execute(_enable_mods_code([KEEP], [BASE]))
        self.assertTrue(lua.globals().MODS[BASE].Enabled)
        self.assertEqual(lua.globals().UPDATES, 0)

    def test_no_disable_targets_leaves_unrelated_mods_enabled(self):
        lua = self.graph({BASE: [], KEEP: [BASE]}, disabled=[KEEP])
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        with patch.object(runner, "_lua", side_effect=lambda state, code, timeout: lua.execute(code)):
            detail = runner._run_step({"action": "enable_mods", "ids": [KEEP]})
        self.assertEqual(detail, f"enabled/verified {KEEP}; disabled none")
        self.assertEqual(list(lua.globals().DISABLED.values()), [])
        self.assertTrue(lua.globals().MODS[BASE].Enabled)

    def test_enable_and_disable_overlap_is_case_insensitive(self):
        with self.assertRaisesRegex(ScenarioError, "invalid disable_ids"):
            validate_scenario({"name": "overlap", "steps": [
                {"action": "enable_mods", "ids": [BASE.upper()], "disable_ids": [BASE]}]})


if __name__ == "__main__":
    unittest.main()
