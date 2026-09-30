# 指導者の人物化 PoC

加入した22勢力の指導者を偉人にし、人物別の総督称号費用で同名の総督へ進化させる。全員に性格・固定特性と、都市に作用する基礎技能＋2昇進を実装した。実機での合格はまだ確認していない。

2026-10-01 16:08 JSTのオーナー回答「1b費用は人物によって変動する / 2c」を反映した。仕様の正本は [4.7](../spec/ssot/04-factions-characters.md#47-1534年の22指導者の初期設計) と [9.4「指導者の進化」](../spec/ssot/09-assimilation-religion.md#指導者の進化)。本書は史料・公式データとの比較と検証方法を記録する。

対象MODは `poc/JAC_PoC_LeaderPersona/`、IDは `91cf7b65-54bb-4f69-a452-19a65527cde2`。万世紀の日本15 km・20 kmと各開拓者開始だけで読み込む。戦国ツリーの後、LoadOrder=400で追加する。3.2で停止した公式の称号獲得源は戻さず、PoCの検証用付与を通常ゲームの獲得源にはしない。

## 22人の設計と1534年の根拠

対象は [指導者肖像](leader-portraits.md) の22人。信秀・信虎・証如を使い、後継世代の実績を転用しない。蘆名盛舜の内部ID末尾ASHINA_MORIKIYOは既存参照を維持する。年齢は原則1534−生年の概数。

**性格・適性・名声の段階・技能は、史料を手掛かりにした暫定のゲーム設計であり、心理・知名度・数値能力の史実認定ではない。** 年少者・補佐を受けた当主・タリコナなどの性格は特に根拠が薄く、役割からの推定である。事典リンクは小学館・平凡社・講談社・山川の人物項目を閲覧した。公的資料は静岡県立中央図書館、沖縄県立図書館、上ノ国町。

名声の段階FameTierは0＝地域・一家の中核、1＝複数国の軍事・政治関係や中央での活動、2＝将軍・広域教団・国王の象徴的役割。採点はバランス用。甲斐統一の信虎は0、将軍還京支援のある孝景は1とした。

全技能は**定着した一都市だけ**に作用する。基礎は任命時に取得する。2昇進はいずれも基礎だけを前提とする独立した枝（Level=1、列0と2）で、各1称号、両方を取得可能。配置・定着・再配置・昇進は公式総督の処理で保持し、Luaで産出を毎ターン加算しない。

| 人物 | 約年齢 | 1534年の役割 | 名声 | 進化費用（称号） | 性格／固定特性 | 基礎技能 | 昇進1 | 昇進2 | 1534年までの根拠・留保・出典 |
|---|---:|---|---:|---:|---|---|---|---|---|
| 伊達稙宗 | 46 | 当主 | 1 | 2 | 冷静／策士 | 南奥の秩序（文化力+10%） | 和親の政（快適性+1） | 所領の勘定（ゴールド+10%） | 陸奥守護。1532年桑折西山へ移転、1533年「蔵方之掟」。婚姻を通じた南奥の関係から秩序・和親・会計を設計。1536年「塵芥集」は開始時の実績ではない。 [資料](https://kotobank.jp/word/伊達稙宗-93717) |
| 蘆名盛舜 | 44 | 当主 | 0 | 1 | 慎重／凡人 | 会津の備え（食料+10%） | 黒川の整備（生産力+10%） | 家中の安定（快適性+1） | 会津黒川の当主。1521年継承、家中の反乱鎮圧、1528年伊達の葛西攻めを支援。家を維持する立場から備蓄・普請を設計。盛氏期の拡張は遡及しない。 [資料](https://kotobank.jp/word/蘆名盛舜-1049893) |
| 北条氏綱 | 47 | 当主 | 1 | 2 | 冷静／教者 | 小田原の勘定（ゴールド+10%） | 城下の整備（生産力+10%） | 家名の継承（文化力+10%） | 後北条第2代。1520年検地、1524年江戸攻略、1532年鶴岡八幡宮再建着手。検地・城下整備・家名から設計。1486年生説あり。1537年河越・1538年国府台は除外。 [資料](https://kotobank.jp/word/北条氏綱-132131) |
| 上杉憲政 | 約11（暫定） | 当主 | 1 | 3 | 忠実／凡人 | 管領家の継承（文化力+10%） | 家臣の補佐（快適性+1） | 上野の備蓄（食料+10%） | 1531年家督・関東管領継承とする事典。生年不詳説と1523年説が併存し、約11歳は暫定入力。家臣補佐技能と性格は家の役割からの推定で、本人の心理や手腕は未確認。 [資料](https://kotobank.jp/word/上杉憲政-33790) |
| 武田信虎 | 40 | 当主 | 0 | 1 | 強気／武人 | 甲斐の動員（軍事ユニット生産+20%） | 府中の整備（生産力+10%） | 盆地の兵糧（食料+10%） | 甲斐守護。1507年継承、1519年躑躅ヶ崎移転、1532年頃までに甲斐統一。1498年生説もある。晴信の後年の制度・騎馬軍団は転用しない。 [資料](https://kotobank.jp/word/武田信虎-93064) |
| 長尾為景 | 不詳 | 当主 | 1 | 2 | 強気／武人 | 越後の統率（軍事ユニット生産+20%） | 春日山の勘定（ゴールド+10%） | 城下の再建（生産力+10%） | 越後守護代。1507年守護交替、1528年幕府から家格上昇、越中でも軍事活動。生年不詳として年少加算なし。越後全域の完成した支配とは扱わない。 [資料](https://kotobank.jp/word/長尾為景-17157) |
| 今川氏輝 | 21 | 当主 | 1 | 2 | 慎重／教者 | 駿府の礼法（文化力+10%） | 駿河の勘定（ゴールド+10%） | 補佐の政（快適性+1） | 1526年継承、母寿桂尼が政務を補佐。礼法・会計・補佐を家の運営として設計し、本人独力の実績にしない。個人の性格の直接史料は薄い。県立図書館のPDF p.4。 [資料](https://www.tosyokan.pref.shizuoka.jp/data/open/cnt/3/1649/1/4-2.pdf) |
| 織田信秀 | 23 | 当主 | 0 | 1 | 強気／策士 | 津島の商い（ゴールド+10%） | 尾張の普請（生産力+10%） | 勝幡の動員（軍事ユニット生産+20%） | 弾正忠家の当主。勝幡を基盤とする尾張の有力者で、尾張全土の支配者ではない。流通圏との関係を商業技能の手掛かりとするが、1534年の掌握範囲・那古野攻略年は留保。1508・1510年生説もある。 [資料](https://kotobank.jp/word/織田信秀-40523) |
| 松平清康 | 23 | 当主 | 0 | 1 | 勇敢／武人 | 三河の動員（軍事ユニット生産+20%） | 岡崎の整備（生産力+10%） | 西三河の兵糧（食料+10%） | 1523年継承、1524年岡崎移転、1529年以降東三河へ進出。動員・城下整備から設計。1535年守山崩れは未来の事件。 [資料](https://kotobank.jp/word/松平清康-136642) |
| 朝倉孝景 | 41 | 当主 | 1 | 2 | 冷静／教者 | 一乗谷の学芸（文化力+10%） | 越前の勘定（ゴールド+10%） | 家中の和（快適性+1） | 1493年生の越前当主。1513年将軍還京支援、1527年三好方と交戦。京都との関係・一乗谷の運営から文化・会計を設計。1428年生の同名人物やその家訓は転用しない。 [資料](https://kotobank.jp/word/朝倉孝景-25008) |
| 足利義晴 | 23 | 将軍 | 2 | 3 | 忠実／教者 | 将軍家の儀礼（文化力+10%） | 幕臣の結束（快適性+1） | 御料所の勘定（ゴールド+10%） | 1521年就任の第12代将軍。1534年開始時は近江流寓の時期とし、年内帰京と分ける。将軍号の権威から儀礼・幕臣・御料所を設計し、実効支配力とは区別する。 [資料](https://kotobank.jp/word/足利義晴-25238) |
| 三好長慶 | 12 | 当主 | 0 | 2 | 慎重／策士 | 若き家の再建（生産力+10%） | 家臣からの学び（文化力+10%） | 阿波の支え（快適性+1） | 1532年父の死後継承の約12歳の当主。1533年本願寺と晴元の和睦仲介とされるが独力の交渉とは断定しない。再建・家臣からの学びを設計し、後年の畿内覇権は転用しない。 [資料](https://kotobank.jp/word/三好長慶-16803) |
| 六角定頼 | 39 | 当主 | 1 | 2 | 冷静／教者 | 近江の流通（ゴールド+10%） | 観音寺の普請（生産力+10%） | 和議の作法（文化力+10%） | 南近江当主。1518年継承、1532年から義晴を桑実寺に迎え、同年山科本願寺攻撃。流通・普請・政治調整から設計。1546年の管領代・元服親は遡及しない。 [資料](https://kotobank.jp/word/六角定頼-153000) |
| 証如 | 18 | 門主 | 2 | 3 | 温厚／霊神 | 門徒の結集（信仰力+10%） | 門徒の相扶（快適性+1） | 教えの継承（文化力+10%） | 1525年継承の本願寺第10世。1532年山科焼失後石山へ移り、加賀・畿内の教団を維持。門徒・相扶・教えを技能化。顕如の後年の石山合戦とは別。 [資料](https://kotobank.jp/word/証如-1082522) |
| 大内義隆 | 27 | 当主 | 1 | 2 | 陽気／教者 | 山口の学芸（文化力+10%） | 西国の商い（ゴールド+10%） | 典籍の保護（科学力+10%） | 1528年継承、初政の領国運営と1534年北九州出兵。家の文化・流通基盤から学芸・典籍保護を設計。1543年敗戦後の文化偏重、1550年頃の宣教許可は遡及しない。 [資料](https://kotobank.jp/word/大内義隆-17848) |
| 尼子経久 | 76 | 当主 | 1 | 2 | 冷静／策士 | 月山の備え（生産力+10%） | 国人の統率（軍事ユニット生産+20%） | 出雲の勘定（ゴールド+10%） | 富田を基盤に大永年間までに山陰・山陽へ拡張した当主。1537年孫への継承前。「11か国」を一様な直轄領とせず、備え・国人動員・勘定を設計。 [資料](https://kotobank.jp/word/尼子経久-27145) |
| 毛利元就 | 37 | 当主 | 0 | 1 | 慎重／策士 | 郡山の備蓄（食料+10%） | 国人との和（快適性+1） | 小領の結束（軍事ユニット生産+20%） | 1523年継承、1529年高橋氏攻略。1534年は安芸吉田の国人で大内方。小領の備え・国人関係から設計。厳島1555年、三子教訓状1557年は根拠にしない。 [資料](https://kotobank.jp/word/毛利元就-16902) |
| 長宗我部国親 | 30 | 当主 | 0 | 1 | 勇敢／凡人 | 岡豊の再興（食料+10%） | 旧領の整備（生産力+10%） | 家臣の再集結（軍事ユニット生産+20%） | 一条房家の庇護を受け、1518年岡豊へ復帰して旧領を回復。再興・普請・家臣再集結から設計。元親による土佐・四国統一は転用しない。 [資料](https://kotobank.jp/word/長宗我部国親-1091612) |
| 大友義鑑 | 32 | 当主 | 1 | 2 | 冷静／教者 | 府内の勘定（ゴールド+10%） | 守護の政（文化力+10%） | 豊後の備え（軍事ユニット生産+20%） | 大友第20代。継承は1515年頃・1518年説。1534年大内と抗争し勢場ヶ原合戦。府内の守護行政・兵備から設計し、宗麟のキリスト教政策は転用しない。 [資料](https://kotobank.jp/word/大友義鑑-17574) |
| 島津貴久 | 20 | 当主 | 0 | 1 | 勇敢／武人 | 伊作の結束（軍事ユニット生産+20%） | 相州家の備蓄（食料+10%） | 家の再建（生産力+10%） | 採用済みの島津代表。1526年勝久の養子、1527年守護継承説の後、父忠良と継承抗争。1534年の安定した三州統一当主とはしない。鉄砲1543年・宣教1549年は除外。 [資料](https://kotobank.jp/word/島津貴久-75142) |
| 尚清王 | 37 | 国王 | 2 | 3 | 温厚／教者 | 海路の交易（ゴールド+10%） | 冊封の儀礼（文化力+10%） | 王府の調和（快適性+1） | 1527年即位の第二尚氏第4代国王。1534年明の冊封。王府・海路交易・儀礼から設計。1537年奄美の軍事行動は除外。冊封が開始日までに完了済みとは断定しない。 [資料](https://kotobank.jp/word/尚清王-1082385)・[県立図書館年表](https://www.library.pref.okinawa.jp/archive/contents/chronology.html) |
| タリコナ | 不詳 | ゲームの全域代表 | 0 | 2 | 慎重／凡人 | 集落の備え（食料+10%） | 地域の交換（ゴールド+10%） | 共同の支え（快適性+1） | 町の年表には1536年の西部の長として登場。1534年の地位・生年・全道統治は未確認。ゲームの全域代表として集落・交換・相扶を設計。実在しないとも断定しない。 [資料](https://www.town.kaminokuni.lg.jp/hotnews/detail_sp/00000312.html) |

開始状態の役割・本拠・所領配分の留保は [1534年開始の調査](../research/japan-1534-start.md) に従う。技能名は加入後のゲーム上の役割を表すもので、その名称の政策や施設が1534年に実在したと主張しない。22人の技能はHeroと相互作用しない。Heroを用いる技能では公式「英雄と伝説」モードを必須にする。

## 人物別の進化費用

費用式と定数の正本は9.4。SQLのJAC_PersonaCostParametersに名前付き暫定定数を置き、初期属性からEvolutionCostを算出する。

```text
費用 = clamp(MIN_TITLES, MAX_TITLES,
 BASE_TITLES + FameTier × FAME_TITLE_WEIGHT
 +（Age1534が判明しYOUTH_AGE_LIMIT未満ならYOUTH_TITLE_SURCHARGE）
 +（Role1534=REPRESENTATIVEならREPRESENTATIVE_TITLE_SURCHARGE）)
```

| 名前付き暫定定数 | 値 | 用途 |
|---|---:|---|
| BASE_TITLES | 1 | 任命分 |
| FAME_TITLE_WEIGHT | 1 | 名声の段階ごとの加算 |
| YOUTH_AGE_LIMIT | 16 | 年少とする境界。史実の元服年齢の確定ではない |
| YOUTH_TITLE_SURCHARGE | 1 | 補佐・育成を要する年少者の加算 |
| REPRESENTATIVE_TITLE_SURCHARGE | 1 | 単一当主が未確認の全域代表の加算 |
| MIN_TITLES / MAX_TITLES | 1 / 3 | 費用範囲 |

伊達は1＋名声1＝**2**、織田は1＋名声0＝**1**。上杉は暫定11歳を入力し1＋名声1＋年少1＝**3**。長尾・タリコナの年齢不詳には年少加算なし。タリコナは全域代表加算で**2**。島津の継承抗争は史料の留保であり、全域代表加算を適用しない。

使える称号は公式画面のGetGovernorPoints()−GetGovernorPointsSpent()（G/DLC/Expansion2/UI/Additions/GovernorPanel.lua:54-61）。進化前に全額を確認し、追加の費用−1をChangeGovernorPoints(-追加分)で確保する。**公式の獲得数を減らすことで称号残数を消費**し、使用済み数は独自に書き換えない。APPOINT_GOVERNORが残る1個を消費する。普通の昇進は公式のPROMOTE_GOVERNORと各1称号（同:547-577）。

正数の付与例はG/Debug/Player.ltp:289-295、獲得数・使用済み数は同:654,676。**負数の受理は正数例だけでは証明できず、要実機確認**。減算直後の残数が追加費用分だけ減らなければTITLE_PAYMENT_FAILEDで任命を止める。シナリオが追加1個の減算、任命・昇進の会計と保存後の残高を断言する。

## 公式総督との効果量の比較

G/は下のインストールディレクトリ。

| 実装した型・値 | 公式の一次資料と比較 |
|---|---|
| MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER。Amount=10、YieldType=対象産出 | ピンガラは同型で文化・科学+15%。G/DLC/Expansion2/Data/Expansion1_Governors.xml:724-729,1092-1109。食料・ゴールド・生産・信仰への割当は同型の引数を用いたMOD設計で、公式ピンガラの能力とは主張しない |
| MODIFIER_SINGLE_CITY_ADJUST_AMENITIES_FROM_GOVERNORS。Amount=1 | 拡張1のアマニ+4。昇進接続・型・引数はG/DLC/Expansion1/Data/Expansion1_Governors.xml:273-275,538-541,825-829。GSではその昇進を削除するが、型はG/DLC/Expansion2/Data/Expansion1_Modifiers.xml:940に残る |
| MODIFIER_GOVERNOR_ADJUST_CITY_MILITARY_UNIT_PRODUCTION。Amount=20 | イブラヒムの基礎と同量。接続・型・引数はG/DLC/Expansion2/Data/Expansion2_Governors.xml:50-53,77-79,104-108。軍事ユニットだけに作用 |

3型をGovernorPromotionModifiersへ直接接続する。対象がCOLLECTION_OWNERである根拠は、産出型がG/Base/Assets/Gameplay/Data/Modifiers.xml:2165-2167、快適性型がG/DLC/Expansion2/Data/Expansion1_Modifiers.xml:940-942、軍事生産型がG/DLC/Expansion2/Data/Expansion2_Modifiers.xml:358-360。基礎・取得済み昇進は赴任完了後に発効し、転任先では赴任し直すという公式説明はG/DLC/Expansion2/Text/en_US/Expansion1_Civilopedia_Text.xml:197,200。自作技能の発効・解除は別途実機確認する。一都市一人のため同じ都市へ重ねられず、全取得でも3つの限定効果。22人の実機収益やAI運用の公平性は未検証。

## エンジンの根拠

以下の `G/` は `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/`。すべてインストール済み公式ファイルの行番号であり、ゲーム本体は変更していない。ソースから読み取れることと実機で確かめたことは区別する。

| 使用する仕組み | 公式ファイルと根拠 |
|---|---|
| 所属・首都・所有ユニット | `G/DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:1200,1389`: 文明と指導者の種類をPlayerConfigurationsから読む。`G/DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:1103`: 首都を取得する。`G/DLC/WarMachineScenario/Scripts/WarMachineScenario.lua:186`: 所有ユニットのMembersを走査する |
| 偉人クラス・個人の定義 | `G/Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:1502,1519`: クラスにUnitType・AvailableInTimeline、個人にName・EraType・ActionChargesがある。`G/Base/Assets/Gameplay/Data/GreatPeople.xml:20` に将軍クラスと `UNIT_GREAT_GENERAL` の対応がある |
| 個人を指定した偉人の生成 | `G/Debug/Map.ltp:577,579`: 個人の行の **Index** を選択する。`G/Base/Assets/UI/Tuner/TunerMapPanel.lua:190`: `Game.GetGreatPeople():CreatePerson(playerID, individualIndex, x, y)` を呼ぶ。戻り値は使わず、生成後に所有ユニットを調べる |
| 本物の偉人か、どの人物かを確認する | `G/Base/Assets/UI/PortraitSupport.lua:50-52`: `unit:GetGreatPerson():IsGreatPerson()` と `GetIndividual()`。`G/Base/Assets/UI/Panels/UnitPanel.lua:2273-2285`: 個人・クラス・残りの使用回数を読む |
| 偉人を取り除く | `G/Base/Assets/UI/Tuner/TunerMapPanel.lua:197-199`: `Players[owner]:GetUnits():Destroy(unit)`。任命成功の確認前には呼ばない |
| 総督と昇進ツリーを追加する | `G/DLC/Expansion1/Data/Expansion1_Schema.sql:131,160,183,190`: Governors、GovernorPromotions、GovernorPromotionPrereqs、GovernorPromotionSets。`G/DLC/Expansion1/Data/Expansion1_Governors.xml:69-75`: 通常総督のIdentityPressure=8・TransitionStrength=100。`G/DLC/Expansion1/Data/Expansion1_GlobalParameters.xml:70`: 定着の基準は5ターン |
| 任命と昇進を要求する | `G/DLC/Expansion2/UI/Additions/GovernorPanel.lua:219-223,547-577`: ボタンと `APPOINT_GOVERNOR`・`PROMOTE_GOVERNOR` の要求。昇進ボタンの値も `G/DLC/Expansion2/UI/Additions/GovernorDetailsPanel.lua:197-199` で確認できる。要求の総督ID・昇進IDは **Index**。エンジンが称号を処理する |
| 都市へ配置・再配置する | `G/DLC/Expansion1/UI/Additions/GovernorAssignmentChooser.lua:372-382`: `ASSIGN_GOVERNOR` に総督Index・都市の所有者・都市IDを渡す。`:180` の `CanAssignGovernor(hash, city)` で派遣可否を調べる |
| 実際の総督と昇進・定着を確認する | `G/DLC/Expansion2/UI/Additions/GovernorPanel.lua:58,536`: `GetGovernorList()` と `GetGovernor(hash)`。`G/DLC/Expansion1/UI/Additions/GovernorSupport.lua:75-86`: `GetAssignedCity()`・`IsEstablished()`・`GetTurnsOnSite()`。`G/DLC/Expansion1/UI/Additions/GovernorAssignmentChooser.lua:90,126`: `GetAssignedGovernor(city)`・`HasPromotion(promotionHash)` |
| 検証用の称号を与える | `G/Debug/Player.ltp:289-295`: `player:GetGovernors():ChangeGovernorPoints(1)`。同ファイル`:654,676`に取得済み・使用済みの称号を読む処理がある。検証用の称号は人物・用途ごとに1回だけ与える。任命は人物の費用分、昇進は1個 |
| 総督数の上限 | `G/DLC/Expansion2/Data/Expansion2_GlobalParameters.xml:200`: GSの上限は8。PoC対象地図で22人物＋8枠の30へ広げる。22人同時任命は要実機確認 |
| UIからゲーム側への変更要求 | `G/DLC/BlackDeathScenario/UI/BlackDeathScenario_PopupHandler.lua:21-28` の `EXECUTE_SCRIPT` と `OnStart`。同 `Scripts/BlackDeathScenario.lua:66` と `BlackDeathScenario_Support.lua:116`: ゲーム側はエンジンが渡す要求元プレイヤーを受け取る。要求のパラメータに所有者を指定させない |
| 人物の独自状態を記録する | `G/DLC/WarMachineScenario/Scripts/WarMachineScenario.lua:39-42`: Gameプロパティを読み、未初期化のときだけ設定する。PoCは同じAPIで人物の記録を保持する。セーブ後の復元は本シナリオで要実機確認 |
| ゲーム側での再確認 | `G/DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:246`: `GameEvents.OnGameTurnStarted`。ゲーム側でUI専用のロード終了イベントは購読しない |
| ターンが有効なときだけUIの要求を送る | `G/Base/Assets/UI/ActionPanel.lua:619`: `player:IsTurnActive()`。`G/Base/Assets/UI/Choosers/CivicsChooser.lua:464`: `Events.GameCoreEventPublishComplete`。`G/DLC/Expansion2/UI/Additions/GovernorPanel.lua:742,746`: 総督変更・人間のターン開始のイベント |
| 偉人の選択欄と総督画面に同じ肖像を出す | `G/Base/Assets/UI/Panels/UnitPanel.lua:2229`: 個人の肖像検索を上書きできる。`G/DLC/Expansion2/UI/Additions/GovernorPanel.lua:127-139` と `GovernorDetailsPanel.lua:129`: 総督の肖像コントロール。公式画面を読み込んだ後、対応する既存指導者アイコンに替える |
| アイコンのキーと小さな都市表示 | `G/Base/Assets/Database/IconManager.sql:31-35`: PKは(Name, Atlas)。`G/DLC/Expansion1/Data/Expansion1_Icons_Governors.xml:10-11,27-39`: 都市表示に22 pxのfill/slotが必要。同じ名前の異なるサイズを別Atlasで用意する |

上記はAPI呼出しとデータ定義の一次資料による根拠であり、このMODの実機合格の代わりにはならない。特に専用偉人クラス、ゲーム側からの総督参照、UI差し替えと画像の表示、称号の消費、セーブ後のエンジン状態をシナリオで確認する。

## 状態と失敗時の処理

加入は既存の加入機能が成立を決めた後のJoin(owner, source, personality, trait)で行う。所有者・指導者・文明を検査し、本物の偉人の個人IDが一致したときだけ記録する。性格指定を省けば設計値を使い、固定特性を別の値に差し替える新規加入は拒否する。世界で人物の所有者は1人。再加入で増殖や固定特性の変更は起きない。元の勢力の都市・外交・存亡はこの接続口では変えない。

未加入 → GREAT_PERSON → EVOLVING → GOVERNORの順に進む。名前・肖像・性格・固定特性・費用はGameプロパティへ保存する。初期費用は加入時に保存し、保存後の初期データ調整や性格変更では変えない。全額の残数を確認して追加費用とEVOLVINGを保存する。遅延や別操作が残る1個を使った場合、人物と予約済み追加費用を保持して称号を得た次の手番で再試行し、追加費用は取り直さない。

同名のエンジン総督を確認した後だけ偉人を除去し、除去も確認してからGOVERNORにする。除去失敗で再徴収しない。定着・配置・昇進は公式総督から読む。UNITや未設定はnilで保存し、空文字が保存をまたいで残らないという実機確認済み事実に従う。InitUnitValidAdjacentHexの戻り値をUnitにせず、偉人の個人IDで所有ユニットを探す。

## 実機シナリオ

このセッションでは実機を起動していない。メインセッションがMODを反映し、次を実行する。

```powershell
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/leader-persona-15km.json
```

北条を人間、伊達と織田をAIにする日本15 kmの史実領土開始。141ステップ、end_turn合計16ターン（偉人保持1＋伊達定着5＋伊達再定着5＋織田定着5）。このブランチのドライバーはend_turnを1回ずつ実行するため、織田の定着も5ステップに分割する。総督の詳細画面も開いて肖像を記録する。EXECUTE_SCRIPTは人間の手番が有効になってから送る。

1. 22人の定義・加入前の不在・称号獲得源の停止を確認する。
2. 伊達の偉人が1体で、名前・肖像・使用回数0・性格・固定特性が一致し、1ターンと保存後も残る。
3. 称号0個と1個では伊達の進化を拒否し、人物と称号を保持する。2個で追加1個＋公式任命1個を消費する。
4. 未配置・定着途中・定着後・再配置を確認する。伊達の基礎文化+10%は定着後だけ、旧都市では再配置後に無効になる。
5. 伊達の快適性+1、ゴールド+10%を各1称号で取得する。
6. 織田は1称号で進化、追加費用0。基礎ゴールド+10%、昇進の生産力+10%・軍事ユニット生産+20%を取得する。
7. GameEffectsのModifierId・Active・対象の所有者と都市・引数を読む。有効な6技能の実体が各1個、未定着・未取得は0個と断言し、独自プロパティや昇進フラグだけで合格にしない。
8. 偉人、未配置総督、定着途中、再配置直後、伊達の昇進・定着後、2人の全技能取得後の保存・読込に加え、3称号の上杉憲政を任命後にも保存・読込して計7回を確認する。2人の確認時点では残数0、公式使用済み6個、伊達追加1個。最後に上杉憲政を3称号で進化させ、追加2個＋公式任命1個を検証する。最終会計は付与10個＝公式使用済み7個＋追加消費3個で残数0。保存後の再進化・再付与でも変わらず、伊達・織田の2都市の技能効果も保持する。

ModifierのID・Active・対象一覧の公式根拠はG/Debug/Modifiers.ltp:233-247,273-280、対象所有者はG/Debug/Diplomacy.ltp:398。都市名はその都市の表示名と照合する。開始時に全ての自都市の表示名が重複しないことを検査し、重複ならそこでエラーにする。

## 自動検証と範囲

tests/test_leader_persona.pyは公式GSスキーマと戦国ツリー削除後のSQLで、22初期属性・費用・66技能・公式型と効果量・外部キー・ja/en/ruを検査する。全シナリオLuaをlupaで構文検査する。tests/test_leader_persona_lua.pyは実際のゲーム側・UI側Luaをlupaで実行し、称号不足・予約費用・別操作による消費・再読込・遅延任命・除去失敗・固定値・伊達と織田の2枝を検査する。生成は戻り値なし、IndexとHashは別値。実機効果の代替検証ではない。

指定のunittest discoverコマンドを作業フォルダから実行し、追加のTemp補助ランナーなしで検証する。

この変更の検証結果：全体362件、実機ドライバー150件が成功した。人物化36件（Lua25件を含む）は全体の内数。シナリオの構文・形式検査も成功した。別モデルのレビューではMustなし。提案された費用上限の実機検査、同名都市の検査、3言語での初期性格の明示を反映し、再レビューでもMustなしだった。

- 加入条件・吸収・後継者・AIの加入と任命・他国都市派遣は範囲外。全員AssignCityState=0。
- 公式の称号獲得源は戻さず、検証用称号を通常の報酬にしない。
- 準備中の任命の取消はない。非同期の任命要求が残る間に返金すると、返金後に任命されうるため、人物と予約を保持して再試行する。
- 外部Luaが加入・費用検査を通らず直接任命を要求する経路まで、エンジン側で禁止する実装ではない。マルチプレイ・AI任命・途中導入・旧PoC保存からの移行は未検証。
- 偉人の地図モデルは公式大将軍、肖像は対応する既存指導者画像。22 px都市表示だけ汎用fill/slotを借りる。[画像資料](leader-portraits.md)参照。
- 偉人を失った場合は自動復活せず、PERSON_MISSINGで進化拒否。
- 負数による称号減算、技能の発効・解除、保存後のエンジン状態・画像は要実機確認。残り20人の実機効果と22人同時任命はこの2人のシナリオで検証しない。
