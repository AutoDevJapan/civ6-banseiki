# 日本マップ：1534年の関係を開始時の外交状態へ反映する

[SSOT 3.1](../spec/ssot/03-map-and-scale.md#31-日本列島)の「1534年の史実の同盟・婚姻・主従の関係と、敵対・不仲の関係を、開始時の外交状態に反映する」の実装メモ。関係の調査は [`research/japan-1534-relations.md`](../research/japan-1534-relations.md)、データは [`spec/data/japan-1534-relations.json`](../spec/data/japan-1534-relations.json)。

## 実装

- `poc/JAC_PoC_JapanMapScale/Scripts/JAC_Japan_1534Relations.lua` を、日本マップの**史実の領土の2種**（15 km・20 km）の条件でだけ `AddGameplayScripts` に登録した。開拓者からの2種と万世紀ルールセットの他の地図では読み込まれず、関係も因縁の減点も適用されない（オーナー判断 2026-09-29・3a）
- 新規ゲームの `LuaEvents.NewGameInitialized` で1回だけ適用し、結果を Game プロパティ `JAC_JAPAN_1534_RELATIONS`（`Turn`・`Applied`・`Missing`・`Failed`）に残す。プロパティがあれば適用しない
- 味方の8組は、組ごとの Game プロパティ `JAC_1534_ALLIANCE_<勢力A>-<勢力B>`（勢力名を辞書順に並べる。例 `JAC_1534_ALLIANCE_HOJO-IMAGAWA`）に、開始時の同盟を結んだ（結び直した）ターン（数値。続いている印を兼ねる）か、終わったことを示す `false` を残す。セーブに残るので、ロード後も同じ判定になる
- 念のため `Events.LoadScreenClose` でも、プロパティがなく開始ターンのままなら適用する。セーブのロードでは開始ターンを過ぎているか、プロパティが残っているので適用しない
- 勢力はプレイヤー番号ではなく文明名 `CIVILIZATION_JAC_<名前>` で引く。プレイヤー順を入れ替えた構成（武田が人間など）でも同じ組に適用する
- Lua の表は JSON の適用分の写し。`tests/test_japan_1534_relations.py` が一致を確かめる

## Civ6 の外交状態への対応

| 1534年の関係 | Civ6 で開始時に行うこと | 根拠 |
|---|---|---|
| 交戦中 | 双方 `SetHasMet` のあと `DeclareWarOn(相手, WarTypes.FORMAL_WAR, true)` | 公式シナリオと同じ。下記 |
| 同盟・婚姻・主従 | 双方 `SetHasMet` のあと、双方から `SetHasDeclaredFriendship(相手, true)`、続けて片側から `SetHasAllied(相手, true)`（通常の同盟。視界の共有は下記の特性 `TRAIT_JAC_1534_ALLY_VISION` が効かせる。オーナー判断G）。組のプロパティを開始ターンにし、以後は期限なしで保つ（オーナー判断H。下記「開始時の同盟を期限なしで保つ」） | 友好宣言は DLL の Lua バインディングにあり、公式の使用例はない（実機で効くことを確認済み）。同盟は公式シナリオと同じ呼び方（下記 9） |
| 友好（島津・琉球。R19） | 同盟・婚姻・主従と同じ（友好宣言のあと同盟。オーナー判断 4a・H） | 同上 |
| 敵対・不仲（7組） | Lua では何もしない。好感度の減点は下記の修正値 | 下記 |
| 全勢力の出会い | 開始時に1回だけ、22家すべてが互いに `SetHasMet`（両方向）。固定キャラだけが出る日本マップなので、全員が互いを知っている（オーナー判断 2026-09-29・Discord G）。ロード時は再適用しない | `Apply` の先頭。下記 |

- 永久同盟 `SetPermanentAlliance` は採らない。解く関数が無く（下記の根拠 5）、イベントで終えられないため。通常の同盟 `SetHasAllied` を使い、期限 `DIPLOMACY_ALLIANCE_TIME_LIMIT` = 30（`Base/Assets/Gameplay/Data/GlobalParameters.xml:232`）で解けたら Lua が同じターンの初めに結び直す（期限そのものは延ばせない。下記）。友好宣言の期限も30ターン（同 `:234`）だが、同盟を保つので延ばさない
- 主従は Civ6 に対応する外交状態がない。友好宣言で近似し、主従そのものは SSOT 9 の関係宣言（従属国など）が実装されたときに置き換える（TODO）

### 開始時の同盟を期限なしで保つ（オーナー判断H）

1. 組のプロパティ `JAC_1534_ALLIANCE_<A>-<B>` には、同盟を結んだ（結び直した）ターンを数値で持つ（続いている印を兼ねる。終わったら `false`）。毎ターンの初め（`GameEvents.OnGameTurnStarted`）に `KeepStartAlliances` が、プロパティが数値の組を `RELATIONS` の順に見る。全員で同じ順・同じ結果になり、マルチプレイでもゲームプレイの状態として同期する
   - エンジンの同盟の期限は延ばせない（下記「期限切れの実機調査」）。エンジンは、結んだターン + 期限 + 1 ターン目（開始ターン1・期限30なら32ターン目）へ切り替わるときに同盟を解く。解くのは `OnGameTurnStarted` より前（実機 `diplo-16` では、この処理が32ターン目に8組とも切れているのを見つけた）
   - 同盟が切れていたら `SetHasAllied(相手, true)` で結び直し、プロパティを今のターンにして、Lua.log に `[JAC_1534_RELATIONS] re-made after lapse R02 HOJO-IMAGAWA on turn 32 (made on turn 1)` の形で出す。回数を Game プロパティ `JAC_1534_ALLIANCE_LAPSES` に数える（一度も切れていなければ nil）。結び直しはどの家の手番よりも前なので、手番の間に同盟と視界の共有が途切れることは無い。結び直した同盟は、また結んだターン + 期限 + 1 ターン目に切れ、同じように結び直す
   - 結び直したら両家の `RecheckVisibilityOnAll` を呼ぶ。失敗は `FAILED re-make`
   - 残る影響（ターンの切り替わりの中で起きる）：`AllianceEnded`・`DiplomacyRelationshipChanged` のイベントが出る。人間の家の同盟が切れると、その人間に「同盟の期限切れ」の通知 `NOTIFICATION_DIPLO_ALLIANCE_EXPIRED` が出る（翌ターンの終わりまで残る。`Base/Assets/Gameplay/Data/Notifications.xml:230`）。結び直した組は友好宣言が外れる（同盟中は影響しない）。未測定：同盟ポイント・レベルが0からになるか、切り替わりの間に相手の領土の自軍ユニットが押し出されるか、噂（ゴシップ）が出るか。通知などを出さずに済むのは期限そのものを上げる案だけで、オーナーに確認中（`questions.md` Q48）
2. 終わらせる入口（将来のイベントの仕組みが呼ぶ。イベントの仕組みは別途設計）：`ExposedMembers.JAC_Japan1534Relations.EndStartAlliance("HOJO", "IMAGAWA")`。組の順は問わない。組のプロパティを `false` にし、以後は結び直さない。同盟を `SetHasAllied(相手, false)` で解こうとするが、スクリプトから同盟をすぐに解く手段は無かった（実機 `diplo-17`・`end-alliance-01`。下記「すぐに解く方法の調査」）。そのため同盟と視界の共有は、エンジンの次の期限切れ（最長で期限 + 1 ターン後）まで残る。宣戦は挟まない。終わらせたら `true`、続いていなかったら `false` を返す。`IsStartAllianceActive(勢力A, 勢力B)` で続いているかを読める。ゲームプレイのスクリプトから呼ぶ（UI から呼ぶとマルチプレイで同期しない）。`ExposedMembers` の公式の使い方は `Base/Assets/UI/MapTacks.lua:8`〜`:9`、同じ MOD の前例は `Scripts/JAC_Resources.lua:172`
3. 通常の外交での宣戦：同盟中の組は、通常の外交では互いに宣戦できない。`DIPLO_STATE_ALLIED` で選べる外交行動（`Base/Assets/Gameplay/Data/DiplomaticActions.xml:137`〜`:149`）に奇襲戦争も正式な戦争も無い（奇襲戦争は友好的・中立・非友好・非難中の `:168`・`:181`・`:193`・`:206`、正式な戦争は非難中の `:207` だけ）。Civilopedia も「同盟中は互いに正式な戦争を宣言できない」（`Base/Assets/Text/en_US/Civilopedia_Concepts_Text.xml:675`）。期限の切れ目はターンの切り替わりの中だけで、手番の前に結び直すので、味方どうしは（人間が味方の家を担当していても）イベントで同盟が終わるまで戦えない
4. それでも交戦になった場合（別のスクリプトの `DeclareWarOn` など）は、次のターンの初めに `EndStartAlliance` を呼んで終わらせ、結び直さない
5. 同盟が終わると、Civ6 の `LEAVE_ALLIANCE` で友好的（`DIPLO_STATE_FRIENDLY`）に戻る（`DiplomaticActions.xml:271`・`:284`）。以後は通常の外交

### 公式ファイルの根拠

公式フォルダ：`C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI`

1. 新規ゲームの初期化：`DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:11`（`InitializeNewGame`）と `:250`（`LuaEvents.NewGameInitialized.Add`）、`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:1303` と `:1397`。どちらも `AddGameplayScripts` で読み込まれるゲームプレイスクリプト（`DLC/AustraliaScenario/AustraliaScenario.modinfo:70`）
2. 開始ターンの判定：`DLC/PiratesScenario/Scripts/PiratesScenario_StartScript.lua:925`（`Game.GetCurrentGameTurn() == GameConfiguration.GetStartTurn()`）
3. 出会い済みにする：`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:1308`、`Base/Assets/Scenarios/ColdWarScenario/ColdWarScenario_StartScript.lua:35`
4. 開始時の宣戦：`DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:33`、`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:153`（`DeclareWarOn(id, WarTypes.FORMAL_WAR, true)`）
5. 永久同盟（不採用）：`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:1316`〜`:1321`、`DLC/PolandScenario/Scripts/PolandScenario.lua:73`〜`:75`、`DLC/WarMachineScenario/Scripts/WarMachineScenario.lua:87`・`:89`、`DLC/PiratesScenario/Scripts/PiratesScenario_StartScript.lua:384`。どれも `SetPermanentAlliance(相手)` の引数1つで、解く使用例は無い。DLL（`Base/Binaries/Win64Steam/GameCore_Base_FinalRelease.dll`・`DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll`）の Lua バインディングの文字列も `lSetPermanentAlliance` だけで、解く関数は無い。内部の印は `m_isPermanentlyAllied` で通常の同盟の `m_isAllied` と別なので、`SetHasAllied(相手, false)` で永久同盟が解けるかも分からない
6. 友好宣言の状態と期限：`Base/Assets/Gameplay/Data/DiplomaticActions.xml:110`（`DIPLO_STATE_DECLARED_FRIEND`）、`Base/Assets/Gameplay/Data/GlobalParameters.xml:234`（`DIPLOMACY_DECLARED_FRIENDSHIP_TIME_LIMIT` = 30）
7. 状態の読み取り（シナリオの確認に使う）：`Base/Assets/UI/DiplomacyActionView.lua:194`〜`:200`（`DiplomaticStates`）、`:870`（`GetDiplomaticAI():GetDiplomaticStateIndex`）
8. Lua から呼べる外交の設定関数：`DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll` の文字列に `lSetHasMet`・`lDeclareWarOn`・`lSetPermanentAlliance`・`lSetHasDeclaredFriendship`・`lSetHasDelegationAt`・`lSetHasEmbassyAt`・`lMakePeaceWith`・`lNeverMakePeaceWith`・`lPrepareForWarWith` がある。非難（Denounce）と不満（Grievance）の設定関数は、取得用の `lGetDenounceTurn`・`lGetGrievancesAgainst` しかない。行番号のないバイナリなので、実機確認で補う
9. 通常の同盟：`DLC/VikingsScenario/Scripts/VikingScenario.lua:18`〜`:20`（`InitializeNewGame` で `SetHasMet` の後に `GetDiplomacy():SetHasAllied(相手, true)` を片側から呼び、`:27`〜`:29` で `RecheckVisibilityOnAll`）。同じ関数は Gathering Storm の `DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll` の文字列にも `lSetHasAllied`・`lHasAllied`・`lSetAlliesShareVisFlag` としてある。ヴァイキングのシナリオは基本ルールセットなので、Gathering Storm で種類（研究・軍事など）の無い同盟がどう扱われるかは実機で確かめる
10. 同盟国の視界共有の設定：`Base/Assets/Configuration/Data/SetupParameters.xml:48`（`GAME_ALLIES_SHARE_VISIBILITY`、既定 1）。ヴァイキングのシナリオは `Game.GetGameDiplomacy():SetAlliesShareVisFlag(false)` で共有を切る（`VikingScenario.lua:13`）。ただしこれは基本ルールセットの話で、Rise and Fall 以降は同盟の種類とレベルで決まる：視界の共有は軍事同盟のレベル2の効果 `ALLIANCE_SHARE_VISIBILITY`（`DLC/Expansion1/Data/Expansion1_Alliances.xml:158`・`:173`〜`:176`、`MODIFIER_ALLIANCE_SHARE_VISIBILITY` → `EFFECT_ALLIANCE_GRANT_SHARED_VIS`、`DLC/Expansion1/Data/Expansion1_Modifiers.xml:332`〜`:336`）。`GameCore_XP2_FinalRelease.dll` にも `m_bAllAlliancesProvideSharedVis` と `EFFECT_ADJUST_PLAYER_ALL_ALLIANCES_PROVIDE_SHARED_VIS` がある。種類の無い `SetHasAllied` の同盟は共有しない（2026-09-30 の実機：`DIPLO_STATE_ALLIED` の確認は通り、北条から今川の掛川城が見えなかった。`artifacts/diplo-11`）。「チームなし」（`NO_TEAMS`=1）を選ぶと設定が 0 になる（`SetupParameters.xml:199`〜`:200`）
11. 毎ターンの初めの処理：`DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:246`（`GameEvents.OnGameTurnStarted.Add`）
12. 同盟の期限の残り：`DLC/Expansion2/UI/Additions/DiplomacyActionView_AllianceTab.lua:44`（`GetAllianceTurnsUntilExpiration`。UI での使用例。ゲームプレイ側には無い（実機 `diplo-12`）ので使わず、結んだターンから数える）
13. 全同盟で視界を共有する特性：プンドメーカー（クリー）の `TRAIT_ALLIANCE_SHARED_VIS`。`MODIFIER_PLAYER_ADJUST_ALLIANCES_SHARED_VIS` + 引数 `ShareVis`=`true`（`DLC/Expansion1/Data/Expansion1_Leaders.xml:289`〜`:292`・`:1054`〜`:1058`、`Expansion1_Leaders_Major.xml:285`。修正値の型は `Expansion1_Modifiers.xml:453`〜`:456`、`COLLECTION_OWNER`）

### 敵対・不仲：AI の好感度の減点「1534年の因縁」（オーナー判断 1a・2026-09-29）

開始時に Civ6 の「非難」や「不満」を置く API はないため、公式のアジェンダと同じ仕組みで AI の好感度に固定の減点を付ける。外交画面の「関係」欄に「-10 1534年の因縁」の行が出る。

- 仕組み：勢力別の特性 `TRAIT_JAC_1534_FEUD_<勢力>` を、その勢力の文明に `CivilizationTraits` で付け、特性に `MODIFIER_PLAYER_DIPLOMACY_SIMPLE_MODIFIER` の修正値 `JAC_1534_FEUD_<自分>_<相手>` を `TraitModifiers` で結ぶ。修正値の `SubjectRequirementSet`（相手プレイヤーへの条件）は「相手の文明が <相手>」で、`REQUIREMENT_PLAYER_TYPE_MATCHES` + `CivilizationType` を使う。理由の文言は `SimpleModifierDescription` = `LOC_JAC_DIPLO_MODIFIER_1534_FEUD`（「1534年の因縁」）。さらに修正値ごとに `ModifierStrings`（`Context="Sample"`、`Text=LOC_TOOLTIP_SAMPLE_DIPLOMACY_ALL` = `{diplomaticReason}`）の行が要る。公式は全エージェンダ修正値に付けている（`Base/Assets/Gameplay/Data/Agendas.xml:2296`〜`:2370`）。この行が無いと `GetDiplomaticModifiers` の `Text` が `LOC_TOOLTIP_DIPLOMACY_UNKNOWN_REASON`（「理由不明」）になり、外交画面は灰色でそう表示する（`DiplomacyActionView.lua:915`）。2026-09-29 の実機（15 km 史実マップ、1〜2ターン目）で、-10 の行は出たが `Text` が Unknown Reason だった。公式の修正値（今川→北条の -4・9、武田→北条の -4「First impressions of you」）まで Unknown Reason になるのは、データの欠落ではなく公式の仕様：見る側のアクセスレベルが「なし」のあいだは、相手の好感度の理由をすべて伏せる（「Limited（限定的）の外交可視性を得ると、相手のあなたへの評価についての情報が分かる」`Base/Assets/Text/en_US/Civilopedia_Concepts_Text.xml:771`、外交画面の説明「アクセスレベルを上げると、ここに追加の情報が出る」`Base/Assets/Text/en_US/DiplomacyPanel_Text.xml:177`）。出会ったばかりの家はアクセスレベルが「なし」で、戦国パックは公式の技術を消すので活版印刷の +1（`TECH_PRINTING` の `PRINTING_ADD_DIPLO_VISIBILITY`、`Base/Assets/Gameplay/Data/Technologies.xml:546`〜`:616`・`:670`〜`:672`）も得られない（2026-09-29 の実機ログ `DiplomacySummary.csv` も全行 Visibility=0）。そこで、この日本マップでは22家すべてに、活版印刷と同じ修正値 `MODIFIER_PLAYER_ADD_DIPLO_VISIBILITY`（`Amount`=1、`Source`=`SOURCE_JAC_1534_ACCESS`、`SourceType`=`DIPLO_SOURCE_ALL_NAMES`）を、全22文明が持つ特性 `TRAIT_JAC_1534_ACCESS` に付け、出会い済みの全勢力へのアクセスレベルを「限定的」に上げる（オーナー判断 2026-09-29・Discord G。実機で「限定的」になることを確認済み）。文明の特性に付ける前例：モンゴルの `TRAIT_TRADING_POST_DIPLO_VISIBILITY`、`DLC/Expansion1/Data/Expansion1_Civilizations_Major.xml:178`〜`:179`・`:352`〜`:353`・`:620`〜`:629`・`:1100`〜`:1102`。アクセスレベルの画面に出る出所の行は `DiplomaticVisibilitySources` の `SOURCE_JAC_1534_ACCESS`（「諸家のあいだを行き交う噂」）。出所の行には、何によって得るかを示す列が要る。公式の出所の行は必ずどれか1つを持つ（`PrereqTech`・`TraitType`・`GreatPersonIndividualType`・`Trader`/`Delegate`/`Ally`/`Spy`、モンゴルは拡張の表の `TradePostTrait`。`Base/Assets/Gameplay/Data/Gossip.xml:75`〜`:81`、`DLC/Expansion1/Data/Expansion1_Civilizations_Major.xml:1100`〜`:1105`）。最初はこの列の無い行にしていて、2026-09-29 の実機（15 km 史実マップ）では1ターン目も2ターン目もアクセスレベルが 0 のままだった（因縁の -10 は付いていたので、特性の修正値そのものは効いている）。そこで、特性で「出会った全文明へのアクセスレベル +1」を最初から与える公式の唯一の例、カトリーヌ・ド・メディシスの `SOURCE_TRAIT` に列をそろえ、`TraitType`=`TRAIT_JAC_1534_ACCESS`、`FromCitizen`=`true` を付けた（`Gossip.xml:78`、特性と修正値は `Base/Assets/Gameplay/Data/Leaders.xml:1088`・`:1698`〜`:1701`・`:4011`〜`:4025`）。修正値の行と引数は活版印刷・カトリーヌと同じ列だけで、主体の条件（`SubjectRequirementSetId`）も持たない（公式も持たない）。カトリーヌの特性は指導者の特性（`LeaderTraits`）で、こちらは文明の特性（`CivilizationTraits`、モンゴルと同じ）だが、実機でアクセスレベルが「限定的」になることを確認した。開始ターンから効くよう、Lua は全員を出会い済みにした後で `RecheckVisibilityOnAll` を呼ぶ（公式：`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:1336`〜`:1339`）。好感度の数値は変わらない（理由の見える範囲が広がるだけ）。代表団を置く案（`SetHasDelegationAt`）は好感度に +3（`STANDARD_DIPLOMATIC_DELEGATION`、`Base/Assets/Gameplay/Data/Leaders.xml:2756`〜`:2765`）が付いて因縁を薄めるので採らない。FireTuner の `GetDiplomaticModifiers` の `Text` も同じ理由で伏せられるので、シナリオの点数の判定（-10 の行の有無）はそのまま残し、文言とアクセスレベルは `japan-1534-feud-ui.json` が外交画面の表示で確かめる
- 減点は両側にかける（北条→武田と武田→北条）。7組で14個。値は -10（公式の別政体 -6、友好宣言中の非難などの範囲に合わせた）
- 減衰しない：`InitialValue` だけを付け、`ReductionTurns` を付けない。公式の `AGENDA_SAME_GOVERNMENT`（`Base/Assets/Gameplay/Data/Agendas.xml:813`・`:2247`）と同じく、条件（相手がその文明）が成り立つ間は値が変わらない。時間で薄まる `STANDARD_DIPLOMACY_JOINT_WAR`（`Base/Assets/Gameplay/Data/Leaders.xml:3160`〜`:3177`、`ReductionTurns` = 20）とは別の作り。因縁が解消する仕組み（和睦の儀式など）は作らない（MVP）
- 開始時から効く：修正値は特性の付与と同時に有効になる（実機で開始ターンに -10 の行が出ることを確認済み）。ただし AI が相手を知らないと表示されないため、Lua が22家すべてを互いに `SetHasMet` で出会い済みにする
- 適用範囲：`Data/JAC_Japan_Feuds.xml` を `UpdateDatabase` で史実の領土の2種にだけ読み込む（開拓者からでは特性・修正値が存在しない）
- 生成：`tools/japan_map_scale/write_feuds.py` が JSON の敵対（`hostility`、high・medium、開始日を含むもの）から生成する。テストが JSON・XML・modinfo の一致を確かめる

公式ファイルの根拠（公式フォルダ内）

1. 好感度の修正値の型と理由の文言：`Base/Assets/Gameplay/Data/Agendas.xml:451`〜`:455`（`AGENDA_ALLY_OF_ENKIDU_FRIEND`）、`:1021`〜`:1050`（`InitialValue`・`ReductionTurns`・`SimpleModifierDescription`）
2. 相手プレイヤーで条件を判定する：`SubjectRequirementSetId`（`Agendas.xml:813`〜`:816`）。アジェンダの特性は `TraitModifiers` で結ぶ（`Agendas.xml:445`〜`:447`）
3. 相手の文明を条件にする：`DLC/GranColombia_Maya/Data/GranColombia_Maya_Civilizations.xml:403`・`:414`・`:427`（`REQUIREMENT_PLAYER_TYPE_MATCHES` + `CivilizationType`）。指導者版は `DLC/RulersOfEngland/Data/RulersOfEngland_Leaders.xml:479`（`REQUIREMENT_PLAYER_LEADER_TYPE_MATCHES`）
4. 外交画面が理由を出す処理：`Base/Assets/UI/DiplomacyActionView.lua:895`〜`:918`（`GetDiplomaticModifiers` の `Score`・`Text`）。シナリオの確認にも同じ関数を使う。`japan-1534-relations.json` の判定は点数だけ（`Text` は見ない）。文言は `japan-1534-feud-ui.json` が外交画面のコントロールから読む。画面の関係欄は `GetDiplomaticModifiers` の結果を `Score`・`Text` の2つのラベルに入れる（`DiplomacyActionView.lua:906`〜`:916`。`Text` が `LOC_TOOLTIP_DIPLOMACY_UNKNOWN_REASON` なら灰色の「理由不明」）。ラベルの ID は `DiplomacyActionView.xml:345`〜`:354`（`IntelRelationshipReasonEntry`）。開く入口は指導者リボンのクリック `DiplomacyRibbon.lua:59`〜`:66` → `LuaEvents.DiplomacyRibbon_OpenDiplomacyActionView` → `DiplomacyActionView.lua:2192`（`OnOpenDiplomacyActionView`）

## 実機確認（メインセッションで実行）

実機で確認済み：開始時の適用、1ターン後の維持、セーブ → ロード後の維持（再適用なし）、アクセスレベル「限定的」、外交画面の関係欄の「1534年の因縁」の文言。

シナリオ：`tools/civ6-autotest/scenarios/japan-1534-relations.json`（15 km・史実の領土、大内が人間、22勢力。席の並びは `japan-prosperous-15km.json` と同じ。織田が人間の構成では、栄えた開始が無かった実機 `diplo-14` で織田が33ターン中29ターン目に滅び自動進行が止まったため、最大の家の大内にした）、`japan-1534-feud-ui.json`（北条が人間。公式の入口で武田との外交画面を開き、関係欄のラベルを読んで「-10 1534年の因縁」を確かめ、スクリーンショット `hojo-takeda-diplomacy.png` を撮る）と、開拓者から版の `japan-1534-relations-settlers.json`（何も適用されないこと）

1. 開始直後：`JAC_JAPAN_1534_RELATIONS` が開始ターンに `Applied=16`・`Failed=0`・`Missing=0` で残る。22家の全組（22×21）が出会い済みで、アクセスレベルが1以上
2. 大内と大友が相互に交戦中
3. 味方の8組（北条・今川、大内・毛利、伊達・蘆名、足利・六角、朝倉・六角、足利・朝倉、尼子・毛利、島津・琉球）が双方向に `DIPLO_STATE_ALLIED` で、組のプロパティが数値（結んだターン）
   - 視界の共有：味方の各組で、相手の全都市の都市中心のタイルが見えている（UI の `PlayersVisibility[自分]:IsVisible(x, y)`、公式 `Base/Assets/UI/WorldInput.lua:2066`）。さらに相手の全ユニットのいるタイルが自分にも見えている。視界の共有は同盟の相手どうしに限られ、相手の別の味方の視界までは伝わらない（2026-09-30 の実機 `diplo-13` で、毛利に見えている尼子由来のタイルが大内には見えていなかった）ので、「相手に見えている全タイル」は条件にしない。遠い無関係の伊達・島津は、相手の都市中心が1つも見えない
4. 敵対7組は交戦も友好宣言もせず、両側の `GetDiplomaticModifiers` に -10 の行がある（点数で判定。文言は `japan-1534-feud-ui.json`）。無関係の織田・伊達と島津・琉球には因縁の行がない。実際の外交画面の「関係」欄に「-10 1534年の因縁」が出ることは `japan-1534-feud-ui.json` が読み取る。同じシナリオは、開く前に北条の武田へのアクセスレベルが1以上であること、関係欄に「理由不明（Unknown Reason）」の行が1つも無いことも確かめる（読み取れなかった場合は、スクリーンショットを目で確認する。「理由不明」が出たら、まずアクセスレベルの画面に「諸家のあいだを行き交う噂」があるかを見て、次に `ModifierStrings` の行を見直す）
5. 1ターン進めても 2〜4 が保たれる
6. セーブ → メニュー → ロード後、プロパティの `Turn` が開始ターンのまま（再適用されていない）で、2〜4 が保たれる
7. ロード後にさらに30ターン進め、エンジンが開始時の同盟を解くターン（開始ターン + 期限 `DIPLOMACY_ALLIANCE_TIME_LIMIT`（30） + 1 = 32ターン目）の人間の手番で確かめる。8組とも、組のプロパティがちょうどこのターン（このターンの初めに結び直した）で、`JAC_1534_ALLIANCE_LAPSES` が 8。そのうえで双方向に同盟のまま交戦せず、視界を共有している。結び直しが1ターンでも遅れる、手番の後になる、エンジンの切れるターンがずれる、のどれでも落ちる。ロード後も毎ターンの処理が動くことも兼ねて確かめる

`Failed` が 0 でなければ、Lua.log の `[JAC_1534_RELATIONS] FAILED` 行に関数名とエラーが出る。7 が落ちたら、Lua.log の `re-made after lapse`・`FAILED re-make` 行で、結び直しがどのターンに呼ばれたかを見る。同盟（`SetHasAllied`）は実機で `DIPLO_STATE_ALLIED` になった（2026-09-30）。32ターン目の人間の手番で同盟と視界の共有が保たれているかは未実機確認（7 で確かめる）。

視界の調査用シナリオ `probe-ally-vision.json`（15 km・史実の領土、開始時と1ターン後）：北条・今川、島津・琉球と、対照の伊達・島津について、外交状態・`HasAllied`・同盟の種類とレベル・期限の残り・相手の本拠の `IsVisible`／`IsRevealed`／`GetVisibilityCount`・自分の本拠の `IsVisible`（対照）・相手に見えているタイルのうち自分にも見えている数と、`GAME_ALLIES_SHARE_VISIBILITY`・`NO_TEAMS`・特性の有無を、ゲームプレイ（`GameCore_Tuner`）と UI（`InGame`）の両方から読む。読み取りはすべて `pcall` で囲み、失敗しても `ERR(...)` を返して最後まで進む。結果は戻り値と Lua.log の `[JAC_ALLY_VISION_PROBE]` 行に出る。特性でも共有されない場合は、各ターンに相手の見えているタイルを `ChangeVisibilityCount` で写す方式（公式の使用例：`DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:52`〜`:59`）に切り替える。

## 期限切れの実機調査（2026-09-30、`probe-alliance-expiry.json`、`artifacts/diplo-17`）

15 km・史実の領土、大内が人間。Game プロパティ `JAC_1534_KEEP_ALLIANCES_DISABLED`=true で `KeepStartAlliances` を止め、エンジン本来の期限切れを27〜34ターン目に1ターンずつ見た。8組に1つずつ結び直し方を割り当て、操作はゲームプレイ（`GameCore_Tuner`）から行った。結果は Lua.log の `[JAC_ALLIANCE_EXPIRY_PROBE]` 行と steps.jsonl の戻り値。

| 変種 | 組 | 操作 | 操作の直後 | 切れたとき | 期限の数え直し |
|---|---|---|---|---|---|
| V1 対照 | R02 北条・今川 | なし | - | 31→32ターン目の切り替わり | - |
| V2 | R04 伊達・蘆名 | 29ターン目に `SetHasAllied(false)`、別の呼び出しで `(true)` | `false` の後も `HasAllied`=true（何も起きない） | 同上 | されない |
| V2b | R08 尼子・毛利 | 29ターン目に1回の呼び出しで `false` → `true` | 同上 | 同上 | されない |
| V3 | R05 足利・六角 | 29ターン目の手番中に `false`、30ターン目の初めに `true` | 同上 | 同上 | されない |
| V4 | R06 朝倉・六角 | 毎ターン、切れていたら `SetHasAllied(true)` | 32ターン目に切れているのを見て結び直し、33・34ターン目も同盟のまま | 同上 | 結び直しで同盟に戻る |
| V5a | R07 足利・朝倉 | 29ターン目に足利から `false`、朝倉から `true` | `HasAllied`=true のまま | 同上 | されない |
| V5b | R03 大内・毛利 | 29ターン目に公式の同盟の更新 `DIPLOACTION_RENEW_ALLIANCE` を申し込む | 外交行動が DB に無い（`CIVIC_CIVIL_SERVICE` を前提に持つので、戦国パックの社会制度の削除で連鎖して消えたと見られる。`DLC/Expansion2/Data/Expansion1_Alliances.xml:37`）。`IsDiplomaticActionValid`=false で申し込めない | 同上。人間の大内に通知 `NOTIFICATION_DIPLO_ALLIANCE_EXPIRED` | できない |
| V5c | R19 島津・琉球 | 29ターン目に同盟中のまま `SetHasAllied(true)` | `HasAllied`=true | 同上 | されない |

1. 切れるターン：8組とも、開始ターン1に結んだ同盟が31→32ターン目の切り替わりで切れた（ゲームプレイ側では32ターン目、UI 側では31ターン目の番号でイベントが出た）。期限30なので、結んだターン + 期限 + 1 ターン目
2. 出たイベント：各組で `DiplomacyRelationshipChanged`（両方向）→ `AllianceEnded` → `DiplomacyRelationshipChanged`。ゲームプレイと UI の両方に出た。`DiplomacyDealExpired` は出なかった。人間の大内には同盟の期限切れの通知が出た（翌ターンの終わりまで残る）
3. 切れた後：どの組も交戦にはならない。北条・今川、伊達・蘆名、島津・琉球は AI が32ターン目に友好宣言をし直した。V4 の朝倉・六角は結び直した後、友好宣言は外れた
4. 読めなかった値：UI 側の期限の残り（`GetAllianceTurnsUntilExpiration`）と視界（相手の本拠の `IsVisible`）。UI の外交オブジェクトに `HasAllied` が無く、1行の最初の読み取りが `ERR` になった。自動テストの戻り値はエラーの改行で切れるので、その後ろの列が残らなかった。ゲームプレイ側の `GetDeclaredFriendshipTurn` も無い（nil）。期限を延ばす手段が無いことは `HasAllied` と切れたターンだけで判定できるので、読み直しはしない
5. 結論：同盟中の組の期限を延ばす手段は無い。期限で切れたターンの `OnGameTurnStarted` で結び直す V4 を採った（上記「開始時の同盟を期限なしで保つ」）。`diplo-16` では、当時の実装（期限の1ターン前の `false` → `true`）が何も起こさず、32ターン目の `OnGameTurnStarted` で8組とも `re-made after lapse` になっていた

## すぐに解く方法の調査（2026-09-30、オーナー質問K「先にすぐ解く方法を調べる。その後で決めたい。」）

### 公式ファイルと DLL の文字列から分かること

1. 同盟を解く専用の Lua 関数は無い。`GameCore_XP2_FinalRelease.dll`・`GameCore_Base_FinalRelease.dll` の `l` で始まるバインディング名に `EndAlliance`・`BreakAlliance`・`CancelAlliance`・`DissolveAlliance`・`LeaveAlliance` は無く、同盟の設定は `lSetHasAllied`・`lSetPermanentAlliance` だけ。外交の設定関数は `lSetHasMet`・`lSetHasDeclaredFriendship`・`lSetHasDelegationAt`・`lSetHasEmbassyAt`・`lDeclareWarOn`・`lMakePeaceWith`・`lNeverMakePeaceWith`・`lPrepareForWarWith`・`lSetAlliesShareVisFlag`。`SetDiplomaticState` は無く、`lGetDiplomaticState`・`lGetDiplomaticStateIndex` の取得だけ
2. 非難の設定関数も無い（`SetHasDenounced` 等は無く `lGetDenounceTurn`・`lGetDenounceTimeLimit` だけ）。DLL の中の処理のファイル名は `Diplomacy_Action_SetAllied.cpp`・`Diplomacy_Action_SetWarState.cpp`・`Diplomacy_Action_SetDenounced.cpp`・`Diplomacy_Action_SetFriend.cpp` などで、同盟の終わりを別に扱うファイル名は無い。同盟を解くのは `SetAllied`（期限切れと同じ）か、戦争の状態の変更のどちらかと見られる
3. 外交の状態の遷移：同盟の終わりは `LEAVE_ALLIANCE`（前提は同盟中、`DIPLO_STATE_FRIENDLY` へ。`Base/Assets/Gameplay/Data/DiplomaticActions.xml:271`・`:284`）。宣戦の `GO_TO_WAR`（`:261`）には前提の状態が無く、同盟中からでも `DIPLO_STATE_WAR` へ移れる定義。講和 `MAKE_PEACE`（`:262`）は `DIPLO_STATE_UNFRIENDLY` へ。非難 `DENOUNCE`（`:264`）も前提の状態が無い
4. AI が同盟を破る経路は無い：同盟中に選べる外交行動（`DiplomaticActions.xml:137`〜`:149`）に非難・宣戦が無い。同盟の更新 `DIPLOACTION_RENEW_ALLIANCE` は万世紀の DB に無い（`diplo-17`）。不満（Grievance）で同盟が自動で解ける定義は無い
5. `DealManager`・`DiplomacyManager` は公式では UI からだけ使われ、ローカルの人間が申し込む形なので、AI どうしの組には使えない（上記「根拠」の 3）
6. `SetPermanentAlliance` は内部の印 `m_isPermanentlyAllied` を立てるだけで、解く関数は無い（上記の根拠 5）

### 調査用シナリオ `probe-end-alliance.json`

15 km・史実の領土、大内が人間（`japan-1534-relations.json` と同じ構成）。`JAC_1534_KEEP_ALLIANCES_DISABLED`=true で結び直しを止め、ターン2〜5を1ターンずつ見る。操作はゲームプレイ（`GameCore_Tuner`）から、1組に1つの変種。戦争を使う変種は、ほかの組と家が重ならない組（北条・今川、伊達・蘆名、島津・琉球）に置いた。講和は宣戦の次のターンだけ（同じターンに講和しない安全の判定つき）。各ターンと操作の直後に、両方向の `HasAllied`・`IsAtWarWith`・`HasDeclaredFriendship`・`CanDeclareWarOn`・外交の状態（`GetDiplomaticAI():GetDiplomaticStateIndex`）をゲームプレイで、外交の状態・同盟の種類とレベルと期限の残り・相手の本拠が見えるか（`PlayersVisibility`）・非難と宣戦の外交行動が有効か（`IsDiplomaticActionValid`）を UI（`InGame`）で読む。`AllianceEnded`・`DiplomacyRelationshipChanged`・`DiplomacyDeclareWar`・`DiplomacyMakePeace` も記録する。結果は Lua.log の `[JAC_END_ALLIANCE_PROBE]` 行と戻り値。エラーは1行にまとめて返す（`diplo-17` で戻り値が改行で切れた対策）

| 変種 | 組 | 操作 | 期待 |
|---|---|---|---|
| E1 | R02 北条・今川 | ターン2に北条から `DeclareWarOn(今川, FORMAL_WAR, true)`、ターン3に `MakePeaceWith(今川, true)` | 宣戦で同盟が解ければ、講和で非友好の平和になり、以後は結び直さない |
| E2 | R04 伊達・蘆名 | ターン2に `DeclareWarOn(蘆名, SURPRISE_WAR, true)`、講和しない | 戦争の種類で違うか、戦争中のターンの切り替わりで同盟が戻らないか |
| E3 | R19 島津・琉球 | ターン2に強制の引数なしで `DeclareWarOn(琉球, FORMAL_WAR)`、戦争になっていればターン3に講和 | 同盟中は拒まれる見込み（`CanDeclareWarOn` も読む） |
| E4 | R05 足利・六角 | ターン2に双方の友好宣言を外し、続けて `SetHasAllied(六角, false)` | 友好宣言が前提で拒まれているなら解ける |
| E5 | R07 足利・朝倉 | ターン2に朝倉から `SetHasAllied(足利, false)`、続けて足利から | 結んでいない側からの `false` は未確認 |
| E6 | R08 尼子・毛利 | ターン2に `SetPermanentAlliance(毛利)`、続けて `SetHasAllied(毛利, false)` | 永久同盟の意味を見る。解けない見込み |
| E7 | R03 大内・毛利 | ターン3の `OnGameTurnStarted` の中で毛利から `SetHasAllied(大内, false)` | ターンの処理の中なら受け付けるか |
| E0 | R06 朝倉・六角 | なし（対照） | 同盟のまま |

### 実機の結果（2026-09-30、`artifacts/end-alliance-01`）

判定は 8/8 で通り、エンジンの停止も Lua のエラーも無かった（ゲームプレイ側の外交の状態の読み取り `GetDiplomaticAI()` だけは、ゲームプレイの Player に無く `ERR` になった。外交の状態は UI 側の値で見る）。`HasAllied` はゲームプレイ側、外交の状態・期限の残り・本拠の見え方は UI 側の値。ターンは開始からの通し番号（ターン1が開始）。

| 変種 | 同盟は終わったか | 戦争 | 講和後 | 視界の共有（相手の本拠が見えるか） | 出たイベントと副作用 |
|---|---|---|---|---|---|
| E0 対照 R06 | 終わらない（ターン5まで `ALLIED`） | なし | - | 保たれる | なし |
| E1 R02 強制の正式な戦争→翌ターン講和 | **終わらない**。ターン2〜5の間 `HasAllied`=true/true のまま、期限の残りも 29→26 と数え続けた。`AllianceEnded` は出ない | ターン2に交戦（`DiplomacyDeclareWar`、状態 `DIPLO_STATE_WAR`） | ターン3に講和（`DiplomacyMakePeace`）。状態は非友好→ターン4 中立→ターン5 友好と毎ターン上がり、ターン5まで平和のまま。非難が有効になり、宣戦は「友好か同盟の相手」で無効のまま | 戦争中（ターン3の初め）は双方見えない。講和後のターン4から双方また見える（同盟の視界の共有が戻ったと見られる） | 各ターン `DiplomacyRelationshipChanged`。好戦度・不満は未測定 |
| E2 R04 強制の奇襲戦争（講和しない） | **終わらない**。交戦中も `HasAllied`=true/true、期限も数え続けた | ターン2〜5の間ずっと交戦（同盟のまま戦争） | - | ターン3から伊達→蘆名は見え、蘆名→伊達は見えない | `DiplomacyDeclareWar`。好戦度・不満は未測定 |
| E3 R19 強制なしの正式な戦争 | 終わらない | 起きない（エラーも無し） | 講和は省略 | 保たれる | なし |
| E4 R05 友好宣言を外す→`SetHasAllied(false)` | 終わらない | なし | - | 保たれる | 友好宣言は双方外れた（`DiplomacyRelationshipChanged` が false で出た）。同盟は残る |
| E5 R07 両家から `SetHasAllied(false)` | 終わらない | なし | - | 保たれる | なし |
| E6 R08 `SetPermanentAlliance`→`SetHasAllied(false)` | 終わらない | なし | - | 保たれる | `DiplomacyRelationshipChanged` が数回。永久同盟になり、解く関数が無いので使わない |
| E7 R03 `OnGameTurnStarted` の中で `SetHasAllied(false)` | 終わらない | なし | - | 保たれる | なし |

結論：スクリプトから同盟をすぐに解く手段は無い。強制の宣戦は戦争にはなるが、同盟（`HasAllied`・期限）はそのまま残り、講和すると視界の共有も戻る。戦争を挟む E1 は採らず、`EndStartAlliance` は宣戦しない形（組を結び直さなくなり、次の期限切れで解ける）に戻した。確認項目：E1 の講和後、ターン6以降に外交の状態が同盟へ戻るかは未測定。

模擬実行：`tests/test_japan_1534_relations_lua.py`（`uv run --no-project --with lupa python -m unittest tests.test_japan_1534_relations_lua`）。終えた組は宣戦も講和もせず、期限切れの後も結び直さない（ほかの組は結び直す）、別の理由で交戦中の組は終わりとして扱い宣戦も講和もしない、を確かめる

## オーナー判断（2026-09-29 Discord・セットA）

1. 1a：敵対・不仲は AI の好感度の減点「1534年の因縁」で表す。史実の領土の日本マップだけ、開始時から、減衰しない
2. 2a：同盟・婚姻・主従は友好宣言で近似する（実装済み）。オーナー判断G（視界の共有）を受けて、さらに通常の同盟にした
3. 3a：史実の関係は史実の領土の日本マップにだけ適用し、開拓者からの2種には適用しない
4. 4a：島津と琉球を友好（友好宣言）で始める。1534年時点の直接の文献は未確認（`research/japan-1534-relations.md` R19）。オーナー判断Hで同盟にもした

## オーナー判断（2026-09-30 Discord・確認事項H）

「bで島津と琉球も。」

1. 開始時の同盟（史実の同盟・婚姻・主従）には期限を設けず、裏切りなどのイベントでだけ終える（選択肢②）。実装は上記「開始時の同盟を期限なしで保つ」
2. 友好の島津と琉球王国も同じく同盟にし、視界を共有する（選択肢③）。味方は8組になる

## オーナー判断（2026-09-30 Discord・確認事項I-1）

「基本的に全都市が軍事同盟を持っている状態から始めればいいんじゃない？そしたら、わざわざ新規実装する必要ないでしょ。」

1. 意図：味方の8組は、Civ6 の標準の軍事同盟（視界の共有はレベル2の効果、`DLC/Expansion2/Data/Expansion1_Alliances.xml:158`・`:173`〜`:176`）を結んだ状態から始める
2. 調べた結果、ゲームプレイのスクリプトから同盟の種類とレベルは設定できない（下記の根拠）。そこで標準の軍事同盟の代わりに、これまでの「通常の同盟 + 特性 `TRAIT_JAC_1534_ALLY_VISION`（全同盟で視界を共有）」で同じ効果（開始時からの視界の共有）を出す。特性がすでに実機で視界を共有できている（上記の実機）ので、新規の実装はしない
3. 特性の副作用（12家が後で結ぶ同盟も視界を共有する）は、この近似の一部として受け入れる（旧 `questions.md` Q30 は削除）

根拠（2026-09-30 調査。公式フォルダ内）

1. DLL の Lua バインディング名（`DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll`・`DLC/Expansion1/Binaries/Win64/GameCore_XP1_FinalRelease.dll`・`Base/Binaries/Win64Steam/GameCore_Base_FinalRelease.dll` の文字列）：同盟の設定は `lSetHasAllied`・`lSetPermanentAlliance` だけ。種類とレベルは取得用の `lGetAllianceType`・`lGetAllianceLevel`・`lGetAlliancePointsPerTurn`・`lGetAllianceTurnsThisLevel`・`lGetAllianceTurnsToNextLevel`・`lGetAllianceTurnsUntilExpiration`・`lGetAllianceBenefitsString`・`lGetAlliancePointsTooltip` しかなく、`SetAllianceType`・`SetAllianceLevel`・`ChangeAlliancePoints`・`AddAlliancePoints` などの設定関数は無い。`l(Set|Change|Add|Grant|Make|Declare|Force|Propose|Do)*` の全バインディングを見ても、同盟の種類・レベル・ポイントを変えるものは無い
2. 公式の `SetHasAllied` の呼び出しは引数2つだけ（`DLC/VikingsScenario/Scripts/VikingScenario.lua:18`〜`:20`）。種類を渡す使用例は無い。`SetPermanentAlliance` も引数1つ（根拠 5）
3. 種類つきの同盟を結ぶ公式の経路は、UI の取引だけ：`DealManager.GetWorkingDeal` → `AddItemOfType(DealItemTypes.AGREEMENTS)` → `SetSubType(DealAgreementTypes.ALLIANCE)` → `SetValueType(DB.MakeHash("ALLIANCE_MILITARY"))` → `DiplomacyManager.RequestSession(自分, 相手, "MAKE_DEAL")`（`DLC/Expansion2/UI/Replacements/DiplomacyActionView_Expansion1.lua:208`〜`:223`）。`DealManager`・`DiplomacyManager` は公式では UI のファイルでしか使われず（`Base/Assets/UI/`・`DLC/Expansion*/UI/` だけ）、ローカルの人間プレイヤーが相手に申し込み、AI が受けるかを判断する。AI どうしの組を開始時に決まった形で結ぶ手段にはならない
4. データだけの経路：同盟ポイントの修正値 `MODIFIER_PLAYER_ADJUST_ALLIANCE_POINTS`（`EFFECT_ADJUST_ALLIANCE_POINTS_FOR_MODIFIER`、`DLC/Expansion2/Data/Expansion1_Modifiers.xml:388`〜`:390`。公式は政策 `ARSENALOFDEMOCRACY_ALLIANCEPOINTS`、`DLC/Expansion2/Data/Expansion1_Policies.xml:110`〜`:111`）は毎ターンのポイントを足すだけで、レベル2（`ALLIANCE_LEVEL_TWO_XP`=320、`DLC/Expansion2/Data/Expansion2_GlobalParameters.xml:12`）に開始時から届かない。また同盟の効果は種類ごと（`AllianceEffects` の `AllianceType`）なので、種類が軍事でない限りレベルを上げても視界の共有は付かない。`Alliances` の行の順を入れ替えて軍事を先頭（既定の種類 0）にする案は、全同盟の種類の対応が変わり、`AllianceEffects`・`DiplomaticActions_XP1` の外部キーの連鎖削除も伴うので採らない
5. 確認項目（静的な推定。`probe-alliance-type.json` で確かめる）：戦国パックは公式の社会制度を全削除する（`poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Unlocks.sql:148`）。同盟の外交行動（`DIPLOACTION_ALLIANCE`・`DIPLOACTION_ALLIANCE_MILITARY` など）は `InitiatorPrereqCivic="CIVIC_CIVIL_SERVICE"` を持ち（`Base/Assets/Gameplay/Data/DiplomaticActions.xml:69`、`DLC/Expansion2/Data/Expansion1_Alliances.xml:32`〜`:36`）、外部キーは `ON DELETE CASCADE`（`Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:923`〜`:924`）なので、万世紀ルールセットでは通常の外交で同盟そのものを結べない可能性が高い（同じ前提の `DIPLOACTION_RENEW_ALLIANCE` が DB に無いことは実機 `diplo-17` で確認）。そうなら上記の特性の副作用（後で結ぶ同盟の視界共有）も今は起きない。通常の同盟を戦国の制度（例 `CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES`「誓約と同盟」）で解放するかは、戦国パックの制度ツリーの設計で決める

種類の設定の調査用シナリオ `probe-alliance-type.json`（15 km・史実の領土、開始時と1ターン後）：ゲームプレイ（`GameCore_Tuner`）から、関係の無い伊達→島津に `SetHasAllied(相手, true, 軍事同盟の Index)`、長尾→毛利に `SetHasAllied(相手, true, DB.MakeHash("ALLIANCE_MILITARY"))` を呼び、UI（`InGame`）で北条→今川（対照）と両組の `GetAllianceType`（種類名）・`GetAllianceLevel`・期限の残り・相手の本拠が見えるかを読む。あわせて、ゲームプレイで `DealManager`・`DiplomacyManager` が使えるか、`GameInfo.Alliances` の先頭行と軍事同盟の Index、`DIPLOACTION_ALLIANCE`・`DIPLOACTION_ALLIANCE_MILITARY` が DB に残っているかを返す。読み取りはすべて `pcall` で囲み、結果は戻り値と Lua.log の `[JAC_ALLIANCE_TYPE_PROBE]` 行に出る。3つ目の引数で種類が軍事になった場合だけ、標準の軍事同盟へ切り替えを検討する（レベル2は別途必要）

## 全勢力の出会いと外交の見え方（オーナー判断 2026-09-29・Discord G）

「日本マップには固定キャラだけが出るので、全員が互いを知っている。最終的に同盟国は視界を共有する」。

1. 出会い：上記のとおり、開始時に22家が互いに出会い済み
2. アクセスレベル：上記の `TRAIT_JAC_1534_ACCESS`（全22家に「限定的」）。関係の理由（「1534年の因縁」を含む）が最初から見える。出所の行は `TraitType` でこの特性に結ぶ（結ばない行では実機でアクセスレベルが 0 のままだった）。実機で「限定的」になり、外交画面の関係欄に「-10 1534年の因縁」が出ることを確認済み
3. 味方の視界共有（オーナー判断G「そのうち、味方の視界が共有されてる感じで」）：史実の同盟・婚姻・主従の7組と島津・琉球（オーナー判断H）を開始時に通常の同盟（`SetHasAllied`）にする。Rise and Fall 以降、種類の無い同盟だけでは視界を共有しない（上記の根拠 10）。そこで味方の組の12家（北条・今川・大内・毛利・伊達・蘆名・足利・六角・朝倉・尼子・島津・琉球）に、プンドメーカーと同じ「全同盟で視界を共有する」修正値 `JAC_1534_ALLY_SHARED_VIS`（`MODIFIER_PLAYER_ADJUST_ALLIANCES_SHARED_VIS`、`ShareVis`=`true`、根拠 13）を特性 `TRAIT_JAC_1534_ALLY_VISION` で付ける（`Data/JAC_Japan_Feuds.xml`、`write_feuds.py` が JSON の味方の組から生成）。ルールセットのパラメータ `GAME_ALLIES_SHARE_VISIBILITY` の既定値も 1 のまま（`poc/JAC_PoC_BanseikiRuleset/Config/JAC_Banseiki_Config.xml:16`）。テストが既定値 1、特性を持つ12家、Lua が味方の組だけを同盟にすることを確かめる
   - 副作用：修正値は「その家の全同盟」に効くので、12家が後で結ぶ通常の同盟も、レベルに関係なく視界を共有する。修正値は外せないので、開始時の同盟が終わった後も残る。オーナー判断I（下記）で、軍事同盟から始める扱いの近似として受け入れた
   - 同盟にした理由：視界の共有をエンジンに任せられる。ユニットが動いても即時に共有され、セーブにも同盟の状態として残り、マルチプレイでもゲームプレイの状態として同期する。各ターンにタイルの見え方を Lua で写す方式は、ターン中の移動が反映されず、写した分をセーブ・ロードをまたいで正しく取り消す必要がある
   - 副作用：同盟国どうしは外交のアクセスレベルが1段上がる（公式の出所 `Ally`。全員の「限定的」に加えて「開放」になる）、互いに正式な宣戦ができない、同盟ポイントが貯まる
   - 終わり方（オーナー判断H）：期限なしで続け、イベントでだけ終える。上記「開始時の同盟を期限なしで保つ」
   - 実機（2026-09-30 `probe-ally-vision.json`、`artifacts/diplo-12`）：`SetHasAllied` の同盟は、UI（`InGame`）で `GetAllianceType`=0（`GameInfo.Alliances` の先頭行 `ALLIANCE_RESEARCH`。種類を選んで結んだのではなく既定値と見られる）、`GetAllianceLevel`=1、`GetAllianceTurnsUntilExpiration`=30（1ターン後 29）。特性で相手の本拠が見え、相手に見えているタイル 203/203 が自分にも見えた。ゲームプレイ（`GameCore_Tuner`）の外交オブジェクトには `GetAllianceType`・`GetAllianceLevel`・`GetAllianceTurnsUntilExpiration` が無い（nil）。期限の切れ目で結び直す今の方式は、上記「開始時の同盟を期限なしで保つ」と下記「期限切れの実機調査」。確認は `japan-1534-relations.json` の 7
4. 戦国パックの技術・制度ツリーは公式の技術をすべて消すので、`TECH_PRINTING` に付いた「アクセスレベル +1」（`TechnologyModifiers` の `PRINTING_ADD_DIPLO_VISIBILITY`、`Base/Assets/Gameplay/Data/Technologies.xml:546`〜`:616`・`:670`〜`:672`）と、出所の行 `SOURCE_TECH`（`Base/Assets/Gameplay/Data/Gossip.xml:75`、`PrereqTech=TECH_PRINTING`）が外部キーの連鎖削除で無くなる。`poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Unlocks.sql` が、削除の前に両方を戦国の技術 T05「街道と荷駄」`TECH_JAC_SENGOKU_ROADS_AND_PACKTRAINS` へ付け替える（全地図で有効）。理由：荷駄・使者が街道を通って諸家の噂を運ぶ、情報に最も近い戦国の技術で、交易商人（`UNIT_TRADER`）も解放する。公式でも交易路は別の出所 `SOURCE_TRADER` としてアクセスレベルを上げる。序盤の第I期（列2）なので、日本マップ以外でも早めに理由が見え始める。テストが付け替えを確かめる

## 未検証・注意

- 開拓者から始める2種には史実の関係を適用しない（オーナー判断 3a）。都市がない状態で交戦が始まるのを避けるため
- AI が開始直後に友好宣言を破棄したり和睦したりする頻度は未確認
- 確認項目：マルチプレイでの同期（全員の出会い・友好宣言・同盟と視界の共有・同盟の結び直し・交戦・因縁・アクセスレベルが全員で一致するか）は未検証
- 確認項目：ゲーム設定で「チームなし」を選ぶと同盟国の視界共有が切れる（`SetupParameters.xml:199`〜`:200`）
- 期限の延ばし方：`GetAllianceTurnsUntilExpiration` はゲームプレイのスクリプトには無い（上記の実機）ので、結んだターンを組のプロパティに持ち、期限の1ターン前に解いて結び直す。要実機確認：結び直した同盟で期限が数え直され、期限切れ（`re-made after lapse`）が一度も起きないか（`japan-1534-relations.json` の 7）
- 確認項目：結び直すたび（30ターンに1回）に、同盟の噂（`LOC_GOSSIP_ALLIED`）や通知が出るかもしれない。多すぎる場合は、結び直しを期限切れのときだけにするかを検討する
- 確認項目：人間が味方の家（北条など）を担当すると、イベントで同盟が終わるまで味方の家に宣戦できない。オーナー判断Hどおりだが、イベントの仕組みができるまでは終える手段がない
- 確認項目：組のプロパティを書かない、または `true` で書いた古いビルドのセーブでは、開始時の同盟を保たない（通常の同盟として30ターンで切れる）。影響はリリース前のセーブだけなので対応しない
- 確認項目：プロパティ `JAC_JAPAN_1534_RELATIONS` を書かない古いビルドで作った開始ターンのセーブをロードすると、`LoadScreenClose` の判定で再適用されうる。影響はリリース前のセーブだけなので対応しない
