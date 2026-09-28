### 14.5 領土主張関係と初期タイル割当データ

ここは3.3で確定した「領有権の主張は関係として記録し、タイルは一つの主体だけが所有する」という規則を、台帳データへ接続した範囲である。全851収録主体の地理的割当候補と、682件の相手主体との関係を別データで保持する。関係のうち既存台帳から継承したものは656件で、残りは係争などの明示的な関係である。国・主体の統計面積から直接所有タイル数を推計しない。

関係種別は、帰属・自治、自由連合、主権をめぐる争い、共同管轄、単なる承認と実効統治を区別する。たとえば[自由連合](https://www.mfat.govt.nz/en/countries-and-regions/australia-and-pacific/cook-islands)は相手国の直接領土ではなく、[北キプロスの緩衝地帯](https://unficyp.unmissions.org/en/about-buffer-zone)は南北いずれかのタイル所有へ自動変換しない。小さな登録主体を優先するのは、実際にそのタイルの所有候補になる現地単位同士の比較である。他国の広域主張だけでは所有候補に昇格しない。

下表の「候補」は地理と基準日時点の統治の裏付けを後で照合するためのIDであり、**Civ6の実タイルに所有者を設定した意味ではない**。入力したタイル座標・ポリゴンは0件。重点27地域のうち17件に暫定候補があり、10件は主体未登録、競合する実効統治、地理資料不足などで候補も保留した。「保留」のタイルを敵対当事者へ勝手に割り当てず、未所有・監査対象とする。以下の三言語名はゲーム表示用の暫定訳で、行のリンクは地域の存在・統治関係の根拠を示すものであり、日露の訳語の典拠とは限らない。調査の証拠上限は2026年9月28日だが、シナリオ開始年は未設定。[ソマリアの局所統治の資料時点](https://www.euaa.europa.eu/somalia-security-situation/27-sool-sanaag-cayn-and-north-eastern-state-nes)は2026年3月31日、[パレスチナの状況報告](https://www.ochaopt.org/content/humanitarian-situation-report-11-september-2026)は2026年9月11日であり、同時刻の世界地図を確定したものではない。

| 地域ID | 地域名（日本語 / English / Русский） | 関係する主体ID | 初期所有候補 | 判断・保留理由 |
|---|---|---|---|---|
| BA-BRC | ブルチコ行政区 / [Brčko District](https://www.vijeceministara.gov.ba/en/o-bih) / Округ Брчко | BIH, BA-FBIH, BA-RS, BA-BRC | 候補：BA-BRC（実タイル未割当） | [共同の法的管轄でもゲーム上はブルチコを単一候補とする](https://www.vijeceministara.gov.ba/en/o-bih) |
| TWN | 台湾の統治領域 / [Taiwan-administered territory](https://www.congress.gov/crs-product/IF10275) / Территория под управлением Тайваня | CHN, TWN | 候補：TWN（実タイル未割当） | [中国の主張は関係に保持。島嶼ごとの実境界は未入力](https://www.congress.gov/crs-product/IF10275) |
| XKX | コソボの統治領域 / [Kosovo-administered territory](https://commonslibrary.parliament.uk/research-briefings/cbp-10012/) / Территория под управлением Косова | SRB, XKX | 候補：XKX（実タイル未割当） | [北部の並行制度があるため全面積への自動確定をしない](https://commonslibrary.parliament.uk/research-briefings/cbp-10012/) |
| XKX-NORTH | 北部コソボ / [Northern Kosovo](https://commonslibrary.parliament.uk/research-briefings/cbp-10012/) / Северное Косово | SRB, XKX | 保留（実タイル未割当） | [並行制度を排他的なタイル統治と同一視せず、地理・統治線を監査](https://commonslibrary.parliament.uk/research-briefings/cbp-10012/) |
| CYP-N | 北キプロスの統治領域 / [Northern Cyprus-controlled area](https://unficyp.unmissions.org/en/about-buffer-zone) / Территория Северного Кипра | CYP, CYP-N | 候補：CYP-N（実タイル未割当） | [国連緩衝地帯は含めない](https://unficyp.unmissions.org/en/about-buffer-zone) |
| CYP-BUFFER | 国連管理緩衝地帯 / [UN-controlled Cyprus buffer zone](https://unficyp.unmissions.org/en/about-buffer-zone) / Буферная зона ООН на Кипре | CYP, CYP-N | 保留（実タイル未割当） | [国連管理を隣接国の直接所有へ自動変換しない](https://unficyp.unmissions.org/en/about-buffer-zone) |
| SOM-SL-CORE | ソマリランドの中核統治地域 / [Somaliland core control](https://www.euaa.europa.eu/somalia-security-situation/27-sool-sanaag-cayn-and-north-eastern-state-nes) / Основная зона управления Сомалиленда | SOM, SOM-SL | 候補：SOM-SL（実タイル未割当） | [周縁の主張区域まで一括適用しない](https://www.euaa.europa.eu/somalia-security-situation/27-sool-sanaag-cayn-and-north-eastern-state-nes) |
| SOM-SOOL | スール地方 / [Sool](https://www.euaa.europa.eu/somalia-security-situation/27-sool-sanaag-cayn-and-north-eastern-state-nes) / Сул | SOM, SOM-SL, SOM-NES, SOM-PL | 保留（実タイル未割当） | [州の一括所有ではなく町・地区へ分割する。NESとプントランドは登録保留](https://www.euaa.europa.eu/somalia-security-situation/27-sool-sanaag-cayn-and-north-eastern-state-nes) |
| SOM-SANAAG | サナーグ地方 / [Sanaag](https://www.euaa.europa.eu/somalia-security-situation/27-sool-sanaag-cayn-and-north-eastern-state-nes) / Санаг | SOM, SOM-SL, SOM-NES, SOM-PL | 保留（実タイル未割当） | [主張が重複しNESの実在が地域全体の実効支配を意味しない](https://www.euaa.europa.eu/somalia-security-situation/27-sool-sanaag-cayn-and-north-eastern-state-nes) |
| SOM-LASANOD | ラス・アノド周辺 / [Laas Caanood area](https://www.euaa.europa.eu/somalia-security-situation/27-sool-sanaag-cayn-and-north-eastern-state-nes) / Район Лас-Анода | SOM, SOM-SL, SOM-NES, SOM-PL | 保留（実タイル未割当） | [連邦側連合の統治区域。ただし未登録のNESを所有者IDにはしない](https://www.euaa.europa.eu/somalia-security-situation/27-sool-sanaag-cayn-and-north-eastern-state-nes) |
| SOM-OOG | オーグ / [Oog](https://www.euaa.europa.eu/somalia-security-situation/27-sool-sanaag-cayn-and-north-eastern-state-nes) / Оог | SOM, SOM-SL, SOM-NES | 候補：SOM-SL（実タイル未割当） | [ソマリランド当局の統治報告。NESの主張は関係として残す](https://www.euaa.europa.eu/somalia-security-situation/27-sool-sanaag-cayn-and-north-eastern-state-nes) |
| SOM-BOOCAME | ブーカメ / [Boocame](https://www.euaa.europa.eu/somalia-security-situation/27-sool-sanaag-cayn-and-north-eastern-state-nes) / Боокаме | SOM, SOM-NES, SOM-PL | 保留（実タイル未割当） | [混合統治が報告され、単一の現地所有者を裏付けられない](https://www.euaa.europa.eu/somalia-security-situation/27-sool-sanaag-cayn-and-north-eastern-state-nes) |
| ESH | 西サハラ / [Western Sahara](https://minurso.unmissions.org/en/mandate) / Западная Сахара | ESH, MAR, POLISARIO | 保留（実タイル未割当） | [全域一括所有不可。停戦線と勢力別境界の地理資料を要する](https://minurso.unmissions.org/en/mandate) |
| GE-AB | アブハジア / [Abkhazia](https://www.bbc.com/news/world-europe-18175030) / Абхазия | GEO, GE-AB | 候補：GE-AB（実タイル未割当） | [ジョージアの帰属主張と地元の実効機構を区別。精密な境界は未入力](https://www.bbc.com/news/world-europe-18175030) |
| GE-SOS | 南オセチア / [South Ossetia](https://www.defactoborders.org/places/south-ossetia) / Южная Осетия | GEO, GE-SOS | 候補：GE-SOS（実タイル未割当） | [独立宣言主体の候補。局地的境界は未入力](https://www.defactoborders.org/places/south-ossetia) |
| MD-TRN | 沿ドニエストル / [Transnistria](https://www.bbc.com/news/world-europe-18284837) / Приднестровье | MDA, MD-TRN | 候補：MD-TRN（実タイル未割当） | [モルドバ側の統治地域を自動で取り込まない](https://www.bbc.com/news/world-europe-18284837) |
| IQ-KRI | クルディスタン地域の自治中核 / [Kurdistan Region administered core](https://presidency.gov.krd/en/kurdistan-region/) / Автономное ядро Региона Курдистан | IRQ, IQ-KRI | 候補：IQ-KRI（実タイル未割当） | [連邦関係を残しつつ中核の局所主体を優先](https://presidency.gov.krd/en/kurdistan-region/) |
| IQ-KIRKUK | キルクークの係争地域 / [Kirkuk disputed area](https://www.reuters.com/business/media-telecom/kurdish-struggle-rights-land-2026-03-05/) / Спорный район Киркука | IRQ, IQ-KRI | 候補：IRQ（実タイル未割当） | [2017年以降のバグダッド統治報道に基づく暫定候補。県全域ではない](https://www.reuters.com/business/media-telecom/kurdish-struggle-rights-land-2026-03-05/) |
| IN-JK | ジャンムー・カシミールのインド管理地域 / [India-administered Jammu and Kashmir](https://en.wikipedia.org/wiki/States_and_union_territories_of_India) / Джамму и Кашмир под управлением Индии | IND, IN-JK, PAK | 候補：IN-JK（実タイル未割当） | [管理区域の局所主体を候補とし、相手国の全域主張は関係に残す。境界資料は未入力](https://en.wikipedia.org/wiki/States_and_union_territories_of_India) |
| IN-LA | ラダックのインド管理地域 / [India-administered Ladakh](https://en.wikipedia.org/wiki/States_and_union_territories_of_India) / Ладакх под управлением Индии | IND, IN-LA, PAK | 候補：IN-LA（実タイル未割当） | [管理区域の局所主体を候補とし、相手国の全域主張は関係に残す。境界資料は未入力](https://en.wikipedia.org/wiki/States_and_union_territories_of_India) |
| PK-JK | アザド・カシミールのパキスタン管理地域 / [Pakistan-administered Azad Kashmir](https://www.euaa.europa.eu/pakistan-country-focus/426-azad-jammu-and-kashmir-ajk-and-gilgit-baltistan-gb) / Азад-Кашмир под управлением Пакистана | PAK, PK-JK, IND | 候補：PK-JK（実タイル未割当） | [管理区域の局所主体を候補とし、相手国の全域主張は関係に残す。境界資料は未入力](https://www.euaa.europa.eu/pakistan-country-focus/426-azad-jammu-and-kashmir-ajk-and-gilgit-baltistan-gb) |
| PK-GB | ギルギット・バルティスタンのパキスタン管理地域 / [Pakistan-administered Gilgit-Baltistan](https://www.euaa.europa.eu/pakistan-country-focus/426-azad-jammu-and-kashmir-ajk-and-gilgit-baltistan-gb) / Гилгит-Балтистан под управлением Пакистана | PAK, PK-GB, IND | 候補：PK-GB（実タイル未割当） | [管理区域の局所主体を候補とし、相手国の全域主張は関係に残す。境界資料は未入力](https://www.euaa.europa.eu/pakistan-country-focus/426-azad-jammu-and-kashmir-ajk-and-gilgit-baltistan-gb) |
| SSD-ABY | アビエイ地域 / [Abyei Area](https://unisfa.unmissions.org/en) / Район Абьей | SDN, SSD, SSD-ABY | 保留（実タイル未割当） | [地位係争。未登録の特別地域をタイル所有者にしない](https://unisfa.unmissions.org/en) |
| PSE-GAZA | ガザ地区 / [Gaza Strip](https://www.ochaopt.org/content/humanitarian-situation-report-11-september-2026) / Сектор Газа | PSE, ISR | 保留（実タイル未割当） | [複数の権限・作戦域があり、全域の所有者は資料から一意に確定できない](https://www.ochaopt.org/content/humanitarian-situation-report-11-september-2026) |
| PSE-WB | ヨルダン川西岸 / [West Bank](https://www.ochaopt.org/content/humanitarian-situation-report-11-september-2026) / Западный берег реки Иордан | PSE, ISR | 保留（実タイル未割当） | [地区別に統治関係が異なり、一つの所有者へ自動割当しない](https://www.ochaopt.org/content/humanitarian-situation-report-11-september-2026) |
| FLK | フォークランド諸島 / [Falkland Islands](https://www.bbc.com/news/articles/cn45jg3zlz4o) / Фолклендские острова | GBR, ARG, FLK | 候補：FLK（実タイル未割当） | [英国の海外領土としての管理を候補としアルゼンチンの主張は関係に記録](https://www.bbc.com/news/articles/cn45jg3zlz4o) |
| AR-V | ティエラ・デル・フエゴ州の非係争域 / [Tierra del Fuego core](https://www.britannica.com/place/Tierra-del-Fuego-province-Argentina) / Основная территория Огненной Земли | ARG, AR-V | 候補：AR-V（実タイル未割当） | [南極・南大西洋の係争域を分類用の州面積に混入しない](https://www.britannica.com/place/Tierra-del-Fuego-province-Argentina) |

機械可読データには各主体の`related_id`、関係種別、相手主体ID、出典URL、重点地域の所有候補、未登録主体のブロック条件を保存した。各地域では`relationship_participant_ids`（主張・所属の関係者）と`eligible_local_owner_candidate_ids`（実地図を作った後に所有候補となれるID）を分ける。両者を同じ配列としてタイル所有の算定に使わない。親国を含む広域の関係データは削除しないが、構成主体に割り当てたタイルを親国へも重ねて所有させない。

以下の26件は、14.4の各行から自動復元できる656件の`関連国ID`とは別に確認した関係の全件である。656件の内訳は通常の憲法上・行政上の関係646件、自由連合5件（FSM、MHL、PLW、COK、NIU）、係争中の親国の主張5件（CYP-N、SOM-SL、GE-AB、GE-SOS、MD-TRN）。これらを足した682件が本書の初期領土関係の全件であり、過去に共有したJSONだけに固有の主張関係は残さない。関係IDは表示訳ではなく実装用の種別キーである。
未編入の参照IDはSOM-NES、SOM-PL、SSD-ABY、SSD-GPB、SSD-RUW、POLISARIOで、これらはこの段階ではタイル所有者にできない。SGSとVE-Wは別の地域監査IDである。

| 主体ID | 対象ID | 関係ID | 対象区域ID | 内容・根拠 |
|---|---|---|---|---|
| CHN | TWN | sovereignty_claim | TWN | [台湾の現行統治を意味しない](https://www.congress.gov/crs-product/IF10275) |
| SRB | XKX | sovereignty_nonrecognition | XKX | [北部の並行制度とタイル所有を同一視しない](https://commonslibrary.parliament.uk/research-briefings/cbp-10012/) |
| CYP | CYP-N | sovereignty_claim | CYP-N | [停戦線・国連管理緩衝地帯を別区画で扱う](https://www.securitycouncilreport.org/un_documents_type/security-council-resolutions/page/4?ctype=Cyprus&cbtype=cyprus) |
| SOM | SOM-SL | sovereignty_claim | SOM-SL | [ソマリランド側の独立主張と並存](https://www.euaa.europa.eu/somalia-security-situation/27-sool-sanaag-cayn-and-north-eastern-state-nes) |
| SOM | SOM-NES | federal_member_recognition | SOM-NES | [連邦所属は親国の直接タイル所有ではない](https://www.euaa.europa.eu/somalia-security-situation/27-sool-sanaag-cayn-and-north-eastern-state-nes) |
| SOM-NES | SOM-SL | territorial_claim_overlap | SOM-SL | [スール、サナーグ、カインで競合](https://www.euaa.europa.eu/somalia-security-situation/27-sool-sanaag-cayn-and-north-eastern-state-nes) |
| SOM-PL | SOM-NES | territorial_claim_overlap | SOM-NES | [連邦内の主張・実効統治が一致しない](https://www.euaa.europa.eu/somalia-security-situation/27-sool-sanaag-cayn-and-north-eastern-state-nes) |
| SOM-PL | SOM-SL | territorial_claim_overlap | SOM-SL | [スール、サナーグで競合](https://www.euaa.europa.eu/somalia-security-situation/27-sool-sanaag-cayn-and-north-eastern-state-nes) |
| MAR | ESH | integration_claim | ESH | [国連の地位確定と同義ではない](https://minurso.unmissions.org/en/mandate) |
| POLISARIO | ESH | self_determination_independence_claim | ESH | [未登録の政治主体。ゲーム所有者にはまだ使わない](https://minurso.unmissions.org/en/mandate) |
| GEO | GE-AB | sovereignty_claim | GE-AB | [アブハジアによる独立宣言と分離](https://www.bbc.com/news/world-europe-18175030) |
| GEO | GE-SOS | sovereignty_claim | GE-SOS | [南オセチアによる独立宣言と分離](https://www.defactoborders.org/places/south-ossetia) |
| MDA | MD-TRN | sovereignty_claim | MD-TRN | [沿ドニエストルの事実上の当局と区別](https://www.bbc.com/news/world-europe-18284837) |
| IRQ | IQ-KRI | federal_constitutional_authority | IQ-KRI | [イラク内の自治地域](https://presidency.gov.krd/en/kurdistan-region/) |
| IQ-KRI | IRQ | disputed_areas_internal_claim | IQ-KIRKUK | [キルクークなどを一括割当しない](https://presidency.gov.krd/en/kurdistan-region/) |
| IND | PK-JK | kashmir_territorial_claim | PK-JK | [インド・パキスタン双方がカシミール全体を主張](https://www.euaa.europa.eu/pakistan-country-focus/426-azad-jammu-and-kashmir-ajk-and-gilgit-baltistan-gb) |
| IND | PK-GB | kashmir_territorial_claim | PK-GB | [同上](https://www.euaa.europa.eu/pakistan-country-focus/426-azad-jammu-and-kashmir-ajk-and-gilgit-baltistan-gb) |
| PAK | IN-JK | kashmir_territorial_claim | IN-JK | [同上](https://www.euaa.europa.eu/pakistan-country-focus/426-azad-jammu-and-kashmir-ajk-and-gilgit-baltistan-gb) |
| PAK | IN-LA | kashmir_territorial_claim | IN-LA | [同上](https://www.euaa.europa.eu/pakistan-country-focus/426-azad-jammu-and-kashmir-ajk-and-gilgit-baltistan-gb) |
| SDN | SSD-ABY | status_dispute | SSD-ABY | [南スーダンとの最終地位が未決](https://unisfa.unmissions.org/en) |
| SSD | SSD-ABY | status_dispute | SSD-ABY | [スーダンとの最終地位が未決](https://unisfa.unmissions.org/en) |
| ARG | FLK | sovereignty_claim | FLK | [英国の行政とアルゼンチンの主張を区別](https://www.bbc.com/news/articles/cn45jg3zlz4o) |
| ARG | SGS | sovereignty_claim | SGS | [監査区域であり独立勢力IDではない](https://commonslibrary.parliament.uk/research-briefings/cbp-9706/) |
| BA-FBIH | BA-BRC | shared_legal_jurisdiction | BA-BRC | [法的な重層権限は二重タイル所有ではない](https://www.vijeceministara.gov.ba/en/o-bih) |
| BA-RS | BA-BRC | shared_legal_jurisdiction | BA-BRC | [ブルチコ地区の単一所有候補を優先](https://www.vijeceministara.gov.ba/en/o-bih) |
| ISR | PSE | regional_administrative_and_security_presence | PSE | [ガザとヨルダン川西岸の当局・軍事的関与を地域別に検証。主権主張の根拠としては使わない](https://www.ochaopt.org/content/humanitarian-situation-report-11-september-2026) |

タイル配置時の自動検証は、ポリゴンで覆われる登録主体の中から最小の有資格単位を優先し、同順位では基準日時点の検証済みの実効統治を使う。未登録主体が残る・緩衝地帯である・両当事者を判別できない場合は空の所有者として停止する。単一タイルへ2主体を記録することはない。テストには[ブルチコ行政区](https://www.vijeceministara.gov.ba/en/o-bih)と[アブハジア](https://www.bbc.com/news/world-europe-18175030)などの**架空の座標を仮定した規則例**を使ったが、現実の地図でそのタイル座標や所有権を確定したという意味ではない。

未完了：世界マップの基準日、実ポリゴンとタイル座標、ソマリア北東州など未編入の主体ID、係争地の地域別統治の再検証。基準日やポリゴンのない状態で実タイル所有を埋めない。

既存のワークスペースで作業するときの生成順は`country_ledger_build.py` → `territory_allocation_build.py` → `render_country_ledger.py` → `render_territory_model.py`。これらは以前の調査結果から本文とJSONを生成する作業用スクリプトであり、**本書だけからJSONを自動再生成する処理はまだ未実装**である。Codex/Claude Codeへの引き継ぎは本書一つで成立し、JSONを再び配布する必要はない。機械出力を使う際は本書の各行・関係表との一致を検証する。
---
