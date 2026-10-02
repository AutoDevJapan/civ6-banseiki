# 戦国パックの暦（PoC）

## 適用範囲と方針

SSOT 3.1 の開始時点は **天文3年5月12日**、3.2 の進行目安は標準速度で約200ターン後に1600年ごろ。`JAC_PoC_SengokuTrees` が有効で `RULESET_JAC_BANSEIKI` のゲームを始めたときだけ、`ERA_ANCIENT` の開始年を1534にし、暦の進行速度を差し替える。他のルールセットの暦は変えない。

ゲーム内の暦表示は当面 **1534年からの年表示**とする（**暫定**）。天文3年5月12日はシナリオの史実上の初期状態であり、Civ6 の暦に5月12日として入力したわけではない。和暦の日付を西暦の5月12日とみなす変換もしない。元号表示は追加しない。

## エンジン根拠

以下のパスはすべて `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/` 以下の公式ファイル。

- `Base/Assets/Gameplay/Data/Eras.xml:25-29` では `StartEras` の `ERA_ANCIENT` に `Year="-4000"` が入る。`Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:2545-2560` によると `StartEras` の暦の起点は整数 `Year` で、月日を指定する列はない。
- `Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:1403-1408` に `GameSpeed_Turns(GameSpeedType, MonthIncrement, TurnsPerIncrement)` がある。`Base/Assets/Gameplay/Data/GameSpeeds.xml:17-61` は速度ごとのコスト倍率、同ファイル `:64-103` と `:141-179` は既定の暦進行例。`Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:3337` は速度別の暦区間を行順に取得する。
- 公式の戦争シナリオは `DLC/WarMachineScenario/Data/WarMachineScenario_GameplayData.xml:380-395` で `StartEras.Year` を1914に更新し、同ファイル `:642-649` で `GameSpeed_Turns` を削除・再登録する。同シナリオの `DLC/WarMachineScenario/Data/WarMachineScenario_Config.xml:23-27,51-54` はルールセット専用 `GameCalendar` ドメインで `CALENDAR_STANDARD_FROM_START_ERA` を選ぶ。戦国パックもこの方式を採用した。同ファイル `:33` は `RulesetSupportedValues` の `StandardEras` で開始時代を1つに絞っており、戦国パックも `JAC_Sengoku_Calendar.xml` で `RULESET_JAC_BANSEIKI` の開始時代を `ERA_ANCIENT` だけにする（開始年1534は `ERA_ANCIENT` にだけ設定しているため、他の時代を選ぶと1534年開始にならない）。`GameStartEra` のドメインは `Base/Assets/Configuration/Data/SetupParameters.xml:10` の `StandardEras`。ルールセット本体の `GAME_START_ERA` は `ERA_ANCIENT` で競合しない。
- 基本設定の `GameCalendar` は `Base/Assets/Configuration/Data/SetupParameters.xml:43` で非表示・既定 `CALENDAR_STANDARD`。Gathering Storm のゲーム上部の年表示は `DLC/Expansion2/UI/Replacements/TopPanel.lua:383-401` で現在ターンと `Calendar.MakeYearStr(turn)` を使う。`Base/Assets/Gameplay/Data/Calendar.xml:5-10` には月単位カレンダーもあるが、開始月日を設定する列はない。`DLC/BlackDeathScenario/Data/BlackDeathScenario_Config.xml:24,35-38` は月表示の公式例。
- ライブシナリオの開始ターン判定は `DLC/Expansion2/UI/Additions/DedicationPopup.lua:37` と同じ `GameConfiguration.GetStartTurn()`、都市建設は `Base/Assets/UI/Panels/UnitPanel.lua:2636-2641` と同じ `UnitManager.RequestOperation(..., UnitOperationTypes.FOUND_CITY)` を使う。`Base/Assets/UI/GreatWorksSupport.lua:20` は `GameConfiguration.GetCalendarType()` の使用例。

## 進行曲線

`GameSpeed_Turns` は戦国用に全速度を入れ替える。`TurnsPerIncrement` は合計10000ターン分を用意し、約200ターン後も暦が進むようにする。ゲームの時間切れ禁止は既存の `JAC_Sengoku_Victories.xml` の設定を維持する。`Data/JAC_Sengoku_Calendar.xml` は `tools/civ6-data-rs/src/reigns.rs` の生成物（`--check` で古さを検査）。

標準速度は4か月/ターン。標準以外は、元号＝時代（[era-equals-reign.md](era-equals-reign.md)）のエンジンの時代の始まりに改元の年月が来るように、元号ごとに1ターンの月数を変える（オーナー判断 U-2b、2026-09-30）。エンジンは区間を行の順に使う（`01_GameplaySchema.sql:3337`）ので、区間は増減を繰り返してよい。

| 速度 | 暦の進み方 | 1600年の最初のターン |
|---|---|---:|
| Online | 元号ごとに3〜12か月（遅代の後は8か月） | 97 |
| Quick | 元号ごとに2〜10か月（遅代の後は6か月） | 131 |
| Standard | 4か月/ターン | 199 |
| Epic | 元号ごとに1〜5か月（遅代の後は3か月） | 293 |
| Marathon | 元号ごとに1〜4か月（遅代の後は1か月） | 584 |

## メインセッションの実機確認

`cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-autotest -- tools/civ6-autotest/scenarios/sengoku-calendar.json --check` は構文検証のみ。実機では同シナリオを実行し、次を確認する。

1. FrontEnd で戦国パックのカレンダーが `CALENDAR_STANDARD_FROM_START_ERA` に切り替わり、開始時代の選択肢が `ERA_ANCIENT` だけになる。
2. 開始ターンで `GameInfo.StartEras['ERA_ANCIENT'].Year == 1534`、`Calendar.MakeYearStr` の文字列が1534年を含む。
3. 標準速度で3ターン進めた後、同じ関数の文字列が1535年を含む。これは年の進行と区間順の短い実機確認であり、200ターンのプレイバランスは検証しない。

未検証: UI の TopPanel 実表示、ロード後の表示、長期ターンでの到達年、ほかのルールセットへの非適用。`Calendar.MakeYearStr` の起点計算はゲームエンジン内部なので、上記シナリオの実測を待つ。

## オーナー確認事項

- 年表示だけで開始時点を表し、天文3年5月12日を仕様・イベントの基準日として保持してよいか。月日までの表示が必要なら、和暦の日付・閏月とゲーム内ターンとの対応規則、および表示対象画面を決める。
