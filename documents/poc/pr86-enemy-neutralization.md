# PR86 敵側人物総督への本体スパイ中和診断

**実機の中和成功・回復は未観測。** この診断は、人間0が正式生産した本体スパイで、AI1が正式加入させた蘆名盛舜の本体Governorを攻撃する。元の織田信秀を人間が育てる都市喪失・スパイ派遣診断とは別ゲームで実行する。仕様は[4.7](../spec/ssot/04-factions-characters.md)、[9.4](../spec/ssot/09-assimilation-religion.md)、[16.1](../spec/ssot/16-roadmap.md)のまま。費用2/3、全113手順、未加入AIの通常資格は合格としない。

## 実体と権限

| 役割 | 本体player | 固定した意味 |
|---|---:|---|
| controller / spy attacker | 人間0（北条） | 公式`UNIT_SPY`を首都の`BUILD`で生産し、公式UIから派遣・任務を一度ずつ要求する |
| defender | AI1（伊達） | 本体Governorと中和される都市の所有者。人間のスパイとは別勢力 |
| person source | AI2（蘆名） | 費用1の蘆名盛舜を、製品の`Join(1,2)`でAI1へ正式加入させる |

テスト専用の別MOD `poc/JAC_Test_EnemyGovernorGate`（UUID `bba3a0d2-8f08-4715-9263-00a773e6ae86`）だけが、15kmマップの診断時にAI1の本体候補を蘆名盛舜へ絞る。この静的Traitは正式Joinより前にも存在するため、**未加入人物の製品資格を修復した根拠ではない**。診断はJoin前に本体任命がないことを検査し、Join後の同じ同期Gameplay処理で公式Civicから称号1個を付与して、`PrepareEvolution`を通す。称号・使用済み数・本体候補・偉人・所有者を読み直す。製品の22人SQLは変更しない。

次の新ゲームがこのMODを明示的に選ばなければ、共通のMOD選択処理が無効に戻す。従来の人間人物・都市喪失診断へ敵用の静的資格を持ち込まない。

AI1への`APPOINT_GOVERNOR`と`ASSIGN_GOVERNOR`は人間0の可視UIから、それぞれ可能なとき一度だけ送る。公式UIの通常例は自分のplayer宛てであり、別playerへの権限は未実証。戻り値を成功とせず、同期Gameplay barrier後の本体型・基礎昇進・使用済み称号・配置都市を判定する。拒否ならAI1の本体AIによる同じ本人の任命・配置を実ターンで待つ。どちらの経路も、本体実体と称号1消費を確認した後にだけ`CompleteEvolution`で元のGreat Personを除く。別の未加入人物が任命されても合格にしない。

実機54は準備の7/26手順で停止した。`Gameplay`の`HasGovernor`呼出しが`function expected instead of nil`となり、敵側設定は保存されなかった。`CanEverAppointGovernor`も同じUI専用領域だった。改訂版はUIで加入前の候補・任命有無を観測し、ターン・UIフレーム・同期番号を付けた要求でGameplayの正式Joinと称号付与を行う。同期barrier後にUIの称号キャッシュが更新されたことを読み、別の同期要求で`PrepareEvolution`を行う。確認・回復も製品の既存UI読取橋で本体Governorを再読取りし、Gameplayでは実証済みの称号数・都市・人物台帳だけを使う。この改訂版の実機結果は未観測。

## 実機で実行する順序

全ファイルは`tools/civ6-autotest/scenarios/leader-persona-spy-neutralization-*.json`。`start-15km`だけが新ゲームを構成する。残りは前段のセーブから開始し、末尾でメニューへ戻る。各`*-tick`は**本体の実ターンを1つだけ**進め、同じ名前で保存する。親担当が`live_chain`の持ち時間を見て、次の段階のファイルを選ぶ。任務・中和・回復のターンは省略しない。

| 順番 | ファイルの末尾 | 次へ進む観測 |
|---|---|---|
| 1 | `start-15km` | AI1の同一人物が`GOVERNOR`になり、`SETTLED_INITIAL`まで`native-tick`を繰り返す。開始時の不受理は`WAITING_NATIVE`として保存する |
| 2 | `native-tick` | AIによる本体任命・配置、称号使用1、5ターンの定着を待つ。任命・配置ができなければ未確定として止める |
| 3 | `travel` | 人間0の本体スパイをAI1の本人Governorがいる都市へ一度だけ派遣。単独roster、階級、目的地、将来の`ReturnTurn`を確認し、観測した移動参照を保存 |
| 4 | `travel-tick` | offMapが消え、唯一の本体スパイが目的都市に現れる。IDは同じでも新しくても照合する。公式`CanStartOperation`が真になるまで必要な実ターンだけ進める |
| 5 | `mission` | 対象都市の本体Governor型を本人と照合。公式の可否、確率、期間を記録し、`UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR`を一度だけ要求。同期barrier、`UnitOperationStarted`、本体operationと将来の終了ターンを照合 |
| 6 | `mission-tick` | 任務の実終了まで1ターンずつ進める。攻撃側0の完了イベントからIDを保存し、そのIDの本体履歴でOperation・Plot・CompletionTurn・InitialResult・EscapeResultを読む。初回成功なら逃亡時の捕獲・死亡でも成功候補とする。失敗はセーブして失敗と報告する |
| 7 | `recovery` | **成功履歴と本人の`GetNeutralizedTurns()>0`がそろった場合のみ**開始。Gameplay側の功績・本体残存期間を保存し、同ターンのセーブ・ロード後にUIの全観測を比較する |
| 8 | `recovery-tick` | 残存期間中は本体Modifier対象数0、都市定着0、功績の新規獲得0をUIとGameplayで照合。期間0では本人が未配置のままか検査する |
| 9 | `reassign` | 回復した未配置の本人へ公式再配置要求を一度送る。拒否なら本体AIの再配置を実ターンで待つ |
| 10 | `reassign-tick` | 本体`GetTurnsOnSite`が0〜4の間は効果・功績が停止し、5で再定着・効果が戻るまで1ターンずつ保存する |

任務開始前の`SETTLED_INITIAL`は本体効果の実対象と所有者、都市の所有者・ID・座標、定着Property、UI/Gameplayの功績一致を要求する。活動不能と回復の判定はGameCoreの人物処理のまま。本体の中和期間、成功率、任務期間、公式の称号・功績・技能、偉人やGovernor実体を診断から偽造しない。派遣の`ReturnTurn`と任務の`GetSpyOperationEndTurn`は別の時刻として記録する。逃亡選択が画面に出た場合は親担当が本体画面で処理し、履歴と状態の照合後に進む。

活動不能中にAIが既存の功績で新技能を買った場合、残高の減少を獲得停止の違反と誤認しない。GameCoreで保存した技能一覧と各公式定義の費用を照合し、`現在の功績残高＋中和後に購入した技能の費用＝中和時の残高`を要求する。取得済み技能と公式の使用済み称号も保持される必要がある。

## 検査と残る実機判定

Lua 5.1を実行するRust検査は、actor誤指定、未任命・非正規人物、別player要求の拒否、別都市・別ID・失敗の履歴、捕獲を伴う初回成功、活動不能期間、未配置回復、0〜4/5ターン、画面読み込み直後の再照合を扱う。Gameplay用モックからUI専用Governor getterを除き、UIの候補確認から同期Join・称号・進化・本体確認まで実行する。JSONの全段階はRustで構文と操作順を検査する。2026-10-03の修正版では、Rustワークスペース全102対象・1,478件が成功し、失敗・ignoredは0件。10シナリオの`--check`、`cargo fmt --check`、差分検査も成功した。これらはCiv6の実測結果ではない。

2026-10-03時点の既存実機では、52手順・6件のログ判定で人間0の正式スパイの生産と敵都市への派遣まで成功した。ゲームターン省略0、LuaError 0。今回のAI1の任命権限、AIの代替任命、本人中和・回復は未実機。初めて拒否または任務失敗が出た結果は、失敗のまま保存して親ToDo853へ返す。
