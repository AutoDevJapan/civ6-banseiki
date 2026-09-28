### 14.4 世界マップ国別台帳（初稿）

この台帳は、面積だけで小国・大国を判定し、独立・帰属・国間関係を別列で記録する。現実の主権をゲーム内分類から推論しない。領土主張は関係として保持し、タイル所有は3.3の最小地理単位優先規則によって一意に決める。世界マップの基準日と国別の指導者・都市・タイルの全件入力は未完了であり、以下は国名・面積・初期関係の**調査済み部分**である。

この初稿は、[国連加盟国193件](https://www.un.org/en/about-us/member-states)と[非加盟オブザーバー2主体](https://www.un.org/en/about-us/non-member-states)を基礎に、関係を別途確認した656主体を加えた**計851行**である。日本語名は[Unicode CLDR](https://www.unicode.org/cldr/charts/48/by_type/)等による表示仮訳、英語・ロシア語名は国別の[国連M49](https://unstats.un.org/unsd/methodology/m49/)または[Unicode CLDR区画名](https://github.com/unicode-org/cldr/tree/main/common/subdivisions)を参照し、非M49の主体は表示仮訳とする。母語話者による校正は未実施。国連M49は統計上の国・地域コードであり、収録しただけで独立承認を意味しない（[国連統計部](https://unstats.un.org/unsd/methodology/m49/)）。

面積は原則として[世界銀行の2023年利用可能最新値（AG.SRF.TOTL.K2）](https://api.worldbank.org/v2/country/all/indicator/AG.SRF.TOTL.K2?format=json&per_page=500&mrv=1)を使用した。モナコの世界銀行API値74.9 km²は採らず、[豪外務省の約2 km²](https://www.dfat.gov.au/countries/monaco/monaco-country-brief)に置換した。ナウルは[豪国防軍の21 km²](https://cove.army.gov.au/article/kyr-nauru-information)、聖座の地図上の領土は別人格の[バチカン市国0.44 km²](https://www.vaticanstate.va/en/state-and-government/general-informations/geography.html)を用いる。パレスチナの6,025 km²は世界銀行の「West Bank and Gaza」統計対象面積であり、未解決の国境・実効支配を確定する値ではない（[世界銀行の面積系列](https://api.worldbank.org/v2/country/all/indicator/AG.SRF.TOTL.K2?format=json&per_page=500&mrv=1)）。

面積系列の外れ値はそのまま地図にしない。カナダは世界銀行APIの15,634,410 km²に代え[統計カナダの9,984,670 km²](https://www150.statcan.gc.ca/n1/pub/11-402-x/2012000/chap/geo/geo-eng.htm)、クロアチアは同APIの88,070 km²に代え[地理資料の56,594 km²](https://www.britannica.com/place/Croatia)を採る。新規に昇格した海外領域の多くは[面積一覧の二次資料](https://en.wikipedia.org/wiki/List_of_countries_and_dependencies_by_area)に基づく暫定値であり、一次統計との一致と領海の除外はまだ全件検証できていない。親国・構成主体の面積を国際比較用の一列に合算せず、タイル分割では親国の列と構成主体の列を二重計上しない。

新規のドイツ16州は[連邦参議院の州一覧](https://www.bundesrat.de/EN/organisation-en/laender-en/laender-en-node.html)を構成根拠とし、[公的統計の2023年面積](https://www.statistikportal.de/de/bevoelkerung/flaeche-und-bevoelkerung)で規模を判定する。スペイン17自治州と2自治都市は[EU地域制度の解説](https://portal.cor.europa.eu/divisionpowers/Pages/Spain-intro.aspx)を構成根拠とし、[スペイン国立地理院の個別面積表](https://www.ign.es/resources/ane/Informacion_Geografica_Destacada/IGN_INFOGEO_CCAA.xlsx)で判定する。県・共同体・親国を同じ領土へ重ねた別所有者にしない。個別面積を足すとドイツ357,684.19 km²、スペイン506,020.70 km²となり、[世界銀行の親国系列](https://api.worldbank.org/v2/country/all/indicator/AG.SRF.TOTL.K2?format=json&per_page=500&mrv=1)の357,680 km²、505,975.98 km²とは一致しない。測定年・面積の定義・飛び地の扱いを地図作業時に検証し、差分を特定の州へ根拠なく配賦しない。

カナダの10州と3準州は[カナダ政府の区分](https://www.canada.ca/en/intergovernmental-affairs/services/provinces-territories.html)で確認し、[カナダ統計局の淡水込み面積](https://www150.statcan.gc.ca/n1/pub/11-402-x/2010000/chap/geo/tbl/tbl07-eng.htm)を使う。13行の合計9,984,670 km²は[カナダの総面積](https://www150.statcan.gc.ca/n1/pub/11-402-x/2012000/chap/geo/geo-eng.htm)と一致するが、州と準州の憲法上の権限まで同一視しない。

米国は[労働統計局の50州とD.C.の識別子](https://www.bls.gov/respondents/mwr/electronic-data-interchange/appendix-d-usps-state-abbreviations-and-fips-codes.htm)に沿って、50州と連邦首都地区を区別する。州の面積は[2010年米国勢調査局の表](https://www.census.gov/geographies/reference-files/2010/geo/state-area.html)の陸地＋内水＋五大湖水域の各km²整数値を合算した暫定値。原表の「総面積」には沿岸水域や領海も入るためそのまま用いない。この算式ではデラウェア州5,284 km²、ロードアイランド州3,148 km²が小国で、D.C.は177 km²の小国区分だが州ではない。D.C.の[限定自治と議会権限](https://www.congress.gov/crs-product/R47927)を初期関係へ記録した。州別の2010年統計と親国の世界銀行2023年系列は同じ境界定義・年ではないため、面積合計の単純な一致は要求しない。

オーストラリアの6州、自治権を持つ首都特別地域（ACT）・北部準州（NT）、連邦管理のジャービス湾特別地域（JBT）を別個に収録する。[Geoscience Australiaの陸地面積表](https://www.ga.gov.au/scientific-topics/national-location-information/dimensions/area-of-australia-states-and-territories)の本土＋島嶼の値を使い、9行の合計は7,688,287 km²。JBTは[豪政府の説明](https://www.infrastructure.gov.au/territories-regions-cities/territories/jervis-bay-territory/governance-administration)どおりACTの一部ではなく、適用可能なACT法が及ぶ別領域である。AU-JBTは独自の台帳IDであり、ISOの州・準州コードではない。親国AUSとのタイル二重所有は避ける一方、[Wreck Bay Aboriginal Community Councilの土地権](https://www.niaa.gov.au/our-work/environment-and-land)はゲーム上の領土タイル所有と同一視しない。既存のクリスマス島・ココス諸島・ノーフォーク島とは重複させず、親国に記載した世界銀行系列の面積と今回の陸地面積の単純な一致も要求しない。

さらにブラジル、インド、メキシコ、アルゼンチン、ナイジェリア、スイス、オーストリア、マレーシア、ネパール、南アフリカ、ボリビア、ミクロネシア連邦、コモロ、ベネズエラ、スーダン、アラブ首長国連邦、パキスタン、エチオピア、南スーダン、ミャンマー、パプアニューギニア、ロシア、中国、パラオ、セントクリストファー・ネービスの25か国群に属する467の第一層区分を追加した。三言語名は[CLDRの区画データ](https://github.com/unicode-org/cldr/tree/main/common/subdivisions)を基礎とし、更新された区画名や面積は各行の出典で補った。登録はゲーム内の個別勢力の候補であり、行政単位を独立国と認定するものではない。親国と構成区画はタイルを二重所有しない。

ロシアの一部自治管区と州は行政上の包含関係があるため、[州面積表の注記](https://en.wikipedia.org/wiki/List_of_federal_subjects_of_Russia_by_population)に沿って包含分を控除した値を使い、ゲームの境界線はなお未確定とする。エチオピアの新設州は旧南部諸民族州を重ねて登録せず、[英国政府の地名資料](https://assets.publishing.service.gov.uk/media/6981e0cae8900d4c455f3759/Ethiopia_Toponymic_Factfile.pdf)も参照した。カシミールは[EUAAの地域説明](https://www.euaa.europa.eu/pakistan-country-focus/426-azad-jammu-and-kashmir-ajk-and-gilgit-baltistan-gb)を踏まえ係争線の帰属を自動確定しない。パラオの1 km²未満の州も最低1タイルとし、ベネズエラの連邦属領の島嶼集約は単一の自治州として登録を保留する。

個別に、[アブハジア](https://www.bbc.com/news/world-europe-18175030)、[南オセチア](https://www.defactoborders.org/places/south-ossetia)、[沿ドニエストル](https://www.bbc.com/news/world-europe-18284837)の独立宣言と限定的または未承認の地位を区別して収録した。[イラクのクルディスタン地域](https://presidency.gov.krd/en/kurdistan-region/)、[モルドバのガガウズ自治区](https://www.osw.waw.pl/en/publikacje/osw-commentary/2014-03-10/gagauzia-growing-separatism-moldova)、[タンザニアのザンジバル](https://www.zipa.go.tz/zanzibar-at-a-glance/)、[インドネシアのアチェ州](https://en.wikipedia.org/wiki/Aceh,_Indonesia)は自治主体であって独立国と自動判定しない。面積資料が相違するものは各行の備考・出典に残し、国境紛争や行政区域が実効支配と一致すると仮定しない。

193加盟国の「親国なし」は国家としての基本属性のみを意味する。他国との個別の同盟・協定が存在しないという断定ではなく、確認した集団協定・自由連合等の参加国IDは初期協定タグとして表示した。「国」としての独立性と面積上の小国・大国は混同しない。開始時の外交ボーナス・連邦参加時の権利義務は未設計のままである。

#### この節のファイル

| ファイル | 内容 |
|---|---|
| [un-members-a-f.md](un-members-a-f.md) / [un-members-g-n.md](un-members-g-n.md) / [un-members-o-z.md](un-members-o-z.md) | 国連加盟国193件（英語名順） |
| [un-observers.md](un-observers.md) | 国連非加盟オブザーバー2主体 |
| [subunits/README.md](subunits/README.md) | 史実上の特別な関係・係争を個別確認した656主体（親国別） |
| [pending-unregistered.md](pending-unregistered.md) | 地図化までの要確認主体（未編入） |
| [agreements.md](agreements.md) | 史実に基づく初期協定・組織所属 |
| [m49-audit.md](m49-audit.md) | M49追加14地域の関係監査 |
