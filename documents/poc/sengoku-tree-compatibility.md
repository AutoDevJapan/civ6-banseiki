# 戦国ツリーPoC：Gathering Storm 互換処理の根拠

この文書は `RULESET_JAC_BANSEIKI` でのみ適用する `JAC_PoC_SengokuTrees` の実装メモ。参照元はこのPCの `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/` 以下。2026-09-29 の実機結果は末尾に記録する。効果量はオーナー確認前の暫定値。

| 参照 | ローカル定義の事実 | PoCの処理 |
|---|---|---|
| `Base/Assets/Gameplay/Data/Eras.xml:16-19`、`DLC/Expansion2/Data/Expansion2_Eras.xml:64-74` | 既存の4 Era IDとGSの `Eras_XP2` 行がある | 新しい Era IDを加えず、`ERA_ANCIENT`～`ERA_RENAISSANCE` の表示名だけを戦国の4期に変更。`Eras_XP1`・`Eras_XP2`・後続 Era ID は保持し、黄金／暗黒時代を無効化しない。 |
| `DLC/Expansion2/Data/Expansion1_Eras.xml:3-8`、`DLC/Expansion2/Data/Expansion1_Moments.xml:349-350` | GSの世界時代のターン数と誓約の対象時代が既存 Era IDを参照する | 既存IDを互換スロットとして使い、時代スコアと誓約を残す。戦国専用の時代スコア閾値・誓約・忠誠効果への再設計は未実装で、このPoCはSSOT 3.2の黄金／暗黒時代の仕様に未達。再設計は次段階のTODO。約200ターンの年表示も未実装。 |
| `Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:391-401,630-657`、`DLC/Expansion2/Data/Expansion2_Technologies.xml:16-25,273-286`、`DLC/Expansion2/Data/Expansion2_Civics.xml:57-66,248-256` | GSは公式研究IDと前提を追加し、研究行への外部キー参照もある | 公式技術・制度の行を、使う公式アイテムの再配線後に削除。付随する公式前提・Boost・GS未来研究も研究候補から外す。`Types` 行や Era 行の全削除はしない。新規ゲームだけを対象とする。 |
| `Base/Assets/Gameplay/Data/Eras.xml:177-179`、`Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:2596-2601` | 古代開始時の `StartingCivics` は `CIVIC_CODE_OF_LAWS` を指す | 公式制度を削除する前に、開始制度を戦国のC01へ付け替える。開始制度を欠かせて文化研究を止めない。 |
| `Base/Assets/UI/Screens/TechTree.lua:1387-1404,1425-1436`、`Base/Assets/UI/Screens/CivicsTree.lua:1581-1630`、`DLC/Expansion2/UI/Replacements/CivicsTree_Expansion2.lua:1-12` | 標準画面は利用可能な研究ノードを読み、`UITreeRow` と前提から配置する | 技術は同時代の費用帯を3列にし、32ノードに行を設定。制度も30ノードに行を設定。制度の列は標準UIが前提から再計算するため、設計表の厳密な列再現と、大型ノードによる行移動は未検証・TODO。画面全体の実機確認が必要。 |
| `DLC/Expansion2/Data/Expansion2_Resources.xml:24-28,68-70`、`DLC/Expansion2/Data/Expansion2_GlobalParameters.xml:54,100-104` | 石油の解放は公式 `TECH_REFINING` を参照し、石炭・石油・ウランに電力／CO₂値がある | 資源の研究ゲートを戦国ノードへ付け替え、GSの気候テーブルは残す。発電と海面上昇の長期動作、史実の年に出る未開発鉱山はTODO。終盤資源をT32に結ぶのは早期出現を防ぐPoCの仮置き。 |
| `DLC/Expansion2/Scripts/WorldCongress.lua:254-281`、`DLC/Expansion2/Data/Expansion2_GlobalParameters.xml:290-300` | 世界会議は区域の `PrereqCivic/PrereqTech` から研究とEraを直接引き、会議開始時代は数値2 | 残す区域の研究ゲートを戦国IDに付け替え、`GameInfo.Civics/Technologies[prereq]` が欠落しないようにする。世界会議自体は残す。世界法律、会議の戦国向け開始時期・議題はTODO。外交勝利は下記のとおり無効化する。 |
| `DLC/Expansion2/Data/Expansion2_Ai.xml:123-143`、`Base/Assets/UI/Screens/TechTree.lua:1387` | GS AIの時代戦略は残り、UIの技術候補はゲームコアの利用可能ノードから取る | AI戦略テーブルは保持。公式研究を削除し、戦国ノードに前提なしの初期候補を技術4・制度3用意する。AIが研究を継続するかはゲーム内シナリオで要確認。 |
| `Base/Assets/Configuration/Data/Schema/AdditionalTables.sql:182-204,289-298,332-337`、`Base/Assets/Configuration/Data/SetupParameters.xml:208,255,268`、`DLC/Expansion2/Config/Expansion2_Config.xml:8,50-57` | ルールセットの `VictoryDomain` に属する `Victories` から勝利設定が生成され、GSドメインには制覇・文化・宗教・スコア・科学・外交がある | FrontEnd設定でBanseikiだけを専用の空ドメインに切り替え、公式勝利の設定値を0にする。通常のGSルールセットは変更しない。 |
| `DLC/BlackDeathScenario/Data/BlackDeathScenario_Config.xml:5-9,26-33`、`Base/Assets/Configuration/Data/SetupParameters.xml:39-40,73-74,172,195-197,262-268`、`Base/Assets/UI/FrontEnd/GameSetupLogic.lua:588-608` | 公式シナリオは専用ドメインと `RulesetSupportedValues` で選択肢を絞る。`MaxTurns=0` はターン上限なしの設定更新を生み、勝利項目が空なら設定画面はその欄を隠す | `TURNLIMIT_NONE` のみを選択可能にし、Banseiki本体の `GAME_TURN_LIMIT` の設定更新を `TURNLIMIT_NONE` に上書きする（表に一意キーがなく、行を足すと既存の `TURNLIMIT_GAMESPEED` と競合する）。`GAME_MAX_TURNS=0`、スコア勝利=0、時間敗北=0も設定する。空の勝利ドメインでも設定画面の他の項目を残す構成。画面そのものは要実機確認。 |
| `DLC/AlexanderScenario/Data/AlexanderScenario_Victories.xml:14-18`、`Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:3083-3095` | 公式シナリオはゲームプレイ側の `Victories.EnabledByDefault` も無効化する | 戦国ツリーのゲーム内アクションで公式7種（既定・制覇・科学・文化・宗教・外交・スコア）の既定有効化を外し、`DEFEAT_TIME` も無効化する。全城支配による統一勝利は次段階で実装する。 |

## 現行PoCでの残る制約

- 公式アイテムを代理にした箇所と、設計案のうち未接続の解放・Boostは[全62ノードの照合表](sengoku-tree-unlocks.md)に記録。元アイテムの効果量や名称は戦国専用には調整していない。新規Unit・建物は追加しない。
- `Units.CanTrain=0`、`Buildings/Districts.InternalOnly=1`、`Improvements.Buildable=0` で公式の近現代候補の早期出現を抑える。既設の建物・改善を削除する処理ではない。ただし他MODと同時適用した場合の専用アイテムや、偉人・蛮族・自由都市の無償生成は未検証。
- 非戦国の公式 `Types` IDは、GS側のメタデータやアイコンが参照できるように残す。研究UIと研究候補を決める `Technologies`・`Civics` 行には残さない。削除に伴う関連行の連鎖削除はローカルスキーマに従う。同盟などの外交行動・総督称号・国境の管理など実害のある行は削除前に戦国ノードへ付け替える（[連鎖削除の監査](sengoku-tree-unlocks.md#公式ツリー削除に伴う連鎖削除の監査)）。50ターンの進行は確認済みだが、GS全機能の長期動作は未確認。
- 1534年固定マップ・22勢力・既設城、資源の年別出現、史実イベント、家別の初期技術は今後のシナリオ側実装。汎用マップで始める自動テストのため、開拓者はこのPoC中は残す。
- 2026-09-29 の提示された実機結果：`RULESET_JAC_BANSEIKI` と日本マップでロード成功。戦国技術32件・社会制度30件のみで、公式技術・制度行は0件。Gathering Stormの仕組みが存在し、50ターンの自動進行とセーブ／ロードが成功した。この結果は今回の公式勝利無効化を加える前のもの。
- 2026-09-29 の自動実機結果（勝利無効化後）：設定を作成画面と同じく組み立てて開始すると、公式勝利7種が無効で、終了ターン（`Game.GetGameEndTurn()`）が0になり、画面上部のターン表示から上限が消えた。戦国ツリーPoCを外した対照では公式勝利が有効で、終了ターンは501。
- **要実機確認**：人が作成画面から始めたときのターン制限欄の表示、500ターンを超える長期進行。ツリー画面の前提線・研究選択・Boostの操作も人が確認する。
