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
- `Players:Expansion2_Players` に両組を登録する。指導者アイコン・家紋・ロード画面・外交画面の画像は自前（下記「画像」）。色は北条の配色を流用し、組合せで両家を区別する
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

結果：**一部実施**（2026-09-28、Steam 版 1.0.12.68、Gathering Storm、シングルプレイ。織田＝人間、武田＝AI の1ゲーム）

| 確認 | 実際の結果 |
|---|---|
| 同時参加 | 成功。`CIVILIZATION_JAC_ODA`（player 0、人間）と `CIVILIZATION_JAC_TAKEDA`（player 1、AI）がともに参加し、2ターン目に両家が都市を建設した |
| AI の独立性 | 武田 AI は独自に行動した。両家とも AI にするゲーム（手順4）は未実施 |
| 改名 | 不可。ゲームプレイ側では `PlayerConfigurations[...]` の `GetValue`／`SetValue` が nil（`function expected instead of nil`）で、`SetCivilizationName`・`SetCivilizationShortDescription`・`SetCivilizationDescription` も無い。UI 側では `GetValue("CivilizationName")` が nil を返した |
| 実行時エラー | `bad argument #1 to 'tostring' (value expected)` が `Scripts/JAC_CustomCivs.lua:88` と `UI/JAC_CustomCivs_Observe.lua:47` で出た。原因は未設定のキーで `Game:GetProperty` が値を1つも返さないこと。1値に直す関数を通すよう修正済み（修正後の実機確認は未実施） |
| ロード後保持・翻訳 | 未実施 |
| 表示の上書き | 未実施（UI 側の上書きを追加した。実機での確認待ち） |

分かったこと：

- ゲーム中に文明名を変える方法は、ゲームプレイ Lua には無い。これまで試していた専用セッターと `SetValue` の呼出しはやめ、3ターン目にその API があるかだけを調べて記録する
- 家名の変更を画面に出すには、UI 側で表示を上書きするしかない。文明名を表示する各 UI で、`Game:GetProperty` に保存した目標名を読んで差し替える（上記の `JAC_CivNameOverride`。実機は未確認）。公開MODの[動的表示名の実装](https://github.com/Gedemon/Civ6-GCO/blob/master/Scripts/GCO_PlayerConfig.lua)もゲッターを独自に上書きしており、同じ方向になる
- 将来のマルチプレイでは、目標名を各端末で同期済みの共有状態から読む。端末ごとの `PlayerConfigurations` の値は使わない

### 画像

[リポジトリルール](../repository-rules.md)の「画像」に従い、2人の指導者と2つの文明に、北条・日本の流用ではなく自前の画像を割り当てた。

| 画像 | テクスチャ名 | 使われる場所 |
|---|---|---|
| 指導者アイコン | `JAC_Leaders{32,45,48,50,55,64,80,256}`（2×1 のアトラス、0番＝織田、1番＝武田） | 設定画面、右上の指導者一覧、外交の一覧など |
| 文明アイコン（家紋） | `JAC_Civs{22,30,32,36,44,45,48,50,64,80,128,200,256}`（45 だけ色付き、他は白＋透明） | 設定画面、都市の旗、文明一覧など |
| ロード画面の肖像・背景 | `LEADER_JAC_<指導者>_NEUTRAL`（570×1024）・`_BACKGROUND`（1920×1080。`LoadScreen.xml` の帯の高さ 987 が切れず、1080p で上下に黒帯が出ない） | ロード画面、ゲーム終了画面、設定画面の肖像ツールチップ |
| 外交画面の肖像 | `FALLBACK_NEUTRAL_JAC_<指導者>`（`Leaders.artdef` の `Leader_BLP_Entry`・`Leader_Background_BLP_Entry` は、公式の 2D 専用指導者 `DLC/WarMachineScenario/ArtDefs/Leaders.artdef` と同じく空にする） | 外交画面 |
| 外交画面の背景 | `DiplomacyInfo` にロード画面の背景を指定 | 外交画面 |

作り方：`python tools/jac_art/build_art.py` を実行すると、`ArtSource/Originals/` の元画像から切り抜き・縮小し、SDK の Cooker をコマンドラインから呼んで `Platforms/Windows/BLPs/` に BLP を3つ作る（`JAC_CustomCivs_Icons`・`JAC_CustomCivs_Leaders`・`JAC_CustomCivs_Fallback`）。中間ファイルは `tools/jac_art/build/`（Git 管理外）に作る。家紋は伝統図案を Pillow で描いた（Commons の家紋 SVG は CC BY-SA／CC BY なので使わない）。

画像の出典（いずれも Wikimedia Commons のファイルページでライセンスを確認した）：

| 用途 | 作品 | 作者・年 | 所蔵・出典 | ライセンス | 元ファイル |
|---|---|---|---|---|---|
| 武田信虎の肖像・アイコン | 絹本著色武田信虎像 | 武田信廉、1574年 | 大泉寺（甲府） | パブリックドメイン（PD-Art・PD-old-auto-expired、平面作品の忠実な複製で、作者の没後70年以上） | [File:Takeda Nobutora by Takeda Nobukado (Daisenji Kofu).jpg](https://commons.wikimedia.org/wiki/File:Takeda_Nobutora_by_Takeda_Nobukado_(Daisenji_Kofu).jpg) |
| 織田信秀の肖像・アイコン | 萬松寺の織田信秀木像の写真 | 撮影者不明、1934年刊 | 『郷土偉人展覧会図録』（国立国会図書館デジタルコレクション 1232545） | パブリックドメイン（PD-Japan-oldphoto、旧著作権法で保護期間が満了した日本の古い写真） | [File:Statue of Oda Nobuhide at Banshō-ji.jpg](https://commons.wikimedia.org/wiki/File:Statue_of_Oda_Nobuhide_at_Bansh%C5%8D-ji.jpg) |
| 織田家の背景 | 冨嶽三十六景「尾州不二見原」 | 葛飾北斎、1830〜32年頃 | メトロポリタン美術館（Open Access） | CC0 | [File:冨嶽三十六景 尾州不二見原 …（MET DP140982）](https://commons.wikimedia.org/wiki/File:%E5%86%A8%E5%B6%BD%E4%B8%89%E5%8D%81%E5%85%AD%E6%99%AF_%E5%B0%BE%E5%B7%9E%E4%B8%8D%E4%BA%8C%E8%A6%8B%E5%8E%9F-Fujimigahara_in_Owari_Province_(Bish%C5%AB_Fujimigahara),_from_the_series_Thirty-six_Views_of_Mount_Fuji_(Fugaku_sanj%C5%ABrokkei)_MET_DP140982.jpg) |
| 武田家の背景 | 「甲陽猿橋之図」 | 歌川広重、1841〜42年頃 | メトロポリタン美術館（Open Access） | CC0 | [File:「甲陽猿橋之図」…（MET DP335039）](https://commons.wikimedia.org/wiki/File:%E3%80%8C%E7%94%B2%E9%99%BD%E7%8C%BF%E6%A9%8B%E4%B9%8B%E5%9B%B3%E3%80%8D-The_Monkey_Bridge_in_Kai_Province_(K%C5%8Dy%C5%8D_Saruhashi_no_zu)_MET_DP335039.jpg) |
| 家紋（織田木瓜・武田菱） | 伝統図案を単純化して新たに描画 | — | `tools/jac_art/build_art.py` | 自作 | — |

武田信虎像は元が 358×864 と小さいため、アイコンは少しぼやける。見た目が足りなければ、この肖像を参考にした独自画像に差し替え、出典と作り方をここに追記する。

画像の確認手順：

1. 追加コンテンツで「JAC PoC：カスタム勢力」を有効にする
2. ゲーム設定で織田と武田を選び、指導者アイコン・家紋・肖像のツールチップを確認する
3. 開始してロード画面の肖像と背景を確認する。武田を人間にした別ゲームも開始し、ロード画面の肖像画と猿橋の背景を確認する
4. ゲーム内で、右上の指導者アイコンと都市の旗の家紋を確認する
5. 武田との外交画面を開き、2D の肖像と背景を確認する。`Lua.log` に `[JAC_POC] leaderScreen loaded` が出ることも確認する

成功条件：

| 確認 | 期待する結果 |
|---|---|
| 設定画面 | 織田は木像の丸いアイコンと木瓜の家紋、武田は肖像画の丸いアイコンと菱の家紋。ツールチップに各家の肖像と背景が出る |
| ロード画面 | 織田は木像と北斎の尾張の図、武田は肖像画と広重の猿橋の図。北条の画像が出ない |
| ゲーム内 | 右上の指導者アイコンと都市の旗が自前の画像になる。日本・北条のアイコンが出ない |
| 外交画面 | 3D の北条ではなく、2D の肖像と各家の背景が出る。`leaderScreen loaded` が出て、画面が読み込み中で止まらない |
| ロード画面の端 | 帯の上下の端が切れず、1080p で背景の上下に黒帯が出ない |
| ログ | `ArtDef.log` に `Loading Package: …/JAC_CustomCivs_Icons.blp`（`_Leaders`・`_Fallback` も）が出て、`Unable to resolve Package` にこの MOD の BLP が無い。`Lua.log` に `Failed to load background image texture` と `We are lacking a texture` が出ない |

未確認：ゲーム画面での表示はまだ確認していない。BLP は Cooker が成功を返し、中に全テクスチャ名があることまで確認した。外交画面の 2D 表示は、公式の WarMachineScenario と同じ設定（`Leaders.artdef` は空、肖像は `FallbackLeaders.artdef`）にした。以前は `Leader_BLP_Entry` に背景の UITexture を入れていたが、公式とも公開MOD（[Republic of Turkey](https://github.com/NerdByFate/Civilization-6-Republic-of-Turkey)）とも一致しないため戻した。肖像が出ない・読み込みが止まる場合は、Turkey と同じく `Leader_BLP_Entry` に 1920×1080 の UITexture を入れる形を試す。

BLP の容量：非圧縮 RGBA（`PF_R8G8B8A8_UNORM`）で作っており、3つで 26,963,968 バイト（`_Leaders` 21,383,168、`_Fallback` 4,705,792、`_Icons` 875,008）。肖像は `_Leaders` と `_Fallback` の両方に入る。作り直すたびに Git の履歴が同じくらい増えるため、実機で表示を確かめた後に、背景だけの圧縮形式・解像度の見直し・Git LFS かビルド時生成を試して容量を測り、決めた方針をここに書く。
