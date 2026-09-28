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

## JAC_PoC_GrowthUI：成長保存・武将帳・言語表示

対象の PoC（SSOT 16.1）：束1「成長保存」、束1「UI」の武将帳部分、束0「言語表示」。束1「UI」のスキル・Vault・Notebooks・Next Action は対象外。

MOD の内容：

- 新規ゲームで人間プレイヤーの開始時ユニットをID順に最大3体選び、`Unit:SetProperty` にレベル・経験値・性格ID・特性ID・装備IDのリスト・Artifact IDを保存する。1体目は装備2件とArtifact 1件、2体目は装備1件を持つ。装備とArtifactは保存確認用の仮IDで、ゲーム内効果はない
- 人間プレイヤーのターン開始時に固定20 XPを加え、必要XPを `floor(30 × 現レベル^1.5 + 0.5)` としてレベルアップする。通常上限30、同じゲームターンの二重加算は防ぐ。20 XPは検証用で報酬量の仕様ではない
- ランチバー直下の「武将帳」ボタンから読み取り専用パネルを開く。生存中の武将ユニットの状態を直接 `Unit:GetProperty` から表示する。日本語・ロシア語・英語の固定テスト行を常時並べ、UI文言と性格・特性名は3言語で登録する

手順：

1. 共通の準備に従い `poc/JAC_PoC_GrowthUI` を `Mods/JAC_PoC_GrowthUI` へジャンクションで配置する。この検証中は `JAC_PoC_Bundle0` を無効にし、本MODを有効にする
2. Gathering Storm のシングルプレイを新規開始し、開始ユニットを失わずに「武将帳」を開く。3言語の固定行と、各武将の全項目を確認する
3. 数ターン進めて武将帳を再度開き、セーブする。メインメニューへ戻って同じセーブをロードし、武将帳を開いてから1ターン進める
4. ゲームの表示言語を日本語・英語・ロシア語に順に切り替え、必要なら再起動して同じセーブを開き、各言語のUI文言と固定3行を確認する

成功条件：

| 確認 | 期待する結果 |
|---|---|
| 読み込み | `Modding.log` に `JAC_GrowthUI_Text`・`JAC_GrowthUI_Gameplay`・`JAC_GrowthUI_InGameUI`・`JAC_GrowthUI_UIFiles` の適用が記録され、`Database.log` に本MOD由来のエラーがない |
| 新規ゲームの `Lua.log` | `[JAC_POC] no saved state (new game)`、`[JAC_POC] assigned unit=...` が開始ユニット数に応じて1～3件、`[JAC_POC] initialized count=...` が出る。装備・Artifactの値も記録される |
| 成長の `Lua.log` | `[JAC_POC] unit=<id> level=... xp=... personality=... trait=... equipment=... artifact=... player=... turn=...` がターンごとに出る。同じユニットで1ターンあたり20 XPを得る。Lv1の必要XPは30で、累計40 XPの時点ではLv2・残り10 XPとなる |
| ロード後の `Lua.log` | `[JAC_POC] restored unit=...` の全項目がセーブ直前の記録と一致し、`restored count=...` も一致する。ロード直後に `assigned unit=` は増えず、次ターンだけ経験値が増える |
| 武将帳と3言語 | 開くたびに `[JAC_POC] ui opened` が出る。UI表示は直近のログの記録と一致し、日本語・キリル文字・英語の固定3行に文字化けや欠落がない。各表示言語でボタン・見出し・項目名・性格・特性名が切り替わる |

結果：**未実施**。実機での `Unit:SetProperty` のテーブル保存、ゲームプレイ側ターンイベント、UI側読み取りと3言語表示はこのPoCで確認する。
