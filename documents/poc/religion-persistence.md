# PoC 束2：宗教持続

仮宗教の信徒や教典を保存し、地下化・滅亡・再興の状態を復元する仕組みを実装した。

正本は [SSOT 9.2・9.3](../spec/ssot/09-assimilation-religion.md) と [16.1「宗教持続」](../spec/ssot/16-roadmap.md)。この文書は保存方式と検証手順を記録する。D8のオリジナル宗教9種は決めていない。

## 実装範囲

`poc/JAC_PoC_ReligionPersistence/` は Gathering Storm と「万世紀」で読み込める独立MOD。IDは `9b0b9a39-bd72-4ff1-9062-05eeac09b693`。番号だけの仮宗教20枠を独自カタログに置く。宗教の内容、宗派、教義、マップ別の抽象化は定義しない。LuaはカタログのIDを受け取り、宗教名や20という個数を条件式に使わない。

都市、離散した集団などを表すコミュニティIDごとの信徒数・公的施設数・保有者を保存し、その世界合計から状態を求める。発祥プレイヤーのIDと滅亡通知も保存するが、滅亡通知で信徒や宗教を消さない。

| 条件 | 保存する状態 |
|---|---|
| 世界合計の信徒が1以上、公的施設が1以上 | `PUBLIC` |
| 信徒が1以上、公的施設が0 | `UNDERGROUND` |
| 信徒が0（施設や資産の有無を問わず） | `LOST`、信徒・資産・来歴をそのまま保持 |
| `LOST` に教典・遺物・記録・人物が残る | `canRevive=true`。自動で信徒を増やさない |
| 残存資料を指定し、正の信徒数を明示して再興 | 再興履歴を追加し、施設数に応じて公開／地下に復帰 |

`lost` と `underground` は別の保存欄。施設0で信徒または資産が残ると地下持続の根拠を `underground=true` として保持する。信徒0なら表示用 `state` は9.2の明示条件に従い `LOST`。教典だけが残る場合の「地下化」と「失われた宗教」の排他性は確定せず、両方の事実を残す。

資産は教典・遺物・記録・人物・聖地の5種。ID、種別、保有者を保持する。亡命者は人物の保有者IDで表せる（検証では `EXILE_1`）。聖地だけでは9.2の再興根拠4種に含めない。移管しても資産IDや種別は変えない。

9.3の発見は、傾奇者・旅人・学者・芸人・探検家・盗賊・僧侶と、遺跡・廃城・洞窟・沈没船・宇宙遺跡をデータ化した。明示した発見入力から教典／遺物を取得し、発見者、ジョブ、場所の型、発見ターン、選択結果を保存する。

再興、売却、秘匿、研究、異端化、他宗教への吸収、Artifact獲得、外交事件、英雄加入の9分岐を記録できる。再興だけは信徒・履歴を実際に変更し `effects=APPLIED`。他8分岐は `effects=DEFERRED` として選択だけを保存し、売却価格、研究量、人物加入や外交効果を作らない。再興人数・施設数は呼出側の明示入力。PoCでは資料を消費せず保存比較に残すが、本番の消費規則を決めたものではない。

## 保存と呼出し

`Game:GetProperty/SetProperty` の単一キー `JAC_RP_STATE_V1` に以下を保存する。

- `schema=1`、変更回数 `revision`、宗教IDをキーとする `religions`。
- 各宗教の発祥者、創始ターン、発祥勢力の滅亡、コミュニティ、世界合計、状態フラグ。
- 資産、未解決／解決済み発見、選択分岐と効果の適用状況。
- 再興回数と、資料・新しい集団・信徒数・施設数・保有者・ターンを含む再興履歴。

読取も変更も毎回プロパティから始める。変更はコピー上で検証してから一度だけ保存する。入力不正、重複創始、重複発見／解決／再興、資料なし、信徒0での再興は保存前にエラー。未知の保存スキーマを初期状態で上書きしない。初回読込・ロード・ターン開始で台帳を作り直す処理はない。空文字列は保存しない。未設定欄や資産削除には `nil` を使う。

Gameplay APIは `ExposedMembers.JAC_ReligionPersistence`。FireTunerの `GameCore_Tuner` から次のように検証する。

```lua
local R = ExposedMembers.JAC_ReligionPersistence
local id = "JAC_RP_PLACEHOLDER_01"
R.Found(id, 0, "origin", 3, 1, "PLAYER_0")
R.PreserveAsset(id, "book", "SCRIPTURE", "CAVE")
R.SetCommunity(id, "origin", 1, 0, "EXILE_1")
R.NotifyFounderDefeated(0) -- 滅亡入力。プレイヤーを実際に消すAPIではない
R.SetCommunity(id, "origin", 0, 0, "EXILE_1")
R.Discover(id, "discovery_1", "book", "SCRIPTURE", "TRAVELER_7", "TRAVELER", "CAVE")
R.ResolveDiscovery(id, "discovery_1", "REVIVE", "revival_1", "new", 2, 0, "PLAYER_1")
return R.Snapshot()
```

`SetCommunity` は人数の差分ではなく集団の現在値を受け取る。`MoveAsset` は保有者だけを変更し、`RemoveAsset` は明示的な喪失入力。`Revive` は教典・遺物以外の記録／人物からの直接再興にも使える。`Inspect` / `Export` はコピーを返し、`Snapshot` はキー順を固定した台帳全体の文字列を返す。

標準宗教の市民圧力、建物の自動走査、人口や人物・Vaultとの同期、自然な布教／迫害、マップ上の探索トリガーとは未接続。世界合計には登録したコミュニティだけが入る。後続の各システムが集計・移管・滅亡・探索の事実をGameplay側へ渡すための保存PoCであり、通常プレイでの宗教シミュレーション完成とは扱わない。UIから台帳変更関数を直接呼ばず、UI操作を付ける場合はGameplayへの同期された要求を使う。

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

## 検証

```powershell
python -m unittest discover -s tests -t .
python -m unittest discover -s tools/civ6-autotest/tests -t .
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/religion-persistence.json --check
```

追加のLuaテストは `tests/test_religion_persistence.py`。lupaを必須importし、スキップしない。保存のスタブでは台帳をJSONでゲーム外へ出し、新しいLua VMへ戻す。20宗教の分離、信徒1→0、発祥者滅亡、4種の再興資料、70通りのジョブ・場所・資料、9分岐、二重処理、入力不正、書込失敗、未知スキーマ、セーブ切替と、実機シナリオのGameplay手順を検査する。これはCiv6での成功報告の代用ではない。

実機はメインセッションで、MODを配置して次を実行する（言語表示は日本語・英語・ロシア語それぞれで同じシナリオを実行）。

```powershell
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/religion-persistence.json
```

「万世紀」・織田／武田・小地図・標準速度で開始する。シナリオはMOD、万世紀ルールセット、カスタム勢力を有効にする。都市を建設し、公式の `RELIGION_CUSTOM_1` を実際に創始する。公式のCathedral信仰1件は創始検証用の既存エンジン部品で、D8の宗教／教義案には数えない。台帳の仮宗教は標準宗教のIDや信徒数に依存しない。

| 保存・再読込する段階 | 主なassertion |
|---|---|
| 公開 | 創始、信徒6、公的施設1、教典・遺物・記録・亡命者・聖地、別宗教の分離 |
| 地下化 | 発祥者滅亡通知後も他勢力の信徒1が存続。相手ターンを含む2ターン進行でも不変 |
| 失われた宗教 | 最後の信徒が0。資料5件を保持し、再興可能だが自動再興しない |
| 発見 | 旅人の教典発見が未解決で残る。別の遺物発見の売却選択は効果未決として保存 |
| 再興 | 指定資料から信徒2で地下に復帰。資料、発見結果、再興回数・履歴を保持 |
| 公開へ復帰 | 公的施設1で公開に復帰。再興履歴を保持 |

46ステップ。最初の都市建設要求は人間のターンが有効になるまで待つ。6回のsave→メニュー→loadで、ロード直後の台帳全体を外部の `capture_as` と `equals_capture` で完全比較する。相手ターン後の比較を含め計7回。期待値をGameプロパティに保存しないので、台帳と期待値の同時消失による偽成功を避ける。各ロード後には標準宗教の創始種別・聖都も別に検査する。不一致は `ui_lua` の `error()` またはドライバーの比較エラーで失敗する。

## 結果と未検証

2026-09-30：宗教PoCの24テスト、リポジトリ全350テスト、自動検証ツール150テストが成功。シナリオの `--check` と全24件のLuaコード片の構文検査も成功。指定の `-t .` での検出を有効にし、制限付きWindowsトークンでPythonの0700一時ディレクトリへ再アクセスできなかった問題をテスト専用の権限継承方式で解消した。既存ACLやゲーム・通常アプリの一時領域は変更していない。

実機は未実行。特に独自表のGameInfoからの取得、Gameプロパティの入れ子全体・空テーブル・false値の保持、GameCore_TunerからのAPI参照、カスタム宗教の創始と読込、日英露の実表示は要実機確認。マルチプレイも未検証。乱数は使わず、保存文字列のキー順を固定しているが、将来のUI入力の同期確認が必要。再興人数・資料消費と、再興以外の分岐効果はオーナー判断後に接続する。
