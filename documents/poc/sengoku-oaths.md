# 戦国パックの家固有の誓約（JAC_PoC_SengokuTrees）

SSOT [3.2](../spec/ssot/03-map-and-scale.md#32-時代パック)の「各家に固有3つ、共通の誓約なし」を実装する。織田家の1つは「天下布武」。名前、史実から効果への対応、数値はこのPoCの暫定案で、オーナーの追加決定とは扱わない。人物個人ではなく家に属するため、長尾→上杉・松平→徳川・織田→羽柴→豊臣にも同じ3つを引き継ぐ。刀狩・楽市楽座・朱印状などの一般社会制度名は家の誓約に転用しない。

史実は自治体・博物館・大学などの資料で確認した。誓約名の大半はゲーム用の創作であり、史料の誓言の引用ではない。1534年の当主がすべての事業を行ったとは扱わず、先代・後継世代の事績も含める。アイヌの3つは複数の地域共同体の営みから作り、タリコナによる全域統治とは結び付けない。

## 1. 公式の Commemorations の調査

以下の `Base/`・`DLC/` はすべて `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/` からの相対パス。公式ファイルは読むだけとし、成果物には収録しない。UIで Dedication と呼ぶ機能のDB名が Commemoration で、Gathering Storm の日本語の「誓約」に対応する。

| 対象 | 公式の仕組み | 根拠（file:line） |
|---|---|---|
| 候補の表 | `CommemorationTypes` は名前・黄金／通常／暗黒の説明、最初／最後の時代を持つ。家や文明の列は無い | `DLC/Expansion2/Data/Expansion2_Schema.sql:86-96` |
| 効果の表 | `CommemorationModifiers` が ModifierId を結ぶ。`ComplimentModifiers` も参照する予約表で、この環境の公式GSデータには行が無い。Modifier の条件と引数が効果を決める | 同 `:80-84,98-102`、`DLC/Expansion2/Data/Expansion1_Moments.xml:511-563`。空の表は公式DBの読み込みによる確認 |
| 時代ごとの候補 | 科学・文化は古典〜中世、基盤・宗教は古典〜ルネサンス、探検・経済はルネサンス〜近代など、XP1由来の11候補にGSの情報時代の自動化を足した12候補を登録する | `DLC/Expansion2/Data/Expansion1_Moments.xml:348-360`、`Expansion2_Moments.xml:78-80` |
| 選べる数 | 通常・暗黒・黄金は1つ、英雄は3つ。最初の時代と開始ターン・観戦者は選択画面を出さない | `DLC/Expansion2/Text/en_US/Expansion1_Eras_Text.xml:130-140,154-164`、`DLC/Expansion2/Data/Expansion2_GlobalParameters.xml:62-64`、`DLC/Expansion2/UI/Additions/DedicationPopup.lua:35-39,65-75` |
| 通常・暗黒の役割 | 出来事ごとの時代スコアのクエスト。科学のブースト・文化のブースト・建造物・交易路の完了など | `DLC/Expansion2/Data/Expansion1_Moments.xml:775-855,1134-1208` |
| 黄金・英雄の役割 | 強い効果。`PLAYER_HAS_GOLDEN_AGE` などの条件を付ける。クエストの条件は非黄金または特別な許可（例外の文明能力） | 同 `:568-657`、`DLC/Expansion2/Data/Expansion1_Leaders.xml:1952-1955,2054-2061` |
| 候補と年齢の取得 | `Game.GetEras()` の `GetPlayerCommemorateChoices`・`GetPlayerNumAllowedCommemorations`・年齢APIを使う | `DLC/Expansion2/UI/Additions/DedicationPopup.lua:41-65,125-140` |
| 確定 | 候補の Index を `PlayerOperations.COMMEMORATE` で送り、公式エンジンに選択を保存させる | 同 `:206-215` |
| 画面と表示 | `Commemoration` インスタンス、年齢ごとの絵・ボタン・アイコン。公式は中高優先度の QueuePopup | 同 `:25,79-99,110-156,220-226`、`DedicationPopup.xml:6-61` |

黄金の量の基準は、軍事ユニット生産 +15%（`Expansion1_Moments.xml:646-649,979-982`）、科学・文化ブースト +10（同 `:866-871,1009-1012`）、開拓者購入の割引30%（同 `:968-976`）、水軍・乗船移動 +2（同 `:894-903`）。通常・暗黒は1回の達成 +1点の引数を使う（同 `:1134-1208`）。本PoCは全都市の産出を主に +10% と補助 +5%、軍事生産 +15%、水軍・防壁の限定生産 +20%、人口成長 +5〜10%に抑える。公式と同じ「1つ／3つ」「クエスト／強い効果」を保つ一方、家ごとの強さが等しいと実証したわけではない。短い元号と22家の都市数の差は、約200ターンの実機進行で調整する。

## 2. 元号と武功の台帳への接続

[元号＝時代](era-equals-reign.md)の48時代と[武功の台帳](sengoku-ages.md)をそのまま使う。標準速度の最初の改元は弘治の66ターン目、次は永禄の73ターン目。速度とエンジンの時代の読み方を誓約側で二重に持たず、台帳の `Judge` が確定した期と年齢を受け取る。

公式の年齢は常に通常となるよう停止し、公式の選択数・提示数も0のままにする（既存 `Data/JAC_Sengoku_Ages.sql`。公式パラメータは `Expansion2_GlobalParameters.xml:62-66,126,250-264`）。公式の `GetEras` から家の黄金・英雄・暗黒を判定すると台帳と食い違うため、新しい誓約の年齢・候補・保存は台帳が持つ。

| 台帳の年齢 | 提示 | 確定数 | 効果 |
|---|---:|---:|---|
| 開始時の天文（NONE） | 0 | 0 | 最初の改元まで誓約を持たない |
| 通常期・暗黒期 | 自家の3つ | 1 | 表の達成条件ごとに武功の台帳へ +1点。強い Modifier は働かない |
| 黄金期 | 自家の3つ | 1 | 選んだ誓約の強い Modifier が次の改元まで働く |
| 英雄期 | 自家の3つ | 3 | 自家の3つを一括で確定し、3つとも強い Modifier が働く |
| 最終の遅代 | 自家の3つ | 1 | 台帳の強制暗黒を尊重。暗黒期の誓約として達成の点を持ち、次の改元は無い |

人間は改元後、自分の手番が始まってから確定する。読み込み直後は活動前でも画面を表示し、確定要求だけを活動中の手番に限る。画面の選択はプレイヤー・家・期・年齢に結び付け、同じ表示の更新では保持し、別の表示に移れば消す。次のターン開始まで答えなければAIと同じ選び方で1つ（英雄期は3つ）を決定する。[既存設計 §6.2・§7.2](../research/sengoku-ages.md#62-マルチプレイ)の暫定運用を使う。公式も振り返り画面を閉じると誓約画面へ進む（`DLC/Expansion2/UI/Additions/EraReviewPopup.lua:241-247`、`DedicationPopup.lua:293`）。本実装のUIは改元の画面→誓約→暗黒期の政策の順。閉じた画面は表示を消すだけで、選択内容はゲームプレイ側が決める。暗黒期の政策は既存の別の救済効果として残る。

保存は `JAC_AGES_LEDGER.houses.p<ID>` の `oaths`（OathType→true）、`oathReign`・`oathChosen`・`oathDue`・`oathKnowledge`。空文字を使わず、未確定値は nil、表のキーは文字列。既存の Game プロパティと都市旗の保存が実機で通った事実を前提とする（[台帳の保存の結果](sengoku-ages.md)）。今回の誓約そのものの実機保存確認は下記シナリオで行う。

クエストは既存の捕獲・同盟／友好宣言の点が受理されたときに足す。捕獲イベントと次ターンの差分は同じ記録なので二重加点しない。技術と社会制度は別々に数え、確定前に完了した研究へ遡って加点しない。改元のターン開始で取り込む前の手番の研究は締める期に入り、改元後の捕獲は新しい期に入る。公式の ERA_SCORE Modifier は再開しない。

## 3. データ・Modifier・UI・AI

元データは [66件のカタログ](data/sengoku-oaths.json)。`cargo run --locked --manifest-path tools/Cargo.toml --bin generate-sengoku-oaths --` が SQL と3言語の名前・黄金効果・達成条件を生成し、`--check` が更新漏れを検査する。`JAC_SengokuOaths` の OathType と CivilizationType が所有を表し、`JAC_SengokuOathAges` が選べる4種の年齢、`JAC_SengokuOathEffects` が各誓約の ModifierId を結ぶ。CustomCivsが後から読まれる場合にもSQLを通すため、CivilizationTypeの外部キーは持たず、テストで登録済み22家との一致を確認する。表に無い文明には誓約を作らない。

プレイヤーIDと誓約IDの順を固定して、全都市の旗 `<OathType>_ON` を `REQUIREMENT_PLOT_PROPERTY_MATCHES` の `PropertyMinimum=1` で読む（公式 `DLC/CivRoyaleScenario/Data/CivRoyaleScenario_Civilizations.xml:388-407`）。旗は台帳の保存と同じ処理で、ターン開始・確定・捕獲・都市建設／移管に書き直す。所有者が変わった城では古い家の66個の旗を消し、今の家の選択だけを付ける。

| 表の効果 | 実際の ModifierType と引数 | 公式の根拠 |
|---|---|---|
| 生産・ゴールド・科学・文化・信仰・食料 | `MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_MODIFIER`、YieldType・Amount、Subject側に都市旗の条件 | `Base/Assets/Gameplay/Data/Modifiers.xml:1055-1058`、`DLC/Expansion2/Data/Expansion1_Policies.xml:276-282` |
| 軍事ユニットの生産 | `MODIFIER_PLAYER_CITIES_ADJUST_MILITARY_UNITS_PRODUCTION`、Amount、Subject側の都市旗 | `DLC/Expansion2/Data/Expansion1_Moments.xml:646-649,979-982` |
| 人口成長 | `MODIFIER_PLAYER_CITIES_ADJUST_CITY_GROWTH`、Amount、Subject側の都市旗 | `Base/Assets/Gameplay/Data/Modifiers.xml:835-838`、`Policies.xml:394-397,1550-1554` |
| 防壁の生産 | `MODIFIER_PLAYER_CITIES_ADJUST_BUILDING_PRODUCTION`、BuildingType・Amount、Subject側の都市旗。古代・中世・ルネサンスの防壁に各1行（BUILDING_WALLS・BUILDING_CASTLE・BUILDING_STAR_FORT） | `Base/Assets/Gameplay/Data/Policies.xml:716-728,2568-2597` |
| 水軍の生産 | 全都市へ `MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER` で `MODIFIER_CITY_ADJUST_UNIT_DOMAIN_PRODUCTION` を付ける。Domain=DOMAIN_SEA・Amount、子のOwner側を都市旗で判定 | `Base/Assets/Gameplay/Data/Modifiers.xml:780-783`、公式 `DLC/Expansion2/Data/Expansion2_Districts.xml:123-125` と `Expansion2_Modifiers.xml:282-286`、既存の `Data/JAC_Sengoku_Ages.sql` と同じ方式 |

全効果は主要文明の共通特性 `TRAIT_LEADER_MAJOR_CIV` から付ける。公式も年齢効果をここに付ける（`DLC/Expansion2/Data/Expansion1_Leaders.xml:138-139`）。その家が選んでいる間だけ旗で働かせるため、効果を毎改元に追加する処理や、外すAPIは使わない。ModifierId は基本 `<OathType>_E1`・`_E2`、防壁は `_ANCIENT`・`_MEDIEVAL`・`_RENAISSANCE`、水軍は追加の `_ATTACH`。具体的な対応は上記効果表のDBに全件入る。

公式の `DedicationPopup.xml` の3候補の部品と年齢の絵をそのまま使い、`ReplaceUIScript` で Luaだけ差し替える。LoadOrder=200で万世紀の家固有名の差し替えより後にする。候補の `Name`・効果説明・Iconだけを差し込み、公式の候補取得・COMMEMORATE操作は呼ばない。UI／ゲームプレイのLuaの名前は別にする。表示は既存の改元画面と同じ、入力を独占しない `ContextPtr:SetHide`（公式の先例 `DLC/Expansion2/UI/Additions/DisloyalCityChooser.lua:23-24,54`、既存の実機結果は[時代画面](sengoku-ages.md)）。

人間の確定は平坦なスカラー引数 `Reign` と `Oath1`・`Oath2`・`Oath3` を `EXECUTE_SCRIPT` で送る（公式 `DLC/BlackDeathScenario/UI/BlackDeathScenario_PopupHandler.lua:21-28`、`Scripts/BlackDeathScenario_StateUtils.lua:64-74`）。エンジンが渡す要求者を本人とし、引数のPlayerIDは信じない。ゲームプレイ側は現在の期・未確定・家の所有・年齢・重複と個数を検査し、合わない要求を捨てる。自分の手番が活動中かは `IsTurnActive` で確認する（公式 `Base/Assets/UI/ActionPanel.lua:688`、開始前の操作が捨てられた実機事実は[朝廷裁定](court-arbitration.md)）。確定後の暗黒政策画面は `GameCoreEventPublishComplete` を待つ（公式 `Base/Assets/UI/Choosers/ResearchChooser.lua:485`）。

AIは乱数を使わず、表示順を同点の順位とする。戦争中は軍備 +100、金が50未満なら経済 +80、海岸の城を持つ水軍志向は +60（海岸が無ければ −100）、平時の学問 +20、城が2以下の成長 +30。家の3つだけから得点が最大のものを選ぶ。英雄期は3つ全部。APIはゲームプレイの `IsAtWarWith`（`DLC/NubiaScenario/Scripts/NubiaScenario.lua:86`）、`GetTreasury():GetGoldBalance`・`IsCoastalLand`（`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:404,270`）、`IsHuman`（`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:1198`）だけを使う。これは暫定の評価で、交易路や全戦略の最適化を主張しない。

公式の候補の除去は戦国ルールセットの読込条件の中だけで行う。候補を消す前に `Moments.CommemorationType` と外交の参照をNULLへし、公式の史実タイムライン行が外部キーの連鎖で消えないようにする（`Expansion2_Schema.sql:489,140`）。専用の取り付け2表とAIの旧候補の好みも消す。素のGathering Stormは変更しない。

## 4. 22家の誓約

全66件は **通常・暗黒・黄金・英雄の4種すべて**で選べる。以下の強い効果は黄金・英雄のみ、達成の点は通常・暗黒のみ。強い効果の対象はすべて自家の全都市。名前と効果を他家にも配る共通誓約は無い。似た制度が複数の家にあったことは尊重し、効果の型が同じでもOathType・名前・所有は分ける。

### 織田 — CIVILIZATION_JAC_ODA

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_ODA_TENKA_FUBU | 天下布武 / Tenka Fubu / Тэнка фубу | 軍事ユニットの生産 +15%、文化 +5% | 城の奪取ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_ODA_TSUSHIMA | 津島の富 / Wealth of Tsushima / Богатство Цусимы | ゴールド +10%、人口成長 +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_ODA_AZUCHI | 安土の石垣 / Stone Walls of Azuchi / Каменные стены Адзути | 防壁の生産 +20%、生産 +5% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |

- **天下布武**：岐阜県歴史資料館の所蔵文書に、信長の「天下布武」印が現存する。統一志向を軍備と統治文化へ抽象化する。 [天下布武の史実資料](https://www.pref.gifu.lg.jp/page/2557.html)
- **津島の富**：湊町津島の財力が信定・信秀・信長の三代を支えた。津島市計画書の歴史解説（5頁）を参照。 [津島の富の史実資料](https://www.city.tsushima.lg.jp/shokai/kankouannai/rekishibunka/town/rekisibunkakekka.files/shakaisihonkeikaku.pdf)
- **安土の石垣**：安土築城で職人を動員し、高度な石垣技術を用いた。後継世代の城郭整備を家の営みとする。 [安土の石垣の史実資料](https://www.pref.shiga.lg.jp/sd00/2591.html)

### 武田 — CIVILIZATION_JAC_TAKEDA

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_TAKEDA_TSUTSUJIGASAKI | 躑躅ヶ崎の府 / Seat of Tsutsujigasaki / Столица Цуцудзигасаки | 生産 +10%、文化 +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_TAKEDA_KOSHU_LAW | 甲州の法 / Laws of Koshu / Законы Косю | ゴールド +10%、科学 +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_TAKEDA_RYUO | 竜王の川除 / Ryuo Riverworks / Дамбы Рюо | 食料 +10%、人口成長 +10% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |

- **躑躅ヶ崎の府**：1519年に信虎が居館を移し、家臣集住と城下整備を進めた。 [躑躅ヶ崎の府の史実資料](https://www.city.kofu.yamanashi.jp/rekishi_bunkazai/documents/takeda-omote.pdf)
- **甲州の法**：晴信の分国法「甲州法度」による領国統治を着想源とする。 [甲州の法の史実資料](https://www.city.kofu.yamanashi.jp/shingenkou-no-machi/)
- **竜王の川除**：1560年の保坂家文書は、免税による移住募集、耕地化と堤防維持を伝える。 [竜王の川除の史実資料](https://www.pref.yamanashi.jp/bunka/bunkazaihogo/bunkazai_data/yamanashinobunkazai_wc0017.html)

### 伊達 — CIVILIZATION_JAC_DATE

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_DATE_JINKAISHU | 塵芥の秩序 / Order of Jinkaishu / Порядок Дзинкайсю | ゴールド +10%、文化 +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_DATE_BLACK_LACQUER | 黒漆の備え / Black Lacquer Guard / Чернолаковая защита | 軍事ユニットの生産 +15%、ゴールド +5% | 城の奪取ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_DATE_IWADEYAMA | 岩出山の再起 / Revival at Iwadeyama / Возрождение в Ивадэяме | 生産 +10%、食料 +5% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |

- **塵芥の秩序**：稙宗が1536年に制定した塵芥集は、土地取引・境界などを詳細に規定した。 [塵芥の秩序の史実資料](https://www.pref.miyagi.jp/soshiki/bunkazai/05jinkaishu.html)
- **黒漆の備え**：政宗所用として伝来する16世紀の五枚胴具足と、家中に継承された具足形式に基づく。 [黒漆の備えの史実資料](https://www.city.sendai.jp/museum/shuzohin/shuzohin/shuzohin-05.html)
- **岩出山の再起**：1591年、政宗が岩出山を新たな居城とした領国再編。城の修復自体は家康によるものと区別する。 [岩出山の再起の史実資料](https://www.city.osaki.miyagi.jp/kanko/sizen/kouen/4294.html)

### 蘆名 — CIVILIZATION_JAC_ASHINA

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_ASHINA_KUROKAWA | 黒川の政庁 / Council of Kurokawa / Совет Курокавы | 文化 +10%、ゴールド +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_ASHINA_MUKAIHAGURO | 向羽黒の要害 / Mukaihaguro Stronghold / Твердыня Мукайхагуро | 防壁の生産 +20%、生産 +5% | 城の奪取ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_ASHINA_AIZU_ALLIANCES | 会津の連衡 / Alliances of Aizu / Союзы Айдзу | ゴールド +10%、文化 +5% | 新たな同盟・友好宣言ごと +1点（同じ組・同種は1期1回、開始時の関係は除外） | 通常・暗黒・黄金・英雄 |

- **黒川の政庁**：黒川城が蘆名氏の公的な政治拠点として機能したという城郭研究に基づく。 [黒川の政庁の史実資料](https://mukaihaguro-yabou.jp/kikou)
- **向羽黒の要害**：盛氏が築いた山城には、曲輪・虎口・九十九折の道が残る。 [向羽黒の要害の史実資料](https://www.town.aizumisato.fukushima.jp/soshiki/1007/6/3/1219.html)
- **会津の連衡**：蘆名家臣宛の北条氏照書状が、盛氏父子への共同出兵要請を伝える。 [会津の連衡の史実資料](https://khirin.rekihaku.ac.jp/en/pid/nmjh_collection/H-1907-24.html)

### 北条 — CIVILIZATION_JAC_HOJO

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_HOJO_REVENUE | 貫高の政 / Assessed Revenue / Учёт земельных доходов | ゴールド +10%、食料 +5% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_HOJO_SERVICE | 知行の軍役 / Service by Fief / Воинская повинность уделов | 軍事ユニットの生産 +15%、文化 +5% | 城の奪取ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_HOJO_ODAWARA | 小田原の総構 / Odawara Enclosure / Внешние укрепления Одавары | 防壁の生産 +20%、人口成長 +10% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |

- **貫高の政**：検地・貫高による賦課と、1550年の税制改革を伝える現存文書に基づく。 [貫高の政の史実資料](https://odawara-digital-museum.jp/selection/definition/24/)
- **知行の軍役**：1581年の軍役文書は、知行高に応じた兵員数・装備を具体的に規定する。 [知行の軍役の史実資料](https://www.city.odawara.kanagawa.jp/field/lifelong/property/cultural/komonjyo/k-houzyou3.html)
- **小田原の総構**：氏政・氏直期に城と町全体を囲む総構を築いた。 [小田原の総構の史実資料](https://odawaracastle.com/history/hojo-godai/)

### 山内上杉 — CIVILIZATION_JAC_YAMANOUCHI_UESUGI

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_YAMANOUCHI_UESUGI_KANREI | 管領の責務 / Duty of the Kanrei / Долг канрэя | ゴールド +10%、文化 +5% | 新たな同盟・友好宣言ごと +1点（同じ組・同種は1期1回、開始時の関係は除外） | 通常・暗黒・黄金・英雄 |
| JAC_OATH_YAMANOUCHI_UESUGI_ASHIKAGA_SCHOOL | 足利の学灯 / Light of Ashikaga School / Свет школы Асикага | 科学 +10%、文化 +5% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_YAMANOUCHI_UESUGI_HIRAI | 平井の城塁 / Ramparts of Hirai / Валы Хираи | 防壁の生産 +20%、生産 +5% | 城の奪取ごと +1点 | 通常・暗黒・黄金・英雄 |

- **管領の責務**：関東管領として公方を補佐し、関東の政治秩序を担った家の制度的役割に基づく。 [管領の責務の史実資料](https://www.city.fujioka.gunma.jp/soshiki/kyoikuiinkai/bunkazaihogo/2/5/uesugisi/1091.html)
- **足利の学灯**：憲実が足利学校を再興し、庠主の任命と貴重な書籍の寄進を行った。 [足利の学灯の史実資料](https://www.city.ashikaga.tochigi.jp/category/000000/p001343.html)
- **平井の城塁**：山内上杉の拠点平井城では、内外の土塁・堀・石積補強が発掘されている。 [平井の城塁の史実資料](https://www.city.fujioka.gunma.jp/soshiki/kyoikuiinkai/bunkazaihogo/2/3/siseki/1113.html)

### 長尾（上杉へ継承） — CIVILIZATION_JAC_NAGAO

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_NAGAO_KASUGAYAMA | 春日山の備え / Kasugayama Defenses / Оборона Касугаямы | 防壁の生産 +20%、人口成長 +5% | 城の奪取ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_NAGAO_DISPATCHES | 小切紙の急報 / Secret Dispatches / Тайные донесения | 軍事ユニットの生産 +15%、科学 +5% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_NAGAO_RINSENJI | 林泉寺の学び / Learning at Rinsenji / Учение в Ринсэндзи | 文化 +10%、信仰 +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |

- **春日山の備え**：1513年には山城として確認でき、謙信・景勝期に拡張された。現存城域全体を為景の築造とはしない。 [春日山の備えの史実資料](https://www.city.joetsu.niigata.jp/site/cultural-property/cultural-property-jpn002.html)
- **小切紙の急報**：1510年の為景書状は、敵の発見を避ける極小の料紙で軍事連絡を伝えた（米沢市上杉博物館学芸ノート、3頁）。 [小切紙の急報の史実資料](https://www.denkoku-no-mori.yonezawa.yamagata.jp/pdf/dayori/dayori39.pdf)
- **林泉寺の学び**：能景が1497年に創建した長尾家菩提寺で、景虎が教養を学んだ。 [林泉寺の学びの史実資料](https://www.city.joetsu.niigata.jp/site/cultural-property/cultural-property-city010.html)

### 今川 — CIVILIZATION_JAC_IMAGAWA

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_IMAGAWA_KANA_LAW | 仮名の秩序 / Order in Kana / Порядок, записанный каной | ゴールド +10%、文化 +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_IMAGAWA_RENGA | 駿河の連歌 / Renga of Suruga / Рэнга Суруги | 文化 +10%、科学 +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_IMAGAWA_SUNPU_CRAFT | 駿府の革工 / Leatherworkers of Sunpu / Кожевники Сумпу | 生産 +10%、ゴールド +5% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |

- **仮名の秩序**：氏親の1526年「今川仮名目録」と、寿桂尼・氏輝・義元への領国統治の継承に基づく。 [仮名の秩序の史実資料](https://scmh.jp/special_exhibition/20260425.html)
- **駿河の連歌**：連歌師宗長が今川氏に仕え、文化と外交に関わった。 [駿河の連歌の史実資料](https://scmh.jp/special_exhibition/20260425.html)
- **駿府の革工**：1549年の義元文書は、革職人の商いを認め、今川御用を命じる職人保護を示す。 [駿府の革工の史実資料](https://jmapps.ne.jp/shizuokalg/det.html?data_id=4019)

### 松平（徳川へ継承） — CIVILIZATION_JAC_MATSUDAIRA

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_MATSUDAIRA_ANJO | 安城譜代 / Anjo Retainers / Наследственные вассалы Андзё | 軍事ユニットの生産 +15%、信仰 +5% | 城の奪取ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_MATSUDAIRA_OKAZAKI | 岡崎の礎 / Foundations of Okazaki / Основание Окадзаки | 生産 +10%、食料 +5% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_MATSUDAIRA_DAIJUJI | 大樹の祈り / Prayers of Daijuji / Молитвы Дайдзюдзи | 信仰 +10%、文化 +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |

- **安城譜代**：安城を拠点とした時期に形成された家臣団。恒常的に一枚岩だったとはせず、家臣組織を題材とする。 [安城譜代の史実資料](https://www.library.city.anjo.aichi.jp/book/shiryo/kyodo2/documents/21siro.pdf)
- **岡崎の礎**：清康期に安城から岡崎へ本拠を移した拠点整備を着想源とする。 [岡崎の礎の史実資料](https://www.city.anjo.aichi.jp/shisei/shisetsu/kyoikushisetsu/maibun-sites-ansyoujou.html)
- **大樹の祈り**：親忠による1475年の大樹寺創建と、松平家の寺院保護に基づく。 [大樹の祈りの史実資料](https://www.city.okazaki.lg.jp/bunka/torikumi_bunka/1004568/1004592/1004595/1004622.html)

### 朝倉 — CIVILIZATION_JAC_ASAKURA

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_ASAKURA_ICHIJODANI | 一乗谷の町割 / Streets of Ichijodani / Улицы Итидзёдани | ゴールド +10%、人口成長 +10% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_ASAKURA_ECHIZEN | 越前の工房 / Workshops of Echizen / Мастерские Этидзэна | 生産 +10%、科学 +5% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_ASAKURA_SUWA_GARDEN | 諏訪館の庭 / Garden of Suwa Manor / Сад усадьбы Сува | 文化 +10%、ゴールド +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |

- **一乗谷の町割**：発掘で確認された道路・屋敷配置と、100尺を基準とする計画的な城下に基づく。 [一乗谷の町割の史実資料](https://asakura-museum.pref.fukui.lg.jp/site/index)
- **越前の工房**：刀装具・ガラス製品を製作した、朝倉直属と考えられる職人集団の工房跡に基づく。 [越前の工房の史実資料](https://asakura-museum.pref.fukui.lg.jp/site/walk)
- **諏訪館の庭**：諏訪館跡に残る大規模な庭園と室町末期の作庭文化。妻のために造ったという伝承は根拠に含めない。 [諏訪館の庭の史実資料](https://asakura-museum.pref.fukui.lg.jp/site/index)

### 足利 — CIVILIZATION_JAC_ASHIKAGA

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_ASHIKAGA_MEDIATION | 公方の和議 / Shogunal Mediation / Посредничество сёгуна | 文化 +10%、ゴールド +5% | 新たな同盟・友好宣言ごと +1点（同じ組・同種は1期1回、開始時の関係は除外） | 通常・暗黒・黄金・英雄 |
| JAC_OATH_ASHIKAGA_HIGASHIYAMA | 東山の雅 / Elegance of Higashiyama / Изящество Хигасиямы | 文化 +10%、信仰 +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_ASHIKAGA_TEMPLE_LANDS | 寺領の安堵 / Protection of Temple Lands / Защита храмовых земель | 信仰 +10%、科学 +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |

- **公方の和議**：1576年、義昭が北条氏規に北条・武田・上杉の和睦を勧めた現存文書に基づく。1573年後の裁定権を回復させる効果ではない。 [公方の和議の史実資料](https://ch.kanagawa-museum.jp/dm/gohojyo/collection/d_collection_12.html)
- **東山の雅**：義政が1482年から造営した東山殿を中心に展開した文化。現在の庭園全体を当初の姿とは扱わない。 [東山の雅の史実資料](https://www2.city.kyoto.lg.jp/somu/rekishi/fm/nenpyou/htmlsheet/bunka10.html)
- **寺領の安堵**：義晴の御内書は、天皇からの依頼を受けた醍醐寺理性院の所領安堵を伝える。 [寺領の安堵の史実資料](https://khirin.rekihaku.ac.jp/en/pid/nmjh_collection/H-743-421-11.html)

### 三好 — CIVILIZATION_JAC_MIYOSHI

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_MIYOSHI_IIMORI | 飯盛の石塁 / Iimori Ramparts / Валы Иимори | 防壁の生産 +20%、生産 +5% | 城の奪取ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_MIYOSHI_SAKAI | 堺の交誼 / Bonds of Sakai / Связи с Сакаи | ゴールド +10%、文化 +5% | 新たな同盟・友好宣言ごと +1点（同じ組・同種は1期1回、開始時の関係は除外） | 通常・暗黒・黄金・英雄 |
| JAC_OATH_MIYOSHI_THOUSAND_VERSES | 千句の座 / Circle of a Thousand Verses / Круг тысячи стихов | 文化 +10%、科学 +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |

- **飯盛の石塁**：長慶が1560年に入城した飯盛城では、石垣・礎石建物・瓦の早期導入が発掘で確認される。 [飯盛の石塁の史実資料](https://www.pref.osaka.lg.jp/o070080/toshimiryoku/osakathemuseum/miyoshinagayoshi.html)
- **堺の交誼**：長慶・実休らは堺の掌握を重視し、有力町人と茶の湯や連歌を通じて交流した。 [堺の交誼の史実資料](https://www.city.sakai.lg.jp/kanko/hakubutsukan/exhibition/kikaku_tokubetsu/kako/74549120220902185140834.html)
- **千句の座**：1561年の連歌会「飯盛千句」など、三好一族の文化交流を着想源とする。 [千句の座の史実資料](https://www.city.sakai.lg.jp/kanko/hakubutsukan/exhibition/kikaku_tokubetsu/kako/74549120220902185140834.files/tenzihin.pdf)

### 六角 — CIVILIZATION_JAC_ROKKAKU

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_ROKKAKU_KANNONJI | 観音寺の堅城 / Kannonji Stronghold / Твердыня Каннондзи | 防壁の生産 +20%、生産 +5% | 城の奪取ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_ROKKAKU_ISHIDERA | 石寺の賑わい / Ishidera Prosperity / Процветание Исидэры | ゴールド +10%、人口成長 +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_ROKKAKU_OMI_COMPACT | 近江の盟約 / Compact of Omi / Договор Оми | 文化 +10%、ゴールド +5% | 新たな同盟・友好宣言ごと +1点（同じ組・同種は1期1回、開始時の関係は除外） | 通常・暗黒・黄金・英雄 |

- **観音寺の堅城**：六角氏の居城・観音寺城では、大石による石塁が主要な曲輪を囲む。 [観音寺の堅城の史実資料](https://www.pref.shiga.lg.jp/sd00/2592.html)
- **石寺の賑わい**：1549年の石寺新市に関する記録を着想源とする。市場政策全般を六角専有とはしない。 [石寺の賑わいの史実資料](https://www.pref.shiga.lg.jp/sd00/2592.html)
- **近江の盟約**：1567年成立の六角氏式目と、家臣による当主権力の規制を含む主従間の法秩序に基づく。 [近江の盟約の史実資料1](https://www.u-tokyo.ac.jp/biblioplaza/ja/F_00231.html)、[近江の盟約の史実資料2](https://syllabus.chs.nihon-u.ac.jp/2022/op/syllabus78454.html)

### 本願寺 — CIVILIZATION_JAC_HONGANJI

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_HONGANJI_LETTERS | 御文の輪 / Letters of Faith / Послания веры | 信仰 +10%、文化 +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_HONGANJI_TEMPLE_TOWN | 寺内の営み / Temple Town Life / Жизнь храмового города | ゴールド +10%、人口成長 +5% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_HONGANJI_OSAKA_DEFENSE | 大坂の守り / Defense of Osaka / Защита Осаки | 防壁の生産 +20%、信仰 +5% | 城の奪取ごと +1点 | 通常・暗黒・黄金・英雄 |

- **御文の輪**：蓮如は御文などを用いて教義を分かりやすく伝え、本願寺教団を拡大した。 [御文の輪の史実資料](https://www.otani.ac.jp/kyo_kikan/museum/nab3mq0000043z6f.html)
- **寺内の営み**：山科本願寺を中心に形成された寺内町と、その居住・都市活動を着想源とする。 [寺内の営みの史実資料](https://www.city.kyoto.lg.jp/yamasina/page/0000012089.html)
- **大坂の守り**：大坂本願寺期の防御性の高い区画施設・V字溝が発掘で確認される（大阪市保存活用計画、42頁）。 [大坂の守りの史実資料](https://www.city.osaka.lg.jp/kyoiku/cmsfiles/contents/0000508/508299/04_p36-46.pdf)

### 大内 — CIVILIZATION_JAC_OUCHI

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_OUCHI_CERTIFIED_ROUTES | 勘合の航路 / Certified Sea Routes / Заверенные морские пути | ゴールド +10%、科学 +5% | 新たな同盟・友好宣言ごと +1点（同じ組・同種は1期1回、開始時の関係は除外） | 通常・暗黒・黄金・英雄 |
| JAC_OATH_OUCHI_WESTERN_CAPITAL | 西京の客人 / Guests of the Western Capital / Гости западной столицы | 文化 +10%、ゴールド +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_OUCHI_PRINTING | 大内の版木 / Ouchi Printing Blocks / Печатные доски Оути | 科学 +10%、文化 +5% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |

- **勘合の航路**：大内氏の朝鮮・中国交易に関係する印が現存し、対外貿易を裏付ける。一般社会制度「朱印状」の名称は専有しない。 [勘合の航路の史実資料](https://online.bunka.go.jp/db/heritages/detail/207120)
- **西京の客人**：大内領には公家・禅僧・学者が来訪し、交易の経済力を背景に文化活動が栄えた。 [西京の客人の史実資料](https://library.pref.yamaguchi.lg.jp/hp/usr/siryou/kichosiryo.htm)
- **大内の版木**：1493年刊の『聚分韻略』など、大内領内の出版事業を着想源とする。 [大内の版木の史実資料](https://library.pref.yamaguchi.lg.jp/hp/usr/siryou/kichosiryo.htm)

### 尼子 — CIVILIZATION_JAC_AMAGO

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_AMAGO_TODA | 富田の要害 / Toda Stronghold / Твердыня Тода | 防壁の生産 +20%、生産 +5% | 城の奪取ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_AMAGO_DAISEN | 大山の造営 / Temples of Daisen / Храмы Дайсэн | 信仰 +10%、生産 +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_AMAGO_RESTORATION | 再興の旗 / Banner of Restoration / Знамя возрождения | 軍事ユニットの生産 +15%、文化 +5% | 城の奪取ごと +1点 | 通常・暗黒・黄金・英雄 |

- **富田の要害**：尼子氏の本拠・月山富田城は、月山と周辺丘陵に築かれた大規模な山城だった。 [富田の要害の史実資料](https://www.pref.shimane.lg.jp/life/bunka/bunkazai/shiseki/shisekidesu/shiseki02.html)
- **大山の造営**：晴久による大山寺造営には作事奉行が置かれ、一族・重臣が仏像制作の施主となった。 [大山の造営の史実資料](https://www.pref.tottori.lg.jp/item/1020156.htm)
- **再興の旗**：尼子勝久を山中鹿之介らが擁立した再興運動を着想源とする。「七難八苦」の逸話には依拠しない。 [再興の旗の史実資料](https://bunkazai.pref.yamaguchi.lg.jp/sp/bunkazai/summary.asp?mid=40028)

### 毛利 — CIVILIZATION_JAC_MORI

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_MORI_THREE_SONS | 三子の結束 / Unity of the Three Sons / Единство трёх сыновей | 生産 +10%、文化 +5% | 新たな同盟・友好宣言ごと +1点（同じ組・同種は1期1回、開始時の関係は除外） | 通常・暗黒・黄金・英雄 |
| JAC_OATH_MORI_SETO_FLEETS | 瀬戸の船手 / Seto Fleets / Флотилии Сэто | 水軍の生産 +20%、ゴールド +5% | 城の奪取ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_MORI_OTA_TOWN | 太田川の城下 / Ota River Town / Город на Оте | ゴールド +10%、人口成長 +10% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |

- **三子の結束**：1557年の現存する三子教訓状は、隆元・元春・隆景の協力と家中の崩壊回避を求めた。「三本の矢」の後世の逸話は根拠にしない。 [三子の結束の史実資料](https://www.mogurin.or.jp/museum/hwm/details/tenzi04/6/t04_6_g3_dai01.html)
- **瀬戸の船手**：毛利一門を支えた小早川隆景の三原城は、船入りを備える水軍拠点だった。 [瀬戸の船手の史実資料](https://www.city.mihara.hiroshima.jp/kankou/mihara.html)
- **太田川の城下**：輝元は1589年から広島城を築き、職人・商人を招いて城下町を建設した。 [太田川の城下の史実資料](https://www.city.hiroshima.lg.jp/shisei/toukei/1006109/1027849/1027852/1017167.html)

### 大友 — CIVILIZATION_JAC_OTOMO

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_OTOMO_SOUTHERN_PORT | 南蛮の津 / Southern Trade Port / Порт южной торговли | ゴールド +10%、科学 +5% | 新たな同盟・友好宣言ごと +1点（同じ組・同種は1期1回、開始時の関係は除外） | 通常・暗黒・黄金・英雄 |
| JAC_OATH_OTOMO_FUNAI_HEALING | 府内の救療 / Healing in Funai / Лечение в Фунаи | 食料 +10%、人口成長 +10% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_OTOMO_FUNAI_LEARNING | 府内の学灯 / Funai Learning / Учёность Фунаи | 科学 +10%、文化 +5% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |

- **南蛮の津**：宗麟期の府内では南蛮貿易が進み、海外産陶磁器や外国人の生活用品も出土する。 [南蛮の津の史実資料](https://www.city.oita.oita.jp/o157/bunkasports/citypromotion/1369370791117.html)
- **府内の救療**：宗麟の支持の下、アルメイダが1555年に育児院、1557年に病院を設立した。 [府内の救療の史実資料](https://www.city.oita.oita.jp/o157/bunkasports/citypromotion/1369370791117.html)
- **府内の学灯**：1581年設立の府内コレジオでは、語学・天文学・哲学・古典などが教えられた。 [府内の学灯の史実資料](https://www.city.oita.oita.jp/o157/bunkasports/citypromotion/1369370791117.html)

### 島津 — CIVILIZATION_JAC_SHIMAZU

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_SHIMAZU_KUROKAWA_GUNS | 黒川の火筒 / Kurokawa Matchlocks / Фитильные ружья Курокавы | 軍事ユニットの生産 +15%、科学 +5% | 城の奪取ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_SHIMAZU_JISSHIN | 日新の学び / Teachings of Jisshin / Наставления Дзиссина | 文化 +10%、信仰 +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_SHIMAZU_UCHIJO | 内城の政庁 / Uchijo Administration / Управление из Утидзё | 生産 +10%、ゴールド +5% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |

- **黒川の火筒**：1549年の黒川崎合戦で島津側の鉄砲使用が記される。「日本初」とは断定しない。 [黒川の火筒の史実資料](https://www.city.aira.lg.jp/airaview/hakken/cape-kurokawa.html)
- **日新の学び**：島津忠良のいろは歌は、学びを実際の行動へ結び付ける倫理的教えを伝える。 [日新の学びの史実資料](https://www.city.tarumizu.lg.jp/bunka/kurashi/kosodate/roman/tarumzusimadubosyo.html)
- **内城の政庁**：1550年に貴久が築いた内城は、交通に便利な恒久的城館として領国経営の拠点となった（調査報告44–45頁）。 [内城の政庁の史実資料](https://www.city.kagoshima.lg.jp/kyoiku/kanri/bunkazai/documents/documents/06kagoshimashichuseijokanatoall.pdf)

### 長宗我部 — CIVILIZATION_JAC_CHOSOKABE

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_CHOSOKABE_TOSA_SURVEY | 土佐の地検 / Tosa Land Survey / Межевание Тосы | 食料 +10%、ゴールド +5% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_CHOSOKABE_HUNDRED_ARTICLES | 百箇条の治め / Rule of the Hundred Articles / Правление по ста статьям | ゴールド +10%、文化 +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_CHOSOKABE_URADO | 浦戸の船路 / Urado Sea Routes / Морские пути Урадо | 水軍の生産 +20%、ゴールド +5% | 新たな同盟・友好宣言ごと +1点（同じ組・同種は1期1回、開始時の関係は除外） | 通常・暗黒・黄金・英雄 |

- **土佐の地検**：元親・盛親期の総検地は1587年から実施され、土佐七郡を記録した地検帳368冊が現存する。一般社会制度「太閤検地」は全家の名称のまま残す。 [土佐の地検の史実資料](https://online.bunka.go.jp/heritages/detail/237391)
- **百箇条の治め**：1597年の法令と奉行・庄屋・刀祢などの行政体系を着想源とする。 [百箇条の治めの史実資料](https://www.city.sukumo.kochi.jp/sisi/017601.html)
- **浦戸の船路**：元親は1591年に浦戸へ移り、港を上方との連絡や水軍保有に利用できる本拠とした。 [浦戸の船路の史実資料](https://www.city.kochi.kochi.jp/site/kanko/uradojyou.html)

### 琉球 — CIVILIZATION_JAC_RYUKYU

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_RYUKYU_BRIDGE | 万国の架橋 / Bridge Between Nations / Мост между народами | ゴールド +10%、水軍の生産 +20% | 新たな同盟・友好宣言ごと +1点（同じ組・同種は1期1回、開始時の関係は除外） | 通常・暗黒・黄金・英雄 |
| JAC_OATH_RYUKYU_KUME | 久米の文才 / Kume Scholarship / Учёность Кумэ | 科学 +10%、文化 +5% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_RYUKYU_OMORO | おもろの響き / Echoes of Omoro / Отзвуки оморо | 文化 +10%、信仰 +5% | 社会制度の完了ごと +1点 | 通常・暗黒・黄金・英雄 |

- **万国の架橋**：1458年の万国津梁の鐘は、琉球の海上交易と諸国を結ぶ役割を伝える現存資料。 [万国の架橋の史実資料](https://okimu.jp/special/bankoku/index.html)
- **久米の文才**：久米村の福建系住民は中国との外交を担い、王国の政治・経済・文化に寄与した。 [久米の文才の史実資料](https://www.city.naha.okinawa.jp/_res/projects/default_project/_page_/001/003/518/japanheritage_story.pdf)
- **おもろの響き**：首里王府による祈りの歌の採録・編纂に基づく。『おもろさうし』第1巻は1531年に編纂され、全巻が同年成立したわけではない。 [おもろの響きの史実資料](https://okimu.jp/userfiles/files/page/museum/issue/bulletin/hakukiyou19/No19_05.pdf)

### アイヌ（地域共同体） — CIVILIZATION_JAC_AINU

| OathType | 日本語 / English / Русский | 黄金・英雄のModifier効果 | 通常・暗黒の達成条件 | 選べる年齢 |
|---|---|---|---|---|
| JAC_OATH_AINU_RIVER_MOUTH | 川口の暮らし / River-Mouth Life / Жизнь у речного устья | 食料 +10%、人口成長 +10% | 技術の完了ごと +1点 | 通常・暗黒・黄金・英雄 |
| JAC_OATH_AINU_NORTHERN_TRADE | 北海の交易 / Northern Seas Trade / Торговля северных морей | ゴールド +10%、文化 +5% | 新たな同盟・友好宣言ごと +1点（同じ組・同種は1期1回、開始時の関係は除外） | 通常・暗黒・黄金・英雄 |
| JAC_OATH_AINU_CHASHI | チャシの備え / Chashi Defenses / Защита часи | 防壁の生産 +20%、生産 +5% | 城の奪取ごと +1点 | 通常・暗黒・黄金・英雄 |

- **川口の暮らし**：小樽地域のアイヌ文化期の河口集落と狩猟・採集、中世のサケ・コンブの流通を着想源とする。資料68–69頁。タリコナによる全域統治や統一制度を意味しない。 [川口の暮らしの史実資料1](https://www.city.otaru.lg.jp/docs/2020120801361/file_contents/soan-rekisibunka-kihonkousou.pdf)、[川口の暮らしの史実資料2](https://hokkaido-digital-museum.jp/hokkaido/kaitaku/k-01/)
- **北海の交易**：中世には本州との物産交換が行われ、16世紀にはアイヌの人々が本州へ出向いて交易したことも説明される。複数の地域共同体の営みを表す。 [北海の交易の史実資料](https://hokkaido-digital-museum.jp/hokkaido/kaitaku/k-01/)
- **チャシの備え**：地域ごとの砦・柵囲いを着想源とする。小樽の例は「おそらく16世紀頃」という推定で、タリコナの築城とは結び付けない。資料69頁。 [チャシの備えの史実資料1](https://ainu-upopoy.go.jp/event/rekishi2023/)、[チャシの備えの史実資料2](https://www.city.otaru.lg.jp/docs/2020120801361/file_contents/soan-rekisibunka-kihonkousou.pdf)

## 5. 検査と実機シナリオ

- `cargo test --locked --manifest-path tools/Cargo.toml --workspace --all-targets`：公式GSのスキーマへ重ねた66候補・年齢・効果・外部キー・ルールセット境界・翻訳・生成物、Luaで捕獲と研究の重複防止、改元の失効、他家／古い期／重複／個数違いの拒否、英雄の一括確定、AIと答えない人間の選択、保存後のスクリプト読み込み直し、実際に描画した3候補とボタンの操作を検査する。Lua のテストは mlua に Lua 5.1 を組み込んで実行する。
- `cargo test --locked --manifest-path tools/Cargo.toml --workspace --all-targets`：既存の進行・保存・自動進行の安全確認と、誓約画面を閉じられること、シナリオの構成とLuaの構文を検査する。
- `cargo run --locked --manifest-path tools/Cargo.toml --bin generate-sengoku-oaths -- --check`：カタログとSQL／翻訳の一致。
- `cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-autotest -- tools/civ6-autotest/scenarios/sengoku-oaths-oda.json --check`：シナリオの形式を検査する。実機合格の意味ではない。

実機では **`sengoku-oaths-oda.json`** を主セッションから実行する。15km日本地図で織田を人間として開始し、最初の改元まで進める。選んだ誓約の強い効果を必ず検査するため、65ターン目だけテスト用に織田の台帳の点を1000へ設定する。この点の操作で年齢判定を検査済みとは主張せず、既存の改元判定を黄金期の条件で通す。

1. 開始時の天文で誓約を持たず、公式候補が無いこと。
2. 66ターン目に台帳が弘治・黄金期を判定し、公式の年齢は通常のまま。
3. 公式の誓約画面に実際に描画された候補が「天下布武」「津島の富」「安土の石垣」の3つだけで、名前・説明も翻訳済み。
4. 活動中の自分の手番で「天下布武」を選び、ゲームプレイが受理したこと、都市旗がその1つだけ立ったこと。
5. 実際の城の文化産出を旗のオン→オフ→オンで比較し、天下布武の文化 +5%のModifierが働くこと。
6. 保存→メニュー→読み込みの直後、次のターンを進める前に選択・期・都市旗の保存を比較し、文化のModifierをもう一度オン／オフ比較すること。
7. 次の改元で古い選択の旗が消えること。

軍事生産 +15%は公式と同じ型・引数と都市旗の条件をDBで検査する。文化の産出比較は少なくとも1つの実効果を確認する実機検査で（公式の都市産出の読み方は `Base/Assets/UI/CitySupport.lua:316`、ゲームプレイからも読めることは既存の台帳の実機検査で確認済み）、軍事生産の速度まで測ったことにはしない。画面の目視、ネットワークの複数人プレイ、全66効果とAIの約200ターンの偏りは **要実機確認**。実機結果は以下に記録する。

既存の `sengoku-ages-screens.json` と `sengoku-dark-policy.json` は、改元画面から誓約、暗黒期の政策へ進む順番を確認する。誓約が未選択なら、改元の通知から開いても政策画面へ進めず、手番終了ボタン・Enter・Shift+Enter・自動終了でも手番を終えられない。終了の阻止は画面や通知の有無ではなく、保存済みの台帳で判定する。実機シナリオは誓約画面を強制表示する前に、自動で出た画面を検査する。

## 検証結果

- 通常のテストと自動実行ツールのテストを実行し、Luaのテストを省略せず成功した。件数と対象コミットはPRコメントに記録する。
- カタログと生成物の一致、シナリオ3本の形式検査を行う。
- `--test-mode` は一人用の万世紀ゲームで長い自動進行の待ちを早送りする。改元の直前から実際の手番を進め、選択・効果・保存・失効を確かめる。連続した約200ターンのバランスや性能を測った結果には含めない。
- サンドボックス用の一時フォルダの変更と、探索のためだけに置いたパッケージ初期化ファイルは除去した。
- 別モデルのレビューと実機結果はPRコメントに記録する。全66効果、軍事生産率、マルチプレイ、長期バランスは別の確認項目として残す。

## 旧保存と友好宣言の追加受け入れ（共有ToDo #134）

`build-oaths-migration` と `build-oaths-friendship` は、製品のLuaを変更せず、有限の実機検査用JSONを指定したフォルダーへ生成するRust実行器である。

```powershell
$env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'tools/target'
cargo run --locked --manifest-path tools/Cargo.toml -p civ6-data-rs --bin build-oaths-migration -- D:/CC/Work/todo134/scenarios
cargo run --locked --manifest-path tools/Cargo.toml -p civ6-data-rs --bin build-oaths-friendship -- D:/CC/Work/todo134/scenarios
```

旧保存は2026年9月30日の `ages-owner-l-05` で実際に作成した2件を使う。保存名は `JAC-SengokuAges-01-ages-owner-l-05-543484304b0c`（天文・34手番）と `JAC-SengokuAges-02-ages-owner-l-05-543484304b0c`（弘治・67手番、判定は66手番）。元の保存を上書きせず、複製・サイズ・SHA-256と作成時の `steps.jsonl` を実機前に照合する。67手番の保存には誓約の5項目が無いこともゲーム内で検査し、現行の保存を旧保存と誤認しない。

- `legacy-capabilities.json` は最初の短い読み取り診断。旧台帳と友好・生産・成長APIの有無を記録する。APIの存在だけを効果や加点の合格には数えない。
- `legacy-control-diagnostic.json` は終手要求前から両側の手番イベントを記録し、要求を1回だけ送る。診断専用の `observe_for` で120秒観測した後と、その10秒後にゲームプレイ側・画面側の操作状態、終手ブロッカー、処理中状態を採る。手番が進まない場合も後半の採取へ到達する。シナリオは `diagnostic_only` を必須とし、結果には観測完了と受け入れ判定を別々に記録する。観測が完了しても受け入れ判定は常にfalseである。短い画面休止の `pause` は従来どおり10秒以下に限定する。
- `legacy-migration.json` は旧保存から自然な次の手番へ進む。人間の新たな選択期限、AIの自家選択、過去の研究への誓約点0、旧判定履歴の維持を検査する。既存の選択画面から確定し、別名で保存・読み込みした直後に選択と全都市の66旗を照合する。
- `legacy-tenbun.json` は改元前の旧保存から1手番だけ進む。誓約の選択・達成点・強い効果がまだ無いことを検査する。
- `friendship.json` は大内を人間として開始する。年齢と選択期限だけを試験用に設定し、既存の選択画面から関係の誓約を選ぶ。未成立・非同盟・別チームの組にゲーム本体の友好宣言を成立させ、自然な次の手番で両家の友好点が各1、誓約点が1増えたことを検査する。保存・読み込みと次の手番でも増分が変わらないことを要求する。年齢の試験設定は、通常進行の年齢判定やバランスの合格に含めない。

これらの生成・構文検査・模擬検査は実機合格を意味しない。軍事生産速度と全66誓約の実効値は別の効果比較で確認し、旗の再構成だけでは合格にしない。実行は共有の `live_chain.exe` を使い、結果が得られるまでToDo #134を完了にしない。

移行後の保存比較は、人間だけでなく全家の文明・年齢・誓約5項目を対象とする。読み込み後に比較用の保存値が無い場合は失敗とし、その場で比較値を作り直さない。友好宣言は成立直後、保存復元後、次の手番のいずれも両方向の成立と非同盟を要求する。

天文の旧保存では、読み込み時に誓約5項目が未設定であることを確認する。自然な次手番では選択を管理する4項目が未設定で、研究数の記録だけが現在の技術・社会制度の数と一致することを要求する。既存の研究取り込みは改元前にも研究数を記録するため、その正常な記録を誓約の開始や加点とは扱わない。

Windowsで複数の作業フォルダーが同じCargo出力先を使うと、実行ファイルに別の作業フォルダーの参照先が残る場合がある。検査と生成では `CARGO_TARGET_DIR` を作業ごとの専用フォルダーへ設定し、同じ出力先で直列に実行する。別の作業フォルダーを参照した検査結果は受け入れの証拠に使わない。

終手の実行器はゲームプレイ側の1手番進行を確認し、人間の操作番が戻るまで別に待つ。人間の操作番が戻った後にも番号を照合し、待機中にさらに進んだ結果を成功にしない。終手要求の再送や締切の延長は行わない。

診断の観測完了は終了コード3で返し、結果ファイルには `observation_completed=true`、`passed=false`、`acceptance_passed=false` を記録する。実行失敗は従来どおり別の非ゼロ終了コードで返す。`live_chain` の終了コードだけでも受け入れ成功と誤認しないようにし、診断の後に受け入れシナリオを実行する。
