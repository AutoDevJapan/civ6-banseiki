# 天下統一の合算（チームと天下統一同盟）PoC

SSOT 3.1 の表「勝利」の「天下統一は1つの勢力で判定するが、プレイヤーどうしが最初から組んでいる同盟と、勝利まで合算することを前提に組む『天下統一同盟』では城を合算し、主体となる勢力だけが勝利する」を対象とする。天下統一の本体は [天下統一 PoC](unification-victory.md)。実機は未確認（下の「実機確認」）。

## 決めた読み方（最も単純な読み。オーナー判断は Q61〜Q64）

1. 「最初から組んでいる同盟」はゲーム設定のチーム。勝利の要件はチーム単位で評価されるので、同じチームの城は追加の実装なしで合算される（下の「チームの合算」）。1534年の開始時の同盟（味方8組）は含めない（Q61）
2. チームでは、Civ6 の勝者がチーム単位のため、チーム全員が勝つ。主体だけを勝たせる仕組みはエンジンにない（Q62）
3. 天下統一同盟は、ゲーム中に申し込んで組む。申し込んだ家が主体。人間の家には相手の受諾が要る。AI の家は、申し込んだ家と同盟（Civ6 の同盟）を結んでいれば、その場で加わる（公式の同盟が AI の同意の代わり）。AI からは申し込まない。1つの同盟に主体は1人で、加わる家の数に上限はない。1つの家が入れる同盟は1つ。同じチームどうしでは組めない（チームで合算済み）。同盟は勝利まで続き、抜けられない（Q63）。滅亡した家は同盟から外れる（都市を持たないので、その家が失った城は取った家の城として数える）。残りの家で数える登録城をすべて持てば主体が勝つ
4. 同盟の合算は登録城のある地図だけ。開拓者から始める版（登録城なし）は、今までどおりチームごとの本拠の条件（Q64）

## 仕組み

1. 画面：`poc/JAC_PoC_SengokuTrees/UI/JAC_UnificationPact.xml`・`.lua`（InGame に追加する画面）。画面上部中央の「天下統一同盟」ボタンで一覧を開く。出会った他チームの主要勢力ごとに「申し込む」「申し込みを受ける」、同盟に入っている家には「◯◯の同盟」を出す。状態と直前の結果（申し込んだ・成立・AI と同盟していない・組めない）を上に出す。画面は Game と Player のプロパティを読むだけで、変更は要求だけを送る
2. 要求：`UI.RequestPlayerOperation(自分, PlayerOperations.EXECUTE_SCRIPT, {OnStart = "JAC_UnificationPactPropose" または "JAC_UnificationPactAccept", Target = 相手})`。エンジンがすべての端末で同名の GameEvent を要求者のプレイヤー ID つきで起こす。要求者は引数でなくエンジンが渡す ID を使う
3. 状態：ゲーム側 `Scripts/JAC_Sengoku_UnificationPact.lua` だけが変える。`Game` のプロパティ `JAC_UNIFICATION_PACT_LEAD_<ID>`（主体の ID。主体自身にも付ける）と `JAC_UNIFICATION_PACT_PROPOSAL_<相手>`（申し込んだ家）、要求の結果は申し込んだ家（成立時は両方）の Player プロパティ `JAC_UNIFICATION_PACT_RESULT`。どれもセーブに含まれる
4. 勝利：ゲーム側 Lua が、都市の建設・移管（`GameEvents.CityBuilt`）、占領（`GameEvents.CityConquered`）、毎ターンの開始、同盟の成立のたびに、各同盟のチームが数える登録城（破却後の再建都市を除く）をすべて持つかを確かめる。持っていれば、主体の首都に隠しの印の建物 `BUILDING_JAC_PACT_UNIFIED`（`InternalOnly`、都市の中心）を付ける。勝利要件の3つ目の枝（`JAC_UNIFICATION_PACT`）は「そのチームがこの印のある都市を1つ以上持つ」なので、主体のチームだけが勝つ。同盟の相手のチームは印の都市を持たないので勝たない
5. 数える登録城は `JAC_Sengoku_RebuiltCastles.lua` の `ExposedMembers.JAC_UnificationCastles.CountedCities()` から読む（ゲーム側どうしの受け渡し。`JAC_Resources.lua` と同じ）。印を外し忘れた再建都市（占領直後）も数えるので、判定が甘くなることはない
6. 印は外さない（公式の Lua に建物を外す使用例がない）。同じ変化の中で勝利が成立する前提（unify-08 では `TransferCity` の直後に勝利した）。印を付けてから勝利の評価までに城を失うと、勝ちが残るおそれがある（要実機確認）

## チームの合算

勝利の要件はチームを対象に評価される。公式の Alexander シナリオは、全都市のコレクション（`COLLECTION_ALL_CITIES`）で各都市が `REQUIREMENT_PLAYER_OWNS_OBJECT` を満たすかを調べる勝利で、天下統一も同じ形。本拠の条件は公式の `REQUIREMENT_TEAM_DOMINATION_VICTORY`。勝者は `Game.GetWinningTeam()` でチームとして返り、勝利画面は自分のチームと比べる。このためチームの城は合算され、チーム全員が勝つ。チーム戦の実機は未確認（[天下統一 PoC](unification-victory.md) の「未確認」）。

## 公式実装との照合

以下はすべて `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/` 以下（行番号はこの PC の導入版）。

| 利用する挙動 | 公式ファイル:行 |
|---|---|
| UI から `EXECUTE_SCRIPT` で要求し、`OnStart` の名前の GameEvent をゲーム側で受ける | `DLC/BlackDeathScenario/UI/BlackDeathScenario_PopupHandler.lua:21-28`、`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:66`、`DLC/BlackDeathScenario/Scripts/BlackDeathScenario_Support.lua:116` |
| UI 側は要求だけ、ゲーム側で `SetProperty`（UI とゲームで分ける） | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario_StateUtils.lua:64-80,83,95` |
| 勝利要件をチームで評価する形（全都市・所有） | `DLC/AlexanderScenario/Data/AlexanderScenario_Victories.xml:18,25-30,53-58,72-81` |
| チームの制覇勝利の要件 | `Base/Assets/Gameplay/Data/Victories.xml:592` |
| 勝者はチーム。勝利画面は自分のチームと比べる | `Base/Assets/UI/EndGame/EndGameMenu.lua:461-462,1074-1075` |
| 件数の下限 `REQUIREMENT_COLLECTION_COUNT_ATLEAST` と `Count` | `Base/Assets/Gameplay/Data/Leaders.xml:6532-6533,6748-6756` |
| 都市が建物を持つかの要件 `REQUIREMENT_CITY_HAS_BUILDING` | `Base/Assets/Gameplay/Data/Buildings.xml:1800-1801,1894-1896` |
| 同盟の確認 `GetDiplomacy():HasAllied`（ゲーム側） | `DLC/VikingsScenario/Scripts/VikingScenario.lua:77` |
| 滅亡した家も含めた全プレイヤー `PlayerManager.GetWasEverAliveCount`（実機シナリオの家の探索） | `Base/Assets/UI/ARXManager.lua:32`、`Base/Assets/Maps/Utility/MapUtilities.lua:705` |
| 生存する主要勢力 `PlayerManager.GetAliveMajors`、`IsHuman` | `DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:47,49` |
| `IsMajor`（ゲーム側） | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:122` |
| `GetTeam`（ゲーム側） | `DLC/CivRoyaleScenario/Scripts/CivRoyaleScenario_UnitCommands.lua:157` |
| 首都 `GetCities():GetCapitalCity()`（ゲーム側） | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:1103` |
| 占領イベント、建設イベント、毎ターンのイベント | `DLC/NubiaScenario/Scripts/NubiaScenario.lua:937,969`、`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:56,255`、`DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:246` |
| UI で Game のプロパティを読む | `DLC/CivRoyaleScenario/UI/Additions/WorldTrackerPanel_CivRoyaleScenario.lua:13` |
| 出会い済みの確認 `HasMet`、自国のターン開始 | `Base/Assets/UI/ActionPanel.lua:772,1334` |
| GameCore の処理後の UI 更新 `Events.GameCoreEventPublishComplete` | `Base/Assets/UI/Choosers/ResearchChooser.lua:485` |
| `InstanceManager`、`CalculateSize`、`DoAutoSize` | `Base/Assets/UI/Panels/CityPanelOverview.lua:53,169`、`Base/Assets/UI/Panels/CityPanel.lua:375`、`Base/Assets/UI/Choosers/EspionageChooser.lua:336` |

## 検証

- 静的検査（`python -m unittest discover -s tests -t .`）：`tests/test_unification_victory.py` が要件の行を模擬評価し、印のある都市を持つチームだけが勝つ（相手のチームと部外者は勝たない）、印がなければ同じ城の分け方でも誰も勝たない、を確かめる。modinfo の登録、画面が状態を変えないこと、文言の3言語もここで見る
- Lua（lupa）：`tests/test_unification_pact_lua.py` が `JAC_Sengoku_RebuiltCastles.lua` と一緒にゲーム側 Lua を動かし、同盟していない AI は加わらない、同盟中の AI は加わる、部外者が城を1つ持つ間は印を付けない、相手に城が移ると主体の首都にだけ印を付ける、人間は受諾が要る（申し込んでいない家や別の家の受諾は無効）、1つの家は1つの同盟だけ、同じチームは組めない、破却後の再建都市は妨げない、登録城のない地図では印を付けない、同盟の相手が滅亡しても残りの家で全城を持てば主体の首都に印を付ける、滅亡した相手の城を部外者が取れば印を付けない（滅亡した家には申し込めない）、を確かめる
- 実機シナリオ `tools/civ6-autotest/scenarios/unification-pact.json`（`python tools/generate_unification_victory.py` で生成）。検証地図（[天下統一 PoC](unification-victory.md) と同じ `JAC_Test_UnificationMap`）で織田（人間）・伊達・蘆名：
  1. 登録城4、織田2・伊達2で勝者なし
  2. 同盟の画面から伊達へ申し込む → 結果が「AI と同盟していない」、同盟は成立しない
  3. 伊達の本拠でない城を蘆名へ移し、織田と伊達を同盟にする（1534年の関係と同じく友好宣言のあと `SetHasAllied`）
  4. もう一度申し込む → 成立（伊達の主体が織田）。同盟は4城のうち3城なので、印も勝者もない
  5. 画面を開いて `unification-pact-panel.png` を撮る
  6. 蘆名の城を伊達（同盟の相手）へ移す（蘆名は唯一の都市を失って滅亡する。以降の家の探索は滅亡した家も含める）→ 織田の首都に印が付く → 勝者が織田のチーム（伊達のチームではない）、勝利タイプが天下統一 → `unification-pact-victory.png`

## 実機確認（途中まで）

2026-09-29 の unification-alliance-01 で、上の手順1〜6の印まで通った（申し込み・「AI と同盟していない」・同盟後の成立・3城で印なし・移管で印、`Lua.log` に終了画面の表示）。最後の勝者の確認だけが、滅亡した蘆名を生存中の家から探して「Date or Ashina missing」で止まった。家の探索を `GetWasEverAliveCount` に直したので、再実行で勝者の確認まで見る。

```
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/unification-pact.json
```

- 上のシナリオ全体（`EXECUTE_SCRIPT` の自作 GameEvent、ゲーム側の `HasAllied`、印による勝利の成立の時期）
- 画面の見た目と位置（上部中央。他の画面と重ならないか）。スクリーンショットで確認する
- マルチプレイ：人間どうしの申し込みと受諾、全端末で同じ状態になるか。相手への通知はなく、相手が画面を開くと「申し込みを受ける」が出る
- チーム戦での合算とチーム全員の勝利
- 印を付けてから勝利の評価までに城を失った場合
