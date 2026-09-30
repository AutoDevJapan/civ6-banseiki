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

合計を3で割って1回だけ切り上げ、プレイヤーID・都市ID順に市民を変換する。圧力は無宗教を含む各宗教の目標人数に比例させるため、布教圧力の過去の比率は保持しない。人口は移動しない。適用後は原宗教の減少、自宗教の増加、他宗教の人数不変をすべて検査する。負の圧力と丸めは公式例だけでは保証できず、要実機確認。自動の布教・市民変動とPoCのコミュニティ台帳の同期は後続のアダプターに残す。発見後に標準宗教の信徒が戻っていても、未解決の吸収などは解決できる。

返還では発祥国の残高を減らし、同額を受取人に加える。金は半額、外交的支持は整数のため半分を切り捨てて残りを発祥国に残す。すでに同盟中なら結び直して期限を延長しない。同盟の期限切れ、戦争、どちらかの滅亡で宗教同盟を終了し、通常の同盟を結び直しただけでは宗教同盟を再開しない。

SSOTの全分割ファイルを`hero/Hero/英雄/弾圧`で確認した。Hero型は8.4にあるが、弾圧の効果量と実行コマンドはまだ定義されていない。今回強制するのは、宗教同盟の保存状態と、`CanPersecute(actorPlayer, actingReligion, citizenPlayer, targetReligion)`および`AssertCanPersecute`の拒否である。相手の宗教が自国民を対象にする場合と、自国が相手の宗教を対象にする場合を拒む。将来の弾圧処理は効果の前にこの接続口を通す必要がある。標準の異端審問、宗教戦闘、布教を自動で遮断する処理には未接続。

英雄は未加入の公式英雄を`HeroClassType`順に選ぶ。公式の`HeroClasses.CreationProjectType`から加入Modifierを取得して、公式の拠点建物を持つ都市へ付ける。新しくできたUnitについて英雄クラス・寿命・加入済み所有者を確認してから、実行前の信仰創出量の10倍を付与する。PoCの生涯1人の制限は保持する。公式モードの寿命・再召喚・能力を使い、独自の人物案は作らない。

AIは人間と同じ前提・費用・上限を検査する。職業の第一候補と、吸収→売却→研究→異端化→Artifact→外交事件→英雄→再興→秘匿の候補順を保持する。自然な探索トリガーは後続のアダプターから接続する。

## 保存と呼び出し

プロパティは`JAC_RP_STATE_V1`、`schema=1`。`PUBLIC/UNDERGROUND/LOST`と、信徒なし・資料あり・地下持続の事実を保持する。資料の履歴、再興、標準宗教への対応、選択の効果、英雄Unit、Vault、宗教同盟と終了時点を保存する。

旧保存は読取用のコピーへ欄を追加するだけで、ロードだけでは書き込まない。旧`DEFERRED`や旧条件で使用済みだった資料は自動で新報酬へ変更しない。今回の条件での再興には`reusable=true`を保存する。未知スキーマを上書きせず、未設定のプロパティには`nil`を使う。

通常の入力は検査後に1回保存する。費用・効果は予約を`PENDING`で保存し、エンジンに適用してから`APPLIED`を保存する。最初の保存失敗なら作用しない。効果・最後の保存が失敗すれば`PENDING`のまま再実行を拒む。残高・Eureka・市民・英雄とGameプロパティは一括で戻せないため、実際の状態を照合して復旧する。ロードやターン開始時に効果を再適用しない。ターン開始処理は宗教同盟の終了だけを更新する。

Gameplay APIは`ExposedMembers.JAC_ReligionPersistence`。`ResolveDiscovery(id, discovery, outcome, options)`と`Revive(id, revival, source, community, player, city)`を使う。`Rules/Inspect/Export`はコピー、`Snapshot`はキー順を固定した台帳文字列。UIからは同期されたGameplay要求を使い、手番開始前に送らない。

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
| 市民の宗教を変えるAPI | `DLC/VikingsScenario/Scripts/VikingScenario.lua:103`の`City:GetReligion():AddReligiousPressure(0, religionIndex, pressure, -1)`。Civ5の`ConvertPercentFollowers`は使わない。元宗教の人数を減らし自宗教を増やす人数配分に圧力を調整する。負の圧力による減算と整数人数への丸めは、この公式例だけでは証明できないため要実機確認。すべての宗教の人数を再読取して検査し、不一致を成功にしない |
| 実機で市民を準備する | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario_Support.lua:266`の`ChangePopulation`、`DLC/NubiaScenario/Scripts/NubiaScenario.lua:820-827`の聖都・創始・`SetAllCityToReligion`。都市全体の変換は準備に限り、吸収そのものには使わない |
| 外交の同盟を作る・存続を読む | `DLC/VikingsScenario/Scripts/VikingScenario.lua:18-20,77`の`SetHasAllied(other, true)`と`HasAllied`。標準30ターンは`Base/Assets/Gameplay/Data/GlobalParameters.xml:232`。戦争状態も毎ターン確認する。`SetHasAllied(false)`が解除にならない点は依頼者の実機検証済み事実 |
| 必須の公式英雄モードとDLC | `DLC/Babylon/Babylon.modinfo:2,38-44`のDLC IDと`GAMEMODE_HEROES=1`条件。作成時の設定は`DLC/Babylon/Data/Babylon_ConfigData.xml:21-23`。独立MODの依存関係にもBabylonを追加 |
| 公式の英雄と加入Modifier | `DLC/Babylon/Data/Babylon_Heroes_MODE.xml:116-127`の`HeroClasses`、`620-631`の`ProjectCompletionModifiers`、`895`の`MODIFIER_CITY_GRANT_HERO`、`903-914`の一度だけ適用するModifier、`985-1008`の英雄クラスと拠点建物。`City:AttachModifierByID`のGameplay例は`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:296-297` |
| 英雄の加入済み所有者と生存 | `DLC/Babylon/UI/Additions/GreatPeopleHeroPanel.lua:124-159`の`GetHeroClaimPlayer`とUnit一覧、`HeroesPopup.lua:211,265`の拠点建物と`GetHeroClassType`、`Debug/Heroes.ltp:129-141`の`IsHero/GetLifespan`。公式の加入処理を都市に直接付けて所有者と寿命まで設定されることは要実機確認 |
| 英雄加入の信仰報酬の基準 | `DLC/Indonesia_KhmerScenario/Scripts/IndonesiaKhmerScenario.lua:245-246`のGameplayの`GetFaithYield`。実行直前の値を保存して10倍を付与する。残高を10倍にする処理ではない |
| 実機で英雄の拠点建物を準備する | `Debug/City.ltp:940-942`の`CreateIncompleteBuilding`、100%完成の指定は`Base/Assets/UI/Tuner/TunerCityPanel.lua:10`。準備後の`HasBuilding`も検査する |
| 同盟終了を記録するターンイベント | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:64`の`GameEvents.OnGameTurnStarted.Add`。ロード時・ターン開始時に報酬を再付与しない |

## 検証

```powershell
python -m unittest discover -s tests -t .
python -m unittest discover -s tools/civ6-autotest/tests -t .
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/religion-persistence.json --check
```

宗教のテストはlupaを必須importし、スキップしない。SQLと実際のLuaを使い、9件の効果、材料消費・再利用、Eurekaの選択、市民数の切り上げ、資金の移転、宗教同盟の双方向の制限と終了、公式英雄の型、予約中の失敗、AIと旧保存を検査する。市民の圧力から人数を算出するスタブは動作条件を模したものにすぎず、エンジンの丸めの証明ではない。JSONと新しいLua VMによる保存の模擬も、Civ6の保存の代用ではない。

実機はメインセッションでMODを配置して、次の1シナリオを実行する。

```powershell
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/religion-persistence.json
```

万世紀・織田／武田・小地図・標準速度で開始し、公式Heroes & Legendsを有効化する。85ステップ、8回のsave→メニュー→load。日・英・露の表示も確認する。創始と市民変換の準備には標準の番号付きカスタム宗教を2件使い、D8の宗教・教義の内容にはしない。

| 段階 | 自己検査 |
|---|---|
| 公開・地下・信徒0・発見 | 発祥者の滅亡で消えないこと、信徒なし・資料あり・地下持続の記録、未解決の発見、売却500、別宗教との分離 |
| 再興と公開への復帰 | 信仰20、信徒1・施設0、資料を保有し再利用可能。ロードで完全一致 |
| 研究・異端化・Artifact | 研究直後の科学10・文化10とEureka2件、異端化後の追加文化20と信仰400減・資料消費、Artifact後の追加文化30とVault1件をそれぞれ検査。研究中の技術をEureka対象から外し、一括科学だけを比較 |
| 吸収 | 標準宗教の市民を2人＋5人で準備し、合計7から3人を2都市にまたがって変換。信仰500減、原宗教4人・自宗教3人を検査。都市ごとの切り上げによる4人への変換を検出 |
| 返還と宗教同盟 | 発祥国の金501・支持101から、金250.5・支持50を移転して同国に金250.5・支持51が残ること。外交の同盟と弾圧を拒む両方向の接続口、ロード後の存続 |
| 英雄 | 公式HeroのUnit・クラス・所有者・寿命を検査。実行直前の信仰創出量の10倍だけを付与し、加入費用なし。ロード後の英雄と残高を外部比較 |
| 8分岐とAI | すべての効果と資料の扱い、AIの売却500。台帳・双方の残高・研究進捗・英雄・Eureka・市民をロード前後で比較し、各分岐の二重実行を拒む |
| 再興先のID衝突 | 直接の再興と発見からの再興で既存IDを上書きせず、施設と保有者と別宗教を保持。2回の信仰支払い計40と保存後の完全一致 |
| 同じ資料での再興 | 10ターン進めて再び信徒0にし、残した同じ資料で信仰20・信徒1の再興ができること |
| 宗教同盟の終了 | さらに31ターン進め、外交の同盟が自然に切れたこと、両方向の制限が解除され終了記録が残ること。強制講和と同ターンの強制宣戦は使わない |

比較にはドライバー側の`capture_as/equals_capture`を使う。台帳と期待値が一緒に消えても成功にならず、不一致はLuaの`error()`か外部比較で失敗する。実機には最後の期限切れの検査まで計41ターンを追加で進める時間が必要。

既存のレビュー修正は保持した。MODは万世紀専用、再興で既存コミュニティを上書きしない。MOD切り替えのドライバーは、有効化指定と矛盾しない範囲で間接依存を先に無効化し、その結果を記録する。今回そのドライバーは変更していない。

## 結果と未検証

宗教のlupaテスト50件、85ステップの形式検査とLua55片の構文検査が成功。実機はこの作業では実行していない。圧力の減算・市民人数の丸め、GameplayでのEurekaフラグと英雄の管理API、公式Modifierによる直接加入、外交残高の端数と同盟の期限切れ、台帳の保存、日英露表示は要実機確認。標準ゲームの弾圧への接続、自然な探索・布教・台帳の自動同期、マルチプレイは未接続・未検証。

指定の全件テストでは、Windowsの制限付きトークンによる0700一時フォルダーの再アクセス不可と、unittestが自動検証ツールの名前空間パッケージを検出できない問題を確認した。新しく作る一時フォルダーだけに親の権限を継承させ、既存の名前空間パッケージの全テストを列挙する実行補助を`C:/Users/makhmeto/AppData/Local/Temp/religion-run-unittest.py`に置いた。既存フォルダーのACLは変更せず、リポジトリへ実行補助や`__init__.py`は追加していない。補助を使い、全376件（宗教50件を含む）と自動検証ツール159件が成功。スキップなし。

別モデルのレビューはCodexのアプリサーバー起動時に`Access is denied`で停止し、未実施。共有ToDoの登録も`D:/CC/Tools/todo/todo.lock`への書き込み権限で拒否された。この作業ではコミット・pushを行わず、変更は作業ツリーに残す。

補助を使う前の失敗で作られた`tmpixfeu7wh/`は、読み取り・削除とも権限で拒否されたため残っている。大きさは読み取れない。指定のDownloads内の削除希望フォルダーも、このセッションの書き込み許可範囲に含まれないため移動できない。

後片付けの記録のDiscord投稿も`D:/CC/Tools/discord-ask/state.lock`への書き込み権限で拒否された。記録本文は`C:/Users/makhmeto/AppData/Local/Temp/religion-cleanup-record.txt`に残した。
