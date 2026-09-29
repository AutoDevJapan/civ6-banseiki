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
- 右上のバーの List of Reports のすぐ左に「手帳」ボタンを置く（SSOT 11.1）。`ReplaceUIScript` で `PartialScreenHooks` を `UI/JAC_PartialScreenHooks.lua` に差し替え、XP2 版を include して `AddScreenHooks()` の Era と Reports の間に1件足す。専用アイコン `UI/Textures/JAC_Notebooks_Hook.dds` を使う
- 手帳は SSOT 11.1 の構成で、記録帳（人物帳・法律帳・宗教帳）と国家金庫（長官たち・宝物庫）を並べる。中身があるのは人物帳だけで、他は「（未実装）」付きの押せない項目。表示名は用語辞書（14-3）の3言語
- 人物帳は生存中の武将ユニットの状態を直接 `Unit:GetProperty` から表示する（読み取り専用）。日本語・ロシア語・英語の固定テスト行を常時並べる。ESC による手帳の閉鎖は要実機確認。同じボタンの再クリックや他の画面（List of Reports など）を開くと手帳は閉じる
- 以前の固定位置の「人物帳」ボタン（実機で表示されなかった）は削除した

### 手帳ボタンとアイコンの再生成

右上の手帳ボタンは `UI/Textures/JAC_Notebooks_Hook.dds` を使う。`UI/JAC_PartialScreenHooks.lua` の `AddScreenHook` から参照し、`JAC_PoC_GrowthUI.modinfo` の `ImportFiles` と `Files` に登録する。表示寸法は `PartialScreenHooks.xml` の 26×26。

公式 `Base/Assets/UI/PartialScreenHooks.xml` では各フックの背景は `LaunchBar_Hook_ButtonSmall.dds` を使う 40×40 の `Button`、図柄はその子の 26×26 の `Image`（`Anchor="C,C"`、`Offset="0,-1"`）。`PartialScreenHooks.lua` の `AddScreenHook` は `ScreenHookImage:SetTexture(texture)` だけを行い、図柄のスプライト位置や状態オフセットを変えない。XP1/XP2 の `PartialScreenHooks_Expansion1.lua` が追加する Era も同じ関数を使う。ゲーム本体の図柄名は WorldRankings、CityStates、Trade、Espionage、Era、Reports の各 `LaunchBar_Hook_*`。SDK の `Civ6Docs.html` によれば、`Image` の既定の `StretchMode=None` は原寸表示して枠外を切り、`Button` 背景だけが状態に応じて縦方向へ 40px ずつずれる（通常、ホバー、押下、無効）。したがって手帳の DDS は状態ストリップではなく、26×26 の単一の透過画像として生成する。

`tools/jac_art/build_art.py` は PR #20 で `main` に追加済みで、このリポジトリから実行できる。GrowthUI の MOD フォルダが存在すると手帳 DDS も生成し、フォルダが無い場合はその工程をスキップして CustomCivs のアート生成を続ける。

```powershell
$env:JAC_ART_LOCAL = 'C:\Users\makhmeto\Desktop\dev\ADJ\civ6-art-local'
$env:JAC_GROWTH_MOD = (Join-Path (Get-Location) 'poc\JAC_PoC_GrowthUI')
python tools/jac_art/build_art.py
```

このスクリプトは両 PoC のアートを生成する。Python の Pillow、ローカルアートストア内の各原本、および Civ6 SDK Asset Cooker が必要。`JAC_GROWTH_MOD` は GrowthUI の別配置先を使う場合に設定する。

手帳アイコンの入力はローカルアートストアの `JAC_PoC_GrowthUI/Originals/notebooks_generated.png`（この PC では `C:\Users\makhmeto\Desktop\dev\ADJ\civ6-art-local\JAC_PoC_GrowthUI\Originals\notebooks_generated.png`）、出力は `poc/JAC_PoC_GrowthUI/UI/Textures/JAC_Notebooks_Hook.dds`。原本はローカルアートストアで管理し、リポジトリに追加しない。生成 DDS も既存の `.gitignore` による除外方針を維持する。生成画像の来歴と利用条件は後段の CustomCivs 節「画像」に記録する。

2026-09-29 の位置修正：従来は 54px 以内の本を 64×64 DDS の中央に配置していたため、26×26 の `Image` では左上だけが切り出され、本が右下へずれて見えた。原本の透過範囲を 24×24 以内に縮小し、26×26 の透明キャンバス中央へ配置する方式に変更した。再生成後の DDS は 26×26、アルファ範囲は `(1,1)-(24,25)`、四隅は完全透明。`Mods/JAC_PoC_GrowthUI` のジャンクションをこの `notebooks-icon` worktree に向け、`JAC_GROWTH_MOD` を指定して `build_art.py` を実行した。Civ6 の再起動後に Gathering Storm の新規ゲームで確認した（再起動前は旧 DDS が表示キャッシュに残った）。

実機では公式アイコンと手帳の `Image` がすべて 26×26、縦の画面内オフセットが 39 で一致し、手帳は Era と Reports の間に表示された。`civ6-autotest` の開始シナリオ 4/4 手順と JAC エラー判定 2/2 件が成功。FireTuner から手帳ボタンの `DoLeftClick()` を2回呼び、手帳の開閉も確認した。右上バーを切り出して4倍に拡大した証跡は `C:\Users\makhmeto\Desktop\dev\ADJ\civ6-art-local\final-01\ingame-v3\notebooks-normal-closeup-x4.png`、`notebooks-hover-offset40-closeup-x4.png`、`notebooks-pressed-offset80-closeup-x4.png`。後二者は入力ドライバのマウス移動が Civ6 のホバー判定に届かなかったため、ゲーム内の手帳 `Button` に背景テクスチャの縦オフセット 40px／80px を一時指定して撮影し、最後に 0px へ戻した。通常・ホバー・押下の背景表示で本の図柄は同じ中央位置を保った。実際のポインター入力によるホバー遷移はこの検証では確認できていない。

### 互換性

`ReplaceUIScript` は `PartialScreenHooks` を置換するため、同じスクリプトを置換する他 MOD との競合があり得る。動作確認はこの MOD 単独の有効化で行っており、競合する MOD との共存は未確認。製品版 MOD は総合改変 MOD として他 MOD を無効にする方針で、共存が必要になった場合は互換パッチを用意する。

手順：

1. 共通の準備に従い `poc/JAC_PoC_GrowthUI` を `Mods/JAC_PoC_GrowthUI` へジャンクションで配置する。この検証中は `JAC_PoC_Bundle0` を無効にし、本MODを有効にする
2. Gathering Storm のシングルプレイを新規開始し、右上のバーで List of Reports のすぐ左のボタン（ツールチップ「手帳」）を押す。アイコンが欠けていないか、左上の「歴史年表」（History Timeline）ボタンと見分けられるかも見る
3. 「人物帳」を押し、各武将の全項目を確認する。ESC による閉鎖は要実機確認（自動操作では ESC がゲームメニューを開くため判定できない）
4. 手帳をもう一度開き、開いたまま List of Reports を押す
5. 4ターン進めてセーブし、メインメニューへ戻って同じセーブをロードする
6. 手帳 → 人物帳を再度開き、セーブ前と同じ内容か確認してから1ターン進める
7. ゲームの表示言語を日本語・英語・ロシア語に順に切り替え、必要なら再起動して同じセーブを開き、各言語の文言がその言語だけで出ることを確認する。ロシア語では手帳の項目名がボタンからはみ出していないかも見る


成功条件：

| 確認 | 期待する結果 |
|---|---|
| 読み込み | `Modding.log` に `JAC_GrowthUI_Text`・`JAC_GrowthUI_Gameplay`・`JAC_GrowthUI_InGameUI`・`JAC_GrowthUI_PartialScreenHooks`・`JAC_GrowthUI_UIFiles` の適用が記録され、`Database.log` に本MOD由来のエラーがない |
| ボタンの位置 | `Lua.log` に `[JAC_POC] notebooks button attached parent=ButtonStack` が出て、ボタンが List of Reports のすぐ左にある。都市国家と出会った後も同じ位置にある。この行が出ないときは置換が効いていない（`ReplaceUIScript` の競合を疑う） |
| アイコン | 専用の `JAC_Notebooks_Hook.dds` が欠けずに表示され、「歴史年表」ボタンと見分けられる |
| 開き直しと切り替え | 手順4で手帳が閉じて List of Reports が開き、手順3〜4の間の `Lua.log` に `UI.DataError`（`Cannot determine if partial screen`）や `Runtime Error` がない。初回に開いたときの `[JAC_POC] notebooks path lookup found=<true/false>` も記録する（`false` なら、閉じた手帳を公式の `IsPartialScreenOpen()` がパスで探せず `UI.DataError` になる。公式の Era Progress と同じ作り） |
| 手帳と人物帳 | 開くたびに `[JAC_POC] notebooks opened`、人物帳を出すたびに `[JAC_POC] character journal shown count=<n>` が出る。`n` は生存中の武将ユニット数（新規ゲームでは開始ユニット数に応じて1～3） |
| 新規ゲームの `Lua.log` | `[JAC_POC] no saved state (new game)`、`[JAC_POC] assigned unit=...` が開始ユニット数に応じて1～3件、`[JAC_POC] initialized count=...` が出る。装備・Artifactの値も記録される |
| 成長の `Lua.log` | `[JAC_POC] unit=<id> level=... xp=... personality=... trait=... equipment=... artifact=... player=... turn=...` がターンごとに出る。同じユニットで1ターンあたり20 XPを得る。Lv1の必要XPは30で、累計40 XPの時点ではLv2・残り10 XPとなる |
| ロード後の `Lua.log` | `[JAC_POC] restored unit=...` の全項目がセーブ直前の記録と一致し、`restored count=...` と再表示時の `character journal shown count=` も一致する。ロード直後に `assigned unit=` は増えず、次ターンだけ経験値が増える |
| 3言語 | 人物帳の表示が直近のログの記録と一致し、表示言語の文言だけが文字化けや欠落なく出る（他言語の訳を並べない。SSOT 14.2）。各表示言語でボタン・見出し・項目名・性格・特性名が切り替わる。項目名がボタン幅を超えると末尾が「…」で切られ、`[JAC_POC] notebooks label truncated key=...` が出る（出たらメニューとボタンの幅を広げるか、未実装の表記を短くする） |

結果：**主要項目は成功**（2026-09-29、Steam 版 1.0.12.68、Gathering Storm、英語表示）。証跡は兄弟 worktree の `../civ6-autotest/tools/civ6-autotest/artifacts/run-11-growth-ui/`。自動シナリオ41手順とログ判定13/13が成功した。

| 確認 | 実際の結果 |
|---|---|
| 読み込み | `Modding.log` に5アクションの適用を確認。`Database.log` に JAC 由来のエラーなし |
| ボタンの位置 | `parent=ButtonStack`。手帳は Reports の左。都市国家1件との遭遇後も同じ位置（`02-top-right-closeup.png`・`12-after-city-state-top-bar-clean.png`） |
| アイコン | 専用の `JAC_Notebooks_Hook.dds` が欠けずに表示され、画面上でも左上の歴史年表の肖像アイコンと区別できた（`03-timeline-closeup.png`）。CustomCivs との併用時にも装丁された本が表示され、履歴タイムラインと区別できた（`civ6-art-local/final-01/ingame/10-notebooks-button-closeup.png`・`ingame/19-notebooks-open-clean.png`） |
| 開き直しと切り替え | `notebooks path lookup found=true`。再クリックで閉じ、手帳から Reports に切り替わった。`UI.DataError`・`Runtime Error` なし |
| 手帳と人物帳 | `notebooks opened` 5件、`character journal shown count=2` 4件。メニュー・人物帳・保存前後の画像あり。日本語の固定行に英語表示で欠字が出たため、同梱の NotoSansCJKjp をその行に指定して再検証し、日露英の固定3行を表示できた |
| 新規ゲーム | `no saved state`、開始2ユニットへ `assigned`、`initialized count=2`。1体目の装備2件・Artifact、2体目の装備1件を確認 |
| 成長 | 両ユニットともターン1～5に毎回20 XP。ターン2で Lv2・残り10 XP（Lv1必要量30）、保存時は Lv2・70 XP |
| 保存・ロード | `restored count=2`。両ユニットの全記録と人物帳の表示が保存前の Lv2・70 XP に一致。追加の `assigned` はなく、次のターン6で Lv3・5 XP |
| 3言語 | 英語表示では英語の項目名と日露英の固定3行を確認。日本語・ロシア語表示への切り替えは未確認。固定3行は、訳を並べて表示しない方針（SSOT 14.2）により 2026-09-29 に削除した |


このブランチの追加自動実行は `tools/civ6-autotest/scenarios/growth-notebooks.json` を使い、`tools/civ6-autotest/artifacts/growth-run-13/` に保存した（41/41 ステップ、ログ判定 13/13 件）。詳細な確認結果は本節の表を参照。


要オーナー確認：表示言語 `ja_JP`・`ru_RU` での文言とロシア語項目名の収まり。Options の表示言語変更はアプリ再起動を要求し、現ドライバはゲームの再起動を扱わない。人物帳を開いた状態で ESC を押して手帳が閉じるかも実機確認が必要。自動入力では公式 Reports List でも ESC がゲーム全体のメニューを開いたため、閉鎖を判定できなかった。

未検証：マルチプレイ。`LastXPTurn` で同じターンの二重加算は防いでいるが、`Unit:SetProperty` の同期とクライアント間の一致はシングルプレイの結果からは確認できない。マルチプレイで採用する前に、2人プレイで加算回数とロード後の一致を確認する。

## JAC_PoC_CustomCivs：カスタム勢力

対象の PoC：SSOT 16.1 束1「カスタム勢力」。別 Civilization ID の同時参加、AI の独立行動、ゲーム途中の家名表示変更を調べる。言語表示も束0の一部として確認する。

MOD の内容：

- `CIVILIZATION_JAC_ODA`／`LEADER_JAC_ODA_NOBUHIDE` と `CIVILIZATION_JAC_TAKEDA`／`LEADER_JAC_TAKEDA_NOBUTORA` を追加する。勢力能力・指導者能力は効果なしの検証用
- `Players:Expansion2_Players` に両組だけを登録する。公式 GS と独自ルールセット「万世紀」が `RulesetDomainOverrides` でこのドメインを選ぶため、標準ルールセットの選択候補には入らない。FrontEnd の `RuleSetInUse` 条件は起動後に標準から GS へ切り替えた際に登録が再実行されなかったため、公式拡張と同じドメイン方式を使う。指導者アイコン・家紋・ロード画面・外交画面の画像は自前（下記「画像」）。色は北条の配色を流用し、組合せで両家を区別する
- 日英露の名前・説明・基本外交文を登録する。都市名は日本の既存一覧を共有し、武田の首都「甲府」を追加する
- ゲームプレイ Lua が生存中の主要文明の ID・文明・指導者・生存をログに記録する。各ターンには両家の都市数を記録する。3ターン目に、改名用のセッターがゲームプレイ側にあるかを API ごとに調べて記録し（呼び出しはしない）、目標名を `Game:SetProperty` に保存する。UI 側の観測 Lua は標準 UI と同じゲッターの返り値を別に記録する
- 家名の表示は UI 側で上書きする（`UI/JAC_CivNameOverride.lua`）。各 UI コンテキストで `PlayerConfiguration` の `GetCivilizationShortDescription`・`GetCivilizationDescription` を差し替え、武田家なら `Game:GetProperty` の目標名を返す。対象は外交リボン・外交画面・ワールドランキング（`ReplaceUIScript` で GS 版を include）と都市バナー（公式の `CityBannerManager_` 拡張用 include）

手順：

1. 共通の準備に従って `JAC_PoC_CustomCivs` を Mods に配置・有効化し、Gathering Storm の詳細設定で織田家を人間、武田家を AI に選んで開始する
2. 3ターン目まで進め、`Lua.log` の `renameField`・`rename`・`nameOverride` と UI 側の `uiRenameState` を確認する
3. 4ターン目に、武田家の名前を外交リボンのツールチップ・外交画面・ワールドランキング・武田の都市バナーのツールチップで確認する（日本語なら「甲斐武田家」）
4. 武田 AI を発見した後、都市建設・ユニット行動を画面で確認する。セーブしてメニューへ戻り、ロード後に `Lua.log` と手順3の4画面を確認する
5. 別ゲームで既存の文明を人間にし、織田家と武田家をともに AI に指定する。両者を発見した後に行動を画面で確認し、`Lua.log` と照合する
6. ゲーム言語を英語・日本語・ロシア語に切り替えて、新規ゲーム設定画面とゲーム中の両家の名前を確認する

成功条件（`Lua.log` の各行に `[JAC_POC]` が付く）：

| 確認 | 期待するログ・画面 |
|---|---|
| 同時参加 | 異なる `player=<id>` の `civ=CIVILIZATION_JAC_ODA leader=LEADER_JAC_ODA_NOBUHIDE alive=true` と `civ=CIVILIZATION_JAC_TAKEDA leader=LEADER_JAC_TAKEDA_NOBUTORA alive=true`、`customCivs oda=1 takeda=1` |
| AI の独立性 | 両 AI を選んだゲームでも2行が別 ID で出る。`turn=… player=… civ=… cities=…` が各家で継続し、ゲーム画面で両者それぞれの都市建設・ユニット行動を確認できる |
| 改名 API の有無 | 3ターン目に `renameField api=… context=gameplay result=nil` が4つの API（`SetCivilizationName`・`SetCivilizationShortDescription`・`SetCivilizationDescription`・`SetValue`）で1行ずつ、続けて `rename turn=3 … result=unavailable` が出る。以降のターン・ロード後には繰り返さない。`result=function`（`setter_found`）が出たらパッチで API が増えたので、方式を見直す |
| 表示の上書き | 各コンテキストの読み込み時に `nameOverride context=<名前> result=installed` が出る（`DiplomacyRibbon`・`DiplomacyActionView`・`WorldRankings`・`CityBannerManager`）。3ターン目以降に `result=applied … target=LOC_JAC_TAKEDA_RENAMED_NAME` が出て、手順3の4画面で武田家が目標名になる。ロード後も同じ。`installed` が出ないコンテキストは差し替えが効いていない（`ReplaceUIScript` の競合を疑う）。`shared_with_…` はゲッターの表がコンテキスト間で共有されている印で、上書きは効く。`no_method_table` なら方式を見直す |
| ロード後保持 | 再読込後の `renameState stage=load … target=LOC_JAC_TAKEDA_RENAMED_NAME attempted=true` と `uiRenameState stage=load …` を確認する。両家の `player=` 行と文明 ID も照合する |
| 読み込み・翻訳 | `Database.log`・`Modding.log` にこの MOD 由来のエラーがなく、設定画面とゲーム中に日英露の名前・説明が表示される |
| 実行時エラー | `Lua.log` に `JAC_CustomCivs` 由来の `bad argument` などのエラーがない |

2026-09-28 の結果：**一部実施**（Steam 版 1.0.12.68、Gathering Storm、シングルプレイ。織田＝人間、武田＝AI の1ゲーム）

| 確認 | 実際の結果 |
|---|---|
| 同時参加 | 成功。`CIVILIZATION_JAC_ODA`（player 0、人間）と `CIVILIZATION_JAC_TAKEDA`（player 1、AI）がともに参加し、2ターン目に両家が都市を建設した |
| AI の独立性 | 武田 AI は独自に行動した。両家とも AI にするゲームはこの時点では未実施（2026-09-29 に別ゲームで実施。下記参照） |
| 改名 | 不可。ゲームプレイ側では `PlayerConfigurations[...]` の `GetValue`／`SetValue` が nil（`function expected instead of nil`）で、`SetCivilizationName`・`SetCivilizationShortDescription`・`SetCivilizationDescription` も無い。UI 側では `GetValue("CivilizationName")` が nil を返した |
| 実行時エラー | `bad argument #1 to 'tostring' (value expected)` が `Scripts/JAC_CustomCivs.lua:88` と `UI/JAC_CustomCivs_Observe.lua:47` で出た。原因は未設定のキーで `Game:GetProperty` が値を1つも返さないこと。1値に直す関数を通すよう修正済み（修正後の実機確認は未実施） |
| ロード後保持・翻訳 | 未実施 |
| 表示の上書き | 未実施（UI 側の上書きを追加した。実機での確認待ち） |

2026-09-29 の結果：**自動シナリオ成功**（Gathering Storm、シングルプレイ）。`civ6-autotest` worktree の `tools/civ6-autotest/scenarios/clans-notebooks.json` を実行し、`artifacts/clans-notebooks-run-09/steps.jsonl` で **17/17 ステップ、ログ判定 2/2 件**の成功を確認した。織田＝人間・武田＝AI の参加と手帳の表示を確認し、ターン4でセーブ、メインメニューからロード、ターン5への進行と手帳の再表示に成功した。手帳ボタンは GrowthUI MOD を併用して確認した。CustomCivs 単独の検証結果には含めない。画面記録は同じ実行フォルダの `01-start.png`、`02-notebooks.png`、`03-turn4.png`、`04-after-load-notebooks.png`。この実行は新アート生成前のため、下記の新アート表示の証拠にはしない。

2026-09-29 のルールセット境界の再確認：**成功**（Steam 版 1.0.12.68、MOD 有効、ゲーム再起動後）。FrontEnd Lua で `GameConfiguration.SetRuleSet` と `GameConfiguration.UpdateEnabledMods` を実行。さらに `HostGame` の Lua で公式 `CreatePlayerParameters(0, true)` と `Refresh()` を呼び、実際の `Parameters.PlayerLeader.Domain` と `Values` を照会した。標準 `RULESET_STANDARD` は `Players:StandardPlayers`、全57候補中 **JAC 0 件**。GS `RULESET_EXPANSION_2` は `Players:Expansion2_Players`、全82候補中 **JAC 2 件**（`LEADER_JAC_ODA_NOBUHIDE`・`LEADER_JAC_TAKEDA_NOBUTORA`）。FrontEnd の `DB.ConfigurationQuery` でも有効ドメインの JAC 行数は標準 0 件・GS 2 件で一致。設定 DB 全体には両行が登録されるが、標準ルールセットの選択候補には入らない。FrontEnd アクションに `RuleSetInUse` 条件を付ける試行では、標準で起動してから GS へ切り替えても両行が追加されなかったため、その条件は採用せず、公式 GS と同じドメイン方式を維持した。

同じ再確認で既存の `scenarios/clans-notebooks.json` を変更せず実行し、**17/17 ステップ、ログ判定 2/2 件が成功**。織田＝人間・武田＝AI の起動、手帳表示、ターン4でのセーブ、ロード、ターン5と手帳の再表示を確認した。ローカル証拠は `civ6-autotest/tools/civ6-autotest/artifacts/clans-notebooks-round5-01/` の `steps.jsonl`・ログ・4枚のスクリーンショット（Git 管理外）。既存の同名セーブは実行前に退避し、実行後に元の場所へ戻した。

| 日付 | 検証 | 結果と画面記録 |
|---|---|---|
| 2026-09-29 | 家名表示の切替・ロード後保持 | **成功**（途中の手動入力なし）。`steps.jsonl` で 102/102 ステップ、ログ判定 2/2 件。ターン1は4 UI コンテキストの原名、遭遇が UI に反映されたターン2は4画面の原名、ターン3の `rename` 後のターン4とセーブ／終了／ロード後は4画面の改名を確認した。証拠一式はローカルの `tools/civ6-autotest/artifacts/clans-house-names-run-08/`（PR #19 のドライバーが生成。Git 管理外のため GitHub からは参照不可）。`steps.jsonl`、`Lua.log` と画面記録（`01-turn1-hud.png`、ターン2の `02-turn2-*`・`03-turn2-city-banner.png`、ターン4の `05-turn4-*`、ロード後の `09-load-*`）を保存。 |
| 2026-09-29 | 織田・武田の両方を AI にした実機検証 | **成功**。Steam 版 1.0.12.68、Gathering Storm、英語 UI、シングルプレイ。公式北条／日本を人間（player 0）、織田を AI（player 1）、武田を AI（player 2）にし、GrowthUI は無効化。`scenarios/clans-both-ai.json` の 41/41 ステップ、ログ判定 2/2 件が成功。`Lua.log` に別 ID の両家と `customCivs oda=1 takeda=1` を記録。両 AI はターン1に都市 0、ターン2に都市 1 となり、ターン4の Lua 照会では織田の首都が名古屋、武田の首都が甲府。家名は織田が `Oda clan` のまま、武田が3ターン目の切替後に `Takeda clan` から `Kai Takeda clan` となり、両都市バナーのツールチップでも一致。セーブ／メニューへ戻る／ロード後も両 AI と家名を確認し、ターン5まで進行した。ユニット移動の個別追跡は行わず、両者の都市建設を AI 行動の証拠とした。ローカル証拠は `tools/civ6-autotest/artifacts/clans-both-ai-run-02/` の `steps.jsonl`、`Lua.log`、`01-start.png`、`02-turn4-oda-capital.png`、`03-turn4-takeda-capital.png`、`04-after-load.png`、`05-turn5-after-load.png`（Git 管理外）。MOD 本体の修正は不要だった。 |


自動シナリオの定義は `tools/civ6-autotest/scenarios/`（`clans-notebooks.json`、`clans-house-names.json`、`clans-both-ai.json`）、このブランチの実行成果物は `tools/civ6-autotest/artifacts/`（`clans-notebooks-run-09/`、`clans-house-names-run-08/`、`clans-both-ai-run-02/`）にある。各結果の詳細は本節の記録を参照。



切替前の外交画面は `TAKEDA CLAN`、ランキング概要は `Takeda clan leading`、都市バナーの実ツールチップは `Kofu (Takeda clan)`。切替後とロード後はそれぞれ `KAI TAKEDA CLAN`、`Kai Takeda clan leading`、`Kofu (Kai Takeda clan)`。外交リボンの実ツールチップも `Takeda clan` から `Kai Takeda clan` に変わった（値は同フォルダの `Lua.log` に `[JAC_HOUSE]` として記録）。都市バナーとリボンの家名は画面上のツールチップ値を UI Lua で読み、スクリーンショットには都市・アイコンの表示を記録した。遭遇状態を Gameplay Lua で設定した直後のターン1では UI のリボンにまだ武田が現れないため、実画面の切替前比較はターン2に行った。

実機で見つかった表示不具合を2点修正した。Gathering Storm の `ReplaceUIScript` が PoC の置換を後から上書きしていたため、3画面のアクションに `LoadOrder=100` を設定。ランキング概要行だけは `GameInfo.Civilizations` を直接読むため、該当行を UI 側で改名した。観測専用 `InGame` コンテキストの `uiRenameState.short/localized` は原名のままで、`target` と表示対象4コンテキストの実文字列で判定した。日英露の言語切替表示は未検証。

### マルチプレイ同期と決定性

目標名を各端末で同期済みの共有状態から読むことは設計要件だが、この PoC はシングルプレイでの確認のみ。マルチプレイ時の共有状態の同期・決定性と、各端末での表示一致は未検証である。

分かったこと：

- ゲーム中に文明名を変える方法は、ゲームプレイ Lua には無い。これまで試していた専用セッターと `SetValue` の呼出しはやめ、3ターン目にその API があるかだけを調べて記録する
- 家名の変更を画面に出すには、UI 側で表示を上書きするしかない。文明名を表示する各 UI で、`Game:GetProperty` に保存した目標名を読んで差し替える（上記の `JAC_CivNameOverride`）。ランキング概要行のようにゲッターを経由しない表示は個別に差し替える。公開MODの[動的表示名の実装](https://github.com/Gedemon/Civ6-GCO/blob/master/Scripts/GCO_PlayerConfig.lua)もゲッターを独自に上書きしており、同じ方向になる
- 将来のマルチプレイで目標名を各端末が同期済みの共有状態から読むことを設計要件とする。端末ごとの `PlayerConfigurations` の値は使わない。同期・決定性は上記のとおり未検証

### 画像

[リポジトリルール](../repository-rules.md)の「画像」に従い、2人の指導者と2つの文明に新規生成した画像を割り当てた。2人の指導者は既存 Civ6 指導者画を画風だけの参照とし、同じ質感・照明の 3D 絵画調に揃えた。公式画像の人物や画素は MOD に含めない。

| 画像 | テクスチャ名 | 使われる場所 |
|---|---|---|
| 指導者アイコン | `JAC_Leaders{32,45,48,50,55,64,80,256}`（2×1 のアトラス、0番＝織田、1番＝武田） | 設定画面、右上の指導者一覧、外交の一覧など |
| 文明アイコン（家紋） | `JAC_Civs{22,30,32,36,44,45,48,50,64,80,128,200,256}`（全サイズ白＋透明のアルファマスク） | 設定画面、都市の旗、文明一覧など |
| 文明・指導者の能力アイコン | `JAC_Abilities{22,30,32,36,44,45,48,50,64,80,128,200,256}`（4×1。0＝織田家、1＝武田家、2＝織田信秀、3＝武田信虎。全サイズ白＋透明） | アイコン定義に登録済み。標準の設定画面・百科事典では専用絵柄は使われない（下記参照） |
| ロード画面の肖像・背景 | `LEADER_JAC_<指導者>_NEUTRAL`（570×1024、髷の上に余白を確保）・`_BACKGROUND`（1920×1080） | ロード画面、ゲーム終了画面、設定画面の肖像ツールチップ |
| 外交画面の肖像 | `FALLBACK_NEUTRAL_JAC_<指導者>`（`Leaders.artdef` の `Leader_BLP_Entry`・`Leader_Background_BLP_Entry` は、公式の 2D 専用指導者 `DLC/WarMachineScenario/ArtDefs/Leaders.artdef` と同じく空にする） | 外交画面 |
| 外交画面の背景 | `LEADER_JAC_<指導者>_DIPLOMACY_BACKGROUND`（1600×1000）を `DiplomacyInfo` に指定。`LeaderScene` の標準レターボックスを両家で隠す | 外交画面 |

`tools/jac_art/build_art.py` の CustomCivs 用パッケージには、アイコンアトラス34点（指導者8＋文明13＋能力13）、指導者画像6点（各指導者のロード肖像・ロード背景・外交背景）、フォールバック画像2点が入り、テクスチャ合計は **42点**。GrowthUI が無い場合、コンタクトシートも同じ42点を並べる（GrowthUI がある場合は別途手帳画像を1点加える）。先行記載の「41」は数え違いであり、コンタクトシートとパッケージで数え方が違うわけではない。スクリプトの定義・出力数はいずれも42点。

### 画像の成功条件

| 確認 | 期待する結果 |
|---|---|
| 設定画面 | 織田・武田の同一画風の丸い肖像アイコン、それぞれ木瓜・菱の家紋、能力名と説明、新規生成した肖像と背景が出る。標準 UI は専用能力アイコンを表示しない |
| 能力アイコン | PoC の成功条件は、4点が登録され、能力定義から正しく参照できること。これは `IconManager:FindIconAtlas` の返り値で検証済み。標準 UI は能力欄で文明／指導者アイコンを再利用するため、専用絵柄の画面表示は対象外。製品版で表示するには能力用のカスタム UI が必要 |
| ロード画面 | 両名の新規生成人物と城内の背景が出る。紫色の縁や公式北条の人物が出ない |
| ゲーム内 | 右上の指導者アイコンと都市の旗が自前の画像になる。日本・北条のアイコンが出ない |
| 外交画面 | 3D の北条ではなく、2D の肖像と各家の背景が出る。背景の上下に黒い余白が出ない。`leaderScreen loaded` が出て、画面が読み込み中で止まらない |
| ロード画面の端 | 帯の上下の端が切れず、1080p で背景の上下に黒帯が出ない |
| ログ | `ArtDef.log` に `Loading Package: …/JAC_CustomCivs_Icons.blp`（`_Leaders`・`_Fallback` も）が出て、`Unable to resolve Package` にこの MOD の BLP が無い。`Lua.log` に `Failed to load background image texture` と `We are lacking a texture` が出ない |

#### 新しい画像の実機表示

2026-09-29、Steam 版 1.0.12.68・Gathering Storm・英語 UI で確認。初回に織田を人間／武田を AI とするゲームと、武田を人間／織田を AI とするゲームを起動した。修正後はゲームを再起動し、武田を人間／織田と公式クレオパトラを AI とするゲーム、および織田を人間／武田を AI とするゲームで再確認した。修正後の保存画像は 1456×939。画像の保存先は `C:/Users/makhmeto/Desktop/dev/ADJ/civ6-art-local/final-01/`（Git 管理外）。表中の `ingame/`・`ingame-v2/` から始まるパスはそこを基準とする。

| 確認 | 実際の結果 |
|---|---|
| Mod と生成物 | 実機確認では `JAC_PoC_CustomCivs` と `JAC_PoC_GrowthUI` を併用した。CustomCivs の BLP 3 個を確認し、手帳 DDS の表示結果は GrowthUI 節に記録した。修正後 BLP 3 個は 03:24〜03:25 に再生成済み。ゲーム再起動後の `ArtDef.log` で3個の読み込み、両ゲームの `Modding.log` で `UpdateArt`・`UpdateIcons`・`JAC_CustomCivs_LeaderScene` の適用を確認した。 |
| Esc 前のロード画面 | 初回は武田の髷が上端で切れた（`ingame/15-loading-takeda-before-esc.png`）。修正後はロード完了・Esc 前でも髷全体と上の余白が表示された（`ingame-v2/10-loading-takeda-before-esc.png`）。織田の初回画像は `ingame/08-loading-oda-before-esc.png`。 |
| FrontEnd の選択・設定 | 両家の指導者アイコンと家紋が詳細設定で並び、各人の肖像も基本設定に表示された。`ingame/02-setup-oda-details.png`、`ingame/07-advanced-setup.png`、`ingame/14-setup-takeda.png` |
| ゲーム中のアイコン | 織田・武田の指導者アイコンが右上に表示され、接触後の外交リボンにも武田の指導者アイコンが表示された。外交画面の左上には武田の家紋も表示された。`ingame/09-ingame-oda.png`、`ingame/11-diplomacy-ribbon.png`、`ingame/12-diplomacy-takeda.png`、`ingame/16-ingame-takeda.png` |
| 武田との外交 | 初回は新規フォールバック人物と背景の上下に黒帯が見えた（`ingame/12-diplomacy-takeda.png`）。修正後は織田を人間にして Gameplay Lua の `SetHasMet(1, true)` で接触させ、公式 UI の `DiplomacyRibbon_OpenDiplomacyActionView(1)` で開いた。人物と背景が上下まで表示され、黒帯は消えた。`LeaderScene` の上下レターボックスも `IsHidden()=true`。`ingame-v2/20-diplomacy-takeda.png` |
| 織田との外交 | 武田を人間、織田を AI にして同じ方法で開いた。人物と背景が上下まで表示され、黒帯は無い。上下レターボックスは `IsHidden()=true`。`ingame-v2/11-diplomacy-oda.png` |
| 公式指導者との外交 | 同じゲームでクレオパトラを AI に追加して開いた。公式の 3D 人物と標準の上下帯が表示され、上下レターボックスは `IsHidden()=false`。閉じて織田を開き直すと再び両方 `true` になった。`ingame-v2/12-diplomacy-cleopatra.png` |
| 文明・指導者能力 | 両家の能力名・説明は設定画面に、武田の能力名・説明は百科事典にも表示された。標準 UI は能力欄で文明／指導者アイコンを再利用するため、能力専用4絵柄の画面表示は対象外。PoC の成功条件である4点の登録と参照は検証済みで、ゲーム内 `IconManager:FindIconAtlas` は4定義とも `JAC_Abilities50.dds` のオフセット 0・50・100・150 を返した。製品版で専用絵柄を画面に表示するには能力用のカスタム UI が必要。`ingame/02-setup-oda-details.png`、`ingame/14-setup-takeda.png`、`ingame/17-civilopedia-takeda.png`、`ingame/18-civilopedia-takeda-leader.png` |
| 実行ログ | 初回の `Lua.log`・`Database.log`・`Modding.log` に CustomCivs／GrowthUI 由来のテクスチャ・ArtDef エラーは見当たらない。修正後の `Lua.log` に `JAC_LeaderScene.lua`／`LeaderScene` の実行エラー、`Failed to load background image texture`、`We are lacking a texture` は無い。修正後のログは `ingame-v2/logs/` に保存した。初回の `Database.log` の `LocalizedText` 重複は `CurrentClickouts/…/ClickoutText_en_US.xml`、`Modding.log` の XML 警告は本体 DLC のファイルに対応する。 |

`C:/Users/makhmeto/AppData/Local/Firaxis Games/Sid Meier's Civilization VI/Logs/ArtDef.log` のパッケージ解決記録（2026-09-29 02:53:59）は次のとおり。同ログには `Unable to resolve Package` と CustomCivs の BLP を含む行は無い。

```text
[2026-09-29 02:53:59]	Loading Package: C:/Users/makhmeto/Documents/My Games/Sid Meier's Civilization VI/Mods/JAC_PoC_CustomCivs/Platforms/Windows/BLPs/JAC_CustomCivs_Fallback.blp
[2026-09-29 02:53:59]	Loading Package: C:/Users/makhmeto/Documents/My Games/Sid Meier's Civilization VI/Mods/JAC_PoC_CustomCivs/Platforms/Windows/BLPs/JAC_CustomCivs_Icons.blp
[2026-09-29 02:53:59]	Loading Package: C:/Users/makhmeto/Documents/My Games/Sid Meier's Civilization VI/Mods/JAC_PoC_CustomCivs/Platforms/Windows/BLPs/JAC_CustomCivs_Leaders.blp
```

`ingame/` は背景・肖像修正前、`ingame-v2/` は修正後にゲームを再起動して撮影した結果。再起動前のセッションでは新しい `ReplaceUIScript` が未適用で黒帯が残ったため、修正後の判定には使っていない。

再生成には Python 3、Pillow、Civ6 SDK の Asset Cooker が必要。ビルドに使う Python に Pillow が無ければ `python -m pip install Pillow` で導入する。Asset Cooker の既定の場所は `C:\Program Files (x86)\Steam\steamapps\common\Sid Meier's Civilization VI SDK\AssetModTools\Cooker`。別の場所にある場合は、`Civ6AssetCooker_FinalRelease.exe` と `Civ6.cfg` を含むディレクトリを環境変数 `JAC_COOKER_DIR` に指定する（PowerShell 例：`$env:JAC_COOKER_DIR = 'D:\Steam\steamapps\common\Sid Meier''s Civilization VI SDK\AssetModTools\Cooker'`）。その後 `python tools/jac_art/build_art.py` を実行する。

CustomCivs の入力元はローカルの `civ6-art-local/JAC_PoC_CustomCivs/Originals/`。既定ではリポジトリの `../civ6-art-local` を使い、この複数 worktree 配置では `../../civ6-art-local` を検出する。任意の場所は `JAC_ART_LOCAL` で指定できる。GrowthUI が `<リポジトリ>/poc/JAC_PoC_GrowthUI` にある場合だけ、`civ6-art-local/JAC_PoC_GrowthUI/Originals/` から手帳 DDS を作る。別の場所にある GrowthUI は `JAC_GROWTH_MOD` で指定する。GrowthUI フォルダが無い場合は手帳工程をスキップし、CustomCivs 単独で完了する。

Pillow で単色背景から人物・手帳を透過処理し、指導者の円形アイコン、家紋・能力の白いアルファマスクと各サイズのアトラスを作る。外交背景は元の城内画像を16:10へ切り出す。SDK の Asset Cooker は3個の BLP を一時ディレクトリに生成する。3個すべての更新時刻・容量・テクスチャ名を検証してから MOD 内の BLP を置き換えるため、Cooker が途中で失敗しても配置済みの BLP は残る。2026-09-29 のビルドでは GrowthUI 不在のままスキップし、3 BLP の再生成が成功した。中間 `.dds`・`.tex`・`.xlp` は `tools/jac_art/build/`、全テクスチャの PNG と一覧はローカルの `civ6-art-local/final-01/` に置く。元画像・Cooker 出力・手帳 DDS は Git 管理外とし、MOD を配布・実機検証する端末では事前にビルドする。

選定した指導者肖像は `civ6-art-local/trial-02/gen_gemini.py`、人物切り抜き用画像・背景・家紋・手帳は `civ6-art-local/generate_final.py` で生成した。両方ともローカル OpenCodex の `http://127.0.0.1:10100/v1/responses` に `google-antigravity/gemini-3.1-flash-image` を指定し、Antigravity の OAuth サブスクリプション枠を使った（API キーや従量課金 API は不使用）。これは生成経路の事実を記録したもので、規約上この経路が許可されるという判断ではない。生成元画像と Cooker の画像出力はローカルの `civ6-art-local/` にのみ保存し、Git には含めていない。プロンプト・生成時刻・出力ファイルは `civ6-art-local/generation-log.jsonl` に保存する（先行して作った肖像2点の時刻は元ファイルの更新時刻から補記）。既存の PD 写真4点は削除せず、`Originals/_old_pd/` に保管した。

生成画像の来歴と利用条件（規約確認日：2026-09-29。下表の現行画像すべてに共通）：

| 対象 | 生成器・経路 | 生成日 | 参照素材 | 規約・再配布 |
|---|---|---|---|---|
| 下表の指導者アイコン、立ち姿、背景、家紋、能力アイコン、手帳ボタン | Gemini 3.1 Flash Image（`google-antigravity/gemini-3.1-flash-image`）。Google Antigravity のサブスクリプション枠をローカル OpenCodex 経由で使用。API キー・従量課金 API は不使用 | 2026-09-29（日本時間） | 公式 Civ6 の指導者画は画風参照のみ。公式画像の人物・画素を生成物に複製せず、リポジトリや MOD に同梱・配布しない | [Google 利用規約](https://policies.google.com/terms)、[生成 AI の禁止用途ポリシー](https://policies.google.com/terms/generative-ai/use-policy)、[Antigravity 追加利用規約](https://antigravity.google/terms)。生成物の再配布・公開の可否は未確定。この画像を含む PoC の公開・再配布は、[質問 D11](../questions.md) のオーナー判断が完了するまで保留する |

Antigravity 追加利用規約の第6項は第三者ソフトウェア経由のアクセスを禁止している。OpenAI 利用規約も自動・プログラムによるデータまたは出力の抽出を禁止しているため、ChatGPT Web UI で出力取得をブラウザーエージェントに自動化する案にも規約上の懸念がある（[OpenAI 利用規約](https://openai.com/policies/row-terms-of-use/)）。いずれも規約解釈や生成経路の許可をここで確定せず、[質問 D11](../questions.md) のオーナー判断に残す。規約へのリンクは生成画像に再配布許諾があることを示すものではない。

現行画像の出典（ローカル元ファイルは Git 管理外）：

| 用途 | 作品 | 作者・年 | 所蔵・出典 | ライセンス・利用条件 | 元ファイル |
|---|---|---|---|---|---|
| 織田信秀・武田信虎の指導者アイコン | 両名の新規生成肖像 | Gemini 3.1 Flash Image、2026-09-29 | ローカル OpenCodex。公式 Civ6 指導者画は画風参照のみ | 上記の規約・公開条件 | `oda_nobuhide_portrait.jpg`・`takeda_nobutora_portrait.jpg` |
| ロード画面・外交画面の人物 | 単色背景で新規生成した両名の立ち姿 | 同上 | 同上。Pillow で透過 | 同上 | `oda_nobuhide_figure.png`・`takeda_nobutora_figure.png` |
| ロード画面・外交画面の背景 | 尾張・甲斐の城内を新規生成 | 同上 | 同上 | 同上 | `oda_nobuhide_background.png`・`takeda_nobutora_background.png` |
| 文明アイコン | 織田木瓜・武田菱を新規生成 | 同上 | 伝統家紋を参照し、新規に描画された画像をアルファマスク化 | 同上 | `oda_mokko_generated.png`・`takeda_bishi_generated.png` |
| 能力アイコン | 織田の軍旗、武田の馬、信秀の軍配扇、信虎の兜を新規生成 | 同上 | 両家の家紋・人物像を参照し、各能力欄へ個別登録 | 同上 | `oda_clan_ability_generated.png`・`takeda_clan_ability_generated_v2.png`・`oda_nobuhide_ability_generated.png`・`takeda_nobutora_ability_generated.png` |
| GrowthUI の手帳ボタン | 装丁された手帳の新規生成アイコン | 同上 | ローカル OpenCodex。公式 Civ6 は画風参照のみ | 同上 | `JAC_PoC_GrowthUI/Originals/notebooks_generated.png` |

旧 PD 画像4点の元ファイルはローカルストア `civ6-art-local/JAC_PoC_CustomCivs/Originals/_old_pd/` に残す（現行 MOD からは参照しない）。出典とライセンスは Wikimedia Commons のファイルページで確認した：

| 用途 | 作品 | 作者・年 | 所蔵・出典 | ライセンス | 元ファイル |
|---|---|---|---|---|---|
| 武田信虎の肖像・アイコン | 絹本著色武田信虎像 | 武田信廉、1574年 | 大泉寺（甲府） | パブリックドメイン（PD-Art・PD-old-auto-expired、平面作品の忠実な複製で、作者の没後70年以上） | [File:Takeda Nobutora by Takeda Nobukado (Daisenji Kofu).jpg](https://commons.wikimedia.org/wiki/File:Takeda_Nobutora_by_Takeda_Nobukado_(Daisenji_Kofu).jpg) |
| 織田信秀の肖像・アイコン | 萬松寺の織田信秀木像の写真 | 撮影者不明、1934年刊 | 『郷土偉人展覧会図録』（国立国会図書館デジタルコレクション 1232545） | パブリックドメイン（PD-Japan-oldphoto、旧著作権法で保護期間が満了した日本の古い写真） | [File:Statue of Oda Nobuhide at Banshō-ji.jpg](https://commons.wikimedia.org/wiki/File:Statue_of_Oda_Nobuhide_at_Bansh%C5%8D-ji.jpg) |
| 織田家の背景 | 冨嶽三十六景「尾州不二見原」 | 葛飾北斎、1830〜32年頃 | メトロポリタン美術館（Open Access） | CC0 | [File:冨嶽三十六景 尾州不二見原 …（MET DP140982）](https://commons.wikimedia.org/wiki/File:%E5%86%A8%E5%B6%BD%E4%B8%89%E5%8D%81%E5%85%AD%E6%99%AF_%E5%B0%BE%E5%B7%9E%E4%B8%8D%E4%BA%8C%E8%A6%8B%E5%8E%9F-Fujimigahara_in_Owari_Province_(Bish%C5%AB_Fujimigahara),_from_the_series_Thirty-six_Views_of_Mount_Fuji_(Fugaku_sanj%C5%ABrokkei)_MET_DP140982.jpg) |
| 武田家の背景 | 「甲陽猿橋之図」 | 歌川広重、1841〜42年頃 | メトロポリタン美術館（Open Access） | CC0 | [File:「甲陽猿橋之図」…（MET DP335039）](https://commons.wikimedia.org/wiki/File:%E3%80%8C%E7%94%B2%E9%99%BD%E7%8C%BF%E6%A9%8B%E4%B9%8B%E5%9B%B3%E3%80%8D-The_Monkey_Bridge_in_Kai_Province_(K%C5%8Dy%C5%8D_Saruhashi_no_zu)_MET_DP335039.jpg) |

画像の確認手順：

1. 追加コンテンツで「JAC PoC：カスタム勢力」を有効にする
2. ゲーム設定で織田と武田を選び、指導者アイコン・家紋・肖像・能力名と説明を確認する
3. 開始してロード画面の透過人物と新規生成背景を確認する。武田を人間にした別ゲームでも確認する
4. ゲーム内で、右上の指導者アイコンと都市の旗の家紋を確認する
5. 武田との外交画面を開き、2D の肖像と背景を確認する。`Lua.log` に `[JAC_POC] leaderScreen loaded` が出ることも確認する


修正前後の実機結果は上記「新しい画像の実機表示」を参照。修正後の新 BLP と `LeaderScene` 差し替えは実機で確認した。都市の旗と1080pでの端の表示は未確認。能力専用アイコン4点は登録・参照を確認済みで、標準 UI が文明／指導者アイコンを再利用するため画面表示は PoC の対象外。製品版で専用絵柄を出すには能力用のカスタム UI が必要。

BLP の容量：非圧縮 RGBA（`PF_R8G8B8A8_UNORM`）で、2026-09-29 の修正後ビルドの 3 個は合計 37,824,512 バイト（`_Leaders` 31,619,072、`_Fallback` 4,705,792、`_Icons` 1,499,648）。人物は `_Leaders` と `_Fallback` の両方に入る。長期保管方式は [質問 D10](../questions.md) の議論待ちで決める。

## JAC_PoC_BanseikiRuleset：独立ルールセットと指導者プール

SSOT 2.2「ゲームモードと指導者プール」および 16.1 束0の検証。`poc/JAC_PoC_BanseikiRuleset` を独立した PoC MOD にした。ルールセットとステージング設定を既存のゲームプレイ PoC から分けることで、FrontEnd の設定登録を常時適用し、各 PoC のゲーム内アクションをルールセット条件で評価できる。3つの既存 MOD は `RULESET_JAC_BANSEIKI` と比較用の `RULESET_EXPANSION_2` の両方で読み込む。GS を残す理由は、既存シナリオの回帰確認と両ルールセットの差分調査である。

新ルールセットの `Rulesets` 行は `RULESET_JAC_BANSEIKI`、`GameCore=Expansion2`、`IsScenario=0`、`SupportsSinglePlayer=1`、`SupportsMultiPlayer=1`。マルチプレイの実動作は未検証。勝利条件のドメインとマップ候補は GS を使う。GS が `RULESET_EXPANSION_2` 専用に宣言する通常オプション、災害の強度（`Realism`）、自然遺産・都市国家除外、公式の指導者プール1・2、および開始時代・ターン制限の設定更新を、このルールセットにも宣言する。

指導者候補には `Players:Expansion2_Players` を共用する。これは公式と JAC の全候補を持つドメインで、`RULESET_JAC_BANSEIKI` の `PlayerLeader` に `RulesetDomainOverrides` で指定する。織田・武田の行は `JAC_PoC_CustomCivs/Config/JAC_CustomCivs_Players.xml` に一度だけ登録し、新 MOD は CustomCivs を依存 MOD とする。`JAC_BanseikiHumanPool` 設定の既定は `JAC_BANSEIKI_ONLY`、もう一方は `JAC_ALL_POOLS`。`DomainValueFilterQueries` と `QueryParameters` がルールセット・プレイヤー ID・設定値を読み、シングルプレイの人間枠 0 かつ既定値のときだけ候補を織田信秀・武田信虎との積集合にする。AI 枠にはフィルターを返さず、公式と JAC の全候補を残す。この仕組みは FrontEnd の設定 DB だけで動き、ゲームプレイ Lua や UI 差し替えは不要。

2026-09-29 の設定画面と FrontEnd Lua の結果：**設定部分は成功**。英語 UI の「Create Game」ルールセット欄に `Banseiki` が現れ、選択すると `Human Leader Choices` の既定 `Use Banseiki pool only` が表示された。`Use all leader pools` への切替も画面で確認した。`HostGame` の `CreatePlayerParameters(id, true)` と `Refresh()` で、既定時は人間枠 2 候補（両 JAC）、AI 枠 1・2 は各 82 候補（公式クレオパトラと両 JAC を含む）。全プールを選ぶと人間枠も 82 候補となった。スクリーンショットは Git 管理外の `tools/civ6-autotest/artifacts/banseiki-ui-01/ruleset-selected.png` と `pool-all.png`。3言語のタグは登録したが、ゲーム画面での表示確認は英語のみ。

ゲーム開始の初回実行 `tools/civ6-autotest/artifacts/banseiki-full-01/` は、ロード画面後にメニューへ戻り、開始ステップで失敗した。`Modding.log` と `Database.log` では公式 GS の `Expansion2Core` と `Expansion2CoreContent` が適用されず、XP1/XP2 の参照整合性エラーでゲームプレイ DB がロールバックされた。この時点の `Target Mods` にルールセット MOD 自体が入っていなかった。診断用に一時的に `IsScenario=1` として起動した `banseiki-flag1-01/` はゲーム内まで進み、3 MOD の `[JAC_POC]` ログも出たが、`GameClimate` と `Game.GetWorldCongress` は存在せず、Base コアの動作だった。対照の公式「黒死病」`blackdeath-core-control-01/` は同じドライバで `GameClimate=true`、`Game.GetWorldCongress=function`、`Expansion2Core` の適用を確認した。現在の MOD は `IsScenario=0` に戻し、公式シナリオと同様にゲーム内アクションおよび既定有効化を追加した。ゲーム内アクション追加後の再検証は Steam のサインイン画面で起動できず、まだ行えていない。

静的検証は XML 6ファイルの解析、`banseiki-ruleset.json --check`（27ステップ）、自動テスト84件、`git diff --check` がすべて成功。これはゲーム内アクション追加後のGS中核読み込みを証明しない。

最終の自動シナリオは `tools/civ6-autotest/scenarios/banseiki-ruleset.json`。ルールセットとプールの画面記録、設定値と候補リストの Lua 照会、織田（人間）・クレオパトラ（AI）・武田（AI）の開始、3 MOD のログ、気候画面と世界会議 API、数ターンの自動進行、保存・メニューへ戻る・ロードを検査する。未検証はこの最終シナリオのゲーム内部分、マルチプレイ、長期プレイでの海面上昇。

## JAC_PoC_SengokuTrees：戦国の技術・社会制度ツリー

`poc/JAC_PoC_SengokuTrees` は `RULESET_JAC_BANSEIKI` だけで適用する独立PoC。設計案の技術32件・制度30件を元ID・AND前提・費用・行位置のまま登録した。4期は既存 Era ID の表示名を戦国用に変え、GSの時代システムを残す。公式研究行は、必要な公式アイテムを戦国ノードへ再配線してから削除する。公式アイテムは新規Unit・建物の代用品で、効果量は暫定。制度の列は標準CivicsTreeが再計算するため、設計表の列位置の再現は未確認。

各ノードの解放先・残るTODO・実装した2件のBoostは[照合表](sengoku-tree-unlocks.md)。公式定義を参照したGSの時代・気候・世界会議・AIへの処理と未検証事項は[互換処理メモ](sengoku-tree-compatibility.md)。SSOT 3.2に従い、このPoCを含むBanseikiルールセットのターン上限は `TURNLIMIT_NONE` に変更した。これは約200ターンの年表示や統一勝利の実装ではない。

設計表から `python tools/generate_sengoku_trees.py` でSQL・翻訳・アイコン・照合表を生成する。`python -m unittest` は62 ID、前提DAGと到達性、配置重複、費用、3言語キー、SQLの模擬適用を検査する。`python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/sengoku-trees.json --check` はシナリオJSONだけを検査する。ゲームを起動する版のシナリオは、Banseiki開始、両公式ツリー画面の候補、最初の技術と制度の進捗を検査する。

**実機結果：未実施。** 本作業ではCiv6/Steamを起動していない。現時点の「ロード可・プレイ可」は静的検証による目標で、ゲーム内での合格判定ではない。特に既存BanseikiルールセットのGS中核読み込みは前節の未検証事項のまま。
