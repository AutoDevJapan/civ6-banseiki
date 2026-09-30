# 束2 PoC：勢力吸収

`poc/JAC_PoC_Absorption` は、SSOT 9.1 と 16.1「吸収」の移管・保存の検証を行う。実機では未実行。Lua の模擬エンジンでの成功と、Civ6 本体での成功は分けて扱う。

9.4 の参照範囲は「小国懐柔」「大国の合併」の加入・前提条件、「合意による関係宣言」の共存と吸収の区別、「都市と国」の統治・忠誠である。使節や合併条件を評価する画面、Governor への進化、独立、旧案の9状態はこの PoC の対象にしない。小国／大国の分類や合併条件を勝手に決めない。

## 構成と呼出方法

- MOD ID：`61320778-2e75-4c11-9009-94101b519aba`。Gathering Storm と万世紀ルールセットだけを依存 MOD とする。日本地図シナリオは既存のカスタム文明・戦国ツリー・日本地図も有効にする。
- `Data/JAC_Absorption.sql`：資産の種類・分岐結果・受入先の表。確率や効果量は持たない。
- `Scripts/JAC_Absorption_Rules.lua`：保存キー、未決事項の PoC 定数、分岐の検証、複製、照合、人物の Unit 参照の付け替え。
- `Scripts/JAC_Absorption_Gameplay.lua`：ゲーム側の移管、保存、再開、照合。UI スクリプトはない。
- `Text/JAC_Absorption_Text.xml` と modinfo：日本語・英語・ロシア語。新しい文明・指導者や画像は追加しない。旧当主の代理 Unit に公式の大将軍を使うのは、人物の存続を検証するためである。

ゲーム側 Lua / `GameCore_Tuner` から次の形で呼ぶ。画面・AI の将来の加入判定が結果を選び、この API に渡す。外交の関係宣言だけで自動吸収するイベントは登録しない。

```lua
local A = ExposedMembers.JAC_Absorption
A.Apply({
  ID = "unique-operation-id", From = donorID, To = absorberID, Mode = "MERGER",
  EntryConditionsMet = true, RelationsResolved = true,
  Leader = "RECRUIT",
  Units = { [A.Rules.UnitKey(donorID, unitID)] = "INCORPORATE" },
  Assets = { [stableAssetID] = "TRANSFER" }
})
```

`Mode` は `CONQUEST` / `SMALL_STATE` / `MERGER`。加入条件を満たすことと、他国との関係を解消・合意済みにすることは呼出側の責任である。未決の外交条件を `true` で迂回する通常の画面は作っていない。実機シナリオの `true` は移管 API 自体を試すための明示的な入力である。PoC では吸収される側を AI に限定し、両国が都市を持つ間に呼ぶ。最後の都市を既に占領した後から失われた Unit を復元する仕組みではない。

## SSOT 9.1 の各規則

| 対象 | PoC の実現方法・保存後の照合 | 制限 |
|---|---|---|
| 都市・城・領土 | `CityManager.TransferCity(city, To)`。中心座標で新しい都市を取り直す。全旧領タイルの所有者、人口、宗教ごとの信徒数、建物・略奪状態・Great Work のスロット、登録した都市プロパティを照合する。城の座標を変更しない | 首都の Palace 建物はエンジンの管理に任せる。宮殿の作品は別に保存し、受入側の全都市で追跡する。建物・Great Work が失われれば成功扱いにせず `BLOCKED`。歴史資産の代替レコードは別途保存する |
| 統治・忠誠・文化摩擦 | 都市に移管元・移管先・方式・ターン・前回の統治記録を保存する。Civ6 の通常の都市・忠誠処理を維持する | 追加の忠誠変化・文化摩擦の効果量は未決で0。PoC の記録だけでは経済ペナルティを実装したことにならない |
| 一般 Unit | 降伏／編入は同じ種類の Unit を移管先に生成し、個体の状態を戻す。生成 API の戻り値は使わず、前後の Unit ID 集合の差で発見する。全置換 Unit の復元・検証が終わってから旧 Unit を消す | 解散／逃亡／抵抗は元の Unit の完全な検証対象スナップショットと結果を保存して地図から外す。逃亡先の勢力や抵抗部隊の行動 AI は未実装 |
| 武将 | 捕縛／登用／臣従は人物レコードと Unit を移管。亡命／処刑／隠遁／離反は記録を残す。レベル・性格・特性・装備などの中身を保ち、明示された Unit 参照だけを付け替える | 捕虜の命令制限、亡命先への配置、処刑・隠遁の演出は未実装。処刑でも履歴を削除しない |
| Hero | 登用／保護／再契約は Unit と人物レコードを移管し、契約・忠誠・寿命・関係値を保つ。離反／潜伏も消さずに保管する | 今回の Hero は Unit プロパティによる代理。公式 Heroes モードの召喚・回想・寿命消費は扱わず、ネイティブ Hero の移管を拒否する |
| Great Person 相当 | ネイティブ偉人は個体 **Index** で `CreatePerson` し、個体 ID の一致を照合する。登用／研究資産化／信仰資産化／文化資産化／亡命／隠遁を表す。資産化は人物の記録を受入側に残して地図の Unit を外す | 資産化による科学・信仰・文化の報酬量は未決。ネイティブ偉人の通常の引退能力は変更しない |
| 仲間入りした当主 | `leader:<旧勢力ID>` の永続人物と `UNIT_GREAT_GENERAL` の代理を受入側に作る。人物と代理の対応も保存する | ネイティブ個体の引退能力を付けずに当主を残す代替表現。固有能力・性格の生成・Governor への進化は未実装 |
| 装備・Artifact・収蔵品 | 固有 ID と Vault／都市・城／人物への関連を保つ。移管／略奪／接収／返還／散逸を記録する。返還は存続する第三勢力を明示する。受入側にある別の品を上書きしない | 本体 Great Work は都市のスロットも検査する。人物用の装備・Vault・収蔵品は独自プロパティ。装備効果や返還の外交報酬は対象外 |
| 解放ジョブ・技術・制度 | 呼出側が指定した知識レコードだけを継承。ジョブや文書は固有 ID を保つ。指定された技術・制度は受入側の `SetTech` / `SetCivic` で解放し、指定しない知識は保管する | 全研究を丸ごとコピーしない。何を人物・文書・施設・文化から継承できるかの条件は加入判定側で決める |
| 宗教 | コミュニティごとの固有 ID と宗教 ID を分け、信徒・聖地・聖職者・教典・遺物・宗派・持続状態を移管する。Founder の歴史上の勢力 ID を変えず、現在の Owner を変える。同じ宗派の受入側コミュニティも残す。都市のネイティブ信徒数・宗教 Unit の宗教も照合する | 地下化・滅亡・再興の遷移は9.2／9.3の別 PoC の責任。ネイティブの「創始文明」の書換えや万世紀20宗教のデータ定義は行わない |

分岐条件は SSOT に数式がないため自動判定しない。全 Unit・全資産・当主の結果を指定しなければ、エンジンを変更する前にエラーにする。矛盾する人物と Unit の結果、資産 ID の衝突、無効な返還先もこの段階で拒否する。相互保護・従属国・植民地国・連邦関係と、未採用の自治・直轄を吸収方式として受け付けない。

## 保存単位と失敗時の扱い

人物・所持品・宗教・知識は各 Player の `JAC_ABSORPTION_ASSETS_V1` に、固有 ID をキーにした表で保存する。最低限の形式は `{ID, Kind, Owner, ...}`。`Kind` は `PERSON` / `HERO` / `GREAT_PERSON` / `ITEM` / `UNLOCK` / `RELIGION`。人物の `Unit={Owner,ID}` と都市所蔵の `City={Owner,X,Y}` は移管先へ付け替える。`Anchor={Kind,ID}` の固有 ID、宗教の Founder、その他の歴史上の ID は変更しない。

Game の `JAC_ABSORPTION_STATE_V1` に入力、元データ、置換 Unit の対応表、都市・領土、処理状況、移管されない資産を保存する。Unit の `JAC_ABSORPTION_UNIT_TOKEN_V1` は「処理 ID＋旧 Unit ID」の永久の識別子。Unit ID が変わっても追跡できる。既存の成長 PoC の `JAC_POC_GROWTHUI_WARLORD` も複写する。空文字列は保存しない。nil で削除する。

順序は、入力検査 → 保存 → 全 Unit の生成・復元・照合 → 旧 Unit の除去 → 都市と領土 → 人物・所持品・宗教・知識 → 統合口 → 最終照合。最後の都市を移した際に旧文明の Unit が消える場合にも、先に移管済みとなる。

失敗は `BLOCKED` と理由を保存し、成功と報告しない。失敗前の都市・Unit・プロパティのスナップショットは残る。ただしエンジン変更全体のロールバックではなく、途中の置換 Unit が存在し得る。同じターンの `A.Resume(id)` は保存した Unit トークンを発見して重複生成を防ぐ。参照の期待値を計算し始める前に対応表全体を固定する。期待値が一部だけ完成した段階でも、参照先の Unit を別 ID へ生成し直さない。受入側の資産は現在の一覧へ移管分だけを加え、処理の途中で追加・更新された別資産を上書きしない。移管元の一覧が途中で変わった場合も、新しい資産を黙って消さず処理を止める。次のターンへ進んだ後の失敗復旧は自動で行わず、判断が必要として拒否する。完了した同じ入力の再実行は処理を繰り返さない。同じ ID に別の入力を渡すことも拒否する。

本体 Unit の傷、残り移動、経験値、昇進、編制、名前、偉人個体と能力の残り回数、宗教、建造・布教回数を対象にする。消費済みの建造・布教・偉人の能力回数を新 Unit で再現できないときは、旧 Unit を消す前に止める。護衛リンク、交易路、進行中の作業・任務、Unit が持ち歩く未設置 Great Work、未登録の他 MOD のプロパティはこの方法だけでは移管できない。利用する機能がある場合は先に統合口で加入可否を検査する。これらを保存済みと主張しない。

`A.Verify(id, true)` は同じターンの完全な照合用。後のターンには通常の行動で傷・移動・経験値・信徒数が変わるので、実機シナリオは進行後の現在値を新たに保存し、その保存前後の同一性を比較する。

## 未マージの PR #81 / #82 との統合口

両 PR のファイルを読み込まず、依存 MOD にも追加しない。別の保存形式を推測して同じ名前のプロパティを書かない。

```lua
local A = ExposedMembers.JAC_Absorption
A.RegisterUnitProperty("their_unit_property")
A.RegisterCityProperty("their_city_property")
-- Optional pure callbacks return data with component references rebound.
A.RegisterUnitProperty("their_mission", function(value, transaction, unitSnapshot)
  -- use transaction.UnitMap to update the actor; keep deadlines and fixed rolls
  return value
end)
A.RegisterAdapter("their_feature", {
  Prepare = function(transaction) return {} end,
  Apply = function(transaction, snapshot)
    -- idempotent transfer of feature-owned Player/Game state
  end,
  Verify = function(transaction, snapshot)
    -- error on mismatch
  end
})
```

各関数はゲーム側に登録する。`Prepare` は変更なしの検査・保存、`Apply` は重複実行できる移管、`Verify` は照合。返すデータは表と数値・文字列・真偽値だけにし、保存後はロードのたびにアダプター関数を登録し直す。処理には `From` / `To` / `UnitMap` / `Cities` と元の資産が含まれる。登録したプロパティ名の集合もスナップショットに含め、再読込で検証対象から落ちないようにする。

登録した Unit・都市プロパティ内の参照変更には、省略可能な純粋変換コールバックを使う。全 Unit 対応表の確定後、旧 Unit を消す前に移管後の期待値を計算して保存し、復元と汎用照合に同じ値を使う。再開時は保存済みの期待値を使い、未登録の関数や元の所有者・IDへ戻さない。対応する置換 Unit 自体が失われていたら、自動で別 ID に作り直して参照を壊さず、復旧の検討が必要として拒否する。アダプターの `Apply` は全国ロックなど別の保存単位を移管し、登録したプロパティを期待値と異なる値へ後から変更しない。

- 暗殺者：現在の任務が移管可能か `Prepare` で検査。許可する場合は純粋変換コールバックと `UnitMap` で人物を付け替え、全国共通の待機・生産ロックなど Player に属する状態の統合を担当する。任務期限や確定済みの抽選値を変えず、吸収が任務の実行・再抽選になる処理は入れない。
- 宗教持続：宗派・教典などの正本は PR #82 側に置き、吸収は所有・保護主体だけを渡す。旧勢力の消滅を「信徒0人」に置き換えない。創始者 ID や失われた宗教の判定を吸収側から書換えない。

## 公式エンジンの根拠

以下のパスはすべて `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/` からの相対パス。UI / FireTuner の例は API の存在の根拠であり、ゲーム側での権限や副作用まで検証した根拠ではない。新規の組合せはすべて要実機確認。

| エンジンに関する前提 | 公式ファイルと行／検証済みの追加事実 |
|---|---|
| MOD のゲーム側 Lua、インポートする Lua を分けられる | `DLC/BlackDeathScenario/BlackDeathScenario.modinfo:46-48,129-150` の `AddGameplayScripts` / `ImportFiles`。本 MOD は UI Lua と名前を共有しない |
| オブジェクトのプロパティはゲーム側から設定し、UI 側からは operation を使う | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario_StateUtils.lua:65-95,126-145`。Game / Player / Unit の表のセーブ復元は既存の束0・成長 PoC でも実機確認済み（`documents/poc/README.md`） |
| Unit の隣接有効地点への生成と、一覧からの取得 | `DLC/NubiaScenario/Scripts/NubiaScenario.lua:144-151`、`DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:193-207`。返り値が Unit でないことは今回の依頼で実機確認済みと指定された |
| 偉人生成に渡すものは個体 Index | `Base/Assets/UI/Tuner/TunerMapPanel.lua:180-193` と `Debug/Map.ltp:564-579`。同じ個体を別の家へ再生成できるかは要実機確認 |
| 偉人の個体を読み取れる | `DLC/NubiaScenario/Scripts/NubiaScenario.lua:116-123` |
| 消した Unit の参照を再使用しない | `DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:807,840` |
| Unit を所有者と ID で引ける | `Base/Assets/UI/Panels/UnitPanel.lua:2755,3317` |
| Unit の建造回数・布教回数・軍団編制等の読取 | `Base/Assets/UI/Panels/UnitPanel.lua:2243-2282`。偉人の能力回数は Unit 本体の ActionCharges と別の Getter。Getter のゲーム側での利用は要実機確認 |
| 傷・残り移動・昇進・経験値・名前の復元 | `Base/Assets/Scenarios/Tutorial/Tutorial.lua:71`、`DLC/PiratesScenario/Scripts/PiratesScenario_UnitCommands.lua:295,743`、`Debug/Unit.ltp:164,175,463`、`DLC/NubiaScenario/Scripts/NubiaScenario.lua:150-151`、`DLC/PiratesScenario/Scripts/PiratesScenario_StartScript.lua:303` |
| 公式 Hero には Unit ごとの寿命・名声・行動回数がある | `Debug/Heroes.ltp:128-139`。代理レコードの保存と、ネイティブ Hero の移管は別の確認事項 |
| 座標から都市を取り直せる | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:507-508` |
| 都市移管 | `Debug/City.ltp:277` は自由都市への移管の使用例。正確な `TransferCity` は公式 Lua の使用例がなく、`DLC/Expansion2/Binaries/Win64/GameCore_XP2_Release.dll` の登録文字列（オフセット14318752）が根拠。テキストの行番号はない。実機で動作した事実は `documents/poc/unification-victory.md:82-86`。同 `:15` のとおり移管でも `CityBuilt` が発生するため、Unit と独自資産の保存を先に済ませる |
| 本体の忠誠を変える方法 | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario_UnitCommands.lua:171,235`、`Debug/City.ltp:254-266`。PoC の追加変化は0 |
| 本体の宗教・都市ごとの信徒数、宗教の創始者と信仰の定義 | `Base/Assets/UI/CitySupport.lua:496-519`、`DLC/VikingsScenario/Scripts/VikingScenario.lua:97-103`、`Base/Assets/UI/ReligionScreen.lua:969-990` |
| 実機の宗教テスト用の創始と圧力、Unit の宗教設定 | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:143`、`DLC/VikingsScenario/Scripts/VikingScenario.lua:103`、`Debug/Unit.ltp:485-511`。シナリオだけで公式の `RELIGION_CUSTOM_1` を使い、万世紀の宗教一覧は定義しない |
| 建物・略奪状態・Great Work のスロット | `Base/Assets/UI/GreatWorksOverview.lua:119-129,195-200`、`Base/Assets/UI/CitySupport.lua:611`、`Debug/City.ltp:1060` |
| 技術・制度の解放 | `DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:1368`、`Debug/Player.ltp:198` |

## テストと実機シナリオ

静的・Lua テスト：

```text
python -m unittest discover -s tests -t .
python -m unittest discover -s tools/civ6-autotest/tests -t .
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/absorption-japan.json --check
```

`tests/test_absorption_lua.py` は lupa を必須として直接 import し、スキップしない。戻り値のない生成、最後の都市で旧文明の Unit が消える模擬エンジン、ID の変更、昇進による傷・移動のリセット、都市移管でのプロパティの消失を再現する。全資産の移管・処分・知識の解放、保存プロパティの複製と Lua の再初期化、衝突・不明な分岐・失敗・復旧・重複抑止・照合エラー・他機能の統合を検査する。シナリオに埋めた照合コードも VM で実行し、改変された Hero を検出する。

主セッションが実機で実行するシナリオは1本：

```text
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/absorption-japan.json
```

日本15 km史実地図・22家、織田家を人間、蘆名家と伊達家を AI にする。蘆名家に人物・Hero の代理、ネイティブ偉人、宗教 Unit、装備・Artifact・収蔵品、ジョブと文書、独自宗教コミュニティを置く。本体の宗教も創始し、両都市に信徒を作る。伊達家の既存の品と同じ宗派の別コミュニティが上書きされないことも確認する。

20手順は以下を自己検証する。不一致は Lua の `assert` / `error` で失敗にする。

1. Lua で蘆名家を伊達家へ吸収する。旧都市・領土の全件、全旧 Unit の種類・個体・傷・移動・経験値・建造／布教回数・プロパティ、人物の全属性、Hero の契約／忠誠／寿命／関係値、3種類の所持品と関連先、当主、宗教の全属性を確認する。ネイティブ宗教の定義・創始者・信仰と、都市の信徒数・建物・Great Work のスロットも確認する。
2. 吸収直後の実データをドライバーへ保存し、`jac-absorption-immediate` をセーブ・メニューへ戻ってロード。Lua に保存前のグローバル変数がなくても全件を再照合し、実データの文字列も `equals_capture` で比較する。同じ吸収要求の再実行で増殖しないことも確認する。
3. `end_turn` の `turns: 1` で相手ターンを進め、人間のターンが有効になるまで待ち、旧所有者への逆戻り・全人物／Hero／装備・所持品／宗教の消失を検出する。吸収直後に保存した別の期待値で、各資産の所有者・関連先と Unit の人物データを検査する。傷・移動・信徒数などの通常の変化は新しい現在値として記録する。
4. `jac-absorption-after-turn` を保存・ロードし、その現在値の全件が再び一致することを確認する。セーブ名への実行別の接尾辞は既存ドライバーに任せる。

実機では生成 API のゲーム側権限、同一の偉人個体の再生成、宗教 Unit の宗教設定、2都市の移管・消滅処理の副作用、相手ターンの進行、セーブの一致、Database / Modding / Lua のログが未確認。実機結果を得る前に成功済みとは記録しない。

2026-10-01 の作業環境では、root の370件（追加の lupa / パッケージ検査44件を含む）とドライバーの150件が成功した。Lua テストのスキップは0件。新シナリオをドライバーの固定一覧にも追加した。通常の実行は、Python の一時ディレクトリの権限と、既存ドライバーテストのパッケージマーカー不足で失敗したため、Temp に置いた `jac-absorption-test-runner.py` から同じ全テストを実行した。これは一時ディレクトリの作成時だけ継承する権限を使い、ドライバーテストの探索開始点を補う処理で、既存の ACL は変更しない。リポジトリのテストや `__init__.py` に回避策は追加していない。

## 判断待ちの定数

別モデル（gpt-6.1-sol）のレビューで、部分失敗後の参照切れ・受入側資産の上書き・宮殿作品の検査漏れ・統治履歴の消失・相手ターン後の検査不足を再現し、修正と回帰テストを追加した。修正後の再レビューでは、対象範囲の具体的な Must は残っていない。実機未確認の箇所は上記シナリオで確認する。

仕様には反映しない。オーナーへの質問は主セッションの最終報告にまとめ、`documents/questions.md` には追加しない。

| 定数・処理 | 今回の控えめな扱い | 判断が必要な点 |
|---|---|---|
| `REQUIRE_EXPLICIT_DISPOSITIONS_POC = true` | 呼出側が全結果を指定する | 方式・忠誠・契約・寿命・関係値から結果を選ぶ規則と確率 |
| `CITY_LOYALTY_DELTA_POC = 0`、`CULTURAL_FRICTION_TURNS_POC = 0` | 統治の経緯だけ追加保存し、追加ペナルティは与えない | 忠誠低下・文化摩擦の効果量と期間、征服と合意吸収の差 |
| `CONVERTED_GREAT_PERSON_REWARD_POC = 0` | 受入側の人物資産として記録し、産出の報酬を付けない | 受入時の報酬か、持続的な効果か、記録だけか |
| `LEADER_REPRESENTATION_POC` | 永続人物＋引退能力のない大将軍代理 | 本実装の人物・能力・Governor との連携は指導者人物化 PoC で扱う |

一時的な方針と未検証箇所を、本実装の仕様確定と混同しない。
