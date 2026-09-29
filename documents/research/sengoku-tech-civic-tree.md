# 戦国パック専用 技術・社会制度ツリー設計案

> **位置づけ：調査・設計案。** SSOT の改定ではない。確定済みの範囲は [3.1・3.2](../spec/ssot/03-map-and-scale.md)、[16.2](../spec/ssot/16-roadmap.md)、[6.6](../spec/ssot/06-nation-logistics.md) 等に従う。以下の研究費、配置、解放先、政策名、時代区分は提案値であり、建物の効果量を確定しない。

## 1. 目的と進行感

- **勝利は登録済みの全城約50の支配**で判定する。研究・制度を完走しても勝利にしない。日本列島全体の固定マップで20大名家・琉球王国・アイヌが1534年の領土と城を所有して始まる。都市創設・他文明発見・新大陸発見を必須の前提や閃き条件にしない。
- 技術は「水利・鉱山・鍛冶」「街道・港・兵站」「築城・攻城・火器」を、社会制度は「家中・村・寺社」「商人・外交」「検地・動員・支配の正当化」を伸ばす。沿岸勢力、内陸勢力、寺社勢力が異なる順路で城の獲得・保持を狙える。前提は**同じツリー内のみ**とし、技術と制度の連携は閃きまたは追加効果にする。別ツリーの研究不足で主経路を閉ざさない。
- 序盤は既存の城と人物を生かして補給・防御・医療を整える。中盤は鉄砲、銀、港、会合、調略に投資し、複数戦線の選択が増える。終盤は拡大した領土の物流・忠誠・宗教を維持しながら、降伏や合併も使って残る城を支配する。兵士の**自然回復なし**、武将固有コンボ、約80武将と探索30地点はツリーの代替物にしない（[5章](../spec/ssot/05-growth-build-combo.md)、[4章](../spec/ssot/04-factions-characters.md)）。
- 通常速度の**調整目標は約200ターン（暫定180～230）**。目安は第I期25、第II期75、第III期45、第IV期55ターン。1534～1600年を均等に進めるなら平均約3ターン/年だが、史実の事件は年表示だけで自動的に発生・成功させない。早期統一も可能とし、1600年に未統一なら続行とする案。時代分けは研究のコスト帯と画面の節目であり、暦年の通過で技術を自動授与しない。区分年の根拠は[国立公文書館年表](https://www.archives.go.jp/exhibition/digital/rekishitomonogatari/history01.html)。
- 各列の費用は通常速度の**研究力／文化力の暫定基礎費用**。全ノード必修ではなく、統一までに各ツリーの6～7割程度を選び、残りは他の戦略として残ることを想定する。閃きは原則40%（ローカル Gathering Storm 定義に合わせる）を試算し、開始済みの城・施設だけで自動達成しない。既存建物を修理する権利は保ち、ツリーは**新設・改修**を解放する。必要な一部の既知技術は家ごとの初期状態として付与できるが、地理や民族によって必須の能力を欠かせない。
- 史実名称は各家の「発明時刻」を示さない。灰吹法は開始前から石見に伝わっていた技術、鉄砲は1543年の種子島伝来を代表例とするが伝播は複線的、検地・印判状は全国一斉ではない。これらを研究可能な**領国への導入・制度化**として扱う。[灰吹法に関する国立国会図書館のレファレンス](https://crd.ndl.go.jp/reference/entry/reference/show?id=1000296195&ldtl=1&mcmd=25&page=ref_view&st=update&type=reference)、[歴博の鉄炮展示](https://archive.rekihaku.ac.jp/exhibitions/project/old/061003/index.html)、[国立公文書館の戦国家法論](https://www.archives.go.jp/about/activity/international/pdf/151106murai_ja.pdf)を参照。

### 表の読み方

`列/行` は両ツリー共通の**目標画面位置**。列1～3＝第I期、4～6＝第II期、7～9＝第III期、10～12＝第IV期。技術の行は `-3..4`、社会制度は `-3..3`。`Tnn` / `Cnn` は表内参照用の短縮記号で、実装 ID は各行に全文を記す。前提はすべて **AND**。`—` は前提なし。「案」と書いたユニット・政策・政体・能力は新規提案で、6.6の建物名と混同しない。閃き条件は原則としてゲーム開始後の**新たな行動または増分**を数える。地形・交易相手がない場合も通常研究で取得できる。**制度の列番号はゲームDBのフィールドではない。** この表どおりの列を再現するには戦国パック専用の CivicsTree UI 配置処理が要る。

| 費用段階 | A1 | A2 | A3 | B1 | B2 | B3 | C1 | C2 | C3 | D1 | D2 | D3 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 対応列 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 |
| 技術費 | 40 | 65 | 90 | 125 | 160 | 205 | 255 | 320 | 400 | 490 | 600 | 730 |
| 制度費 | 30 | 45 | 65 | 90 | 125 | 165 | 215 | 275 | 350 | 430 | 530 | 650 |

## 2. 技術ツリー（32ノード）

### I 領国の基盤（1534～1542年ごろ）

| 番号・日本語 / English / 内部ID | 前提 | 列/行・段階 | 解放案 | ひらめき（Eureka） | 史実メモ・出典 |
|---|---|---|---|---|---|
| T01 用水普請 / Irrigation Works<br>`TECH_JAC_SENGOKU_IRRIGATION_WORKS` | — | 1/-3・A1 | 用水改修（改善案）。初期の水田は既設・利用可 | 開始後に農地を1区画改修 | 武田信玄の築堤伝承は治水を領国経営の題材にできる。[文化遺産オンライン](https://online.bunka.go.jp/heritages/detail/212010) |
| T02 沿岸航法 / Coastal Pilotage<br>`TECH_JAC_SENGOKU_COASTAL_PILOTAGE` | — | 1/-2・A1 | 沿岸輸送・水軍兵の基本行動（案）。既存の港・漁は使用可 | 自勢力の沿岸2地点間を船で移動 | 16世紀の東アジアでは海路が物・人・技術を運んだ。[歴博の海道展](https://archive.rekihaku.ac.jp/exhibitions/project/old/050323/index.html) |
| T03 砂鉄鍛冶 / Iron Forging<br>`TECH_JAC_SENGOKU_IRON_FORGING` | — | 1/1・A1 | **刀鍛冶座**、刀剣修理・槍兵の装備（案） | 鉄または砂鉄資源を改良 | 刀鍛冶には砂鉄由来の鉄と木炭が欠かせない。[文化庁の木炭製造解説](https://online.bunka.go.jp/heritages/detail/299399) |
| T04 堀切・土塁 / Earthwork Forts<br>`TECH_JAC_SENGOKU_EARTHWORK_FORTS` | — | 1/3・A1 | **本丸櫓**の新設、土塁改修（案）。既存城の利用は維持 | 自城の防御施設を1回修理 | 16世紀前半の深大寺城には郭・土塁などが残る。[文化遺産オンライン](https://online.bunka.go.jp/heritages/detail/140324) |
| T05 街道と荷駄 / Roads and Packtrains<br>`TECH_JAC_SENGOKU_ROADS_AND_PACKTRAINS` | T01 | 2/-1・A2 | 街道改修、荷駄輸送Unit（案） | 2城間に道路を接続 | 戦国城郭は街道を意識して構えられた。[柏木城跡](https://online.bunka.go.jp/db/heritages/detail/520604) |
| T06 鉱脈採掘 / Ore Prospecting<br>`TECH_JAC_SENGOKU_ORE_PROSPECTING` | T03 | 2/0・A2 | 鉱山改善の新設・坑道調査（案） | 金銀の鉱山を1か所調査 | 石見銀山には坑道・製錬所・鉱山集落が一体で残る。[UNESCO](https://whc.unesco.org/en/list/1246) |
| T07 兵糧保存 / Grain Stores<br>`TECH_JAC_SENGOKU_GRAIN_STORES` | T01 | 2/2・A2 | **兵糧蔵**、食料備蓄の操作（案） | 包囲下の城へ食料を1回搬入 | 兵糧入れは城の維持と戦役に直結した。[大高城の兵糧入れ資料](https://online.bunka.go.jp/db/heritages/detail/132287) |
| T08 山城縄張 / Hill Fort Layout<br>`TECH_JAC_SENGOKU_CASTLE_LAYOUT` | T04 | 3/3・A3 | 砦改善、城の地形防御改修（案） | 山地・丘陵の城を1回改修 | 山腹の郭・堀切を備えた城館が戦国の防御を支えた。[坂戸城跡](https://online.bunka.go.jp/heritages/detail/162541) |

### II 技術交流と領国競争（1543～1567年ごろ）

| 番号・日本語 / English / 内部ID | 前提 | 列/行・段階 | 解放案 | ひらめき（Eureka） | 史実メモ・出典 |
|---|---|---|---|---|---|
| T09 灰吹法 / Cupellation<br>`TECH_JAC_SENGOKU_CUPELLATION` | T06 | 4/0・B1 | 銀精錬改善、銀の回収効率強化（案） | 銀鉱山を稼働、または精錬知識を交易・吸収で得る | 石見で1533年ごろに導入されたとされる灰吹法は銀の回収を改善した。1534年に全勢力が「発明」する扱いにはしない。[国立国会図書館レファレンス](https://crd.ndl.go.jp/reference/entry/reference/show?id=1000296195&ldtl=1&mcmd=25&page=ref_view&st=update&type=reference) |
| T10 和船建造 / Wooden Shipbuilding<br>`TECH_JAC_SENGOKU_WOODEN_SHIPBUILDING` | T02 | 4/-2・B1 | 小型軍船・輸送船の新造（案） | 船を1隻建造 | 戦国期には大型軍船・安宅船へ発展する造船技術があった。[船大工資料](https://online.bunka.go.jp/heritages/detail/207190) |
| T11 武具製作 / Armor and Blades<br>`TECH_JAC_SENGOKU_ARMOR_AND_BLADES` | T03 | 4/2・B1 | **武具蔵**、侍・槍兵の装備補充（案） | 刀鍛冶座で装備を1件製作 | 鍛冶と武具生産は城下の職人仕事を形成した。[堺と今井の日本遺産](https://japan-heritage.bunka.go.jp/ja/stories/story044/) |
| T12 鉄砲伝来 / Firearms Contact<br>`TECH_JAC_SENGOKU_FIREARMS_CONTACT` | T03 | 4/1・B1 | 輸入火縄銃の調達・試射（案）。国内量産は未解放 | 火器を輸入・鹵獲、または火器を持つ勢力と接触 | 1543年の種子島は代表的な伝来例。西国一帯の複数経路も考慮する。[歴博の鉄炮展示](https://archive.rekihaku.ac.jp/exhibitions/project/old/061003/index.html) |
| T13 鉄砲鍛冶 / Matchlock Craft<br>`TECH_JAC_SENGOKU_MATCHLOCK_CRAFT` | T11, T12 | 5/1・B2 | **鉄砲鍛冶場**、火縄銃兵の国内生産・火器ジョブ候補 | 火器を所有したうえで刀鍛冶座を1か所稼働 | 国友などの鍛冶は伝来後の製作を担った。[歴博の鉄炮展示](https://archive.rekihaku.ac.jp/exhibitions/project/old/061003/index.html) |
| T14 金銀山経営 / Mine Management<br>`TECH_JAC_SENGOKU_MINE_MANAGEMENT` | T06 | 5/0・B2 | **金銀山奉行所**、鉱山産出・資材備蓄の改良（案） | 同一領国内で鉱山と輸送路を接続 | 鉱山と街道・港は一つの生産網をなした。[UNESCO](https://whc.unesco.org/en/list/1246) |
| T15 河川治水 / River Control<br>`TECH_JAC_SENGOKU_RIVER_CONTROL` | T01 | 5/-3・B2 | 堤・水路改善（案）、洪水後の復旧強化 | 水害を受けた農地を復旧、または河川沿いに用水を設ける | 信玄堤と御勅使川の築堤は治水・利水の事例。[文化遺産オンライン](https://online.bunka.go.jp/heritages/detail/212010) |
| T16 畝状竪堀 / Ridge Ditches<br>`TECH_JAC_SENGOKU_RIDGE_DITCHES` | T08 | 5/3・B2 | 山城防衛改修（案） | 攻囲を受けた城で防御工事を完了 | 戦国末期の城に畝状竪堀群が確認される。[文化遺産データベース](https://online.bunka.go.jp/db/heritages/detail/398088) |
| T17 水先と港湾 / Harbor Pilots<br>`TECH_JAC_SENGOKU_HARBOR_PILOTS` | T05, T10 | 6/-2・B3 | **港座**、海上交易路と陸路の接続（案） | 港を経由して2城に物資を輸送 | 瀬戸内海の過所船旗は水先・通航権の実態を伝える。[文化遺産オンライン](https://online.bunka.go.jp/heritages/detail/428306) |
| T18 湯治と薬療 / Baths and Medicine<br>`TECH_JAC_SENGOKU_BATH_AND_MEDICINE` | T07 | 6/2・B3 | **湯屋**、医療系Unitの補助（案）。基本医療ジョブは初期から可 | 負傷Unitを医療ジョブで治療、または療養施設を使用 | 16世紀末の湯治は記録に残り、鍼灸の秘伝書も戦国期から伝わる。[湯治の史料](https://online.bunka.go.jp/db/heritages/detail/389711)、[鍼灸秘伝書](https://online.bunka.go.jp/heritages/detail/540151) |

### III 天下布武と広域戦（1568～1581年ごろ）

| 番号・日本語 / English / 内部ID | 前提 | 列/行・段階 | 解放案 | ひらめき（Eureka） | 史実メモ・出典 |
|---|---|---|---|---|---|
| T19 兵站路網 / Supply Roads<br>`TECH_JAC_SENGOKU_SUPPLY_ROADS` | T05, T07 | 7/-1・C1 | 接続された城間の食料・資材再配分（案） | 3城を道路で連結 | 石見の銀鉱・街道・港は輸送路の管理が重要だった。[UNESCO](https://whc.unesco.org/en/list/1246) |
| T20 軍船建造 / Warship Construction<br>`TECH_JAC_SENGOKU_WARSHIP_CONSTRUCTION` | T17 | 7/-2・C1 | **船蔵**、水軍兵Unit・艦船補給（案） | 輸送船を港で修理 | 村上氏文書は瀬戸内での水軍編成を示す。[文化遺産オンライン](https://api.online.bunka.go.jp/heritages/detail/417392) |
| T21 石垣普請 / Stone Ramparts<br>`TECH_JAC_SENGOKU_STONE_RAMPARTS` | T08 | 7/3・C1 | 石垣改修（案）、本丸櫓の防御改良 | 城の土塁を石垣に改修 | 安土城では石垣・天守台が城郭の大きな転換を示す。[安土城跡](https://online.bunka.go.jp/heritages/detail/161498) |
| T22 鉄砲量産 / Matchlock Production<br>`TECH_JAC_SENGOKU_MATCHLOCK_PRODUCTION` | T13 | 7/1・C1 | 火縄銃兵の量産改良、火器補給能力（案） | 鉄砲鍛冶場で火器を2回生産 | 国友の鍛冶は注文増大に対応して同業者組合を組んだ。[歴博の鉄炮展示](https://archive.rekihaku.ac.jp/exhibitions/project/old/061003/index.html) |
| T23 玉薬生産 / Shot and Powder<br>`TECH_JAC_SENGOKU_SHOT_AND_POWDER` | T11, T22 | 8/1・C2 | 火器の補給・弾薬備蓄（案） | 火器と補給路を備えた城で1戦を終える | 鉄砲の実用には銃とともに弾丸・火薬・道具が必要だった。[歴博の鉄炮展示構成](https://archive.rekihaku.ac.jp/exhibitions/project/old/001003/kosei.html) |
| T24 港湾補給 / Port Supply<br>`TECH_JAC_SENGOKU_PORT_SUPPLY` | T19, T20 | 8/-2・C2 | 船蔵から沿岸城への補給（案） | 港から前線城へ資材を1回輸送 | 石見銀山の沖泊は銀の積出港と補給拠点だった。[文化遺産オンライン](https://online.bunka.go.jp/special_content/component/103) |
| T25 枡形虎口 / Angled Gateways<br>`TECH_JAC_SENGOKU_ANGLED_GATEWAYS` | T21 | 8/3・C2 | 城門防御改修（案） | 石垣のある城を1回守り抜く | 1588年築城の赤木城には枡形虎口がある。[文化遺産データベース](https://online.bunka.go.jp/db/heritages/detail/138594) |
| T26 陣城普請 / Field Forts<br>`TECH_JAC_SENGOKU_FIELD_FORTS` | T08, T19 | 9/3・C3 | 包囲用陣城・土塁（改善案） | 敵城を包囲し、補給線を保つ | 1581年の鳥取城攻めでは大規模な包囲陣地が築かれた。[日本遺産・太閤ヶ平](https://japan-heritage.bunka.go.jp/ja/lp/castles/) |

### IV 統一事業（1582～1600年ごろ）

| 番号・日本語 / English / 内部ID | 前提 | 列/行・段階 | 解放案 | ひらめき（Eureka） | 史実メモ・出典 |
|---|---|---|---|---|---|
| T27 安宅船 / Atakebune<br>`TECH_JAC_SENGOKU_ATAKEBUNE` | T24 | 10/-2・D1 | 大型軍船Unit（案） | 船蔵で軍船を2隻建造 | 安宅船は戦国期に発達した大型軍船。[船大工資料](https://online.bunka.go.jp/heritages/detail/207190) |
| T28 城攻め大筒 / Siege Artillery<br>`TECH_JAC_SENGOKU_SIEGE_ARTILLERY` | T23 | 10/1・D1 | 大筒Unit（案）。希少・高補給費として設計 | 火縄銃兵と攻囲用補給を同時に運用 | 戦国期の大鉄砲・大筒・石火矢についての歴博研究がある。[歴博年報](https://archive.rekihaku.ac.jp/research/nenpo/2005/pdf/005.pdf) |
| T29 本丸石垣 / Keep and Stone Base<br>`TECH_JAC_SENGOKU_KEEP_AND_STONE_BASE` | T25 | 10/3・D1 | **本丸櫓**の上位改修（案） | 本丸櫓と石垣を同じ城に整える | 安土城の本丸・石垣・天守台は近世城郭への転換点。[安土城跡](https://online.bunka.go.jp/heritages/detail/161498) |
| T30 広域兵站 / Regional Supply<br>`TECH_JAC_SENGOKU_REGIONAL_SUPPLY` | T19, T24 | 11/-1・D2 | 複数方面軍と後方備蓄の調整（案） | 異なる2戦線へ補給を完了 | 銀山・城・街道・港の連動は広域補給の史実上の手がかり。[UNESCO](https://whc.unesco.org/en/list/1246) |
| T31 検地尺の標準化 / Survey Standards<br>`TECH_JAC_SENGOKU_SURVEY_STANDARDS` | T19 | 11/0・D2 | 測量補助Unit／検地の事務効率（案）。制度側の検地を代替しない | 2領地で測量を行う | 1594年の島津氏分国検地尺は測量単位の標準化を示す。[文化遺産オンライン](https://online.bunka.go.jp/index.php/heritages/detail/202303) |
| T32 惣構 / Outer Defenses<br>`TECH_JAC_SENGOKU_OUTER_DEFENSES` | T29, T30 | 12/3・D3 | 城下町を含む防御線の改修（案） | 城と城下町の双方に防御施設を整える | 戦国の城下は堀・土塁で町と家臣団屋敷を囲む例がある。[勝尾城跡](https://online.bunka.go.jp/db/heritages/detail/140467) |

## 3. 社会制度ツリー（30ノード）

### I 領国の基盤（1534～1542年ごろ）

| 番号・日本語 / English / 内部ID | 前提 | 列/行・段階 | 解放案 | 着想（Inspiration） | 史実メモ・出典 |
|---|---|---|---|---|---|
| C01 家中掟 / House Rules<br>`CIVIC_JAC_SENGOKU_HOUSE_RULES` | — | 1/0・A1 | 政策案《家臣評定》。政体案《家中合議》は全勢力に開始時付与 | 武将を城主・奉行に1人任命 | 戦国大名は家臣団と向き合い、家法を整えた。[国立公文書館の中世史講演](https://www.archives.go.jp/about/activity/international/pdf/151106murai_ja.pdf) |
| C02 国人盟約 / Kokujin Leagues<br>`CIVIC_JAC_SENGOKU_KOKUJIN_LEAGUES` | — | 1/-2・A1 | 政策案《国人との協約》、小勢力との交渉能力 | 他家との協定を1件結ぶ | 国人たちは一揆・連合を結んだ。臣従や合併と同一視しない。[歴博第2展示室](https://www.rekihaku.ac.jp/exhibitions/room2/) |
| C03 寺社と信徒 / Temples and Believers<br>`CIVIC_JAC_SENGOKU_TEMPLES_AND_BELIEVERS` | — | 1/2・A1 | **寺社伽藍**の新設、宗教資産保護（案） | 既存の寺社・祭祀地点を1回保護・修理 | 神仏習合の形跡が残る寺社がある。宗教の創始や存続をこの制度に従属させない。[油日神社懸仏群](https://online.bunka.go.jp/heritages/detail/276706) |
| C04 関所と通行 / Passes and Passage<br>`CIVIC_JAC_SENGOKU_PASSES_AND_PASSAGE` | C01 | 2/-1・A2 | **関所**、政策案《通行監督》 | 街道上の要衝を1か所掌握 | 北条氏の関所手形は戦国の通行統制を示す。[文化遺産オンライン](https://online.bunka.go.jp/heritages/detail/437479) |
| C05 分国法 / Domain Law<br>`CIVIC_JAC_SENGOKU_DOMAIN_LAW` | C01 | 2/0・A2 | 政体案《領国大名制》、政策案《領国裁判》 | 2城を保持し、裁判・紛争解決を1回行う | 今川・武田・伊達・六角などの戦国家法が知られる。[国立公文書館](https://www.archives.go.jp/about/activity/international/pdf/151106murai_ja.pdf) |
| C06 惣村自治 / Village Assemblies<br>`CIVIC_JAC_SENGOKU_VILLAGE_ASSEMBLIES` | C02 | 2/2・A2 | 政策案《惣村協調》、占領地の住民協議能力 | 城下以外の村落と協定を1件結ぶ | 村々は自ら掟を定め、寄り合いを行った。[歴博第2展示室](https://www.rekihaku.ac.jp/exhibitions/room2/) |
| C07 学問と記録 / Learning and Records<br>`CIVIC_JAC_SENGOKU_LEARNING_AND_RECORDS` | C05 | 3/1・A3 | **学問所**、文書・研究資料の保管能力（案） | 文書・教典・地図などをVaultへ1件収蔵 | 足利学校は室町期に重要な文教拠点だった。[足利学校跡](https://online.bunka.go.jp/heritages/detail/209700) |

### II 技術交流と領国競争（1543～1567年ごろ）

| 番号・日本語 / English / 内部ID | 前提 | 列/行・段階 | 解放案 | 着想（Inspiration） | 史実メモ・出典 |
|---|---|---|---|---|---|
| C08 寺内町自治 / Temple Town Autonomy<br>`CIVIC_JAC_SENGOKU_TEMPLE_TOWN_AUTONOMY` | C03, C06 | 4/2・B1 | **寺内町**、信徒・住民との交渉能力 | 信徒のいる都市で寺社と町を接続 | 『天文日記』は本願寺教団・加賀一向一揆・大阪寺内町を記録する。[歴博DB](https://www.rekihaku.ac.jp/doc/gaiyou/tenbun.html) |
| C09 修験の講 / Mountain Pilgrimage<br>`CIVIC_JAC_SENGOKU_MOUNTAIN_PILGRIMAGE` | C03 | 4/3・B1 | **山伏宿坊**、山岳探索支援（案） | 山地の探索地点を1か所調査 | 大峯奥駈道には修験者の道・宿があった。[大峯奥駈道](https://online.bunka.go.jp/heritages/detail/140154) |
| C10 軍役帳 / Military Service Ledger<br>`CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER` | C05 | 4/-1・B1 | 政策案《軍役割当》、槍兵・騎兵の動員調整 | 家臣2人へ異なる軍役を割り当てる | 北条氏所領役帳は知行高と家臣の役負担を列記する。[平塚市](https://www.city.hiratsuka.kanagawa.jp/bunka/page-c_01292.html) |
| C11 起請文と同盟 / Oaths and Alliances<br>`CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES` | C02 | 4/0・B1 | 政策案《起請文》、同盟・停戦交渉 | 複数ターン続く同盟を1件維持 | 浅井氏の1568年起請文には同盟、所領承認、人質などが記される。[国立公文書館](https://www.archives.go.jp/exhibition/digital/rekishitomonogatari/contents/32.html) |
| C12 郡中惣と密偵 / Local Leagues and Intelligence<br>`CIVIC_JAC_SENGOKU_LOCAL_LEAGUES_AND_INTELLIGENCE` | C02, C11 | 5/-2・B2 | **忍びの里・暗殺者訓練所**、偵察・諜報ジョブの補助（案）。暗殺者Unit自体はここで閉じない | 偵察Unitで他家の城を1か所確認 | 甲賀郡中惣は地侍の水平的連合。密偵能力はゲーム上の創作的接続であり、史料が組織的な「忍者の里」を証明するわけではない。[甲賀郡中惣遺跡群](https://online.bunka.go.jp/db/heritages/detail/192812) |
| C13 港町会合 / Port Merchant Councils<br>`CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS` | C04 | 5/1・B2 | **会合所**、政体案《港市評議制》 | 港市で交易契約を1件結ぶ | 堺は会合衆が自治的に運営した対外貿易都市だった。[歴博・堺展示](https://archive.rekihaku.ac.jp/exhibitions/special/old/room3_2012/index.html) |
| C14 茶会と名物 / Tea Gatherings and Treasures<br>`CIVIC_JAC_SENGOKU_TEA_GATHERINGS` | C13 | 6/2・B3 | **茶室**、文化・外交の会見行動（案） | 茶器または名物を1件入手・収蔵 | 天文年間から唐物茶壺の飾りが行われた。[文化遺産オンライン](https://online.bunka.go.jp/heritages/detail/169505) |
| C15 城下町の集住 / Castle Town Ordinances<br>`CIVIC_JAC_SENGOKU_CASTLE_TOWN_ORDINANCES` | C04, C05 | 6/0・B3 | **城下町**、政策案《町割と移住》 | 城に隣接する町を1区画整備 | 安土の山下町中掟書は城下町の建設・取締りに関する都市法。[文化遺産オンライン](https://online.bunka.go.jp/heritages/detail/197586) |
| C16 朝廷・将軍への取次 / Court and Shogunate Envoys<br>`CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS` | C11 | 6/-1・B3 | 使節・権威獲得の交渉行動（案） | 他家または足利将軍家への使節を1回派遣 | 信長は1568年に足利義昭を奉じて上洛した。[国立公文書館年表](https://www.archives.go.jp/exhibition/digital/rekishitomonogatari/history01.html) |

### III 天下布武と広域戦（1568～1581年ごろ）

| 番号・日本語 / English / 内部ID | 前提 | 列/行・段階 | 解放案 | 着想（Inspiration） | 史実メモ・出典 |
|---|---|---|---|---|---|
| C17 楽市楽座 / Free Markets<br>`CIVIC_JAC_SENGOKU_FREE_MARKETS` | C13, C15 | 7/1・C1 | **楽市**、政策案《市の特権》 | 城下町で国内交易路を開く | 楽市令は城下町の繁栄に向けた特権付与でもあった。[歴博FAQ](https://www.rekihaku.ac.jp/contact/faq/) |
| C18 寺社領安堵 / Shrine Land Guarantees<br>`CIVIC_JAC_SENGOKU_SHRINE_LAND_GUARANTEES` | C08, C16 | 7/2・C1 | 政策案《寺社領保護》、宗教対立時の調停（案） | 他宗派がある城で寺社資産を1件保護 | 1561年の今川氏真証文には寺領安堵が記される。[文化遺産オンライン](https://online.bunka.go.jp/heritages/detail/400265) |
| C19 南蛮使節 / Nanban Envoys<br>`CIVIC_JAC_SENGOKU_NANBAN_ENVOYS` | C13 | 7/-1・C1 | 南蛮貿易契約・外来知識の交渉（案） | 外国商人またはその代理勢力と交易 | 大友宗麟は南蛮貿易に積極的だった。[大友氏館跡](https://online.bunka.go.jp/db/heritages/detail/207143) |
| C20 領国検地 / Domain Land Surveys<br>`CIVIC_JAC_SENGOKU_DOMAIN_LAND_SURVEYS` | C10, C15 | 8/0・C2 | 政策案《検地と年貢》、土地台帳の作成 | 新たに支配した城の耕地を調査 | 後北条氏は検地と貫高に基づく課税を実施した。[小田原デジタルミュージアム](https://odawara-digital-museum.jp/selection/hojo/24/) |
| C21 鉄砲衆編制 / Gun Corps Organization<br>`CIVIC_JAC_SENGOKU_GUN_CORPS_ORGANIZATION` | C10 | 8/-2・C2 | 政策案《鉄砲衆》、火縄銃兵の指揮・補給（案） | 火縄銃兵2Unitを同一戦線で運用 | 鉄砲が主兵器化するのは大名が鉄砲衆を創設した後。[歴博の鉄炮展示](https://archive.rekihaku.ac.jp/exhibitions/project/old/061003/index.html) |
| C22 茶の湯外交 / Tea Diplomacy<br>`CIVIC_JAC_SENGOKU_TEA_DIPLOMACY` | C14, C16 | 8/2・C2 | 政策案《茶会外交》、使節の交渉補助（案） | 茶室で他家との会見を1回行う | 茶道具は大名間の権威と贈答にも用いられた。[唐物茶壺](https://online.bunka.go.jp/heritages/detail/169505) |
| C23 連合軍役 / Coalition Campaigns<br>`CIVIC_JAC_SENGOKU_COALITION_CAMPAIGNS` | C11, C21 | 9/-1・C3 | 政策案《連合出陣》、同盟軍の補給調整（案） | 同盟相手と共通の敵に交戦 | 織田・徳川の連合軍は長篠で共同行動した。[国立公文書館](https://www.archives.go.jp/exhibition/digital/ieyasu/contents1_06/) |

### IV 統一事業（1582～1600年ごろ）

| 番号・日本語 / English / 内部ID | 前提 | 列/行・段階 | 解放案 | 着想（Inspiration） | 史実メモ・出典 |
|---|---|---|---|---|---|
| C24 停戦裁定 / Truce Arbitration<br>`CIVIC_JAC_SENGOKU_TRUCE_ARBITRATION` | C16, C20 | 10/-1・D1 | 政策案《境目裁定》、紛争仲裁・合併交渉の補助（案） | 他家2家の停戦に関与 | 「惣無事令」を単一・全国一律の法令とする見方には研究上の異論があるため、名称・効果を個別の裁定に留める。[国会図書館の論文要旨](https://ndlsearch.ndl.go.jp/books/R000000004-I10694537) |
| C25 太閤検地 / Taikō Land Survey<br>`CIVIC_JAC_SENGOKU_TAIKO_LAND_SURVEY` | C20 | 10/0・D1 | 政策案《広域検地》、征服地の税・兵糧把握（案） | 3城の土地台帳を整備 | 長宗我部地検帳は1587～1598年の土佐一国の検地原簿。[文化遺産オンライン](https://online.bunka.go.jp/db/heritages/detail/186802) |
| C26 朱印状 / Red Seal Writs<br>`CIVIC_JAC_SENGOKU_RED_SEAL_WRITS` | C16, C20 | 10/1・D1 | 政策案《朱印による認可》、所領・交易契約の文書化（案） | 臣従・領地配分の文書を1件交付 | 1587年の豊臣秀吉朱印状は家臣への命令に用いられた。17世紀の朱印船貿易を先取りしない。[鍋島直茂宛朱印状](https://online.bunka.go.jp/db/heritages/detail/293640) |
| C27 刀狩 / Sword Hunt<br>`CIVIC_JAC_SENGOKU_SWORD_HUNT` | C25 | 11/-2・D2 | 政策案《武具接収》、治安と抵抗の両面を持つ選択肢（案） | 占領城の武具を1件接収または保護して処理 | 1588年の刀狩令写は百姓の武器所持停止を命じる。政策の善悪を単純化しない。[文化遺産オンライン](https://online.bunka.go.jp/heritages/detail/622235) |
| C28 兵農分離 / Warrior–Farmer Separation<br>`CIVIC_JAC_SENGOKU_WARRIOR_FARMER_SEPARATION` | C10, C25 | 11/0・D2 | 政体案《広域大名制》、政策案《常備軍役》。従来の村落動員と選択関係 | 常備軍を維持しながら農地の労働力を確保 | 鉄砲・軍団編成と兵農分離は、統一政権への変化とともに進んだ。[歴博第2展示室](https://www.rekihaku.ac.jp/exhibitions/room2/) |
| C29 港市の統制 / Port Administration<br>`CIVIC_JAC_SENGOKU_PORT_ADMINISTRATION` | C19, C26 | 11/2・D2 | 政策案《港市代官》、港・交易契約の監督（案） | 2港の交易を管理 | 1588年の秀吉朱印状は長崎代官任命を示す。[神戸市立博物館所蔵史料](https://online.bunka.go.jp/heritages/detail/450940) |
| C30 所領安堵 / Land Tenure Confirmation<br>`CIVIC_JAC_SENGOKU_LAND_TENURE_CONFIRMATION` | C24, C26 | 12/1・D3 | 降伏・合併後の所領確認能力（案）。人物・宗教・資産の継承判定は[9.1](../spec/ssot/09-assimilation-religion.md)どおり別処理 | 吸収した勢力の領地・人物・財産の記録を1件整備 | 1590年の豊臣秀吉朱印安堵状は南部氏の所領公認を示す。[歴博データベース](https://khirin.rekihaku.ac.jp/pid/nmjh_rekimin_h/10347100.html) |

### 6.6 建物20種の解放照合

この表は**新規建設の解放先**。1534年に既存の施設を持つ勢力はそのまま保有・使用できる。施設の建設条件、区域スロット、効果量、見た目は未設計。暗殺者訓練所は[9.5](../spec/ssot/09-assimilation-religion.md)の生産期間短縮であり、暗殺者を初めて利用可能にする建物とはしない。

| SSOT 6.6 の建物 | 新規建設の解放ノード | SSOT 6.6 の建物 | 新規建設の解放ノード |
|---|---|---|---|
| 本丸櫓 | T04（T29で改修） | 城下町 | C15 |
| 関所 | C04 | 楽市 | C17 |
| 会合所 | C13 | 港座 | T17 |
| 船蔵 | T20 | 兵糧蔵 | T07 |
| 鉄砲鍛冶場 | T13 | 刀鍛冶座 | T03 |
| 武具蔵 | T11 | 茶室 | C14 |
| 寺社伽藍 | C03 | 寺内町 | C08 |
| 山伏宿坊 | C09 | 学問所 | C07 |
| 忍びの里 | C12 | 暗殺者訓練所 | C12 |
| 金銀山奉行所 | T14 | 湯屋 | T18 |

### ツリーと既存システムの境界

- 9.1 の吸収では、研究を済ませた側が城を得るだけでなく、人物、武具、Artifact、宗教、文書から一部の技術・制度を継承できる。**ブースト・研究完了・単なる所持品移管**のどれにするかはデータ型ごとに決める。[9.4](../spec/ssot/09-assimilation-religion.md)の小国懐柔や大国合併はツリーの完走を条件にせず、制度は使節・交渉を補助する案とする。吸収だけで登録城の勝利判定が遅延しないよう、所有権の移管後に判定する。
- [9.2](../spec/ssot/09-assimilation-religion.md)の宗教20種（実在11＋独自9）の信徒数、地下化、滅亡、再興は社会制度の研究可否とは別。C03/C08/C18は寺社との関係を強めるだけ。カトリックは1549年以降のイベントで登場し、C19を全勢力が取得するまで待たない。神祇・仏教、琉球祭祀、アイヌのカムイ信仰を単一の「宗教技術段階」に押し込まない。
- [9.6](../spec/ssot/09-assimilation-religion.md)の結社8つは各ゲームで全て登場し、既存モードの発見・加入・専用Governor・4段階昇進が中核。C12/C13/C09/C21は関連活動の選択肢やボーナス候補に過ぎず、加入を研究で自動付与しない。未決の4結社には名称・効果を割り当てない。
- 政策と政体は**選択肢**。C27の刀狩やC28の兵農分離を自動施行しない。初期に槍兵・弓兵・医療系・偵察系の基本ジョブを持てるようにし、ツリーは増設・専門化を担う。C21の鉄砲衆は汎用の軍制であり、[5.3](../spec/ssot/05-growth-build-combo.md)で禁じた「武将に依存しない汎用コンボ」にはしない。
- [12章](../spec/ssot/12-world-laws.md)の世界法律は世界会議における永続の制定・改廃であり、ここに挙げた大名家の政策カードと同一視しない。

## 4. パック内の時代と Civ6 への接続

| パック内表示名・目標年 | 技術・制度の列 | 通常速度の目安 | MVPでの技術上の時代ID |
|---|---|---:|---|
| I 領国の基盤・1534～1542年 | 1～3 | 約25ターン | `ERA_ANCIENT` を戦国パック内で表示名変更 |
| II 技術交流と領国競争・1543～1567年 | 4～6 | 約75ターン | `ERA_CLASSICAL` を表示名変更 |
| III 天下布武と広域戦・1568～1581年 | 7～9 | 約45ターン | `ERA_MEDIEVAL` を表示名変更 |
| IV 統一事業・1582～1600年 | 10～12 | 約55ターン | `ERA_RENAISSANCE` を表示名変更 |

**実装の第一候補は既存4 Era ID の互換スロット利用。** 各パックの技術・制度ノードはすべて `TECH_JAC_SENGOKU_*` / `CIVIC_JAC_SENGOKU_*` で独立させ、公式ノードは表示・研究から外す。`ERA_ANCIENT` 等はエンジン・UIが参照する器としてだけ使う。独自の `ERA_JAC_SENGOKU_*` を追加する方式は、ローカルの `TechTree.lua` が `GameInfo.Eras()` と内部 `Index` に沿って時代を並べ、GSの `Eras_XP2` にも時代別項目があるため、少なくとも**ロード・進行・保存・AI・UIの実機検証後**に採否を決める。既存IDの利用は「公式ツリーを流用する」ことではない。

`StartEras` の年、固定マップの初期ユニット、`GameSpeed_Turns` 等を戦国シナリオ側で整え、1534年表示と約200ターンの進行を作る。標準の `ERA_ANCIENT` 開始用開拓者・戦士をそのまま22勢力へ追加しない。全城を初期配置済みにし、開拓者の自動生成・宗教枠・ルート接続の前提を実機で確認する。各時代の年数・列数は**バランス目標**で、世界時代の遷移タイミングや研究順が表の年と完全一致する保証はない。特に相手との交易・吸収で特定の技術を先取りできる。空の `ERA_INDUSTRIAL` 以降へ世界時代だけが進んだ場合も、宗教の旧式化・偉人費用・戦争ペナルティ・乗船戦闘力などが勝手に変わらないことを確認する。

**黄金時代・暗黒時代はMVPでは効果を中立化する案。** 22勢力の初期城配置と都市忠誠を、偶発的な時代スコア・暗黒時代の忠誠圧力で崩さないためである。時代スコア、時代の誓約、黄金／暗黒／英雄時代の能力を「画面だけ隠す」のではなく、ゲームプレイにも作用しないことを合格条件にする。都市の忠誠・宗教・吸収に伴う摩擦そのものは9章の仕組みとして残す。Rise and Fall / GS の時代評価と忠誠は別レイヤーである。[Civ6公式の時代紹介](https://civilization.2k.com/en-GB/civ-vi/)。無効化できない場合は、4区分に合わせた時代分数・誓約・忠誠効果の置き換え案を別途作り、実機で確認してからオーナー判断を仰ぐ。

### ローカル Civ6 定義から確認した実装上の制約

以下はこのPCの `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/` 以下で確認した**ローカル定義**で、MOD実装成功の検証ではない。

| 確認ファイル | 設計への影響 |
|---|---|
| `Base/Assets/Gameplay/Data/Technologies.xml`、`Civics.xml` | `Technologies` / `Civics` に `EraType`、`Cost`、`UITreeRow` があり、`TechnologyPrereqs` / `CivicPrereqs` は別表。閃きは `Boosts` に定義。名前、ID、前提、費用、閃きはDBに落とせる。 |
| `Base/Assets/Gameplay/Data/Eras.xml`、`DLC/Expansion2/Data/Expansion2_Eras.xml` | `Eras` に `ChronologyIndex`・ツリー背景・`TechTreeLayoutMethod`、`StartEras` に開始年・開始Unit。GSには `Eras_XP2` と時代スコア関係の値もある。Era ID の新設・削除はこれらと整合が要る。 |
| `DLC/Expansion2/Data/Expansion2_Technologies.xml`、`Expansion2_Civics.xml` | GSの未来技術・制度、新しい前提・閃き、外交勝利点を加える修正がある。ベース定義だけを消しても公式の参照が残る。 |
| `Base/Assets/UI/Screens/TechTree.lua`、`CivicsTree.lua` と `DLC/Expansion2/UI/Replacements/` | 技術は `TechTreeLayoutMethod="Cost"` なら**同じ Era 内の費用の異なる値ごとに列を作る**。`UITreeRow` は -3～4。社会制度の標準UIは同時代の前提から列を再計算し、行は -3～3で、衝突や大きなノードによって表示列・行がずれる。たとえばC12とC13は同時代内の前提がないため、標準UIでは表の列5へ自動配置されない。表の制度列を守る場合はパック限定のUI配置処理を差し替え、GSの置換UIとの競合を検証する。DBに独立した `Column` を書けるという想定は置かない。 |
| `Base/Assets/Gameplay/Data/Units.xml`、`Buildings.xml`、`Improvements.xml`、`Governments.xml`、`Policies.xml` と GS の対応ファイル | 解放参照は `PrereqTech` / `PrereqCivic` 等に分散。Unitの旧式化・区域・改善・プロジェクトの参照も調べ、公式研究IDが残る場合は意図的に遮断または付け替える。 |

閃き条件のうち「新たに1城修理」「1件吸収」「包囲を守る」のようなシナリオ固有条件は、既存 `BoostClass` だけで表現できるとは限らない。標準の建設・Unit保有などに置換できるものはDB定義を使い、残りは**決定的なゲームプレイ側イベント**で達成を記録する案とする。スクリプトから技術・制度のブーストを安全に付与できるかはPoCで検証し、無理なら通常研究で到達できるままにする。初期保有城、スタート時の建物・資源を即時カウントしないこと、セーブ・ロードとマルチプレイで閃きの二重発火がないことも確認する。

## 5. パック有効時の公式コンテンツ整理とリスク

**有効範囲は戦国シナリオ／戦国パックだけ。** DBからの削除・上書きは他の時代パックと共用するグローバルMODにしない。UI非表示だけで公式研究が残るとAIや報酬から選べるため、ゲームプレイ側の到達可能性と表示側の両方を監査する。

| 分類 | 戦国パック内での扱い | 主な確認点 |
|---|---|---|
| 公式技術・社会制度（Base、拡張、GS） | 研究候補・ツリー画面から全件除外し、技術32・制度30の独自IDだけを候補にする。旧 `TechnologyPrereqs` / `CivicPrereqs` / `Boosts` も残存参照を解消 | 科学・文化の研究候補が全勢力に常時1件以上あるか、未来技術・未来制度が再出現しないか、AIが研究を止めないか |
| 公式Unit・建物・区域・改善・プロジェクト | 侍・槍兵・弓兵・騎兵・火縄銃兵・水軍兵、医療・偵察系の戦国Unitと6.6の20建物、必要な改善・区域へ解放を付け替える。開拓者、近現代兵器、工場、宇宙港等の不適切な生産候補は除外。既存の地形・資源・都市・交易のエンジン要素は必要な分を維持 | `PrereqTech` / `PrereqCivic`、旧式化条件、建設の区域条件、初期Unitと購入経路、蛮族・自由都市の自動生成Unitを含めて調べる |
| 公式政体・政策カード | 戦国の政体・政策へ差し替え、公式の古代・現代・イデオロギー政策は候補から外す。政府スロットに空欄が出ないよう初期政体と初期政策を必ず与える | AIの政体変更とスロット充填、政策の旧式化、結社Governorとの相互作用 |
| 公式世界遺産・固有能力・Great Person・研究報酬 | 公式ツリーが必要なものを原則到達不能にする。史実の城・寺社・名物はシナリオ配置・探索報酬として独立管理し、後の世界編に必要なデータは削除しない | 「技術未取得でも建設可」の例外、他DLCやゲームモードの無償付与、偉人の時代参照 |
| 標準勝利条件 | 制覇・科学・文化・宗教・スコア・外交勝利が、約50城の勝利より先に発火しないよう戦国シナリオの勝利設定で除外。**琉球王国・アイヌの登録城も必須**。所有者の変更直後とロード後に判定 | 首都だけを取る標準制覇勝利、宗教20種の上限や布教勝利、GS外交勝利点、ターン上限のスコア勝利 |
| GSの時代・気候・世界会議 | 黄金／暗黒時代を中立化する案。災害・海面上昇はPoC 16.1の検証対象なので勝手に消さず、工業化・電力・炭素回収・未来時代に依存するUI／プロジェクトだけを戦国では遮断。公式世界会議と外交勝利はMVPから外す案。[12章](../spec/ssot/12-world-laws.md)の世界法律は別設計として保留 | `Eras_XP2`、時代スコアの閾値、気候段階の進行、世界会議の開始時代・議題、外交勝利点を与える技術・制度、海面上昇のテスト継続。[2KのGS概要](https://newsroom.2k.com/games/sid-meiers-civilization-vi-gathering-storm) |
| 秘密結社・宗教 | 9.2 / 9.6 に従い新規宗教20種・結社8種を別実装。公式4結社や公式宗教への偶発的な加入・創始・無償付与を遮断 | 専用Governorと4昇進、宗教枠・信徒存続・地下化、1549年カトリック登場、Ainu・琉球の固有信仰を大名家の制度で消さない |

DB作業に入る前に、**公式IDを参照する全行の棚卸し → 戦国用IDへ再配線または明示的な除外 → UI候補・AI候補・報酬の動作確認**の順で検証する。`Delete` だけで全参照が安全に消える、あるいはツリーを隠せば公式内容が到達不能になる、とは仮定しない。ツリーの研究が空になったときの自動研究、Great Personの時代、GSの世界会議・気候、自由都市・蛮族の生産リストも実機の監査対象。既存のユーザーセーブへの後付けは対象外とし、新規シナリオ開始で整合をとる案。

## 6. オーナーに残す判断事項

1. **ゲーム時間**：通常速度で約200ターン、統一しなければ1600年以降も継続、という調整目標でよいか。時間切れ・スコア勝利を置くか。
2. **時代システム**：MVPで黄金・暗黒時代の効果を中立化する案と、戦国向けに再設計して残す案のどちらにするか。新 Era ID の実機PoCが成功した場合でも、既存4 ID の互換スロットを維持するか。
3. **初期知識**：22勢力の既知技術・制度・既存建物を史実に合わせて個別設定する範囲。特に石見銀山・火器伝播と、琉球・アイヌに対する本土用建物名の表示・代替建物をどう扱うか。
4. **史実年と別史**：鉄砲伝来・カトリック登場・太閤検地等を暦年イベントで導入する強さ。研究を先取りした場合の表示、太閤検地という固有名を豊臣以外にも使うか。
5. **外交と勝利**：停戦裁定・所領安堵の効果範囲、琉球・アイヌの城の登録名と支配判定、宗教・結社・使節の既存Civ6枠との整合。世界法律／世界会議を日本MVPでも行うか、後の時代パックにするか。
6. **効果量とPoC**：研究費、閃き40%、政策・建物・Unitの具体的な数値は未決。1勢力とAIの試作で、200ターン前後の進行、22勢力の研究停止、UI列のずれ、保存後の閃き再発火を測ってから確定する。
