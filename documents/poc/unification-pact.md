# 天下統一の合算（チームと天下統一同盟）PoC

実行コマンドは Rust 版に更新している。移行前の検証結果は旧実行器での記録として残す。

SSOT 3.1 の表「勝利」の「ゲーム設定のチーム、1534年の開始時の同盟、『天下統一同盟』では城（登録城のない地図では本拠）を合算し、チームではチーム全員が、開始時の同盟と天下統一同盟では主体となる勢力だけが勝利する」と、その下の「天下統一の合算の決まり」を対象とする。天下統一の本体は [天下統一 PoC](unification-victory.md)。単独プレイの主な流れは実機で確認済み（下の「実機確認」）。

## 決めた読み方（オーナー判断 O-5〜O-8、2026-09-30。SSOT 3.1）

1. ゲーム設定のチームは、勝利の要件がチーム単位で評価されるので、追加の実装なしで城を合算する（下の「チームの合算」）。Civ6 の勝者がチーム単位のため、チーム全員が勝つ（O-6）
2. 1534年の開始時の同盟（味方8組）は、組ごとに、続いている間（Game プロパティ `JAC_1534_ALLIANCE_<A>-<B>` が数値の間。[1534年の関係](japan-1534-relations.md)）だけ合算する。主体は、数える城の多い方（同数ならプレイヤー ID の小さい方）。組どうしはつなげない（六角・足利・朝倉のように3家が互いに組んでいても、組ごとに判定する）。イベントや交戦で同盟が終わる（プロパティが `false`）と合算も終わる。両家が同じ天下統一同盟に入っていれば、天下統一同盟の主体で判定する（2つの印が別々の首都に付かないように）（O-5）
3. 天下統一同盟は、ゲーム中に申し込んで組む。申し込んだ家が主体。人間の家には相手の受諾が要る。AI の家は、申し込んだ家と同盟（Civ6 の同盟）を結んでいれば、その場で加わる（公式の同盟が AI の同意の代わり）。1つの同盟に主体は1人で、加わる家の数に上限はない。1つの家が入れる同盟は1つ。同じチームどうしでは組めない（チームで合算済み）
4. 抜けること（O-7）：どの家も画面の「同盟から抜ける」でその場で抜けられる（`EXECUTE_SCRIPT` で全端末が同じ処理をする）。同じ同盟の家どうしが戦争になると、次の判定（ターンの初め、または都市の変化。ゲーム側に宣戦のイベントが無い）で、主体でない側が抜ける。主体でない家どうしが戦えば両方が抜ける。1家だけになった同盟は解ける（2家の同盟は、どちらかが抜けるか戦争になれば解ける）
5. 主体が滅亡するか抜けても同盟は続き、残った参加者のうち数える城を最も多く持つ家（同数なら小さいプレイヤー ID）が新しい主体になる（全端末で同じ結果。ログに残し、その場で勝利を判定し直す）。滅亡した家は同盟から外れる（都市を持たないので、その家が失った城は取った家の城として数える）
6. AI からの申し込み（O-7、暫定の決まった規則）：毎ターンの初めに、同盟に入っていない AI の家が、プレイヤー ID の小さい順に、Civ6 の同盟を結んでいる相手のうち同盟に入っておらず、受けられる申し込みが届いていない最初の家に申し込む。条件は、2家で数える城（または本拠）の30%以上を持つこと（`AI_SHARE` = 30。実機の約200ターンの進行で調整する）。AI の相手はその場で加わり、人間の相手には申し込みの画面が自動で開く（同じ申し込みでは開き直さない）
7. 登録城のない地図（開拓者から始める版など。O-8）：数える登録城が1つも無いとき（すべて破却された場合を含む）は、勝利の本拠の条件と同じく、全勢力の本拠（最初の首都）を合算する。本拠は、各主要勢力が最初に首都を持ったときの位置をゲーム側 Lua が Game プロパティ `JAC_UNIFICATION_HOME_CAPITAL_<ID>` に記録する（`City:IsOriginalCapital` は公式に UI 側の使用例しか無い：`Base/Assets/UI/PartialScreens/WorldRankings.lua:1844`）。生きている主要勢力がすべて本拠を持つまでは判定しない。滅亡した家の本拠も数える

## 仕組み

1. 画面：`poc/JAC_PoC_SengokuTrees/UI/JAC_UnificationPact.xml`・`.lua`（InGame に追加する画面）。画面上部中央の「天下統一同盟」ボタンで一覧を開く。出会った他チームの主要勢力ごとに「申し込む」「申し込みを受ける」、同盟に入っている家には「◯◯の同盟」を出す。状態と直前の結果（申し込んだ・成立・AI と同盟していない・組めない・抜けた・戦争で抜けた・解けた）を上に出し、同盟に入っていれば「同盟から抜ける」を出す。自分への申し込みが新しく届くと画面が自動で開く。画面は Game と Player のプロパティを読むだけで、変更は要求だけを送る
2. 要求：`UI.RequestPlayerOperation(自分, PlayerOperations.EXECUTE_SCRIPT, {OnStart = "JAC_UnificationPactPropose"・"JAC_UnificationPactAccept"・"JAC_UnificationPactLeave", Target = 相手})`。エンジンがすべての端末で同名の GameEvent を要求者のプレイヤー ID つきで起こす。要求者は引数でなくエンジンが渡す ID を使う
3. 状態：ゲーム側 `Scripts/JAC_Sengoku_UnificationPact.lua` だけが変える。`Game` のプロパティ `JAC_UNIFICATION_PACT_LEAD_<ID>`（主体の ID。主体自身にも付ける）と `JAC_UNIFICATION_PACT_PROPOSAL_<相手>`（申し込んだ家。加わると消す）、`JAC_UNIFICATION_HOME_CAPITAL_<ID>`（本拠の位置）、変化の結果は関わった家の Player プロパティ `JAC_UNIFICATION_PACT_RESULT`（`PROPOSED`・`JOINED`・`NOT_ALLIED`・`INVALID`・`LEFT`・`AT_WAR`・`DISSOLVED`）。どれもセーブに含まれる
4. 勝利：ゲーム側 Lua が、都市の建設・移管（`GameEvents.CityBuilt`）、占領（`GameEvents.CityConquered`）、毎ターンの開始、同盟の成立・脱退のたびに、本拠の記録、主体の引き継ぎ、交戦した家の脱退、1家の同盟の解消をしたうえで、各同盟と各開始時の同盟の組のチームが数える目標（数える登録城。無ければ全勢力の本拠）をすべて持つかを確かめる。持っていれば、主体の首都に隠しの印の建物 `BUILDING_JAC_PACT_UNIFIED`（`InternalOnly`、都市の中心）を付ける。勝利要件の3つ目の枝（`JAC_UNIFICATION_PACT`）は「そのチームがこの印のある都市を1つ以上持つ」なので、主体のチームだけが勝つ。同盟の相手のチームは印の都市を持たないので勝たない。この枝は地図の種類によらないので、登録城のない地図でも同じ印で勝つ
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

- 静的検査（`cargo test --locked --manifest-path tools/Cargo.toml --workspace --all-targets`）：`tools/civ6-data-rs/src/data_tests.rs` が要件の行を模擬評価し、印のある都市を持つチームだけが勝つ（相手のチームと部外者は勝たない）、印がなければ同じ城の分け方でも誰も勝たない、を確かめる。modinfo の登録、画面が状態を変えないこと、文言の3言語もここで見る
- Lua（mlua と Lua 5.1）：`tools/civ6-lua-tests-rs/tests/unification.rs` が `JAC_Sengoku_RebuiltCastles.lua` と一緒にゲーム側 Lua を動かし、同盟していない AI は加わらない、同盟中の AI は加わる、部外者が城を1つ持つ間は印を付けない、相手に城が移ると主体の首都にだけ印を付ける、人間は受諾が要る（申し込んでいない家や別の家の受諾は無効）、1つの家は1つの同盟だけ、同じチームは組めない、破却後の再建都市は妨げない、主体が滅亡すると城の最も多い参加者が主体を継いで勝利を判定し直す、同数なら小さい ID の家が継ぎ新しい参加者も迎えられる、参加者が残らなければ同盟は消える、同盟の相手が滅亡しても残りの家で全城を持てば主体の首都に印を付ける、滅亡した相手の城を部外者が取れば印を付けない（滅亡した家には申し込めない）、を確かめる。O-5〜O-8 の分として、1家だけになった同盟は解ける、参加者がその場で抜け（受けた申し込みは使い切り）、主体が抜けると城の多い参加者が継ぐ、交戦した参加者が抜け主体は残る（主体でない家どうしなら両方）、2家の同盟は交戦で解け占領の判定より先に抜ける、AI が同盟相手の AI に30%以上で申し込み加える、人間への申し込みは受諾を待ち繰り返さない、30%未満や同盟なしでは申し込まない、開始時の同盟は続く間だけ合算し城の多い方（同数なら小さい ID）が主体、両家が同じ天下統一同盟なら天下統一同盟の主体で判定、登録城のない地図で本拠を合算する（滅亡した家の本拠も数え、全勢力が本拠を持つまで判定しない）、を確かめる
- 実機シナリオ `tools/civ6-autotest/scenarios/unification-pact.json`（`cargo run --locked --manifest-path tools/Cargo.toml --bin generate-unification-victory --` で生成）。検証地図（[天下統一 PoC](unification-victory.md) と同じ `JAC_Test_UnificationMap`）で織田（人間）・伊達・蘆名：
  1. 登録城4、織田2・伊達2で勝者なし
  2. 織田と蘆名（登録城なし）を同盟にし、同盟の画面から蘆名へ申し込む → 成立（2城なので印なし）
  3. 画面から抜ける → 織田は「抜けた」、蘆名は「解けた」、どちらも同盟なし
  4. もう一度蘆名へ申し込む → 成立。ゲーム側から織田が蘆名へ宣戦する（`DeclareWarOn(相手, FORMAL_WAR, true)`。同盟中の相手への強制の宣戦は実機で未確認で、効かなければこの手順で止まる）
  5. 1ターン進める → ターンの初めに蘆名は「戦争で抜けた」、織田は「解けた」。同じ手順の中で `MakePeaceWith(蘆名, true)` で講和させる（宣戦の次のターンに講和させる形。講和と同じターンの宣戦はエンジンが止まった：court-arbitration-08）。人間の手番が始まるまで待つ（手番の前の `EXECUTE_SCRIPT` は届かない：court-arbitration-03）
  6. 同盟の画面から伊達へ申し込む → 結果が「AI と同盟していない」、同盟は成立しない
  7. 伊達の本拠でない城を蘆名へ移し、織田と伊達を同盟にする（1534年の関係と同じく友好宣言のあと `SetHasAllied`）
  8. もう一度申し込む → 成立（伊達の主体が織田）。同盟は4城のうち3城なので、印も勝者もない
  9. セーブして終了し、ロードする → 同盟の状態（主体と結果）が残り、印も勝者もない
  10. 画面を開いて `unification-pact-panel.png` を撮る
  11. 蘆名の城を伊達（同盟の相手）へ移す（蘆名は唯一の都市を失って滅亡する。以降の家の探索は滅亡した家も含める）→ 織田の首都に印が付く → 勝者が織田のチーム（伊達のチームではない）、勝利タイプが天下統一 → `unification-pact-victory.png`

## 実機確認

2026-09-30 の unification-alliance-03 で、セーブ／ロードを入れる前のシナリオ（上の手順1・6〜8・10・11。蘆名との手順2〜5とセーブ／ロードの手順9は後から追加）が勝者の確認まで PASS した（申し込み・「AI と同盟していない」・同盟後の成立・3城で印なし・移管で印・勝者は織田のチームで伊達のチームではない・勝利タイプは天下統一）。2026-09-29 の unification-alliance-01 は、滅亡した蘆名を生存中の家から探して勝者の確認で止まったが、家の探索を `GetWasEverAliveCount` に直して解消した。

残りの確認（セーブ／ロードと、O-7 の脱退・交戦での解消は、追加したシナリオの再実行で見る）：

```
cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-autotest -- tools/civ6-autotest/scenarios/unification-pact.json
```

- 手順2〜5：画面からの脱退、同盟中の相手への強制の宣戦が効くか、交戦での解消、その後の講和
- 手順9：同盟成立後のセーブ／ロードで同盟の状態が残るか
- 主体の滅亡による引き継ぎ、開始時の同盟の合算（15 km の史実の地図）、AI からの申し込みと人間への画面の自動表示、登録城のない地図での本拠の合算（どれも mlua では確認済み）
- AI の申し込みの条件（30%）の妥当さ。約200ターンの進行で、同盟がどれだけ組まれるかを見て調整する
- 画面の見た目と位置（上部中央。他の画面と重ならないか）。スクリーンショットで確認する
- マルチプレイ：人間どうしの申し込みと受諾、全端末で同じ状態になるか。相手への通知はなく、相手が画面を開くと「申し込みを受ける」が出る
- チーム戦での合算とチーム全員の勝利
- 印を付けてから勝利の評価までに城を失った場合（要実機確認）。unification-alliance-03 は最後の城の移管の直後に勝った流れだけを見ており、印の設置後・勝利の評価前に別の城を失う流れは見ていない。エンジンが同じ変化の中で必ず勝利を評価するかは未確定
