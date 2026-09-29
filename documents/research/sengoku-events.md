# 戦国パックの史実イベント一覧（1534〜1600年）

調査基準日：2026-09-29

> **位置づけ：調査・設計案。** SSOT の改定ではない。確定済みの範囲は [3.1・3.2](../spec/ssot/03-map-and-scale.md)。イベントの効果値・発火条件・表示名は暫定で、オーナー確認前の下書き。機械可読な同じ内容は [`sengoku-events.json`](../spec/data/sengoku-events.json)（59件）。

## 1. 目的と前提

- SSOT 3.2「鉄砲伝来・キリスト教の伝来・検地のような史実の出来事は、史実の年に出来事として起きる。先に関係する研究を終えた勢力は、その年より早く使える。特定の人物・家に結びつく名前は、その家以外では一般的な名前で表示する」と、同節のオーナー判断（家名の切り替え、室町幕府の終わり、関ヶ原を入れない、太閤検地の表示、月まで合わせる）に沿って、戦国パック（1534〜1600年）の候補イベントを洗い出した。
- 勢力IDは [`japan-1534-start.json`](../spec/data/japan-1534-start.json) と `tools/japan_map_scale/generate.py` の22勢力（`ALL` は全勢力）。研究IDは [ツリー設計案](sengoku-tech-civic-tree.md) と `poc/JAC_PoC_SengokuTrees` の既存ID（技術T01〜T32、社会制度C01〜C30）を使い、JSONには完全なID（`TECH_JAC_SENGOKU_*`、`CIVIC_JAC_SENGOKU_*`）を入れた。
- 『影響勢力』は**1534年配分の勢力**で表す。史実で領国が移った後（例：武田領→徳川領）は、その時点の現所有者に効果を適用する。イベントで『城の所有移転の候補』と書いたものは、自動で所有を変えるのではなく、争奪の目標や吸収・降伏イベントの条件例を意味する。
- **月の換算規則**（SSOT 3.2「暦の月までできるだけ合わせる」）：`month` は `date_text` の旧暦の日付を西暦に換算した月。1582年10月4日まではユリウス暦、10月15日以降はグレゴリオ暦で数える（Wikipedia 等の西暦表記と同じ。1582年より前を先発グレゴリオ暦にはしない。例：長尾→上杉の永禄4年閏3月16日はユリウス暦1561年4月30日で `4`。先発グレゴリオ暦なら5月10日）。旧暦の月だけ分かるものはその月の15日で、幅があるものは開始日で換算する。換算で年をまたぐものは `year` も西暦の年にした（例：永禄9年12月の松平→徳川は1567年1月）。月の境目から数日以内のもの（甲州法度之次第・北条の公事赦免令・北野大茶湯）は、宣明暦と天文計算の差（±1〜2日）で月が変わりうる。北野大茶湯は一般に知られた換算（1587年11月1日）に合わせた。旧暦の日付と幅は `date_text` に残した。同一年内の順序に意味を持たせる場合は、月が不明なものを年の中央として扱う。
- 出典は**調査担当がブラウザを使わずにまとめた参照名**（コトバンク・Wikipedia 日本語版の項目名、および当リポジトリで既に引用済みのURL）で、URLの生存確認はしていない。『確度』は史実の年・内容についての目安（high＝複数の辞典・自治体・文化財資料で年月日が確認できるもの、medium＝年や範囲に幅・異説があるもの、low＝年代が概略か、後世の伝承・産業史に依るもの）。実装前に一次資料または自治体・文化庁の資料で再確認する。

## 2. 発火ルールの提案（暫定）

1. **史実年で発火**：`year`（と `month`）に対応するターンで、全勢力（`affected_factions` が `ALL`）または該当勢力にイベントを通知し、効果を適用する。年→ターンの対応は約3ターン/年（[ツリー設計案](sengoku-tech-civic-tree.md)）を目安にし、PoC の実測で決める。
2. **研究による早期解放**：`earlier_by` のいずれかを先に研究した勢力は、史実年より前にそのイベントの効果を使える。鉄砲伝来は `T12`、キリスト教の伝来は `C19`／`C13`、検地は `C20`／`T31` など。史実年になったら、未研究の影響勢力には `boost` の研究へのひらめき／着想だけを与える（SSOT 3.2）。
3. **家名を伴うイベント**：`house_specific: true` のものは、`house` の勢力では固有名で、他の勢力では `generic_name` で表示する。豊臣は勢力に無く、織田家が羽柴→豊臣に切り替わるため、豊臣期の固有名は `ODA` に置く（SSOT 3.2）。太閤検地・刀狩・身分統制令のような一般的な制度名は家固有にしない（太閤検地はすべての家に『太閤検地』と表示。SSOT 3.2）。
4. **家名の切替（rename）**：長尾→上杉、松平→徳川、本能寺の変の後の織田→羽柴→豊臣を同じ仕組みで行う（SSOT 3.1・3.2）。Civilization ID は変えず、表示名を共有状態から切り替える。
5. **資源開発**：`resource` を持つイベントは、`start_year` まで地図に現れない／交易アクセスが無効の資源を有効にする（SSOT 3.2『1534年にまだ開発されていない資源』）。`action` は `reveal`（タイルに出現）、`trade_access`（港の交易アクセス。硝石）、`ownership`（所有移転。資源自体は初期から存在）。
6. **助言ポップアップと Boost**（オーナー判断：和睦などを、ひらめきを与える形で助言役のポップアップから薦める。SSOT 3.1「因縁は…和睦を促すイベント（ひらめきを与え、助言役のポップアップで知らせる）で解消へ導く」）：
   - `boost`：史実の年に、影響勢力（`ALL` は全勢力）のうち、その研究を終えておらず Boost も済んでいない勢力に与えるひらめき（技術）／着想（社会制度）。`earlier_by` のうち1つを選んだ。表示は公式の Boost 画面（`BoostUnlockedPopup.lua` の技術 `ShowTechBoost`、進み具合の表示は122行目。社会制度は140行目の `ShowCivicBoost`）に任せる。済んでいるかは公式パネルと同じ `HasBoostBeenTriggered`／`CanTriggerBoost`（`TechBoostsPanel.lua:91`）で見る。Boost の量は公式の `Boosts.Boost`（`Technologies.xml:457` 以下）と同じ考え方で、閃き40%（ツリー設計案）の暫定値。
   - `advisor_popup`：助言役のポップアップ（公式の `AdvisorPopup.lua:94` `ShowAdvisorPopup`。`LuaEvents.TutorialUIRoot_AdvisorRaise` で開く、521行目）で薦める行動（日英露）。null のイベントは通常の通知と Boost の表示だけ。画面に出すのは人間の勢力だけで、AI には出さない。
   - Boost を付与するゲームプレイ側の関数（`TriggerBoost` など）と、チュートリアル外で助言ポップアップを開けるかは、ローカルの UI ファイルでは確認できていない（要実機確認）。
   - 公式の `AdvisorPopup.lua:25-38, 94` は文字列ではなく `AdvisorItem`（`Message` などを持つ表）を受け取り、`TutorialUIRoot.lua:1565` はチュートリアル項目の `AdvisorInfo` を渡している。実装時に、`advisor_popup` の日英露の文字列からこの表を組み立てる変換を作る。
   - 下の表のとおり、59件のうち55件に Boost、19件に助言ポップアップを割り当てた。`earlier_by` が無い4件（大友二階崩れ・駿河侵攻・朝倉氏の滅亡・武田氏の滅亡）は Boost を与えない。和睦を薦める10件（島津貴久の薩摩統一戦・伊達氏天文の乱・夷狄商舶往還之法度（蠣崎氏とアイヌの交易取り決め）・三河一向一揆・石山合戦（石山本願寺と織田の戦い）・上杉謙信の死去と御館の乱・四国平定・九州平定・秀吉の琉球への服属要求と島津の仲介・小田原征伐と北条氏の滅亡）は、1534年の因縁の一覧（[1534年開始データ](japan-1534-start.md)の「当日の同盟・不仲」。関係の一覧は未作成）ができたら、因縁を解消するイベントとして対応づける。

| ID | Boost | 助言ポップアップで薦めること |
|---|---|---|
| `EV_JINKAISHU` | C05 | — |
| `EV_SHIMAZU_UNIFY_1` | C02 | 分家と和睦し、薩摩の国人と盟約を結びましょう。 |
| `EV_TENBUN_FAMINE` | T07 | 兵糧蔵を建てて、飢饉に備えましょう。 |
| `EV_IKUNO_SILVER` | T06 | — |
| `EV_TENBUN_DATE` | C02 | 父子の争いを和睦で収め、蘆名との仲を立て直しましょう。 |
| `EV_TEPPO` | T12 | — |
| `EV_KUNITOMO_GUN` | T13 | — |
| `EV_KOSHU_HATTO` | C05 | — |
| `EV_EGUCHI` | C16 | 将軍家へ使節を送り、畿内の情勢を見極めましょう。 |
| `EV_CHRISTIAN` | C19 | — |
| `EV_OTOMO_NIKAI` | — | — |
| `EV_HOJO_TAX` | C20 | — |
| `EV_AINU_TRADE_ACCORD` | C11 | 交易の取り決めを結び、和睦を保ちましょう。 |
| `EV_HIRADO_PORTUGUESE` | C13 | — |
| `EV_OUCHI_DAINEIJI` | C02 | 離反しそうな家臣・国人と盟約を結び直しましょう。 |
| `EV_KANTO_KANREI` | C16 | 上杉憲政を保護し、関東管領職の継承を目指しましょう。 |
| `EV_KAWANAKAJIMA` | T16 | 北信濃の城の守りを固め、長い戦いに備えましょう。 |
| `EV_SANGOKU_DOMEI` | C11 | 武田・北条・今川の三家で同盟を結び、背後を固めましょう。 |
| `EV_ITSUKUSHIMA` | T10 | — |
| `EV_OKEHAZAMA` | C10 | — |
| `EV_KOSHU_GOLD` | T14 | — |
| `EV_NAGAO_TO_UESUGI` | C16 | — |
| `EV_MORI_IWAMI` | T09 | — |
| `EV_MIKAWA_IKKI` | C08 | 門徒と和睦し、寺内町の自治を認めましょう。 |
| `EV_AMAGO_FALL` | T19 | — |
| `EV_MATSUDAIRA_TOKUGAWA` | C16 | — |
| `EV_RAKUICHI` | C17 | — |
| `EV_JORAKU` | C16 | — |
| `EV_IMAGAWA_FALL` | — | — |
| `EV_ISHIYAMA` | C18 | 本願寺と和睦し、寺社領を安堵しましょう。 |
| `EV_NAGASAKI` | C19 | — |
| `EV_MUROMACHI_END` | C16 | — |
| `EV_ASAKURA_FALL` | — | — |
| `EV_NAGASHINO` | C21 | — |
| `EV_SHIMANTO` | C10 | — |
| `EV_AZUCHI` | T21 | — |
| `EV_KENSHIN_DEATH` | C11 | 後継争いを和睦で収め、北条・武田との同盟を探りましょう。 |
| `EV_TAKEDA_FALL` | — | — |
| `EV_HONNOJI` | C16 | 朝廷・将軍への取次で、後継の正統性を固めましょう。 |
| `EV_YAMAZAKI_KENCHI` | C20 | — |
| `EV_OSAKA` | T29 | — |
| `EV_SHIKOKU` | C10 | 天下人との和睦で、本領を守るか考えましょう。 |
| `EV_TENSHO_QUAKE` | T18 | 被災した城を修理し、湯屋と兵糧蔵で民を救いましょう。 |
| `EV_KYUSHU` | T19 | 天下人と和睦し、本領の安堵を求めましょう。 |
| `EV_BATEREN` | C18 | — |
| `EV_KITANO_TEA` | C14 | — |
| `EV_KATANAGARI` | C27 | — |
| `EV_RYUKYU_SHIMAZU_DEMAND` | C11 | 島津を通じて使節を送り、和睦を保ちましょう。 |
| `EV_SURIAGEHARA` | C10 | — |
| `EV_SADO_SUBJUGATION` | T14 | — |
| `EV_ODAWARA` | T32 | 包囲を受ける前に、天下人との和睦を考えましょう。 |
| `EV_MATSUDAIRA_KANTO` | C15 | — |
| `EV_AIZU_LACQUER` | C15 | — |
| `EV_SHINBUN_SEIDO` | C28 | — |
| `EV_BUNROKU` | T24 | — |
| `EV_EZO_MATSUMAE` | C11 | 蠣崎氏との交易の取り決めを結び直しましょう。 |
| `EV_AIKAWA` | T09 | — |
| `EV_SAN_FELIPE` | C18 | — |
| `EV_CHOSOKABE_HYAKKAJO` | C05 | — |


## 3. イベント一覧（年代順）

| 年-月 | ID | 名称（日） | 影響勢力 | 早期解放の研究 | 種別 | 家固有 | 確度 |
|---|---|---|---|---|---|---|---|
| 1536-05 | `EV_JINKAISHU` | 塵芥集の制定（伊達の分国法） | 伊達 | C05 | 制度 | ○ DATE → 分国法の制定 | high |
| 1539〜1554 | `EV_SHIMAZU_UNIFY_1` | 島津貴久の薩摩統一戦 | 島津 | C02、C10 | 継承・内紛 |  | medium |
| 1540 | `EV_TENBUN_FAMINE` | 天文の大飢饉 | 全勢力 | T07、T01 | 災害 |  | medium |
| 1542 | `EV_IKUNO_SILVER` | 生野銀山の本格開発 | 足利将軍家、尼子 | T06、T14、T09 | 資源開発 |  | medium |
| 1542-07〜1548 | `EV_TENBUN_DATE` | 伊達氏天文の乱 | 伊達、蘆名 | C02 | 継承・内紛 |  | high |
| 1543-09 | `EV_TEPPO` | 鉄砲伝来 | 全勢力 | T12 | 技術伝播 |  | high |
| 1544 | `EV_KUNITOMO_GUN` | 国友・堺・根来の鉄砲鍛冶の成立 | 六角、足利将軍家、三好、本願寺 | T13 | 技術伝播 |  | medium |
| 1547-07 | `EV_KOSHU_HATTO` | 甲州法度之次第の制定 | 武田 | C05 | 制度 | ○ TAKEDA → 分国法の制定 | high |
| 1549-07 | `EV_EGUCHI` | 江口の戦いと三好長慶の畿内進出 | 三好、足利将軍家 | C16 | 戦役 |  | high |
| 1549-08 | `EV_CHRISTIAN` | キリスト教の伝来 | 全勢力 | C19、C13 | 宗教 |  | high |
| 1550-03 | `EV_OTOMO_NIKAI` | 大友二階崩れの変 | 大友 | — | 継承・内紛 |  | high |
| 1550-05 | `EV_HOJO_TAX` | 北条氏康の税制改革（公事赦免令） | 北条 | C20、C15 | 制度 |  | medium |
| 1550 | `EV_AINU_TRADE_ACCORD` | 夷狄商舶往還之法度（蠣崎氏とアイヌの交易取り決め） | アイヌ | C11、C13 | 外交 |  | low |
| 1550 | `EV_HIRADO_PORTUGUESE` | 平戸へのポルトガル船来航と南蛮貿易 | 大友、島津 | C19、C13、T17 | 貿易 |  | medium |
| 1551-09 | `EV_OUCHI_DAINEIJI` | 大寧寺の変 | 大内、毛利 | C02 | 継承・内紛 |  | high |
| 1552 | `EV_KANTO_KANREI` | 上杉憲政の越後亡命 | 山内上杉、長尾、北条 | C16 | 継承・内紛 |  | high |
| 1553〜1564 | `EV_KAWANAKAJIMA` | 川中島の戦い | 武田、長尾 | T16、C10 | 戦役 |  | high |
| 1554 | `EV_SANGOKU_DOMEI` | 甲相駿三国同盟 | 武田、北条、今川 | C11 | 外交 |  | high |
| 1555-10 | `EV_ITSUKUSHIMA` | 厳島の戦い | 毛利、大内 | T10、T20、T17 | 戦役 |  | high |
| 1560-06 | `EV_OKEHAZAMA` | 桶狭間の戦い | 今川、織田、松平 | T26、C10 | 戦役 |  | high |
| 1560 | `EV_KOSHU_GOLD` | 甲州金の整備と黒川金山・中山金山の稼働 | 武田、北条 | T14、T06 | 資源開発 |  | low |
| 1561-04 | `EV_NAGAO_TO_UESUGI` | 長尾から上杉への継承（関東管領就任） | 長尾、山内上杉、北条、武田 | C16、C11、C30 | 家名・体制 |  | high |
| 1562 | `EV_MORI_IWAMI` | 毛利氏の石見銀山掌握 | 毛利、大内、尼子 | T06、T09、T14 | 資源開発 |  | high |
| 1563〜1564 | `EV_MIKAWA_IKKI` | 三河一向一揆 | 松平、本願寺 | C08、C18 | 宗教 |  | high |
| 1566-12 | `EV_AMAGO_FALL` | 月山富田城の開城と尼子氏の滅亡 | 尼子、毛利 | T16、T21、T19 | 戦役 |  | high |
| 1567-01 | `EV_MATSUDAIRA_TOKUGAWA` | 松平から徳川への改姓 | 松平 | C16 | 家名・体制 |  | high |
| 1567-11 | `EV_RAKUICHI` | 楽市楽座（加納・安土） | 織田、六角、足利将軍家 | C17 | 制度 |  | high |
| 1568-10 | `EV_JORAKU` | 織田信長の上洛と足利義昭の将軍就任 | 織田、足利将軍家、六角、三好 | C16、C11 | 戦役 |  | high |
| 1569-01 | `EV_IMAGAWA_FALL` | 駿河侵攻と今川氏の滅亡 | 今川、武田、松平、北条 | — | 戦役 |  | high |
| 1570-10〜1580 | `EV_ISHIYAMA` | 石山合戦（石山本願寺と織田の戦い） | 本願寺、織田、毛利、松平 | C08、C18、T21 | 戦役 |  | high |
| 1571 | `EV_NAGASAKI` | 長崎の開港 | 大友、島津 | C13、C19、C29 | 貿易 |  | high |
| 1573-08 | `EV_MUROMACHI_END` | 室町幕府の終焉 | 足利将軍家 | C16 | 家名・体制 |  | high |
| 1573-09 | `EV_ASAKURA_FALL` | 朝倉氏の滅亡 | 朝倉、織田 | — | 戦役 |  | high |
| 1575-06 | `EV_NAGASHINO` | 長篠の戦い | 織田、松平、武田 | T22、T23、C21 | 戦役 |  | high |
| 1575-08 | `EV_SHIMANTO` | 長宗我部元親の土佐統一（四万十川の戦い） | 長宗我部 | C02、C10、T10 | 戦役 |  | high |
| 1576-02 | `EV_AZUCHI` | 安土城の築城 | 織田、六角 | T29、T21、T25 | 築城・土木 | ○ ODA → 大規模城郭の築城 | high |
| 1578-04〜1579 | `EV_KENSHIN_DEATH` | 上杉謙信の死去と御館の乱 | 長尾、北条、武田 | C02、C11 | 継承・内紛 |  | high |
| 1582-04 | `EV_TAKEDA_FALL` | 武田氏の滅亡（天目山） | 武田、織田、松平、北条 | — | 戦役 |  | high |
| 1582-06 | `EV_HONNOJI` | 本能寺の変 | 織田、松平、毛利 | C16、C11 | 継承・内紛 |  | high |
| 1582-07 | `EV_YAMAZAKI_KENCHI` | 山崎検地から太閤検地へ | 全勢力 | C20、C25、T31 | 制度 |  | high |
| 1583-10 | `EV_OSAKA` | 大坂城の築城 | 本願寺、織田、三好 | T29、T21、T25、T32 | 築城・土木 | ○ ODA（豊臣） → 巨大城郭の築城 | high |
| 1585-07 | `EV_SHIKOKU` | 四国平定 | 長宗我部、三好、足利将軍家 | T20、T10、C10 | 戦役 |  | high |
| 1586-01 | `EV_TENSHO_QUAKE` | 天正地震 | 朝倉、六角、織田、足利将軍家 | T18、T07 | 災害 |  | high |
| 1587-01 | `EV_KYUSHU` | 九州平定 | 島津、大友 | T22、T19、T20 | 戦役 |  | high |
| 1587-07 | `EV_BATEREN` | バテレン追放令 | 全勢力 | C18、C29 | 宗教 |  | high |
| 1587-11 | `EV_KITANO_TEA` | 北野大茶湯 | 全勢力 | C14、C22 | 制度 | ○ ODA（豊臣） → 大茶会 | high |
| 1588-08 | `EV_KATANAGARI` | 刀狩令 | 全勢力 | C27、C28 | 制度 |  | high |
| 1589〜1592 | `EV_RYUKYU_SHIMAZU_DEMAND` | 秀吉の琉球への服属要求と島津の仲介 | 琉球王国、島津 | C11、C13、C16 | 外交 |  | medium |
| 1589-07 | `EV_SURIAGEHARA` | 摺上原の戦い（蘆名氏の滅亡） | 蘆名、伊達 | T22、C10 | 戦役 |  | high |
| 1589-08 | `EV_SADO_SUBJUGATION` | 上杉景勝の佐渡平定と西三川砂金 | 長尾 | T14、T06 | 資源開発 |  | medium |
| 1590-05 | `EV_ODAWARA` | 小田原征伐と北条氏の滅亡 | 北条、伊達、松平、山内上杉 | T32、T21 | 戦役 |  | high |
| 1590-08 | `EV_MATSUDAIRA_KANTO` | 徳川家康の関東移封 | 松平、北条 | C15、C30 | 継承・内紛 |  | high |
| 1590-09 | `EV_AIZU_LACQUER` | 蒲生氏郷の会津入封と漆器産業 | 伊達、蘆名 | C15、C13 | 資源開発 |  | medium |
| 1591-10 | `EV_SHINBUN_SEIDO` | 身分統制令（兵農分離） | 全勢力 | C28、C27、C25 | 制度 |  | high |
| 1592-05〜1598 | `EV_BUNROKU` | 文禄・慶長の役（朝鮮出兵） | 島津、毛利、長宗我部、大友、琉球王国 | T19、T20、T24、C23、C29 | 戦役 |  | high |
| 1593 | `EV_EZO_MATSUMAE` | 蠣崎慶広への朱印状とアイヌ交易の変質 | アイヌ | C26、C11 | 外交 |  | medium |
| 1596 | `EV_AIKAWA` | 相川金銀山の発見 | 長尾 | T14、T06、T09 | 資源開発 |  | medium |
| 1596-10 | `EV_SAN_FELIPE` | サン＝フェリペ号事件と二十六聖人の殉教 | 長宗我部、大友、島津 | C18、C19 | 宗教 |  | high |
| 1597-01 | `EV_CHOSOKABE_HYAKKAJO` | 長宗我部氏掟書（百箇条） | 長宗我部 | C05 | 制度 | ○ CHOSOKABE → 分国法の制定 | high |

## 4. イベントごとの詳細

### 塵芥集の制定（伊達の分国法） `EV_JINKAISHU`

- 名称：塵芥集の制定（伊達の分国法） ／ Jinkaishū Domain Code (Date) ／ Кодекс Дзинкайсю (закон владения Датэ)
- 史実：1536-05（天文5年4月）。地域：陸奥、出羽
- 影響勢力：伊達（中心：伊達）
- 早期解放の研究：C05（`CIVIC_JAC_SENGOKU_DOMAIN_LAW`）
- 家固有：`DATE` の名。他の勢力では『分国法の制定／Domain Law Enacted／Принятие законов владения』と表示
- 史実の年のBoost：C05（`CIVIC_JAC_SENGOKU_DOMAIN_LAW`）。助言ポップアップ：なし
- 案（暫定）：分国法の制定。領内の治安・訴訟が安定し、城下の忠誠心が上がる。他家では『分国法の制定』として個別に起こせる。
- 出典：コトバンク『塵芥集』；Wikipedia日本語版『塵芥集』（確度：high）

### 島津貴久の薩摩統一戦 `EV_SHIMAZU_UNIFY_1`

- 名称：島津貴久の薩摩統一戦 ／ Shimazu Takahisa Unifies Satsuma ／ Объединение Сацума под началом Симадзу Такахиса
- 史実：1539〜1554（天文8年ごろ〜天文23年（1554）岩剣城の戦い）。地域：薩摩、大隅
- 影響勢力：島津（中心：島津）
- 早期解放の研究：C02（`CIVIC_JAC_SENGOKU_KOKUJIN_LEAGUES`）、C10（`CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER`）
- 史実の年のBoost：C02（`CIVIC_JAC_SENGOKU_KOKUJIN_LEAGUES`）。助言ポップアップ：『分家と和睦し、薩摩の国人と盟約を結びましょう。』
- 案（暫定）：島津宗家と分家（薩州家・相州家等）の対立を、貴久が加世田・別府・国分等の攻略で収める。SSOT 3.1の『宗家と分家の対立は内紛イベント』の中核。内紛中は島津の戦力が分散、完了後に薩摩の忠誠心が上がる。
- 出典：コトバンク『島津貴久』；Wikipedia日本語版『島津貴久』（確度：medium）
- 補足：当主は SSOT 3.1 のとおり島津貴久（`japan-1534-start.json` は史実の名目上の当主・島津勝久のままで、SSOT に合わせる修正が別途必要）。

### 天文の大飢饉 `EV_TENBUN_FAMINE`

- 名称：天文の大飢饉 ／ Great Tenbun Famine ／ Великий голод годов Тэмбун
- 史実：1540（天文9年（前後の風水害・疫病を含む））。地域：全国
- 影響勢力：全勢力（中心：全勢力）
- 早期解放の研究：T07（`TECH_JAC_SENGOKU_GRAIN_STORES`）、T01（`TECH_JAC_SENGOKU_IRRIGATION_WORKS`）
- 史実の年のBoost：T07（`TECH_JAC_SENGOKU_GRAIN_STORES`）。助言ポップアップ：『兵糧蔵を建てて、飢饉に備えましょう。』
- 案（暫定）：食料の全国的な不作。研究済みの兵糧保存（T07）・用水普請（T01）で被害を軽減。未研究の家は食料と人口成長が数ターン低下（暫定）。
- 出典：コトバンク『天文の飢饉』；Wikipedia日本語版『天文の飢饉』（確度：medium）

### 生野銀山の本格開発 `EV_IKUNO_SILVER`

- 名称：生野銀山の本格開発 ／ Ikuno Silver Mine Opened ／ Начало разработки серебряного рудника Икуно
- 史実：1542（天文11年（山名祐豊の開発と伝わる））。地域：但馬
- 影響勢力：足利将軍家、尼子（中心：足利将軍家）
- 早期解放の研究：T06（`TECH_JAC_SENGOKU_ORE_PROSPECTING`）、T14（`TECH_JAC_SENGOKU_MINE_MANAGEMENT`）、T09（`TECH_JAC_SENGOKU_CUPELLATION`）
- 史実の年のBoost：T06（`TECH_JAC_SENGOKU_ORE_PROSPECTING`）。助言ポップアップ：なし
- 案（暫定）：但馬・生野に銀山が出現。銀（RESOURCE_SILVER）のタイルを地図に追加し、鉱山の建設を解放。T09（灰吹法）を最初に研究した勢力が出れば、年より早く地図に現れる（資源配置と同じ）。
- 資源：`RESOURCE_SILVER`、生野・但馬 (35.167, 134.790)、`reveal`、1542年〜
- 出典：documents/research/japan-map-resources.md（資源の代表地点と年代）；Wikipedia日本語版『生野銀山』（確度：medium）
- 補足：但馬は1534年配分で足利将軍家領。取得先の勢力が変わっていれば現所有者が利益を得る。

### 伊達氏天文の乱 `EV_TENBUN_DATE`

- 名称：伊達氏天文の乱 ／ Date Tenbun Civil War ／ Междоусобица Датэ годов Тэмбун
- 史実：1542-07〜1548（天文11年6月〜天文17年（1548））。地域：陸奥、出羽
- 影響勢力：伊達、蘆名（中心：伊達）
- 早期解放の研究：C02（`CIVIC_JAC_SENGOKU_KOKUJIN_LEAGUES`）
- 史実の年のBoost：C02（`CIVIC_JAC_SENGOKU_KOKUJIN_LEAGUES`）。助言ポップアップ：『父子の争いを和睦で収め、蘆名との仲を立て直しましょう。』
- 案（暫定）：伊達稙宗と晴宗の父子対立が南奥全体へ広がる。伊達と蘆名の外交状態が揺れ、1548年の和睦で当主が晴宗へ交代。内紛中は伊達の戦力・生産が低下（暫定）。
- 出典：コトバンク『伊達氏天文の乱』；Wikipedia日本語版『天文の乱 (伊達氏)』（確度：high）

### 鉄砲伝来 `EV_TEPPO`

- 名称：鉄砲伝来 ／ Introduction of Firearms (Tanegashima) ／ Появление огнестрельного оружия (Танэгасима)
- 史実：1543-09（天文12年8月25日（『鉄炮記』）。1542年説もある）。地域：薩摩、大隅、全国
- 影響勢力：全勢力（中心：島津）
- 早期解放の研究：T12（`TECH_JAC_SENGOKU_FIREARMS_CONTACT`）
- 史実の年のBoost：T12（`TECH_JAC_SENGOKU_FIREARMS_CONTACT`）。助言ポップアップ：なし
- 案（暫定）：種子島（島津領）に火縄銃が伝わる。伝来年に全勢力が鉄砲の入手・試射（T12）を解放される。先にT12を研究した勢力はそれ以前に輸入火縄銃を使える。国内量産（T13）は別イベント『鉄砲鍛冶の成立』。
- 出典：[国立歴史民俗博物館 鉄炮展示](https://archive.rekihaku.ac.jp/exhibitions/project/old/061003/index.html)；コトバンク『鉄砲伝来』；Wikipedia日本語版『鉄砲伝来』（確度：high）
- 補足：複線的な伝播（倭寇船・西国諸港）を考え、島津以外は伝来年に一斉に得る。T12の前提はT03。

### 国友・堺・根来の鉄砲鍛冶の成立 `EV_KUNITOMO_GUN`

- 名称：国友・堺・根来の鉄砲鍛冶の成立 ／ Firearm Smiths Emerge (Kunitomo, Sakai, Negoro) ／ Появление оружейников (Кунитомо, Сакай, Негоро)
- 史実：1544（天文13年ごろ（将軍が国友に鉄砲製作を命じたとされる））。地域：近江、和泉、紀伊
- 影響勢力：六角、足利将軍家、三好、本願寺（中心：六角、三好）
- 早期解放の研究：T13（`TECH_JAC_SENGOKU_MATCHLOCK_CRAFT`）
- 史実の年のBoost：T13（`TECH_JAC_SENGOKU_MATCHLOCK_CRAFT`）。助言ポップアップ：なし
- 案（暫定）：近江・国友、和泉・堺、紀伊・根来で鉄砲の国内生産が始まる。該当地域の所有者に鉄砲鍛冶場（T13）の建設を解放し、他家は交易・調略で技術を得る。
- 出典：[国立歴史民俗博物館 鉄炮展示](https://archive.rekihaku.ac.jp/exhibitions/project/old/061003/index.html)；コトバンク『国友鉄砲鍛冶』；documents/research/sengoku-tech-civic-tree.md（史実メモ）（確度：medium）

### 甲州法度之次第の制定 `EV_KOSHU_HATTO`

- 名称：甲州法度之次第の制定 ／ Kōshū Hatto no Shidai Enacted ／ Принятие «Косю хатто-но сидай»
- 史実：1547-07（天文16年6月）。地域：甲斐
- 影響勢力：武田（中心：武田）
- 早期解放の研究：C05（`CIVIC_JAC_SENGOKU_DOMAIN_LAW`）
- 家固有：`TAKEDA` の名。他の勢力では『分国法の制定／Domain Law Enacted／Принятие законов владения』と表示
- 史実の年のBoost：C05（`CIVIC_JAC_SENGOKU_DOMAIN_LAW`）。助言ポップアップ：なし
- 案（暫定）：武田晴信が分国法を制定。領内の忠誠・訴訟が安定する。C05を研究した他家は同名の別法を制定できる。
- 出典：コトバンク『甲州法度之次第』；Wikipedia日本語版『甲州法度之次第』（確度：high）

### 江口の戦いと三好長慶の畿内進出 `EV_EGUCHI`

- 名称：江口の戦いと三好長慶の畿内進出 ／ Battle of Eguchi and Miyoshi Nagayoshi's Rise ／ Битва при Эгути и возвышение Миёси Нагаёси
- 史実：1549-07（天文18年6月24日）。地域：摂津、山城、河内
- 影響勢力：三好、足利将軍家（中心：三好、足利将軍家）
- 早期解放の研究：C16（`CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS`）
- 史実の年のBoost：C16（`CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS`）。助言ポップアップ：『将軍家へ使節を送り、畿内の情勢を見極めましょう。』
- 案（暫定）：三好長慶が細川晴元・三好政長を破り、将軍家を追って京都・摂津・山城を掌握する（1553年に義輝を京都から追う）。畿内の城の所有移転候補と、足利将軍家の求心力低下。
- 出典：コトバンク『三好長慶』；Wikipedia日本語版『江口の戦い』（確度：high）

### キリスト教の伝来 `EV_CHRISTIAN`

- 名称：キリスト教の伝来 ／ Arrival of Christianity ／ Появление христианства
- 史実：1549-08（天文18年7月22日（1549年8月15日）鹿児島上陸）。地域：薩摩、周防、豊後、全国
- 影響勢力：全勢力（中心：島津、大内、大友）
- 早期解放の研究：C19（`CIVIC_JAC_SENGOKU_NANBAN_ENVOYS`）、C13（`CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS`）
- 史実の年のBoost：C19（`CIVIC_JAC_SENGOKU_NANBAN_ENVOYS`）。助言ポップアップ：なし
- 案（暫定）：ザビエルが鹿児島に上陸。宗教『カトリック』（SSOT 9.2）が伝来年に登場し、島津→大内（山口）→大友（府内）→畿内の順で布教の窓が開く。C19／C13を先に研究した勢力は年より早く宣教師と接触。信徒・地下化・抵抗の規則は9章に従う。
- 出典：コトバンク『キリスト教伝来』；Wikipedia日本語版『フランシスコ・ザビエル』（確度：high）
- 補足：SSOT 9.2『カトリック（1549年以降のイベントで登場）』に対応。

### 大友二階崩れの変 `EV_OTOMO_NIKAI`

- 名称：大友二階崩れの変 ／ Otomo Nikai-kuzure Incident ／ Инцидент Никай-кудзурэ в доме Отомо
- 史実：1550-03（天文19年2月）。地域：豊後
- 影響勢力：大友（中心：大友）
- 案（暫定）：大友義鑑が家臣に襲われ死去し、義鎮（宗麟）が家督を継ぐ。宗麟のキリスト教保護（1551年）への前提。当主交代と一時的な忠誠低下。
- 出典：コトバンク『大友義鎮』；Wikipedia日本語版『二階崩れの変』（確度：high）

### 北条氏康の税制改革（公事赦免令） `EV_HOJO_TAX`

- 名称：北条氏康の税制改革（公事赦免令） ／ Hojo Ujiyasu's Tax Reform ／ Налоговая реформа Ходзё Удзияса
- 史実：1550-05（天文19年4月（『公事赦免令』）。後北条氏の検地全体は年代が広い）。地域：相模、武蔵
- 影響勢力：北条（中心：北条）
- 早期解放の研究：C20（`CIVIC_JAC_SENGOKU_DOMAIN_LAND_SURVEYS`）、C15（`CIVIC_JAC_SENGOKU_CASTLE_TOWN_ORDINANCES`）
- 史実の年のBoost：C20（`CIVIC_JAC_SENGOKU_DOMAIN_LAND_SURVEYS`）。助言ポップアップ：なし
- 案（暫定）：北条氏康が検地・貫高制に基づく課税と、飢饉時の減免を整える。領国検地（C20）の効果と忠誠を上げる。他家はC20の研究で同種の施策が可能。
- 出典：[小田原デジタルミュージアム 北条氏の検地・貫高制](https://odawara-digital-museum.jp/selection/hojo/24/)；コトバンク『北条氏康』（確度：medium）

### 夷狄商舶往還之法度（蠣崎氏とアイヌの交易取り決め） `EV_AINU_TRADE_ACCORD`

- 名称：夷狄商舶往還之法度（蠣崎氏とアイヌの交易取り決め） ／ Trade Accord Between the Kakizaki and the Ainu ／ Торговое соглашение между Какидзаки и айнами
- 史実：1550（天文19〜20年（1550〜51）ごろ。年・内容は史料紹介ごとに差がある）。地域：蝦夷島（道南）
- 影響勢力：アイヌ（中心：アイヌ）
- 早期解放の研究：C11（`CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES`）、C13（`CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS`）
- 史実の年のBoost：C11（`CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES`）。助言ポップアップ：『交易の取り決めを結び、和睦を保ちましょう。』
- 案（暫定）：松前（蠣崎）の和人とアイヌ首長の交易取り決めが成立。アイヌが東西の首長を通じて交易を得る。SSOT 3.1のアイヌ指導者（タリコナ／全域代表は仮設定）に対応する外交イベント。
- 出典：コトバンク『蠣崎季広』；Wikipedia日本語版『蠣崎季広』（確度：low）

### 平戸へのポルトガル船来航と南蛮貿易 `EV_HIRADO_PORTUGUESE`

- 名称：平戸へのポルトガル船来航と南蛮貿易 ／ Portuguese Ships at Hirado (Nanban Trade) ／ Португальские корабли в Хирадо (торговля намбан)
- 史実：1550（天文19年（1550）にポルトガル船が初入港と伝わる）。地域：肥前、豊後、薩摩
- 影響勢力：大友、島津（中心：大友）
- 早期解放の研究：C19（`CIVIC_JAC_SENGOKU_NANBAN_ENVOYS`）、C13（`CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS`）、T17（`TECH_JAC_SENGOKU_HARBOR_PILOTS`）
- 史実の年のBoost：C13（`CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS`）。助言ポップアップ：なし
- 案（暫定）：肥前・平戸が南蛮貿易の港になる（1550年は通知と Boost だけ）。硝石（RESOURCE_NITER）の交易アクセスは、資源配置の開始年（1586年）から平戸の港タイルで有効になる。先にC13／C19を研究した勢力は他の港（薩摩・豊後）でも交易可。
- 資源：`RESOURCE_NITER`、平戸・肥前 (33.368, 129.553)、`trade_access`、1586年〜
- 出典：documents/research/japan-map-resources.md（資源の代表地点と年代）；Wikipedia日本語版『平戸』；Wikipedia日本語版『南蛮貿易』（確度：medium）
- 補足：japan-map-resources.md: 硝石は鉱床ではなく港の交易アクセス。平戸での開始年は、同資料が確認した1586年の島津家久の玉薬購入記録に合わせた。1550年のポルトガル船来航から有効にするかは Q55。

### 大寧寺の変 `EV_OUCHI_DAINEIJI`

- 名称：大寧寺の変 ／ Daineiji Incident ／ Инцидент Дайнэйдзи
- 史実：1551-09（天文20年8月28日）。地域：周防、長門、石見
- 影響勢力：大内、毛利（中心：大内）
- 早期解放の研究：C02（`CIVIC_JAC_SENGOKU_KOKUJIN_LEAGUES`）
- 史実の年のBoost：C02（`CIVIC_JAC_SENGOKU_KOKUJIN_LEAGUES`）。助言ポップアップ：『離反しそうな家臣・国人と盟約を結び直しましょう。』
- 案（暫定）：陶隆房（晴賢）が大内義隆を自害に追い込む。大内の当主・忠誠が動揺し、石見・周防・長門の城が家臣寄りに動く。毛利が独立性を高める契機。
- 出典：コトバンク『大内義隆』；Wikipedia日本語版『大寧寺の変』（確度：high）

### 上杉憲政の越後亡命 `EV_KANTO_KANREI`

- 名称：上杉憲政の越後亡命 ／ Uesugi Norimasa Flees to Echigo ／ Бегство Уэсуги Норимаса в Этиго
- 史実：1552（天文21年（1552）ごろ。1546年の河越夜戦後の凋落）。地域：上野、越後
- 影響勢力：山内上杉、長尾、北条（中心：山内上杉、長尾）
- 早期解放の研究：C16（`CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS`）
- 史実の年のBoost：C16（`CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS`）。助言ポップアップ：『上杉憲政を保護し、関東管領職の継承を目指しましょう。』
- 案（暫定）：山内上杉が北条氏康に追われ、長尾景虎を頼る（関東管領職の継承が条件）。後の長尾→上杉継承イベントの前提。山内上杉の上野の城は北条・武田・長尾のいずれかの取得候補。
- 出典：コトバンク『上杉憲政』；Wikipedia日本語版『上杉憲政』（確度：high）

### 川中島の戦い `EV_KAWANAKAJIMA`

- 名称：川中島の戦い ／ Battles of Kawanakajima ／ Битвы при Каванакадзима
- 史実：1553〜1564（天文22年〜永禄7年（1553〜1564））。地域：信濃、越後
- 影響勢力：武田、長尾（中心：武田、長尾）
- 早期解放の研究：T16（`TECH_JAC_SENGOKU_RIDGE_DITCHES`）、C10（`CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER`）
- 史実の年のBoost：T16（`TECH_JAC_SENGOKU_RIDGE_DITCHES`）。助言ポップアップ：『北信濃の城の守りを固め、長い戦いに備えましょう。』
- 案（暫定）：北信濃をめぐる武田と長尾の長期対立（1553〜1564年の5回）。最大の第四次は1561年9月。該当年に両家の外交状態を敵対に固定し、北信濃の城の獲得を争う。
- 出典：コトバンク『川中島の戦い』；Wikipedia日本語版『川中島の戦い』（確度：high）
- 補足：第四次の戦い1561-10は長尾が上杉を名乗った直後にあたる。

### 甲相駿三国同盟 `EV_SANGOKU_DOMEI`

- 名称：甲相駿三国同盟 ／ Three-Way Alliance of Kai, Sagami, and Suruga ／ Тройственный союз Кай–Сагами–Суруга
- 史実：1554（天文23年（1554））。地域：甲斐、相模、駿河
- 影響勢力：武田、北条、今川（中心：武田、北条、今川）
- 早期解放の研究：C11（`CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES`）
- 史実の年のBoost：C11（`CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES`）。助言ポップアップ：『武田・北条・今川の三家で同盟を結び、背後を固めましょう。』
- 案（暫定）：3家の婚姻を伴う同盟。3家間の外交状態を同盟にし、背後の安全から各家が他方面（長尾・三河・関東）へ戦力を向けられる。破棄は1568年12月。
- 出典：コトバンク『甲相駿三国同盟』；Wikipedia日本語版『甲相駿三国同盟』（確度：high）

### 厳島の戦い `EV_ITSUKUSHIMA`

- 名称：厳島の戦い ／ Battle of Itsukushima ／ Битва при Ицукусима
- 史実：1555-10（天文24年10月1日）。地域：安芸、周防
- 影響勢力：毛利、大内（中心：毛利、大内）
- 早期解放の研究：T10（`TECH_JAC_SENGOKU_WOODEN_SHIPBUILDING`）、T20（`TECH_JAC_SENGOKU_WARSHIP_CONSTRUCTION`）、T17（`TECH_JAC_SENGOKU_HARBOR_PILOTS`）
- 史実の年のBoost：T10（`TECH_JAC_SENGOKU_WOODEN_SHIPBUILDING`）。助言ポップアップ：なし
- 案（暫定）：毛利元就が陶晴賢軍を奇襲して破る。毛利の大勢力化の転機。水軍（T10／T20／T17）を先に研究した勢力は瀬戸内の島や港を活用できる。
- 出典：コトバンク『厳島の戦い』；Wikipedia日本語版『厳島の戦い』（確度：high）

### 桶狭間の戦い `EV_OKEHAZAMA`

- 名称：桶狭間の戦い ／ Battle of Okehazama ／ Битва при Окэхадзама
- 史実：1560-06（永禄3年5月19日）。地域：尾張、三河
- 影響勢力：今川、織田、松平（中心：今川、織田）
- 早期解放の研究：T26（`TECH_JAC_SENGOKU_FIELD_FORTS`）、C10（`CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER`）
- 史実の年のBoost：C10（`CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER`）。助言ポップアップ：なし
- 案（暫定）：織田信長が今川義元を討つ。今川の当主が氏真へ替わり、松平（元康）が今川から離れる契機。今川の求心力低下と織田の躍進。
- 出典：コトバンク『桶狭間の戦い』；Wikipedia日本語版『桶狭間の戦い』（確度：high）

### 甲州金の整備と黒川金山・中山金山の稼働 `EV_KOSHU_GOLD`

- 名称：甲州金の整備と黒川金山・中山金山の稼働 ／ Kōshū Gold Coinage; Kurokawa and Nakayama Gold Mines ／ Чеканка золота Косю; золотые рудники Куроками и Накаяма
- 史実：1560（16世紀中ごろ（黒川は15世紀末〜、中山は16世紀前後〜））。地域：甲斐
- 影響勢力：武田、北条（中心：武田）
- 早期解放の研究：T14（`TECH_JAC_SENGOKU_MINE_MANAGEMENT`）、T06（`TECH_JAC_SENGOKU_ORE_PROSPECTING`）
- 史実の年のBoost：T14（`TECH_JAC_SENGOKU_MINE_MANAGEMENT`）。助言ポップアップ：なし
- 案（暫定）：甲斐の金山経営が拡大し、金の収入が増える。黒川・中山の金（RESOURCE_JAC_GOLD案）は資源配置で開始時から地図にある。T14で鉱山経営の産出が増加。
- 出典：documents/research/japan-map-resources.md（資源の代表地点と年代）；Wikipedia日本語版『甲州金』（確度：low）
- 補足：japan-map-resources.mdの金候補（黒川・中山）は15世紀末〜の操業で、資源配置では開始時から置く。このイベントはタイルを追加しない。

### 長尾から上杉への継承（関東管領就任） `EV_NAGAO_TO_UESUGI`

- 名称：長尾から上杉への継承（関東管領就任） ／ Nagao Becomes Uesugi (Kantō Kanrei Succession) ／ Нагао становится Уэсуги (наследование должности канрэя Канто)
- 史実：1561-04（永禄4年閏3月16日（鶴岡八幡宮での就任））。地域：越後、関東
- 影響勢力：長尾、山内上杉、北条、武田（中心：長尾）
- 早期解放の研究：C16（`CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS`）、C11（`CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES`）、C30（`CIVIC_JAC_SENGOKU_LAND_TENURE_CONFIRMATION`）
- 史実の年のBoost：C16（`CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS`）。助言ポップアップ：なし
- 案（暫定）：長尾景虎が上杉憲政から上杉家の名跡と関東管領職を譲られる。条件を満たすと家名を『長尾』から『上杉』へ表示切替（Civilization IDは変えない）。条件案：山内上杉の庇護（EV_KANTO_KANREI）＋C16（朝廷・将軍への取次）を研究。C16を先に研究すれば1561年より前に切替可能。
- 出典：コトバンク『上杉謙信』；Wikipedia日本語版『上杉謙信』（確度：high）
- 補足：SSOT 3.1の記述に対応。表示切替の実装方式はSSOT 3.1（共有状態＋UI上書き）に従う。

### 毛利氏の石見銀山掌握 `EV_MORI_IWAMI`

- 名称：毛利氏の石見銀山掌握 ／ Mōri Takes Iwami Silver Mine ／ Мори захватывают серебряный рудник Ивами
- 史実：1562（永禄5年（1562）ごろ。灰吹法の導入は1533年（開始前））。地域：石見
- 影響勢力：毛利、大内、尼子（中心：毛利）
- 早期解放の研究：T06（`TECH_JAC_SENGOKU_ORE_PROSPECTING`）、T09（`TECH_JAC_SENGOKU_CUPELLATION`）、T14（`TECH_JAC_SENGOKU_MINE_MANAGEMENT`）
- 史実の年のBoost：T09（`TECH_JAC_SENGOKU_CUPELLATION`）。助言ポップアップ：なし
- 案（暫定）：石見銀山が毛利領となる。灰吹法（T09）により銀の産出が増え、鉄奉行と併せて資金源にする。1534年の石見は大内領のため、所有移転の候補。
- 資源：`RESOURCE_SILVER`、石見銀山・石見 (35.103, 132.437)、`ownership`、1526年〜
- 出典：[UNESCO 石見銀山遺跡とその文化的景観](https://whc.unesco.org/en/list/1246/)；documents/research/japan-map-resources.md（資源の代表地点と年代）；Wikipedia日本語版『石見銀山』（確度：high）
- 補足：石見銀山は1526年の発見で、資源配置では開始時から地図にある（SSOT 3.2 で銀を温帯の丘にも置けるようにした）。灰吹法(T09)は3.2の『開始前から石見に伝わる技術』。

### 三河一向一揆 `EV_MIKAWA_IKKI`

- 名称：三河一向一揆 ／ Mikawa Ikkō-ikki ／ Восстание икко-икки в Микаве
- 史実：1563〜1564（永禄6年秋〜永禄7年（1563〜64））。地域：三河
- 影響勢力：松平、本願寺（中心：松平）
- 早期解放の研究：C08（`CIVIC_JAC_SENGOKU_TEMPLE_TOWN_AUTONOMY`）、C18（`CIVIC_JAC_SENGOKU_SHRINE_LAND_GUARANTEES`）
- 史実の年のBoost：C08（`CIVIC_JAC_SENGOKU_TEMPLE_TOWN_AUTONOMY`）。助言ポップアップ：『門徒と和睦し、寺内町の自治を認めましょう。』
- 案（暫定）：松平（家康）が三河の一向宗（本願寺派）の反乱に直面。寺内町の自治（C08）を認めた勢力は寺社との関係が改善され、否認すると一時的な内乱が起きる。
- 出典：コトバンク『三河一向一揆』；Wikipedia日本語版『三河一向一揆』（確度：high）

### 月山富田城の開城と尼子氏の滅亡 `EV_AMAGO_FALL`

- 名称：月山富田城の開城と尼子氏の滅亡 ／ Fall of Gassan-Toda and the Amago ／ Падение Гассан-Тода и дома Амаго
- 史実：1566-12（永禄9年11月）。地域：出雲、伯耆、石見
- 影響勢力：尼子、毛利（中心：尼子、毛利）
- 早期解放の研究：T16（`TECH_JAC_SENGOKU_RIDGE_DITCHES`）、T21（`TECH_JAC_SENGOKU_STONE_RAMPARTS`）、T19（`TECH_JAC_SENGOKU_SUPPLY_ROADS`）
- 史実の年のBoost：T19（`TECH_JAC_SENGOKU_SUPPLY_ROADS`）。助言ポップアップ：なし
- 案（暫定）：毛利元就が月山富田城を兵糧攻めにし、尼子義久が降伏。出雲・伯耆が毛利領になり、奥出雲のたたら製鉄地帯（RESOURCE_IRON）も毛利の資源になる。
- 資源：`RESOURCE_IRON`、奥出雲・出雲 (35.199, 133.092)、`ownership`、1534年〜
- 出典：コトバンク『尼子義久』；Wikipedia日本語版『月山富田城』（確度：high）
- 補足：japan-map-resources.md: 出雲では戦国期に毛利氏が良鉄確保のため鉄奉行を置いた。

### 松平から徳川への改姓 `EV_MATSUDAIRA_TOKUGAWA`

- 名称：松平から徳川への改姓 ／ Matsudaira Becomes Tokugawa ／ Мацудайра принимают фамилию Токугава
- 史実：1567-01（永禄9年12月（勅許））。地域：三河
- 影響勢力：松平（中心：松平）
- 早期解放の研究：C16（`CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS`）
- 史実の年のBoost：C16（`CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS`）。助言ポップアップ：なし
- 案（暫定）：松平元康（家康）が朝廷から徳川の姓を認められ、三河守に任官。長尾→上杉と同じ仕組みで、条件を満たすと家名を『松平』から『徳川』へ表示切替（Civilization IDは維持。SSOT 3.2）。条件案：C16を研究。
- 出典：コトバンク『徳川家康』；Wikipedia日本語版『徳川家康』（確度：high）
- 補足：SSOT 3.2 で、長尾→上杉と同じ仕組みで松平→徳川を切り替えると決まっている。

### 楽市楽座（加納・安土） `EV_RAKUICHI`

- 名称：楽市楽座（加納・安土） ／ Rakuichi-Rakuza Free Markets (Kanō, Azuchi) ／ Свободные рынки Ракуити-ракудза (Кано, Адзути)
- 史実：1567-11（永禄10年10月（美濃・加納）、天正5年（1577）安土山下町）。地域：美濃、近江
- 影響勢力：織田、六角、足利将軍家（中心：織田）
- 早期解放の研究：C17（`CIVIC_JAC_SENGOKU_FREE_MARKETS`）
- 史実の年のBoost：C17（`CIVIC_JAC_SENGOKU_FREE_MARKETS`）。助言ポップアップ：なし
- 案（暫定）：城下町の市場を開放し、座の特権を制限する。商人が集まり金・生産が増え、忠誠が下がる場合もある（暫定）。C17を研究した勢力は他家でも施行可能。
- 出典：コトバンク『楽市楽座』；Wikipedia日本語版『楽市・楽座』（確度：high）
- 補足：『楽市楽座』は織田だけの名ではなく、そのまま一般名にできる。1549年の六角氏・石寺新市の楽市令（確度中）が先例で、六角は本イベントの年より早く施行できる。

### 織田信長の上洛と足利義昭の将軍就任 `EV_JORAKU`

- 名称：織田信長の上洛と足利義昭の将軍就任 ／ Oda Nobunaga Enters Kyoto; Ashikaga Yoshiaki Becomes Shogun ／ Ода Нобунага входит в Киото; Асикага Есиаки становится сёгуном
- 史実：1568-10（永禄11年9〜10月）。地域：近江、山城、摂津
- 影響勢力：織田、足利将軍家、六角、三好（中心：織田、足利将軍家）
- 早期解放の研究：C16（`CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS`）、C11（`CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES`）
- 史実の年のBoost：C16（`CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS`）。助言ポップアップ：なし
- 案（暫定）：織田が六角氏を観音寺城から追い、三好勢を破って義昭を将軍に擁立する。足利将軍家の当主が交代し、織田が畿内へ進出。近江・山城・摂津の城の所有移転の候補。
- 出典：コトバンク『足利義昭』；Wikipedia日本語版『織田信長』（確度：high）

### 駿河侵攻と今川氏の滅亡 `EV_IMAGAWA_FALL`

- 名称：駿河侵攻と今川氏の滅亡 ／ Invasion of Suruga and Fall of the Imagawa ／ Вторжение в Суруга и падение дома Имагава
- 史実：1569-01（永禄11年12月〜永禄12年5月）。地域：駿河、遠江
- 影響勢力：今川、武田、松平、北条（中心：今川、武田、松平）
- 案（暫定）：武田が三国同盟を破棄して駿河へ、松平（徳川）が遠江へ侵攻。今川氏真は1569年に掛川城を開城し、今川は滅亡。同盟破棄で武田と北条の関係が敵対化。
- 出典：コトバンク『今川氏真』；Wikipedia日本語版『駿河侵攻』（確度：high）

### 石山合戦（石山本願寺と織田の戦い） `EV_ISHIYAMA`

- 名称：石山合戦（石山本願寺と織田の戦い） ／ Ishiyama War ／ Война Исияма
- 史実：1570-10〜1580（元亀元年9月〜天正8年8月（1570〜1580））。地域：摂津、加賀、伊勢
- 影響勢力：本願寺、織田、毛利、松平（中心：本願寺、織田）
- 早期解放の研究：C08（`CIVIC_JAC_SENGOKU_TEMPLE_TOWN_AUTONOMY`）、C18（`CIVIC_JAC_SENGOKU_SHRINE_LAND_GUARANTEES`）、T21（`TECH_JAC_SENGOKU_STONE_RAMPARTS`）
- 史実の年のBoost：C18（`CIVIC_JAC_SENGOKU_SHRINE_LAND_GUARANTEES`）。助言ポップアップ：『本願寺と和睦し、寺社領を安堵しましょう。』
- 案（暫定）：本願寺が織田と10年間戦い、1580年に降伏して石山を退去。寺内町自治（C08）を持つ勢力は防衛が強く、全国の一向一揆勢力が連動。本願寺の吸収・降伏の条件付きイベント。
- 出典：コトバンク『石山合戦』；Wikipedia日本語版『石山合戦』（確度：high）

### 長崎の開港 `EV_NAGASAKI`

- 名称：長崎の開港 ／ Opening of Nagasaki Port ／ Открытие порта Нагасаки
- 史実：1571（元亀2年（1571））。地域：肥前
- 影響勢力：大友、島津（中心：大友）
- 早期解放の研究：C13（`CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS`）、C19（`CIVIC_JAC_SENGOKU_NANBAN_ENVOYS`）、C29（`CIVIC_JAC_SENGOKU_PORT_ADMINISTRATION`）
- 史実の年のBoost：C19（`CIVIC_JAC_SENGOKU_NANBAN_ENVOYS`）。助言ポップアップ：なし
- 案（暫定）：肥前・長崎がポルトガル貿易の港になり、大友・島津の影響下で交易量が増す。長崎の硝石・生糸の交易アクセスは資源配置に無いため付けない（Q55）。
- 出典：コトバンク『長崎』；Wikipedia日本語版『長崎港』（確度：high）
- 補足：1534年配分で肥前は大友領。大村純忠らの影響で実際の帰属は複雑。

### 室町幕府の終焉 `EV_MUROMACHI_END`

- 名称：室町幕府の終焉 ／ End of the Muromachi Shogunate ／ Конец сёгуната Муромати
- 史実：1573-08（元亀4年（天正元年）7月）。地域：山城
- 影響勢力：足利将軍家（中心：足利将軍家）
- 早期解放の研究：C16（`CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS`）
- 史実の年のBoost：C16（`CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS`）。助言ポップアップ：なし
- 案（暫定）：織田が足利義昭を京都から追放する。足利将軍家の勢力は残り、裁定の力（停戦裁定・所領安堵）だけを失う（SSOT 3.2）。
- 出典：コトバンク『足利義昭』；Wikipedia日本語版『室町幕府』（確度：high）
- 補足：SSOT 3.1で『足利将軍家』は勢力の1つ。プレイヤー勢力を消すのではなく、将軍位の喪失イベントとして扱う案。

### 朝倉氏の滅亡 `EV_ASAKURA_FALL`

- 名称：朝倉氏の滅亡 ／ Fall of the Asakura ／ Падение дома Асакура
- 史実：1573-09（天正元年8月20日）。地域：越前
- 影響勢力：朝倉、織田（中心：朝倉、織田）
- 案（暫定）：織田が越前に侵攻し、朝倉義景が自害する。一乗谷が焼かれ、越前が織田の領土になる候補。
- 出典：コトバンク『朝倉義景』；Wikipedia日本語版『一乗谷城の戦い』（確度：high）

### 長篠の戦い `EV_NAGASHINO`

- 名称：長篠の戦い ／ Battle of Nagashino ／ Битва при Нагасино
- 史実：1575-06（天正3年5月21日）。地域：三河
- 影響勢力：織田、松平、武田（中心：織田、松平、武田）
- 早期解放の研究：T22（`TECH_JAC_SENGOKU_MATCHLOCK_PRODUCTION`）、T23（`TECH_JAC_SENGOKU_SHOT_AND_POWDER`）、C21（`CIVIC_JAC_SENGOKU_GUN_CORPS_ORGANIZATION`）
- 史実の年のBoost：C21（`CIVIC_JAC_SENGOKU_GUN_CORPS_ORGANIZATION`）。助言ポップアップ：なし
- 案（暫定）：織田・徳川連合軍の鉄砲隊が武田の騎馬軍を破る。鉄砲量産（T22）・玉薬生産（T23）・鉄砲衆編制（C21）を持つ勢力は、鉄砲ユニットの戦力補正が大きい。三段撃ちの真偽には諸説あり、ゲーム内では『鉄砲衆の一斉射撃』効果とする（暫定）。
- 出典：コトバンク『長篠の戦い』；Wikipedia日本語版『長篠の戦い』（確度：high）

### 長宗我部元親の土佐統一（四万十川の戦い） `EV_SHIMANTO`

- 名称：長宗我部元親の土佐統一（四万十川の戦い） ／ Chosokabe Motochika Unifies Tosa (Shimantogawa) ／ Тёсокабэ Мототика объединяет Тоса (Симантогава)
- 史実：1575-08（天正3年7月）。地域：土佐
- 影響勢力：長宗我部（中心：長宗我部）
- 早期解放の研究：C02（`CIVIC_JAC_SENGOKU_KOKUJIN_LEAGUES`）、C10（`CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER`）、T10（`TECH_JAC_SENGOKU_WOODEN_SHIPBUILDING`）
- 史実の年のBoost：C10（`CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER`）。助言ポップアップ：なし
- 案（暫定）：長宗我部元親が一条兼定を破って土佐を統一。土佐の忠誠が上がり、四国進出（阿波・讃岐）の足がかりになる。
- 出典：コトバンク『長宗我部元親』；Wikipedia日本語版『四万十川の戦い』（確度：high）

### 安土城の築城 `EV_AZUCHI`

- 名称：安土城の築城 ／ Construction of Azuchi Castle ／ Строительство замка Адзути
- 史実：1576-02（天正4年（1576）着工、1579年天主完成）。地域：近江
- 影響勢力：織田、六角（中心：織田）
- 早期解放の研究：T29（`TECH_JAC_SENGOKU_KEEP_AND_STONE_BASE`）、T21（`TECH_JAC_SENGOKU_STONE_RAMPARTS`）、T25（`TECH_JAC_SENGOKU_ANGLED_GATEWAYS`）
- 家固有：`ODA` の名。他の勢力では『大規模城郭の築城／Grand Castle Construction／Строительство великого замка』と表示
- 史実の年のBoost：T21（`TECH_JAC_SENGOKU_STONE_RAMPARTS`）。助言ポップアップ：なし
- 案（暫定）：近江・安土に大規模な天主を備えた城が建つ。石垣（T21）・本丸石垣（T29）を研究した勢力は、他の城で似た改修が可能になる。城の防御・忠誠・文化産出が上がる。
- 出典：コトバンク『安土城』；Wikipedia日本語版『安土城』（確度：high）

### 上杉謙信の死去と御館の乱 `EV_KENSHIN_DEATH`

- 名称：上杉謙信の死去と御館の乱 ／ Death of Uesugi Kenshin; Otate Disturbance ／ Смерть Уэсуги Кэнсин; смута Отатэ
- 史実：1578-04〜1579（天正6年3月13日（謙信死去）〜天正7年（1579））。地域：越後
- 影響勢力：長尾、北条、武田（中心：長尾）
- 早期解放の研究：C02（`CIVIC_JAC_SENGOKU_KOKUJIN_LEAGUES`）、C11（`CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES`）
- 史実の年のBoost：C11（`CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES`）。助言ポップアップ：『後継争いを和睦で収め、北条・武田との同盟を探りましょう。』
- 案（暫定）：後継者争い（景勝と景虎）で上杉が内乱に陥り、北条・武田が介入する。継承者の選択が同盟に影響（暫定）。
- 出典：コトバンク『上杉謙信』；Wikipedia日本語版『御館の乱』（確度：high）

### 武田氏の滅亡（天目山） `EV_TAKEDA_FALL`

- 名称：武田氏の滅亡（天目山） ／ Fall of the Takeda (Tenmokuzan) ／ Падение дома Такэда (Тэммокудзан)
- 史実：1582-04（天正10年3月11日）。地域：甲斐、信濃
- 影響勢力：武田、織田、松平、北条（中心：武田、織田、松平）
- 案（暫定）：織田・徳川・北条の甲州征伐で武田勝頼が自害。武田領（甲斐・信濃・駿河）が分割される。武田の金山（EV_KOSHU_GOLD）の所有移転。
- 出典：コトバンク『武田勝頼』；Wikipedia日本語版『甲州征伐』（確度：high）

### 本能寺の変 `EV_HONNOJI`

- 名称：本能寺の変 ／ Honnō-ji Incident ／ Инцидент Хоннодзи
- 史実：1582-06（天正10年6月2日）。地域：山城、尾張、近江
- 影響勢力：織田、松平、毛利（中心：織田）
- 早期解放の研究：C16（`CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS`）、C11（`CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES`）
- 史実の年のBoost：C16（`CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS`）。助言ポップアップ：『朝廷・将軍への取次で、後継の正統性を固めましょう。』
- 案（暫定）：織田信長が明智光秀に討たれる。山崎の戦い、清洲会議を経て羽柴秀吉が主導権を握る。織田の当主が替わり、表示名を織田→羽柴→豊臣へ切り替える（長尾→上杉と同じ仕組み。SSOT 3.2）。
- 出典：コトバンク『本能寺の変』；Wikipedia日本語版『本能寺の変』（確度：high）
- 補足：豊臣秀吉はSSOTで『成長やイベントで登場』する人物（5章）。勢力としては22勢力に含まれず、織田家の表示名の切り替えで表す（SSOT 3.2）。

### 山崎検地から太閤検地へ `EV_YAMAZAKI_KENCHI`

- 名称：山崎検地から太閤検地へ ／ Yamazaki Survey to Taikō Land Survey ／ Обследование Ямадзаки и земельное обследование Тайко
- 史実：1582-07（天正10年（1582）山崎検地、天正19年（1591）御前帳提出）。地域：山城、全国
- 影響勢力：全勢力（中心：織田）
- 早期解放の研究：C20（`CIVIC_JAC_SENGOKU_DOMAIN_LAND_SURVEYS`）、C25（`CIVIC_JAC_SENGOKU_TAIKO_LAND_SURVEY`）、T31（`TECH_JAC_SENGOKU_SURVEY_STANDARDS`）
- 史実の年のBoost：C20（`CIVIC_JAC_SENGOKU_DOMAIN_LAND_SURVEYS`）。助言ポップアップ：なし
- 案（暫定）：羽柴秀吉が山城・山崎で検地を始め、のちに全国で検地（石高制）が進む。太閤検地（C25）は研究したすべての家に『太閤検地』と表示する（SSOT 3.2）。先に領国検地（C20）／検地尺の標準化（T31）を研究した勢力は年より早く検地の効果を使える。
- 出典：[小田原デジタルミュージアム 北条氏の検地・貫高制](https://odawara-digital-museum.jp/selection/hojo/24/)；コトバンク『太閤検地』；Wikipedia日本語版『太閤検地』（確度：high）

### 大坂城の築城 `EV_OSAKA`

- 名称：大坂城の築城 ／ Construction of Osaka Castle ／ Строительство замка Осака
- 史実：1583-10（天正11年（1583）9月着工）。地域：摂津
- 影響勢力：本願寺、織田、三好（中心：織田）
- 早期解放の研究：T29（`TECH_JAC_SENGOKU_KEEP_AND_STONE_BASE`）、T21（`TECH_JAC_SENGOKU_STONE_RAMPARTS`）、T25（`TECH_JAC_SENGOKU_ANGLED_GATEWAYS`）、T32（`TECH_JAC_SENGOKU_OUTER_DEFENSES`）
- 家固有：`ODA`（羽柴→豊臣に切り替えた後の織田）の名。他の勢力では『巨大城郭の築城／Grand Castle Construction／Строительство великого замка』と表示
- 史実の年のBoost：T29（`TECH_JAC_SENGOKU_KEEP_AND_STONE_BASE`）。助言ポップアップ：なし
- 案（暫定）：石山本願寺の跡地に巨大な城郭が築かれる。石垣・本丸・惣構（T21／T29／T32）を持つ勢力は、大都市防衛を強化する。豊臣の拠点化。
- 出典：コトバンク『大坂城』；Wikipedia日本語版『大坂城』（確度：high）

### 四国平定 `EV_SHIKOKU`

- 名称：四国平定 ／ Shikoku Campaign ／ Поход на Сикоку
- 史実：1585-07（天正13年6〜8月）。地域：阿波、讃岐、伊予、土佐
- 影響勢力：長宗我部、三好、足利将軍家（中心：長宗我部）
- 早期解放の研究：T20（`TECH_JAC_SENGOKU_WARSHIP_CONSTRUCTION`）、T10（`TECH_JAC_SENGOKU_WOODEN_SHIPBUILDING`）、C10（`CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER`）
- 史実の年のBoost：C10（`CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER`）。助言ポップアップ：『天下人との和睦で、本領を守るか考えましょう。』
- 案（暫定）：秀吉が四国へ侵攻し、長宗我部元親が降伏して土佐一国に戻る。阿波・讃岐・伊予の城が動く。
- 出典：コトバンク『長宗我部元親』；Wikipedia日本語版『四国攻め』（確度：high）

### 天正地震 `EV_TENSHO_QUAKE`

- 名称：天正地震 ／ Tenshō Earthquake ／ Землетрясение Тэнсё
- 史実：1586-01（天正13年11月29日）。地域：飛騨、美濃、越中、近江、伊勢
- 影響勢力：朝倉、六角、織田、足利将軍家（中心：足利将軍家）
- 早期解放の研究：T18（`TECH_JAC_SENGOKU_BATH_AND_MEDICINE`）、T07（`TECH_JAC_SENGOKU_GRAIN_STORES`）
- 史実の年のBoost：T18（`TECH_JAC_SENGOKU_BATH_AND_MEDICINE`）。助言ポップアップ：『被災した城を修理し、湯屋と兵糧蔵で民を救いましょう。』
- 案（暫定）：飛騨・美濃・越中・近江・伊勢に大地震。帰雲城が埋没、長浜城が倒壊。被災地域の都市の人口・建物が損傷（暫定）。T18／T07を持つ勢力は被害を軽減。
- 出典：コトバンク『天正地震』；Wikipedia日本語版『天正地震』（確度：high）

### 九州平定 `EV_KYUSHU`

- 名称：九州平定 ／ Kyushu Campaign ／ Поход на Кюсю
- 史実：1587-01（天正14年12月〜天正15年5月（1586〜1587））。地域：日向、豊後、肥後、筑前
- 影響勢力：島津、大友（中心：島津、大友）
- 早期解放の研究：T22（`TECH_JAC_SENGOKU_MATCHLOCK_PRODUCTION`）、T19（`TECH_JAC_SENGOKU_SUPPLY_ROADS`）、T20（`TECH_JAC_SENGOKU_WARSHIP_CONSTRUCTION`）
- 史実の年のBoost：T19（`TECH_JAC_SENGOKU_SUPPLY_ROADS`）。助言ポップアップ：『天下人と和睦し、本領の安堵を求めましょう。』
- 案（暫定）：秀吉が九州へ侵攻し、島津義久が降伏して薩摩・大隅を安堵される(日向の一部は上納)。大友は豊後を安堵。1586年の戸次川の戦いの結果も含む。
- 出典：コトバンク『九州平定』；Wikipedia日本語版『九州平定』（確度：high）

### バテレン追放令 `EV_BATEREN`

- 名称：バテレン追放令 ／ Edict Expelling Missionaries (Bateren) ／ Указ об изгнании миссионеров (батэрэн)
- 史実：1587-07（天正15年6月19日）。地域：九州、全国
- 影響勢力：全勢力（中心：大友、島津）
- 早期解放の研究：C18（`CIVIC_JAC_SENGOKU_SHRINE_LAND_GUARANTEES`）、C29（`CIVIC_JAC_SENGOKU_PORT_ADMINISTRATION`）
- 史実の年のBoost：C18（`CIVIC_JAC_SENGOKU_SHRINE_LAND_GUARANTEES`）。助言ポップアップ：なし
- 案（暫定）：秀吉が宣教師の国外退去を命じる。カトリックの信徒は増加が止まり、地下化する。C18／C29を研究した勢力は信徒への締め付けを弱められる。
- 出典：コトバンク『バテレン追放令』；Wikipedia日本語版『バテレン追放令』（確度：high）

### 北野大茶湯 `EV_KITANO_TEA`

- 名称：北野大茶湯 ／ Kitano Grand Tea Gathering ／ Великое чаепитие в Китано
- 史実：1587-11（天正15年10月1日）。地域：山城
- 影響勢力：全勢力（中心：織田）
- 早期解放の研究：C14（`CIVIC_JAC_SENGOKU_TEA_GATHERINGS`）、C22（`CIVIC_JAC_SENGOKU_TEA_DIPLOMACY`）
- 家固有：`ODA`（羽柴→豊臣に切り替えた後の織田）の名。他の勢力では『大茶会／Grand Tea Gathering／Большое чаепитие』と表示
- 史実の年のBoost：C14（`CIVIC_JAC_SENGOKU_TEA_GATHERINGS`）。助言ポップアップ：なし
- 案（暫定）：秀吉が京の北野で身分を問わず茶会を開く。茶会外交（C22）と名物（C14）による友好度・文化産出が上がる。宇治の茶（RESOURCE_TEA）の価値が上がる。
- 出典：コトバンク『北野大茶湯』；Wikipedia日本語版『北野大茶湯』（確度：high）

### 刀狩令 `EV_KATANAGARI`

- 名称：刀狩令 ／ Sword Hunt Edict ／ Указ об охоте за мечами
- 史実：1588-08（天正16年7月8日）。地域：全国
- 影響勢力：全勢力（中心：織田）
- 早期解放の研究：C27（`CIVIC_JAC_SENGOKU_SWORD_HUNT`）、C28（`CIVIC_JAC_SENGOKU_WARRIOR_FARMER_SEPARATION`）
- 史実の年のBoost：C27（`CIVIC_JAC_SENGOKU_SWORD_HUNT`）。助言ポップアップ：なし
- 案（暫定）：農民の武器を没収する令。一揆の発生を抑え、忠誠が上がるが、兵農分離（C28）前は徴兵基盤が減る。自動施行せず選択制（樹形案の方針）。
- 出典：コトバンク『刀狩』；Wikipedia日本語版『刀狩』（確度：high）

### 秀吉の琉球への服属要求と島津の仲介 `EV_RYUKYU_SHIMAZU_DEMAND`

- 名称：秀吉の琉球への服属要求と島津の仲介 ／ Hideyoshi's Demand for Ryukyu Submission ／ Требование Хидэёси о подчинении Рюкю
- 史実：1589〜1592（天正17年（1589）〜文禄元年（1592））。地域：琉球、薩摩
- 影響勢力：琉球王国、島津（中心：琉球王国、島津）
- 早期解放の研究：C11（`CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES`）、C13（`CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS`）、C16（`CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS`）
- 史実の年のBoost：C11（`CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES`）。助言ポップアップ：『島津を通じて使節を送り、和睦を保ちましょう。』
- 案（暫定）：秀吉が島津を通じて琉球に入貢と朝鮮出兵の兵糧を求める。琉球は拒否・遅延し、島津との関係が悪化(1609年の侵攻へ続く)。
- 出典：コトバンク『琉球王国』；Wikipedia日本語版『琉球の役』（確度：medium）

### 摺上原の戦い（蘆名氏の滅亡） `EV_SURIAGEHARA`

- 名称：摺上原の戦い（蘆名氏の滅亡） ／ Battle of Suriagehara (Fall of the Ashina) ／ Битва при Суриагэхара (падение дома Асина)
- 史実：1589-07（天正17年6月5日）。地域：陸奥、出羽
- 影響勢力：蘆名、伊達（中心：蘆名、伊達）
- 早期解放の研究：T22（`TECH_JAC_SENGOKU_MATCHLOCK_PRODUCTION`）、C10（`CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER`）
- 史実の年のBoost：C10（`CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER`）。助言ポップアップ：なし
- 案（暫定）：伊達政宗が蘆名義広を破り会津を得る。蘆名の吸収条件例。会津の漆（RESOURCE_JAC_LACQUER案）はまだ地図に無く、翌1590年の蒲生氏郷の入封（EV_AIZU_LACQUER）で現れる。
- 出典：コトバンク『摺上原の戦い』；Wikipedia日本語版『摺上原の戦い』（確度：high）

### 上杉景勝の佐渡平定と西三川砂金 `EV_SADO_SUBJUGATION`

- 名称：上杉景勝の佐渡平定と西三川砂金 ／ Uesugi Kagekatsu Takes Sado; Nishimikawa Gold ／ Уэсуги Кагэкацу берёт Садо; золото Нисимикава
- 史実：1589-08（天正17年（1589）平定、西三川の本格採取は1593年ごろ）。地域：佐渡
- 影響勢力：長尾（中心：長尾）
- 早期解放の研究：T14（`TECH_JAC_SENGOKU_MINE_MANAGEMENT`）、T06（`TECH_JAC_SENGOKU_ORE_PROSPECTING`）
- 史実の年のBoost：T14（`TECH_JAC_SENGOKU_MINE_MANAGEMENT`）。助言ポップアップ：なし
- 案（暫定）：上杉が佐渡の本間氏を平定。西三川の砂金（金）が採れる。金（RESOURCE_JAC_GOLD案）を佐渡に追加。相川金銀山の発見（1596）と本格開発（1601）の前提。
- 資源：`RESOURCE_JAC_GOLD`、西三川・佐渡 (37.819, 138.273)、`reveal`、1593年〜
- 出典：documents/research/japan-map-resources.md（資源の代表地点と年代）；Wikipedia日本語版『西三川砂金山』（確度：medium）

### 小田原征伐と北条氏の滅亡 `EV_ODAWARA`

- 名称：小田原征伐と北条氏の滅亡 ／ Odawara Campaign; Fall of the Hojo ／ Поход на Одавару; падение дома Ходзё
- 史実：1590-05（天正18年4月〜7月5日）。地域：相模、武蔵、下野
- 影響勢力：北条、伊達、松平、山内上杉（中心：北条、松平）
- 早期解放の研究：T32（`TECH_JAC_SENGOKU_OUTER_DEFENSES`）、T21（`TECH_JAC_SENGOKU_STONE_RAMPARTS`）
- 史実の年のBoost：T32（`TECH_JAC_SENGOKU_OUTER_DEFENSES`）。助言ポップアップ：『包囲を受ける前に、天下人との和睦を考えましょう。』
- 案（暫定）：秀吉が小田原城を包囲し、北条氏直が降伏。北条領（関東）は徳川家康へ与えられる（次のイベント）。惣構（T32）を持つ勢力は籠城の耐久が高い。
- 出典：コトバンク『小田原征伐』；Wikipedia日本語版『小田原征伐』（確度：high）

### 徳川家康の関東移封 `EV_MATSUDAIRA_KANTO`

- 名称：徳川家康の関東移封 ／ Tokugawa Ieyasu Moves to Kantō ／ Перевод Токугава Иэясу в Канто
- 史実：1590-08（天正18年8月1日（江戸入城））。地域：武蔵、相模、三河
- 影響勢力：松平、北条（中心：松平）
- 早期解放の研究：C15（`CIVIC_JAC_SENGOKU_CASTLE_TOWN_ORDINANCES`）、C30（`CIVIC_JAC_SENGOKU_LAND_TENURE_CONFIRMATION`）
- 史実の年のBoost：C15（`CIVIC_JAC_SENGOKU_CASTLE_TOWN_ORDINANCES`）。助言ポップアップ：なし
- 案（暫定）：徳川家康が旧北条領へ移り、江戸城に入る。三河・遠江・駿河・甲斐・信濃を離れる。徳川の本拠が江戸へ変わる（本拠移動イベント）。
- 出典：コトバンク『徳川家康』；Wikipedia日本語版『徳川家康』（確度：high）

### 蒲生氏郷の会津入封と漆器産業 `EV_AIZU_LACQUER`

- 名称：蒲生氏郷の会津入封と漆器産業 ／ Gamō Ujisato in Aizu; Lacquerware Industry ／ Гамо Удзисато в Аидзу; лаковое производство
- 史実：1590-09（天正18年（1590）8月）。地域：陸奥
- 影響勢力：伊達、蘆名（中心：蘆名）
- 早期解放の研究：C15（`CIVIC_JAC_SENGOKU_CASTLE_TOWN_ORDINANCES`）、C13（`CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS`）
- 史実の年のBoost：C15（`CIVIC_JAC_SENGOKU_CASTLE_TOWN_ORDINANCES`）。助言ポップアップ：なし
- 案（暫定）：奥州仕置で会津が蒲生氏郷に与えられ、近江から漆器職人が招かれる。漆（RESOURCE_JAC_LACQUER案）を会津に追加。会津は蘆名の旧領で、黒川城は若松に改称。
- 資源：`RESOURCE_JAC_LACQUER`、会津 (37.490, 139.930)、`reveal`、1590年〜
- 出典：documents/research/japan-map-resources.md（資源の代表地点と年代）；Wikipedia日本語版『蒲生氏郷』（確度：medium）

### 身分統制令（兵農分離） `EV_SHINBUN_SEIDO`

- 名称：身分統制令（兵農分離） ／ Status Control Edict (Warrior-Farmer Separation) ／ Указ о контроле сословий (разделение воинов и земледельцев)
- 史実：1591-10（天正19年8月21日）。地域：全国
- 影響勢力：全勢力（中心：織田）
- 早期解放の研究：C28（`CIVIC_JAC_SENGOKU_WARRIOR_FARMER_SEPARATION`）、C27（`CIVIC_JAC_SENGOKU_SWORD_HUNT`）、C25（`CIVIC_JAC_SENGOKU_TAIKO_LAND_SURVEY`）
- 史実の年のBoost：C28（`CIVIC_JAC_SENGOKU_WARRIOR_FARMER_SEPARATION`）。助言ポップアップ：なし
- 案（暫定）：武士・町人・百姓の身分を固定する令。C28を持つ勢力は徴兵基盤と税収を安定させる。
- 出典：コトバンク『身分統制令』；Wikipedia日本語版『身分統制令』（確度：high）

### 文禄・慶長の役（朝鮮出兵） `EV_BUNROKU`

- 名称：文禄・慶長の役（朝鮮出兵） ／ Imjin War (Korean Campaigns) ／ Имджинская война (корейские походы)
- 史実：1592-05〜1598（文禄元年4月〜慶長3年（1592〜1598））。地域：九州、対馬、琉球
- 影響勢力：島津、毛利、長宗我部、大友、琉球王国（中心：島津、毛利、長宗我部）
- 早期解放の研究：T19（`TECH_JAC_SENGOKU_SUPPLY_ROADS`）、T20（`TECH_JAC_SENGOKU_WARSHIP_CONSTRUCTION`）、T24（`TECH_JAC_SENGOKU_PORT_SUPPLY`）、C23（`CIVIC_JAC_SENGOKU_COALITION_CAMPAIGNS`）、C29（`CIVIC_JAC_SENGOKU_PORT_ADMINISTRATION`）
- 史実の年のBoost：T24（`TECH_JAC_SENGOKU_PORT_SUPPLY`）。助言ポップアップ：なし
- 案（暫定）：秀吉が朝鮮へ出兵する。兵站（T19／T24）・軍船（T20）・連合軍役（C23）が出兵の戦力補正となる。参加した勢力は人口・生産を消耗し、琉球に軍役が課される。日本国内の城の支配には直接効かない。
- 出典：コトバンク『文禄・慶長の役』；Wikipedia日本語版『文禄・慶長の役』（確度：high）

### 蠣崎慶広への朱印状とアイヌ交易の変質 `EV_EZO_MATSUMAE`

- 名称：蠣崎慶広への朱印状とアイヌ交易の変質 ／ Kakizaki Yoshihiro Receives Hideyoshi's Seal ／ Какидзаки Есихиро получает печать Хидэёси
- 史実：1593（文禄2年（1593））。地域：蝦夷島（道南）
- 影響勢力：アイヌ（中心：アイヌ）
- 早期解放の研究：C26（`CIVIC_JAC_SENGOKU_RED_SEAL_WRITS`）、C11（`CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES`）
- 史実の年のBoost：C11（`CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES`）。助言ポップアップ：『蠣崎氏との交易の取り決めを結び直しましょう。』
- 案（暫定）：蠣崎慶広が秀吉から蝦夷地の交易と支配の朱印状を得る。アイヌは蠣崎を通した交易に組み込まれ、交易の自由度が下がる（暫定）。蠣崎の松前への改姓は1599年。
- 出典：コトバンク『松前慶広』；Wikipedia日本語版『松前慶広』（確度：medium）

### 相川金銀山の発見 `EV_AIKAWA`

- 名称：相川金銀山の発見 ／ Discovery of Aikawa Gold-Silver Mine ／ Открытие золото-серебряного рудника Айкава
- 史実：1596（慶長元年（1596）発見、慶長6年（1601）本格開発）。地域：佐渡
- 影響勢力：長尾（中心：長尾）
- 早期解放の研究：T14（`TECH_JAC_SENGOKU_MINE_MANAGEMENT`）、T06（`TECH_JAC_SENGOKU_ORE_PROSPECTING`）、T09（`TECH_JAC_SENGOKU_CUPELLATION`）
- 史実の年のBoost：T09（`TECH_JAC_SENGOKU_CUPELLATION`）。助言ポップアップ：なし
- 案（暫定）：佐渡の相川に金銀山が発見される（1601年に本格開発）。佐渡の鉱業の最終段階で、金銀のタイルを追加し、T14で産出が大きく増える。鶴子銀山（1542年、銀）は別の資源で、採用の15 kmの地図では隣のタイルに置く。
- 資源：`RESOURCE_JAC_GOLD`、相川・佐渡 (38.032, 138.235)、`reveal`、1596年〜
- 出典：documents/research/japan-map-resources.md（資源の代表地点と年代）；Wikipedia日本語版『佐渡金山』（確度：medium）
- 補足：資源配置の gold-sado-aikawa（38.032, 138.235）。鶴子銀山（silver-sado-tsurushi、37.927, 138.351）とは別地点。

### サン＝フェリペ号事件と二十六聖人の殉教 `EV_SAN_FELIPE`

- 名称：サン＝フェリペ号事件と二十六聖人の殉教 ／ San Felipe Incident and Martyrdom of the 26 Saints ／ Инцидент с «Сан-Фелипе» и казнь 26 мучеников
- 史実：1596-10（文禄5年8月28日（1596年10月19日）土佐・浦戸に漂着。二十六聖人の殉教は慶長元年12月19日（1597年2月5日））。地域：土佐、肥前
- 影響勢力：長宗我部、大友、島津（中心：長宗我部）
- 早期解放の研究：C18（`CIVIC_JAC_SENGOKU_SHRINE_LAND_GUARANTEES`）、C19（`CIVIC_JAC_SENGOKU_NANBAN_ENVOYS`）
- 史実の年のBoost：C18（`CIVIC_JAC_SENGOKU_SHRINE_LAND_GUARANTEES`）。助言ポップアップ：なし
- 案（暫定）：土佐に漂着したスペイン船の積荷が没収され、キリスト教徒が長崎で処刑される（翌1597年）。カトリックの禁教が強まり、信徒の地下化が進む。
- 出典：コトバンク『サン＝フェリペ号事件』；Wikipedia日本語版『サン＝フェリペ号事件』（確度：high）

### 長宗我部氏掟書（百箇条） `EV_CHOSOKABE_HYAKKAJO`

- 名称：長宗我部氏掟書（百箇条） ／ Chosokabe Motochika Hyakkajō ／ Свод законов Тёсокабэ Мототика (Хякукадзё)
- 史実：1597-01（慶長元年11月15日（1596年））。地域：土佐
- 影響勢力：長宗我部（中心：長宗我部）
- 早期解放の研究：C05（`CIVIC_JAC_SENGOKU_DOMAIN_LAW`）
- 家固有：`CHOSOKABE` の名。他の勢力では『分国法の制定／Domain Law Enacted／Принятие законов владения』と表示
- 史実の年のBoost：C05（`CIVIC_JAC_SENGOKU_DOMAIN_LAW`）。助言ポップアップ：なし
- 案（暫定）：長宗我部元親が分国法を制定する。領国の忠誠・訴訟が安定する。
- 出典：コトバンク『長宗我部元親』；Wikipedia日本語版『長宗我部元親百箇条』（確度：high）

## 5. 勢力・地域・研究の網羅状況

| 勢力 | 関わるイベント数（『全勢力』を除く） | 主なイベント |
|---|---:|---|
| 織田 | 11 | 桶狭間の戦い、楽市楽座（加納・安土）、織田信長の上洛と足利義昭の将軍就任、石山合戦（石山本願寺と織田の戦い） |
| 伊達 | 5 | 塵芥集の制定（伊達の分国法）、伊達氏天文の乱、摺上原の戦い（蘆名氏の滅亡） |
| 蘆名 | 3 | 摺上原の戦い（蘆名氏の滅亡）、蒲生氏郷の会津入封と漆器産業 |
| 北条 | 10 | 北条氏康の税制改革（公事赦免令）、甲相駿三国同盟、小田原征伐と北条氏の滅亡 |
| 山内上杉 | 3 | 上杉憲政の越後亡命 |
| 武田 | 9 | 甲州法度之次第の制定、川中島の戦い、甲相駿三国同盟、甲州金の整備と黒川金山・中山金山の稼働 |
| 長尾 | 6 | 上杉憲政の越後亡命、川中島の戦い、長尾から上杉への継承（関東管領就任）、上杉謙信の死去と御館の乱 |
| 今川 | 3 | 甲相駿三国同盟、桶狭間の戦い、駿河侵攻と今川氏の滅亡 |
| 松平 | 10 | 三河一向一揆、松平から徳川への改姓、駿河侵攻と今川氏の滅亡、長篠の戦い |
| 朝倉 | 2 | 朝倉氏の滅亡 |
| 足利将軍家 | 8 | 生野銀山の本格開発、江口の戦いと三好長慶の畿内進出、織田信長の上洛と足利義昭の将軍就任、室町幕府の終焉 |
| 三好 | 5 | 国友・堺・根来の鉄砲鍛冶の成立、江口の戦いと三好長慶の畿内進出 |
| 六角 | 5 | 国友・堺・根来の鉄砲鍛冶の成立 |
| 本願寺 | 4 | 石山合戦（石山本願寺と織田の戦い） |
| 大内 | 3 | キリスト教の伝来、大寧寺の変、厳島の戦い |
| 尼子 | 3 | 月山富田城の開城と尼子氏の滅亡 |
| 毛利 | 7 | 厳島の戦い、毛利氏の石見銀山掌握、月山富田城の開城と尼子氏の滅亡、文禄・慶長の役（朝鮮出兵） |
| 長宗我部 | 5 | 長宗我部元親の土佐統一（四万十川の戦い）、四国平定、文禄・慶長の役（朝鮮出兵）、サン＝フェリペ号事件と二十六聖人の殉教 |
| 大友 | 6 | キリスト教の伝来、大友二階崩れの変、平戸へのポルトガル船来航と南蛮貿易、長崎の開港 |
| 島津 | 7 | 島津貴久の薩摩統一戦、鉄砲伝来、キリスト教の伝来、九州平定 |
| 琉球王国 | 2 | 秀吉の琉球への服属要求と島津の仲介 |
| アイヌ | 2 | 夷狄商舶往還之法度（蠣崎氏とアイヌの交易取り決め）、蠣崎慶広への朱印状とアイヌ交易の変質 |

- `earlier_by` に使った研究：44／62ノード。使っていない研究（イベントと無関係でも問題ない）：T02、T03、T04、T05、T08、T11、T15、T27、T28、T30、C01、C03、C04、C06、C07、C09、C12、C24
- 地域：東北（伊達・蘆名）、関東（北条・山内上杉）、甲信越（武田・長尾）、東海（今川・織田・松平）、北陸（朝倉・本願寺）、畿内（足利・三好・六角・本願寺）、中国（大内・尼子・毛利）、四国（長宗我部・三好）、九州（大友・島津）、琉球、蝦夷の全てにイベントがある。琉球王国とアイヌは史料が少なく、イベントは各2〜4件にとどまる。

## 6. 資源開発イベントと japan-map-resources.md の対応

| 資源 | 場所（1534年配分） | イベント | 資源の開始年（`start_year`） | 反映方法 |
|---|---|---|---|---|
| `RESOURCE_SILVER` | 但馬 | 生野銀山の本格開発（`EV_IKUNO_SILVER`） | 1542 | reveal |
| `RESOURCE_NITER` | 肥前、豊後、薩摩 | 平戸へのポルトガル船来航と南蛮貿易（`EV_HIRADO_PORTUGUESE`） | 1586 | trade_access |
| `RESOURCE_SILVER` | 石見 | 毛利氏の石見銀山掌握（`EV_MORI_IWAMI`） | 1526 | ownership |
| `RESOURCE_IRON` | 出雲、伯耆、石見 | 月山富田城の開城と尼子氏の滅亡（`EV_AMAGO_FALL`） | 1534 | ownership |
| `RESOURCE_JAC_GOLD` | 佐渡 | 上杉景勝の佐渡平定と西三川砂金（`EV_SADO_SUBJUGATION`） | 1593 | reveal |
| `RESOURCE_JAC_LACQUER` | 陸奥 | 蒲生氏郷の会津入封と漆器産業（`EV_AIZU_LACQUER`） | 1590 | reveal |
| `RESOURCE_JAC_GOLD` | 佐渡 | 相川金銀山の発見（`EV_AIKAWA`） | 1596 | reveal |

- 地点と開始年は [資源配置の候補データ](../spec/data/japan-map-resources.json) に合わせ、`tests/test_sengoku_events.py` で照合する。開始時に地図にある資源（開始年が1534年以前、または開始時に置く扱い）は `reveal` にしない。
- 石見銀山（1526年）は開始時から地図にある（SSOT 3.2 で銀を温帯の丘にも置けるようにした）。灰吹法（T09、1533年導入、開始前）は3.2『開始前から石見に伝わる技術』。`EV_MORI_IWAMI` は所有移転で、資源ノードの追加ではない。
- 黒川・中山の金は開始時から地図にあるため、`EV_KOSHU_GOLD` はタイルを追加しない。
- 会津の漆は1590年に現れる（資源配置では開始時に非表示）。1589年の `EV_SURIAGEHARA` の時点では地図に無いので、漆の所有移転は扱わない。
- 硝石の平戸での交易アクセスは、資源配置と同じく1586年から。1550年の `EV_HIRADO_PORTUGUESE` は来航の通知と Boost だけ。長崎（`EV_NAGASAKI`）は資源配置に無いため交易アクセスを付けない。どちらを早めるかは Q55。
- `reveal` の早期出現は、資源配置の `REVEAL_TECH`（`tools/japan_map_scale/generate.py`）の研究を最初の勢力が終えた時点。`earlier_by` はこの研究を含む。それ以外の `earlier_by` は Boost の候補で、資源を早く出さない。
- 鶴子銀山（佐渡・1542年ごろ）と相川（1596年発見）は別の資源で、採用の15 kmの地図では隣り合う別タイル。西三川砂金（1593ごろ）は `EV_SADO_SUBJUGATION`。
- 足尾銅山（1550年発見説）、土肥・宝達金山（1577／1586年）、伊勢湾の捕鯨（16世紀後半）、延沢銀山（1596〜1615）は、年代・確度が低く、イベントとしては採用しなかった。地図側で `start_year` を持たせる扱いに任せる（japan-map-resources.md 参照）。
- 塩・茶・絹・米・馬・魚は、特定の年に『開発』されたと言える出来事が乏しく、イベントにしなかった。茶は北野大茶湯（`EV_KITANO_TEA`）で価値の上昇として扱う。

## 7. 家固有イベントの名称対応

| 固有の名 | 家 | 一般名 |
|---|---|---|
| 塵芥集の制定（伊達の分国法） | 伊達 | 分国法の制定／Domain Law Enacted／Принятие законов владения |
| 甲州法度之次第の制定 | 武田 | 分国法の制定／Domain Law Enacted／Принятие законов владения |
| 安土城の築城 | 織田 | 大規模城郭の築城／Grand Castle Construction／Строительство великого замка |
| 大坂城の築城 | 織田（羽柴→豊臣に切り替えた後） | 巨大城郭の築城／Grand Castle Construction／Строительство великого замка |
| 北野大茶湯 | 織田（羽柴→豊臣に切り替えた後） | 大茶会／Grand Tea Gathering／Большое чаепитие |
| 長宗我部氏掟書（百箇条） | 長宗我部 | 分国法の制定／Domain Law Enacted／Принятие законов владения |

## 8. 採用しなかった候補

関ヶ原の戦い（1600）は SSOT 3.2 のオーナー判断（天下統一でゲームが終わるため入れない）で外した。ほかに、60件を超えないよう次を候補から外した（必要ならJSONに追加できる）：守山崩れ（1535）、花倉の乱（1536）、天文法華の乱（1536）、第一次国府台合戦（1538）、河越城の攻略（1537）、河越夜戦（1546）、甲駿同盟（1537）、第一次月山富田城の戦い（1542）、武田信虎の追放（1541）、遣明船（1547）、観音寺騒動（1563）、姉川・三方ヶ原・賤ヶ岳・小牧長久手・耳川・手取川の各戦い、永禄の変（1565）、比叡山焼き討ち（1571）、天正壬午の乱（1582）、備中高松城（1582）、伊達政宗の家督（1584）、大友宗麟の受洗（1578）、天正遣欧使節（1582）、リーフデ号（1600）、朱印船貿易（1592）、慶長豊後地震（1596）など。

## 9. オーナー確認事項

未決の事項は [質問リスト](../questions.md) の Q40〜Q43・Q55 に移した（豊臣期の固有名、楽市楽座の代表年、カトリックの登場の順、旧領の扱い、硝石の交易アクセスの開始年）。松平→徳川、本能寺の変の後の織田、室町幕府の終わり、島津の当主、関ヶ原は SSOT 3.1・3.2 で決まっている。

## 10. 実機確認の項目（実装後）

1. イベント通知が勢力ごとに表示されること（全勢力向け1件、該当勢力向け1件）。万世紀ルールセットの戦国パックと15 kmの日本マップ以外（JSON の `_schema.scope`）では発火しないこと。
2. `earlier_by` の研究完了時に、史実年より前にイベント効果が有効になること。
3. 家固有名（安土城の築城→大規模城郭の築城など）が、持ち主の家と他の勢力で正しく切り替わり、太閤検地はすべての家に『太閤検地』と出ること。
4. 資源イベント後に該当タイルの資源が現れること（`reveal`）。
5. 史実の年に、`boost` の研究を終えていない影響勢力だけにひらめき／着想が付き、公式の Boost 画面が出ること。研究済み・Boost 済みの勢力には付かないこと。
6. `advisor_popup` のあるイベントで、人間の勢力にだけ助言ポップアップが出て、薦めた行動（和睦など）が日英露で表示されること。文字列から組み立てた `AdvisorItem` で、チュートリアル外でも開けること。
7. イベントの発火後にセーブ・ロードしても、同じイベントが再び発火せず、Boost・家名の切り替え・資源の出現が二重に起きないこと。発火前のセーブをロードした場合は、同じ年・月に1回だけ発火すること。
8. マルチプレイで、全端末が同じターンに同じイベントを発火し、Boost・家名・資源・外交状態が全端末で一致すること（SSOT 16 の「最初からマルチプレイで同期ずれしない書き方」の前提。[PoC README「マルチプレイ同期と決定性」](../poc/README.md#マルチプレイ同期と決定性)）。
