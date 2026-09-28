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

## JAC_PoC_CustomCivs：カスタム勢力

対象の PoC：SSOT 16.1 束1「カスタム勢力」。別 Civilization ID の同時参加、AI の独立行動、ゲーム途中の家名表示変更を調べる。言語表示も束0の一部として確認する。

MOD の内容：

- `CIVILIZATION_JAC_ODA`／`LEADER_JAC_ODA_NOBUHIDE` と `CIVILIZATION_JAC_TAKEDA`／`LEADER_JAC_TAKEDA_NOBUTORA` を追加する。勢力能力・指導者能力は効果なしの検証用
- `Players:Expansion2_Players` に両組を登録する。日本の既存アイコン・北条の指導者画像・ロード画像・色を流用し、色の組合せで両家を区別する
- 日英露の名前・説明・基本外交文を登録する。都市名は日本の既存一覧を共有し、武田の首都「甲府」を追加する
- ゲームプレイ Lua が全プレイヤーの ID・文明・指導者・生存をログに記録する。各ターンには両家の都市数を記録する。3ターン目に武田家だけ「甲斐武田家」への表示名変更を試し、変更前後と文明 ID を出力する

手順：

1. 共通の準備に従って `JAC_PoC_CustomCivs` を Mods に配置・有効化し、Gathering Storm の詳細設定で織田家を人間、武田家を AI に選んで開始する
2. 武田 AI の都市建設・ユニット行動を確認し、3ターン目を越えてセーブする。メニューへ戻り、ロード後の表示名と `Lua.log` を確認する
3. 別ゲームで既存の文明を人間にし、織田家と武田家をともに AI に指定する。両者の行動と `Lua.log` を確認する
4. ゲーム言語を英語・日本語・ロシア語に切り替えて、新規ゲーム設定画面とゲーム中の両家の名前を確認する

成功条件（`Lua.log` の各行に `[JAC_POC]` が付く）：

| 確認 | 期待するログ・画面 |
|---|---|
| 同時参加 | 異なる `player=<id>` の `civ=CIVILIZATION_JAC_ODA leader=LEADER_JAC_ODA_NOBUHIDE alive=true` と `civ=CIVILIZATION_JAC_TAKEDA leader=LEADER_JAC_TAKEDA_NOBUTORA alive=true`、`customCivs oda=1 takeda=1` |
| AI の独立性 | 両 AI を選んだゲームでも2行が別 ID で出る。`turn=… player=… civ=… cities=…` が各家で継続し、ゲーム画面で両者それぞれの都市建設・ユニット行動を確認できる |
| 表示名変更 | `rename turn=3 … result=write_accepted … civBefore=CIVILIZATION_JAC_TAKEDA civAfter=CIVILIZATION_JAC_TAKEDA … shortAfter=LOC_JAC_TAKEDA_RENAMED_NAME`。画面では「甲斐武田家」（他言語では対応訳）に変わり、織田家は変わらない。`SetValue` 経由なら `nameAfter=LOC_JAC_TAKEDA_RENAMED_NAME` も確認する |
| セーブ・ロード | 再読込後の `renameState stage=load … attempted=true` で変更後の `name` と `short` が残り、両家の `player=` 行と文明 ID が変わらない |
| 読み込み・翻訳 | `Database.log`・`Modding.log` にこの MOD 由来のエラーがなく、設定画面とゲーム中に日英露の名前・説明が表示される |

結果：**未実施**。

実行時改名は、このインストールの公式 Lua/UI 例に `GetCivilizationShortDescription` と汎用の `PlayerConfigurations:SetValue` がある一方、`SetCivilizationName` の使用例は見つからなかった。ゲームプレイ Lua は専用セッターが存在すればそれを試し、なければ `SetValue` で3つの表示名キーを書き込む。公開MODの[動的表示名の実装](https://github.com/Gedemon/Civ6-GCO/blob/master/Scripts/GCO_PlayerConfig.lua)は、`SetValue` に加えてゲッターを独自に上書きしているため、標準UIで同じ結果になる根拠にはならない。`write_accepted` は書込みのみを表す。画面反映やセーブ後保持に失敗した場合は、変更後の家名をゲーム状態に保存し、文明名を読む各UIで表示を上書きする方式が必要。
