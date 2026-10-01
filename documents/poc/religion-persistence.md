# PoC 束2：宗教持続

オーナーの2026-10-01 16:07 JSTのDiscord回答を、再興と発見後の8分岐に反映した。仕様の正本は[SSOT 9.2・9.3](../spec/ssot/09-assimilation-religion.md)。以下の9件は提案から削除済みで、確定回答の反映表とする。D8の宗教・教義・人物の内容は未決のまま扱う。

| 回答 | 反映した結果 |
|---|---|
| 1：再興b、資料を使い切らない | 信徒1人、信仰20。資料を保有し、再び信徒0になった後も同じ資料で再興可能 |
| 2：売却 | ゴールド500 |
| 3：秘匿a | 費用・産出0。後日の再興用に保有 |
| 4：研究bとEureka2件、資料を使い切る | 科学10・文化10。未研究かつ未Eurekaの技術2件を識別子順に選んでEurekaを発生。資料は消費 |
| 5：異端化 | 信仰400を払い文化20。資料は消費し、解釈と来歴は履歴に保存 |
| 6：吸収dを変更 | 信仰500を払い、対象宗教の世界の市民信徒数を3で割って切り上げた人数を、自分が創始した宗教へ変換 |
| 7：Artifact a | 文化30、PoCのVaultへ1件 |
| 8：外交事件は返還 | 発祥国から現在の金と外交的支持の半分を移転。宗教同盟を結び、相手の宗教から自国民への弾圧と、自国から相手宗教への弾圧を禁じる |
| 9：英雄d、公式モード必須 | 公式の英雄を加入させ、実行直前の毎ターンの信仰創出量の10倍を信仰として獲得。斥候による代用と加入費用は廃止 |

独立MODは`poc/JAC_PoC_ReligionPersistence/`、IDは`9b0b9a39-bd72-4ff1-9062-05eeac09b693`。Gathering Storm・万世紀・Babylon DLCに依存し、`RULESET_JAC_BANSEIKI`でのみ読み込む。宗教PoCはゲーム作成時に公式のHeroes & Legendsを有効化する必要がある。シナリオで`GAMEMODE_HEROES`を設定し、DLCのMODも有効化する。SSOT 8.4にも、Hero・英雄を使う箇所では公式モードが必須であることを明記した。

## 効果と接続範囲

再興は新しい集団を信徒1人・施設0で記録する。教典・遺物・記録・人物が対象で、聖地だけでは再興しない。資料を使い切らないため、同じ資料も宗教ごとの10ターンの間隔と信徒0と信仰20の条件を満たせば再利用できる。秘匿した資料にも同じ再興条件を適用する。発見1件につき1選択、再興以外は勢力×宗教×分岐で1回までという制限は保持する。

研究はEureka定義を持つ未研究・未発生の技術を`TechnologyType`順に選ぶ。候補が2件に満たなければ実行しない。科学と文化の一括産出にはそれぞれ別のModifierを使い、20宗教×4種で80件を定義した。研究・異端化の資料は台帳から削除し、使用履歴と削除済みIDを残す。再登録・再発見・二重付与を拒む。

吸収の対応付けには`BindEngineReligion(id, nativeReligionIndex)`を使う。原宗教と吸収先は別の創始済み標準宗教で、吸収先はプレイヤー自身が創始した宗教に一致し、自勢力の信徒を持つ存続宗教であることが前提。`NativeFollowers(id)`は世界のすべての生存プレイヤーの都市から、エンジンの`Followers`を合計する。都市数や多数派の都市数、手動入力したコミュニティ人数は使わない。

合計を3で割って1回だけ切り上げ、プレイヤーID・都市ID順に市民を変換する。圧力は無宗教を含む各宗教の目標人数に比例させるため、布教圧力の過去の比率は保持しない。人口は移動しない。適用後は原宗教の減少、自宗教の増加、他宗教の人数不変をすべて検査する。宗教ごとの圧力は量を指定して減算し、無宗教の圧力を保ったまま正の圧力を再配分する。religion-11・13で2都市にまたがる変換と無宗教の保持を確認した。第三の宗教の保持は追加の実機確認として残る。自動の布教・市民変動とPoCのコミュニティ台帳の同期は後続のアダプターに残す。発見後に標準宗教の信徒が戻っていても、未解決の吸収などは解決できる。

返還では発祥国の残高を減らし、同額を受取人に加える。金は半額、外交的支持は整数のため半分を切り捨てて残りを発祥国に残す。すでに同盟中なら結び直して期限を延長しない。同盟の期限切れ、戦争、どちらかの滅亡で宗教同盟を終了し、通常の同盟を結び直しただけでは宗教同盟を再開しない。

SSOTの全分割ファイルを`hero/Hero/英雄/弾圧`で確認した。Hero型は8.4にあるが、弾圧の効果量と実行コマンドはまだ定義されていない。今回強制するのは、宗教同盟の保存状態と、`CanPersecute(actorPlayer, actingReligion, citizenPlayer, targetReligion)`および`AssertCanPersecute`の拒否である。相手の宗教が自国民を対象にする場合と、自国が相手の宗教を対象にする場合を拒む。将来の弾圧処理は効果の前にこの接続口を通す必要がある。標準の異端審問、宗教戦闘、布教を自動で遮断する処理には未接続。

英雄は未加入の公式英雄を`HeroClassType`順に選ぶ。公式の`HeroClasses.CreationProjectType`から加入Modifierを取得して、公式の拠点建物を持つ都市へ付ける。新しくできたUnitについて英雄クラス・寿命・加入済み所有者を確認してから、実行前の信仰創出量の10倍を付与する。PoCの生涯1人の制限は保持する。公式モードの寿命・再召喚・能力を使い、独自の人物案は作らない。

AIは人間と同じ前提・費用・上限を検査する。職業の第一候補と、吸収→売却→研究→異端化→Artifact→外交事件→英雄→再興→秘匿の候補順を保持する。自然な探索トリガーは後続のアダプターから接続する。

## 保存と呼び出し

プロパティは`JAC_RP_STATE_V1`、`schema=1`。`PUBLIC/UNDERGROUND/LOST`と、信徒なし・資料あり・地下持続の事実を保持する。資料の履歴、再興、標準宗教への対応、選択の効果、英雄Unit、Vault、宗教同盟と終了時点を保存する。

旧保存は読取用のコピーへ欄を追加するだけで、ロードだけでは書き込まない。旧`DEFERRED`や旧条件で使用済みだった資料は自動で新報酬へ変更しない。今回の条件での再興には`reusable=true`を保存する。未知スキーマを上書きせず、未設定のプロパティには`nil`を使う。

通常の入力は検査後に1回保存する。費用・効果は予約を`PENDING`で保存し、エンジンに適用してから`APPLIED`を保存する。最初の保存失敗なら作用しない。効果・最後の保存が失敗すれば`PENDING`のまま再実行を拒む。残高・Eureka・市民・英雄とGameプロパティは一括で戻せないため、実際の状態を照合して復旧する。ロードやターン開始時に効果を再適用しない。ターン開始処理は宗教同盟の終了だけを更新する。

Gameplay APIは`ExposedMembers.JAC_ReligionPersistence`。`ResolveDiscovery(id, discovery, outcome, options)`と`Revive(id, revival, source, community, player, city)`を使う。`Rules/Inspect/Export`はコピー、`Snapshot`はキー順を固定した台帳文字列。通常のUIからの操作は、後続の同期されたGameplay要求を使い、手番開始前に送らない。今回の単独プレイ用の英雄確認橋はExposedMembersからGameplayの完了処理を呼ぶ。報酬は受取先の手番だけで払い、マルチプレイの同期要求には未接続。

再興は空いているコミュニティIDを選ぶ。信仰支払い中のGameplay呼び出しで予定のIDが埋まった場合も選び直し、実際のIDを再興履歴に記録する。既存の信徒・施設・保有者と別宗教の同名IDを上書きせず、生成したIDも128文字以内に収める。

## 公式ファイルの根拠

基準フォルダは `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI`。今回、本体ファイルは変更していない。

| 使用箇所・仮定 | 公式の参照先 |
|---|---|
| Gameplayからプロパティを変更／読取 | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario_StateUtils.lua:60-78,126,137-141`。Gameへの呼出しは `DLC/WarMachineScenario/Scripts/WarMachineScenario.lua:42` |
| カタログを `GameInfo` の行イテレータで読む | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:207,210,1027`（イベント表の `EventPopupData` も走査） |
| Gameplayの現在ターン | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:413,675` |
| コンテキスト間の共有名 | `Base/Assets/UI/MapTacks.lua:5-11` の `ExposedMembers` による共有の説明 |
| 実機の開拓者による都市建設要求 | `Base/Assets/UI/Panels/UnitPanel.lua:2636,2641` の `UnitManager.RequestOperation(..., UnitOperationTypes.FOUND_CITY)` |
| メニューでのMOD列挙・有効状態・依存関係・切り替え | `Base/Assets/UI/FrontEnd/Mods.lua:67,255,296-327,391,545-569,904`。無効化可否と依存元ID、依存・参照・競合の別配列、設定への反映。IDからのハンドル取得は `Base/Assets/UI/FrontEnd/MainMenu.lua:486-488` |
| 人間のターンが有効になるまで待つ | `Base/Assets/UI/LocalPlayerActionSupport.lua:6-14` の `IsTurnActive()` 判定 |
| 首都取得と聖都指定・宗教創始・信仰追加 | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:1103,137,143-144` |
| 標準の番号付きカスタム宗教枠 | `Base/Assets/Gameplay/Data/Religions.xml:20,127-131` |
| Gameplayの創始済み宗教一覧 | `DLC/Expansion2/Scripts/WorldCongress.lua:29` |
| UIから創始種別・聖都を読取 | `Base/Assets/UI/ReligionScreen.lua:123,795` |
| 標準の信徒を市民単位で数える | `Base/Assets/UI/ReligionScreen.lua:1074,1091-1092` の `GetReligionsInCity()` と `Followers` |

上記はAPI使用例であり、今回の深い入れ子・空テーブル・false値が保存される証明ではない。GameプロパティのLuaテーブルの保存は束0で確認済み（[PoC結果](README.md)、同文書266行）。今回の台帳については下記の実機シナリオを通す必要がある。空文字Gameプロパティが保存されない件は依頼者の検証済み事実に従った。


| 参照 | 値・仮定 | 公式ファイル:行（上記基準フォルダー配下） |
|---|---|---|
| F5 | 一括産出ModifierとRunOnce・Permanent、Amount・YieldType・Scale | `Base/Assets/Gameplay/Data/GoodyHuts.xml:72-107,233-245`、`Base/Assets/Gameplay/Data/Modifiers.xml:2538-2543` |
| API | Gameplayのゴールドと信仰残高・費用 | `DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:764,847`、`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:404,563-565` |
| API | GameplayでModifierを付ける | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:1204-1205` |
| API | 都市の検索、生存・大勢力・AIの検査 | `DLC/VikingsScenario/Scripts/VikingScenario.lua:95,261`、`DLC/PiratesScenario/Scripts/PiratesScenario_StartScript.lua:866`、`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:122` |
| API | 接触・戦争状態の検査、シナリオの接触入力 | `DLC/Expansion2/Scripts/WorldCongress.lua:301`、`DLC/VikingsScenario/Scripts/VikingScenario.lua:77`、`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:1308` |
| 実機準備 | 安価な知識の付与、UIの研究選択・進捗と費用 | `DLC/VikingsScenario/Scripts/VikingScenario.lua:38-39`、`Base/Assets/UI/Screens/TechTree.lua:239-245,1107,1112-1114`、`Base/Assets/UI/Screens/CivicsTree.lua:239-245,1251,1279,1287-1289` |

Gameplayの外交支持は`Player:GetDiplomacy():GetFavor/ChangeFavor`を使う。UIの`Player:GetFavor()`は使わない。この区別、`GetAllianceType`と`CanTriggerBoost`がUI専用であることは依頼者の実機検証済み事実に従う。

| 今回使う処理 | 公式ファイル:行（上記基準フォルダー配下） |
|---|---|
| 技術の既得判定とEureka発生 | `DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:768-784`の`HasTech`と`TriggerBoost(index, 1)`（集落と同じ契機）。同ファイルの`HasBeenBoosted`ではなく、現在のフラグ`HasBoostBeenTriggered`を使う例は`Base/Assets/UI/Panels/TechBoostsPanel.lua:91`。後者のGameplayでの利用はシナリオでも検査する |
| Eurekaを持つ技術の定義 | `Base/Assets/Gameplay/Data/Technologies.xml:455-472`の`Boosts.TechnologyType`。研究済み・発生済み・Eureka定義なしを除き、`TechnologyType`順に2件選ぶ |
| 世界の生存プレイヤーと都市を読む | `DLC/VikingsScenario/Scripts/VikingScenario.lua:224-234,93-95`の`PlayerManager.GetAlive()`と`GetCities():Members/FindID` |
| 吸収対象の市民数と圧力 | `Base/Assets/UI/ReligionScreen.lua:1074-1092`の`GetReligionsInCity()`、`Religion`、`Followers`。圧力欄は`Base/Assets/UI/WorldView/CityBannerManager.lua:1240-1242`。無宗教を含めた合計が都市人口に一致することは要実機確認 |
| 自分が創始した宗教と明示的な対応付け | `Base/Assets/UI/ReligionScreen.lua:123,129`、Gameplayの創始済み一覧は`DLC/Expansion2/Scripts/WorldCongress.lua:29`。`BindEngineReligion`は創始済みの標準宗教だけを登録し、番号付き仮宗教の名前から対応を推測しない |
| 市民の宗教を変えるAPI | `DLC/VikingsScenario/Scripts/VikingScenario.lua:103`の`City:GetReligion():AddReligiousPressure(0, religionIndex, pressure, -1)`。Civ5の`ConvertPercentFollowers`は使わない。元宗教の人数を減らし自宗教を増やす人数配分に圧力を調整する。減算は`RemovePressureOneReligion(religionIndex, pressure, responsiblePlayer)`で行う。正しい引数は本体解析とreligion-10・12の診断で確認し、整数人数の再配分と無宗教の保持はreligion-11・13で確認した。すべての宗教の人数を再読取して検査し、不一致を成功にしない |
| 実機で市民を準備する | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario_Support.lua:266`の`ChangePopulation`、`DLC/NubiaScenario/Scripts/NubiaScenario.lua:820-827`の聖都・創始・`SetAllCityToReligion`。都市全体の変換は準備に限り、吸収そのものには使わない |
| 外交の同盟を作る・存続を読む | `DLC/VikingsScenario/Scripts/VikingScenario.lua:18-20,77`の`SetHasAllied(other, true)`と`HasAllied`。標準30ターンは`Base/Assets/Gameplay/Data/GlobalParameters.xml:232`。戦争状態も毎ターン確認する。`SetHasAllied(false)`が解除にならない点は依頼者の実機検証済み事実 |
| 必須の公式英雄モードとDLC | `DLC/Babylon/Babylon.modinfo:2,38-44`のDLC IDと`GAMEMODE_HEROES=1`条件。作成時の設定は`DLC/Babylon/Data/Babylon_ConfigData.xml:21-23`。独立MODの依存関係にもBabylonを追加 |
| 公式の英雄と加入Modifier | `DLC/Babylon/Data/Babylon_Heroes_MODE.xml:116-127`の`HeroClasses`、`620-631`の`ProjectCompletionModifiers`、`895`の`MODIFIER_CITY_GRANT_HERO`、`903-914`の一度だけ適用するModifier、`985-1008`の英雄クラスと拠点建物。`City:AttachModifierByID`のGameplay例は`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:296-297` |
| 英雄の加入済み所有者と生存 | `DLC/Babylon/UI/Additions/GreatPeopleHeroPanel.lua:124-159`の`GetHeroClaimPlayer`とUnit一覧、`HeroesPopup.lua:211,265`の拠点建物と`GetHeroClassType`、`Base/Assets/UI/Panels/UnitPanel.lua:2269`の`Unit:GetProperty(UnitPropertyKeys.LifespanRemaining)`。英雄判定は`HeroesPopup.lua:265-268`と同じく英雄クラスの負の値で判定し、今回のUIで存在しなかった直接の寿命 getter は使わない。公式の加入処理を都市に直接付けて所有者と寿命まで設定されることは要実機確認 |
| 英雄加入の信仰報酬の基準 | `DLC/Indonesia_KhmerScenario/Scripts/IndonesiaKhmerScenario.lua:245-246`のGameplayの`GetFaithYield`。実行直前の値を保存して10倍を付与する。残高を10倍にする処理ではない |
| 実機で英雄の拠点建物を準備する | `Debug/City.ltp:940-942`の`CreateIncompleteBuilding`、100%完成の指定は`Base/Assets/UI/Tuner/TunerCityPanel.lua:10`。準備後の`HasBuilding`も検査する |
| 同盟終了を記録するターンイベント | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:64`の`GameEvents.OnGameTurnStarted.Add`。ロード時・ターン開始時に報酬を再付与しない |

## 検証

```powershell
python -m unittest discover -s tests -t .
python -m unittest discover -s tools/civ6-autotest/tests
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/religion-persistence.json --check
```

宗教のテストはlupaを必須importし、スキップしない。SQLと実際のLuaを使い、9件の効果、材料消費・再利用、Eurekaの選択、市民数の切り上げ、資金の移転、宗教同盟の双方向の制限と終了、公式英雄の型、予約中の失敗、AIと旧保存を検査する。市民の圧力から人数を算出するスタブは動作条件を模したものにすぎず、エンジンの丸めの証明ではない。JSONと新しいLua VMによる保存の模擬も、Civ6の保存の代用ではない。

実機はメインセッションでMODを配置して、次の1シナリオを実行する。

```powershell
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/religion-persistence.json --test-mode
```

万世紀・織田／武田・小地図・標準速度で開始し、公式Heroes & Legendsを有効化する。100ステップ、10回のsave→メニュー→load。日・英・露の表示も確認する。創始と市民変換の準備には標準の番号付きカスタム宗教を2件使い、D8の宗教・教義の内容にはしない。

| 段階 | 自己検査 |
|---|---|
| 公開・地下・信徒0・発見 | 発祥者の滅亡で消えないこと、信徒なし・資料あり・地下持続の記録、未解決の発見、売却500、別宗教との分離 |
| 再興と公開への復帰 | 信仰20、信徒1・施設0、資料を保有し再利用可能。ロードで完全一致 |
| 研究・異端化・Artifact | 研究直後の科学10・文化10とEureka2件、異端化後の追加文化20と信仰400減・資料消費、Artifact後の追加文化30とVault1件をそれぞれ検査。研究中の技術をEureka対象から外し、一括科学だけを比較 |
| 吸収 | 標準宗教の市民を2人＋5人で準備し、最初の都市に自宗教1人・無宗教2人も残す。原宗教の合計7から3人を2都市にまたがって変換。信仰500減、原宗教4人・自宗教4人・無宗教2人を検査。都市ごとの切り上げによる4人への変換を検出 |
| 返還と宗教同盟 | 発祥国の金501・支持101から、金250.5・支持50を移転して同国に金250.5・支持51が残ること。外交の同盟と弾圧を拒む両方向の接続口、ロード後の存続 |
| 英雄 | 公式HeroのUnit・クラス・所有者・寿命を検査。実行直前の信仰創出量の10倍だけを付与し、加入費用なし。ロード後の英雄と残高を外部比較 |
| 8分岐とAI | すべての効果と資料の扱い、AIの売却500。台帳・双方の残高・研究進捗・英雄・Eureka・市民をロード前後で比較し、各分岐の二重実行を拒む |
| 再興先のID衝突 | 直接の再興と発見からの再興で既存IDを上書きせず、施設と保有者と別宗教を保持。2回の信仰支払い計40と保存後の完全一致 |
| 同じ資料での再興 | 10ターン進めて再び信徒0にし、残した同じ資料で信仰20・信徒1の再興ができること |
| 実際の市民による再興 | 明示した対応付けで実際の信徒数を状態に使い、台帳を0人にしても存続中の宗教を失われた扱いにしない。資料発見後に他の集団が再興した宗教を吸収する。再び実際の信徒0にした後、保有資料で信徒1人・信仰20の再興を行い、10回目の保存・読み込みで比較 |
| 宗教同盟の終了 | さらに31ターン進め、外交の同盟が自然に切れたこと、両方向の制限が解除され終了記録が残ること。強制講和と同ターンの強制宣戦は使わない |

比較にはドライバー側の`capture_as/equals_capture`を使う。台帳と期待値が一緒に消えても成功にならず、不一致はLuaの`error()`か外部比較で失敗する。`--test-mode`を明示した自動検証では、保存される設定値で検証用SQLだけを読み込み、再興の待機期間とエンジンの同盟期限を1ターンにする。期限の検査は実際のターン経過で行い、計3ターン待つ。通常実行の再興10ターン・同盟30ターンは変えない。

既存のレビュー修正は保持した。MODは万世紀専用、再興で既存コミュニティを上書きしない。MOD切り替えのドライバーは、有効化指定と矛盾しない範囲で間接依存を先に無効化し、その結果を記録する。依存元を無効化する前に依存するMODを無効化する修正と、その検査もこのPRに含む。

## 現在の検証結果

最新の通常テスト456件（宗教のLua70件）と自動検証ツール212件が成功し、Luaのスキップはない。100手順・Lua63片・10回の保存読込と、35手順の圧力診断は形式と構文の検査に成功した。

実機religion-10・12は圧力の診断に成功。religion-11は6段階の保存読込比較、吸収の世界7人から3人だけの変換と無宗教2人の保持、実際の科学・文化、Artifact、返還と宗教同盟まで成功した。7回目の未完了英雄予約の保存読込では、確認待ちで停止した。religion-13では非表示UIを動かす修正の実動作を確認したが、UIに存在しないUnitの直接の寿命 getter などを呼び、73/101で停止した。公式の英雄クラスと寿命プロパティの読取へ修正し、単体テストを通した。実機4回の上限のため、修正後の通し実行は次の担当が行う。

第三の宗教の市民を保持する追加の実機確認は共有ToDo190に登録した。Luaでその人数を保持する検査は成功しているが、現在の実機準備には第三の宗教の市民を含めていない。マルチプレイは共有ToDo9で別途確認する。

## これまでの結果と修正

前の担当が2026-10-01に依頼されたコマンドをそのまま実行し、当時のmain取り込み後の410件と自動検証ツール204件が成功。Luaのスキップなし。前の作業で必要だった権限の実行補助は使用していない。残っていた空の一時フォルダー3件は確認して削除した。

確定回答を反映した実機結果と現在の変更の別モデルレビューは実施中。吸収の圧力減算・人数の丸め、GameplayのEureka・英雄API、公式Modifierによる加入、金の端数、同盟の期限切れは実機結果で確認する。弾圧の接続口、自然な探索・布教の入力連携と未対応宗教の自動の対応付け、マルチプレイはPoCの範囲外として残る。

別モデルの指摘に従い、無宗教の市民は人口から不足分も含めて数え、圧力APIには負の宗教IDを渡さない。実在する宗教だけの圧力を整数の比率へ調整し、エンジンの市民数を再読取しながら有限回で検査する。すべての宗教と無宗教の人数が期待値に一致してから吸収の信仰500を払う。不一致は処理中として残し、再実行を拒む。無宗教の行が省略される場合もテストした。

実機のreligion-02はMOD有効化時の依存IDが登録値と違うため停止。公式BabylonのID表記へ直し、religion-03では有効化・開始・6段階の保存と読み込みが成功した。8分岐の準備では信徒一覧の列挙で停止したため、公式例の`pairs`へそろえ、取得と列挙のどちらが原因か実機で確認中。

二度目のレビューで、台帳0人と対応付けた市民7人の矛盾、0人変換でも信仰500を払う問題が判明した。明示した対応付けがある宗教の信徒数は、市民数を読み取って判定する。コミュニティの入力は施設と来歴を保存する。資料は信徒0の間に発見し、その後ほかの集団が再興してから、保有資料を吸収に使うシナリオへ変更した。対応付けた宗教の再興では、自都市の無宗教の市民を優先し、いなければ宗教ID順に1人を変換する。費用は実人数の一致後に支払い、保存後も実人数と台帳が一致することを検査する。

実機のreligion-04でも一覧取得が停止した。実行中のGameplayオブジェクトを調べ、`GetNumFollowers`・`RemovePressureOneReligion`・`AddReligiousPressure`があり、`GetReligionsInCity`がないことを確認した。信徒数を各実宗教IDについて直接読み、無宗教の人数は人口との差から求める実装へ変更。圧力の調整は正の宗教IDだけを消去し、正の圧力を再配分する。市民数が完全一致してから費用を払う。新しいAPIの引数と変換は追加の実機で確認する。

英雄の管理オブジェクトにもGameplayには所有者と拠点建物の取得APIがないため、表示しないUI用の読取橋を追加した。公式の都市用加入ModifierはGameplayで適用し、UIの次のフレームで英雄・クラス・寿命・加入先を確認してから信仰報酬を払う。予約中の保存を再開でき、報酬の途中で停止した場合は再実行しない。検証は30秒で止め、確認できない場合は処理中の記録を残す。

3回目のレビューで、吸収先の自国信徒の検査が手入力を使う点と、再興の宗教IDが文字列順になる点を指摘された。吸収先の実宗教について自分の各都市の実信徒数を読み、手入力値を検査の根拠から取り除いた。再興の選択だけは数値順でIDを並べる。手入力と実人数が逆転した2ケースとID2・10の比較を追加した。英雄の報酬も受取先の手番まで待ち、他人の手番の待機時間は検証期限に数えない。

再レビューではMustなし。複数人の英雄予約で確認期限を共有する指摘も直し、受取人ごとの手番内の経過時間を独立して数える。別の人の予約が期限切れになっても、手番外で待つ人の予約を止めないテストを追加した。英雄予約中の保存の実機ケースと、最新APIによる市民変換の実機成功は未確認。前のジョブでは実機上限4回で停止した。今回の引き継ぎ依頼で検証を再開し、テスト・レビュー・実機が通ったらマージする。

別モデルレビュー（20261001-063756-rev-religion）はMustなし。発見済み資料を直接再興すると発見記録が未解決になる指摘を受け、未解決の発見はその再興選択からだけ解決するようにした。教典と遺物の両方で拒否時に台帳と信仰が変わらず、通常の発見選択は完了することを検査する。実機シナリオにも拒否と不変の検査を追加した。

検証用の短縮期限が手動の新規ゲームへ残る懸念は、公式MainMenu.luaのOnAdvancedSetup（183行）などが設定を初期化することを確認した。シナリオ末尾でメニューへ戻り、同じ設定初期化で検証フラグが消えることを実機で検査する。英雄予約が未完了の状態での実機保存は確認項目として残る。単体テストでは再開と二重支払いの拒否を検査している。

実機religion-06はCiv6の起動前に停止し、共通ツールの再起動再試行でも成功しなかった。Lua.logとModding.logの差分は空。Steamの画面はゲーム実行中を示す一方、Civ6プロセスはなく、前のCiv6の子として起動したFiraxisBugReporter.exeだけが残っていた。その終了後にCiv6の起動を確認した。ドライバーは起動前に、Civ6が動いていないことと報告用プロセスの実行ファイルがCiv6の配置先であることを同じプロセスハンドルで確認して終了する。他のゲームの報告用プロセスと実行中のCiv6は対象外。起動中への変化、別パス、読取失敗、終了失敗、終了済みの場合も単体テストで確認する。

実機religion-07は6段階の保存・読み込み、再興の拒否、売却・秘匿・研究・異端化を通過。吸収は実人数が計画と一致せず、信仰500を支払う前に停止した。変換の試行ごとに検証用ログへ目標人数と実人数を記録し、次の実機で圧力の再配分を調べる。障害報告プロセスの終了完了を最大10秒で確認してからSteamを起動するようにした。開けないプロセスが残る場合は理由を明示し、終了済みの場合だけ続行する。

実機religion-08は6段階の保存・読み込みと売却・秘匿・研究・異端化を通過し、吸収で停止した。圧力を消す呼び出しの直後も元の信徒2人・自宗教1人・無宗教2人が残り、最初の追加で自宗教5人になった。以後、倍率を1024から1まで下げても5人のままだった。検証都市を作り直し、RemovePressureOneReligion(宗教ID,100)を実行しても信徒5人のまま。引数の不足または並びの違いを疑い、4回目で有限の候補を検証する。現時点で吸収はMustとして未解決であり、マージしない。

実機religion-09でも、RemovePressureOneReligionの引数を宗教IDと100、100と宗教ID、0・宗教ID・100・-1、0・宗教ID・10000・-1に変えた候補は、いずれも検証都市の信徒5人を変えなかった。これらの結果だけでは正しい消去方法は確定できない。吸収の人数不一致は未解決で、実機4回の上限のため追加の実行はこのジョブでは行わない。

religion-09のModding.logには「UserInterface components require only the .xml file to be specified.」があり、英雄確認のUI橋が読み込まれていなかった。AddUserInterfacesを同名Luaを読み込む空のXMLコンテキストへ変更した。既存のJAC_CustomCivs_Observe.xmlと同じ登録方式で、表示用の部品は置かない。他のPoCのUI登録も調べ、同じ誤りはなかった。Luaの単体テストに加えて、XMLの登録・コンテキスト・同名Luaの対応を検査する。修正後のエンジンでの読み込みと英雄加入は次の実機で確認する。

残る実機はreligion-persistence.jsonの通し成功。吸収の消去・人数配分、科学と文化の実産出、Artifactと返還の効果、公式英雄の加入・信仰報酬、残る3段階の保存と読み込み、保有資料の再利用、対応付けた実宗教の再興、宗教同盟の自然終了、手動の新規ゲームでの検証フラグ解除を確認する。英雄が未完了の状態での保存・再開は追加の確認項目。無宗教・他宗教の市民数を変えて通すことや、検査を外して通すことはしない。マージは実機成功とMust解消の後とする。

最終レビューの提案に従い、動作済みのJAC_CustomCivs_UIFilesと同じく、XMLと同名LuaをImportFilesにも登録した。ゲーム開始直後にUI橋の存在を待つ検査を追加し、吸収の不一致で止まる前にも読込を判定する。現行のシナリオは100ステップ・Lua63片。追加後の実機は上限のため次のShepherdで実行する。


## 圧力消去の引数を確認する診断

2026-10-01、次の担当がゲーム本体の関数登録と処理を読むことで、1引数の消去呼び出しは0を減算していたと確認した。GSの `DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll` では、登録箇所0x1806cd2e5が `RemovePressureOneReligion` を0x1806cd680のLua用関数へ結び付ける。この関数は自分自身の後ろの3引数を読み、第1引数を宗教ID、第2引数を圧力量、第3引数を責任プレイヤーとして0x18014b360へ渡す。第2引数は8ビット左シフトされる。減算処理0x18014b486〜0x18014b490は、該当宗教の圧力からその量を引き、0以下の行を残さない。割合指定ではない。追加処理も0x1806cd44fで圧力量を8ビット左シフトする。

消去は `RemovePressureOneReligion(religion,8388607,-1)` と、最大値に残り得る小数部分を消す `RemovePressureOneReligion(religion,1,-1)` の2回へ変更した。追加の圧力量も符号付き24.8形式の上限を超えない倍率へ制限する。同じ1引数呼び出しを使っていた実機準備と再興前の喪失準備も修正した。Luaの模擬は単なる全消去から圧力量の減算へ変え、最大圧力と小数部分がある状態での変換も検査する。

`religion-pressure-probe.json` は35手順の診断である。表示用Luaから圧力の一覧を読み、各候補について消去前・直後・追加による再計算後を記録する。この診断はreligion-12で35手順を完了した。通し実行の全効果と全保存はまだ完了していない。


英雄の未完了予約も通しシナリオで保存する。加入前に検証用コードだけでUIのUnit情報を一時的に取得不能にし、報酬が未払いの予約を確認して保存する。メニューへ戻り読み込むと表示用Luaが新しく読み込まれ、公式英雄の照合と信仰報酬が再開する。報酬を加える前の残高と予定の報酬・Unit・クラスをドライバー側に保存し、読込後の実残高とUnit・クラスを比較する。完了の再実行は拒否される。100手順・Lua63片・10回の保存読込を検査し、実機成功は結果が出てから記録する。

圧力の診断は表示用データの反映前に読み取らないよう、消去と圧力読取を別の手順へ分けた。1引数、100、10000、最大値の4ケースを比較し、最大値で消した直後に実宗教の信徒が0になることを検査する。診断は35手順で、ターン経過は不要である。


実機religion-10は圧力の診断を完了した。1引数と量100では信徒5人のまま、量10000と最大量では0人になった。1手順内で表示用データを読むと古い圧力が返るため、35手順の診断で操作と表示用の読取を分けている。

religion-11は吸収に成功した。世界の信徒7人から3人だけを2都市で変換し、無宗教2人を保持して信仰500を支払った。研究・異端化・Artifactの実際の科学と文化、返還の金250.5・支持50と宗教同盟も確認した。未完了の英雄予約を7回目に保存し読み込んだが、確認完了を待つ73/101で停止した。

公式 `Base/Assets/UI/InGame.lua:347-348` は追加UIを非表示の状態で読み込む。英雄の橋にはフレームごとの確認処理を登録していたが、非表示のコンテキストではその処理が動かない。読み込み画面が閉じる公式イベントで、部品も入力処理も持たない空のコンテキストを動かすようにした。画面は追加しない。非表示の間は報酬が未払いで、保存読込後のイベントから一度だけ完了することを模擬テストでも検査する。修正後の実機成功は次の実行で確認する。


## 次の担当が再開するところ

この担当の実機4回はreligion-10〜13である。10は圧力診断、11は通し実行の英雄予約読込後で停止、12は35手順の圧力診断成功、13はUIの更新再開を確認したが同じ予約の完了待ちで停止した。13のLua.logには各読込後の確認開始と、最後にHeroUI.lua:15の「function expected instead of nil」がある。公式の表示用コードと本体のAPI名を再確認し、英雄判定をクラスから、寿命を`UnitPropertyKeys.LifespanRemaining`から読むようにした。模擬から直接の寿命 getter と英雄判定 getter を取り除いた。寿命のプロパティがまだ届かない場合は報酬を払わず、値が届いた後に一度だけ完了することも検査した。

残る必須シナリオは **religion-persistence.jsonの通し実行**。開始と6段階の保存比較、吸収・無宗教の保持、実産出・Artifact・返還は13でも成功したが、英雄の寿命プロパティ読取、予約の保存再開後の報酬、残る3段階の保存比較、資料の再利用、実宗教の再興、同盟の自然終了、設定初期化は未確認である。圧力診断religion-pressure-probe.jsonは12で完了しており、この担当の最後の修正では変更していない。

PRはMustの実機確認が残るため未マージで保持する。実機は共有のlive作業フォルダーの`tools/civ6-autotest/artifacts/religion-10`〜`religion-13`に残した。最新の456件・自動操作212件が成功し、Luaのスキップはない。100手順・Lua63片と35手順の形式・構文を検査した。日本の史実配置との連携・自然な探索や布教は今後の連携対象で、今回の小地図の試験を日本マップの適合確認としては扱わない。終了済みCiv6の孤立した障害報告プロセスを閉じる処理は自動試験用の起動復旧で、クラッシュ報告ファイルの削除は行わない。手動の未送信報告を保持する運用は追加の確認項目として記録する。

最終再レビューの指摘に従い、30秒を超えてフレームごとの確認を止めた予約も、公式のゲーム更新通知を受けるたびに一度だけ再確認する。寿命が遅れて反映された場合、保存の再読込をしなくても確認を完了できる。受取人の手番だけで検査し、未反映と手番外では支払わず、有効な値の公開後に一度だけ払うことを模擬で検査した。反映待ちの無制限なフレーム処理は再開しない。実機確認は次の担当が行う。
