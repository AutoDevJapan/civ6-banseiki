# 束2 PoC：勢力吸収

公式英雄の API 調査では、古い FireTuner パネルの `GetLifespan` / `SetLifespan` の例だけでは判断しない。導入済みの `Base/Assets/UI/Panels/UnitPanel.lua:2269` は、残り寿命を `unit:GetProperty(UnitPropertyKeys.LifespanRemaining)` で読む。実機ではこの定数表が画面側にだけ存在したため、画面側の実際の数値キーを共有してゲーム処理側から読む。実機診断では、所有者・召喚元都市・召喚元建物の照会、都市の信徒一覧と作品スロットの照会も画面側にだけあり、生成と発見の変更はゲーム処理側にだけあった。画面側の照会関数は数値・都市ID・信徒数の表だけを返す窓口として公開し、変更はゲーム処理側で行う。手番経過後の本体の寿命消費は実機シナリオで検査する。

`poc/JAC_PoC_Absorption` は、SSOT 9.1 と 16.1「吸収」の移管・保存の検証を行う。実機の最新試行は英雄生成と公開待ちを通過した後、布教回数を読むユニットのスナップショットで停止し、吸収と再召喚の成功はまだ確認できていない。Lua の模擬エンジンでの成功と、Civ6 本体での成功は分けて扱う。

9.4 の参照範囲は「小国懐柔」「大国の合併」の加入・前提条件、「合意による関係宣言」の共存と吸収の区別、「都市と国」の統治・忠誠である。使節や合併条件を評価する画面、Governor への進化、独立、旧案の9状態はこの PoC の対象にしない。小国／大国の分類や合併条件を勝手に決めない。

## 構成と呼出方法

- MOD ID：`61320778-2e75-4c11-9009-94101b519aba`。Gathering Storm、Babylon DLC、万世紀ルールセットを依存 MOD とする。SSOT 9.1 の公式英雄が必須という判断に従う。日本地図シナリオは既存のカスタム文明・戦国ツリー・日本地図も有効にする。
- `Data/JAC_Absorption.sql`：資産の種類・分岐結果・受入先の表。確率や効果量は持たない。
- `Scripts/JAC_Absorption_Rules.lua`：保存キー、未決事項の PoC 定数、分岐の検証、複製、照合、人物の Unit 参照の付け替え。
- `Scripts/JAC_Absorption_Gameplay.lua`：ゲーム側の移管、保存、再開、照合。`UI/JAC_Absorption_HeroKeys.lua` は画面側にある本体の寿命キーと英雄管理表の照会関数を公開し、ゲーム状態を変更しない。
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
| Hero | 万世紀ルールセット固有の公式英雄2クラスを使う。登用／保護／再契約では召喚元都市を移し、`HeroesManager` で同じクラスを生成して所有者・召喚元・残り寿命・能力回数と人物記録を照合する。明示した自害、休眠中の召喚権、潜伏／離反、英雄専用の生成失敗は `SUICIDE` に確定し、元の希望結果・原因・取得できた本体状態・契約・忠誠・関係値を保管する。返り値の `HeroResults[人物ID]` と保管記録で継承と自害を区別できる | 本体の旧召喚回数・時代など内部履歴を移す公開 API は未確認。自害として保管した権利は受入側へ移したと主張しない。実機では未検証 |
| Great Person 相当 | ネイティブ偉人は個体 **Index** で `CreatePerson` し、個体 ID の一致を照合する。登用／亡命／隠遁を表す。吸収側に残る偉人は本人の能力を保って人物のまま登用する。科学・信仰・文化への資産化は行わない | ネイティブ偉人の通常の引退能力は変更しない |
| 仲間入りした当主 | `leader:<旧勢力ID>` の永続人物と `UNIT_GREAT_GENERAL` の代理を受入側に作る。人物と代理の対応も保存する | ネイティブ個体の引退能力を付けずに当主を残す代替表現。固有能力・性格の生成・Governor への進化は未実装 |
| 装備・Artifact・収蔵品 | 固有 ID と Vault／都市・城／人物への関連を保つ。移管／略奪／接収／返還／散逸を記録する。返還は存続する第三勢力を明示する。受入側にある別の品を上書きしない | 本体 Great Work は都市のスロットも検査する。人物用の装備・Vault・収蔵品は独自プロパティ。装備効果や返還の外交報酬は対象外 |
| 解放ジョブ・技術・制度 | 呼出側が指定した知識レコードだけを継承。ジョブや文書は固有 ID を保つ。指定された技術・制度は受入側の `SetTech` / `SetCivic` で解放し、指定しない知識は保管する | 全研究を丸ごとコピーしない。何を人物・文書・施設・文化から継承できるかの条件は加入判定側で決める |
| 宗教 | コミュニティごとの固有 ID と宗教 ID を分け、信徒・聖地・聖職者・教典・遺物・宗派・持続状態を移管する。Founder の歴史上の勢力 ID を変えず、現在の Owner を変える。同じ宗派の受入側コミュニティも残す。都市のネイティブ信徒数・宗教 Unit の宗教も照合する | 地下化・滅亡・再興の遷移は9.2／9.3の別 PoC の責任。ネイティブの「創始文明」の書換えや万世紀20宗教のデータ定義は行わない |

分岐条件は SSOT に数式がないため自動判定しない。全 Unit・全資産・当主の結果を指定しなければ、エンジンを変更する前にエラーにする。ただし、Unit のない公式英雄の権利だけを管理表から見つけた場合は、自害の記録を自動で作る。矛盾する人物と Unit の結果、資産 ID の衝突、無効な返還先も事前に拒否する。相互保護・従属国・植民地国・連邦関係と、未採用の自治・直轄を吸収方式として受け付けない。

## 保存単位と失敗時の扱い

画面側の英雄管理表はゲーム処理中の変更が即時には見えない。生成した候補のIDを保存し、画面側がゲーム処理の更新を受け取って照会結果を公開するまで `WAITING_UI` として返す。これは成功でも自害でもない。呼出側は次の処理段階で同じIDを `A.Resume` し、候補を増やさず照合する。更新後にも管理表が一致しない英雄だけを自害として確定する。保存用の表に候補IDと待機状態を残す。Luaスクリプトの再初期化は検査済みだが、待機中の実際のゲーム保存・読み込みと再開は未検証。シナリオは一度のLua処理を終えてから待機・再開し、実ターンを大量に進めない。

人物・所持品・宗教・知識は各 Player の `JAC_ABSORPTION_ASSETS_V1` に、固有 ID をキーにした表で保存する。最低限の形式は `{ID, Kind, Owner, ...}`。`Kind` は `PERSON` / `HERO` / `GREAT_PERSON` / `ITEM` / `UNLOCK` / `RELIGION`。人物の `Unit={Owner,ID}` と都市所蔵の `City={Owner,X,Y}` は移管先へ付け替える。`Anchor={Kind,ID}` の固有 ID、宗教の Founder、その他の歴史上の ID は変更しない。

Game の `JAC_ABSORPTION_STATE_V1` に入力、元データ、置換 Unit の対応表、都市・領土、処理状況、移管されない資産を保存する。Unit の `JAC_ABSORPTION_UNIT_TOKEN_V1` は「処理 ID＋旧 Unit ID」の永久の識別子。Unit ID が変わっても追跡できる。既存の成長 PoC の `JAC_POC_GROWTHUI_WARLORD` も複写する。空文字列は保存しない。nil で削除する。

一般 Unit の順序は、入力検査 → 保存 → 全 Unit の生成・復元・照合 → 旧 Unit の除去 → 都市と領土 → 人物・所持品・宗教・知識 → 統合口 → 最終照合。最後の都市を移した際に旧文明の Unit が消える場合にも、一般 Unit は先に生成されている。

公式英雄は召喚元の都市が必要なので、この順序に例外がある。一般 Unit を先に復元・検査し、英雄の本体・人物・管理表の状態を保存してから旧英雄を除去する。都市を移した後に同じクラスを生成し、保存した ID 集合、クラス、管理表、召喚元を照合する。英雄専用の失敗なら候補が自分の処理で新しく発行されたことを確かめて除去し、人物を自害の履歴へ移す。都市・一般 Unit・資産・統合口の失敗は従来どおり `BLOCKED` とする。権利を継承できない休眠中の英雄も、吸収全体は止めずに自害の履歴へ移す。

英雄には生成前の Unit ID 集合と候補 ID を保存する。生成 API が例外を投げても、正しい所有者・クラス・召喚元を持つ候補が1体できていれば採用する。確認できない候補や別の取引のトークンは削除しない。第三勢力の Unit は削除しない。候補が複数あるなど帰属を決められない場合は `BLOCKED` とし、成功や自害の確定を装わない。元の入力は `Request` に保存し、実際に自害へ変わっても同じ入力の再実行だけを受け付ける。

実際の再召喚は、ゲーム側で `A.BeginHeroRecall(recallID, transactionID, recordID)` を呼び、UI 側で公式画面と同じ `CityManager.RequestCommand(..., PURCHASE, ...)` を発行した後、ゲーム側で `A.CompleteHeroRecall(recallID)` を呼ぶ。開始時に既存 Unit ID と人物記録を保存し、完了時に新しい1体だけへ専用トークンを付ける。人物レコードと Unit の人物プロパティの双方を新 ID へ更新し、契約・忠誠・関係値を保つ。候補がない、複数いる、別のトークンを持つ場合は記録を書き換えずに停止する。現 PoC に一般プレイヤー向けの購入 UI フックはなく、2本目の実機シナリオがこの順序を実行する。

英雄以外の失敗と、候補の帰属が判断できない場合は `BLOCKED` と理由を保存し、成功と報告しない。失敗前の都市・Unit・プロパティのスナップショットは残る。ただしエンジン変更全体のロールバックではなく、途中の置換 Unit が存在し得る。同じターンの `A.Resume(id)` は保存した Unit トークンを発見して重複生成を防ぐ。参照の期待値を計算し始める前に対応表全体を固定する。期待値が一部だけ完成した段階でも、参照先の Unit を別 ID へ生成し直さない。受入側の資産は現在の一覧へ移管分だけを加え、処理の途中で追加・更新された別資産を上書きしない。移管元の一覧が途中で変わった場合も、新しい資産を黙って消さず処理を止める。次のターンへ進んだ後の失敗復旧は自動で行わず、判断が必要として拒否する。完了した同じ入力の再実行は処理を繰り返さない。同じ ID に別の入力を渡すことも拒否する。

本体 Unit の傷、残り移動、経験値、昇進、編制、名前、偉人個体と能力の残り回数、宗教、建造・布教回数を対象にする。移管するネイティブ偉人の能力回数を取得できなければ、エンジンを変更する前に拒否する。消費済みの建造・布教・偉人の能力回数を新 Unit で再現できないときは、旧 Unit を消す前に止める。護衛リンク、交易路、進行中の作業・任務、Unit が持ち歩く未設置 Great Work、未登録の他 MOD のプロパティはこの方法だけでは移管できない。利用する機能がある場合は先に統合口で加入可否を検査する。これらを保存済みと主張しない。

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
| 公式英雄の残り寿命は Unit プロパティ | `Base/Assets/UI/Panels/UnitPanel.lua:2269` の `UnitPropertyKeys.LifespanRemaining`。`Debug/Heroes.ltp:18-55,136-139` の寿命・名声メソッドは旧例であり、導入済み XP2 DLL の Lua 登録に同名の関数がない。残り寿命のプロパティをゲーム側で書き戻して本体が消費することは要実機確認 |
| 公式クラス、管理表の所有者、再召喚元の都市 | `DLC/Babylon/UI/Additions/HeroesPopup.lua:265`、`GreatPeopleHeroPanel.lua:124-159,254-278`。Component ID の `player` / `id` は `Base/Assets/UI/Choosers/TradeRouteChooser.lua:1037-1038` にも例がある |
| ゲーム側の公式英雄生成 | `DLC/Expansion2/Binaries/Win64/GameCore_XP2_Release.dll` の Lua 登録 RVA `0x9d5980-0x9d5a59`、`CreateHero` のラッパー RVA `0x9d5ad0-0x9d5bab`。引数は `CreateHero(playerIndex, heroClassIndex, x, y)`。返す Unit はない。呼出先 RVA `0x8fb760` は都市を取得し、生成前に権利の所有者を変更する。DLL の SHA-256 は `acd6cf2f352ae30ddf6f752bd7661165d9a5cf070f5519425ebab9ee0b865521`。これはインストール済みファイルの静的確認で、実機での移管成功の証拠ではない |
| 発見状態、召喚元の建物、能力回数 | `Debug/Heroes.ltp:62-68,192-209` と `DLC/Babylon/UI/Additions/HeroesPopup.lua:79-81`。能力回数の変更は公式例の `ChangeActionCharges` を使い、0回も復元・照合する |
| Heroes & Legends を有効にする設定 | `DLC/Babylon/Babylon.modinfo:2,47-54` の DLC ID と `GAMEMODE_HEROES`。`DLC/Byzantium_Gaul` の Lua・設定も調べたが、英雄の移管・権利だけの変更の使用例は見つからなかった |
| 再召喚の可否・費用と実際の購入 | `DLC/Babylon/UI/Additions/GreatPeopleHeroPanel.lua:202-245,254-278`。UI 側で英雄 Hash・標準編制・信仰 Yield を渡して可否と費用を照会し、同じ引数で `CityManager.RequestCommand(..., PURCHASE, ...)` を発行する。ゲーム側では新しい Unit ID と人物記録を結び直す |
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
python -m unittest discover -s tools/civ6-autotest/tests
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/absorption-japan.json --check
```

`tests/test_absorption_lua.py` は lupa を必須として直接 import し、スキップしない。戻り値のない生成、最後の都市で旧文明の Unit が消える模擬エンジン、ID の変更、昇進による傷・移動のリセット、都市移管でのプロパティの消失を再現する。全資産の移管・処分・知識の解放、保存プロパティの複製と Lua の再初期化、衝突・不明な分岐・失敗・復旧・重複抑止・照合エラー・他機能の統合を検査する。シナリオに埋めた照合コードも VM で実行し、改変された Hero を検出する。

公式英雄用の模擬エンジンは、Unit の一覧と独立した管理表を持つ。寿命には本体のプロパティを使う。固有の2クラス、休眠中の権利、潜伏・離反・明示的な自害、英雄専用の生成・復元エラー、一般 Unit と都市の停止、候補 ID とトークンの衝突を検査する。購入による再召喚は新 ID を発行し、保存と再読込を挟んで人物記録への再接続までシナリオの Lua を模擬エンジンで実行する。模擬エンジンと本体の挙動が一致するかは実機で確認する。

実機で検証するシナリオは2本。Civ6を起動して試行したが、吸収処理の前に停止した。

```text
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/absorption-japan.json
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/absorption-hero-recall.json
```

どちらも日本15 km史実地図・22家、織田家を人間、蘆名家と伊達家を AI にする。Babylon DLC と Heroes & Legends を有効にする。英雄クラス表には万世紀固有の武将A・Bだけがあることを照合し、既存の公式英雄は登場させない。最初のシナリオは蘆名家の別々の都市に固有の公式英雄2体を召喚し、能力回数を2回／0回、残り寿命を19／21にする。人物・ネイティブ偉人・宗教 Unit・装備・Artifact・収蔵品・ジョブ・文書・宗教コミュニティも置く。本体の宗教を創始し、両都市に信徒を作る。伊達家の既存の品と同じ宗派の別コミュニティが上書きされないことも確認する。

英雄生成と移管の更新待ちを含む98手順は以下を自己検証する。不一致は Lua の `assert` / `error` で失敗にする。

1. Lua で蘆名家を伊達家へ吸収する。旧都市・領土の全件、全旧 Unit の種類・個体・傷・移動・経験値・建造／布教回数・プロパティ、人物の全属性、Hero の契約／忠誠／寿命／関係値、3種類の所持品と関連先、当主、宗教の全属性を確認する。ネイティブ宗教の定義・創始者・信仰と、都市の信徒数・建物・Great Work のスロットも確認する。
2. 吸収直後の実データをドライバーへ保存し、`jac-absorption-immediate` をセーブ・メニューへ戻ってロード。Lua に保存前のグローバル変数がなくても全件を再照合し、実データの文字列も `equals_capture` で比較する。同じ吸収要求の再実行で増殖しないことも確認する。
3. `end_turn` の `turns: 1` で相手ターンを進め、人間のターンが有効になるまで待ち、旧所有者への逆戻り・全人物／Hero／装備・所持品／宗教の消失を検出する。吸収直後に保存した別の期待値で、各資産の所有者・関連先と Unit の人物データを検査する。傷・移動・信徒数などの通常の変化は新しい現在値として記録する。
4. `jac-absorption-after-turn` を保存・ロードし、その現在値の全件が再び一致することを確認する。セーブ名への実行別の接尾辞は既存ドライバーに任せる。
5. UI 側の本体の購入可否を照会し、生存中の2体の再召喚が拒否されることを確認する。その後、武将Aを検査用に除去し、信仰を補充する。同じ時代では拒否され、次の時代には召喚元で購入可能になり、費用が正になることを確認する。受入先は AI の伊達家なので、ここでは購入命令を出さない。

2本目の89手順では蘆名家の固有英雄1体を人間の織田家へ移す。吸収直後と保存・再読込後に所有権を確認し、実際に1ターン進めて寿命を確認する。旧本体を退役させ、ゲーム側で再召喚の意図を保存してから、織田家の都市に UI 側の公式購入命令を出す。新しく発行された ID を人物記録へ結び直し、契約・忠誠・関係値、信仰の実支出、購入前後の費用、英雄作品を含む Great Work の数を記録・照合する。再度保存・読込して人物の接続を確認する。時代は検査用 API で進め、何百ターンも送らない。

実機では生成 API のゲーム側権限、同一の偉人個体の再生成、宗教 Unit の宗教設定、2都市の移管・消滅処理の副作用、相手ターンの進行、セーブの一致、Database / Modding / Lua のログが未確認。実機結果を得る前に成功済みとは記録しない。

全体テストとドライバーのテストは、修正後の各コミットで再実行している。2本のシナリオは `--check` で各98・89手順の JSON と Lua 構文を検査した。現在の件数・コミット・レビュー結果はPR #87に記録する。ACLの変更や一時ディレクトリの回避策は追加していない。

布教回数は、ゲーム処理側に取得 API がある場合はそれを使い、無い場合は画面側から数値だけを読む。移管元も、要求後の公開完了で取得した値を使ってから候補を生成する。同じ手番でも画面の値が古く、使った回数が戻る不具合を回帰テストで再現して直した。新しい候補の照合段階と ID を保存し、公開完了後に同じ候補で再開する。シナリオは候補ごとの更新待ちを最大32段階まで再開し、最後に処理の完了を断言する。布教役の生成後も、画面側の公開を待ってから移管前の値を記録する。

### 実機の試行結果（2026-10-01）

| 実行ID | 対象 | 結果と修正 |
|---|---|---|
| absorb-01 | absorption-japan.json | MODの有効化で依存関係エラー。Babylonの公式ID表記と、有効化する順序を修正した |
| absorb-02 | absorption-hero-recall.json | 同じ依存関係エラー。吸収や再召喚には未到達 |
| absorb-03 | absorption-japan.json | 依存関係の解決・英雄モードの設定・新規ゲーム開始は成功。ゲーム処理側に寿命キーの定数表がなく、英雄生成前に停止。画面側からキーを共有する処理を追加した。アイコンSQLの列名エラーも修正した |
| absorb-04（再起動の再試行を含む） | absorption-japan.json | ゲーム起動待ちで停止。起動要求4回の後も指定期限まで待つドライバーへ修正し、回帰テストを追加した。英雄の移管は未到達 |
| absorb-05 | absorption-japan.json | 旧ゲームの報告用プロセスだけが残りSteamの再起動を妨げていたため、所属と旧ゲームの終了を確認して閉じた。再要求後はゲーム開始、22家、英雄モード、吸収MODと寿命キーの読込みまで成功。英雄の生成準備で所有者照会が「function expected instead of nil」となり停止。診断で3照会が画面側にしかないことを確認し、両側のAPIを使う窓口を追加した。吸収には未到達 |
| absorb-06 | absorption-japan.json | 照会窓口と英雄生成は動作したが、生成した直後の画面側の管理表との照合で停止。生成と画面側の更新後の照合を分け、移管APIは更新待ちを保存して次の段階で同じ候補を再検査する設計へ変更した。修正後の実機は未実行 |
| absorb-07 | absorption-japan.json | 独自英雄2体の生成と画面側の更新後の照合に成功。信徒一覧の照会がゲーム処理側に無く停止。都市の信徒一覧・作品スロットの3照会が画面側にのみ存在することを実機診断し、同じ窓口へ追加した。都市の移管後も画面側の公開を待ってから最終照合する。修正後の実機は未実行 |
| absorb-08（再起動の再試行を含む） | absorption-hero-recall.json | 起動した本体が窓を出す前に終了し報告プロセスだけが残った。確認して閉じ、再起動後は新規ゲーム・独自英雄の生成・公開待ちまで成功。ユニットのスナップショットで布教回数のGetterがゲーム処理側に無く停止。移管・保存・再召喚は未到達。実行途中で起動が失敗した場合も次の起動要求前に報告プロセスを確認するようドライバーを修正した |

InGame側で読み出した本体の寿命キーを共有し、GameCore_Tuner側から同じ数値を読めることは実機で確認した。absorb-05ではMOD自身によるキー公開とゲーム処理側の吸収APIの読込みまで確認した。独自英雄2体の生成・移管、寿命消費、保存・読み込み、再召喚購入は未確認である。

最初の担当は実機4回で引き継いだ。後続の担当が最大4回ずつ修正と検証を続け、2本のシナリオが成功するまでマージしない。テストの継続や再試行は担当が判断する。検証用worktreeの `tools/civ6-autotest/artifacts/absorb-01`〜`absorb-04` に失敗ログを残している。

再起動時には旧ゲームの報告用プロセスだけが残り、Steamがゲームを実行中と見なして起動を拒否することがあった。ドライバーはゲーム本体が動いていないこと、親プロセスが存在しないこと、実際の実行ファイルがCiv6のBase/Binaries/Win64Steamにあることを確認し、取得した同じプロセスのハンドルだけを終了する。終了直前の再確認でゲーム本体が見つかった場合は中断する。確認と終了をOS全体で排他的に行う保証はないが、終了対象は検証済みの旧報告プロセスのハンドルに限られ、新しいゲームや報告用プロセスを終了しない。

Shepherd が汎用ドライバーで設定を扱う場合に必要なのは、Game グループの `GAMEMODE_HEROES=true`（XML の照合値は1）と、DLC ID `8424840C-92EF-4426-A9B4-B4E0CB818049` の有効化である。現シナリオは既存の `ui_lua` / `FrontEnd` で適用するため、汎用ドライバーは変更していない。

## 公式英雄の未解決点と実機確認

- **名声**：導入済み DLL と実行ファイルに `GetRenown` / `SetRenown` の Lua 登録が見当たらず、`UnitPropertyKeys` に対応する公開名も見当たらない。人物記録の `Renown` はそのまま移す。実行時に旧例の Getter と Setter が存在する場合だけ本体の値も復元する。存在しない場合は `NativeHero.RenownUnavailable=true` として保存し、本体の名声を保存できたとは主張しない。
- **召喚履歴**：公開されている変更口は新規生成の `CreateHero` であり、権利だけを移す Setter は見つからない。この呼出しは受入側に新しい召喚履歴を作る。旧所有者の内部の召喚時代・召喚ターン・再召喚履歴を読み出し、同一の値で受入側に移す API は確認できない。元の英雄が以前の時代から存続する場合や既に再召喚された場合について、以前と同じ再召喚制限・費用になることは保証しない。実機シナリオの2体は同じ時代に初召喚した例である。
- **死亡済みの英雄**：移管元だけが権利を持ち本体 Unit がない場合、公開 API で権利を移せない。取得できるクラス・旧所有者・召喚元・既存の人物記録を自害の履歴へ保存する。受入先に召喚権ができたとは報告しない。
- **生成失敗後の管理表**：`CreateHero` は本体を作れなくても権利の所有者を変更し得る。自害の記録には失敗後に読み取れた `NativeClaimAfter` を残す。管理表の権利を取り消す公開 API は未確認であり、自害の記録だけで本体の再召喚を禁止できたとは主張しない。
- **本体の副作用**：残り寿命の書戻しと再生成が本体の作品・信仰費用・召喚制限へ及ぼす影響は未確認。実機シナリオは Great Work の数と費用の実際の変化を残す。元からある作品の消失は照合で検出する。内部召喚履歴の保存を推測しない。
- **実際の再召喚**：2本目のシナリオが人間の織田家で購入命令まで実行する。購入 UI と人物記録の結合は PoC の2つの API を順に呼ぶ方式であり、通常のゲーム画面からの再召喚すべてに自動で適用される機能はまだない。購入が拒否されたり候補が紛らわしかったりすれば検証は失敗する。

以上はレビューとマージ判断に残す。公式英雄の完全対応、実機成功、Must がないことは、この変更だけでは宣言しない。

## 判断待ちの定数

別モデルのレビューとLuaの回帰テストで、部分失敗後の参照切れ・受入側資産の上書き・宮殿作品の検査漏れ・統治履歴の消失・相手ターン後の検査不足を修正した。独自の公式英雄、自害、再召喚もレビューした。実機成功やマルチプレイの同期は、Luaの模擬エンジンの結果だけでは確認できない。

数値が未決の項目は控えめなPoCの定数で扱う。分岐・忠誠と文化摩擦は、`documents/questions.md` のQ84〜Q85に記録している。オーナーの回答後にSSOTと実装へ反映する。

| 定数・処理 | 今回の控えめな扱い | 判断が必要な点 |
|---|---|---|
| `REQUIRE_EXPLICIT_DISPOSITIONS_POC = true` | 呼出側が全結果を指定する | 方式・忠誠・契約・寿命・関係値から結果を選ぶ規則と確率 |
| `CITY_LOYALTY_DELTA_POC = 0`、`CULTURAL_FRICTION_TURNS_POC = 0` | 統治の経緯だけ追加保存し、追加ペナルティは与えない | 忠誠低下・文化摩擦の効果量と期間、征服と合意吸収の差 |
| `LEADER_REPRESENTATION_POC` | 永続人物＋引退能力のない大将軍代理 | 本実装の人物・能力・Governor との連携は指導者人物化 PoC で扱う |

一時的な方針と未検証箇所を、本実装の仕様確定と混同しない。


## 次の担当が再開する実機確認

今回の担当はabsorb-05〜08の4回で終了する。追加実行をオーナーへ質問しない。両シナリオが成功するまでPR #87をマージしない。

- `absorption-japan.json`（98手順）：都市の3照会を修正した後の全体は未実行。一般Unit・偉人・人物・英雄2体・所持品・宗教、直後と相手の手番後の保存・読み込みを確認する。
- `absorption-hero-recall.json`（89手順）：ユニットのスナップショットで停止。診断では経験値・編制・建造回数・名前は両側で取得でき、布教回数の `GetSpreadCharges` だけがゲーム処理側に無かった。布教回数の読み取りを、ユニット生成後や復元後の公開待ちも含めて再開する。移管・手番後の寿命・実際の購入・人物の再接続・保存・読込は未確認。
- `WAITING_UI` 中の実際の保存・読込・再開とマルチプレイ同期は引き続き未検証。現在の模擬エンジンの成功を、その実機確認の代わりにしない。
