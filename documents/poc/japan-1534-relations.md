# 日本マップ：1534年の関係を開始時の外交状態へ反映する

[SSOT 3.1](../spec/ssot/03-map-and-scale.md#31-日本列島)の「1534年の史実の同盟・婚姻・主従の関係と、敵対・不仲の関係を、開始時の外交状態に反映する」の実装メモ。関係の調査は [`research/japan-1534-relations.md`](../research/japan-1534-relations.md)、データは [`spec/data/japan-1534-relations.json`](../spec/data/japan-1534-relations.json)。

## 実装

- `poc/JAC_PoC_JapanMapScale/Scripts/JAC_Japan_1534Relations.lua` を、日本マップの**史実の領土の2種**（15 km・20 km）の条件でだけ `AddGameplayScripts` に登録した。開拓者からの2種と万世紀ルールセットの他の地図では読み込まれず、関係も因縁の減点も適用されない（オーナー判断 2026-09-29・3a）
- 新規ゲームの `LuaEvents.NewGameInitialized` で1回だけ適用し、結果を Game プロパティ `JAC_JAPAN_1534_RELATIONS`（`Turn`・`Applied`・`Missing`・`Failed`）に残す。プロパティがあれば適用しない
- 念のため `Events.LoadScreenClose` でも、プロパティがなく開始ターンのままなら適用する。セーブのロードでは開始ターンを過ぎているか、プロパティが残っているので適用しない
- 勢力はプレイヤー番号ではなく文明名 `CIVILIZATION_JAC_<名前>` で引く。プレイヤー順を入れ替えた構成（武田が人間など）でも同じ組に適用する
- Lua の表は JSON の適用分の写し。`tests/test_japan_1534_relations.py` が一致を確かめる

## Civ6 の外交状態への対応

| 1534年の関係 | Civ6 で開始時に行うこと | 根拠 |
|---|---|---|
| 交戦中 | 双方 `SetHasMet` のあと `DeclareWarOn(相手, WarTypes.FORMAL_WAR, true)` | 公式シナリオと同じ。下記 |
| 同盟・婚姻・主従 | 双方 `SetHasMet` のあと、双方から `SetHasDeclaredFriendship(相手, true)`（暫定） | DLL の Lua バインディングにある。公式の使用例はないため要実機確認 |
| 友好（島津・琉球。R19） | 同盟と同じく双方 `SetHasDeclaredFriendship`（オーナー判断 4a） | 同上 |
| 敵対・不仲（7組） | 双方 `SetHasMet` のみ。好感度の減点は下記の修正値 | 下記 |

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

### 敵対・不仲：AI の好感度の減点「1534年の因縁」（オーナー判断 1a・2026-09-29）

開始時に Civ6 の「非難」や「不満」を置く API はないため、公式のアジェンダと同じ仕組みで AI の好感度に固定の減点を付ける。外交画面の「関係」欄に「-10 1534年の因縁」の行が出る。

- 仕組み：勢力別の特性 `TRAIT_JAC_1534_FEUD_<勢力>` を、その勢力の文明に `CivilizationTraits` で付け、特性に `MODIFIER_PLAYER_DIPLOMACY_SIMPLE_MODIFIER` の修正値 `JAC_1534_FEUD_<自分>_<相手>` を `TraitModifiers` で結ぶ。修正値の `SubjectRequirementSet`（相手プレイヤーへの条件）は「相手の文明が <相手>」で、`REQUIREMENT_PLAYER_TYPE_MATCHES` + `CivilizationType` を使う。理由の文言は `SimpleModifierDescription` = `LOC_JAC_DIPLO_MODIFIER_1534_FEUD`（「1534年の因縁」）
- 減点は両側にかける（北条→武田と武田→北条）。7組で14個。値は -10（公式の別政体 -6、友好宣言中の非難などの範囲に合わせた）
- 減衰しない：`InitialValue` だけを付け、`ReductionTurns` を付けない。公式の `AGENDA_SAME_GOVERNMENT`（`Base/Assets/Gameplay/Data/Agendas.xml:813`・`:2247`）と同じく、条件（相手がその文明）が成り立つ間は値が変わらない。時間で薄まる `STANDARD_DIPLOMACY_JOINT_WAR`（`Base/Assets/Gameplay/Data/Leaders.xml:3160`〜`:3177`、`ReductionTurns` = 20）とは別の作り。因縁が解消する仕組み（和睦の儀式など）は作らない（MVP）
- 開始時から効く：修正値は特性の付与と同時に有効になるはず（要実機確認）。ただし AI が相手を知らないと表示されないため、Lua が7組を `SetHasMet` で出会い済みにする
- 適用範囲：`Data/JAC_Japan_Feuds.xml` を `UpdateDatabase` で史実の領土の2種にだけ読み込む（開拓者からでは特性・修正値が存在しない）
- 生成：`tools/japan_map_scale/write_feuds.py` が JSON の敵対（`hostility`、high・medium、開始日を含むもの）から生成する。テストが JSON・XML・modinfo の一致を確かめる

公式ファイルの根拠（公式フォルダ内）

1. 好感度の修正値の型と理由の文言：`Base/Assets/Gameplay/Data/Agendas.xml:451`〜`:455`（`AGENDA_ALLY_OF_ENKIDU_FRIEND`）、`:1021`〜`:1050`（`InitialValue`・`ReductionTurns`・`SimpleModifierDescription`）
2. 相手プレイヤーで条件を判定する：`SubjectRequirementSetId`（`Agendas.xml:813`〜`:816`）。アジェンダの特性は `TraitModifiers` で結ぶ（`Agendas.xml:445`〜`:447`）
3. 相手の文明を条件にする：`DLC/GranColombia_Maya/Data/GranColombia_Maya_Civilizations.xml:403`・`:414`・`:427`（`REQUIREMENT_PLAYER_TYPE_MATCHES` + `CivilizationType`）。指導者版は `DLC/RulersOfEngland/Data/RulersOfEngland_Leaders.xml:479`（`REQUIREMENT_PLAYER_LEADER_TYPE_MATCHES`）
4. 外交画面が理由を出す処理：`Base/Assets/UI/DiplomacyActionView.lua:895`〜`:918`（`GetDiplomaticModifiers` の `Score`・`Text`）。シナリオの確認にも同じ関数を使う

## 実機確認（メインセッションで実行）

シナリオ：`tools/civ6-autotest/scenarios/japan-1534-relations.json`（15 km・史実の領土、織田が人間、22勢力）と、開拓者から版の `japan-1534-relations-settlers.json`（何も適用されないこと）

1. 開始直後：`JAC_JAPAN_1534_RELATIONS` が開始ターンに `Applied=16`・`Failed=0`・`Missing=0` で残る
2. 大内と大友が相互に交戦中
3. 北条・今川、大内・毛利、伊達・蘆名、足利・六角、島津・琉球が、双方向に `DIPLO_STATE_DECLARED_FRIEND`（`SetHasDeclaredFriendship` が効くかの確認を兼ねる）
4. 敵対7組は交戦も友好宣言もせず、両側の `GetDiplomaticModifiers` に -10「1534年の因縁」がある。無関係の織田・伊達と島津・琉球には因縁の行がない。実際の外交画面で「関係」欄に行が出ることも目で確認する（AI どうしで `GetDiplomaticModifiers` が取れない場合はシナリオの判定を人間側の組へ変える）
5. 1ターン進めても 2〜4 が保たれる
6. セーブ → メニュー → ロード後、プロパティの `Turn` が開始ターンのまま（再適用されていない）で、2〜4 が保たれる

`Failed` が 0 でなければ、Lua.log の `[JAC_1534_RELATIONS] FAILED` 行に関数名とエラーが出る。友好宣言が効かない場合は、同盟・婚姻・主従を未反映に戻すか、永久同盟を使うかをオーナーに確認する。

## オーナー判断（2026-09-29 Discord・セットA）

1. 1a：敵対・不仲は AI の好感度の減点「1534年の因縁」で表す。史実の領土の日本マップだけ、開始時から、減衰しない
2. 2a：同盟・婚姻・主従は友好宣言で近似する（実装済み）
3. 3a：史実の関係は史実の領土の日本マップにだけ適用し、開拓者からの2種には適用しない
4. 4a：島津と琉球を友好（友好宣言）で始める。1534年時点の直接の文献は未確認（`research/japan-1534-relations.md` R19）

## 未検証・注意

- 開拓者から始める2種には史実の関係を適用しない（オーナー判断 3a）。都市がない状態で交戦が始まるのを避けるため
- AI が開始直後に友好宣言を破棄したり和睦したりする頻度は未確認
