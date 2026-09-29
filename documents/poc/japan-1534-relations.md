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
| 敵対・不仲（7組） | Lua では何もしない。好感度の減点は下記の修正値 | 下記 |
| 全勢力の出会い | 開始時に1回だけ、22家すべてが互いに `SetHasMet`（両方向）。固定キャラだけが出る日本マップなので、全員が互いを知っている（オーナー判断 2026-09-29・Discord G）。ロード時は再適用しない | `Apply` の先頭。下記 |

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

- 仕組み：勢力別の特性 `TRAIT_JAC_1534_FEUD_<勢力>` を、その勢力の文明に `CivilizationTraits` で付け、特性に `MODIFIER_PLAYER_DIPLOMACY_SIMPLE_MODIFIER` の修正値 `JAC_1534_FEUD_<自分>_<相手>` を `TraitModifiers` で結ぶ。修正値の `SubjectRequirementSet`（相手プレイヤーへの条件）は「相手の文明が <相手>」で、`REQUIREMENT_PLAYER_TYPE_MATCHES` + `CivilizationType` を使う。理由の文言は `SimpleModifierDescription` = `LOC_JAC_DIPLO_MODIFIER_1534_FEUD`（「1534年の因縁」）。さらに修正値ごとに `ModifierStrings`（`Context="Sample"`、`Text=LOC_TOOLTIP_SAMPLE_DIPLOMACY_ALL` = `{diplomaticReason}`）の行が要る。公式は全エージェンダ修正値に付けている（`Base/Assets/Gameplay/Data/Agendas.xml:2296`〜`:2370`）。この行が無いと `GetDiplomaticModifiers` の `Text` が `LOC_TOOLTIP_DIPLOMACY_UNKNOWN_REASON`（「理由不明」）になり、外交画面は灰色でそう表示する（`DiplomacyActionView.lua:915`）。2026-09-29 の実機（15 km 史実マップ、1〜2ターン目）で、-10 の行は出たが `Text` が Unknown Reason だった。公式の修正値（今川→北条の -4・9、武田→北条の -4「First impressions of you」）まで Unknown Reason になるのは、データの欠落ではなく公式の仕様：見る側のアクセスレベルが「なし」のあいだは、相手の好感度の理由をすべて伏せる（「Limited（限定的）の外交可視性を得ると、相手のあなたへの評価についての情報が分かる」`Base/Assets/Text/en_US/Civilopedia_Concepts_Text.xml:771`、外交画面の説明「アクセスレベルを上げると、ここに追加の情報が出る」`Base/Assets/Text/en_US/DiplomacyPanel_Text.xml:177`）。出会ったばかりの家はアクセスレベルが「なし」で、戦国パックは公式の技術を消すので活版印刷の +1（`TECH_PRINTING` の `PRINTING_ADD_DIPLO_VISIBILITY`、`Base/Assets/Gameplay/Data/Technologies.xml:546`〜`:616`・`:670`〜`:672`）も得られない（2026-09-29 の実機ログ `DiplomacySummary.csv` も全行 Visibility=0）。そこで、この日本マップでは22家すべてに、活版印刷と同じ修正値 `MODIFIER_PLAYER_ADD_DIPLO_VISIBILITY`（`Amount`=1、`Source`=`SOURCE_JAC_1534_ACCESS`、`SourceType`=`DIPLO_SOURCE_ALL_NAMES`）を、全22文明が持つ特性 `TRAIT_JAC_1534_ACCESS` に付け、出会い済みの全勢力へのアクセスレベルを「限定的」に上げる（オーナー判断 2026-09-29・Discord G。仮：推奨案で実装、確認待ち）。文明の特性に付ける前例：モンゴルの `TRAIT_TRADING_POST_DIPLO_VISIBILITY`、`DLC/Expansion1/Data/Expansion1_Civilizations_Major.xml:178`〜`:179`・`:352`〜`:353`・`:620`〜`:629`・`:1100`〜`:1102`。アクセスレベルの画面に出る出所の行は `DiplomaticVisibilitySources` の `SOURCE_JAC_1534_ACCESS`（「諸家のあいだを行き交う噂」）。出所の行には、何によって得るかを示す列が要る。公式の出所の行は必ずどれか1つを持つ（`PrereqTech`・`TraitType`・`GreatPersonIndividualType`・`Trader`/`Delegate`/`Ally`/`Spy`、モンゴルは拡張の表の `TradePostTrait`。`Base/Assets/Gameplay/Data/Gossip.xml:75`〜`:81`、`DLC/Expansion1/Data/Expansion1_Civilizations_Major.xml:1100`〜`:1105`）。最初はこの列の無い行にしていて、2026-09-29 の実機（15 km 史実マップ）では1ターン目も2ターン目もアクセスレベルが 0 のままだった（因縁の -10 は付いていたので、特性の修正値そのものは効いている）。そこで、特性で「出会った全文明へのアクセスレベル +1」を最初から与える公式の唯一の例、カトリーヌ・ド・メディシスの `SOURCE_TRAIT` に列をそろえ、`TraitType`=`TRAIT_JAC_1534_ACCESS`、`FromCitizen`=`true` を付けた（`Gossip.xml:78`、特性と修正値は `Base/Assets/Gameplay/Data/Leaders.xml:1088`・`:1698`〜`:1701`・`:4011`〜`:4025`）。修正値の行と引数は活版印刷・カトリーヌと同じ列だけで、主体の条件（`SubjectRequirementSetId`）も持たない（公式も持たない）。カトリーヌの特性は指導者の特性（`LeaderTraits`）で、こちらは文明の特性（`CivilizationTraits`、モンゴルと同じ）なので、実機でアクセスレベルが「限定的」になるかは要確認。開始ターンから効くよう、Lua は全員を出会い済みにした後で `RecheckVisibilityOnAll` を呼ぶ（公式：`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:1336`〜`:1339`）。好感度の数値は変わらない（理由の見える範囲が広がるだけ）。代表団を置く案（`SetHasDelegationAt`）は好感度に +3（`STANDARD_DIPLOMATIC_DELEGATION`、`Base/Assets/Gameplay/Data/Leaders.xml:2756`〜`:2765`）が付いて因縁を薄めるので採らない。FireTuner の `GetDiplomaticModifiers` の `Text` も同じ理由で伏せられるので、シナリオの点数の判定（-10 の行の有無）はそのまま残し、文言とアクセスレベルは `japan-1534-feud-ui.json` が外交画面の表示で確かめる
- 減点は両側にかける（北条→武田と武田→北条）。7組で14個。値は -10（公式の別政体 -6、友好宣言中の非難などの範囲に合わせた）
- 減衰しない：`InitialValue` だけを付け、`ReductionTurns` を付けない。公式の `AGENDA_SAME_GOVERNMENT`（`Base/Assets/Gameplay/Data/Agendas.xml:813`・`:2247`）と同じく、条件（相手がその文明）が成り立つ間は値が変わらない。時間で薄まる `STANDARD_DIPLOMACY_JOINT_WAR`（`Base/Assets/Gameplay/Data/Leaders.xml:3160`〜`:3177`、`ReductionTurns` = 20）とは別の作り。因縁が解消する仕組み（和睦の儀式など）は作らない（MVP）
- 開始時から効く：修正値は特性の付与と同時に有効になるはず（要実機確認）。ただし AI が相手を知らないと表示されないため、Lua が22家すべてを互いに `SetHasMet` で出会い済みにする
- 適用範囲：`Data/JAC_Japan_Feuds.xml` を `UpdateDatabase` で史実の領土の2種にだけ読み込む（開拓者からでは特性・修正値が存在しない）
- 生成：`tools/japan_map_scale/write_feuds.py` が JSON の敵対（`hostility`、high・medium、開始日を含むもの）から生成する。テストが JSON・XML・modinfo の一致を確かめる

公式ファイルの根拠（公式フォルダ内）

1. 好感度の修正値の型と理由の文言：`Base/Assets/Gameplay/Data/Agendas.xml:451`〜`:455`（`AGENDA_ALLY_OF_ENKIDU_FRIEND`）、`:1021`〜`:1050`（`InitialValue`・`ReductionTurns`・`SimpleModifierDescription`）
2. 相手プレイヤーで条件を判定する：`SubjectRequirementSetId`（`Agendas.xml:813`〜`:816`）。アジェンダの特性は `TraitModifiers` で結ぶ（`Agendas.xml:445`〜`:447`）
3. 相手の文明を条件にする：`DLC/GranColombia_Maya/Data/GranColombia_Maya_Civilizations.xml:403`・`:414`・`:427`（`REQUIREMENT_PLAYER_TYPE_MATCHES` + `CivilizationType`）。指導者版は `DLC/RulersOfEngland/Data/RulersOfEngland_Leaders.xml:479`（`REQUIREMENT_PLAYER_LEADER_TYPE_MATCHES`）
4. 外交画面が理由を出す処理：`Base/Assets/UI/DiplomacyActionView.lua:895`〜`:918`（`GetDiplomaticModifiers` の `Score`・`Text`）。シナリオの確認にも同じ関数を使う。`japan-1534-relations.json` の判定は点数だけ（`Text` は見ない）。文言は `japan-1534-feud-ui.json` が外交画面のコントロールから読む。画面の関係欄は `GetDiplomaticModifiers` の結果を `Score`・`Text` の2つのラベルに入れる（`DiplomacyActionView.lua:906`〜`:916`。`Text` が `LOC_TOOLTIP_DIPLOMACY_UNKNOWN_REASON` なら灰色の「理由不明」）。ラベルの ID は `DiplomacyActionView.xml:345`〜`:354`（`IntelRelationshipReasonEntry`）。開く入口は指導者リボンのクリック `DiplomacyRibbon.lua:59`〜`:66` → `LuaEvents.DiplomacyRibbon_OpenDiplomacyActionView` → `DiplomacyActionView.lua:2192`（`OnOpenDiplomacyActionView`）

## 実機確認（メインセッションで実行）

シナリオ：`tools/civ6-autotest/scenarios/japan-1534-relations.json`（15 km・史実の領土、織田が人間、22勢力）、`japan-1534-feud-ui.json`（北条が人間。公式の入口で武田との外交画面を開き、関係欄のラベルを読んで「-10 1534年の因縁」を確かめ、スクリーンショット `hojo-takeda-diplomacy.png` を撮る）と、開拓者から版の `japan-1534-relations-settlers.json`（何も適用されないこと）

1. 開始直後：`JAC_JAPAN_1534_RELATIONS` が開始ターンに `Applied=16`・`Failed=0`・`Missing=0` で残る。22家の全組（22×21）が出会い済みで、アクセスレベルが1以上
2. 大内と大友が相互に交戦中
3. 北条・今川、大内・毛利、伊達・蘆名、足利・六角、島津・琉球が、双方向に `DIPLO_STATE_DECLARED_FRIEND`（`SetHasDeclaredFriendship` が効くかの確認を兼ねる）
4. 敵対7組は交戦も友好宣言もせず、両側の `GetDiplomaticModifiers` に -10 の行がある（点数で判定。文言は `japan-1534-feud-ui.json`）。無関係の織田・伊達と島津・琉球には因縁の行がない。実際の外交画面の「関係」欄に「-10 1534年の因縁」が出ることは `japan-1534-feud-ui.json` が読み取る。同じシナリオは、開く前に北条の武田へのアクセスレベルが1以上であること、関係欄に「理由不明（Unknown Reason）」の行が1つも無いことも確かめる（読み取れなかった場合は、スクリーンショットを目で確認する。「理由不明」が出たら、まずアクセスレベルの画面に「諸家のあいだを行き交う噂」があるかを見て、次に `ModifierStrings` の行を見直す）
5. 1ターン進めても 2〜4 が保たれる
6. セーブ → メニュー → ロード後、プロパティの `Turn` が開始ターンのまま（再適用されていない）で、2〜4 が保たれる

`Failed` が 0 でなければ、Lua.log の `[JAC_1534_RELATIONS] FAILED` 行に関数名とエラーが出る。友好宣言が効かない場合は、同盟・婚姻・主従を未反映に戻すか、永久同盟を使うかをオーナーに確認する。

## オーナー判断（2026-09-29 Discord・セットA）

1. 1a：敵対・不仲は AI の好感度の減点「1534年の因縁」で表す。史実の領土の日本マップだけ、開始時から、減衰しない
2. 2a：同盟・婚姻・主従は友好宣言で近似する（実装済み）
3. 3a：史実の関係は史実の領土の日本マップにだけ適用し、開拓者からの2種には適用しない
4. 4a：島津と琉球を友好（友好宣言）で始める。1534年時点の直接の文献は未確認（`research/japan-1534-relations.md` R19）

## 全勢力の出会いと外交の見え方（オーナー判断 2026-09-29・Discord G）

「日本マップには固定キャラだけが出るので、全員が互いを知っている。最終的に同盟国は視界を共有する」。

1. 出会い：上記のとおり、開始時に22家が互いに出会い済み
2. アクセスレベル：上記の `TRAIT_JAC_1534_ACCESS`（全22家に「限定的」）。関係の理由（「1534年の因縁」を含む）が最初から見える。出所の行は `TraitType` でこの特性に結ぶ（結ばない行では実機でアクセスレベルが 0 のままだった）。仮：推奨案で実装、実機で「限定的」になるかは確認待ち
3. 同盟国の視界共有：ルールセットのパラメータ `GAME_ALLIES_SHARE_VISIBILITY` の既定値は 1（`poc/JAC_PoC_BanseikiRuleset/Config/JAC_Banseiki_Config.xml:16`、`AlliesShareVisibility`）。追加の実装は不要。テストが既定値 1 を確かめる
4. 戦国パックの技術・制度ツリーは公式の技術をすべて消すので、`TECH_PRINTING` に付いた「アクセスレベル +1」（`TechnologyModifiers` の `PRINTING_ADD_DIPLO_VISIBILITY`、`Base/Assets/Gameplay/Data/Technologies.xml:546`〜`:616`・`:670`〜`:672`）と、出所の行 `SOURCE_TECH`（`Base/Assets/Gameplay/Data/Gossip.xml:75`、`PrereqTech=TECH_PRINTING`）が外部キーの連鎖削除で無くなる。`poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Unlocks.sql` が、削除の前に両方を戦国の技術 T05「街道と荷駄」`TECH_JAC_SENGOKU_ROADS_AND_PACKTRAINS` へ付け替える（全地図で有効）。理由：荷駄・使者が街道を通って諸家の噂を運ぶ、情報に最も近い戦国の技術で、交易商人（`UNIT_TRADER`）も解放する。公式でも交易路は別の出所 `SOURCE_TRADER` としてアクセスレベルを上げる。序盤の第I期（列2）なので、日本マップ以外でも早めに理由が見え始める。テストが付け替えを確かめる

## 未検証・注意

- 開拓者から始める2種には史実の関係を適用しない（オーナー判断 3a）。都市がない状態で交戦が始まるのを避けるため
- AI が開始直後に友好宣言を破棄したり和睦したりする頻度は未確認
