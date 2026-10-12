# PR86の本体中和・回復・都市喪失を観測する診断

**都市喪失の独立診断は実機leaderperson-41で全55手順に合格し、正式スパイの生産・派遣は実機52の全52手順とログ判定6件に合格した。** 中和と回復は未観測で、加入前のAI任命のMustも未解決である。次回の敵側中和は[別playerの診断](pr86-enemy-neutralization.md)に記す。

起点はPR86の81ccf52、作業場所は`fix/pr86-boundary-diagnostic`である。仕様はSSOT [9.4](../spec/ssot/09-assimilation-religion.md)、[4.7](../spec/ssot/04-factions-characters.md)、[16.1](../spec/ssot/16-roadmap.md)に従う。16-roadmapの26行で標準不能時の同等代替が承認されている。PoCの実証は単独プレイで行い、MPの非決定性コードはMust、複数端末での実証だけが未実施ならToDo9の後続とする。

この担当ではAPIキー、Discordトークン、稼働中のCiv6、他のjobを使っていない。実機操作・新job起動・push・PR投稿・mergeを行っていない。既存の113手順と早期・末尾のAI断言は変更していない。費用1/2/3・6回保存・転任・同盟8点の再調査も行っていない。

## 追加した診断

`poc/JAC_Test_PersonaBoundary`は有効にしたときだけ読み込まれるテスト専用MODである。本番MODに常時動く診断や状態変更を追加しない。UUIDは`4a72b1c0-958b-4bd5-81e4-a094f86ce643`。インストールと有効化はこの担当では実行していない。

| シナリオ | 確かめること | 実機の状態 |
|---|---|---|
| `leader-persona-city-loss-15km.json`（54手順） | 織田信秀を正規加入・費用1で進化、非首都へ配置、5ターン定着、実ターンの功績で固有技能を取得。本体で自由都市へ移し、旧都市と残った都市の効果・功績停止、保存読込の一致を検査する | leaderperson-41で起動を含む55手順成功、ログ判定6件成功。実ターンを省略していない |
| `leader-persona-spy-access-15km.json`（実機52時点で52手順） | C12の正式制度と本体スパイ枠を満たし、首都で`UNIT_SPY`を指定生産する。1実ターン後の本体ユニット1個増を確かめ、公式UIから一度派遣する。同期barrier後に地図上とoffMapの両一覧を照合する | 実機52で全52手順・6ログ判定成功。移動の受理まで確認し、中和は未観測 |
| `leader-persona-spy-recovery.json`（旧継続案） | 自国の織田信秀が本体中和された後に継続する構成 | 敵スパイを公式に操作する経路が未確定のため、今回の敵側診断には使わない |
| `leader-persona-spy-neutralization-*.json`（10段階） | 人間0の正式スパイでAI1の正式加入人物を攻撃し、実履歴・中和・回復・5ターン再配置を保存再開で確認する | Lua/Rustで診断コードを検査済み。実機の任命権限・中和・回復は未判定 |

独立fixtureは既存の日本15kmマップと22勢力を使う。対象都市は人間の勢力の非首都、比較対象は首都と他の自都市である。人物所有者・偉人生成・称号獲得・進化・技能取得は既存の正式処理を通す。功績、人物状態、活動不能期間、都市の所有者をPropertyへ書いて結果を作らない。診断用の設定、生産準備・進捗・barrierの印、本体から確認したスパイIDだけを同期Gameplay操作から保存し、印そのものを成功の証拠にはしない。

加入・進化・技能取得の要求、総督配置、スパイ派遣と任務開始は、テストMODの`ContextPtr:SetUpdate`から一度だけ送る。スパイ要求は本体の選択ユニットIDと諜報モードを照合してから送り、別の`EXECUTE_SCRIPT`操作によるGameplay barrierと本体キャッシュの公開を待つ。barrier前は応答不明とし、処理後に本体一覧が不変なら不受理とする。要求が戻っただけでは受理としない。FireTunerは待ち行列へ要求を置く入口に限る。結果不明・拒否・例外の操作を自動で再送しない。人間以外のスパイの要求は明示的に拒否する。

## 同時に記録する一次観測

`JAC_BOUNDARY`で始まるLuaログと、ドライバーの`ui_lua`結果に以下を同時に残す。成功の前後で記録し、任務失敗も履歴から除外しない。

- 人物の所有者・状態・功績・取得技能、同じ人物の本体偉人が残っていないこと、実称号の獲得数と使用済み数。
- **MODの読取橋を使わず**取得した本体Governorの型・Hash・基礎昇進・配置都市・定着状態・活動不能の残存期間。
- 元の都市中心と全自都市について、所有者・都市ID・定着Property・取得技能のPlot選択フラグ・各本体Modifierの有効な都市対象数・全種類の収量。Gameplay側でも都市のPropertyと功績を読み、UIの値と比較する。
- スパイの本体ID・所在・実行中の任務・終了ターン・残り期間。派遣問い合わせ、任務問い合わせ、失敗理由、実際の所要時間、成功と逃亡を含む確率表。
- 完了通知の**攻撃側**プレイヤーIDと任務ID。本体の履歴から任務種類・初回結果・逃亡結果・完了ターン・対象Plotを読み直す。初回成功後に死亡・捕獲されても、中和が起きた可能性を捨てない。

取得技能のPlotフラグは購入済み技能の候補を示すもので、実際の発効を示す値ではない。旧都市にも残る場合があるため、値を記録したうえで、定着Propertyと本体Modifierの有効対象が0になることを停止の判定に使う。人物の取得済み技能は失わない。収量は所有権移管などで他の補正も変わるため、差額だけで人物効果を認定しない。有効対象数と一緒に記録する。

同名都市を同じ所有者が持ち、本体の効果対象を一意に照合できない場合は失敗する。効果の本体所有者を独立して読み、他プレイヤーの同じ総督型の効果と区別する。移管前の都市名や所有者を持つ有効対象が残り、現在の都市へ照合できなければ、その対象もログへ残して失敗する。本体の効果一覧・所有者・有効状態・対象の読み取りが欠けた場合も、0効果として扱わない。都市が消滅した場合は移管の成功とは別に失敗を記録する。

別モデルレビュー後、効果の定義またはIDが読めない場合も失敗させた。都市の同一性は準備時の所有者・ID・中心座標を移管要求前に照合し、同じ同期処理で本体移管後の所有者・IDを読んで診断設定へ保存する。後の照合はこの結果IDと元の中心座標へ一致する都市だけを対象にする。所有者が変わってもIDが不変だとは仮定しない。別都市への置換、消滅、移管結果の欠落と不一致を成功にしない。人物状態・都市所有者をPropertyへ書いて結果を作る処理はない。

同じ領域の再レビューで、UIだけが古い都市を保持してGameplayが別の都市を見ている場合の照合不足も確認した。都市ごとにID・所有者・定着Propertyを両文脈で照合し、対象は移管結果にも一致させる。どちらかの識別情報の欠落・不一致は失敗する。実コードLua23件が成功したが、実機の更新順序は未観測である。

## 公式インストールと既存一次資料の根拠

以下の`G/`は`C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/`である。指定された調査報告[D:/CC/Tools/codex-jobs/jobs/20261002-003855-pr86-virtual-spy/last_message.md](D:/CC/Tools/codex-jobs/jobs/20261002-003855-pr86-virtual-spy/last_message.md)を先に読み、ここで同じ資格調査をやり直していない。

| 操作・観測 | 一次根拠と適用範囲 |
|---|---|
| 本体による都市喪失 | `G/Debug/City.ltp:275–278`は選択した都市を`CityManager.TransferCityToFreeCities(pCity)`へ渡す。診断はこの本体移管を同期Gameplayから呼び、所有者が`PlayerManager.GetFreeCitiesPlayerID()`に変わったことを別に読む。既存暗殺者のGameplay使用例もあるが、この人物総督の喪失を成功と推定する根拠にはしない |
| スパイ生成 | 旧fixtureの汎用`InitUnitValidAdjacentHex`は実機44で型`UNIT_SPY`のunitを作ったが、諜報派遣は証明できなかった。新fixtureは`JAC_Sengoku_Unlocks.sql:18`のC12を本体の`SetCulturalProgress`で完了し、`JAC_Sengoku_Cascade.sql:32`が移した公式`CIVIC_GRANT_SPY`による枠を読み直す。`G/Base/Assets/UI/Panels/ProductionPanel.lua:293–301,2885–2898`の`CityOperationTypes.BUILD`で首都の0番へ`UNIT_SPY`を指定し、同期Gameplayから`G/Debug/City.ltp:200–204`の`AddProgress(500)`を一度だけ使う。完成前後の本体ユニット一覧で増分1個を同定する。`SET_PRODUCTION`という操作名は公式UIに見つからず、同UIが使う`BUILD`を採用した |
| スパイ派遣 | `G/Base/Assets/UI/Panels/UnitPanel.lua:4091–4096`は`UI.SelectUnit`で本体ユニットを選択する。`G/Base/Assets/UI/Choosers/EspionageChooser.lua:630–643,738–765`は自国都市にいる選択中スパイの派遣モードを選ぶ。同605–618行はX/Y付きで一度だけ派遣要求を送る。`G/Base/Assets/UI/PartialScreens/EspionageOverview.lua:72–145,733–765`は地図上のunitと移動中のoffMapを別一覧として扱い、後者に名前・階級・目的地座標・ReturnTurnを持つ。fixtureは派遣前に地図上1人とoffMap 0件を確認し、barrier後にoffMapの唯一の増分、目的都市、将来のReturnTurnを照合する。即時到着は目的都市の本体unitを独立照合し、終了ターン0でも受理する。offMapには一次資料で確かめたunit IDがないため、同名だけで判定しない |
| 中和任務の可否と開始 | 同Chooserの234行で`UnitManager.CanStartOperation(spy, operation.Hash, cityPlot, false, true)`。`G/Base/Assets/UI/Popups/EspionagePopup.lua:474`で`UnitManager.RequestOperation(spy, operation.Hash)`。fixtureも通常のUI frameからこの要求を送る |
| 期間と確率 | `G/DLC/Expansion2/Data/Expansion1_UnitOperations.xml:12`は中和任務8ターン。`G/Base/Assets/UI/EspionageSupport.lua:27–42`は実所要時間と確率を本体から読む。fixtureは確率・任務期間を書き換えない |
| 任務履歴 | `G/Base/Assets/UI/Popups/EspionagePopup.lua:376–383,624`の完了イベントと履歴取得。`G/Base/Assets/UI/PartialScreens/EspionageOverview.lua:279`の`CompletionTurn`。敵側の履歴が読めない場合は「取得不能」を記録し、中和の照合に成功した扱いにしない |
| 回復 | `G/DLC/Expansion2/UI/Additions/GovernorSupport.lua:74–78`で`GetNeutralizedTurns()`、`GovernorDetailsPanel.lua:89`で残存0の再配置条件。回復後の元都市への自動復帰を期待しない |
| 収量 | `G/Base/Assets/UI/CitySupport.lua:316–331`で都市の`GetYield`を使用。fixtureは種類ごとに未丸めの値を記録する |
| 効果の所有者 | `G/Debug/Diplomacy.ltp:397–398`は`GameEffects.GetModifierOwner`と`GetObjectsPlayerId`で効果の所有者を判別する。人物側の所有者が持つ効果を対象とし、移管後の古い都市対象を照合から捨てない |

純Gameplay人物は本体の中和対象ではない。指定報告のDLL読解では、中和は都市に対応する本体総督を検索する。`StartOperationWithCityTarget`は蛮族Managerであり、スパイ開始に使わない。人物→本体Governorの実体照合はテストMODのUI側`JAC_PersonaBoundary_View.lua`に限定した。Gameplayでは本体Governor用のUI getterを呼ばず、製品の既存UI読取橋と公式称号・都市・人物台帳を照合する。

## 親で確定が必要な操作

**自国の織田信秀Governorを攻撃する敵スパイを、公式に根拠のある方法で操作する経路は未確定である。** この方向で自己攻撃を作らず、人間0の本体スパイでAI1の正式加入人物Governorを攻撃する診断を追加した。AI1への本体任命・配置要求は独立に判定し、拒否されたら本体AIの同一人物任命を待つ。公式`EspionagePopup.lua:351–358,440–447`は自分のプレイヤーのユニット一覧からスパイを取得する。Gameplayの公式シナリオ内には、スパイの`RequestOperation`の呼出し例を確認できなかった。

ここを架空のAPI、非人間への未検証の要求、任命・中和Propertyの書換えで埋めていない。親は、敵側に正式加入・本体Governorを準備して人間のスパイで攻撃する接続、または同期Gameplayから敵スパイを動かせる一次根拠を確定する必要がある。自然な敵AIの中和を観測できた場合も、攻撃側の履歴と対象の実状態を照合してから継続診断へ入る。敵履歴の読取可否、イベント配送順、逃亡選択が要る場合の実操作も未実機である。

中和任務が失敗したら失敗履歴と本体の残存値を残す。回復の診断は開始できず、中和成功として合格にしない。保存・読込の中和期間一致も、この前提を満たした実行で初めて検証済みになる。

## 検査と進捗

現在の検査と実機結果は[担当の進捗](pr86-boundary-progress.md)へ記録した。98889d3の指定Rustワークスペース全96対象・1,405件が成功し、失敗・ignored・filtered outは0件、Luaを全件実行した。変更10シナリオの形式とXML・MOD構成も成功した。Lua検査は独立した本体入力を使い、実総督の欠落、任務の失敗・別都市・古い履歴、捕獲を伴う成功、旧都市の残留効果、功績変動、自動復帰、読込での再読取り、通常frameからの一回の要求を検査する。これはMODの診断コードの回帰検査であり、Civ6の成功の代わりではない。

ToDo324/407の未観測項目は親が追跡する。この担当からオーナーへの仕様質問や実機依頼は送らない。
