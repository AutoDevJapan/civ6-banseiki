# 1534年日本マップ：資源・地形要素の調査案

調査基準日：2026-09-29

> オーナー確認用ドラフト。SSOT 3.1「資源・遺跡・宝を配置する」の追記ではない。指定に従い、SSOT、コード、`documents/README.md` その他のファイルには手を加えない。

## 前提と読み方

- 対象期間は1534〜1600年。地図の基準年1534年に存在・知られていた資源と、期間中に発見・開発された地点を分ける。後者は `start_year` を持たせ、開始時に配置するかはゲーム仕様の判断事項とする。
- 地図は154×67、陸地約1,089タイル、約20km/六角形という提示値を採用した。資源案は陸上51地点（陸地の約4.7%、平均約21陸地タイルに1地点）。同一六角形に吸着する候補をまとめると実配置数は減る。水域資源は別枠で魚16、鯨1候補。
- 経緯度はWGS84の代表点を小数第3位で記す。これはタイル照合のための書式で、地図上の精度は概ね1タイル幅（約20km）と考える。鉱山・港など史跡の座標そのものを測量した値ではない。歴史上の令制国名は当時の地域区分に合わせた。
- 根拠強度：**高**＝自治体・国指定文化財・公的研究機関等が時期と場所を明記、**中**＝地域史・産業史の根拠はあるが期間内の操業範囲に幅がある、**低**＝自然条件または広域産業からの配置推定。低確度は史実上の個別鉱床・生産量を断定しない。
- 現在の河川・湖沼・湿地・植生図は形状の参照に使えるが、1534年当時の地形そのものではない。河道変更・干拓・埋立・水田開発を史実年代に合わせて補正する。

## 資源案

### Civ6既定資源との対応

| 史実資源 | Civ6 ID | 既定/新規 | 配置上の意味 |
|---|---|---|---|
| 金 | `RESOURCE_GOLD`（Gifts of the Nileシナリオ用） | シナリオ限定。通常マップは `RESOURCE_JAC_GOLD` 新規追加案 | 鉱山・砂金地。シナリオ用Gold Ore定義の通常マップ再利用は要確認 |
| 銀 | `RESOURCE_SILVER` | 既定 | 銀山。贅沢資源 |
| 銅 | `RESOURCE_COPPER` | 既定（GS） | ボーナス資源 |
| 鉄 | `RESOURCE_IRON` | 既定 | 砂鉄を含むたたら鉄生産圏を代表。戦略資源 |
| 馬 | `RESOURCE_HORSES` | 既定 | 牧・馬産圏。戦略資源 |
| 米 | `RESOURCE_RICE` | 既定 | 水田稲作の適地・地域余剰の代表。ボーナス資源 |
| 塩 | `RESOURCE_SALT` | 既定 | 塩浜・製塩沿岸。贅沢資源 |
| 茶 | `RESOURCE_TEA` | 既定 | 茶産地・茶業圏。贅沢資源 |
| 絹 | `RESOURCE_SILK` | 既定 | 絹織物・流通圏を代表。贅沢資源。生糸産地とは区別する |
| 魚 | `RESOURCE_FISH` | 既定 | 沿岸の漁場。ボーナス資源 |
| 鯨 | `RESOURCE_WHALES` | 既定 | 回遊・捕鯨海域。贅沢資源 |
| 硝石（焔硝） | `RESOURCE_NITER` | 既定 | 輸入品なので鉱床ではなく港の交易アクセスとして表す |
| 漆 | `RESOURCE_JAC_LACQUER`（案） | **新規** | 漆器・漆産業圏。原料樹の分布を直接示す資源ではない |
| 木材 | 該当なし | 地形Featureで表現 | `FEATURE_FOREST`（森林）。別資源ノードは置かない |

IDはCiv6 Base/GS資源データとの照合案。Gold Oreは「Gifts of the Nile」シナリオ用として扱われるため、通常マップで使える資源行とは分け、暫定ではカスタムIDにする。実装時には対象DLCのゲーム内 `Resources` 定義で再確認する。[Civ6 Base資源データ](https://civilization.fandom.com/wiki/Module:Data/Civ6/Base/Resources)、[GS資源データ](https://civilization.fandom.com/wiki/Module:Data/Civ6/GS/Resources)、[Gold Ore（Gifts of the Nile）](https://civilization.fandom.com/wiki/Gold_Ore_%28Civ6%29/Gifts_of_the_Nile)、[2K公式Civ6マニュアル](https://cdn.akamai.steamstatic.com/steam/apps/289070/manuals/CIV_VI_25TH_ONLINE_MANUAL_ENG.pdf)

### 代表地点

座標・同一タイルへの吸着前の候補一覧は、機械処理用の[JSON](../spec/data/japan-map-resources.json)にすべて記録する。主要候補は以下のとおり。

| 資源 | 地点・令制国（代表座標 WGS84） | 年代・確度と扱い |
|---|---|---|
| 金 | 黒川金山・甲斐（35.817, 138.825）、中山金山・甲斐（35.377, 138.464）、土肥・伊豆（34.909, 138.790）、宝達・能登（36.784, 136.803）、西三川・佐渡（37.819, 138.273） | 黒川は15世紀末〜17世紀、中山は16世紀前後〜17世紀末の操業推定。土肥は1577年頃、宝達は1586年頃、西三川の本格採取は1593年頃。佐渡は金銀島として一括せず、鶴子は銀、西三川は砂金、相川は1596年発見・1601年本格開発として分ける。甲斐金山は[山梨県](https://www.pref.yamanashi.jp/bunka/maibun/yamanashinobunkazai_sb0010.html)、[身延町](https://www.town.minobu.lg.jp/site/kinzan/1874.html)、土肥は[土肥金山](https://www.toikinzan.com/about/)、宝達は[学術論文](https://kanazawa-u.repo.nii.ac.jp/record/35554/files/Full-H-1121072706-kobayakawa-yugo.pdf)、佐渡は[佐渡市](https://www.city.sado.niigata.jp/site/mine/52560.html)を参照。後発地点は年次タグ付き。相川金銀山（38.032, 138.235、1596年発見・1601年本格開発）は SSOT 3.2 が例に挙げる後発地点として金の候補に加えた。 |
| 銀 | 石見銀山・石見（35.103, 132.437）、生野銀山・但馬（35.167, 134.790）、鶴子銀山・佐渡（37.927, 138.351）、延沢銀山・出羽（38.568, 140.528） | 石見は1520年代発見・16世紀に本格生産、生野は1542年、鶴子は1542年頃、延沢は慶長年間（1596〜1615）に始まったとする公的文化財資料がある。延沢は1600年以後開始の可能性があるため初期配置は要確認。相川銀鉱床は鶴子と近く、20kmタイルでは別ノードにしない。根拠：[UNESCO石見](https://whc.unesco.org/en/list/1246)、[朝来市](https://www.city.asago.hyogo.jp/soshiki/24/1119.html)、[新潟県・鶴子](https://www.pref.niigata.lg.jp/site/sado-goldmine/sg-290.html)、[山形県・延沢](https://www.pref.yamagata.jp/cgi-bin/yamagata-takara/?id=1133&m=detail)。 |
| 銅 | 足尾銅山・下野（36.642, 139.446） | 1550年発見説、16世紀後半に佐野氏の採掘があったとされるが、本格経営は1610年以降。1地点のみ高〜中確度で置く。ほかの銅鉱床は16世紀の操業根拠を確認できず、数を水増ししない。[日光市](https://www.city.nikko.lg.jp/soshiki/10/1041/2_1/1229.html)、[文化庁文化遺産データベース](https://kunishitei.bunka.go.jp/heritage/detail/401/00003567)。 |
| 鉄 | 奥出雲・出雲（35.199, 133.092／35.185, 132.880／35.274, 132.901）、日野・伯耆（35.160, 133.244／35.225, 133.417／35.410, 133.766）、石見山地（35.000, 132.150） | 砂鉄・木炭によるたたら製鉄圏。出雲では古代から、戦国期には毛利氏が良鉄確保のため鉄奉行を置いた。伯耆は中世以降の製鉄遺跡と戦国期の精錬遺構が確認される。各点は広域産地を代表する候補で、生産量の大小を示さない。[文化庁・奥出雲](https://kunishitei.bunka.go.jp/heritage/detail/412/00003855)、[農水省](https://www.maff.go.jp/j/nousin/kantai/giahs_3_221.html)、[鳥取県](https://www.pref.tottori.lg.jp/280339.htm)。 |
| 馬 | 糠部・陸奥（40.690, 141.150／40.180, 141.590／39.700, 141.150）、穂坂牧・甲斐（35.750, 138.430）、木曽・信濃（35.950, 137.560）、下総（35.800, 140.130） | 南部・甲斐・木曽の牧・馬産を中心に6地点。南部馬は青森東部〜岩手北部、木曽馬は平安期から軍事・農耕用、甲斐には官牧があった。下総は広域候補で年代の確度は中〜低。[岩手県立図書館](https://iplsv005.library.pref.iwate.jp/info/evecale/kikakuten/20190518_umacco.html)、[文化庁・木曽馬](https://japan-heritage.bunka.go.jp/ja/culturalproperties/result/2011/)、[山梨県](https://www.pref.yamanashi.jp/toukei_2/book/13dai1-1.html)。 |
| 米 | 越後平野・越後（37.900, 139.050）、庄内・出羽（38.850, 139.880）、仙台平野・陸奥（38.270, 140.920）、関東平野（36.220, 139.950／35.880, 140.150）、濃尾・尾張/美濃（35.200, 136.820／35.050, 136.780）、琵琶湖東岸・近江（35.150, 136.100）、山城盆地（34.900, 135.790）、河内平野（34.620, 135.580）、岡山平野・備前（34.660, 133.940）、讃岐平野・讃岐（34.250, 133.820）、筑紫平野・筑前（33.310, 130.490）、熊本平野・肥後（32.790, 130.700） | 14地点は水田適地と河川低地の分布案。戦国期には稲作・水田開発が続いたが、中世の土地条件と河川氾濫原の開発可能性は地域差がある。特定の戦国大名領の石高・収穫量を復元した数字ではない。近世以降の大規模新田開発も1534年へ遡及しない。[國學院大學・戦国期農業](https://www.kokugakuin.ac.jp/article/171751)、[農水省・農業水利史](https://www.maff.go.jp/j/council/seisaku/nousin/bukai/h24_4/pdf/data1-2.pdf)、[国交省・信濃川](https://www.mlit.go.jp/river/toukei_chousa/kasen/jiten/nihon_kawa/0404_shinano_karyu/0404_shinano_karyu_00.html)、[最上川](https://www.thr.mlit.go.jp/yamagata/river/kasenseibi/gaiyou/002/index.html)、[筑後川](https://www.mlit.go.jp/river/toukei_chousa/kasen/jiten/nihon_kawa/0916_chikugo/0916_chikugo_00.html)。 |
| 塩 | 珠洲・能登（37.440, 137.260）、志摩・伊勢（34.330, 136.820）、赤穂・播磨（34.750, 134.390）、児島・備前（34.500, 133.830）、弓削島・伊予（34.280, 133.200）、小豆島・讃岐（34.480, 134.230） | 古代・中世の瀬戸内には製塩と塩流通の記録がある。**入浜式塩田は17世紀半ばの開発とされるため、1534年の地形・技術としては置かない。** 6地点のうち瀬戸内外の中世製塩地点は根拠の強さに差がある。[塩事業センター](https://www.shiojigyo.com/siohyakka/made/history.html)、[「瀬戸」第82号](https://www.seto.or.jp/upload/publish/seto_vol_82.pdf)、[三重県](https://www.bunka.pref.mie.lg.jp/rekishi/kenshi/asp/arekore/detail80.html)。 |
| 茶 | 宇治・山城（34.890, 135.800）、宇治田原・山城（34.850, 135.870） | 中世から茶産地、室町中期に上位銘柄となり、戦国期には権力者・大名の茶会と結びつく。2地点は20km級タイル上の産地代表で、茶園の個数ではない。[宇治市](https://www.city.uji.kyoto.jp/site/uji-cha/16216.html)、[京都府](https://www.pref.kyoto.jp/chaken/history.html)。 |
| 絹 | 桐生・上野（36.410, 139.330）、郡内・甲斐（35.550, 138.810）、西陣・山城（35.030, 135.750） | 桐生は中世の仁田山絹と1548/1575年の注文記録がある。郡内織物は長い布産地史を持つが、現在知られるブランド産業の本格成長は江戸初期以降。西陣は絹織物・流通圏の代理。**RESOURCE_SILKは生糸・養蚕地の精密復元ではなく商品・織物産地の代表**とする。[桐生市](https://www.city.kiryu.lg.jp/sangyou/1012348/1012405/1012412/index.html)、[山梨県立博物館](https://www.pref.yamanashi.jp/kaiki/kaiki_museum/kaiki-museum/kaiki-guide3.htm)。 |
| 漆 | 輪島・能登（37.390, 136.900）、会津・陸奥（37.490, 139.930） | 新規カスタム資源案。輪島は室町期に漆器生産、会津は産地の萌芽があり1590年に蒲生氏郷が産業振興。土地の漆樹林を示すとは限らず、原料・加工技術・交易を含む産業圏マーカーとしてのみ提案。[輪島漆器組合](https://wajimanuri.or.jp/kumiai/file07/rekishi2.html)、[東北経済産業局](https://www.tohoku.meti.go.jp/s_densan/fukusima_01.html)。 |

### 魚・鯨・輸入品

- **魚：16海上候補。** 北海道西岸、津軽海峡、三陸（2）、仙台湾、銚子沖、相模湾、伊勢湾、駿河湾、能登沖、瀬戸内（3）、玄界灘、有明海、土佐湾をJSONに代表点として置いた。海域の豊かさ・沿岸漁業・交易の広がりからの**漁場推定**であり、各地点に1534年の漁獲統計が存在したという主張ではない。水産庁は日本近海の自然条件の多様さと中世以降の海民の複合的な漁労・海運を説明している。[水産庁](https://www.jfa.maff.go.jp/j/koho/pr/mado/pdf/62.pdf)。
- **鯨：伊勢湾1候補（34.690, 136.770）。** 日本の在来捕鯨業の草創は16世紀後半の伊勢湾地域とする研究がある。17世紀に入ってから各地へ急拡大するため、紀州・土佐・西海を1534年の捕鯨産業地として固定しない。[九州大学学術リポジトリ](https://api.lib.kyushu-u.ac.jp/opac_download_md/2348717/lit0273_abstract.pdf)。ゲーム内の鯨資源は漁業史ではなく海洋資源の存在も表すため、オーナーは出現の有無を確認する。
- **硝石（焔硝）：港の交易アクセス2候補。** 平戸・肥前（33.368, 129.553）は1586年に島津家久が南蛮船から玉薬を買おうとした記録が紹介される。堺・和泉（34.573, 135.483）は国際交易港・火薬流通の候補だが、今回確認できた資料では平戸ほど個別の硝石取引を直接裏付けられなかったため確度を下げる。[戦国日本の津々浦々（史料紹介）](https://proto.harisen.jp/mono/mono/enshou-yunyu.html)、[大阪21世紀協会](https://www.osaka21.or.jp/web_magazine/osaka100/003.html)。`RESOURCE_NITER`の陸上鉱床としては配置せず、港・交易網の状態に置く。
- **木材：資源ノードを置かず森林Featureで表す。** 戦国末期までに畿内近国、中部、関東西部、中国東部、四国内海側で用材生産が進み、秀吉期には木曽・熊野・四国・九州・中国・東北の林地から用材が集められたという林政史研究を参照。[徳川林政史研究所](https://rinseishi.tokugawa.or.jp/kenkyukatsudo/rinseishi_column/)。

## 地形Feature案

### 河川

JSONには10河川の代表経路点を `[経度, 緯度]` で記録した。河川名・経路根拠は国交省の水系資料、座標はその水系図を20kmタイル用に簡略化した提案値。実装時は**線をタイルの辺にラスタライズ**する。河川線を陸地タイルの「河川Feature」として塗る方式にはしない。

| 水系 | 代表経路（源流→河口、概略） | 注意 |
|---|---|---|
| 石狩川 | 石狩岳→上川・旭川→石狩平野→石狩湾 | 明治以後の直線化・捷水路で現在の河道が変化。湿地と旧川は別Feature候補。[北海道開発局](https://www.hkd.mlit.go.jp/ky/kn/kawa_kei/ud49g70000001a6b.html) |
| 北上川 | 北上山地南部→盛岡→花巻→一関→追波湾 | 主要流路の概略。[東北地方整備局](https://www.thr.mlit.go.jp/Bumon/J74201/homepage/history/kitakami.html) |
| 最上川 | 西吾妻山→米沢・山形盆地→新庄→庄内平野→酒田 | 主要流路。[東北地方整備局](https://www.thr.mlit.go.jp/yamagata/river/kasenseibi/gaiyou/002/index.html) |
| 利根川 | 上流→関東平野→（1534年の旧流路）荒川水系と合流し東京湾へ | 現在は銚子へ注ぐが、利根川東遷（1594〜1654年頃）以前の流路は異なる。**1543年に現代の銚子河口をそのまま置かない。** 下流の代表線は概略案で古河道の確定復元ではない。[国交省・河川整備計画](https://www.ktr.mlit.go.jp/ktr_content/content/000640560.pdf)、[利根川水資源](https://www.ktr.mlit.go.jp/river/bousai/river_bousai00000065.html) |
| 信濃川 | 甲武信ヶ岳→千曲川→長野盆地→魚野川合流→越後平野→日本海 | 近代の大河津・関屋分水路は1534年に存在しない。河口・支流合流を修正する。[信濃川河川事務所](https://www.hrr.mlit.go.jp/shinage/shinanogawa/) |
| 木曽三川 | 木祖村・鉢盛山／大日ヶ岳／冠山→濃尾平野→伊勢湾 | 木曽川・長良川・揖斐川の3線を収録。現在の河口部の堤防・河道分離は後世の形状。[国交省](https://www.mlit.go.jp/river/toukei_chousa/kasen/jiten/nihon_kawa/0509_kiso/0509_kiso_00.html) |
| 淀川 | 琵琶湖→瀬田川→宇治川、桂川・木津川合流→大阪湾 | 現代の放水路・河道を除く。[近畿地方整備局](https://www.kkr.mlit.go.jp/river/kasen/yodogawa.html) |
| 斐伊川 | 船通山→横田盆地→出雲平野→宍道湖→中海→日本海 | 現代の放水路は除く。宍道湖・中海は別に湖沼ポリゴンを置く。[中国地方整備局](https://www.mlit.go.jp/river/toukei_chousa/kasen/jiten/nihon_kawa/0713_hiikawa/0713_hiikawa_00.html) |
| 吉野川 | 瓶ヶ森→大歩危・池田→徳島平野→紀伊水道 | 代表流路。[国交省](https://www.mlit.go.jp/river/toukei_chousa/kasen/jiten/nihon_kawa/0801_yoshino/0801_yoshino_00.html) |
| 筑後川 | 阿蘇・瀬の本高原→日田盆地→久留米→筑紫平野→有明海 | 河口干潟・低湿地と一体で表現。[国交省](https://www.mlit.go.jp/river/toukei_chousa/kasen/jiten/nihon_kawa/0916_chikugo/0916_chikugo_00.html) |

### 湖沼

| 湖 | 中心代表点 WGS84 | 面積の目安 | タイル化 |
|---|---:|---:|---|
| 琵琶湖・近江 | 35.310, 136.170 | 約670 km² | 細長い輪郭を保ち2〜4タイル目安 |
| 猪苗代湖・陸奥 | 37.480, 140.080 | 約103 km² | 1タイル程度 |
| 霞ヶ浦・常陸 | 36.040, 140.410 | 約168 km² | 1〜2タイル |
| 十和田湖・陸奥 | 40.470, 140.880 | 約61 km² | 1タイル程度 |
| 宍道湖・出雲 | 35.420, 132.970 | 約79 km² | 1タイル程度 |
| 諏訪湖・信濃 | 36.050, 138.080 | 約13 km² | 1タイル |

湖沼面積・形状の基準は自治体・国土地理院の現行データ。JSONは代表点を記し、実配置では[国土地理院湖沼データ](https://www.gsi.go.jp/kankyochiri/lakedatalist.html)の湖岸形状を優先する。面積と位置の根拠：[滋賀県](https://www.pref.shiga.lg.jp/de51/2878.html)、[福島県](https://www.pref.fukushima.jp/sec/298/inawashiro-chousa.html)、[国交省・斐伊川流域](https://www.cgr.mlit.go.jp/izumokasen/jimusho/jigyo/hikwawa-gaiyo/index.html)。

### 森林・湿地

- **森林：** 森林分布は標高・降水・土地利用に応じた低解像度の推定。適格な平地・丘陵・丘陵タイル（山岳タイルは除く）のうち、内陸丘陵は高確率、河川沿い・平野の農地候補は低確率で `FEATURE_FOREST` を置く。初期目安は**適格タイルの45%**。主要な森林・用材偏重地域は木曽・飛騨、紀伊・吉野、中国山地、四国山地、九州山地、東北山地、北海道内陸。実測の16世紀森林率・細密植生図がないため、45%はゲーム用調整値。現代植生の形状参照には[環境省自然環境調査Web-GIS](https://eadas.env.go.jp/eiadb/ebidbs/Service/About?title=Gis)、歴史的用材地は[徳川林政史研究所](https://rinseishi.tokugawa.or.jp/kenkyukatsudo/rinseishi_column/)を用いる。
- **湿地：** `FEATURE_MARSH` は河口・氾濫原・高層湿原候補に限定し、初期は**陸地の約2〜3%（22〜33タイル）**を上限にする。集中域は釧路・サロベツ、尾瀬、石狩低地、中海・宍道湖周辺、木曽三川デルタ、利根川下流、筑後川・有明海。湿地座標は候補クラスター中心で、個々の22〜33タイル配置を厳密に表す境界ではない。現行ラムサール湿地一覧は[環境省](https://www.env.go.jp/nature/ramsar/conv/RamsarSites_in_Japan.html)。有明海河口の干潟などは、1534年の海岸線・干拓状況に合わせて陸地判定を補正する。

### 火山と自然の驚異

- `FEATURE_VOLCANO`（Gathering Storm）候補は、16世紀に噴火記録がある**浅間山（36.406, 138.523）・富士山（35.361, 138.727）・阿蘇山（32.884, 131.104）・桜島（31.590, 130.660）・霧島山（31.910, 130.860）**。位置と年代は気象庁の活火山データベース・噴火履歴を使用。[浅間山](https://www.data.jma.go.jp/vois/data/tokyo/306_Asamayama/306_history.html)、[富士山](https://www.data.jma.go.jp/vois/data/tokyo/314_Fujisan/314_history.html)、[阿蘇山](https://www.data.jma.go.jp/vois/data/fukuoka/503_Asosan/503_history.html)、[桜島](https://www.data.jma.go.jp/vois/data/fukuoka/506_Sakurajima/506_history.html)、[霧島山](https://www.data.jma.go.jp/vois/data/fukuoka/505_Kirishimayama/505_history.html)。山頂/火口を含む地形タイルを1候補ずつ選ぶ。同一火山群に複数置くのは避ける。
- **自然の驚異案：富士山を第一候補、阿蘇カルデラを代替候補。** 富士山は国際的知名度・文化景観が強いが、Civ6 GSの既定Natural Wonderではないためカスタム追加が必要。Civ5の既定Wonderと混同しない。阿蘇を選ぶなら火山Featureとの重複ルールが必要。採用上限は1件を推奨。[富士山世界文化遺産](https://whc.unesco.org/en/list/1418/)、[GS公式マニュアル](https://cdn.2kgames.com/manuals/civilization-vi/gathering_storm/2KGMKT_CIV6_Gathering_Storm_PC_Online_Manual_V7_US.pdf)、[Civ6 Wonder一覧](https://civilization.fandom.com/wiki/Natural_wonder)。

## 生成器に適用する簡易ルール

1. 歴史地点をWGS84から地図投影へ変換し、公式 `Resource_ValidTerrains`・`Resource_ValidFeatures` と資源の湖・河川・沿岸条件に合う60 km以内の最寄りのタイル1つへ吸着。同じタイルに複数候補が来たら、根拠強度・時代適合度の高い1件だけ残し、他を重複監査リストに記録する。絹には森林を付け、沿岸の塩田は開けた平原として近似する。石見銀山は公式の銀が砂漠・ツンドラ系しか許さず、現地の気候を誤って表すため資源ノードを保留し、その理由を生成記録に残す。
2. `start_year > 1534` は1534年開始マップでは非表示、または技術/イベント解放まで未発見扱いにする。1590年代発見地点を1534年の既知産地と誤表示しない。
3. `RESOURCE_NITER` は鉱床配置を禁止。港の交易アクセスとして記録し、輸入が裏付けられる年代以降にだけ有効にする。
4. 河川は簡略化ポリラインを陸地・海岸の辺へ割り当てる。旧利根川、石狩川下流、淀川、信濃川は近世・近代の河道工事を除去する。
5. 湖は現代輪郭データから現在の陸地形状を参照し、歴史的干拓・埋立が確認できるデルタは別途年代補正する。
6. 森林Featureは適格陸地の45%を初期値として、森林資源・山地に重み付け。湿地は主要クラスターへ22〜33タイルを上限として配る。
7. 火山は気象庁の候補位置に対して火山地形タイルを1つずつ。自然の驚異を採用した火山は重複させず、カスタムWonderはオーナー選定後に1つだけ配置する。

## オーナー確認事項

1. 1534年の開始時点で未発見の資源を配置せず、後発の時代イベントで解禁する方針にするか。
2. 魚・米・森林のような広域資源を生産余剰の代表として置く案（魚16、米14、森林45%）は、鉱山史跡より推定幅が広い。この密度を採るか。
3. 漆をカスタム資源にし、木材は森林Feature、硝石は港交易アクセスとして分けるか。
4. 瀬戸内の塩ノードは置くが、入浜式製塩の名称・Featureを1534年へ遡らせない扱いでよいか。
5. 富士山をカスタム自然の驚異にするか、阿蘇カルデラを代替にするか、Wonder自体を置かないか。
