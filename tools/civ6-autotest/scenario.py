"""Run a JSON Civ VI PoC scenario, restarting Civ VI when its mods changed."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
import json
import math
import os
from pathlib import Path, PureWindowsPath
import re
import time

from capture import CaptureError, capture_civ_window
from click import (InputError, VK_ESCAPE, click_fraction, press_key, request_civ_window_close,
                   require_single_civ_process)
from judge import (default_log_dir, judge, log_marker_replaced, print_table, read_since,
                   slice_log_bytes, snapshot_logs)
from metrics import TurnMetrics, process_memory
from restart import (DEFAULT_MODS_DIR, enabled_mod_folders, launch_via_steam, newest_mod_file,
                     restart_reason, single_civ_process, terminate_civ_process, wait_for_civ_window,
                     wait_for_exit)
from tuner import LuaExecutionError, LuaResponseUnknown, TunerClient, TunerError, validate_lua_code


ACTIONS = {"restart_game", "enable_mods", "configure", "start_game", "autoplay", "end_turn", "ui_lua",
           "screenshot", "save", "exit_to_menu", "load", "wait_for", "pause", "manual"}
SAFE_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,79}$")
MOD_ID = re.compile(r"^[0-9a-fA-F]{8}-(?:[0-9a-fA-F]{4}-){3}[0-9a-fA-F]{12}$")
TYPE_ID = re.compile(r"^[A-Z][A-Z0-9_]+$")
MAP_FILE = re.compile(r"^\{[0-9a-fA-F-]{36}\}Maps/[A-Za-z0-9_-]+\.Civ6Map$")


class ScenarioError(RuntimeError):
    pass


def _remaining(deadline: float) -> float:
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        raise ScenarioError("Step deadline expired")
    return remaining


@dataclass(frozen=True)
class AutoplayStatus:
    turn: int
    active: bool
    event: bool
    remaining: int
    local_player: int


def _parse_autoplay_status(value: str) -> AutoplayStatus:
    parts = value.split("|")
    if len(parts) != 5 or parts[1] not in ("true", "false") or parts[2] not in ("true", "false"):
        raise ScenarioError(f"Unexpected autoplay status: {value!r}")
    try:
        return AutoplayStatus(int(parts[0]), parts[1] == "true", parts[2] == "true",
                              int(parts[3]), int(parts[4]))
    except ValueError as exc:
        raise ScenarioError(f"Unexpected autoplay status: {value!r}") from exc


def _lua_string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def _safe_relative_path(value: str) -> Path:
    if not isinstance(value, str):
        raise ScenarioError("Screenshot path must be a string")
    path = Path(value)
    windows_path = PureWindowsPath(value)
    if (path.is_absolute() or windows_path.drive or windows_path.root or not path.parts
            or ".." in path.parts or ".." in windows_path.parts):
        raise ScenarioError(f"Path must be relative to output directory: {value!r}")
    return path


def validate_scenario(data: dict) -> None:
    if not isinstance(data, dict) or not isinstance(data.get("name"), str):
        raise ScenarioError("Scenario needs a name")
    if data.get("judge_profile", "generic") not in ("bundle0", "generic", "growth", "japan_map"):
        raise ScenarioError("judge_profile must be bundle0, generic, growth or japan_map")
    if data.get("judge_profile") == "growth":
        if type(data.get("expected_units")) is not int or data["expected_units"] < 1:
            raise ScenarioError("growth scenario needs a positive expected_units count")
    steps = data.get("steps")
    if not isinstance(steps, list) or not steps:
        raise ScenarioError("Scenario needs a nonempty steps array")
    for number, step in enumerate(steps, 1):
        if not isinstance(step, dict) or step.get("action") not in ACTIONS:
            raise ScenarioError(f"Step {number}: unknown action")
        action = step["action"]
        if action == "restart_game":
            if set(step) - {"action", "timeout"}:
                raise ScenarioError(f"Step {number}: restart_game only accepts timeout")
        elif action == "enable_mods":
            if not isinstance(step.get("ids"), list) or not step["ids"] or any(
                not isinstance(item, str) or not MOD_ID.fullmatch(item) for item in step["ids"]
            ):
                raise ScenarioError(f"Step {number}: ids must be mod UUIDs")
            disabled = step.get("disable_ids", [])
            if (not isinstance(disabled, list) or any(not isinstance(item, str) or not MOD_ID.fullmatch(item)
                                                    for item in disabled) or set(disabled) & set(step["ids"])):
                raise ScenarioError(f"Step {number}: invalid disable_ids")
        elif action == "configure":
            for key in ("ruleset", "map_size", "speed"):
                if not isinstance(step.get(key), str) or not TYPE_ID.fullmatch(step[key]):
                    raise ScenarioError(f"Step {number}: invalid {key}")
            if type(step.get("seed")) is not int or not -2**31 <= step["seed"] < 2**31:
                raise ScenarioError(f"Step {number}: seed must be a signed 32-bit integer")
            if "map_file" in step and (not isinstance(step["map_file"], str)
                                       or not MAP_FILE.fullmatch(step["map_file"])):
                raise ScenarioError(f"Step {number}: invalid map_file")
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
            if "metrics" in step and type(step["metrics"]) is not bool:
                raise ScenarioError(f"Step {number}: metrics must be boolean")
        elif action == "ui_lua":
            if not isinstance(step.get("state_name"), str) or not isinstance(step.get("code"), str) or not step["code"].strip():
                raise ScenarioError(f"Step {number}: ui_lua needs state_name and code")
            try:
                validate_lua_code(step["code"])
            except ValueError as exc:
                raise ScenarioError(f"Step {number}: {exc}") from exc
        elif action == "screenshot":
            _safe_relative_path(step.get("path", ""))
            crop = step.get("crop")
            if crop is not None and (not isinstance(crop, list) or len(crop) != 4
                                     or any(type(v) not in (int, float) or not 0 <= v <= 1 for v in crop)
                                     or not crop[0] < crop[2] or not crop[1] < crop[3]):
                raise ScenarioError(f"Step {number}: crop must be [left, top, right, bottom] fractions")
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
            if condition["type"] == "lua":
                try:
                    validate_lua_code(condition["code"])
                except ValueError as exc:
                    raise ScenarioError(f"Step {number}: {exc}") from exc
            if condition["type"] == "log_line" and (condition.get("log") not in ("Lua.log", "Database.log", "Modding.log")
                                                        or not isinstance(condition.get("contains"), str)
                                                        or not condition["contains"]):
                raise ScenarioError(f"Step {number}: log wait needs log and contains")
            if condition["type"] == "state" and not isinstance(condition.get("name"), str):
                raise ScenarioError(f"Step {number}: state wait needs name")
        elif action == "pause":
            seconds = step.get("seconds")
            if type(seconds) not in (int, float) or not 0 < seconds <= 10 or not math.isfinite(seconds):
                raise ScenarioError(f"Step {number}: pause seconds must be in (0, 10]")
        elif action == "manual":
            if not isinstance(step.get("prompt"), str) or not step["prompt"].strip():
                raise ScenarioError(f"Step {number}: manual step needs prompt")
            if "timeout" in step:
                raise ScenarioError(f"Step {number}: manual waits for Enter and does not support timeout")
        if "timeout" in step:
            timeout = step["timeout"]
            if type(timeout) not in (int, float) or timeout <= 0:
                raise ScenarioError(f"Step {number}: timeout must be a finite positive number")
            try:
                finite = math.isfinite(float(timeout))
            except OverflowError:
                finite = False
            if not finite:
                raise ScenarioError(f"Step {number}: timeout must be a finite positive number")


def _enable_mods_code(ids: list[str], disable_ids: list[str] | None = None) -> str:
    lines = ["if not UI.IsInFrontEnd() then error('main menu required') end"]
    for mod_id in disable_ids or []:
        literal = _lua_string(mod_id)
        lines += [
            f"local disabledHandle = Modding.GetModHandle({literal})",
            "if disabledHandle and Modding.GetModInfo(disabledHandle).Enabled then Modding.DisableMod(disabledHandle) end",
            "if disabledHandle and Modding.GetModInfo(disabledHandle).Enabled then error('mod stayed enabled') end",
        ]
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
        *([f"MapConfiguration.SetValue('MAP_SCRIPT', {_lua_string(step['map_file'])})"]
          if "map_file" in step else []),
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
    def __init__(self, scenario: dict, output: Path, logs: Path, saves: Path, port: int,
                 *, auto_restart: bool = True, mods_dir: Path = DEFAULT_MODS_DIR, owner_start: bool = False):
        self.scenario = scenario
        self.owner_start = owner_start
        self.output = output
        self.logs = logs
        self.saves = saves
        self.port = port
        self.auto_restart = auto_restart
        self.mods_dir = mods_dir
        self.client: TunerClient | None = None
        self.baseline: dict | None = None
        self.log_step_baseline: dict | None = None
        self.log_matches_consumed: dict[tuple[str, str], int] = {}
        self.metrics_runs = 0

    def _log(self, action: str, status: str, detail: str) -> None:
        record = {"at": datetime.now(timezone.utc).isoformat(), "action": action,
                  "status": status, "detail": detail}
        print(f"[{status}] {action}: {detail}", flush=True)
        with (self.output / "steps.jsonl").open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(record, ensure_ascii=False) + "\n")

    def _connect(self, timeout: float = 5.0) -> TunerClient:
        if self.client is None:
            try:
                self.client = TunerClient(port=self.port, timeout=timeout).__enter__()
            except (TunerError, OSError) as exc:
                raise TunerError(f"FireTuner に接続できません（Civ6 が起動していない可能性）: {exc}") from exc
        return self.client

    def _disconnect(self) -> None:
        if self.client is not None:
            self.client.close()
            self.client = None

    def _states(self, timeout: float = 5.0) -> list[str]:
        require_single_civ_process()
        deadline = time.monotonic() + timeout

        def fetch() -> list[str]:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TunerError("FireTuner に接続できません（応答待ちがタイムアウト）")
            client = self._connect(remaining)
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TunerError("FireTuner に接続できません（応答待ちがタイムアウト）")
            return client.list_states(timeout=remaining)

        try:
            return fetch()
        except (TunerError, OSError):
            self._disconnect()
            return fetch()

    def _lua(self, state: str, code: str, timeout: float = 5.0) -> str:
        deadline = time.monotonic() + timeout
        self._states(_remaining(deadline))  # Reconnect before sending the command.
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TunerError("FireTuner の応答待ちがタイムアウトし、Lua コマンドは未送信です")
        result = self._connect(remaining).run_lua(state, code, remaining)
        return result.value

    def _log_since_same_generation(self, name: str, baseline: dict) -> str:
        path = self.logs / name
        mark = baseline["files"][name]
        try:
            with path.open("rb") as handle:
                content = handle.read()
                inode = os.fstat(handle.fileno()).st_ino
        except FileNotFoundError:
            if mark["inode"] is None:
                return ""
            raise ScenarioError(f"{name} disappeared during wait; outcome undecidable") from None
        if log_marker_replaced(content, mark, inode):
            raise ScenarioError(f"{name} rotated or truncated during wait; outcome undecidable")
        return slice_log_bytes(content, mark, inode)

    def _wait(self, condition: dict, timeout: float) -> None:
        end = time.monotonic() + timeout
        last = "not ready"
        log_baseline = None
        if condition["type"] == "log_line":
            # Include output from the action that triggered this wait, but not older steps.
            log_baseline = self.log_step_baseline or snapshot_logs(self.logs)
        while time.monotonic() < end:
            try:
                kind = condition["type"]
                if kind == "state":
                    remaining = min(5.0, end - time.monotonic())
                    if remaining <= 0:
                        break
                    present = condition["name"].casefold() in [s.casefold() for s in self._states(remaining)]
                elif kind == "lua":
                    remaining = min(5.0, end - time.monotonic())
                    if remaining <= 0:
                        break
                    value = self._lua(condition["state_name"], condition["code"], remaining)
                    present = value == str(condition.get("equals", "true")).lower()
                    last = f"Lua returned {value!r}"
                else:
                    assert log_baseline is not None
                    key = (condition["log"], condition["contains"])
                    content = self._log_since_same_generation(condition["log"], log_baseline)
                    occurrences = sum(condition["contains"] in line for line in content.splitlines())
                    present = occurrences > self.log_matches_consumed.get(key, 0)
                if present:
                    _remaining(end)
                    if kind == "log_line":
                        self.log_matches_consumed[key] = self.log_matches_consumed.get(key, 0) + 1
                    return
            except (LuaResponseUnknown, LuaExecutionError):
                # This predicate may have executed; never poll it again.
                raise
            except (TunerError, OSError) as exc:
                last = str(exc)
            time.sleep(min(0.5, max(0.0, end - time.monotonic())))
        raise ScenarioError(f"Timed out after {timeout}s waiting for {condition}; last: {last}")

    def _wait_frontend_ready(self, timeout: float, *, skip_opening: bool = False) -> None:
        """Retry only this read-only startup probe after a lost Tuner reply."""
        deadline = time.monotonic() + timeout
        next_click = time.monotonic() + 12
        last = "FrontEnd not ready"
        while time.monotonic() < deadline:
            if skip_opening and time.monotonic() >= next_click:
                click_fraction(0.5, 0.15)
                next_click = time.monotonic() + 12
            try:
                value = self._lua("FrontEnd", "return UI.IsInFrontEnd()",
                                  min(5, _remaining(deadline)))
                if value == "true":
                    return
                last = f"FrontEnd returned {value!r}"
            except LuaExecutionError:
                raise
            except (TunerError, OSError) as exc:
                last = str(exc)
                self._disconnect()
            time.sleep(min(0.5, max(0, deadline - time.monotonic())))
        raise ScenarioError(f"FrontEnd did not answer after {timeout}s; last: {last}")

    def _launch_steam_and_wait(self, deadline: float) -> None:
        """Steam can reject a launch while it is still releasing the prior game."""
        attempts = 0
        next_launch = 0.0
        last = "no Civ VI process yet"
        while time.monotonic() < deadline:
            now = time.monotonic()
            try:
                process_present = single_civ_process() is not None
            except InputError as exc:
                if not str(exc).startswith("Multiple Civ VI processes found"):
                    raise
                process_present = True  # Never launch another copy during an overlap.
            if not process_present and now >= next_launch:
                if attempts >= 4:
                    raise ScenarioError(f"Steam did not start Civ VI after {attempts} attempts; last: {last}")
                launch_via_steam()
                attempts += 1
                next_launch = now + 45
                print(f"Steam Civ VI launch attempt {attempts}", flush=True)
            try:
                wait_for_civ_window(min(deadline, time.monotonic() + 5))
                return
            except InputError as exc:
                last = str(exc)
        raise ScenarioError(f"Civ VI window did not appear after Steam launch; last: {last}")

    LOADSCREEN_READY = "LoadScreen: OnLoadGameViewStateDone"

    def _loadscreen_count(self) -> int:
        assert self.baseline is not None
        return self._log_since_same_generation("Lua.log", self.baseline).count(self.LOADSCREEN_READY)

    def _enter_game_after_loadscreen(self, before: int, timeout: float, *, deadline: float | None = None,
                                     response_unknown: bool = False) -> None:
        """After HostGame/LoadGame the loading screen waits for Begin Game (verified live).

        Wait for a new "load view ready" line in Lua.log, then press Esc once
        (LoadScreen.lua maps Esc to Begin Game). An InGame Tuner state can already
        exist behind the load screen, so its presence must not suppress that key.
        """
        end = deadline if deadline is not None else time.monotonic() + timeout
        begin_game_sent = False
        last = "loading screen not ready"
        while time.monotonic() < end:
            loadscreen_count = self._loadscreen_count()  # Check the log generation until the transition ends.
            if not begin_game_sent:
                if loadscreen_count <= before:
                    time.sleep(min(0.5, max(0, end - time.monotonic())))
                    continue
                _remaining(end)
                press_key(VK_ESCAPE, deadline=end)
                begin_game_sent = True
                last = "Esc sent on loading screen"
                # The FrontEnd connection can keep its old state list across the transition.
                self._disconnect()
            try:
                if "ingame" in (state.casefold() for state in self._states(min(5, _remaining(end)))):
                    if self._lua("InGame", "return Game ~= nil and Game.GetLocalPlayer() >= 0",
                                 min(5, _remaining(end))) == "true":
                        self._close_options_menu(end)
                        self._dismiss_advisor(end)
                        _remaining(end)
                        return
                    last = "InGame exists; waiting for the local player"
                else:
                    last = "InGame state not available yet"
                    self._disconnect()
            except (LuaResponseUnknown, LuaExecutionError):
                raise
            except (TunerError, OSError) as exc:
                last = str(exc)
                self._disconnect()
            time.sleep(min(2, max(0, end - time.monotonic())))
        uncertain = "; outcome undecidable after unknown Lua response" if response_unknown else ""
        raise ScenarioError(f"Timed out after {timeout}s entering the game; last: {last}{uncertain}")

    def _save_path(self, name: str) -> Path:
        return self.saves / (name + ".Civ6Save")

    def _close_options_menu(self, deadline: float | None = None) -> bool:
        def remaining() -> float:
            return min(5, _remaining(deadline)) if deadline is not None else 5

        if "ingametopoptionsmenu" not in (state.casefold() for state in self._states(remaining())):
            return False
        closed = self._lua("InGameTopOptionsMenu", "if not ContextPtr:IsHidden() then OnReturn(); CloseImmediately(); return true end\nreturn false",
                           remaining()) == "true"
        if closed:
            self._wait({"type": "lua", "state_name": "InGameTopOptionsMenu",
                        "code": "return ContextPtr:IsHidden()"}, remaining())
            print("Closed in-game options menu", flush=True)
        return closed

    def _dismiss_advisor(self, deadline: float | None = None) -> bool:
        # AdvisorPopup.lua's OnHideAdvisorDialog releases its tutorial input block.
        def remaining() -> float:
            return min(5, _remaining(deadline)) if deadline is not None else 5

        if "advisorpopup" not in (state.casefold() for state in self._states(remaining())):
            return False
        dismissed = self._lua("AdvisorPopup", "if IsBlockingInput() then OnHideAdvisorDialog(); return true end\nreturn false",
                              remaining()) == "true"
        if dismissed:
            print("Dismissed blocking Advisor popup", flush=True)
        return dismissed

    def _autoplay_status(self, deadline: float) -> AutoplayStatus:
        value = self._lua("InGame", "return tostring(Game.GetCurrentGameTurn()) .. '|' .. "
                          "tostring(AutoplayManager.IsActive()) .. '|' .. "
                          "tostring(JAC_AUTOTEST_AUTO_DONE == true) .. '|' .. "
                          "tostring(AutoplayManager.GetTurns()) .. '|' .. tostring(Game.GetLocalPlayer())",
                          min(5, _remaining(deadline)))
        return _parse_autoplay_status(value)

    def _autoplay(self, turns: int, timeout: float, *, deadline: float | None = None,
                  record_metrics: bool = False) -> str:
        deadline = deadline if deadline is not None else time.monotonic() + timeout
        # An open partial screen and the base game's advisor can both hold input.
        self._lua("InGame", "if LuaEvents.JAC_Notebooks_Close then LuaEvents.JAC_Notebooks_Close() end\nreturn true",
                  min(5, _remaining(deadline)))
        self._close_options_menu(deadline)
        self._dismiss_advisor(deadline)
        start_code = (
            "if AutoplayManager.IsActive() then error('autoplay already active') end\n"
            "local startTurn = Game.GetCurrentGameTurn()\n"
            "local human = Game.GetLocalPlayer()\n"
            "if human < 0 then error('no local human player to return to') end\n"
            "if JAC_AUTOTEST_AUTO_HANDLER then LuaEvents.AutoPlayEnd.Remove(JAC_AUTOTEST_AUTO_HANDLER) end\n"
            "JAC_AUTOTEST_AUTO_DONE = false\n"
            "JAC_AUTOTEST_AUTO_HANDLER = function() JAC_AUTOTEST_AUTO_DONE = true end\n"
            "LuaEvents.AutoPlayEnd.Add(JAC_AUTOTEST_AUTO_HANDLER)\n"
            f"AutoplayManager.SetTurns({turns})\n"
            "AutoplayManager.SetReturnAsPlayer(human)\n"
            "AutoplayManager.SetObserveAsPlayer(PlayerTypes.NONE)\n"
            "AutoplayManager.SetActive(true)\n"
            "return tostring(startTurn) .. '|' .. tostring(human)"
        )
        try:
            start_value = self._lua("InGame", start_code, min(5, _remaining(deadline)))
            try:
                start_turn, human = (int(part) for part in start_value.split("|"))
            except ValueError as exc:
                raise ScenarioError(f"Unexpected autoplay start: {start_value!r}") from exc
            target = start_turn + turns
            recorder = None
            if record_metrics:
                self.metrics_runs += 1
                process = single_civ_process()
                if process is None:
                    raise ScenarioError("Civ VI process disappeared before metrics sampling")
                metrics_pid = process[0]
                recorder = TurnMetrics(self.output / f"autoplay-metrics-{self.metrics_runs}.jsonl", start_turn)
                recorder.start(process_memory(metrics_pid))
            restarts = 0
            inactive_since = None
            status = None
            while time.monotonic() < deadline:
                status = self._autoplay_status(deadline)
                if recorder and status.turn != recorder.turn:
                    process = single_civ_process()
                    if process is None or process[0] != metrics_pid:
                        raise ScenarioError("Civ VI process changed during metrics sampling")
                    recorder.sample(status.turn, process_memory(metrics_pid))
                _remaining(deadline)
                if status.turn > target:
                    raise ScenarioError(f"Autoplay overshot turn {target}: {status}")
                if (status.event and status.turn == target and not status.active
                        and status.local_player == human):
                    break
                self._close_options_menu(deadline)
                self._dismiss_advisor(deadline)
                if not status.active and status.turn < target:
                    if inactive_since is None:
                        inactive_since = time.monotonic()
                    elif time.monotonic() - inactive_since >= 5 and status.local_player == human:
                        if restarts >= 2:
                            raise ScenarioError(f"Autoplay stopped early at turn {status.turn}/{target}: {status}")
                        remaining = target - status.turn
                        self._lua("InGame", "JAC_AUTOTEST_AUTO_DONE = false\n"
                                  f"AutoplayManager.SetTurns({remaining})\n"
                                  f"AutoplayManager.SetReturnAsPlayer({human})\n"
                                  "AutoplayManager.SetObserveAsPlayer(PlayerTypes.NONE)\n"
                                  "AutoplayManager.SetActive(true)\nreturn true",
                                  min(5, _remaining(deadline)))
                        restarts += 1
                        inactive_since = None
                else:
                    inactive_since = None
                time.sleep(min(0.1 if recorder else 0.5, max(0, deadline - time.monotonic())))
            else:
                raise ScenarioError(f"Autoplay timed out at turn {status.turn if status else '?'} / {target}; last: {status}")
        except (LuaResponseUnknown, LuaExecutionError):
            # A chunk may have run, or Lua reported an ERR: stop without more Lua mutations.
            raise
        except (ScenarioError, TunerError, OSError):
            # A previously acknowledged start may still be active when later checks fail.
            try:
                self._lua("InGame", "if AutoplayManager.IsActive() then AutoplayManager.SetActive(false) end\n"
                          "if JAC_AUTOTEST_AUTO_HANDLER then LuaEvents.AutoPlayEnd.Remove(JAC_AUTOTEST_AUTO_HANDLER) end\n"
                          "JAC_AUTOTEST_AUTO_HANDLER = nil\nreturn true", 5)
            except (ScenarioError, TunerError, OSError) as cleanup_error:
                print(f"Autoplay cleanup failed: {cleanup_error}", flush=True)
            raise
        self._lua("InGame", "if JAC_AUTOTEST_AUTO_HANDLER then LuaEvents.AutoPlayEnd.Remove(JAC_AUTOTEST_AUTO_HANDLER) end\n"
                  "JAC_AUTOTEST_AUTO_HANDLER = nil\nreturn true", min(5, _remaining(deadline)))
        _remaining(deadline)
        detail = f"autoplay completed {turns} turns ({start_turn}->{status.turn}; event={status.event})"
        if recorder:
            detail += (f"; metrics=autoplay-metrics-{self.metrics_runs}.jsonl"
                       f" observed={recorder.observed_turns} missed={recorder.missed_turns}")
        return detail

    def _end_turn(self, timeout: float, *, deadline: float | None = None) -> str:
        deadline = deadline if deadline is not None else time.monotonic() + timeout
        self._lua("InGame", "if LuaEvents.JAC_Notebooks_Close then LuaEvents.JAC_Notebooks_Close() end\nreturn true",
                  min(5, _remaining(deadline)))
        self._close_options_menu(deadline)
        self._dismiss_advisor(deadline)
        start = int(self._lua("InGame", "return Game.GetCurrentGameTurn()", min(5, _remaining(deadline))))
        self._lua("InGame", "UI.RequestAction(ActionTypes.ACTION_ENDTURN)\nreturn true",
                  min(5, _remaining(deadline)))
        self._wait({"type": "lua", "state_name": "InGame",
                    "code": f"return Game.GetCurrentGameTurn() == {start + 1} and Game.GetLocalPlayer() >= 0"},
                   _remaining(deadline))
        self._dismiss_advisor(deadline)
        _remaining(deadline)
        return f"human turn {start}->{start + 1}"

    def _restart_game(self, timeout: float) -> str:
        deadline = time.monotonic() + timeout
        old = single_civ_process()
        if old is not None:
            pid, started = old
            graceful = False
            try:
                states = self._states(min(5, _remaining(deadline)))
                state = next((name for name in states if name.casefold() == "frontend"), None)
                if state is None:
                    state = next((name for name in states if name.casefold() == "ingame"), None)
                if state is not None:
                    try:
                        self._lua(state, "Events.UserConfirmedClose()\nreturn true",
                                  min(5, _remaining(deadline)))
                    except LuaResponseUnknown:
                        # The quit event may have run; observe the process, never resend it.
                        pass
                    graceful = True
            except (TunerError, InputError, OSError, ValueError) as exc:
                print(f"FireTuner quit unavailable: {exc}", flush=True)
            finally:
                self._disconnect()
            if not graceful and single_civ_process() == old:
                try:
                    request_civ_window_close()
                except InputError as exc:
                    print(f"Civ VI window quit unavailable: {exc}", flush=True)
            if not wait_for_exit(pid, min(deadline, time.monotonic() + 15)):
                terminate_civ_process(pid, started)
                if not wait_for_exit(pid, min(deadline, time.monotonic() + 10)):
                    raise ScenarioError(f"Civ VI process {pid} did not exit after termination")
        _remaining(deadline)
        if old is not None:
            # Steam may still hold the previous game session after the EXE disappears.
            time.sleep(min(8, _remaining(deadline)))
        if single_civ_process() is not None:
            raise ScenarioError("A Civ VI process appeared before Steam relaunch")
        self._launch_steam_and_wait(deadline)
        if self.owner_start:
            # The owner is using the PC: Windows refuses to move focus, so let the opening finish.
            self._disconnect()
            self._wait_frontend_ready(_remaining(deadline), skip_opening=False)
            return "Civ VI relaunched via Steam; opening played to the end; FrontEnd answered"
        # Let the opening movie start before sending its skip click. Keep clear of the menu buttons.
        time.sleep(min(8, _remaining(deadline)))
        click_fraction(0.5, 0.15)
        self._disconnect()
        self._wait_frontend_ready(_remaining(deadline), skip_opening=True)
        return "Civ VI relaunched via Steam; opening skipped with a left click; FrontEnd answered"

    def _run_step(self, step: dict) -> str:
        action = step["action"]
        if action == "manual":
            print("MANUAL / Computer Use (no timeout; waits until Enter): " + step["prompt"], flush=True)
            input("Press Enter after the action is complete: ")
            return step["prompt"]
        timeout = float(step.get("timeout", 360 if action == "restart_game" else 120))
        deadline = time.monotonic() + timeout
        if action == "restart_game":
            return self._restart_game(_remaining(deadline))
        if action == "enable_mods":
            self._lua("FrontEnd", _enable_mods_code(step["ids"], step.get("disable_ids")),
                      min(5, _remaining(deadline)))
            return "enabled/verified " + ", ".join(step["ids"])
        if action == "configure":
            self._lua("FrontEnd", _configure_code(step), min(5, _remaining(deadline)))
            return f"{step['ruleset']}, {step['map_size']}, {step['speed']}, seed={step['seed']}, players={len(step['players'])}"
        if action == "start_game" and self.owner_start:
            # The owner presses Start Game; the configured setup is shown, not reset (MainMenu's
            # OnAdvancedSetup would call SetToDefaults, so the popup is queued directly).
            before = self._loadscreen_count()
            self._lua("FrontEnd", "if not UI.IsInFrontEnd() then error('main menu required') end\n"
                      "UIManager:QueuePopup(ContextPtr:LookUpControl('/FrontEnd/MainMenu/AdvancedSetup'), PopupPriority.Current)\n"
                      "return true", min(5, _remaining(deadline)))
            print("OWNER ACTION: press Start Game on the Create Game screen", flush=True)
            wait = max(timeout, 3600.0)
            self._enter_game_after_loadscreen(before, wait, deadline=time.monotonic() + wait)
            return "owner pressed Start Game; single-player game reached InGame"
        if action == "start_game":
            before = self._loadscreen_count()
            response_unknown = False
            try:
                self._lua("FrontEnd", "if not UI.IsInFrontEnd() then error('main menu required') end\nNetwork.HostGame(ServerType.SERVER_TYPE_NONE)\nreturn true",
                          min(5, _remaining(deadline)))
            except LuaResponseUnknown as exc:
                print(f"HostGame response interrupted during transition: {exc}", flush=True)
                response_unknown = True
            self._enter_game_after_loadscreen(before, timeout, deadline=deadline,
                                              response_unknown=response_unknown)
            return "single-player game reached InGame"
        if action == "autoplay":
            return self._autoplay(step["turns"], timeout, deadline=deadline,
                                  record_metrics=step.get("metrics", False))
        if action == "end_turn":
            return self._end_turn(timeout, deadline=deadline)
        if action == "ui_lua":
            value = self._lua(step["state_name"], step["code"], min(5, _remaining(deadline)))
            return f"ran Lua in {step['state_name']}; return={value}"
        if action == "screenshot":
            require_single_civ_process()
            base = self.output.resolve()
            target = (base / _safe_relative_path(step["path"])).resolve()
            if not target.is_relative_to(base):
                raise ScenarioError(f"Screenshot path escapes output directory: {step['path']!r}")
            self._close_options_menu(deadline)
            self._dismiss_advisor(deadline)
            _remaining(deadline)
            capture_civ_window(target, step.get("crop"), deadline=deadline)
            _remaining(deadline)
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
                "Type=Network.GetGameConfigurationSaveType(), IsAutosave=false, IsQuicksave=false})\n"
                "if result == false then error('Network.SaveGame returned false') end\nreturn true"
            )
            self._lua("InGame", code, min(30, _remaining(deadline)))
            self._wait({"type": "lua", "state_name": "InGame", "code": "return JAC_AUTOTEST_SAVE_DONE == true"},
                       _remaining(deadline))
            self._lua("InGame", "Events.SaveComplete.Remove(JAC_AUTOTEST_SAVE_HANDLER)\nJAC_AUTOTEST_SAVE_HANDLER = nil\nreturn true",
                      min(5, _remaining(deadline)))
            until = min(deadline, time.monotonic() + 5)
            while not target.is_file() and time.monotonic() < until:
                time.sleep(0.2)
            if not target.is_file() or target.stat().st_size == 0:
                raise ScenarioError(f"SaveComplete fired but save not found at {target}; check --saves")
            _remaining(deadline)
            return str(target)
        if action == "exit_to_menu":
            self._lua("InGame", "Events.ExitToMainMenu()\nreturn true", min(5, _remaining(deadline)))
            # The FrontEnd Lua state can appear while its Tuner socket is still changing.
            # Wait for the state on a fresh connection before probing its Lua context.
            self._disconnect()
            self._wait({"type": "state", "name": "FrontEnd"}, _remaining(deadline))
            self._wait({"type": "lua", "state_name": "FrontEnd", "code": "return UI.IsInFrontEnd()"},
                       _remaining(deadline))
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
                "IsAutosave=false, IsQuicksave=false}, ServerType.SERVER_TYPE_NONE)\n"
                "if ok == false then error('Network.LoadGame returned false') end\nreturn true"
            )
            before = self._loadscreen_count()
            response_unknown = False
            try:
                self._lua("FrontEnd", code, min(5, _remaining(deadline)))
            except LuaResponseUnknown as exc:
                print(f"LoadGame response interrupted during transition: {exc}", flush=True)
                response_unknown = True
            self._enter_game_after_loadscreen(before, timeout, deadline=deadline,
                                              response_unknown=response_unknown)
            return f"loaded {name}"
        if action == "wait_for":
            self._wait(step["condition"], _remaining(deadline))
            return f"condition met: {step['condition']}"
        if action == "pause":
            if step["seconds"] > _remaining(deadline):
                raise ScenarioError("Step deadline would expire during pause")
            time.sleep(step["seconds"])
            return f"paused {step['seconds']}s for UI animation"
        raise AssertionError(action)

    def run(self) -> int:
        self.output.mkdir(parents=True, exist_ok=False)
        self._log("start", "OK", self.scenario["name"])
        try:
            front_end_ready = False
            if self.auto_restart:
                folders = enabled_mod_folders(self.scenario, self.mods_dir)
                newest = newest_mod_file(folders)
                process = single_civ_process()
                reason = restart_reason(newest, process[1] if process else None)
                if reason is not None:
                    self._log("auto_restart", "START", reason)
                    detail = self._restart_game(360)
                    latest = newest_mod_file(enabled_mod_folders(self.scenario, self.mods_dir))
                    relaunched = single_civ_process()
                    still_stale = restart_reason(latest, relaunched[1] if relaunched else None)
                    if still_stale is not None:
                        raise ScenarioError(f"Civ VI is still stale after restart: {still_stale}")
                    self._log("auto_restart", "OK", detail)
                    front_end_ready = True
                else:
                    self._log("auto_restart", "SKIP", "Civ VI process is newer than all scenario mod files")
            self.baseline = snapshot_logs(self.logs)
            (self.output / "baseline.json").write_text(json.dumps(self.baseline, indent=2), encoding="utf-8")
            if self.scenario["steps"][0]["action"] != "restart_game":
                require_single_civ_process()
                if not front_end_ready:
                    self._wait({"type": "lua", "state_name": "FrontEnd", "code": "return UI.IsInFrontEnd()"}, 10)
            for index, step in enumerate(self.scenario["steps"], 1):
                action = f"{index}/{len(self.scenario['steps'])} {step['action']}"
                self._log(action, "START", json.dumps(step, ensure_ascii=False))
                try:
                    if step["action"] != "wait_for":
                        self.log_step_baseline = snapshot_logs(self.logs)
                        self.log_matches_consumed.clear()
                    detail = self._run_step(step)
                    if step["action"] == "restart_game":
                        self.baseline = snapshot_logs(self.logs)
                        (self.output / "baseline.json").write_text(
                            json.dumps(self.baseline, indent=2), encoding="utf-8")
                        self.log_step_baseline = None
                        self.log_matches_consumed.clear()
                except (ScenarioError, TunerError, CaptureError, InputError, OSError, EOFError, ValueError) as exc:
                    self._log(action, "FAIL", str(exc))
                    return 1
                self._log(action, "OK", detail)
            assert self.baseline is not None
            checks = judge(read_since(self.logs, self.baseline), self.scenario.get("judge_profile", "generic"),
                           expected_units=self.scenario.get("expected_units"))
            print_table(checks)
            result = all(check.passed for check in checks)
            self._log("judge", "OK" if result else "FAIL", f"{sum(c.passed for c in checks)}/{len(checks)} checks passed")
            return 0 if result else 1
        except (ScenarioError, TunerError, InputError, OSError, ValueError) as exc:
            self._log("run", "FAIL", str(exc))
            return 1
        finally:
            if self.baseline is not None:
                try:
                    for name, content in read_since(self.logs, self.baseline).items():
                        (self.output / name).write_text(content, encoding="utf-8")
                except OSError as exc:
                    print(f"Could not save run log slices: {exc}", flush=True)
            if self.client:
                self.client.close()


def main() -> int:
    parser = argparse.ArgumentParser(description="Run a Civ VI PoC scenario with automatic mod freshness checks")
    parser.add_argument("scenario", type=Path)
    parser.add_argument("--check", action="store_true", help="Validate JSON only; do not connect to the game")
    parser.add_argument("--output", type=Path, help="New directory for baseline, trace and screenshots")
    parser.add_argument("--logs", type=Path)
    parser.add_argument("--saves", type=Path)
    parser.add_argument("--port", type=int, default=4318)
    parser.add_argument("--mods", type=Path, default=DEFAULT_MODS_DIR, help="Installed Civ VI Mods directory")
    parser.add_argument("--no-auto-restart", action="store_true", help="Skip startup freshness check (debugging only)")
    parser.add_argument("--owner-start", action="store_true",
                        help="Open the configured Create Game screen and wait for the owner to press Start Game")
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
        return Runner(data, output, args.logs or default_log_dir(), saves, args.port,
                      auto_restart=not args.no_auto_restart, mods_dir=args.mods,
                      owner_start=args.owner_start).run()
    except (OSError, json.JSONDecodeError, ScenarioError, ValueError) as exc:
        parser.exit(2, f"Scenario error: {exc}\n")


if __name__ == "__main__":
    raise SystemExit(main())
