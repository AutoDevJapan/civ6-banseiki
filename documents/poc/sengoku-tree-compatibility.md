# 戦国ツリーPoC：Gathering Storm 互換処理の根拠

この文書は `RULESET_JAC_BANSEIKI` でのみ適用する `JAC_PoC_SengokuTrees` の実装メモ。参照元はこのPCの `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/` 以下。**ローカル定義の調査結果であり、ゲーム内ロード成功の記録ではない。** 効果量はオーナー確認前の暫定値。

| 参照 | ローカル定義の事実 | PoCの処理 |
|---|---|---|
| `Base/Assets/Gameplay/Data/Eras.xml:16-19`、`DLC/Expansion2/Data/Expansion2_Eras.xml:64-74` | 既存の4 Era IDとGSの `Eras_XP2` 行がある | 新しい Era IDを加えず、`ERA_ANCIENT`～`ERA_RENAISSANCE` の表示名だけを戦国の4期に変更。`Eras_XP1`・`Eras_XP2`・後続 Era ID は保持し、黄金／暗黒時代を無効化しない。 |
| `DLC/Expansion2/Data/Expansion1_Eras.xml:3-8`、`DLC/Expansion2/Data/Expansion1_Moments.xml:349-350` | GSの世界時代のターン数と誓約の対象時代が既存 Era IDを参照する | 既存IDを互換スロットとして使い、時代スコアと誓約を残す。戦国専用の時代スコア閾値・誓約・忠誠効果、約200ターンの年表示は TODO。現行の値を戦国向け確定効果とは呼ばない。 |
| `Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:391-401,630-657`、`DLC/Expansion2/Data/Expansion2_Technologies.xml:16-25,273-286`、`DLC/Expansion2/Data/Expansion2_Civics.xml:57-66,248-256` | GSは公式研究IDと前提を追加し、研究行への外部キー参照もある | 公式技術・制度の行を、使う公式アイテムの再配線後に削除。付随する公式前提・Boost・GS未来研究も研究候補から外す。`Types` 行や Era 行の全削除はしない。新規ゲームだけを対象とする。 |
| `Base/Assets/Gameplay/Data/Eras.xml:177-179`、`Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:2596-2601` | 古代開始時の `StartingCivics` は `CIVIC_CODE_OF_LAWS` を指す | 公式制度を削除する前に、開始制度を戦国のC01へ付け替える。開始制度を欠かせて文化研究を止めない。 |
| `Base/Assets/UI/Screens/TechTree.lua:1387-1404,1425-1436`、`Base/Assets/UI/Screens/CivicsTree.lua:1581-1630`、`DLC/Expansion2/UI/Replacements/CivicsTree_Expansion2.lua:1-12` | 標準画面は利用可能な研究ノードを読み、`UITreeRow` と前提から配置する | 技術は同時代の費用帯を3列にし、32ノードに行を設定。制度も30ノードに行を設定。制度の列は標準UIが前提から再計算するため、設計表の厳密な列再現と、大型ノードによる行移動は未検証・TODO。画面全体の実機確認が必要。 |
| `DLC/Expansion2/Data/Expansion2_Resources.xml:24-28,68-70`、`DLC/Expansion2/Data/Expansion2_GlobalParameters.xml:54,100-104` | 石油の解放は公式 `TECH_REFINING` を参照し、石炭・石油・ウランに電力／CO₂値がある | 資源の研究ゲートを戦国ノードへ付け替え、GSの気候テーブルは残す。発電と海面上昇の長期動作、史実の年に出る未開発鉱山はTODO。終盤資源をT32に結ぶのは早期出現を防ぐPoCの仮置き。 |
| `DLC/Expansion2/Scripts/WorldCongress.lua:254-281`、`DLC/Expansion2/Data/Expansion2_GlobalParameters.xml:290-300` | 世界会議は区域の `PrereqCivic/PrereqTech` から研究とEraを直接引き、会議開始時代は数値2 | 残す区域の研究ゲートを戦国IDに付け替え、`GameInfo.Civics/Technologies[prereq]` が欠落しないようにする。世界会議自体は残す。世界法律、会議の戦国向け開始時期・議題、外交勝利の扱いはTODO。 |
| `DLC/Expansion2/Data/Expansion2_Ai.xml:123-143`、`Base/Assets/UI/Screens/TechTree.lua:1387` | GS AIの時代戦略は残り、UIの技術候補はゲームコアの利用可能ノードから取る | AI戦略テーブルは保持。公式研究を削除し、戦国ノードに前提なしの初期候補を技術4・制度3用意する。AIが研究を継続するかはゲーム内シナリオで要確認。 |
| `Base/Assets/Configuration/Data/SetupParameters.xml:73,172-195`、`DLC/Expansion2/Config/Expansion2_Config.xml:49-57` | `TURNLIMIT_NONE` ならスコア勝利パラメータを無効にする設定がある | Banseiki ルールセットのターン上限を `TURNLIMIT_NONE` に変更。全城の統一勝利とほかの公式勝利の整理はこのツリーPoCの対象外。 |

## 現行PoCでの残る制約

- 公式アイテムを代理にした箇所と、設計案のうち未接続の解放・Boostは[全62ノードの照合表](sengoku-tree-unlocks.md)に記録。元アイテムの効果量や名称は戦国専用には調整していない。新規Unit・建物は追加しない。
- `Units.CanTrain=0`、`Buildings/Districts.InternalOnly=1`、`Improvements.Buildable=0` で公式の近現代候補の早期出現を抑える。既設の建物・改善を削除する処理ではない。ただし他MODと同時適用した場合の専用アイテムや、偉人・蛮族・自由都市の無償生成は未検証。
- 非戦国の公式 `Types` IDは、GS側のメタデータやアイコンが参照できるように残す。研究UIと研究候補を決める `Technologies`・`Civics` 行には残さない。削除に伴う関連行の連鎖削除はローカルスキーマに従うが、GS全機能の長期プレイはまだ確認できていない。
- 1534年固定マップ・22勢力・既設城、資源の年別出現、史実イベント、家別の初期技術と「太閤検地」の表示分岐は今後のシナリオ側実装。汎用マップで始める自動テストのため、開拓者はこのPoC中は残す。
- Civ6/Steamは本作業では起動していない。DBの適用順、GS中核、UIの実画面、研究・制度進捗は[自動シナリオ](../../tools/civ6-autotest/scenarios/sengoku-trees.json)の実行待ち。
