# PoC 束2：宗教持続

信徒・資料の持続に加え、有料の再興と発見後の8分岐を実装した。仕様の正本は[SSOT 9.2・9.3](../spec/ssot/09-assimilation-religion.md)。この文書は回答、提案の比較、実装・検証の根拠を記録する。

## 回答の反映

2026-09-30のDiscord回答は依頼者から受領した。

| 回答 | 反映 |
|---|---|
| Q79 → a | 信徒0と資料の残存を両方記録。SSOT 9.2へ明記し、質問から削除 |
| Q80 → c | 人数・費用・資料の扱い・前提・制限をまとめて提案し、推奨案を実装 |
| Q81 → c | 再興以外の8分岐もすべて提案し、推奨案の効果・制限・AIを実装 |

数値は**PoCで試す推奨案**としてSSOT 9.3に記載した。本番の数値へのオーナー承認とは区別する。比較案は未実装。D8は議論待ちのまま、宗教・教義・人物の内容は定義しない。

独立MODは `poc/JAC_PoC_ReligionPersistence/`、IDは `9b0b9a39-bd72-4ff1-9062-05eeac09b693`。Gathering Stormと万世紀で読み込み、番号付き仮宗教20枠を使う。判定はカタログIDを使い、具体的な宗教名や20という個数に依存しない。

## 再興の提案（Q80）

| 選択 | 人数・費用・資料の使用後 | 理由・違い |
|---|---|---|
| a（推奨・実装） | 信徒2人、信仰60。利用権を1回消費し、現物は記念資料として残す | 来歴を守り、同じ資料からの無限再興を防ぐ |
| b | 信徒1人、信仰20。資料は消費して来歴だけ残す | 安価だが現物を失う |
| c | 信徒3人、信仰100。資料は残し、再興に再利用できない | 高価だが再興時の集団が大きい |
| d | 信徒2人、ゴールド120。利用権を1回消費し、現物は残す | 信仰のない勢力も資金で再興できる |

aの前提は、世界の信徒0、未使用の再興資料1件、その所有者が生存する大勢力、自勢力の都市、信仰60。教典・遺物・記録・人物が対象で、聖地だけでは再興しない。都市に結び付けた新しいコミュニティIDで信徒2人・公的施設0から開始する。人数・施設数の自由指定は廃止した。

信仰60を実際の残高から引く。現物・資料ID・来歴は記念資料として残し、人物資料も死亡させない。同じ資料は再興にも他の報酬にも再利用できない。宗教ごとに10ターン以上空け、再び信徒0になった場合は**別の未使用資料**と費用で再興できる。秘匿資料は後日の有料再興1回だけに使える。`canRevive`は未使用資料の存在を示し、費用・都市・間隔は実行時に検査する。

信仰60は公式の集落の中信仰報酬60、小信仰報酬20の3倍。社の信仰2/ターンでは30ターン分（F1・F2）。信徒2人はMODの台帳上の集団で、標準宗教の市民・圧力を変換した人数ではない。

## 再興以外の8分岐（Q81）

各行の先頭aを推奨・実装した。秘匿以外は資料の利用権を消費するが、現物や移管の来歴は保存する。

| 選択 | a：推奨・実装（理由） | b | c | d |
|---|---|---|---|---|
| 売却 | ゴールド40、市場へ移管（公式の小報酬相当で価値を抑える） | ゴールド20 | ゴールド60 | 信仰20と交換 |
| 秘匿 | 費用・産出0、後日の有料再興用に残す（再興の選択肢を守る） | 保管費ゴールド20 | 秘匿時に信仰10 | 資料を破棄し信仰20 |
| 研究 | 科学20、記録保管（技術1件の即時解放を避けて研究に寄与する） | 科学10・文化10 | 技術Boost1件 | 科学40、資料を破棄 |
| 異端化 | 文化20、異端解釈の注釈（D8を決めずに解釈の違いを残せる） | 信仰20 | 信仰20を払い文化40 | 科学10・文化10 |
| 他宗教への吸収 | 信仰20、存続する自勢力の宗教へ移管（継承と小さな利益を両立する） | 信仰10・文化10 | 信仰40、原資料を消費 | 信仰20を払い吸収先の信徒1人を増やす |
| Artifact獲得 | 文化30、PoCのVaultへ1件収蔵（恒久産出を積み重ねず収蔵に価値を付ける） | 文化10・信仰10 | 文化1/ターンを20ターン | 標準Artifact1件、文化3/ターン・観光3、考古博物館の空き枠が必要 |
| 外交事件 | 発祥者へ返還し外交支持10（平和的解決を試せる） | 返還しゴールド40 | 接収し文化20、相手の不平25 | 返還し文化10 |
| 英雄加入 | 信仰60、仮英雄として斥候1体・勢力に1人（探索人物の加入を小さな戦力で検証する） | 信仰80、戦士1体 | 信仰40、宗教ごとに斥候1体 | 信仰120、公式Heroesモードの英雄1体・同モードが必要 |

| 分岐 | Civ6への効果と公式値との比較 | 前提・制限・資料の使用後 | AI |
|---|---|---|---|
| 売却 | `ChangeGoldBalance(40)`。公式の小ゴールド報酬40と同額（F1） | 市場へ移し所有者を外す | 盗賊の第一候補 |
| 秘匿 | 産出・費用0。公式の小信仰報酬20より当面の利益は小さい（F1） | 隠し場所へ。後日の有料再興以外の再発見・報酬は不可 | 他の候補が使えない場合に温存 |
| 研究 | `MODIFIER_PLAYER_GRANT_YIELD`で科学20を1回。図書館2/ターンの10ターン分（F2・F5） | 都市あり、記録保管へ | 学者の第一候補 |
| 異端化 | 同Modifierで文化20を1回。モニュメント2/ターンの10ターン分（F2・F5） | 都市あり、注釈資料へ。新宗教・信徒・教義は増やさない | 芸人の第一候補 |
| 他宗教への吸収 | `ChangeFaithBalance(20)`。公式の小信仰報酬と同額（F1） | 別宗教が存続し、自勢力に信徒あり。資料IDが移管先で空いていること。原宗教の来歴と利用済み記録も移管 | 第一候補が使えず吸収先がある場合 |
| Artifact獲得 | 同Modifierで文化30を1回。標準Artifact文化3/ターンの10ターン分（F3・F5） | 都市あり、勢力全体で生涯1枠。収蔵欄へ。標準Great Work・観光・同調効果は付けない | 探検家の第一候補、空き枠が必要 |
| 外交事件 | Gameplayの`GetDiplomacy():ChangeFavor(10)`。緊急事態の支持50の1/5、解放の支持100の1/10（F4） | 生存する他勢力の発祥者、接触済み、平和時のみ。発祥者へ返還。選択効果による自動宣戦・講和はなし | 旅人の第一候補、返還条件を満たす場合 |
| 英雄加入 | 信仰60を引き`UNIT_SCOUT`1体を生成・台帳に登録。公式斥候の生産30相当・戦闘力10・移動3（F6） | 自勢力の都市・有効な隣接配置場所・信仰60、勢力全体で生涯1人。資料は契約の証として残す | 傾奇者の第一候補、費用と空き枠が必要 |

共通の制限は、発見1件で1選択、非再興分岐は勢力×宗教×分岐で各1回。資料を削除・移管・再発見しても利用済み記録を残す。発見後に宗教が再興されても非再興の未解決選択は解決できる。値は速度によらず標準速度相当で固定し、Modifierの`Scale=false`を明示する。

AIは乱数を使わず、職業の第一候補が使えなければ吸収→売却→研究→異端化→Artifact→外交事件→英雄→再興→秘匿から最初に条件を満たすものを選ぶ。人間と同じ前提・費用・上限を検査する。探索アダプターが`ResolveForAI`を呼ぶ形で、自然な探索トリガーはまだ接続していない。

Artifactの同調・探索効果とNotebooksのVaultはSSOT 8.2・8.3、英雄の固有技・寿命・再登場は8.4に従う後続実装。ここでは収蔵の台帳と一括文化、人物加入と実Unitの生成を検証する。

## 保存と呼び出し

世界の信徒・公的施設は、登録されたコミュニティの合計。発祥者の滅亡入力だけでは消さない。

| 条件 | 保存状態 |
|---|---|
| 信徒1以上・施設1以上 | `PUBLIC` |
| 信徒1以上・施設0 | `UNDERGROUND` |
| 信徒0 | `LOST`、信徒・資料・来歴を保持 |
| 施設0で信徒または資料あり | 表示状態とは独立して`underground=true` |
| `LOST`で未使用の教典・遺物・記録・人物あり | `canRevive=true`、自動再興しない |

プロパティは`JAC_RP_STATE_V1`、`schema=1`。旧保存の欄はコピー上に追加し、読込だけでは書かない。旧`DEFERRED`は旧仕様の未適用の選択として保持し、その資料を新報酬へ再利用しない。旧再興資料も利用済みにする。未知スキーマは上書きせず、未設定欄は`nil`、空文字のGameプロパティは作らない。

宗教・コミュニティ・資産・発見・再興履歴に加え、資料の使用履歴、削除済みID、分岐の回数、仮英雄Unit ID、PoCのVault欄を保存する。通常入力はコピーを検証して1回保存する。費用・効果のある解決は、予約して`PENDING`を保存→エンジンへ費用・効果→`APPLIED`と移管・再興集団を保存する。最初の保存失敗ならエンジンへ作用しない。効果や最後の保存の失敗では`PENDING`を残し、再実行を拒む。残高・Modifier・Unitとプロパティの一括ロールバックはできないため、実状態を照合してから復旧する。ロード時の自動再適用は行わない。

Gameplay APIは`ExposedMembers.JAC_ReligionPersistence`。UI操作を接続する場合はGameplayへの同期された要求を使う。都市と信仰60以上があるプレイヤー0のGameCore_Tunerでの例：

```lua
local R = ExposedMembers.JAC_ReligionPersistence
local id = "JAC_RP_PLACEHOLDER_01"
R.Found(id, 0, "origin", 3, 1, "PLAYER_0")
R.PreserveAsset(id, "book", "SCRIPTURE", "CAVE")
R.SetCommunity(id, "origin", 0, 0, "PLAYER_0")
R.Discover(id, "discovery_1", "book", "SCRIPTURE", "TRAVELER_7", "TRAVELER", "CAVE", 0)
R.ResolveDiscovery(id, "discovery_1", "REVIVE", {
  revival = "revival_1", community = "new", city = Players[0]:GetCities():GetCapitalCity():GetID()
})
return R.Snapshot()
```

`SetCommunity`は差分ではなく現在値。`PreserveAsset`・`MoveAsset`の最後の任意引数は保管先とは別の所有プレイヤーID。`Discover`の最後のプレイヤーIDは必須。`Revive(id, revivalID, sourceID, communityID, playerID, cityID)`は記録・人物・秘匿資料にも使う。`Rules()`はSQLの名前付き定数のコピー。科学・文化の60件のModifierも同じ定数から生成する。`Inspect`・`Export`はコピー、`Snapshot`はキー順を固定した全台帳文字列。`ChooseOutcome`は候補を返すだけ、`ResolveForAI`はAIの発見に限って実際に解決する。

## 公式ファイルの根拠

基準フォルダは `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI`。今回、本体ファイルは変更していない。

| 使用箇所・仮定 | 公式の参照先 |
|---|---|
| Gameplayからプロパティを変更／読取 | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario_StateUtils.lua:60-78,126,137-141`。Gameへの呼出しは `DLC/WarMachineScenario/Scripts/WarMachineScenario.lua:42` |
| カタログを `GameInfo` の行イテレータで読む | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:207,210,1027`（イベント表の `EventPopupData` も走査） |
| Gameplayの現在ターン | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:413,675` |
| コンテキスト間の共有名 | `Base/Assets/UI/MapTacks.lua:5-11` の `ExposedMembers` による共有の説明 |
| 実機の開拓者による都市建設要求 | `Base/Assets/UI/Panels/UnitPanel.lua:2636,2641` の `UnitManager.RequestOperation(..., UnitOperationTypes.FOUND_CITY)` |
| 人間のターンが有効になるまで待つ | `Base/Assets/UI/LocalPlayerActionSupport.lua:6-14` の `IsTurnActive()` 判定 |
| 首都取得と聖都指定・宗教創始・信仰追加 | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:1103,137,143-144` |
| 標準の番号付きカスタム宗教枠 | `Base/Assets/Gameplay/Data/Religions.xml:20,127-131` |
| Gameplayの創始済み宗教一覧 | `DLC/Expansion2/Scripts/WorldCongress.lua:29` |
| UIから創始種別・聖都を読取 | `Base/Assets/UI/ReligionScreen.lua:123,795` |
| 市民単位の標準信徒は別システム | `Base/Assets/UI/ReligionScreen.lua:1074,1091-1092` の `GetReligionsInCity()` と `Followers` |

上記はAPI使用例であり、今回の深い入れ子・空テーブル・false値が保存される証明ではない。GameプロパティのLuaテーブルの保存は束0で確認済み（[PoC結果](README.md)、同文書266行）。今回の台帳については下記の実機シナリオを通す必要がある。空文字Gameプロパティが保存されない件は依頼者の検証済み事実に従った。


| 参照 | 値・仮定 | 公式ファイル:行（上記基準フォルダー配下） |
|---|---|---|
| F1 | 小ゴールド40、小信仰20、中信仰60と公式の速度補正 | `Base/Assets/Gameplay/Data/GoodyHuts.xml:233-245,264-291` |
| F2 | モニュメント文化2、社信仰2、図書館科学2/ターン | `Base/Assets/Gameplay/Data/Buildings.xml:341,345,380` |
| F3 | 標準Artifact文化3/ターン・観光3 | `Base/Assets/Gameplay/Data/GreatWorks.xml:417,616` |
| F4 | 緊急事態の外交支持50、解放時100 | `DLC/Expansion2/Data/Expansion2_GlobalParameters.xml:76,110,112` |
| F5 | 一括産出ModifierとRunOnce・Permanent、Amount・YieldType・Scale | `Base/Assets/Gameplay/Data/GoodyHuts.xml:72-107,233-245`、`Base/Assets/Gameplay/Data/Modifiers.xml:2538-2543` |
| F6 | 斥候の生産30・戦闘力10・移動3、戦士の生産40・戦闘力20 | `Base/Assets/Gameplay/Data/Units.xml:771-772` |
| API | Gameplayのゴールドと信仰残高・費用 | `DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:764,847`、`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:404,563-565` |
| API | GameplayでModifierを付ける | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:1204-1205` |
| API | Unit生成と所持Unitの走査・ID検索・型 | `DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:194,239`、`DLC/WarMachineScenario/Scripts/WarMachineScenario.lua:186`、`DLC/PiratesScenario/Scripts/PiratesScenario_StartScript.lua:1072,1554` |
| API | 都市の検索、生存・大勢力・AIの検査 | `DLC/VikingsScenario/Scripts/VikingScenario.lua:95,261`、`DLC/PiratesScenario/Scripts/PiratesScenario_StartScript.lua:866`、`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:122` |
| API | 接触・戦争状態の検査、シナリオの接触入力 | `DLC/Expansion2/Scripts/WorldCongress.lua:301`、`DLC/VikingsScenario/Scripts/VikingScenario.lua:77`、`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:1308` |
| 実機準備 | 安価な知識の付与、UIの研究選択・進捗と費用 | `DLC/VikingsScenario/Scripts/VikingScenario.lua:38-39`、`Base/Assets/UI/Screens/TechTree.lua:239-245,1107,1112-1114`、`Base/Assets/UI/Screens/CivicsTree.lua:239-245,1251,1279,1287-1289` |

Unit生成がUnitを返さないこと、Gameplayの外交支持は`Player:GetDiplomacy():GetFavor/ChangeFavor`であることは依頼者の実機検証済み事実。生成前後のUnit IDを比較し、戻り値には依存しない。UIの`Player:GetFavor()`をGameplayでは使わない。今回の型と全効果の成功は下記の実機で確認する。

## 検証

```powershell
python -m unittest discover -s tests -t .
python -m unittest discover -s tools/civ6-autotest/tests -t .
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/religion-persistence.json --check
```

`tests/test_religion_persistence.py`はlupaを必須importし、スキップしない。SQLの定数とModifierを使ってLuaを実行し、費用・産出・移管・再利用拒否・AI・旧保存・失敗を検査する。保存は外部JSONと新しいLua VMで模擬し、残高・Unitのスタブも別に保持する。Civ6の動作保証の代用ではない。

実機はメインセッションでMODを配置し、日・英・露それぞれで次を実行する。

```powershell
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/religion-persistence.json
```

万世紀・織田／武田・小地図・標準速度で開始。都市を建設し、創始保存の検証用に標準`RELIGION_CUSTOM_1`と既存Cathedral信仰1件を使う。D8の宗教・教義案ではない。

| 保存・再読込の段階 | 自己検査 |
|---|---|
| 公開 | 信徒6・施設1、資料5種、別宗教の分離 |
| 地下化 | 発祥者滅亡通知でも他勢力の信徒1を保持。`end_turn`の`turns=2`で実際に2ターン進んだことも外部比較 |
| 失われた宗教 | 最後の信徒0、地下持続の根拠と資料を保持、自動再興なし |
| 発見 | 教典発見は未解決。別資料の売却でゴールド40 |
| 再興 | 信仰60減、信徒2・施設0、資料の利用権を消費し現物と履歴を保持 |
| 公開へ復帰 | 施設1で公開、再興履歴を保持 |
| 全8分岐 | 秘匿・解釈・吸収・収蔵・返還・仮英雄。人間のゴールド+40、信仰−40（吸収+20・英雄−60）、支持+10、斥候+1。AIの売却+40。UIで科学+20・文化+50を外部比較 |

64ステップ、7回のsave→メニュー→load。台帳・手番・残高・研究進捗を外部の`capture_as`と`equals_capture`で完全比較する。ゲーム内の期待値と台帳が同時に消えても成功にならない。各分岐の再実行はエラーと台帳不変を検査。最後のロードでは残高・Unit・研究進捗も比較し、ロード時の二重付与を検出する。不一致は`ui_lua`の`error()`か比較エラーで失敗する。ドライバーは`end_turn`の回数を尊重し、複数ターンで共通の期限を使う。

## 結果と未検証

2026-10-01：宗教PoCのlupaテスト36件を含む全362件、自動検証ツール152件が成功。64ステップの形式チェックとLua39片の構文検査も成功。実機はこの作業では実行しない。独自表のGameInfo参照、入れ子・空テーブル・false値・使用履歴の保存、GameCore_Tunerの参照、科学／文化Modifier・外交支持・隣接Unit生成、日英露表示は要実機確認。マルチプレイ、自然な探索・布教・迫害、建物／人口／人物／本体Vaultの同期は未検証・未接続。

指定のテスト検出では、制限付きWindowsトークンによる0700一時フォルダーの再アクセス不可と、自動検証ツールの名前空間パッケージをunittestが検出できない問題があった。一時領域だけの実行補助で、新規一時フォルダーに親の権限を継承させ、既存の名前空間パッケージを検出して全件実行した。既存ACL・リポジトリのテスト・__init__.pyは変更していない。別モデルのレビューはCodexの起動時にAccess deniedで停止し、未実施。
