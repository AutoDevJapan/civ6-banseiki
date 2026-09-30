# 元号＝時代：エンジンの時代の長さの事前検証（プローブ）

オーナー判断 T「元号と時代を同義にしたい」：改元のたびに Civ6 の時代（`Game.GetEras():GetCurrentEra()`）が実際に変わるようにする。本実装の前に、エンジンが時代をどう進めるかを実機で確かめる。本書はその第1段階（プローブ）の記録。

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

状態：プローブを用意しただけで未実行（2026-09-30）。実行後、結果と案A への反映をここに書く。
