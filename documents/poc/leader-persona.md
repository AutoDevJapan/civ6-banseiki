# 指導者の人物化 PoC

日本マップの22勢力について、加入した指導者を本物の偉人にし、同名の本物の総督へ進化させる実装を用意した。総督の配置・再配置・定着・昇進はCiv6の処理を使う。実機での合格はまだ確認していない。

対象MODは `poc/JAC_PoC_LeaderPersona/`。IDは `91cf7b65-54bb-4f69-a452-19a65527cde2`。万世紀ルールセットの日本15 km・20 kmの各地図と、それぞれの開拓者開始だけで読み込む。データは戦国ツリーより後の `LoadOrder=400` で追加する。

## 仕様から実装への対応

読んだ正本は9.4の加入・進化・総督UIの記述、4章の人物分類・性格・特性、16.1の対象行。これに関係する3.2の総督停止の記述も確認した。仕様の追加・変更は行っていない。

| 正本の規則 | PoCの実装 |
|---|---|
| [9.4:92,129](../spec/ssot/09-assimilation-religion.md): 仲間になった指導者がGreat Personになる | ゲーム側の `ExposedMembers.JAC_LeaderPersona.Join(owner, source, personality, trait)` を加入成立後の接続口にする。配置済み22勢力の指導者から名前・肖像参照・偉人・総督の対応を作る |
| 9.4:92: 能力の使用で退役させず、自国の人物として残す | 専用偉人クラスにポイント獲得源を付けず、各個人の `ActionCharges=0`。加入だけで退役させない |
| [4.4・4.5](../spec/ssot/04-factions-characters.md): 性格と生涯固定の特性を持つ | 接続口に指定された確定8種のコードを保存し、進化後も保持する。人物ごとの未定義の割当は `nil` のままにする。再加入による特性の書換えは拒否する |
| 9.4:133: 進化コマンドで同名のGovernorになる | 人物一覧の「総督へ進化」からゲーム側へ要求し、公式の任命操作へ接続する。エンジンに同名総督が存在することを確認してから偉人を取り除く |
| 9.4:133,271: 配置・定着・一都市一人・昇進は既存Governorの操作 | 公式の総督画面を読み込み、肖像と人物候補の表示だけを変更する。配置と再配置には既存の配置先選択画面を使う。自国の都市への配置を検証する |
| 9.4:135: 人物ごとに固有のスキルツリー | 各人物に別IDの基礎技能と昇進技能を1つずつ定義する。枝の形と取得だけを検証し、追加効果は付けない |
| [16.1:26](../spec/ssot/16-roadmap.md): 人物化と総督の各操作を実ゲームで確認 | `leader-persona-15km.json` が各状態をエンジンの偉人・総督・都市から読み、想定と違えばエラーにする。実機結果は未取得 |
| [3.2:108](../spec/ssot/03-map-and-scale.md): 戦国向けの総督を作るまで公式の称号獲得源を停止 | 公式の称号獲得源は戻さない。検証時だけTunerから称号を1つずつ与える。取得源・進化条件・費用のゲームルールは未決のまま |

「加入」は他の加入機能が成立を決めた後に呼ぶ接続口で、同盟・友好・領土の変更を見て自動成立させる処理ではない。PoCのシナリオはこの口をLuaから直接呼ぶ。元のAI勢力の都市・外交・存亡は変更しない。

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
| 検証用の称号を与える | `G/Debug/Player.ltp:289-295`: `player:GetGovernors():ChangeGovernorPoints(1)`。同ファイル`:654,676`に取得済み・使用済みの称号を読む処理がある。接続口の `GrantTestTitle` は人物と用途ごとに1回だけ与える |
| 総督数の上限 | `G/DLC/Expansion2/Data/Expansion2_GlobalParameters.xml:200`: GSの上限は8。PoC対象地図で22人物＋8枠の30へ広げる。22人同時任命は要実機確認 |
| UIからゲーム側への変更要求 | `G/DLC/BlackDeathScenario/UI/BlackDeathScenario_PopupHandler.lua:21-28` の `EXECUTE_SCRIPT` と `OnStart`。同 `Scripts/BlackDeathScenario.lua:66` と `BlackDeathScenario_Support.lua:116`: ゲーム側はエンジンが渡す要求元プレイヤーを受け取る。要求のパラメータに所有者を指定させない |
| 人物の独自状態を記録する | `G/DLC/WarMachineScenario/Scripts/WarMachineScenario.lua:39-42`: Gameプロパティを読み、未初期化のときだけ設定する。PoCは同じAPIで人物の記録を保持する。セーブ後の復元は本シナリオで要実機確認 |
| ゲーム側での再確認 | `G/DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:246`: `GameEvents.OnGameTurnStarted`。ゲーム側でUI専用のロード終了イベントは購読しない |
| ターンが有効なときだけUIの要求を送る | `G/Base/Assets/UI/ActionPanel.lua:619`: `player:IsTurnActive()`。`G/Base/Assets/UI/Choosers/CivicsChooser.lua:464`: `Events.GameCoreEventPublishComplete`。`G/DLC/Expansion2/UI/Additions/GovernorPanel.lua:742,746`: 総督変更・人間のターン開始のイベント |
| 偉人の選択欄と総督画面に同じ肖像を出す | `G/Base/Assets/UI/Panels/UnitPanel.lua:2229`: 個人の肖像検索を上書きできる。`G/DLC/Expansion2/UI/Additions/GovernorPanel.lua:127-139` と `GovernorDetailsPanel.lua:129`: 総督の肖像コントロール。公式画面を読み込んだ後、対応する既存指導者アイコンに替える |
| アイコンのキーと小さな都市表示 | `G/Base/Assets/Database/IconManager.sql:31-35`: PKは(Name, Atlas)。`G/DLC/Expansion1/Data/Expansion1_Icons_Governors.xml:10-11,27-39`: 都市表示に22 pxのfill/slotが必要。同じ名前の異なるサイズを別Atlasで用意する |

上記はAPI呼出しとデータ定義の一次資料による根拠であり、このMODの実機合格の代わりにはならない。特に専用偉人クラス、ゲーム側からの総督参照、UI差し替えと画像の表示、称号の消費、セーブ後のエンジン状態をシナリオで確認する。

## 状態と失敗時の処理

人物ごとに `JAC_PERSONA_<LeaderType>_<項目>` というGameプロパティを保存する。項目はOWNER、SOURCE、NAME、PORTRAIT、PERSONALITY、TRAIT、UNIT、STATEと検証用称号の付与済み印。人物の状態は次の順に進む。

```text
未加入 -> GREAT_PERSON -> EVOLVING -> GOVERNOR
```

加入時は指定した個人に対応する偉人が実際に所有ユニット一覧に現れたことを確認する。生成に失敗したら加入済みと記録せず、再試行できる。世界全体で1人の指導者に所有者は1人。既に加入した人物への繰り返し呼出しは、別の偉人を作らない。

進化は所有者・偉人の実在・称号をゲーム側で確認してから開始する。UIが総督の任命要求を送る。エンジンの総督がまだなければ偉人を残す。総督を確認し、偉人の除去も確認してから完了にする。再要求やロード後の再処理は二重に除去しない。定着ターン・配置先・昇進はGameプロパティに模倣して保存せず、エンジンの総督から読む。

未設定や進化後のUNITは `nil` にする。空文字のGameプロパティは保存をまたいで残らない、という依頼時の実機確認済み事実に従った。`UnitManager.InitUnitValidAdjacentHex` がUnitを返さないという同じ実機確認済み事実にも依存せず、偉人の個人IDを使って検索する。

## 実機シナリオ

メインセッションがMODを反映し、次を1回実行する。この作業セッションではゲームは起動していない。

```powershell
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/leader-persona-15km.json
```

シナリオは日本15 kmの史実領土開始で、織田を人間、伊達をAIにする。加入口へ伊達稙宗を渡す。`CALM`・`ASTUTE` はシナリオだけの入力で、伊達稙宗の性格・特性を仕様として割り当てたものではない。

83ステップで次を確認する。

1. 22勢力の対応表があり、自動加入や公式の称号獲得源が復活していない。
2. 加入後に本物の偉人が1体だけ現れる。名前・個人ID・肖像参照・使用回数0・性格・特性が一致する。標準のユニット選択欄の肖像検索も一致する。
3. 1ターンとセーブ／ロードを経ても偉人が残る。再加入で増えず、性格・特性を書き換えられない。称号なしの進化は拒否される。
4. 検証用に称号を1つだけ与え、進化コマンドで本物の総督が1人だけ任命される。偉人が消え、総督の名前・基礎技能が一致する。
5. 総督を自国の都市Aへ置く。1ターン定着を進めて保存し、読込後も同じ進行度で、合計5ターンで定着する。
6. 都市Bへ再配置し、Aに残っていないこと、Bで定着が最初から始まることを確認する。
7. 称号なしでは昇進できず、検証用称号を1つ与えるとその人物専用の技能を取得する。Bで5ターン進めて再定着する。
8. 最後の読込後に人物・配置先・定着・昇進が一致する。検証用称号は2個だけ付与・消費され、再付与・再進化で重複しない。

偉人、未配置の総督、定着1ターン目、再配置直後、昇進・定着後の **5回** 保存・読込を行う。人物の状態は各段階の `ui_lua` で一致を断言する。最後に人物一覧と公式の総督画面の画像を保存する。待機は必要な状態が反映されたことを確認するために使う。`EXECUTE_SCRIPT` は人間のターンが有効になってから送る。定着を進める `end_turn` は各ステップで `turns=1` と明示し、1＋5＋5の11ターンを進める。

## 自動検証と制約

`tests/test_leader_persona_lua.py` はlupaを直接読み込み、ゲーム側とUIの実ファイルを実行する。省略はしない。生成・除去の戻り値を空にし、要求を遅らせ、IndexとHashを違う値にして、失敗時の人物保持・所有者検査・再処理・既存画面の接続を検証する。これはCiv6の代わりに作ったLua環境であり、実機結果ではない。

`tests/test_leader_persona.py` はインストール済みGSのスキーマと戦国ツリーの削除処理の後にPoCのSQLを読み込み、22勢力の対応と固有昇進・参照・3言語・地図条件を検証する。既存のデータ読込器は他DLCの行をすべて取り込まないため、外部キーはPoCによって増えた違反を検査する。シナリオ内のLuaもlupaで全件構文検査する。

新規は27件（Lua18件、データ4件、静的・シナリオ5件）。全体の実行結果はメインセッションへの報告に記す。このWindowsサンドボックスではPythonの一時フォルダの権限とテスト探索に補助が必要になった。補助ランナーはOSのTempにだけ置き、リポジトリのACL変更や `__init__.py` の追加は行っていない。

- 加入の条件・国の吸収・人物の後継者選択はこのPoCの範囲外。自動の加入判定を追加していない。
- 人物ごとの性格・特性・技能効果・進化条件と費用は未定義のまま。無効果の技能は取得操作を検証するためのもの。
- 他国都市への派遣は実装していない。全員 `AssignCityState=0`。Amaniの都市国家派遣を他国都市派遣と読み替えない。
- 標準の総督画面では、加入・進化していない人物の候補を隠す。ただしエンジンに22種類を定義しているため、外部Luaから任意の任命操作を直接送った場合まで加入条件を強制する実装ではない。AIの任命選択も未実装。製品化時は取得条件をエンジンにも接続する必要がある。
- 偉人の地図上のモデルは公式の大将軍。新しい人物モデルは作っていない。選択欄・人物一覧・総督画面は既存の指導者肖像を参照する。資料とライセンスは [leader-portraits.md](leader-portraits.md)。大きな総督肖像の切抜き・配置は画像で要実機確認。
- 22 pxの小さな都市表示では対応する指導者Atlasがないため、公式の汎用総督のfill画像を借りる。32 px以上の参照はその指導者の既存Atlas。小さなfill/slotの見た目の違いと22人同時の表示は未検証。
- 偉人が死亡・削除された場合には自動復活させない。進化は `PERSON_MISSING` で拒否する。完成版の人物死亡・再加入・所有権移転は未実装。
- このMODはPoC専用。同時進行のAI任命・マルチプレイ・途中導入・別ルールセットの地図は検証していない。実機合格まで「確認済み」とは扱わない。

## オーナーに残す判断

SSOTの変更や `questions.md` への追加はしていない。依頼どおり、次の選択肢を最終報告にも短く記す。

1. 進化の条件・費用と総督称号の獲得源：a) このPoCでは検証用の称号だけを使い、完成版の設計は別途決める（推奨。未定義のゲームルールを固定しない）／b) 通常の総督称号を使う設計を先に決める／c) 人物の経験・技能を条件にする設計を先に決める。
2. 人物ごとの性格・特性・技能：a) 割当は空、技能は無効果で経路の実証を先に行う（推奨。史実人物の割当を推測しない）／b) 対象の一人から割当と技能を決める／c) 22勢力分を先に設計する。
