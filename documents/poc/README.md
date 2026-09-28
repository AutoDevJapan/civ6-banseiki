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

## JAC_PoC_GrowthUI：成長保存・手帳（人物帳）・言語表示

対象の PoC（SSOT 16.1）：束1「成長保存」、束1「UI」の Notebooks（手帳）と人物帳、束0「言語表示」。束1「UI」のスキル・Vault・Next Action の中身は対象外。

MOD の内容：

- 新規ゲームで人間プレイヤーの開始時ユニットをID順に最大3体選び、`Unit:SetProperty` にレベル・経験値・性格ID・特性ID・装備IDのリスト・Artifact IDを保存する。1体目は装備2件とArtifact 1件、2体目は装備1件を持つ。装備とArtifactは保存確認用の仮IDで、ゲーム内効果はない
- 人間プレイヤーのターン開始時に固定20 XPを加え、必要XPを `floor(30 × 現レベル^1.5 + 0.5)` としてレベルアップする。通常上限30、同じゲームターンの二重加算は防ぐ。20 XPは検証用で報酬量の仕様ではない
- 右上のバーの List of Reports のすぐ左に「手帳」ボタンを置く（SSOT 11.1）。`ReplaceUIScript` で `PartialScreenHooks` を `UI/JAC_PartialScreenHooks.lua` に差し替え、XP2 版を include して `AddScreenHooks()` の Era と Reports の間に1件足す。アイコンは公式の `LaunchBar_Hook_Timeline` を流用する
- 手帳は SSOT 11.1 の構成で、記録帳（人物帳・法律帳・宗教帳）と国家金庫（長官たち・宝物庫）を並べる。中身があるのは人物帳だけで、他は「（未実装）」付きの押せない項目。表示名は用語辞書（14-3）の3言語
- 人物帳は生存中の武将ユニットの状態を直接 `Unit:GetProperty` から表示する（読み取り専用）。日本語・ロシア語・英語の固定テスト行を常時並べる。ESC、同じボタンの再クリック、他の画面（List of Reports など）を開くと手帳は閉じる
- 以前の固定位置の「人物帳」ボタン（実機で表示されなかった）は削除した

手順：

1. 共通の準備に従い `poc/JAC_PoC_GrowthUI` を `Mods/JAC_PoC_GrowthUI` へジャンクションで配置する。この検証中は `JAC_PoC_Bundle0` を無効にし、本MODを有効にする
2. Gathering Storm のシングルプレイを新規開始し、右上のバーで List of Reports のすぐ左のボタン（ツールチップ「手帳」）を押す。アイコンが欠けていないか、左上の「歴史年表」（History Timeline）ボタンと見分けられるかも見る
3. 「人物帳」を押し、3言語の固定行と各武将の全項目を確認する。ESC で閉じることも確認する
4. 手帳をもう一度開き、開いたまま List of Reports を押す
5. 4ターン進めてセーブし、メインメニューへ戻って同じセーブをロードする
6. 手帳 → 人物帳を再度開き、セーブ前と同じ内容か確認してから1ターン進める
7. ゲームの表示言語を日本語・英語・ロシア語に順に切り替え、必要なら再起動して同じセーブを開き、各言語の文言と固定3行を確認する。ロシア語では手帳の項目名がボタンからはみ出していないかも見る

成功条件：

| 確認 | 期待する結果 |
|---|---|
| 読み込み | `Modding.log` に `JAC_GrowthUI_Text`・`JAC_GrowthUI_Gameplay`・`JAC_GrowthUI_InGameUI`・`JAC_GrowthUI_PartialScreenHooks`・`JAC_GrowthUI_UIFiles` の適用が記録され、`Database.log` に本MOD由来のエラーがない |
| ボタンの位置 | `Lua.log` に `[JAC_POC] notebooks button attached parent=ButtonStack` が出て、ボタンが List of Reports のすぐ左にある。都市国家と出会った後も同じ位置にある。この行が出ないときは置換が効いていない（`ReplaceUIScript` の競合を疑う） |
| アイコン | 欠けずに表示され、「歴史年表」ボタンと見分けられる。同じ `LaunchBar_Hook_Timeline` を流用しているため、紛らわしい・欠ける場合は 26x26 の専用 `.dds` を `ImportFiles` に追加し、`AddScreenHook` の第2引数に渡す |
| 開き直しと切り替え | 手順4で手帳が閉じて List of Reports が開き、手順3〜4の間の `Lua.log` に `UI.DataError`（`Cannot determine if partial screen`）や `Runtime Error` がない。初回に開いたときの `[JAC_POC] notebooks path lookup found=<true/false>` も記録する（`false` なら、閉じた手帳を公式の `IsPartialScreenOpen()` がパスで探せず `UI.DataError` になる。公式の Era Progress と同じ作り） |
| 手帳と人物帳 | 開くたびに `[JAC_POC] notebooks opened`、人物帳を出すたびに `[JAC_POC] character journal shown count=<n>` が出る。`n` は生存中の武将ユニット数（新規ゲームでは開始ユニット数に応じて1～3） |
| 新規ゲームの `Lua.log` | `[JAC_POC] no saved state (new game)`、`[JAC_POC] assigned unit=...` が開始ユニット数に応じて1～3件、`[JAC_POC] initialized count=...` が出る。装備・Artifactの値も記録される |
| 成長の `Lua.log` | `[JAC_POC] unit=<id> level=... xp=... personality=... trait=... equipment=... artifact=... player=... turn=...` がターンごとに出る。同じユニットで1ターンあたり20 XPを得る。Lv1の必要XPは30で、累計40 XPの時点ではLv2・残り10 XPとなる |
| ロード後の `Lua.log` | `[JAC_POC] restored unit=...` の全項目がセーブ直前の記録と一致し、`restored count=...` と再表示時の `character journal shown count=` も一致する。ロード直後に `assigned unit=` は増えず、次ターンだけ経験値が増える |
| 3言語 | 人物帳の表示が直近のログの記録と一致し、日本語・キリル文字・英語の固定3行に文字化けや欠落がない。各表示言語でボタン・見出し・項目名・性格・特性名が切り替わる。項目名がボタン幅を超えると末尾が「…」で切られ、`[JAC_POC] notebooks label truncated key=...` が出る（出たらメニューとボタンの幅を広げるか、未実装の表記を短くする） |

結果：**未実施**。前回の実機確認では、ゲームプレイ側の記録とレベルアップは動いたが、固定位置のボタンが表示されず UI は確認できなかった。今回ボタンをフックバーへ移したので、上の手順で再確認する。
