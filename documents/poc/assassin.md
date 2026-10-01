# 暗殺者 PoC（束2、SSOT 9.5）

正本は [9.5](../spec/ssot/09-assimilation-religion.md) と [16.1 の暗殺者行](../spec/ssot/16-roadmap.md)。この記録は仕様の追加ではない。未決の効果量は `JAC_Assassin_Rules.lua` の `PROVISIONAL` に集め、オーナーの決定として扱わない。

実装は `poc/JAC_PoC_Assassin/`。万世紀ルールセットと、日本15/20 kmの史実領土版・開拓者版の組合せだけでデータ・Gameplay・UIを読み込む。戦国ツリーの LoadOrder 200 より後の350で追加する。オーナー回答はSSOT 9.5へ反映し、回答済みのQ73〜Q78は質問リストから除去する。

**実機確認の結果と留保は末尾に記録する。** 2026-10-01の所有者の回答（自由都市への分裂、複数要因による隣国吸収、外交支持による採決、血縁優先で兼任しない継承）はSSOT 9.5に記録した。自由都市は公式の移管APIを呼ぶ。継承者・人物の所属は独自台帳で表し、Civ6本体の肖像・特性、Governor効果、外交罰則、正式世界会議の議題は未接続である。

## 規則との対応

| SSOT 9.5 の規則 | 実現方法／留保 |
|---|---|
| マップ上を歩くUnit、都市内の人物・Unitへの任務 | 専用の陸上Unit、移動3・視界2・通常の経路探索。潜入は公式の他国領土進入効果を参照。画面で実在する暗殺者Unitを選び、対象から6タイル以内で実行する。指導者の所在は暫定で首都、Governorは人物登録簿の配属都市。対象への瞬間移動を通常コマンドに含めない |
| 通常100ターン・最低20ターン | 都市を選ぶ独自の育成時計。`ProductionTurns` が補正後に20を適用し、速度換算して `Start/Due/Turns/Speed` を保存。開始からの経過ターンを現在の総所要時間と比較する。完成時にUnitを生成。通常のCiv6都市生産キューとハンマーへの接続は未実装で、育成と通常キューは並行する。育成制度の成立性を検証する代替経路であり、通常の都市生産への統合まで完成したとはしない |
| 外的要因と治安の方向 | 建物・区域・政策・政体・育成する都市の武将特性／ジョブ・治安の各入力を分離。治安0で80、50で100、100で120ターン、他の単位補正は暫定で10/10/10/10/15/15の短縮。Q78に従い毎ターンの現在の入力から再計算する。育成期間に外交関係を含めるかはSSOT内の記載が一致せず、[質問リスト](../questions.md)で確認中。列挙された育成要因だけを使う案を仮実装し、外交関係の育成補正を外した。任務待機への外交関係補正は維持する。入力は後述のプロパティ契約。実際の建物・政策・人物台帳からの自動接続は未実装 |
| 同時に1都市 | 国別Player Propertyに `Production` を1つのみ持つ。別都市からの開始は拒否。育成を中止すると進捗は失い、他都市で最初から。中止と都市喪失の扱いは暫定 |
| 完成後全国10ターンロック | 実際にUnitを生成したターンを `LastCompletion` に保存し、`ProductionUnlock=完成+換算10`。完了+9は全都市で拒否、完了+10で許可。都市変更・育成取消・ロードで解除できない。生成場所が塞がる場合は完成を延期し、成功した生成時点からロック |
| 育成とロックと任務は別時計 | `Production.Due`、`ProductionUnlock`、`Mission.Due`、`CommandUnlock` は別フィールド。育成期限は毎ターン更新し、他の3期限はそれぞれの起点で固定する。生産ロック中にも既存Unitの任務は可能 |
| 対象別の基本成功率 | 指導者1%、Governor15%、Great Person30%、Hero50%、一般Unit（暗殺者含む）50%、Builder系90%。Unitの実際の種類から分類し、要求側がBuilderを一般Unitと偽っても拒否 |
| 治安・関係・護衛で成功率補正 | 相手治安・両国の関係・護衛の各倍率を掛ける。暫定式は `基本×(1+(50-治安)/100)×(1+0.25×関係)×(1-0.5×護衛)`、0〜100へ制限。関係は戦争で1、他は入力値。自国の治安はコマンド待機に使用 |
| 任務期間 | Great Person2・Governor3・指導者5に、距離1以下×1、3以下×1.5、それ以上×2、切上げ。その後速度換算。距離区分と最大距離6、Hero／Unit／Builder基準2は暫定。`nil`にすれば未決の対象の任務を停止できる |
| 実行時に成否を固定、ロードで再抽選しない | Gameplayの同期乱数 `Game.GetRandNum` で成否・発覚を別々に抽選し、数値のロール・結果・期限・対象の同一性をPlayer Propertyに保存。指導者の場合は分岐用ロールと抽選結果も保存。解決時に乱数を呼ばない |
| 全国コマンド待機50・最低1 | 前回実行時点に `LastCommand/CommandUnlock` を保存。自国治安最低10、関係最悪10、訓練所10、Governorスキル7、Heroスキル6、Great Personスキル6を合算すると49短縮。全6項目が最大でのみ通常速度1に到達。Unit・対象国を変更しても回避不能。失敗しても起点は実行時 |
| 進行中の次任務禁止 | 待機が既に切れていても、国別の `Mission` が1つあれば開始を拒否 |
| 失敗と発覚の独立判定 | 発覚率25%は暫定。未発覚失敗／発覚失敗／未発覚成功／発覚成功をテスト。共通罰則は両方が起きても一度のみ計上 |
| 共通罰則 | 相手の外交台帳へ友好度-10、不平+25を保存。自作の関係補正入力にも反映。本体の友好度・Grievancesへの変更は未確認・未接続。台帳とネイティブ表示の一致は未達 |
| 失敗のみ | 暫定では死亡を選択し `UnitManager.Kill`。捕縛・拘禁・釈放の仕様は未決のため追加しない |
| 発覚のみ | 対象国に暗殺の開戦理由、第三国に非難の記録を保存。本体のCasus Belli／非難の強制は未接続。日本マップに世界会議はないため非難議題を捏造しない |
| 指導者死亡後に実行者へ全国土を自動譲渡しない | 当該指導者の死亡と処遇をPlayerとGameの国家履歴へ保存。自由国家は全都市をFree Citiesへ移す。吸収は複数要因の最大得点の隣国へ移す。継承は同じ枠を維持し、後継者は新しい人物IDで暗殺対象になる |
| 死亡後の四分岐 | SSOTの2026-10-01の決定を `POST_DEATH` の名前付き定数で実行する。日本では公式世界会議の代わりに、通常プレイでも本体の外交支持を重みにする採決を行う。候補者なし／隣国なしの案は除外する |
| 状況に応じた重みと実行時固定 | 分岐の抽選用重みと受入国の採点は別。従来の分岐抽選用暫定重みを維持し、受入国の採点入力・重み・得点、採決の参加国・票・外交支持・票数、後継者、結果を実行時に固定保存する。係数と式の正本はSSOT 9.5 |
| 文化圏の名称 | 日本の20家は暗殺忍者 / Assassin Ninja / Ниндзя-убийца。アイヌ・琉球・未確認の国は共通名の暗殺者 / Assassin / Убийца。日本マップ全域へ日本名を一律割当せず、候補表の中国・朝鮮・ニザール派の名称も自動割当しない。日本型と共通型は名前だけを分け、同じ時計・制限・確率を使う |

## 指導者死亡後の四分岐の実装

方針・係数・式は [SSOT 9.5](../spec/ssot/09-assimilation-religion.md#指導者死亡後の国家の行方) が正本である。ここには実装と検査の対応だけを記録する。所有者が回答した選択肢は提案・質問として残さない。

### 自由都市へ分裂

`POST_DEATH.FreeState="ENGINE_FREE_CITIES"`。対象国の都市・UnitのID一覧を先にコピーし、Unitを解散してから各都市へ `CityManager.TransferCityToFreeCities` を呼ぶ。最後の都市を移す前に、死亡指導者・存命人物・元の家・旧役職をGameの履歴へ保存する。存命人物は無所属になり、除去済みのUnitやGovernorを現在の役職として参照しない。自由都市の所有者は `PlayerManager.GetFreeCitiesPlayerID()` で取り、番号を固定しない。

旧国のプレイヤー枠を手動で削除・作り直す操作はしない。資産喪失後の本体の通常敗北に委ね、自由都市は本体のFree Cities AIに任せる。移管後にCity・Unitオブジェクトを使わず、履歴を座標と人物IDで保持する。読み込み後はGameの履歴を優先し、再移管・再生成しない。自由都市の忠誠心と再取得は本体に従う。旧国の敗北・AI停止・自由都市の保存は実機シナリオで検査する。

### 隣国による吸収

`POST_DEATH.Absorption="WEIGHTED_NEIGHBOR_DISBAND"`。`Snapshot` が現存主要国の軍事力・都市数・同盟・敵対度・領土の境界辺数・最短都市間距離・多数都市宗教・文化入力を取り、`Selection` と各候補の原値・正規化値・得点を保存する。得点順、同点はPlayer ID順で選ぶ。式と名前付き暫定重みはSSOT 9.5に記載した。APIがない軍事力は都市数で、取得できない宗教・文化・国境はその項目を除いて計算する。

全都市を固定した受入国へ移管し、旧国の全マップUnitを解散する。存命Great Person・武将・Governorは受入国の人物登録簿へ加入する。死亡人物を別のUnitトークンの人物として再登録しない。都市の建物・区域・人口は本体の移管処理に従う。旧枠の敗北は本体に委ねる。受入国が解決前に失われた場合は自由都市へ分裂し、再選定しない。ネイティブGreat Person再付与・Governor任命や技能移管は未接続である。

### 外交支持による採決

`POST_DEATH.Congress="FAVOR_WEIGHTED_JAPAN_BALLOT"`。このPoCは日本マップ専用で、[世界会議能力の削除](japan-no-world-congress.md) を維持する。公式UIの世界会議の投票・費用表を参考に、Gameplayの `Player:GetDiplomacy():GetFavor()` を読む代替採決を通常プレイでも行う。テスト専用の1国1票を廃止する。外交支持を票数へ変換する暫定式、票未入力時の選び方、同点と無効票の扱いはSSOT 9.5に記載した。

実行時に各国の票と外交支持・票数を保存する。任務中に支持や票が変わっても再集計せず、保存済みの勝者の資産処理を実行する。代替採決は外交支持を消費しない。朝廷・将軍の裁定や正式世界会議の開催・任意議題送信・投票UIを実装したという意味ではない。

### 後継者が継承

`POST_DEATH.Succession="KIN_FIRST_SINGLE_ROLE"`。登録済み存命候補を血縁優先、同順位は人物ID順で固定する。人物を捏造せず、Great PersonはUnitの個体トークンと登録人物ID、Governorは登録人物IDを照合する。Great PersonのUnitを除去し、Governorは人物登録から退役させる。非血縁の国名は提供された名称がある場合だけ使う。候補者が解決前に失われた場合は自由都市へ分裂し、別候補を選ばない。本体の肖像・固有特性・AI性格とGovernorのネイティブ効果の交換は未接続である。

## 保存と入力契約

保存単位は国別Player Property `JAC_ASN_STATE_V1`、対象国別 `JAC_ASN_NATION_V1`、Gameの国家履歴 `JAC_ASN_NATIONS_V1`、人物登録簿 `JAC_ASN_HOUSE_PEOPLE_V1`、外交台帳 `JAC_ASN_DIPLOMACY_V1`。Game Property `JAC_ASN_TICK` に最後に処理したターンを保存し、同ターン再処理・ロードで二重完成を防ぐ。外交台帳の `Applied[任務ID]` で罰則の重複を防ぐ。完成状態を通知前に保存し、通知エラーは捕捉する。終了した `Production/Mission` は `nil` で除去する。空文字にしない。自由都市移管と入れ子の育成・任務・人物の保存復元は、b5baf41のassassin-05と58c3ecbのassassin-06で2回ずつ確認済み。58c3ecbで追加した総督の元役職IDはLuaで検証し、実機では未確認。継承・吸収・採決後の3回目の保存復元は完走結果で確認する。

UnitにはGame Propertyで連番を発行した個体トークンを保存する。登録済み偉人には、生死や候補の採否と独立した人物IDも `JAC_ASN_PERSON_V1` に保存する。人物IDとUnit ID・個体トークンの対応をPlayerの `JAC_ASN_PERSON_UNITS_V1` へ永続保存する。候補を再列挙しても対応を書き換えず、同IDの別個体を旧人物へ結び直さない。Unitを人物IDへ結び付けてから候補を絞り、死亡した人物をUnitトークンの別人物として登録し直さない。同ID・同型の別個体への置換でも対象をすり替えない。任務中に暗殺者が消失した場合は、Q76の回答どおり成功・失敗・発覚の全結果を取り消す。成否と発覚の組合せにかかわらず、固定済みの友好度-10・不平+25だけを一度適用し、全国待機を保持する。捕縛・死亡、開戦理由、第三国の非難は起こさない。物理的対象だけが消失してもペナルティは免除しない。通知例外からの回復はテスト済みだが、エンジンの全副作用とプロパティ保存をまたぐトランザクションは保証していない。

処理対象は存命国の列挙だけに依存しない。育成・任務を持った国のIDを `JAC_ASN_ACTORS_V1` に保存し、国家滅亡後も任務の終了と固定済み罰則を記録する。滅亡国の育成は取り消し、成功効果は適用しない。滅亡後のPlayer Propertyへのアクセス継続は要実機確認。再レビューで実行者消失の同じ領域を指摘されたため、Unitの存在確認だけを足す修正から、保存した仕事の所有国を追う設計へ改めた。

所在地は暫定で、指導者／Governorの都市は発見済み、Unitは視認中を必要とする。UIとGameplayの双方で検査し、不可視Unitを一覧へ出さない。未決のため `PROVISIONAL.Visibility` で切替可能にする。

`JAC_ASN_INPUT_V1` は後続コンテンツから入力するためのテーブルであり、プロパティを設定したという事実を「実際にその建物がある」「実際にその技能を解放した」という検証の代わりにしない。

| 所有者 | 入力 |
|---|---|
| Player | `Security`（0〜100、未設定50）、`Policy/Government`、`School/Governor/Hero/GreatPerson`（0〜1）、`Relations[対象IDの文字列]`（敵対度：-1が最良、1が最悪、戦争時1）、`Guard`（0〜1）、`Successor` または `Successors`（SSOT 9.5の登録形式）、`CultureGroup`（任意）、`Congress={Enabled,Votes}`（代替採決の通常入力）、`Regions.Unrest`（0〜100） |
| 育成する都市 | `Building/District/Trait/Job`（補正の単位数） |
| 人物化したGovernor | 対象Playerの `Governors[ID文字列]={City=都市ID,Identity=人物ID,Name=LOCタグ,Dead=0/1}`。通常Governor枠からの列挙・死亡処理は未接続 |
| Unit | `Hero=1` は独自Heroの分類用。通常のHeroesモードは `UNIT_HERO_` の型名で分類 |

`ExposedMembers.JAC_Assassin` は状態読取・Tuner検査用。通常の画面操作は `UI.RequestPlayerOperation(..., EXECUTE_SCRIPT, {OnStart='JAC_AssassinOperation', ...})` によってGameplayへ送る。ローカル手番が非アクティブな間は送信しない。通常リクエストの成否指定は受け付けない。

強制結果は実機シナリオ専用。GameCoreで `Game:SetProperty('JAC_ASN_TEST_MODE',1)` を明示し、実行者へ `JAC_ASN_FORCE_NEXT={Success=1/0,Detected=1/0,Branch='FREE_STATE'等}` を設定する。1回で消費し、最後にテストモードも `nil` に戻す。通常UIに強制結果を入力する操作はない。

待ち時間の短縮もテスト専用。`ExposedMembers.JAC_Assassin.TestAdvanceDeadlines(playerID, clocks)` は保存した育成・任務のDueと全国育成・コマンドのUnlockを現在ターン+1まで短縮する。`clocks` 未指定なら4時計、指定なら `Training / Mission / ProductionLock / CommandLock = true` の時計だけを対象にする。既に切れた期限を延ばさず、成否・ロール・分岐・補正・開始ターン・速度・計画を変えない。育成の `TestDue` を保存し、毎ターンの通常の所要時間再計算を続けながら、テスト中だけDueへ上限を適用する。ゲームのターン数や処理済み印を飛ばさず、フック内で完了・生成・移管は行わない。実際の `end_turn` を1回行って本来のターン処理へ渡す。テストモードが1でない場合は `TEST_MODE_OFF` を返し、保存状態を一切変更しない。通常の操作送信にこのフックは公開しない。


## 公式ファイルの根拠

基点を `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/` とする。公式ファイルは変更していない。呼出し例はそのAPIの存在の根拠であって、このMODの実機動作を保証しない。

| 使用箇所・仮定 | ファイル:行／限界 |
|---|---|
| Unit定義、移動・訓練・購入属性 | `Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:2833`、スカウト定義 `Base/Assets/Gameplay/Data/Units.xml:771`。`CanTrain=0`・購入用Yieldなし・アップグレード元／先なしで通常キュー・購入による時計回避を閉じる |
| 潜入 | `Base/Assets/Gameplay/Data/UnitAbilities.xml:552`、`:948`、`:1392` の `MOD_ENTER_FOREIGN_LANDS`。専用クラスへ接続。道以外を禁止する規則は追加していない |
| Unit生成 | `DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:193` の `InitUnitValidAdjacentHex`。隣接生成・塞がった場所・新しい型のモデルは要実機確認 |
| Unit死亡 | `DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:807`。Kill後にUnitのオブジェクトを使わない |
| 首都、都市列挙 | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:1103`、`:136` |
| Unit検索 | `Base/Assets/UI/Panels/UnitPanel.lua:424` の `GetUnits():FindID`。Gameplayでの呼出しは要実機確認 |
| 距離 | `DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:132` の `Map.GetPlotDistance` |
| ゲーム速度 | `Base/Assets/Gameplay/Data/GameSpeeds.xml:19`、`:28`、`:37`、`:46`、`:55`。換算規則の正本はSSOT1.3。暦・元号の開始ターンを育成時計へ混ぜない |
| 同期抽選 | `DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:526`、`DLC/WarMachineScenario/Scripts/WarMachineScenario.lua:261` の `Game.GetRandNum`。MPの端末一致は未実証 |
| ターン処理 | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:64`、`:457` の `GameEvents.OnGameTurnStarted`。ロード直後のUI初期化はGameplayに置かない |
| UI→Gameplay | `DLC/BlackDeathScenario/UI/BlackDeathScenario_PopupHandler.lua:22` の `EXECUTE_SCRIPT`、Gameplay側 `Scripts/BlackDeathScenario.lua:66` のGameEvents登録。手番前の送信が落ちることは本ジョブに渡された実機確認済み事実 |
| プロパティ保存 | `DLC/PiratesScenario/Scripts/PiratesScenario_GameCore_Script.lua:119`、`:129` のテーブルGame Property、`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:373` のPlayer Property。今回の入れ子テーブルの保存はシナリオで検査する |
| Unit個体のプロパティ | `DLC/PiratesScenario/Scripts/PiratesScenario_GameCore_Script.lua:75` のUnit Property読取。Unit Property保存は束0で確認された方式を使用、今回の個体トークンは要実機確認 |
| 視界の検査 | `DLC/PiratesScenario/Scripts/PiratesScenario_Shared_Script.lua:353`、`:359` のGameplay `PlayersVisibility/IsRevealed`、`:1044` の `IsVisible(X,Y)`。今回の条件は要実機確認 |
| 通知 | `DLC/PiratesScenario/Scripts/PiratesScenario_GameCore_Script.lua:136` のGameplay翻訳、`Scripts/PiratesScenario_Shared_Script.lua:334` の `NotificationManager.SendNotification`。実際の表示は要実機確認 |
| UIの選択・表示名 | `Base/Assets/UI/Panels/UnitPanel.lua:4262`、`Base/Assets/UI/ActionPanel.lua:775`、`Base/Assets/UI/DiplomacyDealView.lua:231`。選択Unit取得と更新イベントは同UnitPanelの呼出し例に従う |
| UIの更新と選択Unit | `Base/Assets/UI/Choosers/CivicsChooser.lua:464` の `GameCoreEventPublishComplete`、`Base/Assets/UI/Panels/UnitPanel.lua:2371` の `UI.GetHeadSelectedUnit()` |
| 都市／Playerの入力テーブル | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario_UnitCommandDefs.lua:112` のCity Property、`DLC/PiratesScenario/Scripts/PiratesScenario_Shared_Script.lua:541`、`:546` のPlayerテーブルProperty |
| 国ごとのUnit名 | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:115` のGameplay `PlayerConfigurations[id]:GetCivilizationTypeName()`。日本の20家を明示列挙し、未確認の国は共通型を生成する |
| 生存主要国の列挙 | `DLC/PiratesScenario/Scripts/PiratesScenario_Shared_Script.lua:77` の `PlayerManager.GetAliveMajors()`。そのIDで並べ替え、列挙順に依存しない |
| VFSの同名Lua衝突回避 | `DLC/Expansion2/Expansion2.modinfo` の `UI/Replacements/CitySupport.lua` インポート。Gameplayは `JAC_Assassin_Gameplay.lua`、UIは `JAC_AssassinPanel.lua` に分離 |
| 本体の友好度変更 | `Debug/Diplomacy.ltp:33` の `GetAi_Diplomacy():AdjustBaseDiplomacy` はTuner例。通常Gameplayで動くとは断言しないため呼ばない |
| 本体の不平・非難・開戦理由 | 不平は `DLC/Expansion2/UI/Additions/DiplomacyActionView_WorldCongressTab.lua:42` の読取だけ確認。非難は `Base/Assets/UI/DiplomacyActionView.lua:469` のUIセッションで、Gameplayからの第三国強制経路ではない。`DLC/NubiaScenario/Scripts/NubiaScenario.lua:172` は実際の開戦であり理由の付与ではない。未検証のSetterを作らない |
| 吸収の都市所有移管 | `CityManager.TransferCity(city,recipient)` は既存実機で確認済み（[天下統一PoC](unification-victory.md):15,82,86、`tools/generate_unification_victory.py:277`）。公式 `DLC/Expansion2/Binaries/Win64/GameCore_XP2_Release.dll` の登録文字列（オフセット14318752）。公式Luaの同じ呼出し例は未発見なので、ここはテキスト行での根拠がない。`Debug/City.ltp:277` のFree Cities用APIとは区別する。暗殺者による移管は要実機確認 |
| 0都市での敗北・旧AI停止 | `Base/Assets/Gameplay/Data/Victories.xml:441` の敗北条件。最後の都市を移す前に全Unitを解散。自然敗北・旧AI終了とロード後の退役は要実機確認 |
| 指導者・国名の本体差替の限界 | `Base/Assets/UI/WorldBuilderPlayerEditor.lua:1250` の交換操作はWorldBuilder専用。通常Gameplayへ持ち込まない。PoCは人物IDと国名を独自UIへ表示する |
| Great Personの人物と再付与 | `DLC/NubiaScenario/Scripts/NubiaScenario.lua:106` の `GetGreatPerson():GetIndividual()` と `Game.GetGreatPeople():GrantPerson`。吸収aでは再付与せず人物登録簿へ渡す。ネイティブ技能・Governor任命は未接続 |
| ネイティブGovernor任命 | `DLC/Expansion2/UI/Additions/GovernorPanel.lua:547` はUIからの操作送信。PoCの人物登録の退役を、ネイティブ効果の解除と同一視しない |
| 世界会議の公式票費用と投票 | `DLC/Expansion2/UI/Additions/WorldCongressPopup.lua:575` の `GetVotesandFavorCost`、`:983,988,991,998` の費用表による票数増減、`:2242,2251,2261` の投票操作。費用表と `UI.RequestPlayerOperation` はUI側。日本は会議能力を削除するため任意の議題を送らず、外交支持による代替集計を行う。追加票の暫定費用を公式の同一数式とはしない |
| Gameplayの外交支持 | `Debug/Player.ltp:694` の `GetDiplomacy():GetFavor()`、`:365,379` の `ChangeFavor()`。通常採決では前者を読み、実機テストの票数差を作るときだけ後者を使う。UI側の `Player:GetFavor()` をGameplayへ混ぜない |
| 自由都市へ移す本体API | `Debug/City.ltp:277` の `CityManager.TransferCityToFreeCities(pCity)`。`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:46,47` の自由都市プレイヤー取得。本体の忠誠心／自由都市表示は `DLC/Expansion2/UI/Additions/LoyaltySupport.lua:25,27,28` と `DLC/Expansion2/UI/CityBanners/CityBannerManager.lua:1831,1832`。今回は忠誠心を独自計算せず公式のFree Cities移管経路を直接呼ぶ。Gameplayからの全都市連続移管は要実機確認 |
| 隣国の軍事力・同盟・宗教・文化 | 軍事力は `Base/Assets/UI/ARXManager.lua:160` の `GetStats():GetMilitaryStrength()` を参考に、Gameplayでgetterがあれば使う（可用性は要実機確認、なければ都市数）。同盟はGameplayの `DLC/VikingsScenario/Scripts/VikingScenario.lua:77` の `HasAllied`。`GetAllianceType` は使わない。多数都市宗教は同ファイル`:88,90` の `GetReligion()/GetReligionInMajorityOfCities()`。パンテオンの除外は `Base/Assets/Gameplay/Data/Religions.xml:38`。文化の弱い代理値 `Ethnicity` は `Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:666` と `Base/Assets/Gameplay/Data/Civilizations.xml:151` |
| 国境の長さ | 全タイルはGameplayの `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:490,491`、所有者は`:396`、隣接タイルと `ipairs` は `DLC/PiratesScenario/Scripts/PiratesScenario_Shared_Script.lua:159,160,162`。対象国から相手国に接する辺を一度ずつ数える |

PoC画像は公式スカウトのアイコンを参照するだけで、公式DDSを同梱しない。参照は `Base/Assets/UI/Icons/Icons_Units.xml:32`、`:133`、`Icons_UnitPortraits.xml:77`、`Icons_UnitFlags.xml:47`。新Unitの専用3Dモデルは未作成で、表示は要実機確認。借用理由は育成・任務の技術検証を先に行うため。完成版の固有画像の条件はSSOT15.2.1に従う。

## 検査と実機シナリオ

`tests/test_assassin_lua.py` はlupaを必須インポートし、Luaテストをskipしない。育成100／20と5速度、各ターンの延長・短縮、20ターン下限と完成後10ターンの境界、2都市・2Unit、全成功率、成否と発覚の4組合せ、暗殺者消失、全分岐の処理・受入先消失・偉人の二重所属防止・保存復元を検査する。追加の検査は、自由都市の本体API呼出しと人物履歴、受入国の全6要因と同盟・同点・未取得項目、外交支持の多い1国が支持のない多数国を上回る採決、テストフック無効時の不変性・選択時計・ロード後再計算と次ターン完了である。

実機シナリオは `tools/civ6-autotest/scenarios/assassin-leader-save-load.json`（79手順、**ターン終了は16回**）。`tools/generate_assassin_poc.py` の `build_scenario()` が同じデータを返し、テストでJSON全体の一致とターン数を検査する。assassin-04で人間のターン終了に約150秒かかったため、各手順の待機上限は600秒にする。16回ならターン処理の目安は約40分で、保存・ロード・開始の時間は別にかかる。

1. 日本15 km・万世紀・22家、織田家を人間として開始。AI松平家への成功・未発覚・自由都市分裂を強制し、育成と任務をアクティブ手番の同期操作で開始する。Unit生成APIの戻り値は使わずID差から見つける。通常100／最短20、1都市制限・別Unitの任務拒否を検査する。
2. ターンを1回ずつ進めて治安の変化で育成期限100→120→80を検査する。途中のsave→メニュー→loadで育成の全フィールドと任務のロール・スナップショット・処理計画を比較する。その後だけ期限を短縮し、次の実ターンで育成と自由都市移管を解決する。各都市の実所有者、旧枠の実際の敗北、旧軍の除去、存命偉人の無所属履歴を検査する。
3. 完成時の全国育成ロックが完成+10であることを検査し、全都市で開始を拒否する。2回目のsave→loadで国の履歴・人物・自由都市の所有とロック状態を比較する。保存した本来の+9拒否／+10許可を規則関数で確認した後、ロックだけを短縮する。短縮後も直前は全都市で拒否し、実ターンで境界に達した後、別都市で通常の20ターン期限を検査してから短縮・完成する。**100／80／20ターンの実経過は短縮しており、通常の経過時間の境界はlupaで検証する。**
4. 別々のAI国で継承・吸収・採決を実行し、各計画の固定を確認してから次の実ターンで解決する。Great Personの除去と後継者の照合、候補の原値から式を独立に計算した最大得点の受入国、全都市移管・Unit解散・人物加入・旧枠敗北を検査する。採決は外交支持の多い1国の継承票が多数国の自由都市票を上回るように本体のFavorを変え、任務中に票とFavorを変えても結果が変わらないことを確認する。テスト用に変えたFavorは検査後に戻す。
5. 継承で存続する今川家のUnitを別の暗殺者で狙い、成功／失敗×発覚／未発覚の4組合せで暗殺者を解決前に除去する。各任務の期限だけを短縮する。対象生存、取消済み結果、固定の友好-10・不平+25が一度だけ適用、開戦理由・第三国非難なし、元の全国待機保持を検査する。次の任務の前には待機も短縮し、1回の実ターンで期限を通過する。
6. 3回目のsave→loadで全国家履歴・人物・計画・状態・罰則の値と適用済み任務一覧を保存前の値に比較する。ロード直後と次のターンで二重適用・旧枠の復活がないことを検査する。最後にテストモードを解除し、フックが状態を変えないことを確認する。

合格の判定は各 `ui_lua` の `assert` と `japan_map` のログ判定で行う。物理的な都市所有や枠の生存は本体のgetterで確認し、台帳だけで成功としない。

2026-10-01の実機 `assassin-05`（対象b5baf41）は手順28まで成功した。育成期限100→120→80、次の実手番での育成完了、全国ロック、松平家の全都市の自由都市移管・全Unit解散・本体の敗北・存命人物の無所属履歴を確認した。2回の保存と読み込みで育成・任務・都市所有・人物・ロックの一致も確認した。これは全シナリオの合格ではない。

手順29は、外交画面を一度閉じた後に別のAIが使節団を提案し、ターンが進まず失敗した。`end_turn`の待機中に外交画面を順に閉じる処理と、ターン終了を一度だけ要求する回帰テストを追加した。最初の起動失敗（`assassin-05-try1`）は前の試験のクラッシュ報告プロセスを終了して回復した。起動失敗は同じ成果物ルートの `assassin-05-try1/steps.jsonl` に記録されている。成果物は共有実機用worktreeの `tools/civ6-autotest/artifacts/assassin-05/` にあり、`Lua.log`、`Modding.log`、`steps.jsonl`、停止時の `turn-stall.png` と原因の `diagnosis.txt` を含む。

修正後の再試験とレビューの最新結果は [PR #81のコメント](https://github.com/AutoDevJapan/civ6-banseiki/pull/81) に記録する。新しい四分岐の全体合格を判定するには、継承・隣国吸収・外交支持による採決・3回目の保存復元までの完走が必要。58c3ecbで全体419件・自動ドライバー188件が成功し、Luaの省略なし、生成JSONとの一致とシナリオ構文検査も成功した。

育成要因に外交関係を含めるかは [質問リスト](../questions.md) の回答待ちであり、回答をSSOTへ反映するまでマージしない。滅亡した実行国の仕事のPlayer Propertyと、成功・発覚した指導者任務で対象国が滅亡した後の罰則保存は、追加の実機確認項目として共有ToDo #172に残す。外交罰則の本体表示、通常の都市生産キューへの統合、ネイティブ指導者交換、Governor効果の除去、正式世界会議の開催、3D表示、MP一致は未確認・一部未接続である。


実機 `assassin-06`（コード58c3ecb）は手順29の外交画面を3回閉じてターン4→5へ進み、2都市目の育成完了まで成功した。手順35の継承開始では、検証用に生成した暗殺者だけでは首都が発見済みにならず `TARGET_UNKNOWN` で拒否された。通常の対象可視性チェックは維持し、シナリオの準備だけでテストモードを確認して本体の `ChangeVisibilityCount` により対象の首都・Unitのタイルを可視化する。根拠は公式 `DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:59`。この試験はUnit生成だけで自然に視界が更新されることを保証しない。未発見の対象を拒否する規則はLuaテストで検証する。

視界の準備を修正した後は、全体420件・自動ドライバー188件、生成JSONの一致とシナリオ構文検査が成功した。Luaテストの省略はない。

追加のLua検査では、発見済みの首都と別の座標にいるUnitを分けて扱う。Unitのタイルが発見済みでも未視認なら任務を拒否し、首都だけを可視化しても受理しない。テストモードで対象Unitのタイルを可視化すると、通常の可視性規則を維持したまま任務を受理する。同じタイルを2回準備しても可視性を重ねて加算しない。全体421件が成功した。本体での視界更新の成否は通しの実機結果で別途確認する。

実機 `assassin-07`（b5dd987）は34手順まで成功し、継承の準備で停止した。`steps.jsonl` の手順35は、FireTuner側で取得したタイルを参照する行のnilエラーを記録している。視界準備をGameplayのテスト専用関数 `TestRevealTile` へ移し、呼出側のMapやPlayersVisibilityを使わない設計にした。この関数はテストモードが1でない場合は変更せず拒否し、タイル取得や視界更新が失敗した場合は理由を返す。通常のUI操作からは呼べず、任務の対象可視性規則も維持する。呼出側でこれらのAPIが使えない場合と、首都だけを準備した後もUnit任務を拒否する操作経路をLuaで検査した。実行場所の違いが停止原因だったかと、修正後の成否は次の実機結果で確認する。
