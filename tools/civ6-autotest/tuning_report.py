"""Summarize only archived artifacts of a 200-turn tuning run; never connect to Civ6."""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re

AGE = re.compile(
    r'\[JAC_AGES\] judged reign=(\S+) turn=(\d+) length=(\d+)(?: era=(\d+))?'
    r'(?: engineEra=\S+)? player=(\d+) age=(GOLDEN|NORMAL|DARK|HEROIC)'
    r' score=(-?\d+) golden>=(\d+) dark>=(\d+) policy=(\S*)')
ARB = re.compile(r'\[JAC_ARB\] (.*)')
EXPECTED_CHECKPOINTS = {1, 51, 101, 151, 201}
EXPECTED_REIGNS = [('REIGN_KOJI',66), ('REIGN_EIROKU',73), ('REIGN_GENKI',110),
                   ('REIGN_TENSHO',119), ('REIGN_BUNROKU',178), ('REIGN_KEICHO',189)]


def coverage(checkpoints: list[str], judgments: list[dict], falls: list[dict] | None = None) -> tuple[list[str], int]:
    """Cross-check saved history, initial players, elimination events and checkpoints.

    If a revival or a defeat without a merit event breaks this chain, report partial
    coverage rather than guessing which players should have been judged.
    """
    issues, seen, rosters, alive = [], set(), {}, {}
    falls = falls or []
    for checkpoint in checkpoints:
        parts = checkpoint.split(' ; ')
        header = re.fullmatch(r'checkpoint turn=(\d+) era=\S+ targets=\d+', parts[0])
        if not header:
            issues.append('invalid checkpoint header')
            continue
        turn = int(header[1])
        if turn in seen:
            issues.append(f'duplicate checkpoint: T+{turn}')
        seen.add(turn)
        houses = [int(m[1]) for part in parts for m in [re.match(r'house player=(\d+) ',part)] if m]
        if len(houses)!=22 or set(houses)!=set(range(22)):
            issues.append(f'incomplete house observations: T+{turn}')
        statuses = [m.groups() for part in parts for m in [re.match(r'house player=(\d+) .*?\balive=(true|false)\b',part)] if m]
        if len(statuses)!=22 or {int(i) for i,_ in statuses}!=set(range(22)):
            issues.append(f'incomplete survivor observations: T+{turn}')
        alive[turn] = {int(i) for i,status in statuses if status=='true'}
        local_rosters = set()
        for part in parts:
            if not part.startswith('judgment-roster '):
                continue
            m = re.fullmatch(r'judgment-roster reign-index=(\d+) turn=(\d+) players=(\d+(?:,\d+)*)',part)
            if not m:
                issues.append(f'invalid judgment roster: T+{turn}')
                continue
            index, due = int(m[1]), int(m[2])
            ids = tuple(int(v) for v in m[3].split(','))
            if index in local_rosters:
                issues.append(f'duplicate roster within T+{turn}: reign {index}')
            local_rosters.add(index)
            if len(ids)!=len(set(ids)) or not set(ids)<=set(range(22)):
                issues.append(f'invalid player IDs in reign {index}')
            if index in rosters and rosters[index]!=(due,ids):
                issues.append(f'inconsistent saved roster: reign {index}')
            rosters[index]=(due,ids)
    if seen!=EXPECTED_CHECKPOINTS:
        issues.append(f'checkpoint turns mismatch: {sorted(seen)}')
    initial = alive.get(1,set())
    if initial!=set(range(22)):
        issues.append('initial roster is not the configured 22 houses')
    for turn,ids in alive.items():
        expected = initial - {f['player'] for f in falls if f['turn']<=turn}
        if ids!=expected:
            issues.append(f'cannot reconcile survivors with elimination events: T+{turn}')
    actual_reigns = {row['reign'] for row in judgments}
    if actual_reigns!={name for name,_ in EXPECTED_REIGNS}:
        issues.append('age log does not contain all six expected reigns')
    if set(rosters)!=set(range(1,7)):
        issues.append('saved judgment rosters do not cover all six expected reigns')
    for index,(name,turn) in enumerate(EXPECTED_REIGNS,1):
        rows = [row for row in judgments if row['reign']==name]
        expected = rosters.get(index)
        if expected is None:
            continue
        if expected[0]!=turn or any(row['turn']!=turn for row in rows):
            issues.append(f'judgment turn mismatch: {name}')
        survivors = initial - {f['player'] for f in falls if f['turn']<=turn}
        if set(expected[1])!=survivors:
            issues.append(f'saved roster survivor coverage mismatch: {name}')
        if {row['player'] for row in rows}!=set(expected[1]) or len(rows)!=len(expected[1]):
            issues.append(f'age log player coverage mismatch: {name}')
    return issues, len(seen & EXPECTED_CHECKPOINTS)


def summarize(artifact: Path) -> dict:
    steps = [json.loads(line) for line in (artifact / 'steps.jsonl').read_text(encoding='utf-8').splitlines() if line]
    lua_path = artifact / 'Lua.log'
    lua_bytes = lua_path.read_bytes()
    lua = lua_bytes.decode('utf-8-sig', errors='replace')
    completed = [s['detail'] for s in steps if s['status'] == 'OK' and s['action'].endswith('autoplay')]
    checkpoints = [s['detail'].split('return=', 1)[1] for s in steps
                   if s['status'] == 'OK' and s['action'].endswith('ui_lua') and 'return=checkpoint turn=' in s['detail']]
    overview = [s['detail'].split('return=', 1)[1] for s in steps
                if s['status'] == 'OK' and 'return=T+' in s['detail']]
    judgments, seen, grouped = [], set(), {}
    for m in AGE.finditer(lua):
        reign, turn, length, era, player, age, score, golden, dark, policy = m.groups()
        key = (reign, int(player))
        if key in seen:
            raise ValueError(f'duplicate age judgment: {key}; inspect run/reload boundaries')
        seen.add(key)
        row = dict(reign=reign, turn=int(turn), length=int(length), era=int(era) if era else None, player=int(player),
                   age=age, score=int(score), golden=int(golden), dark=int(dark), policy=policy or None)
        judgments.append(row)
        g = grouped.setdefault(reign, dict(turn=int(turn), length=int(length), ages=Counter(), policies=Counter()))
        if (g['turn'], g['length']) != (int(turn), int(length)):
            raise ValueError(f'inconsistent reign timing: {reign}')
        g['ages'][age] += 1
        if policy:
            g['policies'][policy] += 1
    arb_lines = [m.group(1) for m in ARB.finditer(lua)]
    counts = {kind: sum(line.startswith(kind + ' ') for line in arb_lines)
              for kind in ('request', 'accepted', 'refused', 'decision', 'void request', 'rejected request')}
    request_turns = [int(m.group(1)) for line in arb_lines if line.startswith('request ')
                     for m in [re.search(r'\bturn=(\d+)', line)] if m]
    counts['request_turns'] = request_turns
    counts['request_intervals'] = [b-a for a,b in zip(request_turns, request_turns[1:])]
    counts['safety_net'] = sum('SAFETY NET' in line for line in arb_lines)
    counts['refused_cases'] = len({m.groups() for line in arb_lines if line.startswith('refused ')
                                  for m in [re.search(r'pair=(\d+)-(\d+) turn=(\d+)', line)] if m})
    falls = [dict(capturer=int(m[1]), player=int(m[2]), turn=int(m[3]))
             for m in re.finditer(r'\[JAC_AGES\] points player=(\d+) part=ko .*?key=ko:(\d+) turn=(\d+)', lua)]
    failures = [s for s in steps if s['status'] == 'FAIL']
    judge = [s for s in steps if s['action'] == 'judge']
    full = (len(completed) == 4 and all('autoplay completed 50 turns' in s for s in completed)
            and any('return=T+201 surviving majors=' in s['detail'] for s in steps if s['status'] == 'OK')
            and bool(judge) and judge[-1]['status'] == 'OK' and not failures)
    issues, checkpoint_count = coverage(checkpoints, judgments, falls)
    return dict(artifact=artifact.name, lua_sha256=hashlib.sha256(lua_bytes).hexdigest(),
                complete_200_turns=full, autoplay=completed, overview=overview, checkpoints=checkpoints,
                reigns=grouped, judgments=judgments, arbitration=counts, elimination_point_events=falls,
                age_data_present=bool(judgments), checkpoint_count=checkpoint_count,
                measurement_complete=full and not issues, coverage_issues=issues, failures=failures, judge=judge,
                limitations=['No autonomous AI arbitration requests in this implementation.',
                             'Refused counts respondents; accepted counts cases.',
                             'Era values are observations, not measured trade/grievance/road effects.'])


def markdown(data: dict) -> str:
    lines = [f"実行: {data['artifact']}。200ターン完了とログ判定: {'PASS' if data['complete_200_turns'] else '未達'}。",
             f"計測の完全性: {'PASS' if data['measurement_complete'] else '未達（以下は部分集計。数値調整の根拠にしない）'}。",
             '', '| 改元 | ターン | 期の長さ | 黄金 | 英雄 | 通常 | 暗黒 |',
             '|---|---:|---:|---:|---:|---:|---:|']
    for reign, g in sorted(data['reigns'].items(), key=lambda item:item[1]['turn']):
        a = g['ages']
        lines.append(f"| {reign} | {g['turn']} | {g['length']} | {a['GOLDEN']} | {a['HEROIC']} | {a['NORMAL']} | {a['DARK']} |")
    c = data['arbitration']
    lines += ['', f"裁定の請求 {c['request']}件、成立 {c['accepted']}件、拒否した当事者 {c['refused']}人。",
              '現行実装にAIの自発請求が無いため、請求0件を頻度・費用の適正さの根拠には使わない。', '',
              '各家の存続・城数・暦などのチェックポイント：']
    lines += ['- ' + line for line in data['overview']]
    if not data['age_data_present']:
        lines += ['', '年齢判定の記録が無い。年齢の割合は評価できない。']
    lines += ['', f"追加の計測が完了した回数: {data['checkpoint_count']}/5。"]
    return '\n'.join(lines)+'\n'


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('artifact', type=Path)
    parser.add_argument('--json', type=Path)
    args = parser.parse_args()
    data = summarize(args.artifact)
    if args.json:
        args.json.write_text(json.dumps(data, ensure_ascii=False, indent=2)+'\n',encoding='utf-8')
    print(markdown(data))


if __name__ == '__main__':
    main()
