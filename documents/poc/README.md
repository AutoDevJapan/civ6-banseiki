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
- `Players:Expansion2_Players` に両組だけを登録する。公式 GS の `RulesetDomainOverrides` が `RULESET_EXPANSION_2` の `PlayerLeader` をこのドメインに切り替えるため、標準ルールセットの選択候補には入らない。FrontEnd の `RuleSetInUse` 条件は起動後に標準から GS へ切り替えた際に登録が再実行されなかったため、公式拡張と同じドメイン方式を使う。指導者アイコン・家紋・ロード画面・外交画面の画像は自前（下記「画像」）。色は北条の配色を流用し、組合せで両家を区別する
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

2026-09-29 の結果：**自動シナリオ成功**（Gathering Storm、シングルプレイ）。`civ6-autotest` worktree の `tools/civ6-autotest/scenarios/clans-notebooks.json` を実行し、`artifacts/clans-notebooks-run-09/steps.jsonl` で **17/17 ステップ、ログ判定 2/2 件**の成功を確認した。織田＝人間・武田＝AI の参加と手帳の表示を確認し、ターン4でセーブ、メインメニューからロード、ターン5への進行と手帳の再表示に成功した。手帳ボタンは PR #21（`feat/poc-growth-ui`）の GrowthUI を併用して確認したため、PR #20 単独ではこの表示を再現できない。画面記録は同じ実行フォルダの `01-start.png`、`02-notebooks.png`、`03-turn4.png`、`04-after-load-notebooks.png`。この実行は新アート生成前のため、下記の新アート表示の証拠にはしない。

2026-09-29 のルールセット境界の再確認：**成功**（Steam 版 1.0.12.68、MOD 有効、ゲーム再起動後）。FrontEnd Lua で `GameConfiguration.SetRuleSet` と `GameConfiguration.UpdateEnabledMods` を実行。さらに `HostGame` の Lua で公式 `CreatePlayerParameters(0, true)` と `Refresh()` を呼び、実際の `Parameters.PlayerLeader.Domain` と `Values` を照会した。標準 `RULESET_STANDARD` は `Players:StandardPlayers`、全57候補中 **JAC 0 件**。GS `RULESET_EXPANSION_2` は `Players:Expansion2_Players`、全82候補中 **JAC 2 件**（`LEADER_JAC_ODA_NOBUHIDE`・`LEADER_JAC_TAKEDA_NOBUTORA`）。FrontEnd の `DB.ConfigurationQuery` でも有効ドメインの JAC 行数は標準 0 件・GS 2 件で一致。設定 DB 全体には両行が登録されるが、標準ルールセットの選択候補には入らない。FrontEnd アクションに `RuleSetInUse` 条件を付ける試行では、標準で起動してから GS へ切り替えても両行が追加されなかったため、その条件は採用せず、公式 GS と同じドメイン方式を維持した。

同じ再確認で既存の `scenarios/clans-notebooks.json` を変更せず実行し、**17/17 ステップ、ログ判定 2/2 件が成功**。織田＝人間・武田＝AI の起動、手帳表示、ターン4でのセーブ、ロード、ターン5と手帳の再表示を確認した。ローカル証拠は `civ6-autotest/tools/civ6-autotest/artifacts/clans-notebooks-round5-01/` の `steps.jsonl`・ログ・4枚のスクリーンショット（Git 管理外）。既存の同名セーブは実行前に退避し、実行後に元の場所へ戻した。

| 日付 | 検証 | 結果と画面記録 |
|---|---|---|
| 2026-09-29 | 家名表示の切替・ロード後保持 | **成功**（途中の手動入力なし）。`steps.jsonl` で 102/102 ステップ、ログ判定 2/2 件。ターン1は4 UI コンテキストの原名、遭遇が UI に反映されたターン2は4画面の原名、ターン3の `rename` 後のターン4とセーブ／終了／ロード後は4画面の改名を確認した。証拠一式はローカルの `tools/civ6-autotest/artifacts/clans-house-names-run-08/`（PR #19 のドライバーが生成。Git 管理外のため GitHub からは参照不可）。`steps.jsonl`、`Lua.log` と画面記録（`01-turn1-hud.png`、ターン2の `02-turn2-*`・`03-turn2-city-banner.png`、ターン4の `05-turn4-*`、ロード後の `09-load-*`）を保存。 |
| 2026-09-29 | 織田・武田の両方を AI にした実機検証 | **成功**。Steam 版 1.0.12.68、Gathering Storm、英語 UI、シングルプレイ。公式北条／日本を人間（player 0）、織田を AI（player 1）、武田を AI（player 2）にし、GrowthUI は無効化。`scenarios/clans-both-ai.json` の 41/41 ステップ、ログ判定 2/2 件が成功。`Lua.log` に別 ID の両家と `customCivs oda=1 takeda=1` を記録。両 AI はターン1に都市 0、ターン2に都市 1 となり、ターン4の Lua 照会では織田の首都が名古屋、武田の首都が甲府。家名は織田が `Oda clan` のまま、武田が3ターン目の切替後に `Takeda clan` から `Kai Takeda clan` となり、両都市バナーのツールチップでも一致。セーブ／メニューへ戻る／ロード後も両 AI と家名を確認し、ターン5まで進行した。ユニット移動の個別追跡は行わず、両者の都市建設を AI 行動の証拠とした。ローカル証拠は `tools/civ6-autotest/artifacts/clans-both-ai-run-02/` の `steps.jsonl`、`Lua.log`、`01-start.png`、`02-turn4-oda-capital.png`、`03-turn4-takeda-capital.png`、`04-after-load.png`、`05-turn5-after-load.png`（Git 管理外）。MOD 本体の修正は不要だった。 |

切替前の外交画面は `TAKEDA CLAN`、ランキング概要は `Takeda clan leading`、都市バナーの実ツールチップは `Kofu (Takeda clan)`。切替後とロード後はそれぞれ `KAI TAKEDA CLAN`、`Kai Takeda clan leading`、`Kofu (Kai Takeda clan)`。外交リボンの実ツールチップも `Takeda clan` から `Kai Takeda clan` に変わった（値は同フォルダの `Lua.log` に `[JAC_HOUSE]` として記録）。都市バナーとリボンの家名は画面上のツールチップ値を UI Lua で読み、スクリーンショットには都市・アイコンの表示を記録した。遭遇状態を Gameplay Lua で設定した直後のターン1では UI のリボンにまだ武田が現れないため、実画面の切替前比較はターン2に行った。

実機で見つかった表示不具合を2点修正した。Gathering Storm の `ReplaceUIScript` が PoC の置換を後から上書きしていたため、3画面のアクションに `LoadOrder=100` を設定。ランキング概要行だけは `GameInfo.Civilizations` を直接読むため、該当行を UI 側で改名した。観測専用 `InGame` コンテキストの `uiRenameState.short/localized` は原名のままで、`target` と表示対象4コンテキストの実文字列で判定した。日英露の言語切替表示とマルチプレイ時の同期・決定性は、オーナー確認待ちの**未検証**項目として残す。

分かったこと：

- ゲーム中に文明名を変える方法は、ゲームプレイ Lua には無い。これまで試していた専用セッターと `SetValue` の呼出しはやめ、3ターン目にその API があるかだけを調べて記録する
- 家名の変更を画面に出すには、UI 側で表示を上書きするしかない。文明名を表示する各 UI で、`Game:GetProperty` に保存した目標名を読んで差し替える（上記の `JAC_CivNameOverride`）。ランキング概要行のようにゲッターを経由しない表示は個別に差し替える。公開MODの[動的表示名の実装](https://github.com/Gedemon/Civ6-GCO/blob/master/Scripts/GCO_PlayerConfig.lua)もゲッターを独自に上書きしており、同じ方向になる
- 将来のマルチプレイでは、目標名を各端末で同期済みの共有状態から読む。端末ごとの `PlayerConfigurations` の値は使わない

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

#### 新しい画像の実機表示

2026-09-29、Steam 版 1.0.12.68・Gathering Storm・英語 UI で確認。初回に織田を人間／武田を AI とするゲームと、武田を人間／織田を AI とするゲームを起動した。修正後はゲームを再起動し、武田を人間／織田と公式クレオパトラを AI とするゲーム、および織田を人間／武田を AI とするゲームで再確認した。修正後の保存画像は 1456×939。画像の保存先は `C:/Users/makhmeto/Desktop/dev/ADJ/civ6-art-local/final-01/`（Git 管理外）。表中の `ingame/`・`ingame-v2/` から始まるパスはそこを基準とする。

| 確認 | 実際の結果 |
|---|---|
| Mod と生成物 | 実機確認では `JAC_PoC_CustomCivs` と、PR #21（`feat/poc-growth-ui`）にある `JAC_PoC_GrowthUI` を併用した。CustomCivs の BLP 3 個に加え、GrowthUI 側の `JAC_Notebooks_Hook.dds` が存在した。GrowthUI とこの DDS は PR #21 の成果物で、PR #20 単独には含まれない。修正後 BLP 3 個は 03:24〜03:25 に再生成済み。ゲーム再起動後の `ArtDef.log` で3個の読み込み、両ゲームの `Modding.log` で `UpdateArt`・`UpdateIcons`・`JAC_CustomCivs_LeaderScene` の適用を確認した。 |
| Esc 前のロード画面 | 初回は武田の髷が上端で切れた（`ingame/15-loading-takeda-before-esc.png`）。修正後はロード完了・Esc 前でも髷全体と上の余白が表示された（`ingame-v2/10-loading-takeda-before-esc.png`）。織田の初回画像は `ingame/08-loading-oda-before-esc.png`。 |
| FrontEnd の選択・設定 | 両家の指導者アイコンと家紋が詳細設定で並び、各人の肖像も基本設定に表示された。`ingame/02-setup-oda-details.png`、`ingame/07-advanced-setup.png`、`ingame/14-setup-takeda.png` |
| ゲーム中のアイコン | 織田・武田の指導者アイコンが右上に表示され、接触後の外交リボンにも武田の指導者アイコンが表示された。外交画面の左上には武田の家紋も表示された。`ingame/09-ingame-oda.png`、`ingame/11-diplomacy-ribbon.png`、`ingame/12-diplomacy-takeda.png`、`ingame/16-ingame-takeda.png` |
| 手帳ボタン | 右上の List of Reports の隣に新規アイコンが表示され、開くと手帳メニューが出た。`ingame/10-notebooks-button-closeup.png`、`ingame/19-notebooks-open-clean.png` |
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
| 下表の指導者アイコン、立ち姿、背景、家紋、能力アイコン、手帳ボタン | Gemini 3.1 Flash Image（`google-antigravity/gemini-3.1-flash-image`）。Google Antigravity のサブスクリプション枠をローカル OpenCodex 経由で使用。API キー・従量課金 API は不使用 | 2026-09-29（日本時間） | 公式 Civ6 の指導者画は画風参照のみ。公式画像の人物・画素を生成物に複製せず、リポジトリや MOD に同梱・配布しない | [Google 利用規約](https://policies.google.com/terms)、[生成 AI の禁止用途ポリシー](https://policies.google.com/terms/generative-ai/use-policy)、[Antigravity 追加利用規約](https://antigravity.google/terms)。生成物の再配布・公開の可否は確定しておらず、公開前にオーナーが確認する（[質問 D8](../questions.md)） |

Antigravity 追加利用規約の第6項は第三者ソフトウェア経由のアクセスを禁止している。OpenAI 利用規約も自動・プログラムによるデータまたは出力の抽出を禁止しているため、ChatGPT Web UI で出力取得をブラウザーエージェントに自動化する案にも規約上の懸念がある（[OpenAI 利用規約](https://openai.com/policies/row-terms-of-use/)）。いずれも規約解釈や生成経路の許可をここで確定せず、[質問 D8](../questions.md) のオーナー判断に残す。規約へのリンクは生成画像に再配布許諾があることを示すものではない。

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

成功条件：

| 確認 | 期待する結果 |
|---|---|
| 設定画面 | 織田・武田の同一画風の丸い肖像アイコン、それぞれ木瓜・菱の家紋、能力名と説明、新規生成した肖像と背景が出る。標準 UI は専用能力アイコンを表示しない |
| 能力アイコン | PoC の成功条件は、4点が登録され、能力定義から正しく参照できること。これは `IconManager:FindIconAtlas` の返り値で検証済み。標準 UI は能力欄で文明／指導者アイコンを再利用するため、専用絵柄の画面表示は対象外。製品版で表示するには能力用のカスタム UI が必要 |
| ロード画面 | 両名の新規生成人物と城内の背景が出る。紫色の縁や公式北条の人物が出ない |
| ゲーム内 | 右上の指導者アイコンと都市の旗が自前の画像になる。日本・北条のアイコンが出ない |
| 外交画面 | 3D の北条ではなく、2D の肖像と各家の背景が出る。背景の上下に黒い余白が出ない。`leaderScreen loaded` が出て、画面が読み込み中で止まらない |
| ロード画面の端 | 帯の上下の端が切れず、1080p で背景の上下に黒帯が出ない |
| ログ | `ArtDef.log` に `Loading Package: …/JAC_CustomCivs_Icons.blp`（`_Leaders`・`_Fallback` も）が出て、`Unable to resolve Package` にこの MOD の BLP が無い。`Lua.log` に `Failed to load background image texture` と `We are lacking a texture` が出ない |

修正前後の実機結果は上記「新しい画像の実機表示」を参照。修正後の新 BLP と `LeaderScene` 差し替えは実機で確認した。都市の旗と1080pでの端の表示は未確認。能力専用アイコン4点は登録・参照を確認済みで、標準 UI が文明／指導者アイコンを再利用するため画面表示は PoC の対象外。製品版で専用絵柄を出すには能力用のカスタム UI が必要。

BLP の容量：非圧縮 RGBA（`PF_R8G8B8A8_UNORM`）で、2026-09-29 の修正後ビルドの 3 個は合計 37,824,512 バイト（`_Leaders` 31,619,072、`_Fallback` 4,705,792、`_Icons` 1,499,648）。人物は `_Leaders` と `_Fallback` の両方に入る。長期保管方式は [質問リスト](../questions.md) の議論待ちで決める。

## JAC_PoC_GrowthUI：手帳ボタンの画像

`JAC_PoC_GrowthUI` MOD と `JAC_Notebooks_Hook.dds` は PR #21（`feat/poc-growth-ui`）にある。PR #20 単独には GrowthUI の MOD フォルダも DDS も含まれず、手帳ボタンのカスタム画像表示は再現できない。PR #20 単独で再現できるアート工程は CustomCivs 用の3個の BLP の生成・確認まで。

この PR の `tools/jac_art/build_art.py` は、GrowthUI の MOD フォルダが存在する場合だけ手帳 DDS の生成を行う（既定の `poc/JAC_PoC_GrowthUI`、または `JAC_GROWTH_MOD` で指定した場所）。フォルダが無ければ手帳工程をスキップし、CustomCivs 単独で完了する。GrowthUI の `UI/JAC_PartialScreenHooks.lua` の `AddScreenHook` は生成した `JAC_Notebooks_Hook.dds` を参照し、`JAC_PoC_GrowthUI.modinfo` の `ImportFiles` と `Files` に登録する。公式の History Timeline アイコンとの重複を解消する。表示寸法は `PartialScreenHooks.xml` の 26×26。PR #21 と併用した実機確認では、右上の手帳ボタンに装丁された本が表示され、履歴タイムラインと区別できた。再生成方法・入力先・出典は上記「画像」を参照。
