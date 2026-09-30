# 暗殺者 PoC（束2、SSOT 9.5）

正本は [9.5](../spec/ssot/09-assimilation-religion.md) と [16.1 の暗殺者行](../spec/ssot/16-roadmap.md)。この記録は仕様の追加ではない。未決の効果量は `JAC_Assassin_Rules.lua` の `PROVISIONAL` に集め、オーナーの決定として扱わない。

実装は `poc/JAC_PoC_Assassin/`。万世紀ルールセットと、日本15/20 kmの史実領土版・開拓者版の組合せだけでデータ・Gameplay・UIを読み込む。戦国ツリーの LoadOrder 200 より後の350で追加する。SSOT・`questions.md` は変更しない。

**実機未確認。時計・抽選・保存データのPoCであり、Civ6本体の外交罰則・指導者差替・国家再編・Governor削除まで実証したものではない。** 下表の「保留」は未実装の本体効果であり、台帳への記録を本体の挙動と同一視しない。

## 規則との対応

| SSOT 9.5 の規則 | 実現方法／留保 |
|---|---|
| マップ上を歩くUnit、都市・Unitへの任務 | 専用の陸上Unit、移動3・視界2・通常の経路探索。潜入は公式の他国領土進入効果を参照。画面で実在する暗殺者Unitを選び、対象から6タイル以内で実行する。指導者の所在は暫定で首都、Governorは人物登録簿の配属都市。対象への瞬間移動を通常コマンドに含めない |
| 通常100ターン・最低20ターン | 都市を選ぶ独自の募集時計。`ProductionTurns` が補正後に20を適用し、速度換算して `Start/Due/Turns` を保存。完成時にUnitを生成。通常のCiv6生産キューとハンマーへの接続は未実装で、募集と通常キューは並行する。募集制度の成立性を検証する代替経路であり、通常生産への統合まで完成したとはしない |
| 外的要因と治安の方向 | 建物・区域・政策・政体・生産都市の武将特性／ジョブ・治安の各入力を分離。治安0で80、50で100、100で120ターン、他の単位補正は暫定で10/10/10/10/15/15の短縮。補正は暫定で募集着手時に固定。入力は後述のプロパティ契約。実際の建物・政策・人物台帳からの自動接続は未実装 |
| 同時に1都市 | 国別Player Propertyに `Production` を1つのみ持つ。別都市からの開始は拒否。募集を中止すると進捗は失い、他都市で最初から。中止と都市喪失の扱いは暫定 |
| 完成後全国10ターンロック | 実際にUnitを生成したターンを `LastCompletion` に保存し、`ProductionUnlock=完成+換算10`。完了+9は全都市で拒否、完了+10で許可。都市変更・募集取消・ロードで解除できない。生成場所が塞がる場合は完成を延期し、成功した生成時点からロック |
| 生産とロックと任務は別時計 | `Production.Due`、`ProductionUnlock`、`Mission.Due`、`CommandUnlock` は別フィールド。生産ロック中にも既存Unitの任務は可能 |
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
| 指導者死亡後に実行者へ全国土を自動譲渡しない | 当該指導者の論理的死亡を `JAC_ASN_NATION_V1` に保存。プレイヤーを滅亡させず、都市の所有者を変更しない。以後その指導者への再暗殺を拒否。本体の指導者表示・AI統治は継続しており、差替は未達 |
| 死亡後の四分岐 | 自由国家・隣国吸収・世界会議・後継者の分岐を保存し、`PENDING_<分岐>` とする。会議なし／候補者なし／隣国なしの枝を除外。自由国家をFree Cityに置換せず、隣国を実行者に固定せず、継承者・改名を捏造しない。重み・統治・資産移管の実行は保留 |
| 状況に応じた重みと実行時固定 | 状況スナップショットと分岐は実行時に保存。「死亡時点の状況」と「実行時固定」の関係は判断待ち。暫定重みは自由国家 `200-治安+地域不穏度`、隣国吸収 `max(100,100×隣国都市数/対象都市数+25×敵対度)`、会議100、継承 `100+治安`。枝の有無を候補・隣接・会議で絞り、合計重みを乱数の範囲に使う。強さの指標も係数も未決であり、確定仕様にしない |
| 文化圏の名称 | 日本の20家は暗殺忍者 / Assassin Ninja / Ниндзя-убийца。アイヌ・琉球・未確認の国は共通名の暗殺者 / Assassin / Убийца。日本マップ全域へ日本名を一律割当せず、候補表の中国・朝鮮・ニザール派の名称も自動割当しない。日本型と共通型は名前だけを分け、同じ時計・制限・確率を使う |

## 保存と入力契約

保存単位は国別Player Property `JAC_ASN_STATE_V1`、対象国別 `JAC_ASN_NATION_V1`、外交台帳 `JAC_ASN_DIPLOMACY_V1`。Game Property `JAC_ASN_TICK` に最後に処理したターンを保存し、同ターン再処理・ロードで二重完成を防ぐ。外交台帳の `Applied[任務ID]` で罰則の重複を防ぐ。完成状態を通知前に保存し、通知エラーは捕捉する。終了した `Production/Mission` は `nil` で除去する。空文字にしない。実機での保存検証はこれからである。

UnitにはGame Propertyで連番を発行した個体トークンを保存する。同ID・同型の別個体への置換でも対象をすり替えない。任務中に暗殺者が消失した場合は、暫定で成功効果を中断するが、固定済みの失敗・発覚ペナルティと全国待機は保持する。物理的対象だけが消失してもペナルティは免除しない。通知例外からの回復はテスト済みだが、エンジンの全副作用とプロパティ保存をまたぐトランザクションは保証していない。

処理対象は存命国の列挙だけに依存しない。募集・任務を持った国のIDを `JAC_ASN_ACTORS_V1` に保存し、国家滅亡後も任務の終了と固定済み罰則を記録する。滅亡国の募集は取り消し、成功効果は適用しない。滅亡後のPlayer Propertyへのアクセス継続は要実機確認。再レビューで実行者消失の同じ領域を指摘されたため、Unitの存在確認だけを足す修正から、保存した仕事の所有国を追う設計へ改めた。

所在地は暫定で、指導者／Governorの都市は発見済み、Unitは視認中を必要とする。UIとGameplayの双方で検査し、不可視Unitを一覧へ出さない。未決のため `PROVISIONAL.Visibility` で切替可能にする。

`JAC_ASN_INPUT_V1` は後続コンテンツから入力するためのテーブルであり、プロパティを設定したという事実を「実際にその建物がある」「実際にその技能を解放した」という検証の代わりにしない。

| 所有者 | 入力 |
|---|---|
| Player | `Security`（0〜100、未設定50）、`Policy/Government`、`School/Governor/Hero/GreatPerson`（0〜1）、`Relations[対象IDの文字列]`（敵対度：-1が最良、1が最悪、戦争時1）、`Guard`（0〜1）、`Successor`、`Regions.Unrest`（0〜100） |
| 生産都市 | `Building/District/Trait/Job`（補正の単位数） |
| 人物化したGovernor | 対象Playerの `Governors[ID文字列]={City=都市ID,Identity=人物ID,Name=LOCタグ,Dead=0/1}`。通常Governor枠からの列挙・死亡処理は未接続 |
| Unit | `Hero=1` は独自Heroの分類用。通常のHeroesモードは `UNIT_HERO_` の型名で分類 |

`ExposedMembers.JAC_Assassin` は状態読取・Tuner検査用。通常の画面操作は `UI.RequestPlayerOperation(..., EXECUTE_SCRIPT, {OnStart='JAC_AssassinOperation', ...})` によってGameplayへ送る。ローカル手番が非アクティブな間は送信しない。通常リクエストの成否指定は受け付けない。

強制結果は実機シナリオ専用。GameCoreで `Game:SetProperty('JAC_ASN_TEST_MODE',1)` を明示し、実行者へ `JAC_ASN_FORCE_NEXT={Success=1/0,Detected=1/0,Branch='FREE_STATE'等}` を設定する。1回で消費し、最後にテストモードも `nil` に戻す。通常UIに強制結果を入力する操作はない。

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
| ゲーム速度 | `Base/Assets/Gameplay/Data/GameSpeeds.xml:19`、`:28`、`:37`、`:46`、`:55`。換算規則の正本はSSOT1.3。暦・元号の開始ターンを生産時計へ混ぜない |
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

PoC画像は公式スカウトのアイコンを参照するだけで、公式DDSを同梱しない。参照は `Base/Assets/UI/Icons/Icons_Units.xml:32`、`:133`、`Icons_UnitPortraits.xml:77`、`Icons_UnitFlags.xml:47`。新Unitの専用3Dモデルは未作成で、表示は要実機確認。借用理由は募集・任務の技術検証を先に行うため。完成版の固有画像の条件はSSOT15.2.1に従う。

## 検査と実機シナリオ

`tests/test_assassin_lua.py` はlupaを必須インポートし、Luaテストをskipしない。実Luaの生産100／20、5速度、補正方向、2都市、取消、都市喪失、生成失敗、完成+9／+10、全国待機、2Unit、6成功率、独立した成否／発覚、Unit死亡、Governor登録簿、論理的な指導者死亡、保存データの復元を検査する。スタブのgetter/setterはテーブルをコピーする。公式GSスキーマ→戦国ツリー→暗殺者SQLのロードも検査する。本体の外交効果をスタブの台帳で「確認済み」としない。

実行するシナリオは `tools/civ6-autotest/scenarios/assassin-leader-save-load.json`。

1. 日本15 km・万世紀・22家、織田家を人間として開始。AI松平家の首都付近へテスト用Unitを付与する。
2. 100ターン募集と指導者任務を同期操作で開始。成功・未発覚・自由国家分岐を強制。別都市／別Unitの開始拒否を検査する。
3. 任務の途中で実際にsave→メニュー→load。ロール・成否・分岐・期限が同一であることを検査する。
4. 任務解決後、論理的死亡・`PENDING_FREE_STATE`・Civ6プレイヤー存続・都市の非移管・コマンド待機を検査する。
5. 100ターン募集の完成を検査し、全国10ターンロック中にもsave→load。完成+9の拒否、+10で別都市の開始許可を検査する。
6. 別都市へ極端な短縮入力を与え、19ターンでは未完成・20ターンで完成・再び全国10ターンロックとなることを検査する。

実機の結果は未取得。敵の自然な戦争・都市喪失が検査に干渉すればシナリオは失敗し、ログとターンを調べる。失敗／発覚の本体外交効果、通常生産キュー統合、Governorのネイティブ効果除去、3D表示、MP一致はこのシナリオで実証しない。

全体検査では、ドライバーテストの `__init__.py` 欠落を修正し、新シナリオを固定一覧へ追加した。また、Python3.12の `tempfile.py:384` が新しい一時フォルダを `mkdir(...,0o700)` で作るため、Windowsの制限されたテストプロセスからそのフォルダへ入れない問題を再現した。通常の `os.mkdir` で作った新フォルダでは書込み・削除が通った。`tools/windows_test_fixtures.py` はunittestの2パッケージからだけ読み、合成テスト用の新規フォルダに親のACLを継承させる。既存のACL、サンドボックス、ゲーム本体、ドライバーの本番挙動は変更しない。

2026-09-30の検査結果：リポジトリのunittest **367件成功**（暗殺者41件：Lua37件と包装4件を含む）、ドライバーのunittest **150件成功**、実機用シナリオの `--check` **36手順が有効**。Luaのskipはない。ドライバーには既存のsocket ResourceWarningが1件出たが、テストの失敗はない。実機実行・pushは行っていない。

別モデルの再レビューでMustなしを確認した。コミットのための `git add` は、共通Gitディレクトリ内の `worktrees/assassin/index.lock` 作成がPermission deniedとなり失敗した。ロックファイルは存在せず、ステージもコミットも行われていない。共有ToDoとDiscord質問も、それぞれ指定ツールの共有ロックファイルを作成・開く権限がなく登録・送信できなかった。SSOTと `questions.md` は変更していない。

## 判断が必要な点

- 「死亡時点の状況」と「実行時に分岐固定」の両立：a 実行時の状況で固定（PoCの推奨）、b 死亡時に保存済み乱数で選び直す、c 実行時に死亡時状況を予測する。bでは「分岐固定」の意味を変更する必要がある。
- 距離区分・Hero/Unitの基準：a PoC値（1／3／6タイル・基準2ターン）を仮値として検証（推奨）、b 数値を指定、c 該当対象を止める。係数・発覚・罰則量も未決値として扱う。
- 自由国家・吸収・継承の実際の処理：a 今回は判断待ち台帳を検証（推奨）、b 1分岐の具体像から決める、c 全分岐を先に決める。ネイティブ指導者の差替と資産移管を独断で追加しない。
- 任務中に暗殺者または実行国が失われた場合：a 成功効果は止め、固定した失敗・発覚の罰則は適用（PoCの推奨。死亡済みの暗殺者が暗殺しないため）、b 成功も予定どおり実行、c 全結果を取消。待機はどの案でも実行時の記録を保持する。
- 「敵国」は戦争中の相手だけか：a 他国なら平時も対象（PoCの推奨。関係補正を検証できるため）、b 戦争中だけ、c 対象国を個別指定する。`AllowNonWarTargets` は暫定でtrue。
- 募集中に外的要因が変わった場合：a 着手時の補正を固定（PoCの推奨。期限を保存できるため）、b 毎ターン計算し直す、c 決定まで補正なし。`ProductionSnapshot` は暫定でSTART。
