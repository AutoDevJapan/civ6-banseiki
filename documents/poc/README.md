# PoC の検証記録

SSOT 16.1 の PoC の手順と結果。実機検証待ちの項目もその状態を明記する。検証用の MOD は `poc/` にあり、本番の MOD（SSOT 15.1 の構成）とは分けている。

## 共通の準備

| 項目 | 内容 |
|---|---|
| 基準環境 | Steam 版 Civ6 1.0.12.68、Gathering Storm ルールセット、シングルプレイ |
| MOD の配置 | `Documents\My Games\Sid Meier's Civilization VI\Mods\<MOD名>` から、リポジトリの `poc/<MOD名>` へジャンクションを張る |
| ログ | `%LOCALAPPDATA%\Firaxis Games\Sid Meier's Civilization VI\Logs\` の `Database.log`・`Modding.log`・`Lua.log` |
| FireTuner | `AppOptions.txt` の `EnableTuner` を `1` にする（元のファイルは `AppOptions.txt.bak-20260928`） |
| イントロ動画 | 自動検証では `AppOptions.txt` の `PlayIntroVideo` を `0` にし、起動時のオープニングを流さない（2026-09-29 変更、元のファイルは `AppOptions.txt.bak-20260929`）。Windows が Civ6 を前面に出すのを拒むと、スキップのクリックが届かないため |

ジャンクションの作り方（PowerShell）:

```powershell
New-Item -ItemType Junction -Path "$([Environment]::GetFolderPath('MyDocuments'))\My Games\Sid Meier's Civilization VI\Mods\JAC_PoC_Bundle0" -Target "<リポジトリ>\poc\JAC_PoC_Bundle0"
```

## JAC_PoC_JapanMapScale：1534年開始状態（現行）

万世紀ルールセット用の開始状態。基準日は **天文3年5月12日**。日本列島の22勢力をそれぞれ専用の指導者・文明で登録した。島津は貴久を1勢力の代表とし、アイヌはタリコナを全域代表とするゲーム上の扱い。長尾の文明 ID は `CIVILIZATION_JAC_NAGAO` のまま維持する。人間と AI の指導者候補は、地図を選択すると `MapLeaders` に登録した22人に限定され、同じ指導者は重複できない。万世紀の通常の「人間は2人から選ぶ」フィルタは日本地図では適用しない。`MapSizes` で22人・都市国家0を固定する。

開始方法は設定画面の地図一覧から「史実の領土」または「開拓者から」を選ぶ。各縮尺で前者を先に表示する。史実の領土版には22人の `Players`、草案の53城・拠点と追加の代表本拠3つからなる56都市、都市中心地区、全陸タイルの所有者、人口2を格納した。`Units` に開拓者は入れず、この地図と万世紀ルールセットの組み合わせに限る modinfo 条件で `MajorStartingUnits` を全行除く（部隊は地図の `Units` の行だけ。[japan-prosperous-start.md](japan-prosperous-start.md) §3.2）。開拓者版は同じ地形・22開始位置で、都市・所有地を持たず、通常の開拓者初期配置を使う。シングルプレイの主経路は史実の領土版。マルチプレイは開拓者版を選択できる。別ルールセットの初期ユニット表は変更しない。**旧20 km版での都市消失は実機確認済み。修正後の15 km史実領土版の統合実機結果は下表のとおり。資源配置と指導者選択の変更後の再実機結果は後段の「2026-09-29 の再実機」にある。資源の画面表示と設定画面の22人の候補は未確認。**

日本マップ4種は `Maps:JAC_Banseiki` に登録し、万世紀ルールセット側でこのドメインに `Maps:Expansion2Maps` を合併する。地図 MOD を無効にしても、万世紀から Gathering Storm の地図を選べる。万世紀の `Map` パラメータだけが専用ドメインを参照し、素の Gathering Storm には22勢力が必要な地図を表示しない。地図 MOD は Gathering Storm・CustomCivs・万世紀ルールセットを依存対象に宣言する。ローカル実装の根拠：`DLC/Expansion2/Config/Expansion2_StandardMaps.xml:3-4` のドメイン合併、`DLC/Expansion2/Config/Expansion2_Config.xml:10-12` と `Base/Assets/Configuration/Data/Schema/AdditionalTables.sql:308-312` のルールセット別置換。15 km史実領土版を選んだ実機開始は確認済み。地図4種すべての表示順は未確認。

通常の移動 UI は目的地を指定して `UnitManager.CanStartOperation(..., MOVE_TO, ...)` を確認する（`Base/Assets/UI/UnitFlagManager.lua:291-298`）。外交 UI は開放国境の有無を方向別に確認し（`Base/Assets/UI/DiplomacyActionView.lua:1429-1432`）、戦争状態も別に扱う（`Base/Assets/UI/PartialScreens/CityStates.lua:1129-1130`）。Rise and Fall の緊急事態は `OpenBorders=True` を設定できる（`DLC/Expansion1/Data/Expansion1_Emergencies.xml:16-24`）一方、Alexander などのシナリオは外交行動そのものを削除している（`DLC/AlexanderScenario/Data/AlexanderScenario_RemoveData.xml:207`）。移動を最終判断する GameCore の内部実装はこのローカルの Lua/XML にはないため、通行権の効果は実機シナリオで確認する。

領土境の通行には、日本マップでだけ `TRAIT_LEADER_MAJOR_CIV` から `MODIFIER_PLAYER_UNITS_GRANT_ABILITY` を使い、`CLASS_ALL_UNITS` に `MOD_ENTER_FOREIGN_LANDS` を付ける。既存・新規ユニットに適用され、セーブ後も同じルールから復元される設計。根拠：`Base/Assets/Gameplay/Data/Leaders.xml:825-827`、`Base/Assets/Gameplay/Data/Units.xml:502-505`、`Base/Assets/Gameplay/Data/Modifiers.xml:1929-1933`、`Base/Assets/Gameplay/Data/UnitAbilities.xml:548-553,948-949,1392-1394`。Gathering Storm のロックバンド（`DLC/Expansion2/Data/Expansion2_UnitAbilities.xml:104,135-137`）、秘密結社の Cultist（`DLC/Ethiopia/Data/Ethiopia_SecretSocieties_MODE.xml:591-592`）、英雄 Himiko（`DLC/Babylon/Data/Babylon_Heroes_MODE.xml:590`）も同じ進入効果を使う。開放国境協定を全組に付け直す方法は採らない。協定は30ターン期限付き（`Base/Assets/Gameplay/Data/DiplomaticActions.xml:62`）であり、観光ボーナス25%（`Base/Assets/Gameplay/Data/GlobalParameters.xml:575`）や外交評価（`Base/Assets/Gameplay/Data/Leaders.xml:1332-1335`）まで変わる。協定の通行上の価値は日本マップでは減るが、協定自体とその外交・観光効果は残る。通常の近接警戒とは別に、開放国境なしの領土侵犯を扱う外交修正だけを日本マップで外す（`Base/Assets/Gameplay/Data/Leaders.xml:1296-1300`）。宣戦・和平と他の外交修正は維持する。シナリオでは、協定と戦争のない相手の領土への初手の移動可能範囲を検査し、15 km史実領土版の織田→松平で実機確認した。AI の通行・外交判断と保存・読込後の通行は未確認。

忠誠心は日本マップ4種に限り、近隣市民圧力による毎ターン変化の上限 `LOYALTY_PER_TURN_FROM_NEARBY_CITIZEN_PRESSURE_MAX_LOYALTY` を Gathering Storm の20から5に変更する。半径10、圧力の基礎値と比率、総忠誠心100は維持する（`DLC/Expansion2/Data/Expansion2_GlobalParameters.xml:36-42,184-194`）。Rise and Fall 側も同じ上限20と比率を持つ（`DLC/Expansion1/Data/Expansion1_GlobalParameters.xml:110-116`）。総督の基本効果8（`DLC/Expansion2/Data/Expansion1_Governors.xml:69-75`）、駐留や総督に応じた政策の毎ターン加算（`DLC/Expansion2/Data/Expansion1_Policies.xml:154-174,647-664`）、政府複合施設の加算8（`DLC/Expansion2/Data/Expansion1_Districts.xml:88-112,125-128`）は調整しない。[同じ設定を使う MOD 作者の記録](https://steamcommunity.com/sharedfiles/filedetails/?id=1330549455)は、5刻み以外の値でクラッシュした例があると注意している。

2026-09-29 のオーナー判断：城が密集する日本マップで、通常の Gathering Storm の忠誠心圧力による反乱・独立で都市を自由都市にしない。上限を5に下げても、織田の首都は1ターン目に -2.9/ターンで、都市が自由都市になることを実機で確認した。ローカルの公式テキストは忠誠心が0になると自由都市になると説明する（`DLC/Expansion2/Text/en_US/Expansion1_Civilopedia_Text.xml:167`）。近隣圧力以外にも不満・飢餓・占領などの減算がある（`DLC/Expansion2/Data/Expansion1_Happinesses.xml:5-11`、`DLC/Expansion2/Data/Expansion1_GlobalParameters.xml:75-87`）。

対策は `JAC_Japan_MapRules.xml` に `MODIFIER_PLAYER_CITIES_ADJUST_IDENTITY_PER_TURN` の +100/ターンを追加し、`TRAIT_LEADER_MAJOR_CIV` に付ける。公式の Modifier 定義は対象を `COLLECTION_PLAYER_CITIES`、効果を `EFFECT_ADJUST_CITY_IDENTITY_PER_TURN` とする（`DLC/Expansion2/Data/Expansion1_Modifiers.xml:859-861`）。この共通 Trait は `LEADER_DEFAULT` に付き（`Base/Assets/Gameplay/Data/Leaders.xml:825-827`）、本 PoC の指導者はこれを継承する（`poc/JAC_PoC_CustomCivs/Data/JAC_CustomCivs_Gameplay.sql:20-22`、`poc/JAC_PoC_CustomCivs/Data/JAC_Japan1534_Gameplay.sql:71-88`）。そのため22勢力の既存都市だけでなく、後から建設・占領した都市にも所有者を通じて適用し、保存・読込後もデータ定義から有効になる設計。+100は既定の忠誠心最大値100（`DLC/Expansion2/Data/Expansion2_GlobalParameters.xml:188-194`）に合わせ、通常の減算を毎ターン相殺するための値。15 km統合実機では開始時と50ターン進行の効果を確認した。拡張した都市別の保存比較は、15 km（10ターン）と20 km（50ターン）の再実機で一致を確認した。

宮殿は各勢力の首都1都市だけ（`Base/Assets/Gameplay/Data/Buildings.xml:98`）、総督による圧力無視は配属と昇進が必要（`DLC/Expansion2/Data/Expansion2_Governors.xml:71-72,99-100,135-137`、`DLC/Expansion2/Data/Expansion2_Modifiers.xml:383-385`）なので、全都市の対策には使わない。Gameplay Lua でターンごとに忠誠心を回復する方法は、GameCore の独立判定より先に必ず実行されることをローカルの Lua/XML から確認できないため採用しない。将来の特別なイベント・行動による離反は別機能として、対象都市の補正を明示的に上書きするか、独立・所有者変更をイベントから直接実行する余地を残す（シナリオの都市固有忠誠心補正例：`DLC/BlackDeathScenario/Data/BlackDeathScenario_GameplayData.xml:539,579-583`）。

生成シナリオは開始時の全都市と50ターン後の残存都市の忠誠心変化が負でないことを調べる。50ターン後に自由都市の所有都市が0件であることに加え、公式 UI も使う `Events.CulturalIdentityCityConverted`（`DLC/Expansion2/UI/Loaders/TutorialLoader_Expansion1.lua:95-122`）で、途中に主要勢力が忠誠心によって都市を失った場合も失敗させる。戦争による占領はこのイベントの対象外として扱う。各勢力の都市数は開始時→50ターン後で報告する。15 km史実領土版の56都市、50ターンの自由都市0、保存・読込後の勢力別都市数・生存・陸所有者ハッシュは統合実機で確認済み。都市別の位置・所有者・宮殿・守備隊・忠誠心の保存比較は再実機で一致を確認した。開拓者版での新設都市、通常の Gathering Storm 地図への非波及は未実機。

SSOT 3.1 は開始日の同盟関係を合わせると定めるが、現行地図に初期外交状態はない。2026-09-29 のオーナー判断で、史実の同盟・不仲を開始時の外交状態に反映し、各勢力をある程度栄えた状態で始めることになった（SSOT 3.1）。現行の生成器はまだ外交と発展度を設定しない。

代表本拠と草案の城が同じ場所を示す19勢力では、その城を首都にした。米沢・室町殿は草案の53拠点にないため追加した。那古野は草案上では今川領だが、SSOT の代表本拠は織田の那古野である。織田の首都を追加し、今川の那古野も別タイルの都市として残した。20 km版で今川の那古野は元タイルから60 km移動する。この2都市の同名・近接は地図表現上の妥協で、1534年に両家が同じ城を同時に実効支配したという意味ではない。長尾・山内上杉は別の Civilization ID を使い、文明 ID だけで同盟を組ませる処理はない。

| 対辺間距離 | 盤面 | 陸タイル | 史実領土版 | 開拓者版 | 都市間1タイル | 2タイル | 3タイル |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 20 km | 154×67 | 1,082 | 532,480 B | 516,096 B | 0組 | 31組 | 23組 |
| 15 km | 204×88 | 1,906 | 745,472 B | 716,800 B | 0組 | 20組 | 10組 |

城・本拠間の距離は都市タイル配置後の六角距離。公式インストールの `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/Base/Assets/Gameplay/Data/GlobalParameters.xml:98` に `<Replace Name="CITY_MIN_RANGE" Value="3" />` があり、修正前の実機では3タイル以内の後続都市が落ちた。日本マップ4種だけのゲームプレイ DB アクションで `CITY_MIN_RANGE=1` に更新し、都市間2タイルを許す。万世紀の通常の Gathering Storm 地図には適用しない。新条件で `close_city_pairs` は20 km・15 kmとも **0組**（隣接都市なし）。首都22件を最優先し、残る都市は調査データの番号順に配置・書き込みする。隣接する優先度の低い都市は、移動前の領有判定で同じ勢力に属する空き陸タイルから最短の地点へ、最大3タイルまで移す。20 kmでは11都市（最大60 km）、15 kmでは8都市（最大15 km）を移動した。各都市の元座標からの `displacement_km` と、移動した都市の `nudge.from_plot`・`to_plot`・`distance_km` は `Maps/manifest.json` に記録した。40/30/25 km の生成物とシナリオは削除し、現行の比較対象を20/15 kmに絞った。下の旧試験の表は実装前の履歴であり、現在の生成コマンド・シナリオには対応しない。

### 都市名の表示言語（2026-09-30）

英語 UI の実機画面（`leader-portraits` シナリオ、2026-09-29）で、日本マップの都市名の一部が □□□ の豆腐文字になった。首都（NAGOYA・OKAZAKI など）は正常だった。原因は、史実領土版の地図（20 km・15 km）の `Cities.Name` に首都以外の34都市の日本語名をそのまま書き込んでいたこと。Civ6 は LOC タグでない地図の都市名を翻訳せずにそのまま表示し、英語・ロシア語のフォントには漢字・かなのグリフがない。首都22件は `LOC_JAC_1534_CAPITAL_<勢力>` を使っていたため正常だった。表示名の上書き（`JAC_CivNameOverride.lua`）は武田の家名だけを扱い、都市名には関係しない。

修正として、全56都市の `Cities.Name` を LOC タグにした。首都は従来どおり `LOC_JAC_1534_CAPITAL_<勢力>`、それ以外は `LOC_JAC_1534_CITY_<城ID>`（城 ID は `japan-1534-start.json`）。日英露の行は `write_custom_civs.py` が `JAC_Japan1534_Text.xml` に書き出す。英語はヘボン式の城名（例：桑実寺正覚院 → Kuwanomi-dera、後瀬山城 → Nochiseyama）、ロシア語はポリワノフ式のキリル文字で、首都のロシア語名もキリル文字にそろえた。訳名の表は `generate.py` の `CASTLE_NAMES` で、草案の城の構成が変わると生成時に止まる。既存の地図2枚は地形を作り直さず、`manifest.json` の都市（位置・所有者）に対して `Cities.Name` だけを書き換えた（生成器の出力と同じ値）。

同種の問題（指導者名・勢力名・家名・手帳の文言）も洗い出したが、地図の都市名以外に LOC タグでない日本語の表示名はなかった。`tests/test_localized_names.py` は、使われている `LOC_JAC_*` タグすべてに en_US・ja_JP・ru_RU の行があること、ja_JP 以外の行と地図・SQL・Lua・XML（ja_JP の行と modinfo の `<ja_JP>` を除く）に漢字・かながないこと、地図の都市名がすべて LOC タグであることを検査する。実機では `japan-city-names.json` が15 km史実領土版で全56都市のバナー文字列を読み、日本語以外の UI では漢字・かなを含まないこと、LOC タグのまま表示されていないことを確かめる。2026-09-30 に英語 UI で再実機（`city-name-glyphs-02`）を実行し、合格した。全56都市のバナーがローマ字の訳名で表示され、漢字・かなは含まれなかった。残る確認項目は、ロシア語 UI と日本語 UI での実機確認。都市ごとに期待どおりの名前が付いているかの照合は、生成器とテストによる静的な検査だけで確かめている。

### 史実領土版の実機結果と都市消失の修正（2026-09-29）

修正前の20 km版を `RULESET_JAC_BANSEIKI` で開始した結果、主要文明22、陸1,089タイルすべて領有、開拓者0だったが、静的マップの56都市中30都市だけが存在した。先に処理された都市から3タイル以内の都市が落ち、松平・六角・本願寺・毛利は首都を含めて都市0となった。確認された勢力別都市数（実際／期待）は次のとおり。

| 勢力 | 実際／期待 | 勢力 | 実際／期待 |
|---|---:|---|---:|
| 織田 | 1／2 | 伊達 | 1／4 |
| 蘆名 | 1／2 | 北条 | 2／3 |
| 山内上杉 | 2／5 | 武田 | 1／2 |
| 今川 | 1／3 | 松平 | 0／1 |
| 足利 | 8／12 | 三好 | 2／4 |
| 六角 | 0／2 | 本願寺 | 0／1 |
| 大内 | 2／3 | 毛利 | 0／1 |

新しい地図は各城・本拠56都市を保持し、生成シナリオは勢力別の全都市数・合計56都市・首都位置・22主要文明・全陸領有・開拓者0を開始時に検査する。15 km史実領土版の統合実機結果は下表のとおり。20 kmの修正版と追加変更の再実機結果は後段の「2026-09-29 の再実機」にある。

領土の判断は [`japan-1534-start.json`](../spec/data/japan-1534-start.json) の68令制国配分と53拠点に従う。海岸線・陸域・標高の元データと利用条件は下の旧縮尺試験の出典表と同じ。境界ポリゴンを直接使えるデータは採用できなかった。[CODH「旧国・旧郡境界データセット」](https://geoshape.ex.nii.ac.jp/kg/) は江戸末期の境界で、CC BY-NC の非商用条件がある。代わりに53拠点・22代表本拠の位置と、拠点がない国の概略中心33点（`TERRITORY_ANCHORS`）を使う最近接割当とし、その中心点の勢力が草案の配分表に載ることを生成時に検査する。北海道は二風谷と同じ連結島をアイヌ、南西諸島の南側は琉球として補正する。地理的な代理境界であり、郡境・1534年の実効支配を再現したものではない。陸の全タイルに22勢力のいずれか、さらに同勢力で最寄りの都市を `PlotOwners` の `CityOwner` / `CityWorking` として付ける。城座標・所有者・国配分の史料と留保は[調査記録](../research/japan-1534-start.md)にある。

### 統合実機結果（2026-09-29、15 km・万世紀・戦国ツリーあり）

日本マップ・資源・首都（宮殿）・守備隊・領土境の通行・自由都市化なし（忠誠心+100/ターン）・戦国ツリー（`poc/integration` ブランチで #31 と統合）を有効にし、全自動（`restart_game` から）で実行した。

| 項目 | 結果 |
|---|---|
| 開始 | 盤面204×88、主要文明22、都市国家0、都市56（首都22に宮殿）、陸地1,906タイルすべて所有、開拓者0、各首都に戦士1 |
| 領土境 | 開始時に織田の戦士が、国境開放なしで隣接勢力（松平）の領土へ移動できる |
| 忠誠心 | 開始時の首都 +97/ターン、最低の都市でも +92/ターン（修正前は首都でも −2.9、50ターンで24都市が自由都市化） |
| 戦国ツリー | 技術32・社会制度30、公式の技術・制度なし、GS の気候・世界会議などの仕組みは存在 |
| 50ターン | 自由都市0。1ターン平均6.45秒・中央値2.59秒・最大24.6秒（最後の10ターン平均2.35秒）、プライベートメモリ約8.2〜8.3 GB で横ばい |
| 序盤の戦争 | AI どうしの自動進行で、50ターン以内に織田（自動進行中は AI が操作）が滅亡した回がある。開始時の発展度と史実の外交で対処する（SSOT 3.1） |
| セーブ・ロード | 10ターン後にセーブ→メニュー→ロードし、勢力ごとの都市数・生存・全タイルの所有者のハッシュが一致。ロード後1ターンも成功 |
| ログ | 判定6項目すべて合格（本 MOD 由来のエラーなし） |

途中で見つかった検証側の問題（修正済み）：地図の都市は宮殿がないと首都にならない、開拓者なしで開始するとユニットが0になる、自動進行中に人間の勢力が滅亡すると操作プレイヤーに戻らずターンが進み続ける（ドライバは滅亡として報告する）、FireTuner が自動進行の開始直後に空のフレームを送ることがある、Windows が Civ6 の前面化を拒むと入力が届かない（ロード画面の開始ボタンを Lua で押し、イントロ動画は `AppOptions.txt` で無効化）。

長時間（300ターン相当）の安定性と、20 km との比較は未実施。

### 全 MOD の200ターン健全性検査（`japan-15km-200turn-full.json`）

`poc/` の MOD のうち、この設定に当てはまるものをすべて有効にして（万世紀・CustomCivs・GrowthUI・戦国ツリー・JapanMapScale・HistoricalEvents・CourtArbitration。Bundle0 は他の日本シナリオと同じく無効、試験用の `JAC_Test_UnificationMap` は対象外）、15 km 史実地図・22勢力で200ターンを自動進行する。人間の席は `japan-prosperous-15km.json` と同じ大内（滅亡で打ち切られにくい）。`allow_human_elimination` は有効なので、人間の勢力が滅んでも最後まで進む。

`python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/japan-15km-200turn-full.json` で実行する。50ターンごとに要約を2行出す。要約はエラーにならず、失敗した場合も `summary failed: <理由>` を返す。

1. `GameCore_Tuner` の行：`T+<経過ターン> engine=<内部ターン> alive=<生存勢力数> dead=[滅亡した家] cities=[家=都市数,…] ages reign=<改元の番号>:<ReignType> last(G/N/D/H)=<直近の判定の黄金/通常/暗黒/英雄の家数> events=<史実イベントの発火数> alliances=<今同盟している主要勢力の組の数> arb pending=<審理中> truce=<停戦中の組> await=<講和待ちの組> peace=<講和させた組> outlaw=<討伐対象の家> lastReq=<最後の請求ターン> winner=<勝利チーム。未決は -1>`。値を返さない呼び出しは `nil` と出す（Lua 5.1 の `tostring()` は引数なしだとエラーになるため、要約は `S(v)` で包む）
2. `InGame` の行：`year=<暦の表示> localPlayer=<操作プレイヤー> officialEraScore=<公式の時代スコアの合計>`。万世紀では公式の時代スコアは常に0なので、`OK` 以外（`NONZERO [家ID=値,…]`）は不具合として扱う

最後の手順だけが合否を決め、200ターンに届かない（`T+201` 未満）か生存勢力が0ならエラーにする。ターンごとの時間とメモリは `autoplay-metrics-<N>.jsonl`、研究の推移は `autoplay-research-<N>.jsonl`（N は50ターン区切りの番号）に出る。

#### 実機結果（health-200-01、2026-09-30）

合格。

1. `T+201` まで到達し、22勢力のうち21勢力が生存
2. 暦は 1534 → 1550（T51）→ 1567（T101）→ 1584（T151）→ 1600（T201）
3. 公式の時代スコアはすべての確認で0。判定は 8/8、Lua の実行時エラーなし
4. 元号ごとの時代（黄金/英雄/通常/暗黒の家数）：弘治 4/0/13/5、永禄 0/0/8/14、元亀 0/8/6/7、天正 1/1/9/10、文禄 5/5/7/4、慶長 4/0/6/11。短い元号で暗黒に偏る件は `fix/ages-short-reigns` で修正中
5. 史実イベントの発火は108回、ブーストは231回
6. 開始時同盟の結び直しは48回（当時の実装。オーナー判断K で結び直しをやめ、要約の `lapses=` は `alliances=` に替えた）
7. AI からの仲裁請求はなし（AI の請求は MVP では未実装）
8. この回では `GameCore_Tuner` の要約（手順8・11・14・17）がすべて `summary failed: bad argument #1 to 'tostring' (value expected)` になった。値を返さない呼び出しを `tostring()` に直接渡していたためで、`S(v)` で包むように直した（上の数値は他のログから集計）

### 資源・河川・湖沼・植生・火山

生成器は[資源候補JSON](../spec/data/japan-map-resources.json)の50陸上・17海上候補、河川10本、湖沼6か所、森林・湿地規則、火山候補を両縮尺へ反映する。史実領土版と開拓者版は同じ資源・地形データを持つ。開始地図にはCiv6既定資源だけを書き、資源は都市中心タイルに置かない。

| 内容 | 20 km | 15 km | 配置・近似 |
|---|---:|---:|---|
| Civ6既定資源 | 50件 | 53件 | 陸上33/36件、海上17件。競合5/2件を保留し、石見銀山1件は公式の合法地形が現地の気候に合わないため保留 |
| 河川 | 10本・177辺 | 10本・232辺 | 1534年旧流路案を含む座標列を、連結した共有辺へ吸着 |
| 湖沼 | 6か所・7タイル | 6か所・8タイル | 面積とタイル目安から水面を近似 |
| 森林 | 404タイル | 750タイル | 絹資源に必要な森林3タイルと、残りの適格タイルの44.96%/45.03%（401/747）。丘陵・山地側を優先し、都市・他の資源・湿地・湖・火山・稲作平野近傍を除外 |
| 湿地 | 27タイル | 33タイル | 主要7クラスターへ集中。陸地の2.5%を目標に22〜33タイルで上限 |
| Gathering Storm 火山 | 5か所 | 5か所 | 浅間・富士・阿蘇・桜島・霧島。Civ6の火山Featureが山岳地形を要するため、山岳タイルでない火口候補は地形を山岳へ補正 |

資源の候補座標はWGS84の代表点で、公式の `Resource_ValidTerrains`・`Resource_ValidFeatures` と `LakeEligible`・河川・沿岸条件を満たす60 km以内の最寄りのタイルへ吸着する。絹3件は公式条件に従って森林を付ける。瀬戸内など沿岸の塩6件は製塩用の開けた土地を平原タイルで近似する。これは気候を砂漠にする表現ではない。石見銀山の銀は公式資源が砂漠・ツンドラ系しか許さず、温暖な石見をその地形に変える根拠がないため開始地図では保留し、`no_historically_sensible_terrain` と理由を記録する。調査確度・出典ID・吸着先・競合・開始時状態は `Maps/manifest.json` に記録した。金・漆は通常マップ用の資源行が必要なため開始地図から除外し、`RESOURCE_JAC_GOLD` と `RESOURCE_JAC_LACQUER` を今後追加する候補として記録した。既定資源でも1534年後の開発となる生野銀山・鶴子銀山・延沢銀山・足尾銅山は開始時には置かず、後年イベント／研究で解禁する候補として保持した。金山・漆器産地の年次条件がある地点も同様に記録する。硝石は鉱床資源として置かず、平戸（1586年以後）と年代未確定の堺の交易アクセス候補として記録する。

湖には公式 `WorldBuilderMap.sql` の専用テーブルも `FEATURE_LAKE` もないため、面積比で選んだタイルを `TERRAIN_COAST` の水域として表現した。実際の湖岸線、埋立・干拓、1534年の河道を測量復元したものではない。河川の中心線も概略点列で、利根川は東遷前の旧流路案、石狩川・淀川などは改修前の線形を簡略化している。森林率は16世紀の実測値ではなく、草地・丘陵のゲーム調整値。湿地は現代の湿地情報を参照した地域代表で、1534年の面積を直接示さない。自然の驚異は富士山／阿蘇カルデラのどちらも選ばず、未実装候補 `FEATURE_JAC_FUJI`・`FEATURE_JAC_ASO_CALDERA` として記録した。

候補ごとの典拠と確度は[調査記録](../research/japan-map-resources.md)とJSONの `sources` にある。主な地理資料は[Natural Earth 10m land](https://www.naturalearthdata.com/downloads/10m-physical-vectors/10m-land/)、[NOAA ETOPO 2022](https://www.ncei.noaa.gov/products/etopo-global-relief-model)、国土地理院の[湖沼データ](https://www.gsi.go.jp/kankyochiri/lakedatalist.html)、環境省の[現存植生図](https://eadas.env.go.jp/eiadb/ebidbs/Service/About?title=Gis)・[ラムサール湿地](https://www.env.go.jp/nature/ramsar/conv/RamsarSites_in_Japan.html)、気象庁の各火山履歴（例：[浅間山](https://www.data.jma.go.jp/vois/data/tokyo/306_Asamayama/306_history.html)）である。資源の鉱山・産業史と各河川の出典は、JSONの地点・河川ごとの `source_ids` から辿れる。

| 用途 | 作品 | 作者と年 | 所蔵と出典 | ライセンス | 元ファイル |
|---|---|---|---|---|---|
| 53拠点・1534年の所有者・令制国配分 | 日本1534年開始データ草案 | 本リポジトリの調査、2026 | [調査記録](../research/japan-1534-start.md)の各参照先 | 各史料を参照。位置・人物などの事実を入力し、外部の境界図形は複製しない | `documents/spec/data/japan-1534-start.json` |
| 20人の仮指導者アイコン・ロード肖像・外交フォールバック | 既存の織田信秀 PoC 画像を共用 | Gemini 3.1 Flash Image による生成、2026-09-29 | [CustomCivs の画像記録](#画像)に詳記。画像自体はローカルアートストア | 生成画像の公開・再配布可否は未決（[質問 D11](../questions.md)）。新たな画像を収録しない | `oda_nobuhide_portrait.jpg`・`oda_nobuhide_figure.png`・`oda_nobuhide_background.png` → `JAC_CustomCivs_Icons.blp`・`JAC_CustomCivs_Leaders.blp`・`JAC_CustomCivs_Fallback.blp` |
| 20文明の仮紋章 | Civ6 日本文明のシンボル | Firaxis Games、2016 | インストール済みの Civ6 `Base/Assets/UI/Icons/Icons_Civilizations.xml` の `ICON_CIVILIZATION_JAPAN` | ゲーム本体の素材。MOD に画像画素を複製せず、ゲーム内参照のみ | `CivSymbols*.dds`（ゲーム本体） |

新規20勢力に専用肖像・家紋がまだないため、上の仮画像を割り当てた。共用する織田肖像は他の20人の史実肖像ではない。画面で「?」を避けるには、CustomCivs 節のアートビルドで作る BLP 3個を MOD とともに配置する必要がある。配布前に各勢力の絵へ差し替え、画像の利用条件を確定する。BLP は Git 管理外。

再生成・静的検査は `python tools/japan_map_scale/generate.py`（地図を再生成しない場合は `python tools/japan_map_scale/write_support.py`）、`python tools/japan_map_scale/write_custom_civs.py`、`python -m unittest discover -s tools/japan_map_scale/tests`、`python -m unittest discover -s tools/civ6-autotest/tests`。今回の静的検査は前者25件成功、後者98件成功。後者はこの環境で `tempfile` の通常の `0700` 作成ディレクトリに書き込めないため、作業ツリー内で権限を継承する一時ディレクトリを使った。テスト中の Civ6 起動表示はモックであり、実際の Civ6 と Steam は起動していない。

15 km史実領土版の統合実機では、織田を人間として56都市・首都22・全陸領有・開拓者0・通行・忠誠心・50ターン進行・従来の保存比較・ロード後1ターン・ログと性能を確認した。

**2026-09-29 の再実機（資源修正・武田開始・保存比較の拡張の後、全自動）：**

| シナリオ | 結果 |
|---|---|
| 武田信虎を人間にした開始（15 km・20 km） | 合格。ローカルプレイヤーが武田で、56都市の座標と所有者、宮殿、首都守備隊、全陸所有者が勢力ごとに一致。判定6/6 |
| 織田、10ターン後にセーブ→ロード（15 km） | 合格。拡張した保存比較（都市ごとの位置・所有者・宮殿・守備隊・忠誠心）が一致し、ロード後1ターンも成功。判定6/6 |
| 織田、戦国ツリーあり50ターン（20 km） | 合格（全12手順、判定6/6）。自由都市0、1ターン平均2.18秒・最大2.7秒、プライベートメモリ7.3〜7.4 GiB。織田は2都市のまま残存、六角が滅亡、北条3→1、山内上杉5→9、足利12→15 |
| 織田、戦国ツリーあり50ターン（15 km） | 不合格。50ターン以内に織田（自動進行中は AI）が滅亡。開始時の発展度と史実の外交で対処する |
| 織田、戦国ツリーなし50ターン（15 km・20 km） | 両方とも50ターン以内に織田が滅亡。20 km の52ターン時点で織田・六角が滅亡、足利12→17、都市は56→85 |

日本マップの検証シナリオは、統合実機と同じく戦国ツリーを有効にする。15 km の50ターン検証は、開始時の発展度と史実の外交を実装するまで、織田の滅亡で止まることがある。22人の選択画面、開拓者版、マルチプレイ、資源の画面表示は未実機。史実地図の `Players.ID` は城・領土・宮殿・守備隊の所有者IDと一致させ、公式 Vikings 地図と同じく選択可能な行を `Human` とする。生成した `japan-map-20km-takeda.json`・`japan-map-15km-takeda.json` は設定枠0を武田信虎とし、実際のローカルプレイヤーの指導者、首都、56都市の座標と所有者、宮殿、首都守備隊、全陸所有者を開始時に照合する。

織田シナリオの保存前後のスナップショットは、ターン、主要勢力数、各勢力の都市数・生存状態、全陸タイルの所有者の31ビットハッシュに加え、**各残存都市のタイル位置・所有者ID・宮殿の有無・同じ所有者の守備隊が都市タイルにいるか・現在の忠誠心・忠誠心の毎ターン変化**を位置順で比較する。戦争で50ターン後の都市数が開始時と変わることは許すが、保存前後の都市行は一致が必要。陸所有者はハッシュ比較であり、都市以外の地形・資源・ユニット位置全体は比較しない。最後に1ターン進める。

## JAC_PoC_JapanMapScale：旧縮尺試験（保存記録）

以下は領土・都市を持たない旧版の20/25/30/40 km比較の記録で、現行ファイルや手順には対応しない。20 kmの実機結果も旧条件によるもの。現行の受け入れ結果として扱わない。

対象は SSOT 16.1「日本列島マップの縮尺」と「ターン処理とメモリ」。独自ルールセット「万世紀」（`JAC_PoC_BanseikiRuleset`）で遊ぶ前提で検証する。**生成・静的検査済み、Civ6 の実機検証は未実施**。最終縮尺はこの表から決めず、実機で開始・50ターン進行・セーブ・ロードを比較してから決める。

| 対辺間の距離 | 盤面 | 総タイル | 日本の陸タイル | `.Civ6Map` 容量 | 本拠からの最大移動 | 3タイル以内の本拠の組 |
|---:|---:|---:|---:|---:|---:|---:|
| 40 km | 79×36 | 2,844 | 278 | 319,488 B | 20.9 km | 24 |
| 30 km | 104×47 | 4,888 | 483 | 368,640 B | 16.8 km | 12 |
| 25 km | 124×55 | 6,820 | 698 | 417,792 B | 13.9 km | 10 |
| 20 km | 154×67 | 10,318 | 1,089 | 503,808 B | 10.8 km | 4 |

Civ6 は都市から3タイル以内に別の都市を建てられない（`CITY_MIN_RANGE`）。最後の列の組は、両方が本拠の位置に首都を置けない。20 km でも織田と松平、伊達と蘆名、足利と六角・本願寺が該当する。組の一覧は `Maps/manifest.json` の `close_start_pairs`。

距離は正六角形の向かい合う辺の間隔。中心 (138°E, 36°N) の Lambert 正積方位図法を45°回し、南西から北東へ伸びる列島を盤面の横軸に近づける。北が画面上向きではない試作地図で、面積は投影回転で変わらない。奇数行を半タイル東へずらした Civ6 のオフセット座標を使う。北海道から与那国・石垣・宮古・沖縄・徳之島・奄美まで含み、30/40 km で小島が中心点抽出から消える場合は Natural Earth の島ポリゴン上の基準点を1タイルとして残す。日本以外の陸地（朝鮮半島・サハリン・大陸）は海にする。対象から離れた小笠原・硫黄島・南鳥島は、この北海道〜琉球の縮尺試験範囲に含めない。国境は Natural Earth の de facto 表現に従う。

陸・海は Natural Earth 10m land と admin-0 countries の日本領域の**両方**に含まれるかで判定する。陸に隣接する水タイルは海岸、それ以外は外洋。標高は NOAA ETOPO 2022 の60秒 DEM を3分間隔で取得し、400 m 以上を丘、1,200 m 以上を山として近似した。山は通行不可。湖・河川・資源は入れていない。土壌や気候も未再現で、陸の基底地形は草原。標高区分は地形性能試験用の暫定値で、戦略地形の完成形ではない。

22人は SSOT 3.1 の20家・琉球王国・アイヌの本拠に置く。開始地点は米沢・黒川・小田原・平井・躑躅ヶ崎・春日山・駿府・那古野・岡崎・一乗谷・室町殿・勝瑞・観音寺・石山本願寺・山口・月山富田・吉田郡山・岡豊・府内・鹿児島・首里・二風谷の概略座標。三好は1534年頃の阿波の拠点である勝瑞を使う（[日本遺産の資料](https://www.japan-heritage.bunka.go.jp/ja/datas/files/2024/03/15/ad9723e8037d6fd0fa0cf1cc61fc87d2a1213da3.pdf)）。城でない室町殿・石山本願寺・二風谷は「本拠／地点」として扱う。座標、選ばれたタイル、移動距離、指導者と文明は `Maps/manifest.json` に全件記録する。各地点は日本の陸上で、生成マップ内でも重複せず通行可能。

この PoC の目的は地図と22人の負荷測定であり、22家の専用指導者はまだない。人間は万世紀プールの織田信秀（`JAC_PoC_CustomCivs`、織田の本拠）、武田の本拠は武田信虎（AI）、残り20か所は**公式の指導者と公式の文明**の AI にする。万世紀の規則に合わせ、指導者は重複させず、文明は重複してよい（ギリシャ・インドが2人ずつ）。公式の指導者を仮の文明に付け替えると、同じ指導者が2つの文明に登録されて「ゲームの作成」画面がプレイヤー設定を不正と判定するため、仮の文明は作らない。1534年の人物や領土をこの割当で主張しない。後続の勢力 PoC で専用の人物に置き換える。

### データの出典と利用条件

| 用途 | 作品 | 作者・年 | 所蔵・出典 | ライセンス | 元ファイル |
|---|---|---|---|---|---|
| 海岸線・島 | Natural Earth 10m land, v5.1.1 | Tom Patterson、Nathaniel Vaughn Kelso ほか、版5.1.1 | [Natural Earth Land](https://www.naturalearthdata.com/downloads/10m-physical-vectors/10m-land/) | [パブリックドメイン](https://www.naturalearthdata.com/about/terms-of-use/) | `tools/japan_map_scale/cache/ne_10m_land.zip` |
| 日本以外の陸の除外 | Natural Earth 10m admin-0 countries, v5.1.1 | 同上 | [Natural Earth Admin 0](https://www.naturalearthdata.com/downloads/10m-cultural-vectors/10m-admin-0-countries/) | 同上 | `tools/japan_map_scale/cache/ne_10m_admin_0_countries.zip` |
| 標高 | ETOPO 2022 60 arc-second ice surface elevation、日本周辺の3分抽出 | NOAA NCEI、2022 | [NOAA ETOPO 2022](https://www.ncei.noaa.gov/products/etopo-global-relief-model)、[抽出元 ERDDAP](https://oceanwatch.pifsc.noaa.gov/erddap/griddap/ETOPO_2022_v1_60s.html) | [NOAA メタデータの CC0 1.0](https://www.ncei.noaa.gov/access/metadata/landing-page/bin/iso?id=gov.noaa.ngdc.mgg.dem%3Aetopo_2022) | `tools/japan_map_scale/cache/etopo_2022_japan_3arcmin.csv` |

取得物はすべて Git 除外の `tools/japan_map_scale/cache/` に保存し、生成器が SHA-256 を照合する。公式 Civ6 の `Base/Assets/Maps/EarthMaps/WorldMapHuge.Civ6Map` は SQLite **スキーマだけ**の参照元で、公式の地形・画像は生成物へ複製しない。Python の標準ライブラリだけで生成できる。4つの生成マップはいずれも10 MB未満なので Git に収録する。

サイズの根拠：公式 Huge は手元の `Base/Assets/Gameplay/Data/Maps.xml` で106×66。付属 `WorldMapHuge.Civ6Map` には130×66の静的マップ例がある。[YnAMP の作者による設定文](https://github.com/Gedemon/Civ6-YnAMP/blob/master/Configuration/ConfigText.xml)では Enormous が128×80、Giant が180×94で終盤不安定、230×116はフルサイズでロード不可と記される。25 km は公式 Huge に近い6,820タイル、20 km は Enormous に近い10,318タイルになる。ただし盤面の縦横比もAI・GSの処理も異なるため、これらは受け入れ保証ではない。

### 再生成・検証手順

```powershell
python tools/japan_map_scale/generate.py
python -m unittest discover -s tools/japan_map_scale/tests -v
python -m unittest discover -s tools/civ6-autotest/tests -v
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/japan-map-40km.json --check
```

実機では共通の準備に従って `poc/JAC_PoC_JapanMapScale` を `Mods/JAC_PoC_JapanMapScale` から参照できるようにする。シナリオは万世紀ルールセット・カスタム勢力・成長 UI・本 MOD を有効にし、Bundle0 を無効にする。都市国家は0にする。万世紀の設定画面に「日本列島 40/30/25/20 km」4候補が出ることと日英露の表示を確かめる。各 `japan-map-<km>km.json` を `scenario.py` で別の `--output` ディレクトリへ実行する。再実行時は JSON 内の `save` と `load` の名前を両方新しい名前にする。シナリオは22人の指導者と開始地点（自分の本拠から1タイル以内）、都市国家0、50ターンの自動進行、セーブ、ロード、ロード後の生存勢力数、ロード後1ターンを確認する。`autoplay-metrics-1.jsonl` と `-2.jsonl` に観測したターンごとの経過秒数・作業セット・プライベートメモリを残し、飛ばしたターンは `duration_seconds: null` とする。`steps.jsonl` に保存・ロード所要時間も残り、`judge_profile=japan_map` は本 MOD のゲーム内テキストの適用と `Database.log`・`Modding.log` エラーを検出する。フロントエンドの Config 適用ログは実行前の基準位置より古いことがあるため、設定画面の候補表示とシナリオで選んだ地図の実寸を別途確認する。ログの無関係な本体エラーは切り分けて記録する。

### 実機結果（2026-09-29、20 km）

| 項目 | 結果 |
|---|---|
| 設定画面 | 万世紀ルールセットで「Japan archipelago 20 km/hex」を選べた。人間は織田信秀。オーナーが「ゲーム開始」を押して開始（`--owner-start`） |
| 開始 | 盤面154×67、主要文明22、都市国家0（GS の自由都市は数えない）。22人とも自分の本拠から1タイル以内 |
| 50ターン | 1→51 を自動進行。1ターン平均1.86秒、中央値1.89秒、最大2.3秒。プライベートメモリ約7.8〜7.9 GB でほぼ横ばい、作業セット最大約3.4 GB |
| セーブ・ロード | セーブ、メニューへの復帰、ロード、ロード後1ターンが成功 |
| 生存 | 51ターン時点で22勢力中16。蘆名・武田・今川・松平・足利・六角が滅亡（本拠が密集する南東北・東海・畿内） |
| ログ | 本 MOD 由来のエラーなし。公式 CivRoyale シナリオの中国語訳ファイル欠落の警告が出るが、本 MOD とは無関係 |

この試験は開拓者で始める Civ6 標準の開始で行った。万世紀の日本マップは、選んだ指導者の史実の領土を持って開始する（開拓者なし）ため、開始条件を実装してから負荷と安定性を測り直す。

実機で未確認なのは、設定画面の登録、22人の実際の開始位置、各縮尺の50ターン進行・セーブ・ロード・次ターン、万世紀（GS 基盤）でのクラッシュとログ、ターン時間・メモリ、3言語表示。4候補の結果を比較し、成功した中で最も細かい縮尺を束0の結論として記録する。50ターンでは終盤の海面上昇や長期肥大は判定できないので、採用候補は別途長時間試験に回す。SSOT 3.4 の最終値は実機確認まで変更しない。

## 束0：ターン処理とメモリ・長時間プレイの計測

SSOT [16.1](../spec/ssot/16-roadmap.md) の2項目を測るシナリオを追加した。**早送りで保存・復元と計測出力の実機確認に成功**（2026-10-01、`perf-18`・`perf-19`）。通常進行300ターンの性能・長期安定性は未検証。目標値や許容範囲は計測結果から決めるため、この追加では数値を設定しない。

- `japan-15km-300turn-full-metrics.json` は15 km史実領土版、22勢力、統合試験と同じ7つの JAC PoC MODで300ターンを進める。
- `japan-15km-20turn-full-metrics.json` は同じ構成を20ターンで確認する。
- 人間は設定の先頭（枠0）の琉球。操作可能な人間がロードに必要なので、人間が滅亡した実行は失敗にする。
- ターン1の保存とロードを測った後、300ターン版では50ターンずつ進め、51・101・151・201・251・301で保存容量を測る。途中ではロードしない。20ターン版では21で保存する。
- 最後にロードし、ターン・時代・台帳全体・資源の保存プロパティ・史実イベントの発生回数・全勢力の生存／都市／軍勢／研究・全タイルの所有者が保存前と一致することを確認する。その後1ターン進める。

成果物はターン時間とメモリのCSV、保存容量とロード時間のCSV、`result.json` の集計。時間の定義、欠測、実行方法は [autotest.md](autotest.md#計測の成果物) を参照する。`perf-18` は開始時と21ターンの保存・復元、`perf-19` は開始時と301ターンの保存・復元に成功し、どちらも復元後の次ターンが通った。300ターン版は272ターンを省き、28ターンを通常進行した。CSV単体にも早送りの印を残す。これらの確認だけで束0の性能・長期プレイの成功とは判定しない。

PR #79 の確認には、叙事詩・マラソンの最初の3回の改元、10番目以降の時代、音楽家0人の偉人画面、資源の年のシナリオも追加した。対象と根拠は [元号＝時代](era-equals-reign.md#pr-79-の追加確認シナリオ) に記す。

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

2026-09-30 の大きさ調整：オーナーから「手帳のアイコンが両隣より大きすぎる」との指摘を受け、図柄の最大辺を 24px から 16px に縮めた（`build_art.py` の `NOTEBOOKS_GLYPH`）。公式の図柄は BLP 内にあり取り出せないため、上記の実機画像 `04-normal-full.png`（等倍）で、26×26 枠内の金色の画素を数えて測った。World Rankings は幅23×高さ18・223px、Era は20×22・175px、Reports は14×17・181px。旧手帳は23×24・約470px で、面積が両隣の2倍を超えていた。本は塗りつぶしの形で線画より重く見えるため、外形でなく面積を両隣の平均（約190px）に合わせ、15×16・約205px とした。`tools/jac_art/tests` の `test_notebooks_glyph_matches_official_neighbours` が、DDS の図柄の幅・高さが公式3種の範囲（±1px）に入り、面積が平均の±15%に収まり、中心が枠の中央から1px以内であることを検査する。比較画像は Git 除外の `tools/jac_art/build/previews/notebooks-button-vs-neighbours.png`（同じ実機画像の手帳ボタンの図柄を消し、新しい DDS を重ねたもの。背景の消し跡は合成による）。実機での見え方は `leader-portraits.json` シナリオの `notebooks-hook-bar-closeup.png` で確認する（未実施）。

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
| 手帳と人物帳 | `notebooks opened` 5件、`character journal shown count=2` 4件。メニュー・人物帳・保存前後の画像あり。**固定3行の削除前の結果**：日本語の固定行に英語表示で欠字が出たため、同梱の NotoSansCJKjp をその行に指定して再検証し、日露英の固定3行を表示できた |
| 新規ゲーム | `no saved state`、開始2ユニットへ `assigned`、`initialized count=2`。1体目の装備2件・Artifact、2体目の装備1件を確認 |
| 成長 | 両ユニットともターン1～5に毎回20 XP。ターン2で Lv2・残り10 XP（Lv1必要量30）、保存時は Lv2・70 XP |
| 保存・ロード | `restored count=2`。両ユニットの全記録と人物帳の表示が保存前の Lv2・70 XP に一致。追加の `assigned` はなく、次のターン6で Lv3・5 XP |
| 3言語 | **固定3行の削除前の結果**：英語表示では英語の項目名と日露英の固定3行を確認。日本語・ロシア語表示への切り替えは未確認。固定3行は、訳を並べて表示しない方針（SSOT 14.2）により 2026-09-29 に削除した |


このブランチの追加自動実行は `tools/civ6-autotest/scenarios/growth-notebooks.json` を使い、`tools/civ6-autotest/artifacts/growth-run-13/` に保存した（41/41 ステップ、ログ判定 13/13 件）。詳細な確認結果は本節の表を参照。


要オーナー確認：表示言語 `ja_JP`・`ru_RU` での文言とロシア語項目名の収まり。Options の表示言語変更はアプリ再起動を要求し、現ドライバはゲームの再起動を扱わない。人物帳を開いた状態で ESC を押して手帳が閉じるかも実機確認が必要。自動入力では公式 Reports List でも ESC がゲーム全体のメニューを開いたため、閉鎖を判定できなかった。

未検証：マルチプレイ。`LastXPTurn` で同じターンの二重加算は防いでいるが、`Unit:SetProperty` の同期とクライアント間の一致はシングルプレイの結果からは確認できない。マルチプレイで採用する前に、2人プレイで加算回数とロード後の一致を確認する。

## JAC_PoC_CustomCivs：カスタム勢力

対象の PoC：SSOT 16.1 束1「カスタム勢力」。別 Civilization ID の同時参加、AI の独立行動、ゲーム途中の家名表示変更を調べる。言語表示も束0の一部として確認する。

MOD の内容：

- `CIVILIZATION_JAC_ODA`／`LEADER_JAC_ODA_NOBUHIDE` と `CIVILIZATION_JAC_TAKEDA`／`LEADER_JAC_TAKEDA_NOBUTORA` を追加する。勢力能力・指導者能力は効果なしの検証用
- `Players:Expansion2_Players` に両組だけを登録する。公式 GS と独自ルールセット「万世紀」が `RulesetDomainOverrides` でこのドメインを選ぶため、標準ルールセットの選択候補には入らない。FrontEnd の `RuleSetInUse` 条件は起動後に標準から GS へ切り替えた際に登録が再実行されなかったため、公式拡張と同じドメイン方式を使う。指導者アイコン・家紋・ロード画面・外交画面の画像は自前（下記「画像」）。色は北条の配色を流用し、組合せで両家を区別する
- 日英露の名前・説明・基本外交文を登録する。都市名は家ごとの史実の地名一覧を使う（[city-names.md](city-names.md)。以前は公式の日本の一覧を共有していた）
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

> **差し替え前の記録。** 以下は織田・武田の2人だけが自前の画像で、残る20人が織田の画像を共用していた時点の記録である。現行の22人の画像参照・出典・検証は [1534年日本22勢力の指導者肖像](leader-portraits.md) を参照する。

[リポジトリルール](../repository-rules.md)の「画像」に従い、2人の指導者と2つの文明に新規生成した画像を割り当てた。2人の指導者は既存 Civ6 指導者画を画風だけの参照とし、同じ質感・照明の 3D 絵画調に揃えた。公式画像の人物や画素は MOD に含めない。

| 画像 | テクスチャ名 | 使われる場所 |
|---|---|---|
| 指導者アイコン | `JAC_Leaders{32,45,48,50,55,64,80,256}`（2×1 のアトラス、0番＝織田、1番＝武田） | 設定画面、右上の指導者一覧、外交の一覧など |
| 文明アイコン（家紋） | `JAC_Civs{22,30,32,36,44,45,48,50,64,80,128,200,256}`（全サイズ白＋透明のアルファマスク） | 設定画面、都市の旗、文明一覧など |
| 文明・指導者の能力アイコン | `JAC_Abilities{22,30,32,36,44,45,48,50,64,80,128,200,256}`（4×1。0＝織田家、1＝武田家、2＝織田信秀、3＝武田信虎。全サイズ白＋透明） | アイコン定義に登録済み。標準の設定画面・百科事典では専用絵柄は使われない（下記参照） |
| ロード画面の肖像・背景 | `LEADER_JAC_<指導者>_NEUTRAL`（570×1024、髷の上に余白を確保）・`_BACKGROUND`（1920×1080） | ロード画面、ゲーム終了画面、設定画面の肖像ツールチップ |
| 外交画面の肖像 | `FALLBACK_NEUTRAL_JAC_<指導者>`（`Leaders.artdef` の `Leader_BLP_Entry`・`Leader_Background_BLP_Entry` は、公式の 2D 専用指導者 `DLC/WarMachineScenario/ArtDefs/Leaders.artdef` と同じく空にする） | 外交画面 |
| 外交画面の背景 | `LEADER_JAC_<指導者>_DIPLOMACY_BACKGROUND`（1600×1000）を `DiplomacyInfo` に指定。仮指導者20人は織田の背景を共用し、`LeaderScene` の標準レターボックスを22人で隠す | 外交画面 |

`tools/jac_art/build_art.py` の CustomCivs 用パッケージには、アイコンアトラス34点（指導者8＋文明13＋能力13）、指導者画像6点（各指導者のロード肖像・ロード背景・外交背景）、フォールバック画像2点が入り、テクスチャ合計は **42点**。20人の仮指導者アイコンは織田のアトラス0番を、文明アイコンはゲーム本体の日本文明アトラス12番を参照し、ロード・外交画像も織田のテクスチャを共用する。GrowthUI が無い場合、コンタクトシートも同じ42点を並べる（GrowthUI がある場合は別途手帳画像を1点加える）。先行記載の「41」は数え違いであり、コンタクトシートとパッケージで数え方が違うわけではない。スクリプトの定義・出力数はいずれも42点。

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

Pillow で単色背景から人物・手帳を透過処理し、指導者の円形アイコン、家紋・能力の白いアルファマスクと各サイズのアトラスを作る。外交背景は元の城内画像を16:10へ切り出す。SDK の Asset Cooker は3個の BLP を一時ディレクトリに生成する。3個すべての更新時刻・容量・テクスチャ名を検証してから MOD 内の BLP を置き換えるため、Cooker が途中で失敗しても配置済みの BLP は残る。2026-09-29 の先行ビルドでは GrowthUI 不在のままスキップし、3 BLP の再生成が成功した。中間 `.dds`・`.tex`・`.xlp` と全テクスチャの PNG・コンタクトシートは、この作業ツリー内の `tools/jac_art/build/` に置く。元画像・Cooker 出力・手帳 DDS は Git 管理外とし、MOD を配布・実機検証する端末では事前にビルドする。ビルド後に `python -m unittest discover -s tools/jac_art/tests -v` を実行すると、FrontEnd の22行から参照するアイコン・肖像とゲーム内のフォールバックを、ローカルの Civ6 本体アイコン定義と生成済み BLP に照らして検査する。ゲーム本体が標準の Steam インストール先にない場合は `CIV6_INSTALL_DIR` を設定する。

今回の「?」の調査では、2026-09-29 14:31 の `ArtDef.log` に `JAC_CustomCivs_Icons.blp`・`JAC_CustomCivs_Leaders.blp`・`JAC_CustomCivs_Fallback.blp` の `Unable to resolve Package` が記録され、実機側の CustomCivs MOD フォルダに BLP がなかった。`FrontEndActions` の `UpdateIcons` と `UpdateArt` は適用されているため、画像名の登録だけでは画素を読み込めない。公式の Portugal DLC と同じく、設定画面には `UpdateArt` と `UpdateIcons` を用い、BLP に収録した UI テクスチャを `.dep` の `UITexture` ライブラリで参照する。UI 用スクリプトの `ImportFiles` はこの画像のロード経路ではない。今回の作業ツリーでは BLP 3個を再生成し、GrowthUI があるため43点のコンタクトシートと手帳 DDS も作った。これらは Git 除外で作業ツリー内にあり、実機の MOD フォルダには配置していない。実機の再表示確認は未実施。

選定した指導者肖像は `civ6-art-local/trial-02/gen_gemini.py`、人物切り抜き用画像・背景・家紋・手帳は `civ6-art-local/generate_final.py` で生成した。両方ともローカル OpenCodex の `http://127.0.0.1:10100/v1/responses` に `google-antigravity/gemini-3.1-flash-image` を指定し、Antigravity の OAuth サブスクリプション枠を使った（API キーや従量課金 API は不使用）。これは生成経路の事実を記録したもので、規約上この経路が許可されるという判断ではない。生成元画像と Cooker の画像出力はローカルの `civ6-art-local/` にのみ保存し、Git には含めていない。プロンプト・生成時刻・出力ファイルは `civ6-art-local/generation-log.jsonl` に保存する（先行して作った肖像2点の時刻は元ファイルの更新時刻から補記）。既存の PD 写真4点は削除せず、`Originals/_old_pd/` に保管した。

生成画像の来歴と利用条件（規約確認日：2026-09-29。下表の現行画像すべてに共通）：

| 対象 | 生成器・経路 | 生成日 | 参照素材 | 規約・再配布 |
|---|---|---|---|---|
| 下表の指導者アイコン、立ち姿、背景、家紋、能力アイコン、手帳ボタン | Gemini 3.1 Flash Image（`google-antigravity/gemini-3.1-flash-image`）。Google Antigravity のサブスクリプション枠をローカル OpenCodex 経由で使用。API キー・従量課金 API は不使用 | 2026-09-29（日本時間） | 公式 Civ6 の指導者画は画風参照のみ。公式画像の人物・画素を生成物に複製せず、リポジトリや MOD に同梱・配布しない | [Google 利用規約](https://policies.google.com/terms)、[生成 AI の禁止用途ポリシー](https://policies.google.com/terms/generative-ai/use-policy)、[Antigravity 追加利用規約](https://antigravity.google/terms)。生成物の再配布・公開の可否は未確定。この画像を含む PoC の公開・再配布は、[質問 D11](../questions.md) のオーナー判断が完了するまで保留する |

Antigravity 追加利用規約の第6項は第三者ソフトウェア経由のアクセスを禁止している。OpenAI 利用規約も自動・プログラムによるデータまたは出力の抽出を禁止しているため、ChatGPT Web UI で出力取得をブラウザーエージェントに自動化する案にも規約上の懸念がある（[OpenAI 利用規約](https://openai.com/policies/row-terms-of-use/)）。今後の生成経路は、ChatGPT のサブスクリプション枠で公式の Codex CLI の画像生成を使うことに決まった（SSOT 15.2.1）。この PoC 画像の公開・再配布と作り直しは [質問 D11](../questions.md) に残す。規約へのリンクは生成画像に再配布許諾があることを示すものではない。

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

SSOT 2.2「ゲームモードと指導者プール」および 16.1 束0の検証。`poc/JAC_PoC_BanseikiRuleset` を独立した PoC MOD にした。ルールセットとステージング設定を既存のゲームプレイ PoC から分けることで、FrontEnd の設定登録を常時適用し、各 PoC のゲーム内アクションをルールセット条件で評価できる。3つの既存 MOD は `RULESET_JAC_BANSEIKI` と比較用の `RULESET_EXPANSION_2` の両方で読み込む。GS を残す理由は、既存シナリオの回帰確認と両ルールセットの差分調査である。**この PoC は設定と候補フィルターの機構を検証するもの**で、SSOT 2.2 の段階制限（異宇宙に行けるまでマップ別の固定指導者プール・固定プレイヤー数）は未実装。現在の開始画面ではプール設定と人数変更が可能で、AI も全プールの対応済み指導者を候補にする。

新ルールセットの `Rulesets` 行は `RULESET_JAC_BANSEIKI`、`GameCore=Expansion2`、`IsScenario=0`、`SupportsSinglePlayer=1`、`SupportsMultiPlayer=1`。マルチプレイの実動作は未検証。基本の勝利条件ドメインとマップ候補は GS を使う。戦国ツリーPoCを有効にしたときだけ、勝利条件を下記の専用ドメインへ切り替える。GS が `RULESET_EXPANSION_2` 専用に宣言する通常オプション、災害の強度（`Realism`）、自然遺産・都市国家除外、公式の指導者プール1・2、および開始時代・ターン制限の設定更新を、このルールセットにも宣言する。

指導者候補には `Players:Expansion2_Players` を共用する。これは公式と JAC の候補を持つドメインで、`RULESET_JAC_BANSEIKI` の `PlayerLeader` に `RulesetDomainOverrides` で指定する。織田・武田の行は `JAC_PoC_CustomCivs/Config/JAC_CustomCivs_Players.xml` に一度だけ登録し、新 MOD は CustomCivs を依存 MOD とする。`JAC_BanseikiHumanPool` 設定の既定は `JAC_BANSEIKI_ONLY`、もう一方は `JAC_ALL_POOLS`。`DomainValueFilterQueries` と `QueryParameters` がルールセット・プレイヤー ID・設定値を読み、シングルプレイの人間枠 0 かつ既定値のときだけ候補を織田信秀・武田信虎との積集合にする。AI 枠と「全プール」指定の人間枠には、下記の非対応指導者を除く公式・JAC の候補を残す。この仕組みは FrontEnd の設定 DB だけで動き、ゲームプレイ Lua や UI 差し替えは不要。

2026-09-29 にこの PC の公式 `DLC/*/*.modinfo` 全 37 件を読み取り専用で監査した。うち 23 件の FrontEnd アクションが `Players:Expansion2_Players` に指導者を登録する。各 InGame アクションの参照する判定条件とゲーム内 `<Leaders>` 定義を突き合わせ、`any="1"` と `LeaderPlayable` により別の条件でも有効になるアクションは除外対象に数えなかった。万世紀で候補だけ残り、指導者定義の読み込み条件を満たさないものは次の 1 件。

| DLC | 指導者 / 文明 | 根拠（公式ファイル:行） | 万世紀での扱い |
|---|---|---|---|
| GreatBuilders | `LEADER_SEJONG` / `CIVILIZATION_KOREA` | `DLC/GreatBuilders/GreatBuilders.modinfo:36,38,61`（XP2 専用の InGame アクション）、`DLC/GreatBuilders/Data/GreatBuilders_ConfigData_Korea.xml:7`（FrontEnd 登録）。XP1 側も同 `.modinfo:32-34,58-59` に限定 | `JAC_Banseiki_UnsupportedLeaderFilter` が `difference` で人間・AI の全候補から除外 |

他の公式指導者の基礎定義には、万世紀でも成立する InGame アクションがある。公式ファイルは変更せず、専用クエリにルールセット条件を付けた。`LEADER_SEJONG` が元のドメインに無い環境では除外行は返さず、公式 `RULESET_EXPANSION_2` の候補も変えない。XP2 専用の追加能力や各ゲームモードの完全な互換性までは、この基礎定義の監査だけでは保証しない。

2026-09-29 の設定画面と FrontEnd Lua の結果：**設定部分は成功**。英語 UI の「Create Game」ルールセット欄に `Banseiki` が現れ、選択すると `Human Leader Choices` の既定 `Use Banseiki pool only` が表示された。`Use all leader pools` への切替も画面で確認した。**非対応指導者の除外を追加する前**の `HostGame` の `CreatePlayerParameters(id, true)` と `Refresh()` では、既定時は人間枠 2 候補（両 JAC）、AI 枠 1・2 は各 82 候補（公式クレオパトラと両 JAC を含む）。全プールを選ぶと人間枠も 82 候補となった。スクリーンショットは Git 管理外の `tools/civ6-autotest/artifacts/banseiki-ui-01/ruleset-selected.png` と `pool-all.png`。3言語のタグは登録したが、ゲーム画面での表示確認は英語のみ。除外追加後の候補数・世宗の不在は要実機確認。

ゲーム開始の初回実行 `tools/civ6-autotest/artifacts/banseiki-full-01/` は、ロード画面後にメニューへ戻り、開始ステップで失敗した。`Modding.log` と `Database.log` では公式 GS の `Expansion2Core` と `Expansion2CoreContent` が適用されず、XP1/XP2 の参照整合性エラーでゲームプレイ DB がロールバックされた。この時点の `Target Mods` にルールセット MOD 自体が入っていなかった。診断用に一時的に `IsScenario=1` として起動した `banseiki-flag1-01/` はゲーム内まで進み、3 MOD の `[JAC_POC]` ログも出たが、`GameClimate` と `Game.GetWorldCongress` は存在せず、Base コアの動作だった。対照の公式「黒死病」`blackdeath-core-control-01/` は同じドライバで `GameClimate=true`、`Game.GetWorldCongress=function`、`Expansion2Core` の適用を確認した。これは後述の成功結果より前の診断記録である。

2026-09-29、この PC で日本マップのブランチを重ねた状態の実機結果：`RULESET_JAC_BANSEIKI` のシングルプレイを 22 主要文明で開始し、Gathering Storm の仕組みが存在する状態で 50 ターン進行、保存・ロードに成功した。設定画面では `NO_DUPLICATE_CIVILIZATIONS=0` の導入前に同文明の別指導者を拒否し、導入後は受け入れた。これらは日本マップのブランチとの組合せ結果で、PoC 単独実行や世宗を選ぶ経路の成功証拠ではない。

以前の静的検証では XML 6 ファイルの解析、`banseiki-ruleset.json --check`（27 ステップ）、自動テスト 84 件、`git diff --check` が成功した。これは当時の差分の検査記録であり、今回の除外クエリとシナリオ変更を含まない。今回の静的検証では、除外クエリを監査結果と突き合わせて SQLite で人間・AI 枠に適用するテストを追加し、自動テスト全 86 件、改訂シナリオの `--check`（29 ステップ）、XML 解析、`git diff --check` に成功した。Civ6 の実行結果ではない。

最終の自動シナリオは `tools/civ6-autotest/scenarios/banseiki-ruleset.json`。ルールセットとプールの画面記録、設定値と候補リストの Lua 照会、`NO_DUPLICATE_LEADERS=1`・`NO_DUPLICATE_CIVILIZATIONS=0` の FrontEnd 値、織田（人間）・クレオパトラ／武田／ゴルゴー／ペリクレス（AI）の開始を検査する。ゴルゴーとペリクレスは同じギリシャ文明で別指導者。さらに 3 MOD のログ、気候画面と世界会議 API、数ターンの自動進行、保存・メニューへ戻る・ロードを検査する。この**改訂後の全シナリオは未実行**であり、上記 22 主要文明・50 ターンの実機確認とは別の受入項目である。

後続の受入条件：異宇宙に行けるまでのマップ別固定指導者プールと固定プレイヤー数（SSOT 2.2。マルチプレイは例外）を実装し、開始画面と AI 選択の両方で確認する。PoC 単独でも万世紀を開始・進行・保存・ロードし、GS 中核と改訂後の全シナリオを確認する。GreatBuilders 有効時に世宗が人間・AI の候補に出ないことを設定画面で確認する。長期プレイでの海面上昇と日本語・ロシア語表示も未確認。

**要実機確認（マルチプレイ）**：SSOT 2.2 はマルチプレイを上記段階制限の例外とするが、複数の人間プレイヤーに `JAC_BanseikiHumanPool` をどう適用するかは未確定。現行クエリは `PLAYER_ID=0` を人間枠と仮定し、それ以外の人間枠には既定の「万世紀プールのみ」を適用しない。複数人の設定画面で各枠の候補を確認し、適用範囲を決めてから実装する。

### 単独の実機結果（2026-09-29、`banseiki-ruleset.json` 31手順）

全自動（`restart_game` から）で31手順すべて成功、判定2/2合格。設定画面で万世紀を選べ、既定の人間候補は万世紀プールの2人、すべてのプールでは81人（世宗を除外して82→81）。ルールセット選択後に `NO_DUPLICATE_LEADERS`=true・`NO_DUPLICATE_CIVILIZATIONS`=false。ゴルゴーとペリクレスが同時にギリシャとして開始。気候・世界会議が存在し、気候パネルを開閉できた。3ターン進行、セーブ、メニューへの復帰、ロード（Bundle0 の状態 turnsSeen=4 と万世紀ルールセットを復元）、ロード後1ターンが成功。

シナリオ側で直した点：ルールセットの ConfigurationUpdates は設定画面のパラメータを組み立てた後に反映されるため、重複ルールの確認は HostGame の `BuildHeadlessGameSetup` の後に行う。設定値は真偽値で返る。HostGame の設定確認の後は設定が既定に戻るため、開始直前に `configure` をもう一度行う。`wait_for` は直前の手順のログ位置を使うため、読み込み時のログ待ちは開始の直後に置く。

## JAC_PoC_SengokuTrees：戦国の技術・社会制度ツリー

`poc/JAC_PoC_SengokuTrees` は `RULESET_JAC_BANSEIKI` だけで適用する独立PoC。設計案の技術32件・制度30件を元ID・AND前提・費用・行位置のまま登録した。時代は元号と同じにした（オーナー決定 T）。48の元号を時代として登録し、改元の手番でエンジンの時代が変わる。ツリーの列の見出しも元号になる（[時代＝元号 PoC](era-equals-reign.md)）。黄金・暗黒などの年齢は、公式の時代スコアではなく武功の台帳で改元ごとに判定する（[年齢 PoC](sengoku-ages.md)）。誓約は未実装。公式研究行は、必要な公式アイテムを戦国ノードへ再配線してから削除する。公式アイテムは新規Unit・建物の代用品で、効果量は暫定。制度の列は標準CivicsTreeが再計算するため、設計表の列位置の再現は未確認。

各ノードの解放先・残るTODO・実装した2件のBoostは[照合表](sengoku-tree-unlocks.md)。公式定義を参照したGSの時代・気候・世界会議・AI・勝利設定への処理と未検証事項は[互換処理メモ](sengoku-tree-compatibility.md)。戦国ツリーPoCが有効なBanseikiでは、専用の空の勝利ドメイン、公式勝利7種の設定値とゲーム側の既定値の無効化、`TURNLIMIT_NONE` と最大ターン0で統一前の公式勝利を防ぐ。ターン上限は Banseiki 本体の `GAME_TURN_LIMIT` の設定更新を上書きする（`ConfigurationUpdates` には一意キーがなく、行を足すと `TURNLIMIT_GAMESPEED` と競合する）。登録済み全城（琉球王国・アイヌを含む）の支配による天下統一勝利は `VICTORY_JAC_SENGOKU_UNIFICATION` として実装した（[天下統一 PoC](unification-victory.md)）。2026-09-30 の実機で、検証地図の破却・再建・移管を経て織田の天下統一が成立すること（unify-15）と、15 km 地図が登録城56で始まること（unify-16）を確認した。開拓者開始・通常地図の本拠の条件、チーム・同盟での合算、マルチプレイは未確認。約200ターンの年表示は未実装。

設計表から `python tools/generate_sengoku_trees.py` でSQL・翻訳・アイコン・照合表を生成する。`python -m unittest` は62 ID、前提DAGと到達性、配置重複、費用、3言語キー、SQLの模擬適用、勝利設定と公式勝利無効化を検査する。`python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/sengoku-trees.json --check` はシナリオJSONだけを検査する。ゲームを起動する版のシナリオは、ゲーム作成画面と同じく設定を組み立ててターン上限が `TURNLIMIT_NONE` になること、Banseiki開始、公式勝利が無効でゲーム内の終了ターン（`Game.GetGameEndTurn()`）が0であること、両公式ツリー画面の候補、開始時に与える技術4・社会制度3を持つこと、その次に研究できる技術と制度の進捗を検査する（開始時の知識を与える前は最初のノードを研究させていたが、与えた後は研究済みで経路が得られず失敗していた。実機 sengoku-cascade-deletes-03）。

**2026-09-29 の提示された実機結果：** `RULESET_JAC_BANSEIKI` と日本マップでゲームがロードし、戦国技術32件・社会制度30件、公式技術・制度行0件、Gathering Stormの仕組みの存在を確認。50ターンの自動進行とセーブ／ロードが成功した。今回の公式勝利無効化を加える前の結果である。

**2026-09-29 の自動実機結果（勝利無効化後）：** 上記シナリオ21手順が通り、判定2/2。公式勝利7種が無効、終了ターン0で画面上部のターン表示は「TURN 1」（上限の表示なし）、両ツリー画面は戦国ノードだけ（技術32・制度30）、3ターンの自動進行で最初の技術と制度が進んだ。対照として戦国ツリーPoCを外した同じ設定では、制覇・スコアなどの公式勝利が有効で、終了ターンは501だった。自動テストが作成画面を経ずに開始すると設定の組み立てが走らず、ターン上限は既定のまま残る。そのためシナリオは開始前に設定を組み立てる。

**要実機確認：** 人が作成画面から始めたときのターン制限の表示と選択肢、500ターンを超える長期進行。ツリー画面の前提線、研究選択、Boostの操作も人が確認する。開始年は「4000 BC」のままで、1534年開始の暦は未実装。

## JAC_PoC_ReligionPersistence：束2の宗教持続

番号付きの仮宗教で、信徒・地下化・滅亡・教典・探索分岐・再興をGameプロパティへ保存する。[宗教持続の検証記録](religion-persistence.md)に実装範囲、公式APIの行番号、実機で実行するシナリオをまとめた。D8のオリジナル宗教9種は定義していない。

2026-10-01：確定回答に従い、信仰20・信徒1人の再興と、資料の8つの使い道を実装。資料の再利用、市民の宗教変換、宗教同盟、公式Heroes & Legendsの英雄に対応した。宗教用のlupaテスト67件を含む全453件と、自動検証ツール212件が成功。実行補助は不要で、Luaのスキップなし。100ステップ・Lua63片のシナリオ検査も成功。実機は6段階の保存・再読込が成功し、吸収・無宗教の保持・実産出・返還が成功。英雄の未完了予約を含む10段階の保存・再読込と残る効果を検証中。検証モードだけで期限を短縮し、実際のエンジンで待機期間と同盟終了を確認する。自然な探索・布教の入力連携と未対応宗教の自動の対応付け、画面、標準の弾圧への接続、マルチプレイは今回のPoCの範囲外。
