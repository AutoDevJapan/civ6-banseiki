# 日本マップの世界会議の無効化（実装メモ）

SSOT 3.2（2026-09-29）の決定「日本マップでは Civ6 の世界会議（世界法律）を行わない」の実装と、実機確認の項目。朝廷・将軍の裁定は別設計（[`../research/court-arbitration.md`](../research/court-arbitration.md)）。

## 実装

`poc/JAC_PoC_JapanMapScale/Data/JAC_Japan_MapRules.xml` の `<Types>` に `<Delete Type="CAPABILITY_WORLD_CONGRESS"/>` を1行足した。このファイルは日本マップ4種（20km/15km、史実領土/入植）の条件でだけ読み込まれる（`JAC_PoC_JapanMapScale.modinfo` の `RuleSetInUse` + `MAP_SCRIPT`）。世界マップなど他のマップでは世界会議は残る。

### 根拠（公式ファイル）

公式シナリオは、世界会議を切るときに同じ行を使っている。ゲーム本体の「能力（Capability）」を型ごと消す方式で、`GameCapabilities` の行も外れる。

- 定義: `DLC/Expansion2/Data/Expansion2_GameCapabilities.xml:4`（`CAPABILITY_WORLD_CONGRESS` の型）、`:14`（`GameCapabilities` への登録）
- 公式の無効化: `DLC/BlackDeathScenario/Data/BlackDeathScenario_RemoveData.xml:34`、`DLC/WarMachineScenario/Data/WarMachineScenario_RemoveData.xml:50`、`DLC/CivRoyaleScenario/Data/CivRoyaleScenario_RemoveData.xml:60`、`DLC/PiratesScenario/Data/PiratesScenario_RemoveData.xml:141`（すべて `<Delete Type="CAPABILITY_WORLD_CONGRESS"/>`）
- 黒死病は同じ場所で `CAPABILITY_WORLD_CLIMATE` や `CAPABILITY_RANDOM_EVENTS` も消している（`BlackDeathScenario_RemoveData.xml:33,35`）。今回は世界会議の1つだけを消し、気候・ランダムイベント・外交支持（Favor）は残す。
- UI が能力を見て初期化を止める箇所（能力が無ければエラーにならず何も作らない）:
  - `DLC/Expansion2/UI/Additions/WorldCongressPopup.lua:2684-2686`（コメント「No capability means never initialize this so it can never be used.」）
  - `DLC/Expansion2/UI/Additions/WorldCongressBetweenTurns.lua:228-230`
  - `DLC/Expansion2/UI/Replacements/DiplomacyRibbon_Expansion2.lua:105-107`（会議ボタンを作らない）

### 開催時期の仕組み

GS の開催時期は次の値で決まる。`Expansion2_GlobalParameters.xml:296`（`WORLD_CONGRESS_INITIAL_ERA` = 2、時代番号2（公式の並びでは古代0・古典1・中世2）に入ると初回）、`:300`（`WORLD_CONGRESS_MAX_TIME_BETWEEN_MEETINGS` = 30 ターン）、`:302`（`WORLD_CONGRESS_MIN_TIME_BETWEEN_SPECIAL_SESSIONS` = 15）。開催の判定はゲーム本体（DLL）の中にあり、Lua やデータからは読めない。

- 暫定: この値は変えない。能力を消せば、公式シナリオと同じく会議が開かれない、という公式の使い方に乗る。
- 公式シナリオが同じ削除で会議を止めていること自体は確かめてある。ただし「万世紀のルールセット（GameCore Expansion2）＋日本マップでも DLL が会議を開かない」ことは、実機でしか確かめられない（下記）。

### 触れていないもの

- 外交支持（Favor）: `Players[id]:GetFavor()` 系。会議とは別の能力ではなく、能力削除の対象にしていない。
- 世界気候、ランダムイベント、緊急事態（Emergency）の型・データ: 消していない。緊急事態は会議の議題として出るので、会議が無ければ開かれない（未確認、実機の警告ログで見る）。
- 会議に依存する万世紀の仕様（SSOT 9.x の「世界会議の投票」「非難議題」、5章の上限解放条件）は、日本マップでは使えない。世界マップ用の仕様として残し、日本マップ向けは朝廷裁定（研究文書）に置き換える。

## テスト

- `tests/test_japan_no_world_congress.py`: 削除が日本マップ4種の条件（ルールセット + `MAP_SCRIPT`）でだけ読み込まれること、削除が世界会議の1行だけで他の GS の能力に触れていないこと。
- `tools/civ6-autotest/judge.py` / `tests/test_judge.py`: 日本マップ判定に「世界会議に関わる UI・ゲーム側のエラー」の検査を追加（`ActionPanel_Expansion2`、`DiplomacyRibbon_Expansion2` などの実行時エラーを失敗にする）。

## 実機確認（メインセッション）

シナリオ: `tools/civ6-autotest/scenarios/japan-no-world-congress.json`（15km 史実領土マップ（採用の縮尺、SSOT 3.4）、AI 21 + 琉球を人間、180 ターン自動進行）。確認する内容:

1. 世界会議の能力が無い一方、`CAPABILITY_WORLD_CLIMATE` / `RANDOM_EVENTS` / `DIPLOMACY` / `ERAS` があり、外交支持と気候の値が取れる。
2. `WORLD_CONGRESS_INITIAL_ERA`（2）以上の時代に入ってから `MAX_TIME_BETWEEN_MEETINGS`（30）ターンを超えるまで進め、世界会議のイベント（`WorldCongressStage1/2/Finished`、臨時会合の招集）が1回も起きない。会議が進行中でもない。
3. 進行後も政府画面・偉人画面が開ける（読み取り専用にならない）。
4. ログ判定（`japan_map` プロファイル）で、世界会議に関わるエラーが無い。

180 ターンで時代番号2に届かない場合は、シナリオが失敗して知らせる。その場合は autoplay のターン数を増やす（戦国ツリーで時代の進み方が変わるため、180 は暫定）。

