# 日本マップ：1534年の関係を開始時の外交状態へ反映する

[SSOT 3.1](../spec/ssot/03-map-and-scale.md#31-日本列島)の「1534年の史実の同盟・婚姻・主従の関係と、敵対・不仲の関係を、開始時の外交状態に反映する」の実装メモ。関係の調査は [`research/japan-1534-relations.md`](../research/japan-1534-relations.md)、データは [`spec/data/japan-1534-relations.json`](../spec/data/japan-1534-relations.json)。

## 実装

- `poc/JAC_PoC_JapanMapScale/Scripts/JAC_Japan_1534Relations.lua` を、日本マップ4種（史実の領土・開拓者から × 15 km・20 km）の条件で `AddGameplayScripts` に登録した。万世紀ルールセットの他の地図では読み込まれない
- 新規ゲームの `LuaEvents.NewGameInitialized` で1回だけ適用し、結果を Game プロパティ `JAC_JAPAN_1534_RELATIONS`（`Turn`・`Applied`・`Missing`・`Failed`）に残す。プロパティがあれば適用しない
- 念のため `Events.LoadScreenClose` でも、プロパティがなく開始ターンのままなら適用する。セーブのロードでは開始ターンを過ぎているか、プロパティが残っているので適用しない
- 勢力はプレイヤー番号ではなく文明名 `CIVILIZATION_JAC_<名前>` で引く。プレイヤー順を入れ替えた構成（武田が人間など）でも同じ組に適用する
- Lua の表は JSON の適用分の写し。`tests/test_japan_1534_relations.py` が一致を確かめる

## Civ6 の外交状態への対応

| 1534年の関係 | Civ6 で開始時に行うこと | 根拠 |
|---|---|---|
| 交戦中 | 双方 `SetHasMet` のあと `DeclareWarOn(相手, WarTypes.FORMAL_WAR, true)` | 公式シナリオと同じ。下記 |
| 同盟・婚姻・主従 | 双方 `SetHasMet` のあと、双方から `SetHasDeclaredFriendship(相手, true)`（暫定） | DLL の Lua バインディングにある。公式の使用例はないため要実機確認 |
| 敵対・不仲 | 何もしない（TODO） | 非難・不満（Grievance）を開始時に設定する Lua API がない |

- 同盟を友好宣言で近似したのは、Civ6 の同盟を Lua で結ぶ手段が `SetPermanentAlliance`（解消できない永久同盟）しかないため。1534年の同盟は数年で崩れる（R02 は1537年に破綻）ので、永久同盟は採らない。友好宣言は標準で30ターンで切れ、その後は AI の判断に任せる（暫定）
- 主従は Civ6 に対応する外交状態がない。友好宣言で近似し、主従そのものは SSOT 9 の関係宣言（従属国など）が実装されたときに置き換える（TODO）

### 公式ファイルの根拠

公式フォルダ：`C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI`

1. 新規ゲームの初期化：`DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:11`（`InitializeNewGame`）と `:250`（`LuaEvents.NewGameInitialized.Add`）、`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:1303` と `:1397`。どちらも `AddGameplayScripts` で読み込まれるゲームプレイスクリプト（`DLC/AustraliaScenario/AustraliaScenario.modinfo:70`）
2. 開始ターンの判定：`DLC/PiratesScenario/Scripts/PiratesScenario_StartScript.lua:925`（`Game.GetCurrentGameTurn() == GameConfiguration.GetStartTurn()`）
3. 出会い済みにする：`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:1308`、`Base/Assets/Scenarios/ColdWarScenario/ColdWarScenario_StartScript.lua:35`
4. 開始時の宣戦：`DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:33`、`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:153`（`DeclareWarOn(id, WarTypes.FORMAL_WAR, true)`）
5. 永久同盟（不採用）：`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:1316`（`SetPermanentAlliance`）
6. 友好宣言の状態と期限：`Base/Assets/Gameplay/Data/DiplomaticActions.xml:110`（`DIPLO_STATE_DECLARED_FRIEND`）、`Base/Assets/Gameplay/Data/GlobalParameters.xml:234`（`DIPLOMACY_DECLARED_FRIENDSHIP_TIME_LIMIT` = 30）
7. 状態の読み取り（シナリオの確認に使う）：`Base/Assets/UI/DiplomacyActionView.lua:194`〜`:200`（`DiplomaticStates`）、`:870`（`GetDiplomaticAI():GetDiplomaticStateIndex`）
8. Lua から呼べる外交の設定関数：`DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll` の文字列に `lSetHasMet`・`lDeclareWarOn`・`lSetPermanentAlliance`・`lSetHasDeclaredFriendship`・`lSetHasDelegationAt`・`lSetHasEmbassyAt`・`lMakePeaceWith`・`lNeverMakePeaceWith`・`lPrepareForWarWith` がある。非難（Denounce）と不満（Grievance）の設定関数は、取得用の `lGetDenounceTurn`・`lGetGrievancesAgainst` しかない。行番号のないバイナリなので、実機確認で補う

### 敵対・不仲の表し方（TODO・オーナー判断待ち）

開始時に Civ6 の「非難」や「不満」を置く API はない。候補は2つ。

1. AI の好感度の修正値で表す：`MODIFIER_PLAYER_DIPLOMACY_SIMPLE_MODIFIER`（`Base/Assets/Gameplay/Data/Agendas.xml:452`）に `InitialValue`・`ReductionTurns`（`Base/Assets/Gameplay/Data/Leaders.xml:3160`〜`:3177` の例）を付け、相手を `REQUIREMENT_PLAYER_HAS_CIVILIZATION_OR_LEADER_TRAIT`（`DLC/Expansion2/Data/Expansion2_Emergencies.xml:898`）で絞る。ただし現在の22勢力は特性 `TRAIT_JAC_1534_FACTION` を共有しており、勢力ごとの特性を足す必要がある。外交画面の「関係」に「1534年の因縁」のような行が出る
2. 強い敵対（R09・R10 など）だけ開始時に交戦中にする

## 実機確認（メインセッションで実行）

シナリオ：`tools/civ6-autotest/scenarios/japan-1534-relations.json`（15 km・史実の領土、織田が人間、22勢力）

1. 開始直後：`JAC_JAPAN_1534_RELATIONS` が開始ターンに `Applied=8`・`Failed=0`・`Missing=0` で残る
2. 大内と大友が相互に交戦中
3. 北条・今川、大内・毛利、伊達・蘆名、足利・六角が、双方向に `DIPLO_STATE_DECLARED_FRIEND`（`SetHasDeclaredFriendship` が効くかの確認を兼ねる）
4. 敵対の北条・武田は交戦も友好宣言もしていない。無関係の織田・伊達は交戦していない
5. 1ターン進めても 2〜4 が保たれる
6. セーブ → メニュー → ロード後、プロパティの `Turn` が開始ターンのまま（再適用されていない）で、2〜4 が保たれる

`Failed` が 0 でなければ、Lua.log の `[JAC_1534_RELATIONS] FAILED` 行に関数名とエラーが出る。友好宣言が効かない場合は、同盟・婚姻・主従を未反映に戻すか、永久同盟を使うかをオーナーに確認する。

## 未検証・注意

- 開拓者から始める2種では都市がない状態で交戦が始まる。史実の関係は両方の開始方法に適用した（暫定）
- AI が開始直後に友好宣言を破棄したり和睦したりする頻度は未確認
