# 戦国パックの時代（黄金・暗黒・英雄）再設計案

> **位置づけ：調査・設計案。SSOT の改定ではない。** 根拠は [SSOT 3.2](../spec/ssot/03-map-and-scale.md)「Civ6 の黄金時代・暗黒時代（時代スコア）は、戦国向けに作り直して残す」と、[3.1](../spec/ssot/03-map-and-scale.md)「日本マップでは忠誠心による離反を起こさない」。SSOT が沈黙している数値・名称・効果はすべて**暫定**で、オーナー確認前の下書き。実装はしない。
> 参照元は `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/`（以下「公式」）。`DLC/Expansion2` が Gathering Storm（GS）、`DLC/Expansion1` が Rise and Fall。`RULESET_JAC_BANSEIKI` のゲームコアは GS なので、**`DLC/Expansion2` 内のコピーを有効な定義として引用する**（`Expansion1_*.xml` は Expansion1 側と内容が違う。例：閾値は Expansion1 が 12/24、Expansion2 が 14/28）。テキストは `DLC/Expansion2/Text/en_US/`、UI の Lua は `DLC/Expansion2/UI/Additions/` にある。行番号は `ファイル:行`。

## 0. 要約（オーナー向け）

1. **推奨は「案A：公式の時代エンジンを土台に、値・誓約・効果だけ戦国化する（データのみ）」。** 時代の判定・閾値・誓約 UI・年齢表示は公式のまま使う。マルチプレイ・セーブ・AI に強く、実装量が小さい。
2. **公式の時代スコアはデータでしか増やせない。** 出典を探した範囲で、Lua から時代スコアを加算する API は無い（§1.3）。したがって「支城を1つ落とすたびに時代スコア」「同盟を結ぶと時代スコア」「統一進捗で時代スコア」は案A単体では**できない**。できるのは、公式が持つ「歴史的瞬間」の点数調整（本拠陥落・大名家滅亡・旧領回復など）と、誓約のスコアクエスト（討伐数など）。
3. 支城の攻略や統一進捗そのものを点にしたいなら「案B：Lua で独自の武功台帳を持ち、公式の時代を置き換える」が要る。表現力は最大だが、UI・保存・マルチ同期・AI 対応がすべて自作になる。**案Aを先に作り、実測で不足が確認できたら案Bへ拡張する**のを勧める。
4. 4 つの期は、既存 PoC の名称を維持する（領国の基盤／技術交流と領国競争／天下布武と広域戦／統一事業）。ターンは 1–25／26–100／101–145／146–（統一まで続行）。黄金・暗黒・英雄の表示名は「隆盛／衰乱／再興」を提案。
5. 黄金・暗黒の効果から**忠誠を外す**。日本マップでは公式の忠誠効果が実質ゼロ（§2）で、SSOT 3.1 とも矛盾するため。代わりに「政策枠+1（隆盛）」「戦国版の暗黒政策 4 種（衰乱）」「誓約（期ごとに 3 種、9 種）」を軸にする。
6. **最大の落とし穴は 3 つ**：①「他勢力と遭遇」の歴史的瞬間が 1 人 +1 で、22 勢力だと第 I 期だけで最大 +21 になり、全員が黄金時代になり得る。②戦国 PoC の政策削除で**暗黒政策が 1 つも残っていない**。③公式の誓約は時代 ID で出し分けられ、`MILITARY` などは PoC の 4 期（`ERA_ANCIENT`〜`ERA_RENAISSANCE`）では一度も出ない。いずれも §5・§6 で対処案を出した。
7. 実機で測るべき項目を 1 本のシナリオにまとめた（`tools/civ6-autotest/scenarios/ages-probe.json`、読み取り専用）。**メインセッションで実行して数値を確定してから実装に入る**。

## 1. 公式の時代システム（調査）

### 1.1 全体像

```
歴史的瞬間（Moments.EraScore）＋ 加算 Modifier ─→ 時代スコア（勢力ごと・累積）
        │
時代の終わり（Eras_XP1 の最短/最長ターン、残り10ターンのカウントダウン）
        ▼
スコア ≥ 黄金閾値 → 黄金（前の時代が暗黒なら英雄）
スコア ≥ 暗黒閾値 → 通常
それ未満        → 暗黒
        ▼
新しい時代の開始時に「誓約(Dedication)」を選ぶ（黄金1・通常1・暗黒1・英雄3）
        ▼
その時代の間、年齢ごとの効果（忠誠圧力±0.5、暗黒政策、誓約の強効果 or スコアクエスト）
```

時代は**世界共通**、年齢は**勢力ごと**に決まる。最初の時代（`ERA_ANCIENT`）に年齢は無く、誓約は 2 つ目の時代から（`DedicationPopup.lua:36-39`）。

### 1.2 事実表

| 項目 | 公式の事実 | 出典 |
|---|---|---|
| 時代 ID | 基本 8（`ERA_ANCIENT`〜`ERA_INFORMATION`）＋ GS で `ERA_FUTURE`。`ChronologyIndex` 順 | `Base/Assets/Gameplay/Data/Eras.xml:15-24`、`DLC/Expansion2/Data/Expansion2_Eras.xml:3-8` |
| 時代の長さ | 各時代に最短・最長ターン 40/60（`ERA_FUTURE` だけ無し＝終端）。残り 10 ターンのカウントダウンが別にある | `DLC/Expansion2/Data/Expansion1_Eras.xml:3-13`、`DLC/Expansion2/Data/Expansion2_GlobalParameters.xml:204`、`DLC/Expansion2/UI/Additions/EraProgressPanel.lua:100-115` |
| 時代スコアの閾値 | 黄金 28、暗黒 14（GS）。閾値 ＝ 「時代開始時のスコア」＋基準値＋補正の合計 | `Expansion2_GlobalParameters.xml:66,126`、`DLC/Expansion2/UI/Additions/EraSupport.lua:3-42` |
| 閾値の補正 | 1 都市あたり +1、過去の黄金 1 回ごとに +5、過去の暗黒 1 回ごとに −5。未完了の技術・制度、不足アメニティの補正は 0。`ERA_ANCIENT` だけ −3 | `Expansion2_GlobalParameters.xml:250-264`、`Expansion2_Eras.xml:64-65`（`Eras_XP2.EraScoreThresholdShift`）、テキスト `DLC/Expansion2/Text/en_US/Expansion1_Eras_Text.xml:253-286` |
| 年齢の判定 | スコア ≥ 黄金 → 黄金、≥ 暗黒 → 通常、未満 → 暗黒。前時代が暗黒で今回黄金なら**英雄** | `EraProgressPanel.lua:117-133`、`DedicationPopup.lua:41-50`、`Expansion1_Eras_Text.xml:149`（暗黒の説明文） |
| 歴史的瞬間 | `Moments` 表に約 120 行。`EraScore`（多くは 1〜5、「世界初」が上位）、`MinimumGameEra/MaximumGameEra/ObsoleteEra` で出る時代を絞る。**新しい瞬間の型を足すにはエンジン側（GameCoreTypes）の登録が要る**と明記されている | `DLC/Expansion2/Data/Expansion1_Moments.xml:191-321`（注記 :192）、`Expansion2_Moments.xml:31-77`（注記 :32）、列定義 `DLC/Expansion2/Data/Expansion2_Schema.sql:472-493` |
| 城に関係する瞬間 | 元首都の占領 +4（`:226`）、その勢力の最後の都市の占領 +5（`:227`）、元の持ち主への返還 +2（`:228`）。**通常の都市占領に瞬間は無い** | `Expansion1_Moments.xml:225-228` |
| 同盟・外交に関係する瞬間 | 同盟成立・同盟レベルの瞬間は無い。あるのは開戦事由つきの宣戦 +2（`:318`）、他勢力との遭遇 +1（`:286`）、全勢力と遭遇 +3（`:284`）／世界初 +5（`:285`）、都市国家関連 | `Expansion1_Moments.xml:279-286,318`、同盟の型は `DLC/Expansion2/Data/Expansion1_Alliances.xml:23-29`（瞬間との紐付けなし） |
| 加算 Modifier（時代スコアの供給元） | `MODIFIER_PLAYER_ADJUST_PLAYER_ERA_SCORE_PER_*`：技術ひらめき・制度ひらめき・区域建設・軍団/軍の撃破・交易路完了・海軍撃破・科学/文化/産業/飛行場の建物・偉人獲得・遺物・宗教改宗・大陸/自然遺産発見・スパイ成功・歴史的瞬間ごとの上乗せ。全プレイヤー向けに「現在の時代以降の技術/制度の完了ごと」「昇進ごと」もある | `Expansion1_Modifiers.xml:13-32,184-186`、定義 `:469-572,1076-1090` |
| Modifier を建物に載せた先例 | タージ・マハルは歴史的瞬間ごとの上乗せを持つ | `DLC/Expansion1/Data/Expansion1_Buildings_Major.xml:309-312` |
| 誓約(Dedication) | `CommemorationTypes`（11 種）。出現時代は `MinimumGameEra/MaximumGameEra`。黄金では強効果（`OwnerRequirementSetId=PLAYER_HAS_GOLDEN_AGE`）、通常・暗黒ではスコアクエスト（`PLAYER_ELIGIBLE_FOR_COMMEMORATION_QUEST`、達成ごと +1）。選択数の基本 1、提示は最大 3 | `Expansion1_Moments.xml:348-360,511-563,565-860,1128-`、`Expansion2_GlobalParameters.xml:62-64`、`Expansion2_Schema.sql:86-96`、要件セット `Expansion1_Leaders.xml:1949-1963,2046-2061` |
| 誓約 UI | 選択肢は `GetPlayerCommemorateChoices`、選択数は `GetPlayerNumAllowedCommemorations`、確定は `PlayerOperations.COMMEMORATE`。アイコン名は `ICON_` ＋型名 | `DedicationPopup.lua:55-75,114,206-217`、`Expansion1_Icons_HistoricMoments.xml:23-33` |
| 年齢の効果 | (1) 忠誠：黄金・英雄は市民 1 人あたりの圧力 +0.5、暗黒 −0.5（`MODIFIER_PLAYER_CITIES_ADJUST_IDENTITY_PER_CITIZEN`、`TRAIT_LEADER_MAJOR_CIV` に付く）。(2) 暗黒政策：`Policies_XP1.RequiresDarkAge`。(3) 誓約の強効果。(4) 黄金の誓約 MILITARY は「黄金時代の宣戦（GA war）」外交行動を解放 | `Expansion1_Leaders.xml:138-139,192-201,760-769`、`Expansion1_Civilopedia_Text.xml:152,170`、`Expansion1_Policies.xml:74-82`、`Expansion2_Policies.xml:195-198`、`Expansion1_DiplomaticActions.xml:5,24,30,40` |
| 黄金時代を持つ公式モードの先例 | **「Dramatic Ages」モード**：誓約を廃止（選択数・提示数 0）し、黄金/暗黒を**政策**（`Policies_XP1.RequiresGoldenAge/RequiresDarkAge`）で表現。閾値 25/25、1 都市 +3、過去の暗黒 −10。全プレイヤーに「技術/制度完了ごと +1」「昇進ごと +1」の時代スコアを `GameModifiers` で付与。`ERA_ANCIENT` の閾値補正を 0 に | `DLC/Byzantium_Gaul/Data/Byzantium_Gaul_DramaticAges_MODE.xml:6-55,96-131,187-205,231-239`、`Byzantium_Gaul_DramaticAges_Expansion2_MODE.xml:7,18-25`、`Byzantium_Gaul_DramaticAges_Georgia_MODE.xml:40-68` |
| Lua で読める API（すべて読み取り） | `Game.GetEras()` の `GetCurrentEra/StartTurn/MinimumEndTurn/MaximumEndTurn`、`GetNextEraCountdown`、`GetPlayerCurrentScore`、`GetPlayerGoldenAgeThreshold/DarkAgeThreshold/ThresholdBaseline`、`GetPlayerCurrentEraScoreBreakdown`、`HasGoldenAge/HasDarkAge/HasHeroicGoldenAge`、`GetPlayerCommemorateChoices/NumAllowedCommemorations/ActiveCommemorations`。`Game.GetHistoryManager():GetAllMomentsData/GetMomentData`。**加算・設定の呼び出しは公式 Lua 全体に無い** | `EraProgressPanel.lua:61-133`、`EraSupport.lua`、`DedicationPopup.lua:42-65`、`HistoricMoments.lua:153,217,286`、`DLC/Expansion2/Scripts/WorldCongress.lua:255`（ゲームプレイ側でも `Game.GetEras():GetCurrentEra()` を読める） |
| Lua で使える周辺 API | 都市占領イベント `GameEvents.CityConquered(capturerID, ownerID, cityID, x, y)`、ターン開始 `GameEvents.OnGameTurnStarted`、`Player/City:AttachModifierByID`、`Game:SetProperty/GetProperty`（自 MOD でも使用済み） | `DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:174,248`、`DLC/NubiaScenario/Scripts/NubiaScenario.lua:937,969`、`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:1204`、`poc/JAC_PoC_Bundle0/Scripts/JAC_PoC_Bundle0.lua:11-14` |

### 1.3 データでは変えられない（エンジン内部）と推定されるもの

- 時代が**いつ**終わるか（最短〜最長の間で何を条件に進むか）。データにあるのは最短/最長/カウントダウン 10 ターンだけ。
- 英雄時代の成立条件、誓約の選択肢の抽出（提示が最大 3 のとき何が選ばれるか）、「暗黒時代を 2 回続けると都市が廃墟になる」という説明文（`Expansion1_Eras_Text.xml:330`）の実装の有無。この文は Rise and Fall 由来の古い文言と思われ、データ側に対応する実装は見つからなかった。**要実機確認**。
- 時代スコアの加算・年齢の付与を Lua から行う手段。公式 Lua には呼び出しが無い（未確認 ≠ 不可能。`ages-probe.json` でメソッド一覧を取る）。

## 2. 戦国ツリー PoC の現状と穴

現状（`documents/poc/sengoku-tree-compatibility.md`、`poc/JAC_PoC_SengokuTrees`、`poc/JAC_PoC_JapanMapScale`）：

- 既存 4 ID `ERA_ANCIENT`〜`ERA_RENAISSANCE` を互換スロットとして使い、表示名だけ戦国の 4 期に変更（`Data/JAC_Sengoku_Trees.sql:220-226`）。`Eras_XP1/XP2`、後続の時代 ID、GS の時代スコア・誓約・忠誠は**未加工で残っている**。互換メモ自身が「SSOT 3.2 に未達、再設計は次段階の TODO」と書いている。
- 技術は 8／10／8／6、制度は 7／9／7／7 個（計 62）が 4 期に割り当てられている（`JAC_Sengoku_Trees.sql:70-`、`144-`）。
- 日本マップは全都市に「毎ターン忠誠 +100」を付け（`Data/JAC_Japan_MapRules.xml:17,21,26`）、近隣市民の圧力の上限を 20→5 にしている（同 `:36`）。

データから読める**現状で起きるはずのこと**（すべて実機未検証。`ages-probe.json` で確認する）：

| # | 起きること | 根拠 | 影響 |
|---|---|---|---|
| P1 | 日本マップでは黄金・暗黒の**忠誠効果が事実上ゼロ**。±0.5/市民の圧力は毎ターン +100 に打ち消され、圧力上限も 5 | `Expansion1_Leaders.xml:760-769` と `JAC_Japan_MapRules.xml:17-36` | 現状の黄金＝誓約 1 つ、暗黒＝何も起きない。SSOT 3.2 の「残す」が中身なしで通っている |
| P2 | **暗黒政策が 0 件**。PoC は政策を 16 件に絞って残りを削除しており、公式の暗黒政策 13 件（`Expansion1_Policies.xml:74-82`、`Expansion2_Policies.xml:195-198`）は全滅 | `Data/JAC_Sengoku_Unlocks.sql:120-126` | 暗黒時代の「代償つきの選択肢」が消える |
| P3 | 誓約の出現が公式の時代 ID 帯のまま。第 II・III 期（`CLASSICAL`・`MEDIEVAL`）は SCIENTIFIC/CULTURAL/INFRASTRUCTURE/RELIGIOUS の 4 種、第 IV 期（`RENAISSANCE`）は INFRASTRUCTURE/RELIGIOUS/EXPLORATION/ECONOMIC の 4 種が候補で、提示は最大 3。**MILITARY/INDUSTRIAL（`INDUSTRIAL`〜）は一度も出ない**。黄金の強効果も港・商業ハブ・信仰など戦国に無い区域を前提にする | `Expansion1_Moments.xml:349-359` | 戦国らしい選択肢が無い。軍事が選べない |
| P4 | 時代の長さが公式の 40〜60 ターン（4 期で 160〜240）。ツリー設計の 25／75／45／55 と合わない | `Expansion1_Eras.xml:3-8` と `sengoku-tech-civic-tree.md` §4 | 第 I 期が長すぎる／第 II 期が短すぎる |
| P5 | 第 IV 期の後に**空の `ERA_INDUSTRIAL` へ進む**恐れ。進んだ場合の誓約（`INDUSTRIAL/MILITARY`）や宗教旧式化などの副作用 | 同上（`ERA_RENAISSANCE` に最長 60 が付いている）、ツリー設計 §4 の懸念 | 統一まで続行する SSOT 3.2 と矛盾 |
| P6 | 「他勢力と遭遇」が 1 人 +1、全員と遭遇で +3 / 世界初 +5。22 勢力・都市国家なしの日本マップでは、第 I 期の閾値（黄金 28−3＝25 付近）に遭遇だけで届き得る | `Expansion1_Moments.xml:284-286`、`Expansion2_Eras.xml:65`、SSOT 14（22 勢力） | **全員が黄金時代**になり、年齢が意味を失う |

## 3. 設計の前提

- SSOT 3.2：戦国パックは約 200 ターン（1534〜1600 年ごろ）、**天下統一するまで続行し、時間切れ・スコアによる勝利は置かない**。時代スコアは勝利条件ではない。
- SSOT 3.1：日本マップでは忠誠心の圧力で都市が離反することを絶対に起こさない。年齢の効果は忠誠に頼らない。
- 勝利＝登録した全城（約 50）の支配（SSOT 3.2 の表）。時代は**勝利の速度と接戦度を調整する装置**として置く。
- 数値は暫定。SSOT 3.2 の「PoC の実機で約 200 ターンの進行と 22 勢力の研究の進み方を測ってから決める」に従い、§11 の実測で確定する。
- 新規ゲーム限定（既存セーブへの後付けは対象外）。マルチプレイの同期は未検証。

## 4. 4 つの時代（期）

名称は既存の PoC（`JAC_Sengoku_Text.xml`、ja/en/ru 済み）を維持し、ターンと主題を確定する。ターンは標準速度で 1 ターン≒0.33 年（200 ターン＝66 年）から換算した目安（年はツリー設計の区分に一致：`sengoku-tech-civic-tree.md` §4、国立公文書館年表）。

| 期 | 名称（PoC 済み） | ターン | 年 | 技術ID | 主題（時代スコアの見どころ） | 代表的な史実（通説） |
|---|---|---|---|---|---|---|
| I | 領国の基盤 | 1–25 | 1534〜1542 | `ERA_ANCIENT` | 領内整備。年齢は無く、**次期の年齢の元手**を貯める短い期 | 領国の再編、守護代・国人の台頭 |
| II | 技術交流と領国競争 | 26–100 | 1543〜1567 | `ERA_CLASSICAL` | 鉄砲・南蛮文物の流入、隣国との競り合い、大名家の淘汰 | 1543 鉄砲伝来、1549 キリスト教伝来、1560 桶狭間 |
| III | 天下布武と広域戦 | 101–145 | 1568〜1581 | `ERA_MEDIEVAL` | 上洛と広域の戦、同盟と包囲網、鉄砲の実戦投入 | 1568 上洛、1573 幕府滅亡、1575 長篠 |
| IV | 統一事業 | 146–（統一まで続行） | 1582〜1600 | `ERA_RENAISSANCE` | 検地・刀狩・築城などの統治と、残る城の攻略 | 1582 本能寺、1583 賤ヶ岳、1590 小田原、1600 関ヶ原 |

年齢の評価は**期の切り替わり（T26・T101・T146）で 3 回**。各期の年齢は前の期のスコアで決まり、誓約はその期の頭に選ぶ。第 IV 期のスコアを受け取る次の期は無い（終端。§8）。

年齢の呼び名（公式の `LOC_ERAS_*` を文言差し替え。ja/en/ru が必要）：

| 公式 | 提案（暫定） | en 案 |
|---|---|---|
| Golden | 隆盛 | Ascendancy |
| Normal | 平時 | Steady Times |
| Dark | 衰乱 | Decline |
| Heroic | 再興 | Revival |

出典：`Expansion1_Eras_Text.xml:13-24,61-72`（差し替え対象のタグ）。

## 5. 時代スコア（何に点を与えるか）

### 5.1 方針

- 点の出どころを 3 層に分ける。①**歴史的瞬間**（公式の型のまま点数だけ調整）、②**常時加算**（`GameModifiers` で全員に付ける。Dramatic Ages 方式）、③**誓約のスコアクエスト**（通常・暗黒時代の勢力が誓約で得る追加加点）。
- 日本マップで**起きえない出来事の点はゼロにする**（都市国家・蛮族・新大陸・世界一周・近現代）。ゼロにしないと「遭遇」のような常時発火する瞬間が閾値を占有する（P6）。
- 公式の年齢の骨格（黄金＝強い、暗黒＝低い閾値で救済）は変えない。

### 5.2 歴史的瞬間（`Moments.EraScore` を SQL で調整）

`UPDATE Moments SET EraScore=…` はデータのみで可能（`Moments.EraScore` は列 :478）。**エンジンが実行時にこの列を読むか**は実機確認（`ages-probe.json` の `moment_scores` と、加点後の内訳）。

| 区分 | 瞬間 ID（行） | 公式値 | 提案 | 理由 |
|---|---|---:|---:|---|
| **城・統一に効かせる** | `CITY_TRANSFERRED_FOREIGN_CAPITAL` (:226) | 4 | **6** | 元首都（22 勢力の本拠）の占領。天下統一への大きな一歩 |
| | `CITY_TRANSFERRED_PLAYER_DEFEATED` (:227) | 5 | **8** | 大名家の滅亡（最後の城の陥落） |
| | `CITY_TRANSFERRED_TO_ORIGINAL_OWNER` (:228) | 2 | **3** | 旧領回復・再興 |
| **武功に効かせる** | `WAR_DECLARED_USING_CASUS_BELLI` (:318) | 2 | 2 | 大義名分のある開戦 |
| | `UNIT_KILLED_UNDERDOG_MILITARY_FORMATION` (:316) / `_PROMOTIONS` (:317) | 1／3 | 1／3 | 寡兵で大軍を破る（桶狭間型） |
| | `UNIT_KILLED_ASSISTED_BY_GENERAL` (:315) | 2 | 2 | 名将の指揮（武将システムと接続。SSOT 4） |
| | `FORMATION_ARMY/CORPS/FLEET/ARMADA_FIRST` (:245-252) | 1〜2 | 1〜2 | 軍制の整備 |
| **内政・文化** | `TECH/CIVIC_..._IN_ERA_FIRST` (:229-230,300-301) | 1／2 | 1／2 | 期の最初の研究 |
| | `BUILDING_CONSTRUCTED_GAME_ERA_WONDER` (:206) 他 `FIRST_UNIQUE` (:201,231,270,311) | 4 | 4 | 固有の建造物・ユニット（**戦国パックに驚異・固有物が残るか要確認**） |
| | `GREAT_PERSON_CREATED_*` (:266-269) | 1〜3 | 1〜3 | 名将・名匠 |
| | `RELIGION_FOUNDED*` (:295-296)、`BELIEF_ADDED_*` (:199-200) | 2〜4 | 2〜4 | 宗教（SSOT 9.2 の 20 宗教） |
| | `CITY_SIZE_*_FIRST` (:217-224) | 1〜2 | 1〜2 | 城下町の発展 |
| | `UNIT_CREATED_FIRST_DOMAIN_SEA*` (:307-308) | 2／3 | 2／3 | 水軍の整備 |
| **ゼロにする** | `PLAYER_MET_MAJOR` (:286)、`PLAYER_MET_ALL_MAJORS*` (:284-285) | 1／3／5 | **0** | 22 勢力で常時発火する（P6） |
| | `BARBARIAN_CAMP_DESTROYED*` (:196-197)、`GOODY_HUT_TRIGGERED` (:257) | 1〜3 | 0 | 蛮族・遺跡なし（要確認） |
| | `CITY_BUILT_*` (:208-214)、`FIND_NEW_CONTINENT*` (:244)、`WORLD_CIRCUMNAVIGATED*` (:319-320) | 1〜5 | 0 | 都市は配置済み・新大陸なし |
| | `PLAYER_GAVE_ENVOY_*` (:279-281)、`PLAYER_LEVIED_MILITARY*` (:282-283)、`TRADING_POST_*` (:302-304)、`EMERGENCY_*` (:240-241) | 1〜5 | 0 | 都市国家・世界会議・緊急事態は日本マップで使わない（SSOT 3.2） |
| | `GOVERNMENT_ENACTED_TIER_*` (:258-263) | 2〜3 | 0 | 政体は 4 種のみで意味が薄い |
| | `CITY_TRANSFERRED_DISLOYAL_*` (:225 ほか) | 2 | 0 | 忠誠離反は起こさない（SSOT 3.1） |
| | 空・宇宙・核、`XP2` の鉄道・運河・再エネ等（`Expansion2_Moments.xml:33-54`） | 1〜4 | 0 | 近現代 |

**注意**：占領された城の返還や大名家の滅亡は、SSOT 9 の吸収・降伏・暗殺者などで「都市が譲渡」される場合に、エンジンが `TRANSFERRED_*` 瞬間を発火するか未確認。`GameEvents.CityConquered` 相当は戦闘占領のみの可能性がある。§11 で確認。

### 5.3 常時加算（`GameModifiers`）

Dramatic Ages の実装をそのまま流用する（`Byzantium_Gaul_DramaticAges_MODE.xml:96-131`）：

| Modifier | 内容 | 初期値 |
|---|---|---:|
| `MODIFIER_ALL_PLAYERS_ADJUST_ERA_SCORE_PER_CURRENT_TECH` | 現在の時代以降の**技術**を完了するごと | +1 |
| `MODIFIER_ALL_PLAYERS_ADJUST_ERA_SCORE_PER_CURRENT_CIVIC` | 同・**制度** | +1 |
| `MODIFIER_ALL_PLAYERS_ADJUST_ERA_SCORE_PER_UNIT_PROMOTION_EARNED` | 昇進（初回を除く）ごと | +1 |

各期の完走点は技術 8／10／8／6 ＋ 制度 7／9／7／7 で 15／19／15／13。統一までにツリーの 6〜7 割を選ぶ想定（ツリー設計 §1）なので、研究だけで各期 **約 10〜12 点**の床が付く。1534 年開始時に共通で「研究済み」として与える第 I 期の一部ノード（SSOT 3.2）が加点や「期の最初の研究」の瞬間になるかは要実機確認（全員に同じ点なら害は小さい）。これは暗黒閾値（14）に届かない程度で、黄金閾値（28）には残り 16 点前後を「戦果・内政・誓約」で稼がせる設計になる。

### 5.4 要望された出来事と実現可否

| 出来事 | 案A（データのみ） | 案B（Lua 台帳） |
|---|---|---|
| 本拠・最後の城の陥落 | ○ 既存瞬間の点数調整（§5.2） | ○ |
| **通常の支城の占領** | ×（瞬間が無い）。代わりに誓約の「撃破数」クエストで間接的に反映 | ○ `GameEvents.CityConquered` で加点 |
| **統一進捗**（保有城／全城） | ×（Lua から時代スコアを足せない）。閾値の 1 都市あたり補正（`THRESHOLD_SHIFT_PER_CITY`）で「大領国ほど黄金が遠い」逆進性だけ調整できる（§9 決定4） | ○ 保有率で加点。期末に判定 |
| **史実イベント**（鉄砲伝来・キリスト教伝来・検地・刀狩など） | △ イベントの実装が**技術/制度のひらめきを付与**する形なら、誓約クエスト（ひらめき+1）を通じて間接加点。イベント自体は点を持てない | ○ イベント参加・成功を加点 |
| **同盟** | ×（同盟に紐づく瞬間・Modifier なし） | ○ 毎ターン同盟状態を走査して加点（同盟成立イベントは公式 Lua に無い） |

## 6. 誓約（Dedication）案

公式の 11 種は時代 ID 帯で固定（P3）で、戦国に無い区域を前提にする。戦国用に**新規 9 種（期ごとに 3 種）**を定義し、公式の 11 種は出現時代を `ERA_FUTURE` にして到達不能にする（`DELETE` は `Moments.CommemorationType` と `DiplomaticActions_XP1.RequiresGoldenAgeCommemorationType` の `ON DELETE CASCADE` を巻き込むため避ける。`Expansion2_Schema.sql:133-140,489`）。あわせて `DIPLOACTION_DECLARE_GOLDEN_AGE_WAR` は Dramatic Ages 同様に削除する（`Byzantium_Gaul_DramaticAges_Expansion2_MODE.xml:7`）。

各期の提示を 3 種ちょうどにして `COMMEMORATE_OPTIONS_MAX=3`（現行値）で全部提示する（抽出の乱数を排除）。効果はすべて**公式が使っている ModifierType**だけで書き、値は公式値を初期値にする。

**黄金時代の強効果／通常・暗黒時代のスコアクエスト（達成ごと +1、軍は撃破ごと軍団 +1・軍 +2）**

| 期 | 誓約 ID（案） | 名称 | 黄金の強効果（初期値） | スコアクエスト（加算源） | 公式の対応 |
|---|---|---|---|---|---|
| II | `COMMEMORATION_JAC_SENGOKU_FIREARMS` | 鉄砲と南蛮の風 | 技術のひらめき効果 +10%（`ADJUST_TECHNOLOGY_BOOST`） | 技術ひらめき 1 回 +1（`PER_TECH_BOOST`） | SCIENTIFIC（`:673,1011`） |
| II | `COMMEMORATION_JAC_SENGOKU_DOMAIN` | 検地と普請 | 区域 1 つにつき 生産 +1（`CITIES_ADJUST_CITY_YIELD_PER_DISTRICT`、`YieldType` を生産に） | 区域建設 1 回 +1（`PER_DISTRICT_CONSTRUCTED`） | CULTURAL/INFRASTRUCTURE（`:574,827`） |
| II | `COMMEMORATION_JAC_SENGOKU_ARMS` | 足軽の軍備 | 軍事ユニット生産 +15%（`ADJUST_MILITARY_UNITS_PRODUCTION`） | 軍団撃破 +1／軍撃破 +2（`PER_CORPS_KILLED`/`PER_ARMY_KILLED`） | MILITARY（`:647,981`、クエスト `:558-559`） |
| III | `COMMEMORATION_JAC_SENGOKU_MARKETS` | 楽市楽座 | 交易路の金 +3／専門区域（`ADJUST_TRADE_ROUTE_YIELD_PER_SPECIALTY_DISTRICT_FOR_INTERNATIONAL`） | 交易路完了 1 回 +1（`PER_TRADE_ROUTE_COMPLETED`） | ECONOMIC（`:579,886`） |
| III | `COMMEMORATION_JAC_SENGOKU_INTRIGUE` | 調略 | スパイの設置時間 −100%、攻撃任務 −25%（`ADJUST_SPY_ESTABLISH_TIME`/`..OFFENSIVE_OPERATION_TIME`） | 成功した攻撃任務 +1（`PER_SPY_SUCCESSFUL_MISSION`） | ESPIONAGE（`:709,715,1061-1066`）。**戦国のスパイ相当ユニットが残るか要確認**（SSOT 9.5 の暗殺者と関連） |
| III | `COMMEMORATION_JAC_SENGOKU_FIELD` | 野戦と城攻め | 全ユニットの戦闘経験 +100%（`UNITS_ADJUST_UNIT_EXPERIENCE_MODIFIER`） | 軍撃破 +2（`PER_ARMY_KILLED`） | AERONAUTICAL_GA_AIR（`:771,1126`）の型を流用。公式は空軍限定の要件セット（`:772`）なので、陸上ユニット向けの要件セットを新設する |
| IV | `COMMEMORATION_JAC_SENGOKU_CASTLES` | 天下普請 | 建造物・驚異の生産 +10%（`CITIES_ADJUST_WONDER_ERA_PRODUCTION`、`StartEra/EndEra` は戦国4期の ID） | 文化建造物 1 回 +1（`PER_CULTURE_BUILDING_CONSTRUCTED`） | INDUSTRIAL（`:610,916-931`） |
| IV | `COMMEMORATION_JAC_SENGOKU_SEAS` | 朱印船と海上支配 | 海上移動 +2（`UNITS_ADJUST_SEA_MOVEMENT`） | 敵水軍の撃破 +1（`PER_NON_BARBARIAN_NAVAL_UNIT_KILLED`） | EXPLORATION（`:589,896`） |
| IV | `COMMEMORATION_JAC_SENGOKU_TEA_AND_TEMPLES` | 茶の湯と寺社 | 制度のひらめき効果 +10%（`ADJUST_CIVIC_BOOST`） | 制度ひらめき 1 回 +1／宗教都市の改宗 1 回 +1 | CULTURAL/RELIGIOUS（`:569,864`） |

- 英雄時代は 3 つ同時に選べる（`Expansion1_Eras_Text.xml:131`）。第 II〜IV 期が各 3 種のため**英雄時代は全部を取る**形になる。差別化が欲しければ各期を 4 種に増やす（`COMMEMORATE_OPTIONS_MAX` を 4 に）。
- 「軍団」「軍」「スパイ」「海軍」「文化建造物」が戦国パックの実データに残っているかは未確認（`Units.CanTrain`・`Buildings.InternalOnly` の絞り込みがあるため。`poc/sengoku-tree-compatibility.md` の制約欄）。残らない誓約は主題を差し替える。
- アイコンは `ICON_` ＋型名（`DedicationPopup.lua:114`）。**新型ごとに `IconDefinitions` を足す**必要がある（公式の 11 種の絵を流用する方式でよい）。
- 家別の固有誓約（信長=天下布武、信玄=風林火山など）は SSOT に規定が無いため今回は含めない（§12 Q7）。

## 7. 黄金・暗黒・英雄の効果

**忠誠は使わない。** 削除は日本マップ限定にして、SSOT 3.1 の適用範囲（万世紀の通常 GS 地図には日本マップ特例を適用しない）に合わせる。`GOLDEN_AGE_CITY_IDENTITY`／`DARK_AGE_CITY_IDENTITY` を `TRAIT_LEADER_MAJOR_CIV` から `<Delete>` する（`JAC_Japan_MapRules.xml:25` の `<Delete>` と同じ書式。公式の削除例は `Expansion1_Leaders.xml:135-137`）。年齢ダイアログの「市民 1 人あたり 1.5 の忠誠圧力」（`Expansion1_Eras_Text.xml:142-153,166-177`）の文言は戦国用に差し替える。

| 年齢 | 誓約 | 政策枠・暗黒政策 | その他 |
|---|---|---|---|
| **隆盛**（黄金） | 1 つ（強効果） | **ワイルドカード枠 +1**（`MODIFIER_PLAYER_CULTURE_ADJUST_GOVERNMENT_SLOTS_MODIFIER`、`PLAYER_HAS_GOLDEN_AGE`。`Byzantium_Gaul_DramaticAges_Georgia_MODE.xml:40-57`）。暗黒政策は使えない | 次の閾値 +5（公式。`Expansion2_GlobalParameters.xml:264`）＝先頭集団への軽いブレーキ |
| **平時**（通常） | 1 つ（スコアクエスト） | なし | — |
| **衰乱**（暗黒） | 1 つ（スコアクエスト） | **戦国版の暗黒政策 4 種**を解禁（下表） | 次の閾値 −5（公式 `:262`）＝立て直しの追い風。閾値が低い（14）ので回復しやすい |
| **再興**（英雄） | 3 つ | 暗黒政策も解禁（暗黒から立ち直った勢力の特権） | 暗黒の翌期に黄金の閾値へ届いた場合（公式ルール） |

暗黒の追加ペナルティは**実測後に決める**（初期はなし）。候補は「全都市の文化 −10%」など Monasticism 型の減算（`Expansion1_Civics_Text.xml:129-130`）。追加すると、先頭集団を止めない暗黒＝救済型の性格が崩れるため。

**戦国版の暗黒政策 4 種**（公式の暗黒政策 13 件から選び、名称と説明だけ戦国化して再利用。`RequiresDarkAge` が付いた行を PoC の政策残し一覧に足し、出現時代を 4 期に合わせる。効果値は公式の説明文どおり）：

| 公式 ID（行） | 戦国名（暫定） | 効果（公式の説明） | 時代 ID の調整 |
|---|---|---|---|
| `POLICY_TWILIGHT_VALOR`（`Expansion1_Policies.xml:75`） | 背水の陣 | 近接攻撃ユニットの戦闘力 +5。ただし領外では回復できない（`Expansion1_Civics_Text.xml:132-133`） | `CLASSICAL`〜`RENAISSANCE` のまま |
| `POLICY_ELITE_FORCES`（`:80`） | 精鋭主義 | 全ユニットの戦闘経験 +100%。ただし戦闘・スパイユニット 1 体ごとに維持費 +2 金（`:147-148`） | `INDUSTRIAL`→`CLASSICAL` へ |
| `POLICY_LETTERS_OF_MARQUE`（`:78`） | 海賊衆の免許（村上水軍） | 海上襲撃ユニット生産 +100%・移動 +2、ただし交易路収益 −50%（`:141-142`） | `RENAISSANCE` 以降で第 IV 期に一致 |
| `POLICY_MONASTICISM`（`:74`） | 寺院学問 | 聖地のある都市の科学 +75%、ただし全都市の文化 −25%（`:129-130`） | `CLASSICAL`〜`MEDIEVAL`（第 II・III 期） |

`POLICY_ISOLATIONISM`（新規入植禁止）は都市を配置済みの日本マップでは代償が無意味なので外す。`POLICY_INQUISITION` は宗教ユニット前提で対象が狭いので外す。これらのポリシーに前提研究が付いているか（暗黒時代のみで解禁される作りか）は要実機確認。

## 8. 統一勝利・約 200 ターンとの関係

- **勝利は登録した全城の支配だけ。** 時代スコアもターン数も勝利条件に入れない（SSOT 3.2）。年齢は勝利の**速度と接戦度**を調整する。
- **接戦度**：黄金は強効果（誓約）と次の閾値 +5 で先頭集団に軽いブレーキ、暗黒は閾値 −5 で立て直しと暗黒政策の代償つき選択肢、英雄は 3 誓約で逆転の芽。22 勢力のうち上位が突き抜けやすい戦国では、「暗黒から再興へ」が弱小家の物語（毛利・長宗我部のような）を支える。
- **200 ターン**：期の切り替わりは T26・T101・T146。統一まで続行するため第 IV 期は終端にし、公式の時代切り替え（`ERA_RENAISSANCE`→空の `ERA_INDUSTRIAL`）が起きないように `Eras_XP1` の `ERA_RENAISSANCE` を `GameEraMaximumTurns=999` などにする（P5）。それでも進むなら、誓約・宗教・政策の時代帯（`INDUSTRIAL` 以降）を空にして副作用を封じる。**第 IV 期のスコアは受け取る次期が無いので、内訳の表示だけが残る**。最終期のスコアに使い道を持たせるのは案B。
- 早期統一：勝利が即時終了になるだけで、時代は関与しない。1600 年以降に未統一でも続行。
- 期の長さの決め方は決定2（§9）。案は固定（最短＝最長）と幅つき（最短＜最長）。

## 9. 選択肢とオーナー判断

### 決定1：時代スコアの仕組み（最重要）

| | **A（推奨）公式基盤・データのみ** | **B 独自の武功システム（Lua）** | **C 中立化（MVP）** |
|---|---|---|---|
| 概要 | 公式の判定・閾値・誓約 UI を使い、値・誓約・効果・時代長・暗黒政策だけ戦国化 | Lua が武功（城攻略・統一進捗・史実イベント・同盟）を集計し、期の切り替わりで年齢を判定。公式の時代スコアは無効化し、効果は `AttachModifierByID` で付与 | 時代スコアと年齢の効果を無効にして画面も隠す |
| SSOT 3.2 | ○ 誓約と年齢が「作り直して残る」 | ○ | ✕ 「残す」に反する（前回案。フォールバックのみ） |
| 支城占領・統一進捗・同盟を点にできるか | ✕（本拠・最後の城・旧領のみ。他は誓約クエストで間接） | ○ | — |
| 実装量 | 小：SQL/XML 数ファイルとテキスト。Lua なし | 大：Lua 台帳、保存、AI 用の選択ロジック、独自 UI（誓約と年齢表示）、公式の時代スコア UI との二重表示を隠す処理 | 小 |
| リスク | 公式ルール（時代終了条件・英雄条件）の未確認が主。他は既存 UI を使うため低い | ゲームプレイ Lua のイベント精度、マルチ同期、AI が誓約を選ぶ手段、`AttachModifierByID` の持続とセーブ復元が未検証 | 無効化が漏れて SSOT 3.1 の忠誠離反が起きる可能性 |
| 判断 | 先に作り、§11 の実測で得点分布を確認 | 案Aの実測で「支城占領が点にならないと遊びが痩せる」と分かったら追加 | A・B が実機で成立しない場合のみ |

案Bの要点：公式の年齢を殺すには `GOLDEN_AGE_SCORE_BASE_THRESHOLD` を大きく、`DARK_AGE_SCORE_BASE_THRESHOLD` を 0、`COMMEMORATE_BASE_CHOICES_ALLOWED` と `OPTIONS_MAX` を 0 にする（Dramatic Ages が 0 にした先例：`Byzantium_Gaul_DramaticAges_MODE.xml:6-18`）。**案A→Bは段階移行が可能**（案Aのデータは案Bの効果部分にそのまま流用できる）。

### 決定2：期の長さ

| 案 | 内容 | 長所 | 短所 |
|---|---|---|---|
| **固定（推奨）** | `GameEraMinimumTurns=GameEraMaximumTurns`＝25／75／45／999 | 誓約と史実イベントの時期が読める | `min==max` をエンジンが受けるか未確認。研究の進みに依らず切り替わる |
| 幅つき | 公式同様 min＜max（例：25〜35 など） | 研究速度に追従 | 切り替えの条件が不明で、AI の得点分布が読みにくい |

### 決定3：暗黒時代の重さ

| 案 | 内容 |
|---|---|
| **救済型（推奨）** | 暗黒は代償つきの選択肢（暗黒政策）と低い閾値（回復しやすい）。追加ペナルティなし。再興（英雄）で逆転 |
| 罰型 | 暗黒に生産・文化などの明確な減算を付ける。脱落する勢力が出やすい。「統一まで続行」の SSOT と、脱落した AI が遊びを止めない配慮に注意 |

### 決定4：大領国の扱い（`THRESHOLD_SHIFT_PER_CITY`）

| 案 | 値 | 意味 |
|---|---:|---|
| **公式（推奨）** | +1 | 都市が多いほど閾値が上がる＝先頭集団に軽いブレーキ。統一に近い勢力が年齢面で有利にならない |
| 中立 | 0 | 都市数は影響しない |
| 逆進 | −1 | 大領国ほど黄金に届きやすい＝統一進捗が年齢に反映されるが、雪だるま式になる |

## 10. 実装スケッチ（次段階。今回は実装しない）

置き場は `JAC_PoC_SengokuTrees` に `Data/JAC_Sengoku_Ages.sql`（ルールセット条件は既存と同じ）を足す案。日本マップ限定の忠誠削除だけ `JAC_Japan_MapRules.xml` に足す。

1. `UPDATE Eras_XP1 SET GameEraMinimumTurns=…, GameEraMaximumTurns=… WHERE EraType IN (…)`（§8）。`Eras_XP2` の `ERA_ANCIENT` 補正は実測で決める（−3 のままか 0 か）。
2. `UPDATE GlobalParameters …`（閾値・補正・選択数。初期は公式値）。
3. `UPDATE Moments SET EraScore=… WHERE MomentType IN (…)`（§5.2）。
4. `GameModifiers` に 3 行（§5.3）。
5. `Types`／`CommemorationTypes`／`CommemorationModifiers`／`Modifiers`／`ModifierArguments`／`IconDefinitions` を 9 型分。公式 11 型は `MinimumGameEra=MaximumGameEra='ERA_FUTURE'`。`DIPLOACTION_DECLARE_GOLDEN_AGE_WAR` を削除。
6. 暗黒政策 4 種を政策の残し一覧に追加し、`Policies_XP1` の時代を調整。
7. 黄金の政策枠 +1（`TraitModifiers`＋`Modifiers`）。日本マップ限定で `GOLDEN/DARK_AGE_CITY_IDENTITY` を削除。
8. テキスト（ja/en/ru）：期名は済み。年齢名、誓約 9 種、暗黒政策 4 種、瞬間の文言（必要なら戦国化）、年齢ダイアログの忠誠文の差し替え。
9. 自動テスト：DB 断面（時代 4 期、誓約 9 型、暗黒政策 4、忠誠 Modifier なし、`MET_MAJOR` が 0）を返す `ui_lua` を追加。

## 11. 実機確認項目（メインセッションが実行）

`tools/civ6-autotest/scenarios/ages-probe.json`（18 ステップ、日本 15 km、武田が人間・その他 AI、**読み取りのみ**で 110 ターン）。実行：

```powershell
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/ages-probe.json --check
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/ages-probe.json --output tools/civ6-autotest/artifacts/ages-probe-01
```

`steps.jsonl` の出力で確認すること：

| # | 確認 | 見る出力 | 判断 |
|---|---|---|---|
| L1 | 想定した表・行が PoC 下で存在する（`Eras_XP1/XP2`、`CommemorationTypes`、`Policies_XP1`、`GlobalParameters`、閾値の値） | ステップ5の DB 断面 | §1・§2 の前提の確認 |
| L2 | PoC 下で暗黒政策が 0 件（P2）、`GOLDEN/DARK_AGE_CITY_IDENTITY` が付いている | `dark_golden_policies`、`age_identity_modifiers` | P2 の確認 |
| L3 | **Lua に時代スコアを加算・年齢を付与する API があるか** | ステップ6のメソッド名一覧（`Game`、`Eras`、`HistoryManager`、`Player`）。UI 状態のみ。ゲームプレイ側は `tuner.py states` で状態名を確認して追加調査 | あれば案A+の幅が広がる。無ければ案A/Bの前提どおり |
| L4 | 時代の長さ：既定（40〜60）で第 I 期が何ターン目に終わるか（`eraStart`、`minEnd`、`maxEnd`、`countdown`） | T1／T31／T61／T111 の各スナップ | 決定2、P4 |
| L5 | 得点分布：22 勢力の第 I 期の得点と内訳（特に他勢力との遭遇） | `score`／`src{…}`、`moments`（ステップ末尾） | P6、§5.2 のゼロ化 |
| L6 | 閾値の実値と、最初の年齢判定 | `golden>=`、`dark>=`、`G/D/H` | §5.3 の初期値 |
| L7 | 第 IV 期の先へ進むか | `era`（4 = `ERA_INDUSTRIAL` に進んだか。今回の 110 ターンでは T146 に届かないので、実装後に別途） | P5 |

- 得点が積まれた後の誓約ダイアログの内容（人間側）は自動化されていない。実装後に手動確認：第 II 期の頭に何が提示されるか（P3）。
- `autoplay` が「人間が敗退」で失敗した場合は、`seed` か人間枠を変えて再実行（既知：織田が 50 ターン内に敗退しやすい。武田を人間にした本シナリオでも起こり得る）。
- 実装後のスクリーンショット（`EraProgressPanel`、誓約ポップアップ）は人が見る。自動判定で見た目の成功を主張しない。

## 12. オーナーへの質問

1. **Q1（最重要）** 案A（データのみ）で先に作ってよいか。支城の通常占領は点にならず、本拠陥落・大名家滅亡・旧領回復と誓約クエストで代替する。これで十分か、案Bが必須か。
2. **Q2** 期の長さは固定（25／75／45／∞）でよいか。
3. **Q3** 暗黒時代は救済型（追加ペナルティなし、暗黒政策で代償）でよいか。
4. **Q4** 都市が多い勢力ほど黄金が遠くなる公式の補正（+1）を維持するか。
5. **Q5** 年齢の名称「隆盛／平時／衰乱／再興」でよいか（暫定）。
6. **Q6** 同盟・統一進捗・史実イベントを時代スコアの点にしたいか（案Bを選ぶ場合の要件）。
7. **Q7** 家別の固有誓約（信長＝天下布武など）を作るか。今回の 9 種は全家共通。

## 参照

- 戦国ツリーの互換メモ：[`../poc/sengoku-tree-compatibility.md`](../poc/sengoku-tree-compatibility.md)
- ツリー設計案（期の定義：§4）：[`sengoku-tech-civic-tree.md`](sengoku-tech-civic-tree.md)
- 主な公式ファイル：`DLC/Expansion2/Data/Expansion1_Moments.xml`、`Expansion1_Eras.xml`、`Expansion2_GlobalParameters.xml`、`Expansion1_Leaders.xml`、`Expansion1_Modifiers.xml`、`DLC/Expansion2/UI/Additions/{EraProgressPanel,EraSupport,DedicationPopup,HistoricMoments}.lua`、`DLC/Byzantium_Gaul/Data/Byzantium_Gaul_DramaticAges_*_MODE.xml`
