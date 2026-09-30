# 元号＝時代：エンジンの時代の長さのプローブと本実装

オーナー判断 T「元号と時代を同義にしたい」：改元のたびに Civ6 の時代（`Game.GetEras():GetCurrentEra()`）が実際に変わるようにする。本実装の前にエンジンが時代をどう進めるかを実機で確かめ（プローブ）、その結果で本実装した。本書はその両方の記録。

## 本実装の案（案A）

1. 公式の9つの `Eras` の ID はそのまま残し、名前を最初の9つの元号に変える。残りの元号は `ERA_JAC_<ID>` の行を足す（`ChronologyIndex` 10 以降。公式は `Base/Assets/Gameplay/Data/Eras.xml:16` の `ERA_ANCIENT` が 1）
2. `Eras_XP1` の `GameEraMinimumTurns` = `GameEraMaximumTurns` = その元号の長さ（ターン）。最後の時代は両方 NULL（終わらない）。公式は `ERA_FUTURE` 以外 40/60、`ERA_FUTURE` は NULL（`DLC/Expansion2/Data/Expansion1_Eras.xml:3-13`）。どちらの列も NULL 可（`DLC/Expansion2/Data/Expansion2_Schema.sql:293-299`）
3. `NEXT_ERA_TURN_COUNTDOWN` を 1 にする（公式 10：`DLC/Expansion2/Data/Expansion2_GlobalParameters.xml:204`）
4. 足した時代にも `Eras_XP2`・`RandomAgendasInEra` の行を写す。技術・社会制度の `EraType` は年代順に割り当てる
5. 武功の台帳は `GetCurrentEra()` が変わったときに判定する

このうち 2・3 がエンジン上どう働くか（最小＝最大で時代がちょうどその長さで変わるか、カウントダウン 1 の意味、速度で長さが伸び縮みするか、研究の進み具合で早まるか）は資料で確かめられないため、プローブで確かめる。UI は残りターンを `GetNextEraCountdown() + 1` で出す（`DLC/Expansion2/UI/Additions/EraProgressPanel.lua:100`）。

## プローブの構成

| 項目 | 内容 |
|---|---|
| テスト専用 MOD | `poc/JAC_Test_EraTiming`（`57a3af09-fb38-41da-b367-818de11a995c`）。万世紀ルールセットかつ天下統一の検証地図（`JAC_Test_UnificationMap`）のときだけ効く。戦国ツリー（LoadOrder 200）の後に LoadOrder 400 で上書きする |
| 時代の長さ（最小/最大） | 古代 3/3、古典 5/5、中世 1/1、ルネサンス 2/2、産業 2/4（最小＜最大の観察用）、以降は公式の 40/60。`NEXT_ERA_TURN_COUNTDOWN` = 1 |
| シナリオ | `tools/civ6-autotest/scenarios/probe-era-timing-standard.json`（標準速度）と `probe-era-timing-quick.json`（迅速。公式の `CostMultiplier` 67：`Base/Assets/Gameplay/Data/GameSpeeds.xml:49`）。違いは速度だけ |
| 地図・勢力 | 天下統一の検証地図（DUEL・3家）、織田が人間・伊達と蘆名が AI。15 km 日本地図より読み込みが速い |
| 進め方 | 1 ターンずつ 16 回自動進行し、毎回 `GameCore_Tuner` と `InGame` の両方で記録する |
| 記録 | `Lua.log` の `[JAC_ERA_PROBE]` 行（ステップの戻り値にも同じ文字列）。ターン、経過ターン（`現在 - GameConfiguration.GetStartTurn() + 1`）、暦の日付、現在の時代（番号と `EraType`）、`GetCurrentEraStartTurn`、`GetNextEraCountdown`、`GetCurrentEraMinimumEndTurn`、`GetCurrentEraMaximumEndTurn`、`GetFinalEra`、各家の時代（`Player:GetEra()`）と研究済みの技術・社会制度の数。時代が変わった行には `CHANGED from=<前の時代> eraLenObserved=<その時代のターン数>` が付く |

getter は1つずつ `pcall` で呼び、無いものは `absent`、失敗は `ERR(...)` と書いて先へ進む（GameCore にどの getter があるかも結果の一部）。例外として、`InGame` の最初の手順は前提が崩れていれば止める：ルールセット、テスト MOD の `Eras_XP1` と `NEXT_ERA_TURN_COUNTDOWN` が効いていること、指定の速度、開始日が1534年（戦国の暦）。前提が崩れた記録は読み違いのもとになるため。

このテスト MOD は、`enable_mods` がほかの MOD を無効にしないため、一度有効にすると残る。影響を検証地図に限り、同じ地図を使う `unification-victory.json`・`unification-pact.json` は `disable_ids` でこの MOD を切る（`tests/test_era_timing_probe.py` が検査する）。

## 準備（オーナー）

ジャンクションを1つ張る（フォルダ名 `JAC_Test_EraTiming`）：

```powershell
New-Item -ItemType Junction -Path "$([Environment]::GetFolderPath('MyDocuments'))\My Games\Sid Meier's Civilization VI\Mods\JAC_Test_EraTiming" -Target "<リポジトリ>\poc\JAC_Test_EraTiming"
```

## 実行

```powershell
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/probe-era-timing-standard.json --output tools/civ6-autotest/artifacts/probe-era-timing-standard-01
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/probe-era-timing-quick.json --output tools/civ6-autotest/artifacts/probe-era-timing-quick-01
```

## 読み方（結果で決まること）

| 問い | 見る値 | 案A どおりなら |
|---|---|---|
| 最小＝最大でちょうどその長さで変わるか | 各 `CHANGED` の `eraLenObserved`、`eraStart` | 標準速度で古代 3・古典 5・中世 1・ルネサンス 2（`eraStart` が 1→4→9→10→12） |
| カウントダウン 1 の意味 | `countdown` の推移、`CHANGED` の直前の値 | 変わる前のターンで 0（UI 表示は「1 ターン」）。時代が1ターン遅れて変わるなら、長さからの差し引きが要る |
| 速度で長さが変わるか | 迅速の `eraLenObserved` | 同じ値なら元号の長さは速度ごとに換算して入れる。67% で縮むなら標準の値を入れる |
| 研究の進み具合で早まるか | 産業（2/4）が 2〜3 ターンで終わるか、各家の `era` | 4 ターンで終わるなら最大が効く。最小＝最大にする案A では研究に左右されない見込み |
| GameCore から読めるか | GameCore 行の `absent` / `ERR(` | 台帳（ゲームプレイ Lua）が使える getter の一覧 |

## プローブの結果（2026-09-30）

記録：`tools/civ6-autotest/artifacts/era-equals-reign-01`（標準）と `-02`（迅速）の `Lua.log` の `[JAC_ERA_PROBE]` 行（live 作業フォルダ）。

| 問い | 結果 |
|---|---|
| 最小＝最大でちょうどその長さで変わるか | 変わる。標準で 3→3、5→5、2→2 ターン。1 を入れると 2（最短は 2 ターン）。`eraStart` + 長さ = `maxEnd` の手番に次の時代になる |
| カウントダウン 1 の意味 | 変わる前の手番だけ `countdown` = 0（UI は「1 ターン」）。それ以外は −1 |
| 速度で長さが変わるか | 変わる。迅速（`CostMultiplier` 67）は 3→2、5→3、1→2、2→2、2/4→2。`floor(長さ × 67 / 100)` で、2 未満は 2（`minEnd`・`maxEnd` もこの値） |
| 研究の進み具合で早まるか | 2/4 の時代は標準で 4、迅速で 2（= floor(4 × 0.67)）で変わり、研究では早まらなかった（短い試験の範囲） |
| GameCore から読めるか | `Game.GetEras():GetCurrentEra()` は GameCore（`GameCore_Tuner`）でも読め、時代の切り替わりも InGame と同じ手番で見えた。`GetCurrentEraStartTurn`・`GetNextEraCountdown`・`GetCurrentEraMinimumEndTurn`・`GetCurrentEraMaximumEndTurn`・`GetFinalEra` は GameCore に無い（`absent`）。GameCore の行が壊れて見えたのは `Calendar` が GameCore に無く、日付の欄の `ERR(...)` に改行入りのスタックトレースが入ったため |
| 各家の時代（`Player:GetEra()`） | 研究で決まり、ゲームの時代とは別に動く（試験中は全家 0 のまま） |

## 本実装（オーナー判断 T、2026-09-30）

### 時代の表（`Data/JAC_Sengoku_Eras.sql`）

`tools/generate_sengoku_reigns.py` が改元の表（`documents/spec/data/sengoku-reigns.json`）から作る。

1. 天文から遅代までの 48 の元号を 1 つずつ時代にする。公式の 9 つの時代は ID を残して最初の 9 つの元号（天文・弘治・永禄・元亀・天正・文禄・慶長・元和・寛永）の名前にする（ID を消すと参照する行が連鎖して消える）。残りの 39 は `ERA_JAC_<ID>`（`ChronologyIndex` 10〜48）を足す
2. 時代の名前は `LOC_JAC_ERA_<ID>_NAME`（「弘治の世」「Kōji Era」「Эпоха Кодзи」、台帳の画面の `LOC_JAC_AGE_ERA_NAME` と同じ形）。説明も同じキー。公式の書式は `LOC_GAME_ERA_DESC` = `{1_EraName}`、予告は「{2_EraName}終了まで{1}ターン」なので、そのまま元号の名前が出る
3. `Eras_XP1` の最小＝最大＝その元号の標準速度のターン数（改元の月を含む暦のターンの差。天文 65、弘治 7、永禄 37 …）。遅代は NULL/NULL で終わらない。以前の `ERA_RENAISSANCE` の最長 10000 は消した。`NEXT_ERA_TURN_COUNTDOWN` = 1
4. 標準速度で 2 ターン未満の元号は無い（最短は万延の 3 ターン）。2 ターン未満の元号が表に入ったら、生成器は元号をまとめずに止まる（まとめると「元号＝時代」が崩れ、画面の名前の出し方を別に決める必要があるため、その時にオーナーに確認する）
5. 足した時代の `Eras` の残りの列（偉人の基礎費用・戦争屋の点など）と `Eras_XP1.LiberatedEnvoys`・`Eras_XP2` の行は、公式の最後の時代 `ERA_FUTURE` の値を写す

### 公式の表との関係

時代を参照する公式の表（`01_GameplaySchema.sql`・`Expansion1_Schema.sql`・`Expansion2_Schema.sql`）を洗い出した。公式の ID は残すので、どの行も消えない。

| 表 | 扱い |
|---|---|
| `Eras_XP1`・`Eras_XP2` | 全 48 時代に行がある（上の 3・5） |
| `Technologies`・`Civics`（`EraType`） | 下の「ツリーの時代」 |
| `GreatPersonIndividuals`・`GreatWorks`（`EraType`） | 公式の産業〜未来の時代の偉人は、そのままでは天正〜寛永（1573〜1644年）に出てくる。オーナー判断 U-3b（2026-09-30）で戦国パックには出さない：`Types` の行を消し、`GreatPersonIndividuals`（`01_GameplaySchema.sql:1519,1566`）からその偉人の作品（`GreatWorks`）・効果（`GreatPersonIndividualActionModifiers`・`BirthModifiers`）・近代の偉人メアリー・キャサリン・ゴダードの情報源（`DiplomaticVisibilitySources`）まで外部キーで連鎖して消える（`:1012,1573,1579,1598`、いずれも `ON DELETE CASCADE`）。公式の 178 人（`Base/Assets/Gameplay/Data/GreatPeople_*.xml`）のうち 103 人が消え、古典〜ルネサンスの 75 人が残る。公式の大音楽家は全員が産業以降なので、音楽家の偉人はいなくなる（公式のゲームで産業時代より前に候補がいないのと同じ状態）。偉人を持たない産業の遺物 5 つ（`GREATWORK_ARTIFACT_21`〜`25`、`GreatWorks.xml`）だけは、公式の時代の開始年（産業 1725）の元号＝享保へ移す。シナリオ・テストに消えた偉人を前提にするものは無い（`GREAT_PERSON_INDIVIDUAL` を参照するのは公式の偉人の時代スコアの効果を消す行だけ） |
| `RandomAgendasInEra`・`RandomAgendaEraTags` | 公式は `ERA_ANCIENT` の行だけ（`Expansion2_Leaders.xml`）。開始時に 1 回使うだけなので足さない |
| `StartEras`・`StartEras_XP2`・`MajorStartingUnits`・`BonusMinorStartingUnits`・`StartingBuildings`・`StartingCivics`・`StartingBoostedTechnologies`・`StartingBoostedCivics`・`StartingGovernments` | 開始の時代の行。戦国パックは `ERA_ANCIENT`（天文）開始だけなので足さない |
| `HeroClassProgressions`・`HeroClasses.DiscoveryMinEraType` | 英雄モード（Babylon）だけ。日本マップでは使わない。10番目以降の時代の行が無いときのエンジンの扱いは未確認（英雄モードを使うなら要実機確認） |
| `Moments`・`CommemorationTypes`・`Policies_XP1`・`GovernorPromotionConditions`・`Resolutions` | 公式の ID を指すので残る。瞬間は 0 点、誓約・公式の暗黒期の政策・総督は止めてある。世界会議の決議の時代の範囲は元号の順で早まる（日本マップは世界会議なし） |
| `Routes.PrereqEra`・`Buildings.ObsoleteEra`・`Resources.PeakEra`（外部キー無し） | 公式の ID のまま。道の格上げ（中世＝永禄、産業＝天正、近代＝文禄）は元号の順で早まる。公式の不思議は戦国パックでは作れない（`InternalOnly`）ので廃れの時代は効かない |
| `Gossips.ErasUntilObsolete`・`DiplomaticActions.RequiresLeadXEras`（数） | 時代の数で数えるので、元号が短いと早く古くなる・早く条件を満たす（要観察） |
| `Eras_XP2` の不満の減り方・交易路の最短ターン | 公式の時代の値のまま進むので、公式より早い手番で値が変わる（永禄 73 ターンで交易路 +10、天正 119 ターンで +20）。オーナー判断 U-4a（2026-09-30）：道の格上げを含め、時代で変わる公式の値はこのままにし、約200ターンの自動進行で見て判断する |

### 速度と暦（オーナー判断 U-2b、2026-09-30）

エンジンは `Eras_XP1` のターンを速度の `CostMultiplier`（`Base/Assets/Gameplay/Data/GameSpeeds.xml`：オンライン 50・迅速 67・標準 100・叙事詩 150・マラソン 300）で縮め伸ばしする（切り捨て、最短 2）。以前の暦（迅速 6・オンライン 8 か月／ターン、叙事詩・マラソンは 2 段階）では、改元の月を含む暦のターンとエンジンの時代の始まりが、迅速・オンラインで最大 −13・−14 ターン、叙事詩で −793、マラソンで −1556 ターンずれていた。

標準以外の暦は、`tools/generate_sengoku_reigns.py` が改元の表とエンジンの時代の長さから作る（`Data/JAC_Sengoku_Calendar.xml` も生成物になった）。

1. 各時代の最初のターンの日付は、その改元の月か、その 2 か月前まで（そのターンが改元の月を含む）。遅代は改元の月ちょうど
2. 時代の間のターンは、その元号の月数をなるべく均等に分ける（1 ターンの月数は隣どうしで 1 か月違いの 2 種類を交互に。長い元号は約 8 ターン以上の区切りごとに交互）
3. `GameSpeed_Turns` の主キーは（速度、`MonthIncrement`、`TurnsPerIncrement`）（`01_GameplaySchema.sql:1403-1408`）なので、同じ組を 2 行にできない。生成器は同じ組になる行を 2 つに割り、それでも無理なら元号の分け方を変える（乱数の種を固定した貪欲法で、出力は毎回同じ）。1 ターンは 12 か月以下（年が飛ばない）
4. 遅代の後（10000 ターンまで）は、標準の 4 か月を倍率で割った月数（オンライン 8・迅速 6・叙事詩 3・マラソン 1）
5. 標準速度は 4 か月／ターンのまま（SSOT 3.2 の「約200ターンで1600年ごろ」。時代の長さはこの暦から決まる）

改元の月を含む暦のターンとエンジンの時代の始まりの差（エンジン − 暦）：

| 速度 | 行数 | 1600年まで | 全体 | 最初のターンが改元の月より前（最大、時代の数） | 1600年の最初のターン | 元号の途中の日付の偏り（1600年まで／全体、月） | 1 ターンの月数 |
|---|---:|---:|---:|---|---:|---|---|
| オンライン | 62 | 0 | 0 | 2 か月、24 | 97 | 5.4／31.3 | 3〜12 |
| 迅速 | 66 | 0 | 0 | 2 か月、17 | 131 | 3.8／29.9 | 2〜10 |
| 標準 | 1 | 0 | 0 | 3 か月、33 | 199 | 3.0／3.0 | 4 |
| 叙事詩 | 99 | 0 | 0 | 2 か月、10 | 293 | 10.2／28.5 | 1〜5 |
| マラソン | 126 | 0 | 0 | 1 か月、9 | 584 | 14.8／33.3 | 1〜4 |

「偏り」は、元号の中のターンの日付が均等に分けた場合から最大何か月ずれるか（改元の日付そのものは上の表のとおり合う）。長い元号（昭和・令和など）では行数の制約で 2〜3 年ずれる。1600年の最初のターンは以前の目安（100・134・300・600）より少し早い（エンジンが各時代の長さを切り捨てるため）。

台帳・史実イベント・資源の年（`JAC_HistoricalEvents.lua`・`JAC_CourtArbitration_Rules.lua`・`JAC_Resources.lua`）は、速度の行をエンジンと同じ行の順に読む。以前は `MonthIncrement` の大きい順に並べ替えていたが、新しい暦では増減が交互になり並べ替えで順を戻せないため、ナビゲーションプロパティ `GameInfo.GameSpeeds[速度].GameSpeedTurnCollection`（`ORDER BY rowid`、`01_GameplaySchema.sql:3337`。公式の Lua も `leader.TraitCollection` の形で使う：`Base/Assets/UI/MapTacks.lua:172`）を読む。

**要実機確認**：叙事詩・マラソンの倍率（150・300）での時代の長さ、`GameSpeedTurnCollection` がゲームプレイの Lua から読めること、叙事詩・マラソン・迅速・オンラインで改元の画面とトップパネルの年が合うこと。

### ツリーの時代

各列を、設計の 4 期（1534〜1542・1543〜1567・1568〜1581・1582〜1600 年、[ツリー設計](../research/sengoku-tech-civic-tree.md) §1・§4）の年を列で3等分した中央の年月の元号に入れる（`tools/generate_sengoku_trees.py` の `column_era`）。列 1〜5 は天文（列5 は 1555年7月）、6 は永禄（1563年11月）、7 は元亀（1570年5月）、8〜11 は天正、12 は慶長（1597年11月）。ツリーの画面は技術・社会制度のある時代だけを並べる（`Base/Assets/UI/Screens/TechTree.lua:1452-1476`）ので、見出しは天文・永禄・元亀・天正・慶長の5つ。公式の研究費の補正（今の時代より後の時代の項目 +20%、前の時代の項目 −20%。`Expansion2_GlobalParameters.xml:48-50,244-246`）は元号で効く。各家の時代（`Player:GetEra()`）も研究した項目の元号で進む。

4 期の名前（領国の基盤・技術交流と領国競争・統一への広域戦・統一事業）と、織田の家固有名「天下布武と広域戦」は無くなった（[家固有名](house-names.md)）。「天下布武」は、誓約を実装するときに織田家の固有の誓約3つのうち1つの名前にする（オーナー判断 U-1「a」、2026-09-30。SSOT 3.2、[時代の設計](../research/sengoku-ages.md) §7.2）。それまではどこにも表示しない。今、戦国パックに登録した家固有名は無い。

### 武功の台帳と画面

1. 台帳（`Scripts/JAC_Sengoku_Ages.lua`）は、改元の行（`JAC_SengokuReigns.EraType`）の時代が始まる手番に判定する。始まりの手番は、エンジンと同じ `Eras_XP1` の長さを速度の倍率で切り捨てて足したもの（最短 2）。加えて、ゲームプレイ側で読める `Game.GetEras():GetCurrentEra()` がその時代に達していたら、その手番で判定する（手番の始まりの出来事がエンジンの時代の切り替えより前に来ても、倍率の計算が外れても判定が遅れない）。どちらも全端末で同じ値なので、マルチプレイでもずれない。判定の記録（`history.rN.engineEra`）とログにエンジンの時代を残す
2. 同じ手番に2つの改元が重なることは無くなった（どの時代も 2 ターン以上）ので、重なった改元をまとめる処理は消した
3. 公式の新しい時代の画面（`EraCompletePopup`）は出さない。戦国パックの `UI/JAC_EraCompletePopup_Sengoku.lua` が、万世紀ルールセットの差し替え（`UI/JAC_EraCompletePopup.lua`）のワイルドカードから読まれ、公式の `StartEraShow` を空にする（`DLC/Expansion2/UI/Additions/EraCompletePopup.lua:64,68-113`。10番目以降の時代には公式の絵が無く、出すと DataError になる：同 :75-78）。改元の画面（`UI/JAC_SengokuReignPopup.lua`）が代わりに出る
4. 時代の終わりの予告（`NewTurnPanel_Expansion1.lua:11-24`）はそのまま残り、改元の前の手番に「天文の世終了まで1ターン」と出る。右下の時代の目盛り・シヴィロペディア・ツリーの時代名は `Eras.Name` から元号の名前になる

### 検査

- `python tools/generate_sengoku_reigns.py --check`（暦 `Data/JAC_Sengoku_Calendar.xml` も含む）、`python tools/generate_sengoku_trees.py --check`
- `tests/test_sengoku_ages.py`：公式のデータに重ねた時代の表（48、年代順、各 2 ターン以上、標準速度で改元の手番 = 時代の始まり、遅代は終わらない、`NEXT_ERA_TURN_COUNTDOWN` = 1、ツリーの時代が列の順、産業〜未来の偉人が無く連鎖先も残らない、遺物は享保、外部キー）、全速度で改元の手番＝時代の始まり（差 0、最初のターンは改元の月の 2 か月前まで、暦の行の主キーが重ならない）、台帳（エンジンの時代が変わった手番の判定、全速度の判定の手番が生成器と同じ）
- `tests/test_sengoku_trees.py`：列の元号、`tests/test_sengoku_ages_screens.py`：公式の新しい時代の画面を出さない
- 実機シナリオ `tools/civ6-autotest/scenarios/sengoku-era-reigns.json`（15 km 日本地図、標準速度、75 ターン）：時代の表、ツリーの見出し、エンジンの時代が弘治（66）・永禄（73）ちょうどで変わること（GameCore と InGame）、台帳の判定、改元の画面が1回ずつ出て公式の画面は出ないこと、公式の時代スコアと年齢が止まったままのこと。改元の画面は、`JAC_REIGN_POPUP_LAST` で出た改元（元号・ターン・年齢・点が台帳どおり）を確かめ、自動進行の間にドライバが閉じていれば（`scenario.py` の `POPUP_CLOSERS`）`JAC_ShowReignPopup(true)` で出し直してから、見出し・年齢・点の文を確かめて閉じる

実機 `era-equals-reign-03`（2026-09-30）：エンジンの時代は弘治（66）・永禄（73）ちょうどで変わり、台帳は両方を判定し、公式の画面は出なかった。永禄の改元の画面の確認だけが「画面が開いていない」で失敗した（弘治では画面が開いたまま残っていたが、永禄では自動進行の間にドライバが閉じた）。上の出し直しに直した。

**要実機確認**：`sengoku-era-reigns.json` の再実行（改元の画面の確認以降）。48 の時代の読み込み・保存・AI の動き。叙事詩・マラソンの倍率。10番目以降の時代での公式 UI（時代スコアの画面・ツリーの背景画像は `ERA_FUTURE` の絵を写した）。
