"""Count AI arbitration requests, outcomes and actual costs from an uninterrupted 200-turn artifact."""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import re

ARB = re.compile(r"\[JAC_ARB\] ([^\r\n]+)")
FIELDS = re.compile(r"(\w+)=([^\s]+)")


def summarize(artifact: Path) -> dict:
    lua_bytes = (artifact / "Lua.log").read_bytes()
    events = ARB.findall(lua_bytes.decode("utf-8-sig", errors="replace"))
    steps = [json.loads(line) for line in (artifact / "steps.jsonl").read_text(encoding="utf-8").splitlines() if line.strip()]
    issues, requests, costs, outcomes = [], {}, {}, {}
    refusals, decisions, ai, blocked = [], Counter(), {}, Counter()
    refused_players, loads = {}, 0
    end_turns = set()
    for event in events:
        f = dict(FIELDS.findall(event))
        if event.startswith("loaded "):
            loads += 1
            end_turns.add(int(f["shogunate_end_turn"]))
        if event.startswith("request "):
            turn = int(f["turn"])
            if turn in requests:
                issues.append(f"duplicate request on turn {turn}")
            requests[turn] = {k: int(f[k]) for k in ("player", "turn", "power")}
            requests[turn]["pair"] = tuple(sorted(map(int, f["pair"].split("-"))))
        elif event.startswith("cost "):
            turn = int(f["turn"])
            if turn in costs:
                issues.append(f"duplicate request cost on turn {turn}")
            costs[turn] = dict(player=int(f["player"]), pair=tuple(sorted(map(int, f["pair"].split("-")))),
                               source=f["by"], gold=int(f["gold"]), legitimacy=int(f["legitimacy"]),
                               gold_after=float(f["gold_after"]), legitimacy_after=int(f["legitimacy_after"]))
        elif event.startswith("ai request "):
            turn = int(f["turn"])
            if turn in ai:
                issues.append(f"duplicate AI decision on turn {turn}")
            ai[turn] = dict(player=int(f["player"]), source=f["by"], score=int(f["score"]),
                            pair=tuple(sorted(map(int, f["pair"].split("-")))), detail=f)
        elif event.startswith(("accepted ", "refused ", "void request ")):
            turn = int(f["turn"])
            pair = tuple(sorted(map(int, f["pair"].split("-"))))
            kind = event.split(" ")[0]
            if kind == "void":
                kind = "void"
            key = (turn, pair)
            if key in outcomes and (kind != "refused" or outcomes[key] != "refused"):
                issues.append(f"duplicate or conflicting ruling {key}")
            outcomes[key] = kind
            if kind == "refused":
                respondent = (turn, pair, int(f["player"]))
                if respondent in refused_players:
                    issues.append(f"duplicate refused respondent {respondent}")
                refused_players[respondent] = int(f["legitimacy"])
        elif event.startswith("refusal cost "):
            row = {k: int(f[k]) for k in ("player", "turn", "intended", "applied", "legitimacy_before", "legitimacy_after")}
            row["pair"] = tuple(sorted(map(int, f["pair"].split("-"))))
            refusals.append(row)
        elif event.startswith("decision "):
            decisions[f["by"] + ":" + f["accept"]] += 1
        elif event.startswith("ai blocked "):
            for key in ("gold", "legitimacy", "interval"):
                blocked[key] += int(f[key])
    sources, spending = Counter(), defaultdict(lambda: Counter())
    player_turns, pair_turns = defaultdict(list), defaultdict(list)
    for turn, request in requests.items():
        cost = costs.get(turn)
        if not cost or (cost["player"], cost["pair"]) != (request["player"], request["pair"]):
            issues.append(f"request cost missing or mismatched on turn {turn}")
            continue
        source = cost["source"]
        sources[source] += 1
        spending[source]["gold"] += cost["gold"]
        spending[source]["legitimacy"] += cost["legitimacy"]
        expected = ((100 + request["power"] - 1) // request["power"], (5 + request["power"] - 1) // request["power"])
        if (cost["gold"], cost["legitimacy"]) != expected or cost["gold_after"] < 0 or cost["legitimacy_after"] < 0:
            issues.append(f"request cost violates recorded power or balance on turn {turn}")
        if source.startswith("ai_"):
            decision = ai.get(turn)
            if not decision or (decision["player"], decision["pair"], decision["source"]) != (request["player"], request["pair"], source):
                issues.append(f"AI decision missing or mismatched on turn {turn}")
            player_turns[request["player"]].append(turn)
            pair_turns[request["pair"]].append(turn)
    for turn in costs.keys() - requests.keys():
        issues.append(f"cost without request on turn {turn}")
    for turn in ai.keys() - requests.keys():
        issues.append(f"AI decision without request on turn {turn}")
    for (turn, pair), outcome in outcomes.items():
        request = requests.get(turn - 1)
        if not request or request["pair"] != pair:
            issues.append(f"ruling without preceding request on turn {turn}")
    for turn, request in requests.items():
        if (turn + 1, request["pair"]) not in outcomes and turn < 201:
            issues.append(f"request has no following-turn ruling on turn {turn}")
    refusal_keys = Counter()
    for row in refusals:
        key = (row["turn"], row["pair"], row["player"])
        refusal_keys[key] += 1
        request = requests.get(row["turn"] - 1)
        if (key not in refused_players or refused_players[key] != row["legitimacy_after"]
                or not request or request["pair"] != row["pair"]
                or row["intended"] != 15 * request["power"]):
            issues.append(f"refusal cost missing or mismatched respondent {key}")
        if row["legitimacy_before"] - row["legitimacy_after"] != row["applied"] or not 0 <= row["applied"] <= row["intended"]:
            issues.append(f"invalid applied refusal loss on turn {row['turn']}")
    if set(refusal_keys) != set(refused_players) or any(count != 1 for count in refusal_keys.values()):
        issues.append("refusal costs must match each respondent exactly once")
    if len(end_turns) != 1:
        issues.append("missing or inconsistent shogunate cutoff")
    cutoff = next(iter(end_turns)) if len(end_turns) == 1 else None
    shogun_after = [turn for turn, cost in costs.items() if cost["source"] == "ai_shogun" and cutoff is not None and turn >= cutoff]
    if shogun_after:
        issues.append(f"third-party shogun requests after cutoff: {shogun_after}")
    if cutoff is not None and any(request["power"] != 1 for turn, request in requests.items() if turn >= cutoff):
        issues.append("empowered request after shogunate cutoff")
    player_intervals = {str(key): [b - a for a, b in zip(sorted(turns), sorted(turns)[1:])] for key, turns in player_turns.items()}
    pair_intervals = {"_".join(map(str, key)): [b - a for a, b in zip(sorted(turns), sorted(turns)[1:])] for key, turns in pair_turns.items()}
    if any(interval < 10 for groups in (player_intervals, pair_intervals) for intervals in groups.values() for interval in intervals):
        issues.append("AI request interval shorter than ten turns")
    completed = [s for s in steps if s["status"] == "OK" and s["action"].endswith("autoplay")
                 and "autoplay completed 50 turns" in s["detail"]]
    judge = [s for s in steps if s["action"] == "judge"]
    failures = [s for s in steps if s["status"] == "FAIL"]
    full = (len(completed) == 4 and bool(judge) and judge[-1]["status"] == "OK" and not failures
            and any(s["status"] == "OK" and "return=T+201 surviving majors=" in s["detail"] for s in steps))
    if loads != 1 or any(s["status"] == "OK" and s["action"].endswith(" load") for s in steps):
        full = False
        issues.append("reloaded or missing gameplay log is not an uninterrupted measurement")
    if (artifact / "result.json").exists():
        result = json.loads((artifact / "result.json").read_text(encoding="utf-8"))
        if result.get("test_mode"):
            full = False
            issues.append("controlled clock/test mode is not an uninterrupted balance measurement")
    outcome_counts = Counter(outcomes.values())
    refused_rows = sum(event.startswith("refused ") for event in events)
    if refused_rows != len(refusals):
        issues.append("refusal losses missing or duplicated")
    unfinished = sum((turn + 1, request["pair"]) not in outcomes for turn, request in requests.items())
    return dict(artifact=artifact.name, lua_sha256=hashlib.sha256(lua_bytes).hexdigest(),
                complete_200_turns=full, measurement_complete=full and not issues, issues=issues,
                requests=len(requests), sources=sources, outcomes=outcome_counts, unfinished=unfinished,
                refused_respondents=refused_rows, decisions=decisions, spending=dict(spending),
                refusal_loss_intended=sum(row["intended"] for row in refusals),
                refusal_loss_applied=sum(row["applied"] for row in refusals),
                blocked_candidate_visits=blocked, player_intervals=player_intervals, pair_intervals=pair_intervals,
                shogunate_cutoff=cutoff, shogun_after_cutoff=shogun_after, costs=list(costs.values()),
                ai_decisions=list(ai.values()), failures=failures,
                limitations=["A single seed does not isolate the causal effect of fees, refusal penalties or cooldowns.",
                             "Blocked counts are candidate visits before selecting a request, not distinct houses.",
                             "Army and city contractions are sampled proxies, not opponent-attributed losses."])


def markdown(data: dict) -> str:
    result = "PASS" if data["measurement_complete"] else "未達（部分集計）"
    lines = [f"200ターンの裁定計測: {result}。実行: {data['artifact']}。",
             f"請求 {data['requests']}件、成立 {data['outcomes'].get('accepted', 0)}件、拒否のあった裁定 {data['outcomes'].get('refused', 0)}件。",
             f"拒否した当事者 {data['refused_respondents']}人、失効 {data['outcomes'].get('void', 0)}件、未裁定 {data['unfinished']}件。",
             "", "| 請求経路 | 件数 | 金の支出 | 正当性の支出 |", "|---|---:|---:|---:|"]
    names = {"ai_party": "疲弊した当事者AI", "ai_shogun": "将軍家AIの仲介", "request": "画面からの請求"}
    for source, count in sorted(data["sources"].items()):
        cost = data["spending"][source]
        lines.append(f"| {names.get(source, source)} | {count} | {cost['gold']} | {cost['legitimacy']} |")
    lines += ["", f"拒否による正当性の損失: 規則上 {data['refusal_loss_intended']}、実際の控除 {data['refusal_loss_applied']}（下限0）。",
              f"権限終了後の将軍家の仲介: {len(data['shogun_after_cutoff'])}件。",
              "費用不足・正当性不足・間隔待ちで進めなかった候補の延べ観測: "
              + ", ".join(f"{k}={v}" for k, v in sorted(data["blocked_candidate_visits"].items())) + "。",
              "1つの対局では費用・不利益・間隔の因果効果を分離できない。比較の根拠が無い値は変更しない。"]
    if data["issues"]:
        lines += ["", "計測の不足:"] + ["- " + issue for issue in data["issues"]]
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact", type=Path)
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()
    data = summarize(args.artifact)
    if args.json:
        args.json.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(markdown(data))


if __name__ == "__main__":
    main()
