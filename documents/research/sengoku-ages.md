# 戦国パックの時代（黄金期・暗黒期）設計

> **位置づけ：設計。実装はしない。** 決定の正本は [SSOT 3.2](../spec/ssot/03-map-and-scale.md)（時代は改元ごとに判定し、時代スコアは Lua の「武功の台帳」で数える、ほか）と [3.1](../spec/ssot/03-map-and-scale.md)（日本マップでは忠誠心による離反を起こさない）。本書はそれを実装できる形に落とす。点数・閾値の数値は**暫定**で、実機で測ってから決める（SSOT 3.2 の数値の決め方に従う）。
> 参照元は `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/`（以下「公式」）。`DLC/Expansion2` が Gathering Storm（GS）、`DLC/Expansion1` が Rise and Fall。`RULESET_JAC_BANSEIKI` のゲームコアは GS なので、**`DLC/Expansion2` 内のコピーを有効な定義として引用する**（`Expansion1_*.xml` は Expansion1 側と内容が違う。例：閾値は Expansion1 が 12/24、Expansion2 が 14/28）。テキストは `DLC/Expansion2/Text/en_US/`、UI の Lua は `DLC/Expansion2/UI/Additions/` にある。行番号は `ファイル:行`。

## 0. 要約（オーナー向け）

1. **時代スコアは Lua の「武功の台帳」で数える（決定）。** 公式の時代スコアは Lua から足せない（§1.3）ので、公式の時代スコアと公式の誓約は止め、台帳が点を数え、判定し、効果を付ける。
2. **判定は改元のとき（決定）。** 天文→弘治（1555）→永禄（1558）→元亀（1570）→天正（1573）→文禄（1593）→慶長（1596）。その後も史実の改元を令和（2019）まで続け、令和を100年続けた後は架空の「遅代」（2119）で全ての家を暗黒期にする（オーナー判断 L-3、§4.1）。改元のたびに全勢力を同時に判定し、結果は Civ6 と同じく**黄金期・通常期・暗黒期**（暗黒期の次に黄金期なら英雄期）と表示する。改元の画面と時代スコアの画面は Civ6 と同じ見せ方にする。
3. **点になるもの**：本拠陥落・大名家滅亡・旧領回復・支城の占領・同盟・史実イベント（SSOT 3.2）。加えて技術・社会制度の完了と誓約の達成を点にする。
4. **暗黒期は救済型（決定）**：追加の罰は無い。暗黒期だけ選べる専用の政策と、次の改元での逆転（英雄期）がある。**都市が多い家ほど黄金期が遠くなる公式の補正は残す（決定）。** 補正は台帳の閾値の式で同じ値を使う（§5.3）。公式の側の補正は、公式の暗黒時代が起きないように 0 にする（§4.2）。
5. **誓約は家ごとに固有のもの（決定）**を作る。信長の「天下布武」など。各家に固有の誓約を3つずつ作り、共通の誓約は置かない（オーナー判断 L-4、§7.2）。
6. **天下統一同盟の合算は台帳とは別の仕組み。** 城の合算と「主体の家だけが勝つ」は勝利判定の側で行う（[天下統一勝利](../poc/unification-victory.md)）。台帳の点は家ごとに数え、同盟の仲間どうしで合算しない。
7. 台帳は Game プロパティに置き、セーブと一緒に保存・復元する。書き込みはゲームプレイ側の Lua だけで行い、マルチプレイでも全端末で同じ結果になるように組む（§6）。
8. 元号の見せ方・友好宣言の点・慶長の後の区切り・誓約の組み方（旧 Q36〜Q39）は、オーナー判断 L（2026-09-30）で決まった（L-1〜L-4。§4.1・§5.4・§7.2・§8）。ツリーの段階はオーナー判断 T（元号＝時代）で元号そのものになった（[`../poc/era-equals-reign.md`](../poc/era-equals-reign.md)）。

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
| 時代スコアの閾値 | 黄金 28、暗黒 14（GS）。閾値 ＝ 「時代開始時のスコア」＋基準値＋補正の合計。スコアはゲーム開始からの累計（前の時代の合計を含む） | `Expansion2_GlobalParameters.xml:66,126`、`DLC/Expansion2/UI/Additions/EraSupport.lua:6,27`（時代開始時のスコア `GetPlayerThresholdBaseline`。表示名は「Score when Era began」：`Expansion1_Eras_Text.xml:250-251`）、`EraProgressPanel.lua:69,73-86`（累計のスコアと前の時代の合計） |
| 閾値の補正 | 1 都市あたり +1、過去の黄金 1 回ごとに +5、過去の暗黒 1 回ごとに −5。無政府・未完了の技術・制度・不足アメニティの補正は 0。`ERA_ANCIENT` だけ −3 | `Expansion2_GlobalParameters.xml:248-264`、`Expansion2_Eras.xml:64-65`（`Eras_XP2.EraScoreThresholdShift`）、テキスト `DLC/Expansion2/Text/en_US/Expansion1_Eras_Text.xml:253-286` |
| 年齢の判定 | スコア ≥ 黄金 → 黄金、≥ 暗黒 → 通常、未満 → 暗黒。前時代が暗黒で今回黄金なら**英雄**。まとめると、その時代に得た点を `E`（スコア − 時代開始時のスコア）とすると **`E` < 暗黒の基準値＋補正の合計 なら暗黒**。基準値を 0 にしても、都市 1 つの補正 +1 だけで `E` = 0 の家は暗黒になる | `EraProgressPanel.lua:117-133`（閾値 `:117,119`、判定 `:127-133`）、`DedicationPopup.lua:41-50`、`Expansion1_Eras_Text.xml:149`（暗黒の説明文） |
| 歴史的瞬間 | `Moments` 表に約 120 行。`EraScore`（多くは 1〜5、「世界初」が上位）、`MinimumGameEra/MaximumGameEra/ObsoleteEra` で出る時代を絞る。**新しい瞬間の型を足すにはエンジン側（GameCoreTypes）の登録が要る**と明記されている | `DLC/Expansion2/Data/Expansion1_Moments.xml:191-321`（注記 :192）、`Expansion2_Moments.xml:31-77`（注記 :32）、列定義 `DLC/Expansion2/Data/Expansion2_Schema.sql:472-493` |
| 城に関係する瞬間 | 元首都の占領 +4（`:226`）、その勢力の最後の都市の占領 +5（`:227`）、元の持ち主への返還 +2（`:228`）。**通常の都市占領に瞬間は無い** | `Expansion1_Moments.xml:225-228` |
| 同盟・外交に関係する瞬間 | 同盟成立・同盟レベルの瞬間は無い。あるのは開戦事由つきの宣戦 +2（`:318`）、他勢力との遭遇 +1（`:286`）、全勢力と遭遇 +3（`:284`）／世界初 +5（`:285`）、都市国家関連 | `Expansion1_Moments.xml:279-286,318`、同盟の型は `DLC/Expansion2/Data/Expansion1_Alliances.xml:23-29`（瞬間との紐付けなし） |
| 加算 Modifier（時代スコアの供給元） | `MODIFIER_PLAYER_ADJUST_PLAYER_ERA_SCORE_PER_*`：技術ひらめき・制度ひらめき・区域建設・軍団/軍の撃破・交易路完了・海軍撃破・科学/文化/産業/飛行場の建物・偉人獲得・遺物・宗教改宗・大陸/自然遺産発見・スパイ成功・歴史的瞬間ごとの上乗せ。全プレイヤー向けに「現在の時代以降の技術/制度の完了ごと」「昇進ごと」もある | `Expansion1_Modifiers.xml:13-32,184-186`、定義 `:469-572,1076-1090` |
| Modifier を建物に載せた先例 | タージ・マハルは歴史的瞬間ごとの上乗せを持つ | `DLC/Expansion1/Data/Expansion1_Buildings_Major.xml:309-312` |
| 誓約(Dedication) | `CommemorationTypes`（11 種）。出現時代は `MinimumGameEra/MaximumGameEra`。黄金では強効果（`OwnerRequirementSetId=PLAYER_HAS_GOLDEN_AGE`）、通常・暗黒ではスコアクエスト（`PLAYER_ELIGIBLE_FOR_COMMEMORATION_QUEST`、達成ごと +1）。選択数の基本 1、提示は最大 3 | `Expansion1_Moments.xml:348-360,511-563,565-860,1128-`、`Expansion2_GlobalParameters.xml:62-64`、`Expansion2_Schema.sql:86-96`、要件セット `Expansion1_Leaders.xml:1949-1963,2046-2061` |
| 誓約 UI | 選択肢は `GetPlayerCommemorateChoices`、選択数は `GetPlayerNumAllowedCommemorations`、確定は `PlayerOperations.COMMEMORATE`。アイコン名は `ICON_` ＋型名 | `DedicationPopup.lua:55-75,114,206-217`、`Expansion1_Icons_HistoricMoments.xml:23-33` |
| 年齢の効果 | (1) 忠誠：黄金・英雄は市民 1 人あたりの圧力 +0.5、暗黒 −0.5（`MODIFIER_PLAYER_CITIES_ADJUST_IDENTITY_PER_CITIZEN`、`TRAIT_LEADER_MAJOR_CIV` に付く）。(2) 暗黒政策：`Policies_XP1.RequiresDarkAge`。(3) 誓約の強効果。(4) 黄金の誓約 MILITARY は「黄金時代の宣戦（GA war）」外交行動を解放 | `Expansion1_Leaders.xml:138-139,192-201,760-769`、`Expansion1_Civilopedia_Text.xml:152,170`、`Expansion1_Policies.xml:74-82`、`Expansion2_Policies.xml:195-198`、`Expansion1_DiplomaticActions.xml:5,24,30,40` |
| 黄金時代を持つ公式モードの先例 | **「Dramatic Ages」モード**：誓約を廃止（選択数・提示数 0）し、黄金/暗黒を**政策**（`Policies_XP1.RequiresGoldenAge/RequiresDarkAge`）で表現。閾値 25/25、1 都市 +3、過去の暗黒 −10。全プレイヤーに「技術/制度完了ごと +1」「昇進ごと +1」の時代スコアを `GameModifiers` で付与。`ERA_ANCIENT` の閾値補正を 0 に | `DLC/Byzantium_Gaul/Data/Byzantium_Gaul_DramaticAges_MODE.xml:6-55,96-131,187-205,231-239`、`Byzantium_Gaul_DramaticAges_Expansion2_MODE.xml:7,18-25`、`Byzantium_Gaul_DramaticAges_Georgia_MODE.xml:40-68` |
| Lua で読める API（すべて読み取り） | `Game.GetEras()` の `GetCurrentEra/StartTurn/MinimumEndTurn/MaximumEndTurn`、`GetNextEraCountdown`、`GetPlayerCurrentScore`、`GetPlayerGoldenAgeThreshold/DarkAgeThreshold/ThresholdBaseline`、`GetPlayerCurrentEraScoreBreakdown/PreviousEraScoreBreakdown`、`GetFinalEra`、`HasGoldenAge/HasDarkAge/HasHeroicGoldenAge`、`GetPlayerCommemorateChoices/NumAllowedCommemorations/ActiveCommemorations`。`Game.GetHistoryManager():GetAllMomentsData/GetMomentData`。**加算・設定の呼び出しは公式 Lua 全体に無い** | `EraProgressPanel.lua:61-133`（前の時代の内訳は `:73`、最後の時代は `:122`）、`EraSupport.lua`、`DedicationPopup.lua:42-65`、`HistoricMoments.lua:153,217,286`、`DLC/Expansion2/Scripts/WorldCongress.lua:255`（ゲームプレイ側でも `Game.GetEras():GetCurrentEra()` を読める） |
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
| P2 | **暗黒政策が 0 件**。PoC は政策を 16 件に絞って残りを削除しており、公式の暗黒政策 13 件（`Expansion1_Policies.xml:74-82`、`Expansion2_Policies.xml:195-198`）は全滅 | `Data/JAC_Sengoku_Unlocks.sql:122-128` | 暗黒時代の「代償つきの選択肢」が消える |
| P3 | 誓約の出現が公式の時代 ID 帯のまま。第 II・III 期（`CLASSICAL`・`MEDIEVAL`）は SCIENTIFIC/CULTURAL/INFRASTRUCTURE/RELIGIOUS の 4 種、第 IV 期（`RENAISSANCE`）は INFRASTRUCTURE/RELIGIOUS/EXPLORATION/ECONOMIC の 4 種が候補で、提示は最大 3。**MILITARY/INDUSTRIAL（`INDUSTRIAL`〜）は一度も出ない**。黄金の強効果も港・商業ハブ・信仰など戦国に無い区域を前提にする | `Expansion1_Moments.xml:349-359` | 戦国らしい選択肢が無い。軍事が選べない |
| P4 | 時代の長さが公式の 40〜60 ターン（4 期で 160〜240）。ツリー設計の 25／75／45／55 と合わない | `Expansion1_Eras.xml:3-8` と `sengoku-tech-civic-tree.md` §4 | 第 I 期が長すぎる／第 II 期が短すぎる |
| P5 | 第 IV 期の後に**空の `ERA_INDUSTRIAL` へ進む**恐れ。進んだ場合の誓約（`INDUSTRIAL/MILITARY`）や宗教旧式化などの副作用 | 同上（`ERA_RENAISSANCE` に最長 60 が付いている）、ツリー設計 §4 の懸念 | 統一まで続行する SSOT 3.2 と矛盾 |
| P6 | 「他勢力と遭遇」が 1 人 +1、全員と遭遇で +3 / 世界初 +5。22 勢力・都市国家なしの日本マップでは、第 I 期の閾値（黄金 28−3＝25 付近）に遭遇だけで届き得る | `Expansion1_Moments.xml:284-286`、`Expansion2_Eras.xml:65`、SSOT 14（22 勢力） | **全員が黄金時代**になり、年齢が意味を失う |

`JAC_Japan_MapRules.xml` の忠誠 +100 は日本マップの離反防止（SSOT 3.1）のためのもので、本設計でも残す。P1〜P6 の扱いは §4 で決まる（P1・P6 は公式の年齢と時代スコアを止めるので問題でなくなる。P2 は §7.2 の暗黒期の専用政策で置き換える。P3 は公式の誓約を止めて家ごとの誓約に置き換える。P4・P5 は §4.3）。

## 3. 決定事項（オーナー、2026-09-29）

SSOT 3.2 に反映済みの決定。本書はこれを前提にし、別案は扱わない。

| # | 決定 | 本書の節 |
|---|---|---|
| 1 | 時代スコアは Lua の「武功の台帳」で数える。本拠陥落・大名家滅亡・旧領回復に加え、支城の占領・同盟・史実イベントも点にする | §5、§6 |
| 2 | 期の長さは改元の時期に合わせて幅を持つ | §4.1 |
| 3 | 暗黒期は救済型（追加の罰なし、専用の政策、次の区切りでの逆転）。Civ6 らしい演出は残す | §7、§8 |
| 4 | 都市が多い家ほど黄金期が遠くなる公式の補正を残す | §5.3 |
| 5 | 改元のたびに各勢力の黄金期・暗黒期を判定し、表示は「黄金期」「暗黒期」とそのまま呼ぶ（Civ6 の通常の表示と同じ） | §4.1、§8 |
| 6 | 誓約は家ごとに固有のものを作る（信長の天下布武など） | §7.2 |
| 7 | 天下統一同盟では城を合算し、主体の家だけが勝つ | §5.4（台帳とは別の仕組み） |

## 4. 期（改元）と公式の時代の扱い

### 4.1 期の区切り

改元の日を暦の月まで合わせ（SSOT 3.2「史実イベントは暦の月までできるだけ合わせて起こす」と同じ扱い）、その日を含むターンの開始時に判定する。ターンは標準速度（4 か月/ターン、1534 年がターン 1〜3。[暦の PoC](../poc/sengoku-calendar.md)）で `1 + 3×(年−1534) + floor((月−1)/4)` から求めた目安。本書のターンは画面の表示と同じく開始ターンを 1 と数える（戦国パックの暦ではエンジンのターン番号が約 16603 から始まる：#51、`JAC_Sengoku_Calendar.xml`）。台帳は `Game.GetCurrentGameTurn() − GameConfiguration.GetStartTurn() + 1` で数える（同じ数え方の先例：`poc/JAC_PoC_JapanMapScale/Scripts/JAC_Resources.lua:149`）。他の速度では、台帳が開始時に `GameSpeed_Turns`（現在の速度の行）から同じ日付のターンを求める。改元の日は西暦（ユリウス暦）の通説の日付で、実装時に国立公文書館の年表などで再確認する。

| 期 | 始まり（改元） | 西暦の日付 | 判定ターン（標準） | 期の長さ（標準） |
|---|---|---|---:|---:|
| 天文 | 開始時 | 1534 | —（判定なし） | 65 |
| 弘治 | 天文→弘治 | 1555-11 | 66 | 7 |
| 永禄 | 弘治→永禄 | 1558-03 | 73 | 37 |
| 元亀 | 永禄→元亀 | 1570-05 | 110 | 9 |
| 天正 | 元亀→天正 | 1573-08 | 119 | 59 |
| 文禄 | 天正→文禄 | 1593-01 | 178 | 11 |
| 慶長 | 文禄→慶長 | 1596-12 | 189 | 57 |
| 元和 | 慶長→元和 | 1615-09 | 246 | 25 |
| 寛永 | 元和→寛永 | 1624-04 | 271 | 63 |
| （正保〜慶応の33の改元） | | 1645-01〜1865-05 | 334〜995 | 3〜60 |
| 明治 | 慶応→明治 | 1868-10 | 1005 | 131 |
| 大正 | 明治→大正 | 1912-07 | 1136 | 43 |
| 昭和 | 大正→昭和 | 1926-12 | 1179 | 187 |
| 平成 | 昭和→平成 | 1989-01 | 1366 | 91 |
| 令和 | 平成→令和 | 2019-05 | 1457 | 300 |
| 遅代 | 令和→遅代 | 2119-05 | 1757 | ゲーム終了まで |

- 天下統一まで続けるため（SSOT 3.2）、慶長の後も史実の改元で区切る（オーナー判断 L-3、2026-09-30。旧 Q38）。元和・寛永から明治・大正・昭和・平成・令和まで、史実の順に 46 回の改元（弘治から令和まで）で判定する。令和は 100 年続け（2019 年 5 月から 2119 年）、その次は架空の元号「遅代」（ちだい、Chidai、Тидай）にする。戦国時代のまま令和の後まで続くと日本が世界から遅れたままになる、という位置づけで、遅代の改元では点によらず全ての家を暗黒期にする（暗黒期の家が黄金期の点に届いても英雄期にならない。暗黒期の政策の選択は通常どおり）。遅代の期はゲーム終了まで続き、その点は次の判定が無いので画面の表示だけに残る。
- 改元の年月の表は [`../spec/data/sengoku-reigns.json`](../spec/data/sengoku-reigns.json)（出典：Wikipedia「Japanese era name」の一覧。1582 年 10 月より前はユリウス暦、以後はグレゴリオ暦の西暦換算）。`tools/generate_sengoku_reigns.py` が表の行と ja・en・ru の名前・見出しを作る。各速度のターンは生成した SQL の各行の末尾に書く。
- 暦（`JAC_Sengoku_Calendar.xml`）の標準速度は変えない。標準以外の速度の暦は、元号＝時代の実装でエンジンの時代の始まりに合わせて作り直した（オーナー判断 U-2b、[元号＝時代](../poc/era-equals-reign.md)）。2119 年（遅代）は、オンライン 865・クイック 1159・標準 1757・エピック 2620・マラソン 5269 ターン目。令和は 715・958・1457・2170・4369 ターン目。戦国パックの目安（標準で約 200 ターン、SSOT 3.2）を大きく超えるので、遅代まで進むのは天下統一が大きく遅れたゲームだけになる。
- **同じターンの改元（長さ 0 の期）**：一番短い期は万延（1860 年 4 月〜1861 年 3 月）で、オンラインで 1 ターン、標準で 3 ターン。今の表と暦では、どの速度でも 2 つの改元が同じターンに入ることは無い（`tests/test_sengoku_ages.py` が全速度で確かめる）。表か暦が変わって同じターンに入った場合は、そのターンの改元のうち最後のものにまとめ、1 回だけ判定する（間の改元の期は長さ 0 で、判定・通知・画面を出さない）。判定の結果には締めた期の番号（`from`）を残し、改元の画面はその期の元号を出す。
- 期の長さの差（弘治 7 ターン、天正 59 ターン）は、閾値を期の長さに比例させて吸収する（§5.3）。
- 開始時の天文の期も判定する（最初の判定は弘治への改元のとき）。天文の期はそれまでの 65 ターン分の点で判定する。天文の期そのものに年齢は無い（Civ6 の最初の時代に年齢が無いのと同じ）。

### 4.2 公式の時代スコア・年齢・誓約を止める

台帳の結果と公式の年齢が二重に出ないように、公式の仕組みはデータで止める。止め方は公式の Dramatic Ages モードの先例に合わせる（`DLC/Byzantium_Gaul/Data/Byzantium_Gaul_DramaticAges_MODE.xml:6-18`）。対象は `RULESET_JAC_BANSEIKI` の戦国パック（`JAC_PoC_SengokuTrees` と同じ条件）だけ。

公式の判定（§1.2）は、その時代に得た点 `E` と「基準値＋補正の合計」の比較になる。公式の年齢が起きないことは、**どの家・どの時代でも `暗黒の基準値＋補正の合計 ≤ 0 ≤ E < 黄金の基準値＋補正の合計`** が成り立つことで保証する。そのために補正をすべて 0 にし、基準値で上下を大きく離す。都市数の補正は台帳の式で使うので（§5.3、決定 4）、公式の側で 0 にしても決定は守られる。

| 設定 | 値 | ねらい |
|---|---|---|
| `DARK_AGE_SCORE_BASE_THRESHOLD` | −9999 | 暗黒の閾値を「時代開始時のスコア − 9999」にし、スコアが下回らないようにする（0 では都市の補正 +1 で暗黒になる） |
| `GOLDEN_AGE_SCORE_BASE_THRESHOLD` | 9999 | 公式の黄金時代にならない |
| `THRESHOLD_SHIFT_PER_CITY`・`THRESHOLD_SHIFT_PER_PAST_GOLDEN_AGE`・`THRESHOLD_SHIFT_PER_PAST_DARK_AGE` | 0 | 補正を消す（公式 +1・+5・−5：`Expansion2_GlobalParameters.xml:250,264,262`） |
| `THRESHOLD_SHIFT_PER_ANARCHY`・`…_INCOMPLETE_ERA_CIVIC`・`…_INCOMPLETE_ERA_TECH`・`…_INCOMPLETE_OLD_CIVIC`・`…_INCOMPLETE_OLD_TECH`・`…_MISSING_AMENITY` | 0（公式のまま） | 公式でも 0（`:248-260`）。他の MOD が変えても 0 に戻るよう、同じ値で上書きする |
| `Eras_XP2.EraScoreThresholdShift` | 全行 0 | `ERA_ANCIENT` の −3（`Expansion2_Eras.xml:65`）を消し、式を補正なしにそろえる |
| `COMMEMORATE_BASE_CHOICES_ALLOWED`・`COMMEMORATE_OPTIONS_MAX` | 0 | 公式の誓約ダイアログを出さない（Dramatic Ages と同じ） |
| `Moments.EraScore` | 全行 0 | 公式の歴史的瞬間は年表の演出として残し、点は持たせない |
| 時代スコアを加える Modifier の取り付け行 | 削除 | 瞬間以外の点の入口（型は `DLC/Expansion2/Data/Expansion1_Modifiers.xml:15-32,184-186`、`Expansion2_Modifiers.xml:80`）を塞ぐ。公式で取り付けているのは次の 3 か所だけ（2026-09-30 に公式 XML 全体を検索）：(a) 誓約の目標 `CommemorationModifiers` の `COMMEMORATION_*_QUEST*` 17 行（`Expansion1_Moments.xml`・`Expansion2_Moments.xml:149`。誓約を選べなくするので本来は効かないが、二重の備えで消す）、(b) タージ・マハル `BuildingModifiers` の `TAJ_MAHAL_EXTRA_ERA_SCORE`（`DLC/Expansion1/Data/Expansion1_Buildings_Major.xml:78`。2 点以上の瞬間ごとに +1 なので瞬間 0 点では効かないが消す）、(c) Dramatic Ages の `GameModifiers` 3 行（`Byzantium_Gaul_DramaticAges_MODE.xml:96-100`。技術・制度の完了と昇進ごとに +1。モードを使わない前提だが、モードで読み込まれても消えるよう `LoadOrder` を後にして消す） |

| `DIPLOACTION_DECLARE_GOLDEN_AGE_WAR` | 削除 | 公式の黄金時代に結びついた外交行動を消す（`Byzantium_Gaul_DramaticAges_Expansion2_MODE.xml:7`） |

瞬間の点と上の 3 か所をすべて 0・削除にして初めて `E` は 0 のままになる（瞬間の点だけでは足りない）。

公式の年齢に結びついた効果も、万一の年齢の発生に備えて外す（二重の備え）：

| 効果 | 処置 | 出典 |
|---|---|---|
| 黄金・暗黒の忠誠 ±0.5/市民 | `TraitModifiers` の `TRAIT_LEADER_MAJOR_CIV` の `GOLDEN_AGE_CITY_IDENTITY`・`DARK_AGE_CITY_IDENTITY` を削除 | `Expansion1_Leaders.xml:138-139,192-201,760-769` |
| 公式の暗黒政策（`Policies_XP1.RequiresDarkAge`）13 件 | 戦国パックが政策ごと削除済み（P2）。残っていないことをテストで固定する | `Expansion1_Policies.xml:74-82`、`Expansion2_Policies.xml:195-198`、`JAC_Sengoku_Unlocks.sql:122-128` |
| AI のランダムな思惑「追従」「同情」の、相手の黄金・暗黒による好感度 | 年齢が起きなければ効かない。処置しない | `Expansion1_Agendas.xml:60-61,76-95` |

Dramatic Ages モード（`Byzantium_Gaul_DramaticAges_*_MODE.xml`）は同じ値を別の値で上書きするので、戦国パックでは使わない。上の値は `LoadOrder` を公式・モードの後にして書き、データのテストで最終値を確かめる（§10）。

公式の年齢を止めるので、公式の忠誠効果（P1）と「遭遇」の点（P6）は効かなくなる。公式の誓約 11 種は出なくなるので、到達しない時代（`ERA_FUTURE` など）へ移して隠す必要も無い。

**要実機確認**：
1. エンジンが負の基準値をそのまま使うか（閾値を 0 や「時代開始時のスコア」で下限に丸めても、`E` ≥ 0 なので暗黒にはならない。丸めの下限がそれより上の場合だけ破れる）。確認は §10 の 5：公式の時代が切り替わるたびに全勢力の `HasDarkAge`・`HasGoldenAge`・`HasHeroicGoldenAge` が偽で、毎ターン `GetPlayerDarkAgeThreshold` ≤ `GetPlayerCurrentScore` ＜ `GetPlayerGoldenAgeThreshold` であること。
2. 歴史的瞬間を 0 点にしても年表に残るか。残らない場合も台帳には影響しない。

### 4.3 公式の時代（ツリーの 4 期）と `ERA_FUTURE`

> オーナー判断 T（2026-09-30、元号＝時代）で、この節の「4 期は改元と独立」「`ERA_RENAISSANCE` の最長ターンを大きくする」は置き換えた。今は元号ごとに1つの時代で、長さを元号のターン数に固定する（[実装](../poc/era-equals-reign.md)）。以下は当時の設計の記録。

- 技術・社会制度ツリーの 4 期（`ERA_ANCIENT`〜`ERA_RENAISSANCE`、[ツリー設計](sengoku-tech-civic-tree.md) §4）は公式の時代のまま進み、改元とは独立する。改元は台帳の判定の区切り、4 期はツリーの区切りで、別のものとして扱う。画面で「時代」と呼ぶのは元号の方（オーナー判断 L-1、§8）。ツリーの 4 期の名前はツリーの段階を指す所（ツリーの画面の列など）に残す。4 期が変わるときの公式の予告と画面の扱いは Q74。
- 公式の時代の最後は `ERA_FUTURE`（`DLC/Expansion2/Data/Expansion2_Eras.xml:4,7`、`GetFinalEra`）。戦国パックのツリーは `ERA_RENAISSANCE` で終わるので、`ERA_INDUSTRIAL` 以降へ進まないように `Eras_XP1` の `ERA_RENAISSANCE` の最長ターンを非常に大きくする（P5）。戦国ツリーには `ERA_INDUSTRIAL` 以降の技術・社会制度が無いので、研究による切り替えの秒読み（`EraProgressPanel.lua:100-115`）も始まらない見込み。
- それでも `ERA_INDUSTRIAL` 以降へ進んだ場合も、年齢と誓約は台帳が持つので影響しない。残る影響（時代で決まる旧式化など）は要実機確認とし、進むと分かったら `Eras_XP1` の値を見直す。`ERA_FUTURE` は台帳では使わない。

## 5. 武功の台帳（点の数え方）

### 5.1 点になるもの（暫定値）

| 区分 | 条件 | 点 | 付く相手 | 重複の防止（キー） |
|---|---|---:|---|---|
| 本拠陥落 | 他の家の本拠（開始時の首都。開拓者から始める場合は最初の首都。§5.2）を奪う | 6 | 奪った家 | `cap:<タイル番号>:<n>`（下） |
| 大名家滅亡 | 奪った城で相手の家の城が 0 になる | 8 | 最後の城を奪った家 | `ko:<滅んだ家>` |
| 旧領回復 | 最初の持ち主が自分だった城（開始時の城、または自分で建てた城。§5.2）を取り戻す | 3 | 取り戻した家 | `cap:<タイル番号>:<n>`（下） |
| 支城の占領 | 上のどれにも当たらない城の奪取 | 2 | 奪った家 | `cap:<タイル番号>:<n>`（下） |
| 同盟 | 外交で Civ6 の同盟を新しく結ぶ（開始時の同盟は除く。§5.4） | 2 | 両方の家 | `ally:<小さい番号>:<大きい番号>:<期>` |
| 友好宣言（同盟の区分） | ゲーム中に友好宣言を新しく結ぶ（開始時の関係は除く。§5.4。オーナー判断 L-2） | 1 | 両方の家 | `friend:<小さい番号>:<大きい番号>:<期>` |
| 史実イベント | イベントごとに決める（1〜3 の目安） | 表で定義 | イベントの対象の家 | `evt:<イベント ID>:<家>` |
| 技術・社会制度 | 1 つ完了するごと（開始時に与えたものは除く） | 1 | 研究した家 | 完了数の差分 |
| 誓約の達成 | 通常期・暗黒期の誓約の条件（§7.2） | 1 | 誓約した家 | 誓約ごとに定義 |

- 奪取のキーの `<n>` は、そのタイルの城の記録に持たせる奪取の回数（記録するたびに 1 増やす）。ターンや新しい持ち主をキーにすると、同じターンに同じ城が A→B→A→B と奪い合われたとき 2 回目以降が同じキーになり点が漏れるため、回数で区別する。占領イベントと差分は同じ記録を書き換えるので、どちらが先に走っても 1 回の奪取は 1 つの `<n>` にしかならない（§5.2）。
- 1 回の奪取は 1 区分だけで数える（本拠 > 旧領 > 支城の順）。大名家滅亡は、その奪取の区分に重ねて数える。
- 奪取は戦闘に限らず、所有者が変わったものを数える（吸収・降伏・移管で城が移る場合も同じ。SSOT 9）。忠誠による離反は日本マップでは起きない（SSOT 3.1）。
- 研究の点は Dramatic Ages の「技術・制度の完了ごと +1」に相当する床（`Byzantium_Gaul_DramaticAges_MODE.xml:96-131`）。戦わない家が毎回暗黒期になるのを避ける。
- 史実イベントの点は、台帳の表（出来事ごとの点。`EventType` をキーにする）で持たせる。出来事の仕組み（PR #42、`poc/JAC_PoC_HistoricalEvents`。main にマージ済み：`poc/JAC_PoC_HistoricalEvents/Scripts/JAC_HistoricalEvents.lua:11-12`）は、効果を与えた家のプレイヤーのプロパティ `JAC_EVENT_FIRED_<EventType>` に与えたターンを書く（1 家 1 回。セーブに残る）。台帳はこれを読むだけで、出来事の側に書き込みを足さない（§5.2）。ひらめきだけの `JAC_EVENT_BOOSTED_<EventType>` は点にしない。

### 5.2 数え方（ターン開始時の差分）

点はイベントの通知ではなく、**毎ターンの開始時（`GameEvents.OnGameTurnStarted`）に前のターンの記録との差分**で数える。

1. 城：全都市について、タイル番号ごとに所有者を記録する。前の記録と所有者が違えば奪取とし、§5.1 の区分に振り分ける。記録にない城のタイルは新しい城として、そのときの持ち主を「最初の持ち主」にする（点にしない）。前の記録にあった城のタイルに都市が無ければ破却として記録から外す（点にしない。奪ってすぐ破却した城の点は、下の占領イベントで付いている）。
2. 同盟：主要な家の組ごとに、ゲームプレイ側の `Players[a]:GetDiplomacy():HasAllied(b)` で同盟の有無を記録し、§5.4 の規則で新しく結んだ組を数える。`HasAllied` はゲームプレイ側で使える実績がある（開始時の同盟の維持 `poc/JAC_PoC_JapanMapScale/Scripts/JAC_Japan_1534Relations.lua:76,102`、実機の調査 `tools/civ6-autotest/scenarios/probe-alliance-expiry.json`）。公式 UI の `GetAllianceType` はゲームプレイ側の外交オブジェクトに無い（nil。[関係の実装メモ](../poc/japan-1534-relations.md) の「味方の視界共有」の実機）ので使わない。同盟の種類は点に関係しない。
3. 研究：家ごとの技術・社会制度の完了数を記録し、増えた分を数える。開始時に全勢力へ与える第 I 期の技術・社会制度（SSOT 3.2）は、付与後の数を初期値にして点にしない。
4. 史実イベント：表にある出来事ごとに、各家の `JAC_EVENT_FIRED_<EventType>` が付いていて、キー `evt:<EventType>:<家>` が未記録なら点にし、キーを記録する。点を入れる期は、プロパティの値（出来事が起きたターン）で決める（§5.3）。

差分だけでは、占領してすぐ破却された城（公式では占領のときに破却を選べる）が次の記録の時点でタイルに無く、点が漏れる。最後の城なら大名家滅亡の点も漏れる。そこで次の 2 つのイベントで補う。

- **占領** `GameEvents.CityConquered(capturerID, ownerID, cityID, x, y)`（`DLC/NubiaScenario/Scripts/NubiaScenario.lua:937,969`）：戦闘で城を奪ったその場で、§5.1 の区分で点を付け、城の記録の持ち主を奪った家に書き換える。負けた家の城が 0 になっていれば大名家滅亡（`ko:<家>`）もここで付ける。占領の直後に破却されても点は残る。記録を書き換えてあるので、次のターン開始の差分は同じ奪取を数えない。キーも差分と同じ `cap:<タイル番号>:<n>`（§5.1）を使い、どちらが先に走っても 1 回だけにする。同じターンに同じ城が何度奪われても、占領のたびに `<n>` が進むのでそのつど点になる。
- **建設** `GameEvents.CityBuilt(playerID, cityID, x, y)`（`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:56,255`）：新しく建てた都市だけでなく、`CityManager.TransferCity` で持ち主が変わった都市でも出る（2026-09-29 の実機。[天下統一勝利 PoC](../poc/unification-victory.md) の記録方法 2）。そのため `CityBuilt` では点を付けず、(a) 記録に無いタイルなら新しい城として登録する（最初の持ち主＝`playerID`）、(b) §7.1 の年齢のプロパティをその家の都市に書き直す、の 2 つだけを行う。どちらも今の状態に合わせて書くだけなので、移管で何度呼ばれても結果は同じ。移管の点は次のターン開始の差分で数える。

本拠：ターン 1 に首都がある家は、その首都を本拠にする。開拓者から始める設定（マルチプレイ。SSOT 3.1）で本拠が未登録の家は、最初の持ち主が自分で最初の首都の城（`City:GetOriginalOwner()`・`City:IsOriginalCapital()`：`DLC/Expansion2/UI/CityBanners/CityBannerManager.lua:1765`）を初めて記録したときに本拠にする。移管で受け取った他の家の首都は、最初の持ち主が違うので本拠にならない。この 2 つの関数をゲームプレイ側の Lua で呼べるかは**要実機確認**（UI 側の使用例しかない。呼べなければ、本拠が未登録の家が建てた最初の都市を本拠にする）。

移管・降伏・忠誠による離反など、イベントの無い経路はターン開始の差分で拾う。

制約：同じターン内に、イベントの出ない経路で城が 2 回持ち主を変えると、最後の持ち主への 1 回として数える。戦闘の占領はそのつど数える。通常のプレイでは起きにくいので許容する。

### 5.3 判定（改元のとき）

改元のターンの開始時に、その期の点で全勢力を同時に判定する。

- 閾値は期の長さ `L`（ターン）に比例させる。基準は公式の GS の値（黄金 28、暗黒 14）と公式の補正（1 都市 +1、過去の黄金期 1 回 +5、過去の暗黒期 1 回 −5：`Expansion2_GlobalParameters.xml:66,126,250-264`）で、これを公式の時代の標準の長さ 50 ターン（最短 40・最長 60 の中央：`Expansion1_Eras.xml:4`）あたりの値とみなす。
  - 台帳の点は公式の時代スコアよりずっと少ないので、公式の値を `K` で割る（`K` ＝台帳の 1 点あたりの公式の点。暫定 6）。
  - 暗黒期の閾値 = `floor((14 + 都市数 + 5×過去の黄金期 − 5×過去の暗黒期) × L / (50 × K))`、下限 0（切り捨て。期待される点に丸1点届かない家だけが暗黒期になる）
  - 黄金期の閾値 = `ceil((28 + 都市数 + 5×過去の黄金期 − 5×過去の暗黒期) × L / (50 × K))`、下限 2（短い期に技術1つで黄金期にならないように）。黄金の期待値は暗黒の期待値より大きいので、切り上げた黄金の閾値は切り捨てた暗黒の閾値より必ず大きく、通常期が間に残る。
  - 暗黒を切り捨て・下限 0 にした根拠（2026-09-30 の実機 health-200-01、200 ターン、AI 22 家）：7〜11 ターンの短い期では、期待される点が 1 点未満（研究が約 0.08 点/ターン）で、点は 0〜1。旧式（切り上げ・下限 1）では点 0 の家がすべて暗黒期になり、黄金・英雄・通常・暗黒の数は 弘治（L=65）4/0/13/5、永禄（L=7）0/0/8/14、元亀（L=37）0/8/6/7、天正（L=9）1/1/9/10、文禄（L=59）5/5/7/4、慶長（L=11）4/0/6/11 だった（元亀以降は 21 家）。記録の点と、記録の閾値から逆算した都市数に新しい式を順に当てると、4/0/17/1、0/0/22/0、5/0/13/3、1/1/19/0、6/0/10/5、4/0/15/2 になり、どの期も暗黒期は 1/3 以下、通常期が最多（単体テスト `ShortReignDistributionTests`）。長い期でも暗黒の閾値は 1 下がる（2026-09-29 の弘治の 22 家は黄金 3・通常 19・暗黒 0）。
  - 「最低の長さ」や前の期の点の持ち越しも考えたが、切り捨てだけで上の目標を満たすので採らない。切り上げのまま期待値が 1 未満のときだけ 0 にする案は、元亀で暗黒 10/21 になり（過去の暗黒期が減った分、補正が上がる）目標を満たさない。
  - 都市数は改元のターン開始時の数。
  - `K` の根拠（2026-09-29 の実機、15 km の日本マップ、AI 22 家、ターン 1〜65）：弘治の判定での点は 3〜12（研究が 1 家 1 ターンあたり約 0.08 点、出来事が 0〜3 点、城と同盟は 0 点）、中央値 5.5 点・都市 4。この家の公式の通常期の真ん中は `(21 + 4) × 65 / 50 = 32.5`、`32.5 / 5.5 ≈ 6`。`K = 1`（公式の値そのまま）では 22 家すべてが暗黒期、`K = 6` では黄金 3・通常 15・暗黒 4 になる（暗黒の切り上げ・下限 1 の旧式のとき。今の式では暗黒 0）。
  - 28・14・1・5・−5・`K` は台帳の定数として持つ。`GlobalParameters` から読まない（§4.2 で公式の値を 9999・−9999・0 に書き換えるため）。
- 改元のターンの開始時の処理は「取り込み（§5.2 の差分と出来事）→ 判定」の順にする。前のターンの手番の間に起きたことは古い期に入る。出来事は、プロパティの値が改元のターン以後なら新しい期に入れる（`OnGameTurnStarted` の中で出来事の仕組みと台帳のどちらが先に走っても、同じ期に入る）。
- 点 ≥ 黄金期の閾値 → 黄金期（前の期が暗黒期なら英雄期）、点 ≥ 暗黒期の閾値 → 通常期、それ未満 → 暗黒期。公式の判定（`EraProgressPanel.lua:117-133`）と同じ並び。
- 滅んだ家は判定しない。判定後、各家の期の点を 0 に戻す。
- 数値（28・14・50・`K`・各点）はすべて暫定。約 200 ターンの実機で、22 勢力の黄金期・通常期・暗黒期の割合を見て決める。

### 5.4 同盟の点と天下統一同盟

- 点になる「同盟」は、**外交（取引）で** Civ6 の同盟（研究・経済・軍事など、`DLC/Expansion2/Data/Expansion1_Alliances.xml:23-29`。戦国パックでは社会制度 C11 で解放：[ツリーの付け替え](../poc/sengoku-tree-unlocks.md)）を新しく結んだとき。
- ゲーム中に新しく結んだ**友好宣言**も同盟の区分で点にする（1 点ずつ、同じ組は 1 つの期に 1 回。オーナー判断 L-2、2026-09-30。旧 Q37）。見分け方と除き方は同盟と同じ（下の 1〜3 の `HasAllied` を `HasDeclaredFriendship` に読み替える。開始時の関係は友好宣言のあと同盟にして組のプロパティを書くので、プロパティが数値で友好宣言が続いている間は 1 で、プロパティが `false` になった後に残る友好宣言は 2 で除かれる）。台帳は組ごとに同盟とは別に友好宣言の状態を持つ。友好宣言から同盟に進んだ組は、友好宣言の 1 点と同盟の 2 点の両方が付く。
- 友好宣言の判定はゲームプレイ側の `Players[a]:GetDiplomacy():HasDeclaredFriendship(b)`。DLL に `lHasDeclaredFriendship` の文字列はある（`DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll`）が公式の使用例は無く、同じ系統の `GetDeclaredFriendshipTurn` はゲームプレイ側で nil だった（[関係の実装メモ](../poc/japan-1534-relations.md)）。ゲームプレイ側に無い場合は友好宣言を点にせず、Lua.log に 1 回だけ `no gameplay HasDeclaredFriendship` を出す（要実機確認）。
- **開始時の同盟（味方 8 組）は点にしない。** SSOT 3.1 の味方 8 組は、友好宣言に加えて Civ6 の通常の同盟で始まり（`SetHasAllied`）、外交で結んだものではない。エンジンの期限（30 ターン）で解けるターン 32 からは結び直さず、各家が通常の外交で結び直す（SSOT 3.1、オーナー判断K。[関係の実装メモ](../poc/japan-1534-relations.md)「開始時の同盟の終わり方」）。期限切れの後に外交で結び直した同盟は、同じ組でも点にする。
- 見分け方：組の Game プロパティ `JAC_1534_ALLIANCE_<勢力A>-<勢力B>`（勢力名は `CIVILIZATION_JAC_` を除いた名前の辞書順。値は開始時の同盟が続いている間は結んだターンの数値、期限切れ・イベント・交戦で終わったら `false`。`JAC_Japan_1534Relations.lua` の `ALLIANCE_KEY_PREFIX`・`TrackStartAlliances`）を読む。台帳は組ごとに前のターンの状態を「同盟なし」「同盟あり」「開始時の同盟」の 3 つで記録し、毎ターン次の順で決める（ゲームプレイの Lua だけで、`HasAllied` とプロパティを読むだけなので全端末で同じ結果になる）。
  1. プロパティが数値で `HasAllied` が真の組：状態を「開始時の同盟」にし、点にしない。期限切れのターン 32 は、`OnGameTurnStarted` の中で台帳と関係のスクリプトのどちらが先に走っても（プロパティがまだ数値でも、もう `false` でも）`HasAllied` が偽なので「同盟なし」になり、そのターン中に外交で結び直せば次のターンに点になる。
  2. プロパティが `false`（期限切れ・イベントで終えた）の組：前の状態が「開始時の同盟」なら、今の `HasAllied` をそのまま新しい状態にし、点にしない。イベントで終えても同盟はエンジンの期限切れまで残る（同メモ「開始時の同盟の終わり方」）ので、その残りを新しい同盟と数えない。以後は下の 3 と同じ。
  3. プロパティが無い組（と 2 の後）：前の状態が「同盟なし」で今 `HasAllied` が真なら、新しく結んだ同盟として点にする。
  - 組の 2 家が同じチーム（天下統一同盟）なら数えない（下）。
- 点は同盟を結んだ両方の家に 2 点ずつ付く。同じ組は 1 つの期に 1 回だけ数える（キー `ally:<小さい番号>:<大きい番号>:<期>`。期限切れの後の更新や、切って結び直す往復で稼げない）。
- 台帳が無い状態から始めるとき（ターン 1 の初期化）は、そのときの同盟を点にせず記録だけする。開始時の同盟は `LuaEvents.NewGameInitialized` で結ばれ（同 `:190`）、プロパティも同時に書かれるので、初期化の順番に関係なく 1 で除かれる。
- 開始時の同盟を結ぶ Lua は、史実の領土の日本マップ 2 種でだけ読み込まれる（同メモ「実装」）。他の地図ではプロパティが無く、すべて 3 で数える。
- 他のスクリプトが将来 `SetHasAllied` で同盟を作る場合（史実イベントなど）も、同じ形の組のプロパティを書いて 1・2 で除く。書かないスクリプトの同盟は、外交で結んだものと区別できずに点になる。
- 天下統一同盟（チーム）の仲間どうしは最初から組んでいるので、新しい同盟として数えない（`Players[a]:GetTeam() == Players[b]:GetTeam()` の組は飛ばす）。台帳の点・判定・効果は家ごとで、仲間どうしで合算・共有しない。
- 天下統一同盟の城の合算と「主体の家だけが勝つ」は、勝利判定（チーム単位の登録城の支配。[天下統一勝利](../poc/unification-victory.md)）の仕組みで、台帳とは独立する。台帳の点は勝利条件に入れない（SSOT 3.2：時間切れ・スコアによる勝利は置かない）。

## 6. 保存・マルチプレイ同期

### 6.1 保存と読み込み

- 台帳は Game プロパティ（`Game:SetProperty`／`Game:GetProperty`）に置く。Game プロパティはセーブに入り、読み込むと戻る（自 MOD で表を保存している先例：`poc/JAC_PoC_Bundle0/Scripts/JAC_PoC_Bundle0.lua:11-14`。家名の切り替えも同じ置き場：SSOT 3.1）。
- 中身：版番号、現在の期、家ごとの期の点と内訳・過去の黄金期と暗黒期の回数・現在の年齢・選んだ誓約と政策、城の所有者の記録（タイル番号ごと）、開始時の所有者と本拠、同盟の組、研究の完了数、数えたキーの一覧。表のキーは文字列にする（`"p0"`、`"plot1234"` など。数値キーの表が保存で崩れないかを試す手間を省く）。
- ターン 1 の初期化は「台帳が無いときだけ」行い、読み込みで初期化し直さない（`poc/JAC_PoC_GrowthUI/Scripts/JAC_PoC_GrowthUI_Gameplay.lua` の `INITIALIZED_KEY` と同じ形）。読み込み直後は何も数えず、次のターン開始で保存された記録との差分から続きを数える。
- 年齢の効果は、読み込み後に付け直さなくても続く作りにする（§7.1。Modifier の条件をタイルのプロパティで判定する）。タイルのプロパティがセーブに入ることは前提で、先例は Game プロパティだけなので**要実機確認**。入らない場合は、読み込み後の最初のターン開始で §7.1 の書き直す関数が台帳から書き直すまで効果が切れる。そのときは読み込み時の呼び出しを足すかを、マルチプレイの同期（§6.2）と合わせて決め直す。
- 実装時の確認（§10 の 5）：セーブ→読み込みの前後で (1) 台帳が一致すること、(2) 読み込み直後のターンで点が二重に入らないこと、(3) 既存の都市・占領した都市の中心タイルの `JAC_AGE`・`JAC_AGE_POLICY`・`JAC_AGE_ANCHOR` が一致すること、(4) 読み込み直後（ターンを進める前）に Modifier の効果（都市の産出の倍率、要の都市の家向けの効果）が読み込み前と同じ値で出ていること。

### 6.2 マルチプレイ同期

- 台帳への書き込みは**ゲームプレイ側の Lua だけ**で行う（`AddGameplayScripts`）。UI 側の Lua は Game プロパティを読むだけにする（家名の切り替えと同じ設計要件：SSOT 3.1）。端末ごとの設定値は使わない。
- 書き込みのきっかけは `GameEvents.OnGameTurnStarted`・`GameEvents.CityConquered`・`GameEvents.CityBuilt`（§5.2）と、下の選択の受け付けだけにする。全端末で同じ順に同じ処理が走るように、家は番号順、城はタイル番号順に処理し、`pairs` の順番に依存して書き込まない。乱数は使わない。
- 人間の選択（誓約・暗黒期の専用政策）は UI から `UI.RequestPlayerOperation(..., PlayerOperations.EXECUTE_SCRIPT, ...)` で送り、ゲームプレイ側の `GameEvents` で受けて台帳に書く（公式の先例：`DLC/BlackDeathScenario/Scripts/BlackDeathScenario_StateUtils.lua:64-74`）。受け側で「送った家が本人か」「今の期の選択か」「まだ選んでいないか」「候補にある誓約・政策か」を確かめ、合わないものは捨てる。
- AI の選択は家ごとの決まった順位表で選ぶ（乱数なし）。人間が期の間に選ばなかった場合も、次のターン開始で順位表の先頭を選んだものとする。
- 重複の防止は §5.1 のキーで行う。同じキーを 2 回数えない。
- **未検証**：マルチプレイの実機。家名の切り替えと同じく、マルチプレイ時の同期・決定性は未検証として扱う（[PoC README「マルチプレイ同期と決定性」](../../poc/README.md#マルチプレイ同期と決定性)）。

## 7. 効果

**時代の仕組みでは忠誠の効果を足さない。** 日本マップの忠誠の扱い（通常の忠誠圧力による離反を起こさず、総忠誠心・政策・総督などの個別の効果は維持する：SSOT 3.1）はそのまま。なお総督は、戦国パックでは戦国向けに作り直すまで無効（#59、[ツリーの付け替え](../poc/sengoku-tree-unlocks.md)「付け替えずに失う行」）なので、年齢の効果にも総督を使わない。公式の年齢を止めるので、公式の年齢の忠誠効果（`Expansion1_Leaders.xml:760-769`）も効かない。

### 7.1 付け方

効果は主体（都市か、家とそのユニットか）で付け方を分ける。どちらも Modifier は最初から付けておき、プロパティで入り切りする（`AttachModifierByID` で付けた効果は外せないため、年齢ごとに付け外ししない）。

- **付け先**：戦国パックの全指導者に共通の特性 `TRAIT_JAC_AGES` を持たせ（`LeaderTraits`。既存の共通特性 `TRAIT_JAC_1534_LEADER` と同じ形：`poc/JAC_PoC_CustomCivs/Data/JAC_Japan1534_Gameplay.sql:146-149`）、`TraitModifiers` で下の Modifier を付ける。特性の Modifier はゲームの開始から家に付き、対象の `COLLECTION_PLAYER_CITIES` は後から建てた都市・奪った都市も含む。開拓者から始める場合も、途中で建てた都市も、Modifier の付与は要らない。
- **条件**：公式の要件でプロパティを読めるのは `REQUIREMENT_PLOT_PROPERTY_MATCHES`（タイル）だけ。公式データ全体で唯一のプロパティの要件で、使用例は `DLC/CivRoyaleScenario/Data/CivRoyaleScenario_Civilizations.xml:222-225,377-403`（ユニットのいるタイルのプロパティで能力を入り切りする）。家のプロパティを読む要件は無い。家単位の要件（`REQUIREMENT_PLAYER_HAS_GOLDEN_AGE` など）は公式の年齢を読むもので、公式の年齢は止めてある（§4.2）ので使えない。そこで両方とも**都市の中心タイルのプロパティ**で入り切りする。

| 主体 | 例（§7.2） | Modifier | 条件 |
|---|---|---|---|
| 都市 | 黄金期の全都市の生産・金 +10%、寺院学問の科学 +25%・文化 −25% | 特性に都市向けの型を置く（例 `MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_MODIFIER`。寺院学問の元 `MONASTICISM_CULTURE_MODIFIER` と同じ型：`DLC/Expansion2/Data/Expansion1_Policies.xml:279-282`） | `SubjectRequirementSetId`：対象の都市の中心タイルの `JAC_AGE`（年齢）または `JAC_AGE_POLICY`（暗黒期の専用政策）が一致。都市ごとに 1 回ずつ効くのが正しい効果なので、重複の問題は無い |
| 家・ユニット | 背水の陣（近接の戦闘力・自領外の回復）、精鋭主義（経験・維持費）、海賊衆の免許（水軍の移動）、交易路の収益 | 特性に `MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER`（`Base/Assets/Gameplay/Data/Modifiers.xml:780`）を置き、各都市に家向けの型の Modifier を付ける。型は元にした公式の暗黒政策と同じ（`MODIFIER_PLAYER_DISABLE_HEALING`・`MODIFIER_PLAYER_UNITS_ADJUST_MOVEMENT`・`MODIFIER_PLAYER_UNITS_ADJUST_UNIT_EXPERIENCE_MODIFIER`・`MODIFIER_PLAYER_ADJUST_UNIT_MAINTENANCE_DISCOUNT`：`Expansion1_Policies.xml:283-285,354-357,427-432`）。都市が持つ家向けの Modifier が家のユニット全体に効くのは公式の建物と同じ（ピラミッド `Base/Assets/Gameplay/Data/Buildings.xml:498-499`、兵馬俑 `:770-771`） | 付けた Modifier の `OwnerRequirementSetId`（持ち主＝都市の中心タイル）で、`JAC_AGE_ANCHOR`（要の都市にだけ書く年齢・政策の値）が一致。`OwnerRequirementSetId` の公式の使用例：`Base/Assets/Gameplay/Data/Features.xml:289`。付ける側の `SubjectRequirementSetId` にしないのは、一度付いた Modifier が条件の変化で外れるかが分からないため |

- **要の都市**：家単位の効果は、家ごとに 1 都市（持っている都市のうちタイル番号が最小のもの）だけで入れる。全都市で入れると都市の数だけ重なる（戦闘力 +5 が都市の数だけ足されるなど）。
- **プロパティを書く関数**（ゲームプレイ側の Lua、1 つ）：家ごとに、全都市の中心タイルへ `JAC_AGE`・`JAC_AGE_POLICY` を書き、要の都市にだけ `JAC_AGE_ANCHOR` を書き、要でない都市の `JAC_AGE_ANCHOR` を消す。台帳の今の値から毎回書き直すだけなので、何度呼んでも結果は同じ（冪等）。呼ぶのは、(1) 台帳の初期化（ターン 1。開拓者から始める場合は都市が無いので書くものが無い）、(2) `CityBuilt`（新設と移管の両方。§5.2）、(3) `CityConquered`（奪った家と奪われた家の両方）、(4) 毎ターンの開始（降伏・離反などイベントの無い経路の取りこぼしを直す）、(5) 改元と暗黒期の専用政策の選択のとき。
- プロパティはタイルに残るので、破却された城のタイルにも残る。都市が無いので効果は出ない。そのタイルに都市が建てば `CityBuilt` で書き直す。
- **要実機確認**：
  1. `REQUIREMENT_PLOT_PROPERTY_MATCHES` を都市の要件（Subject・Owner）に使ったとき、都市の中心タイルを見るか。プロパティの変更に追従するか。
  2. `MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER` で都市に付けた家向けの Modifier が、`OwnerRequirementSetId` で入り切りするか。占領で都市の持ち主が変わったとき、新しい持ち主の家に効くか。
  3. 1・2 が成り立たない場合の代わり：家単位の要件 `REQUIREMENT_PLAYER_HAS_BUILDING`（公式データに 19 件）と、要の都市に置く見えない建物。置く方は、天下統一勝利（#37）がゲームプレイ側の Lua で `city:GetBuildQueue():CreateIncompleteBuilding(建物, タイル, 100)` を使い、実機で動いている（`poc/JAC_PoC_SengokuTrees/Scripts/JAC_Sengoku_RebuiltCastles.lua:52-53`、[天下統一勝利](../poc/unification-victory.md)）。消す公式の例は見つからない（`RemoveBuilding` は WorldBuilder の UI だけ：`Base/Assets/UI/WorldBuilderPlacement.lua:1255`）ので、年齢が変わったときに外せるかは実機で確かめる。都市の中心の建物は占領で消える（同 PoC）ので、占領のたびに置き直す。

### 7.2 年齢ごとの効果（暫定値）

| 年齢 | 誓約 | 暗黒期の専用政策 | その他 |
|---|---|---|---|
| **黄金期** | 1 つ（強い効果） | 使えない | 全都市の生産・金 +10%。次の閾値 +5（公式の補正） |
| **通常期** | 1 つ（達成ごとに点） | 使えない | — |
| **暗黒期** | 1 つ（達成ごとに点） | 1 つ選べる（下表） | 次の閾値 −5（公式の補正）＝立て直しの追い風。追加の罰は無い |
| **英雄期** | 3 つ（強い効果） | 使えない | 黄金期と同じ効果 |

誓約：

- 各家に固有の誓約を 3 つずつ作り、共通の誓約は置かない（オーナー判断 L-4、2026-09-30。旧 Q39）。候補はその家の 3 つで、英雄期は 3 つとも取る。例：織田家「天下布武」（強い効果：支城の占領の点 +1、全都市の軍事ユニットの生産 +15%）。22 家 × 3 = 66 の中身は別の表で作る（未実装）。
- 「天下布武」は織田家の 3 つの誓約のうち 1 つの名前にする（オーナー判断 U-1「a」、2026-09-30）。ツリーの期の家固有名「天下布武と広域戦」は元号＝時代（[`../poc/era-equals-reign.md`](../poc/era-equals-reign.md)）で無くなったので、誓約を実装するまで「天下布武」はゲーム内のどこにも出ない。誓約を作るときは、この名前を家固有名の登録（[`../poc/house-names.md`](../poc/house-names.md)）ではなく誓約の名前として持たせる。
- 旧案の共通の誓約（鉄砲と南蛮の風、検地と普請、足軽の軍備、楽市楽座、野戦と城攻め、茶の湯と寺社）は、各家の固有の誓約を作るときの主題の目安として使ってよい。
- 黄金期・英雄期は強い効果、通常期・暗黒期は達成ごとに台帳へ 1 点（例：支城の占領 1 回、技術 1 つの完了）。公式の誓約（`Expansion1_Moments.xml:511-563`：黄金は強い効果、通常・暗黒は時代スコアのクエスト）と同じ役割分担にする。

暗黒期の専用政策（改元の画面で 1 つ選び、暗黒期の間だけ効く。公式の暗黒政策の考え方を借り、名前と説明を戦国向けにする）：

| 名前（暫定） | 効果 | 代償 | 元にした公式の暗黒政策 |
|---|---|---|---|
| 背水の陣 | 近接攻撃ユニットの戦闘力 +5 | 自領の外では回復しない | `POLICY_TWILIGHT_VALOR`（`Expansion1_Policies.xml:75`） |
| 精鋭主義 | 全ユニットの戦闘経験 +100% | 軍事ユニット 1 体ごとに維持費 +2 金 | `POLICY_ELITE_FORCES`（`:80`） |
| 海賊衆の免許 | 水軍の生産 +100%、移動 +2 | 交易路の収益 −50% | `POLICY_LETTERS_OF_MARQUE`（`:78`） |
| 寺院学問 | 全都市の科学 +25% | 全都市の文化 −25% | `POLICY_MONASTICISM`（`:74`） |

公式の暗黒政策は公式の暗黒時代でしか使えない（`Policies_XP1.RequiresDarkAge`）ため、そのままは使えない。上の表は政策の画面ではなく改元の画面で選び、効果は §7.1 の条件つきの Modifier で付ける（都市の産出は都市の型、戦闘力・経験・維持費・移動・回復・交易路は家の型を要の都市で入れる）。政策の画面に並べる方法として、隠した社会制度を Lua で与えて解禁する手（`pCulture:SetCivic`、`poc/JAC_PoC_SengokuTrees/Scripts/JAC_Sengoku_StartingKnowledge.lua:49`）もあるが、暗黒期が終わったときに取り消せるかが未検証なので採らない。

## 8. 画面と演出（Civ6 と同じ見せ方）

- **改元の画面**：公式の時代の画面（`EraCompletePopup`／`EraReviewPopup`）と同じ作りで、「天文から弘治へ改元」、その家の判定結果（黄金期・通常期・暗黒期・英雄期）、誓約と暗黒期の専用政策の選択を出す。表示の元は台帳（Game プロパティ）。
- **時代スコアの画面**（`EraProgressPanel`）：台帳の点・内訳（本拠陥落、支城の占領、同盟など）・黄金期と暗黒期の閾値・次の改元までのターンを出す。公式の画面の組み立て（前の期の合計と今の期の内訳、`EraProgressPanel.lua:61-133`）に合わせる。
- 公式の時代（ツリーの 4 期）が進んだときの公式の画面では、年齢の部分を出さない（公式の年齢は止めてあり、常に「通常」になるため）。
- 家固有名の表示切り替えが同じ画面を差し替えている（`poc/JAC_PoC_BanseikiRuleset/UI/JAC_EraProgressPanel.lua`、`JAC_EraCompletePopup.lua`、`JAC_EraReviewPopup.lua`）。台帳の表示は同じ差し替えに足し、別の差し替えを作って競合させない。
- 年齢の呼び名は Civ6 のとおり「黄金期」「通常期」「暗黒期」「英雄期」（SSOT 3.2 は「黄金期」「暗黒期」とそのまま呼ぶ）。ja・en・ru を用意する。
- **時代の名前は元号**（オーナー判断 L-1、2026-09-30。旧 Q36）：通常「XX時代」（XX Era）と出る所を、今の元号で「弘治の世」「Kōji Era」「Эпоха Кодзи」と出す。公式の UI が今の時代の名前を出す所は、`GameInfo.Eras[...].Name` を `Locale.Lookup` する次の所（公式のパス）：右下の時代の目盛りの説明（`DLC/Expansion2/UI/Replacements/ActionPanel_Expansion1.lua:187-216` の `RealizeEraIndicator`、基の `Base/Assets/UI/ActionPanel.lua:343-364,1161-1168`）、時代スコアの画面の題（`DLC/Expansion2/UI/Additions/EraProgressPanel.lua:60-68`）、時代の終わりの予告（`NewTurnPanel_Expansion1.lua:11-24`）、公式の時代の画面（`Base/Assets/UI/Popups/EraCompletePopup.lua:55`、`DLC/Expansion2/UI/Additions/EraReviewPopup.lua:62`、`DedicationPopup.lua:53`）。このうち前の 2 つと改元の通知を元号にした（[実装メモ](../poc/sengoku-ages.md)）。残り（予告と時代の画面）はツリーの段階が変わる時機に出るので段階の名前のままにし、扱いは Q74。技術・社会制度ツリー（`TechTree.lua:1013`、`CivicsTree.lua:1126`）と偉人（`GreatPeoplePopup.lua:179`）の時代名は、それぞれの項目の段階を指すので変えない。
- 自動判定で見た目の成功は主張しない。画面は実装後に人が見る。

## 9. 統一勝利・約 200 ターンとの関係

- 勝利は登録した全城の支配だけ（SSOT 3.2）。台帳の点もターン数も勝利条件に入れない。年齢は勝利の**速さと接戦度**を調整する。
- 黄金期は強い効果を持つが、次の閾値 +5 と都市数の補正で先頭の家には軽いブレーキがかかる。暗黒期は閾値 −5 と専用の政策、英雄期の 3 つの誓約で弱い家に逆転の芽を残す。
- 標準速度の約 200 ターンで判定は 6 回（弘治・永禄・元亀・天正・文禄・慶長）。統一が遅れれば元和・寛永から令和まで改元ごとに判定し、2119 年（標準で 1757 ターン目）の遅代で全ての家が暗黒期になる。

## 10. 実装の順番（次の段階）

置き場は `JAC_PoC_SengokuTrees`（ルールセット条件は既存と同じ）。

1. データ：公式の年齢・誓約を止める設定（§4.2）、`ERA_RENAISSANCE` の最長ターン（§4.3）、改元の表（名前・年・月、ja/en/ru）、年齢の効果・誓約・暗黒期の専用政策の `Modifiers` と、それを付ける共通の特性 `TRAIT_JAC_AGES`（§7）。
2. ゲームプレイ Lua：台帳の初期化・ターン開始の差分・占領と建設のイベント・改元の判定・タイルのプロパティを書く関数・選択の受け付け（§5・§6・§7.1）。
3. UI Lua：改元の画面と時代スコアの画面（§8）。家固有名の差し替えに足す。
4. 自動テスト：データの断面（§4.2 の最終値：暗黒・黄金の基準値が −9999・9999、閾値の補正がすべて 0、`Eras_XP2.EraScoreThresholdShift` が全行 0、誓約の選択数・提示数が 0、`Moments.EraScore` が全行 0。型に `ERA_SCORE` を含む Modifier を参照する取り付け行（`CommemorationModifiers`・`BuildingModifiers`・`GameModifiers`・`TraitModifiers`・`PolicyModifiers` など全表）が 0 件。公式の年齢の忠誠の `TraitModifiers` 2 行が無い、`RequiresDarkAge`・`RequiresGoldenAge` の政策が 0 件。改元の表が 47 行（弘治〜令和と遅代）、Modifier がある）と、台帳の Lua を Python（lupa）で動かす単体テスト（奪取の区分、キーの重複防止、同じ城を同じターンに再奪取（A→B→A、A→B→A→B）してそのつど点になる、閾値の式、同盟の 1 期 1 回、開始時の同盟を数えない（ターン 32 の期限切れを、台帳が先に走る順と後に走る順の両方で 0 点、そのターンに外交で結び直せばどちらの順でも 2 点。プロパティが `false` になった後に残る同盟も 0 点で、エンジンが解いた後に外交で結び直せば 2 点。同じチームの組は 0 点）、出来事の点（フラグの 1 家 1 回、改元のターンのフラグは新しい期）、読み込み後に数え直さない、占領してすぐ破却した城の点と滅亡の点、占領イベントと差分で二重に数えない、移管で出る `CityBuilt` で点を付けない、開拓者から始めた家の本拠の登録、プロパティを書く関数を何度呼んでも同じで要の都市が家ごとに 1 つ）。
5. 実機：約 200 ターンの自動進行で年齢の割合を測り、数値を決める。あわせて次を確かめる。
   - 公式の時代スコアが入らない：毎ターン全勢力で今の期の公式の点（`GetPlayerCurrentEraScoreBreakdown` の合計）が 0（§4.2 の Modifier の削除が効いていること）。
   - 公式の年齢が起きない：毎ターン全勢力で `GetPlayerDarkAgeThreshold` ≤ `GetPlayerCurrentScore` ＜ `GetPlayerGoldenAgeThreshold`、公式の時代が切り替わるたびに `HasDarkAge`・`HasGoldenAge`・`HasHeroicGoldenAge` が偽（§4.2）。
   - 開始時の同盟：史実の領土の日本マップで、ターン 31 に 8 組とも「開始時の同盟」で 0 点、ターン 34 に 8 組ともプロパティが `false` で、外交で結び直していない組は同盟・友好宣言とも 0 点であること（§5.4）。
   - セーブ→読み込み：§6.1 の (1)〜(4)。都市はターン 1 からある城と、占領した城の両方を見る。

## 11. 実機確認（現状の測定）

`tools/civ6-autotest/scenarios/ages-probe.json`（日本 15 km、武田が人間・他は AI、**読み取りのみ**で 110 ターン）。公式の時代システムを止める前の現状と、エンジン側の期の長さを測る。実行：

```powershell
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/ages-probe.json --check
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/ages-probe.json --output tools/civ6-autotest/artifacts/ages-probe-01
```

- ステップ 4 は、ルールセット・22 勢力の生存と並び・`Eras_XP1` の 4 期の行を確かめ、違えば Lua のエラーで止まる（シナリオが失敗する）。
- 自動進行は 30・30・25・25 ターンに分け、公式の時代の最短ターンがこの間隔（30）以上であることをステップ 4 で確かめる。これで 2 つの記録の間に時代の切り替えは多くても 1 回になる。
- 毎回の記録（ステップ 6・8・10・12・14）で、全 22 勢力について今の期の点と内訳、**前の時代の点と内訳**（`GetPlayerPreviousEraScoreBreakdown`、公式の `EraProgressPanel.lua:73` と同じ API）、閾値、年齢を出す。`changedSinceLast=true` の記録の `prevEra` が、切り替わった時代の各勢力の確定した点になる。滅んだ家は `dead` と出す。API の呼び出しに失敗したとき、並べた生存数がエンジンの生存数と合わないときは、シナリオが失敗する。
- 最後に全 22 勢力の歴史的瞬間を集計する。
- 自動進行は人間の敗退を許す設定（`allow_human_elimination`）にしてある。

| # | 確認 | 見る出力 | 使い道 |
|---|---|---|---|
| L1 | 想定した表・値（`Eras_XP1/XP2`、`CommemorationTypes`、`Policies_XP1`、`GlobalParameters`） | ステップ 4 | §4.2 で止める値の確認 |
| L2 | Lua に時代スコアを足す・年齢を付ける API が無いこと | ステップ 5 のメソッド名一覧。`enumerated N keys` と出たオブジェクトだけ「一覧に無い＝無い」と読む。`NOT_ENUMERABLE` と出たものはメソッドを列挙できなかっただけで、有無は分からない（要実機確認のまま残す） | §1.3 の前提（あれば台帳の効果の付け方を見直す） |
| L3 | 公式の時代の長さ（`eraStart`・`minEnd`・`maxEnd`・`countdown`） | 各記録の 1 行目 | §4.3、ツリーの 4 期の長さ |
| L4 | 公式のままの点の分布（特に遭遇の点） | `prevEra` と歴史的瞬間 | 台帳の点の目安 |
| L5 | 公式の時代が `ERA_RENAISSANCE` の先へ進むか | `era`・`final`（110 ターンでは届かないので実装後に別途） | §4.3 |

## 参照

- 戦国ツリーの互換メモ：[`../poc/sengoku-tree-compatibility.md`](../poc/sengoku-tree-compatibility.md)
- ツリー設計案（4 期の定義：§4）：[`sengoku-tech-civic-tree.md`](sengoku-tech-civic-tree.md)
- 暦：[`../poc/sengoku-calendar.md`](../poc/sengoku-calendar.md)
- 天下統一勝利：[`../poc/unification-victory.md`](../poc/unification-victory.md)
- 主な公式ファイル：`DLC/Expansion2/Data/Expansion1_Moments.xml`、`Expansion1_Eras.xml`、`Expansion2_Eras.xml`、`Expansion2_GlobalParameters.xml`、`Expansion1_Leaders.xml`、`Expansion1_Modifiers.xml`、`DLC/Expansion2/UI/Additions/{EraProgressPanel,EraSupport,DedicationPopup,HistoricMoments,AllianceResearchSupport}.lua`、`DLC/Byzantium_Gaul/Data/Byzantium_Gaul_DramaticAges_*_MODE.xml`、`DLC/BlackDeathScenario/Scripts/BlackDeathScenario_StateUtils.lua`
