# PoC の検証記録

SSOT 16.1 の PoC のうち、実機で検証した項目の手順と結果。検証用の MOD は `poc/` にあり、本番の MOD（SSOT 15.1 の構成）とは分けている。

## 共通の準備

| 項目 | 内容 |
|---|---|
| 基準環境 | Steam 版 Civ6 1.0.12.68、Gathering Storm ルールセット、シングルプレイ |
| MOD の配置 | `Documents\My Games\Sid Meier's Civilization VI\Mods\<MOD名>` から、リポジトリの `poc/<MOD名>` へジャンクションを張る |
| ログ | `%LOCALAPPDATA%\Firaxis Games\Sid Meier's Civilization VI\Logs\` の `Database.log`・`Modding.log`・`Lua.log` |
| FireTuner | `AppOptions.txt` の `EnableTuner` を `1` にする（元のファイルは `AppOptions.txt.bak-20260928`） |

ジャンクションの作り方（PowerShell）:

```powershell
New-Item -ItemType Junction -Path "$([Environment]::GetFolderPath('MyDocuments'))\My Games\Sid Meier's Civilization VI\Mods\JAC_PoC_Bundle0" -Target "<リポジトリ>\poc\JAC_PoC_Bundle0"
```

## JAC_PoC_Bundle0：読み込みと保存

対象の PoC：束0「起動と読み込み」、束1「読み込みと保存」の2項目（16.1 の他の項目は未検証）

MOD の内容：

- `GlobalParameters` に固有の接頭辞を持つ行 `JAC_POC_BUNDLE0_VERSION` を1行追加する
- ゲームプレイ用の Lua が、ターンの開始ごとに `Game:SetProperty` で記録（初回ターン・最終ターン・観測したターン開始の回数）を保存し、ロード画面を閉じたときに読み戻して `Lua.log` に出力する

手順：

1. 追加コンテンツで「JAC PoC Bundle 0: Load and Save」を有効にする
2. Gathering Storm ルールセットで、小さいマップ・標準速度のシングルプレイを新規に始める
3. 3ターン進めてセーブし、メインメニューに戻る
4. そのセーブをロードし、1ターン進める

成功条件：

| 確認 | 期待する結果 |
|---|---|
| `Database.log` | この MOD に起因するエラーがない（ゲーム本体・オンライン配信由来の既知のエラーは記録して許容） |
| `Modding.log` | MOD が有効になり、各アクションが適用されている |
| `Lua.log`（新規ゲーム） | `loaded version=1` と `no saved state (new game)`、以降ターンごとに `turnsSeen` が増える |
| `Lua.log`（ロード後） | `restored firstTurn=… lastTurn=… turnsSeen=…` がセーブ時点の値と一致し、次のターンで `turnsSeen` が1増える |

結果：**成功**（2026-09-28、Steam 版 1.0.12.68、Gathering Storm、シングルプレイ）

| 確認 | 実際の結果 |
|---|---|
| `Database.log` | `JAC` を含むエラーなし。ゲームプレイ側の外部キー検証は `Passed Validation`。文字データの重複エラー（`UNIQUE constraint failed: LocalizedText`）が出るが、発生元は Firaxis のオンライン配信テキスト（`CurrentClickouts/…`、`LOC_CLICKOUT_26_CAROUSEL…`）で、この MOD とは無関係 |
| `Modding.log` | MOD が読み込まれ、`JAC_PoC_Bundle0_Gameplay`（UpdateDatabase）と `JAC_PoC_Bundle0_Scripts`（AddGameplayScripts）が適用された |
| `Lua.log`（新規ゲーム） | `loaded version=1`、`no saved state (new game)`、以降 `turn=1〜4` で `turnsSeen` が1〜4に増えた |
| `Lua.log`（ロード後） | `restored firstTurn=1 lastTurn=4 turnsSeen=4` でセーブ時点の値に戻り、次の `turn=5` で `turnsSeen=5` になった |

分かったこと：

- `Documents\My Games\...\Mods` に置いたジャンクション経由でも MOD は読み込まれる
- `Game:SetProperty` に保存した Lua のテーブルは、セーブ・ロードをまたいで復元される（シングルプレイで確認。マルチプレイでの同期・決定性は未検証で、本番で採用する前に確かめる）
- `print` の出力は `Lua.log` に `<MOD名>: ` 付きで記録される（束4「外部とゲームの経路」で、ゲームから外へ渡す経路の候補になる。外部プロセスからの読み取りは未検証）

## カスタム氏族と手帳 UI：自動シナリオ

結果：**成功**（2026-09-29、Gathering Storm、シングルプレイ）。`tools/civ6-autotest` で `python scenario.py scenarios/clans-notebooks.json --output artifacts/clans-notebooks-run-09` を実行し、17/17 ステップとログ判定 2/2 件が通過した。

織田氏族を人間、武田氏族を AI に設定し、両氏族の読み込みを `Lua.log` で確認した。手帳を開いた記録と画面を確認した後、autoplay でターン1から4へ進めて保存した。メインメニューに戻って同じセーブを読み込み、さらにターン4から5へ進め、手帳を再表示した。スクリーンショットは実行フォルダ内の `01-start.png`、`02-notebooks.png`、`03-turn4.png`、`04-after-load-notebooks.png`。

ゲーム内メニューと Advisor ダイアログはターン進行を止めるため、ドライバーが閉じてから autoplay を開始する。終了イベントがターン表示より先に届く場合があるため、完了時は実際のターン数と autoplay の停止、人間プレイヤーへの復帰も確認する。

## 成長 UI と手帳：自動シナリオ

結果：**成功**（2026-09-29、Steam 版 1.0.12.68、Gathering Storm、シングルプレイ）。`tools/civ6-autotest` から `python scenario.py scenarios/growth-notebooks.json --output artifacts/growth-run-13` を実行し、41/41 ステップと `growth` ログ判定 13/13 件が通過した。人間をクレオパトラ（エジプト）、AI をギルガメシュ（シュメール）、小マップ・標準速度・シード 620930 に設定した。

ゲーム開始後、Notebooks と Character Journal の開閉、ReportsList への切替、4 回のターン終了、保存・メニュー復帰・再読込、さらに 1 回のターン終了を確認した。`Lua.log` では 2 ユニットの成長値がターン 5 にレベル 2・経験値 70、再読込後も同値、ターン 6 にレベル 3・経験値 5 となった。`artifacts/growth-run-13/` に実行記録、ログ抜粋、10 枚の PNG がある。

PNG の目視確認は、上部の手帳ボタンとタイムライン、ReportsList への切替画面、Character Journal に表示された 2 ユニットの項目、および保存前・読込後・次ターンのレベルと経験値の変化まで。すべての表示条件や操作経路を網羅した確認ではない。
