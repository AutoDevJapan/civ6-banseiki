"""Run a JSON Civ VI PoC scenario against an already-open single-player game."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import time

from capture import CaptureError, capture_civ_window
from judge import default_log_dir, judge, print_table, read_since, snapshot_logs
from tuner import TunerClient, TunerError


ACTIONS = {"enable_mods", "configure", "start_game", "autoplay", "ui_lua",
           "screenshot", "save", "exit_to_menu", "load", "wait_for", "manual"}
SAFE_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,79}$")
MOD_ID = re.compile(r"^[0-9a-fA-F]{8}-(?:[0-9a-fA-F]{4}-){3}[0-9a-fA-F]{12}$")
TYPE_ID = re.compile(r"^[A-Z][A-Z0-9_]+$")


class ScenarioError(RuntimeError):
    pass


def _lua_string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def _safe_relative_path(value: str) -> Path:
    if not isinstance(value, str):
        raise ScenarioError("Screenshot path must be a string")
    path = Path(value)
    if path.is_absolute() or not path.parts or any(part in ("..", "") for part in path.parts):
        raise ScenarioError(f"Path must be relative to output directory: {value!r}")
    return path


def validate_scenario(data: dict) -> None:
    if not isinstance(data, dict) or not isinstance(data.get("name"), str):
        raise ScenarioError("Scenario needs a name")
    if data.get("judge_profile", "generic") not in ("bundle0", "generic"):
        raise ScenarioError("judge_profile must be bundle0 or generic")
    steps = data.get("steps")
    if not isinstance(steps, list) or not steps:
        raise ScenarioError("Scenario needs a nonempty steps array")
    for number, step in enumerate(steps, 1):
        if not isinstance(step, dict) or step.get("action") not in ACTIONS:
            raise ScenarioError(f"Step {number}: unknown action")
        action = step["action"]
        if action == "enable_mods":
            if not isinstance(step.get("ids"), list) or not step["ids"] or any(
                not isinstance(item, str) or not MOD_ID.fullmatch(item) for item in step["ids"]
            ):
                raise ScenarioError(f"Step {number}: ids must be mod UUIDs")
        elif action == "configure":
            for key in ("ruleset", "map_size", "speed"):
                if not isinstance(step.get(key), str) or not TYPE_ID.fullmatch(step[key]):
                    raise ScenarioError(f"Step {number}: invalid {key}")
            if type(step.get("seed")) is not int or not -2**31 <= step["seed"] < 2**31:
                raise ScenarioError(f"Step {number}: seed must be a signed 32-bit integer")
            players = step.get("players")
            if not isinstance(players, list) or len(players) < 2 or sum(p.get("human") is True for p in players if isinstance(p, dict)) != 1:
                raise ScenarioError(f"Step {number}: specify at least two players and exactly one human")
            for player in players:
                if (not isinstance(player, dict) or type(player.get("human")) is not bool
                        or any(not isinstance(player.get(key), str) or not TYPE_ID.fullmatch(player[key])
                               for key in ("leader", "civ"))):
                    raise ScenarioError(f"Step {number}: invalid player")
        elif action == "autoplay":
            if type(step.get("turns")) is not int or step["turns"] < 1:
                raise ScenarioError(f"Step {number}: turns must be positive")
        elif action == "ui_lua":
            if not isinstance(step.get("state_name"), str) or not isinstance(step.get("code"), str) or not step["code"].strip():
                raise ScenarioError(f"Step {number}: ui_lua needs state_name and code")
        elif action == "screenshot":
            _safe_relative_path(step.get("path", ""))
        elif action in ("save", "load"):
            if not isinstance(step.get("name"), str) or not SAFE_NAME.fullmatch(step["name"]):
                raise ScenarioError(f"Step {number}: invalid save name")
        elif action == "wait_for":
            condition = step.get("condition")
            if not isinstance(condition, dict) or condition.get("type") not in ("lua", "log_line", "state"):
                raise ScenarioError(f"Step {number}: wait_for needs a lua, log_line or state condition")
            if condition["type"] == "lua" and (not isinstance(condition.get("state_name"), str)
                                                   or not isinstance(condition.get("code"), str)
                                                   or not condition["code"].strip()):
                raise ScenarioError(f"Step {number}: Lua wait needs state_name and code")
            if condition["type"] == "log_line" and (condition.get("log") not in ("Lua.log", "Database.log", "Modding.log")
                                                        or not isinstance(condition.get("contains"), str)
                                                        or not condition["contains"]):
                raise ScenarioError(f"Step {number}: log wait needs log and contains")
            if condition["type"] == "state" and not isinstance(condition.get("name"), str):
                raise ScenarioError(f"Step {number}: state wait needs name")
        elif action == "manual":
            if not isinstance(step.get("prompt"), str) or not step["prompt"].strip():
                raise ScenarioError(f"Step {number}: manual step needs prompt")
        if "timeout" in step and (not isinstance(step["timeout"], (int, float)) or step["timeout"] <= 0):
            raise ScenarioError(f"Step {number}: timeout must be positive")


def _enable_mods_code(ids: list[str]) -> str:
    lines = ["if not UI.IsInFrontEnd() then error('main menu required') end"]
    for mod_id in ids:
        literal = _lua_string(mod_id)
        lines += [
            f"local handle = Modding.GetModHandle({literal})",
            f"if not handle then error('mod is not installed: ' .. {literal}) end",
            "local info = Modding.GetModInfo(handle)",
            "if not info.Enabled then",
            "  local reason = Modding.CanEnableMod(handle, true)",
            "  if reason ~= 'OK' then error('cannot enable mod: ' .. tostring(reason)) end",
            "  Modding.EnableMod(handle, true)",
            "end",
            "if not Modding.GetModInfo(handle).Enabled then error('mod did not become enabled') end",
        ]
    lines += ["GameConfiguration.UpdateEnabledMods()", "return true"]
    return "\n".join(lines)


def _configure_code(step: dict) -> str:
    players = step["players"]
    lines = [
        "if not UI.IsInFrontEnd() then error('main menu required') end",
        "GameConfiguration.SetToDefaults()",
        f"GameConfiguration.SetRuleSet({_lua_string(step['ruleset'])})",
        "GameConfiguration.UpdateEnabledMods()",
        f"MapConfiguration.SetMapSize({_lua_string(step['map_size'])})",
        f"GameConfiguration.SetGameSpeedType({_lua_string(step['speed'])})",
        f"MapConfiguration.SetValue('RANDOM_SEED', {step['seed']})",
        f"GameConfiguration.SetValue('GAME_SYNC_RANDOM_SEED', {step['seed']})",
        f"MapConfiguration.SetMaxMajorPlayers({len(players)})",
        f"GameConfiguration.SetParticipatingPlayerCount({len(players)} + GameConfiguration.GetHiddenPlayerCount())",
    ]
    for index, player in enumerate(players):
        leader, civ = _lua_string(player["leader"]), _lua_string(player["civ"])
        slot = "SS_TAKEN" if player["human"] else "SS_COMPUTER"
        lines += [
            f"local p = PlayerConfigurations[{index}]",
            f"if not p then error('player slot {index} unavailable') end",
            f"p:SetSlotStatus(SlotStatus.{slot})",
            f"p:SetLeaderTypeName({leader})",
            f"p:SetCivilizationTypeName({civ})",
            f"if p:GetLeaderTypeName() ~= {leader} then error('leader mismatch in slot {index}') end",
            f"if p:GetCivilizationTypeName() ~= {civ} then error('civilization mismatch in slot {index}') end",
        ]
    lines.append("return true")
    return "\n".join(lines)


class Runner:
    def __init__(self, scenario: dict, output: Path, logs: Path, saves: Path, port: int):
        self.scenario = scenario
        self.output = output
        self.logs = logs
        self.saves = saves
        self.port = port
        self.client: TunerClient | None = None
        self.baseline: dict | None = None

    def _log(self, action: str, status: str, detail: str) -> None:
        record = {"at": datetime.now(timezone.utc).isoformat(), "action": action,
                  "status": status, "detail": detail}
        print(f"[{status}] {action}: {detail}", flush=True)
        with (self.output / "steps.jsonl").open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(record, ensure_ascii=False) + "\n")

    def _connect(self) -> TunerClient:
        if self.client is None:
            self.client = TunerClient(port=self.port).__enter__()
        return self.client

    def _states(self) -> list[str]:
        try:
            return self._connect().list_states()
        except TunerError:
            if self.client:
                self.client.close()
                self.client = None
            return self._connect().list_states()

    def _lua(self, state: str, code: str, timeout: float = 5.0) -> str:
        self._states()  # Reconnect safely after a game/menu transition, before sending the command.
        result = self._connect().run_lua(state, code, timeout)
        return result.value

    def _wait(self, condition: dict, timeout: float) -> None:
        end = time.monotonic() + timeout
        last = "not ready"
        while time.monotonic() < end:
            try:
                kind = condition["type"]
                if kind == "state":
                    present = condition["name"].casefold() in [s.casefold() for s in self._states()]
                elif kind == "lua":
                    value = self._lua(condition["state_name"], condition["code"])
                    present = value == str(condition.get("equals", "true")).lower()
                    last = f"Lua returned {value!r}"
                else:
                    assert self.baseline is not None
                    content = read_since(self.logs, self.baseline)[condition["log"]]
                    present = condition["contains"] in content
                if present:
                    return
            except (TunerError, FileNotFoundError) as exc:
                last = str(exc)
            time.sleep(min(0.5, max(0.0, end - time.monotonic())))
        raise ScenarioError(f"Timed out after {timeout}s waiting for {condition}; last: {last}")

    def _save_path(self, name: str) -> Path:
        return self.saves / (name + ".Civ6Save")

    def _run_step(self, step: dict) -> str:
        action = step["action"]
        timeout = float(step.get("timeout", 120))
        if action == "enable_mods":
            self._lua("FrontEnd", _enable_mods_code(step["ids"]))
            return "enabled/verified " + ", ".join(step["ids"])
        if action == "configure":
            self._lua("FrontEnd", _configure_code(step))
            return f"{step['ruleset']}, {step['map_size']}, {step['speed']}, seed={step['seed']}, players={len(step['players'])}"
        if action == "start_game":
            self._lua("FrontEnd", "if not UI.IsInFrontEnd() then error('main menu required') end\nNetwork.HostGame(ServerType.SERVER_TYPE_NONE)\nreturn true")
            self._wait({"type": "lua", "state_name": "InGame", "code": "return Game ~= nil and Game.GetLocalPlayer() >= 0"}, timeout)
            return "single-player game reached InGame"
        if action == "autoplay":
            code = (
                "if JAC_AUTOTEST_AUTO_HANDLER then LuaEvents.AutoPlayEnd.Remove(JAC_AUTOTEST_AUTO_HANDLER) end\n"
                "JAC_AUTOTEST_AUTO_DONE = false\n"
                "JAC_AUTOTEST_AUTO_HANDLER = function() JAC_AUTOTEST_AUTO_DONE = true end\n"
                "LuaEvents.AutoPlayEnd.Add(JAC_AUTOTEST_AUTO_HANDLER)\n"
                f"AutoplayManager.SetTurns({step['turns']})\n"
                "AutoplayManager.SetReturnAsPlayer(0)\n"
                "AutoplayManager.SetObserveAsPlayer(0)\n"
                "AutoplayManager.SetActive(true)\nreturn true"
            )
            self._lua("InGame", code)
            self._wait({"type": "lua", "state_name": "InGame", "code": "return JAC_AUTOTEST_AUTO_DONE == true"}, timeout)
            self._lua("InGame", "LuaEvents.AutoPlayEnd.Remove(JAC_AUTOTEST_AUTO_HANDLER)\nJAC_AUTOTEST_AUTO_HANDLER = nil\nAutoplayManager.SetActive(false)\nreturn true")
            return f"autoplay completed {step['turns']} turns"
        if action == "ui_lua":
            value = self._lua(step["state_name"], step["code"], timeout)
            return f"ran Lua in {step['state_name']}; return={value}"
        if action == "screenshot":
            target = self.output / _safe_relative_path(step["path"])
            capture_civ_window(target)
            return str(target)
        if action == "save":
            name = step["name"]
            target = self._save_path(name)
            if target.exists():
                raise ScenarioError(f"Save already exists (will not overwrite): {target}")
            literal = _lua_string(name)
            code = (
                "JAC_AUTOTEST_SAVE_DONE = false\n"
                "if JAC_AUTOTEST_SAVE_HANDLER then Events.SaveComplete.Remove(JAC_AUTOTEST_SAVE_HANDLER) end\n"
                "JAC_AUTOTEST_SAVE_HANDLER = function() JAC_AUTOTEST_SAVE_DONE = true end\n"
                "Events.SaveComplete.Add(JAC_AUTOTEST_SAVE_HANDLER)\n"
                f"local result = Network.SaveGame({{Name={literal}, Location=SaveLocations.LOCAL_STORAGE, "
                "Type=SaveTypes.SINGLE_PLAYER, IsAutosave=false, IsQuicksave=false}})\n"
                "if result == false then error('Network.SaveGame returned false') end\nreturn true"
            )
            self._lua("InGame", code)
            self._wait({"type": "lua", "state_name": "InGame", "code": "return JAC_AUTOTEST_SAVE_DONE == true"}, timeout)
            self._lua("InGame", "Events.SaveComplete.Remove(JAC_AUTOTEST_SAVE_HANDLER)\nJAC_AUTOTEST_SAVE_HANDLER = nil\nreturn true")
            until = time.monotonic() + min(timeout, 5)
            while not target.is_file() and time.monotonic() < until:
                time.sleep(0.2)
            if not target.is_file():
                raise ScenarioError(f"SaveComplete fired but save not found at {target}; check --saves")
            return str(target)
        if action == "exit_to_menu":
            self._lua("InGame", "Events.ExitToMainMenu()\nreturn true")
            self._wait({"type": "lua", "state_name": "FrontEnd", "code": "return UI.IsInFrontEnd()"}, timeout)
            return "main menu reached"
        if action == "load":
            name = step["name"]
            if not self._save_path(name).is_file():
                raise ScenarioError(f"Save not found: {self._save_path(name)}")
            literal = _lua_string(name)
            code = (
                "if not UI.IsInFrontEnd() then error('main menu required') end\n"
                f"local ok = Network.LoadGame({{Name={literal}, Location=SaveLocations.LOCAL_STORAGE, "
                "Type=SaveTypes.SINGLE_PLAYER, Directory=SaveDirectories.DEFAULT, "
                "IsAutosave=false, IsQuicksave=false}}, ServerType.SERVER_TYPE_NONE)\n"
                "if ok == false then error('Network.LoadGame returned false') end\nreturn true"
            )
            self._lua("FrontEnd", code)
            self._wait({"type": "lua", "state_name": "InGame", "code": "return Game ~= nil and Game.GetLocalPlayer() >= 0"}, timeout)
            return f"loaded {name}"
        if action == "wait_for":
            self._wait(step["condition"], timeout)
            return f"condition met: {step['condition']}"
        if action == "manual":
            print("MANUAL / Computer Use: " + step["prompt"], flush=True)
            input("Press Enter after the action is complete: ")
            return step["prompt"]
        raise AssertionError(action)

    def run(self) -> int:
        self.output.mkdir(parents=True, exist_ok=False)
        self.baseline = snapshot_logs(self.logs)
        (self.output / "baseline.json").write_text(json.dumps(self.baseline, indent=2), encoding="utf-8")
        self._log("start", "OK", self.scenario["name"])
        try:
            self._wait({"type": "lua", "state_name": "FrontEnd", "code": "return UI.IsInFrontEnd()"}, 10)
            for index, step in enumerate(self.scenario["steps"], 1):
                action = f"{index}/{len(self.scenario['steps'])} {step['action']}"
                self._log(action, "START", json.dumps(step, ensure_ascii=False))
                try:
                    detail = self._run_step(step)
                except (ScenarioError, TunerError, CaptureError, OSError, EOFError) as exc:
                    self._log(action, "FAIL", str(exc))
                    return 1
                self._log(action, "OK", detail)
            assert self.baseline is not None
            checks = judge(read_since(self.logs, self.baseline), self.scenario.get("judge_profile", "generic"))
            print_table(checks)
            result = all(check.passed for check in checks)
            self._log("judge", "OK" if result else "FAIL", f"{sum(c.passed for c in checks)}/{len(checks)} checks passed")
            return 0 if result else 1
        except (ScenarioError, TunerError, OSError, ValueError) as exc:
            self._log("run", "FAIL", str(exc))
            return 1
        finally:
            if self.client:
                self.client.close()


def main() -> int:
    parser = argparse.ArgumentParser(description="Run a Civ VI PoC scenario; never starts Civ VI itself")
    parser.add_argument("scenario", type=Path)
    parser.add_argument("--check", action="store_true", help="Validate JSON only; do not connect to the game")
    parser.add_argument("--output", type=Path, help="New directory for baseline, trace and screenshots")
    parser.add_argument("--logs", type=Path)
    parser.add_argument("--saves", type=Path)
    parser.add_argument("--port", type=int, default=4318)
    args = parser.parse_args()
    try:
        data = json.loads(args.scenario.read_text(encoding="utf-8"))
        validate_scenario(data)
        if args.check:
            print(f"VALID: {data['name']} ({len(data['steps'])} steps)")
            return 0
        output = args.output or (Path(__file__).resolve().parent / "artifacts" /
                                 (datetime.now().strftime("%Y%m%d-%H%M%S") + "-" + args.scenario.stem))
        saves = args.saves or (Path.home() / "Documents" / "My Games" / "Sid Meier's Civilization VI" / "Saves" / "Single")
        return Runner(data, output, args.logs or default_log_dir(), saves, args.port).run()
    except (OSError, json.JSONDecodeError, ScenarioError, ValueError) as exc:
        parser.exit(2, f"Scenario error: {exc}\n")


if __name__ == "__main__":
    raise SystemExit(main())
