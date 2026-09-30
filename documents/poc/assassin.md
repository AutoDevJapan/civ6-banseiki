# 暗殺者 PoC（束2、SSOT 9.5）

正本は [9.5](../spec/ssot/09-assimilation-religion.md) と [16.1 の暗殺者行](../spec/ssot/16-roadmap.md)。この記録は仕様の追加ではない。未決の効果量は `JAC_Assassin_Rules.lua` の `PROVISIONAL` に集め、オーナーの決定として扱わない。

実装は `poc/JAC_PoC_Assassin/`。万世紀ルールセットと、日本15/20 kmの史実領土版・開拓者版の組合せだけでデータ・Gameplay・UIを読み込む。戦国ツリーの LoadOrder 200 より後の350で追加する。オーナー回答はSSOT 9.5へ反映し、回答済みのQ73〜Q78は質問リストから除去する。

**この変更は実機未確認。** 四分岐は下記の推奨案で処理する。吸収は都市移管とUnit解散を呼ぶ。評議会・継承者・人物の所属は独自台帳で表し、Civ6本体の指導者の肖像・特性、Governor効果、外交罰則、会議の議題は未接続である。保存した記録を本体効果の実証と同一視しない。

## 規則との対応

| SSOT 9.5 の規則 | 実現方法／留保 |
|---|---|
| マップ上を歩くUnit、都市・Unitへの任務 | 専用の陸上Unit、移動3・視界2・通常の経路探索。潜入は公式の他国領土進入効果を参照。画面で実在する暗殺者Unitを選び、対象から6タイル以内で実行する。指導者の所在は暫定で首都、Governorは人物登録簿の配属都市。対象への瞬間移動を通常コマンドに含めない |
| 通常100ターン・最低20ターン | 都市を選ぶ独自の育成時計。`ProductionTurns` が補正後に20を適用し、速度換算して `Start/Due/Turns/Speed` を保存。開始からの経過ターンを現在の総所要時間と比較する。完成時にUnitを生成。通常のCiv6都市生産キューとハンマーへの接続は未実装で、育成と通常キューは並行する。育成制度の成立性を検証する代替経路であり、通常の都市生産への統合まで完成したとはしない |
| 外的要因と治安の方向 | 建物・区域・政策・政体・育成する都市の武将特性／ジョブ・治安の各入力を分離。治安0で80、50で100、100で120ターン、他の単位補正は暫定で10/10/10/10/15/15の短縮。Q78に従い毎ターンの現在の入力から再計算する。関係は存命他国に対する最大の敵対度（0〜1）とし、最大で10ターン短縮する暫定定数を使う。入力は後述のプロパティ契約。実際の建物・政策・人物台帳からの自動接続は未実装 |
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
| 指導者死亡後に実行者へ全国土を自動譲渡しない | 当該指導者の死亡と国家の処理を `JAC_ASN_NATION_V1` とGameの履歴へ保存。自由国家・継承は都市を同枠に残す。吸収は実行者へ固定せず、実行時に選んだ隣国へ移す。後継者は新しい人物IDで暗殺対象になる。本体の指導者表示・特性の差替は未達 |
| 死亡後の四分岐 | 下記の推奨案を `POST_DEATH` の名前付き定数の下で実行し、`FREE_STATE / ABSORPTION / SUCCESSION` を保存。会議は保存した票からこの3結果を選ぶ。会議なし／候補者なし／隣国なしの枝を除外。自由国家をFree Cityへ置換せず、継承者・国名は提供された人物登録を使う |
| 状況に応じた重みと実行時固定 | 状況スナップショットと分岐は実行時に保存。Q73の回答どおりコマンド実行時の状況で固定する。受入国、後継人物、会議の票と集計結果も同時に保存する。暫定重みは自由国家 `200-治安+地域不穏度`、隣国吸収 `max(100,100×隣国都市数/対象都市数+25×敵対度)`、会議100、継承 `100+治安`。枝の有無を候補・隣接・会議で絞り、合計重みを乱数の範囲に使う。強さの指標も係数も未決であり、確定仕様にしない |
| 文化圏の名称 | 日本の20家は暗殺忍者 / Assassin Ninja / Ниндзя-убийца。アイヌ・琉球・未確認の国は共通名の暗殺者 / Assassin / Убийца。日本マップ全域へ日本名を一律割当せず、候補表の中国・朝鮮・ニザール派の名称も自動割当しない。日本型と共通型は名前だけを分け、同じ時計・制限・確率を使う |

## 指導者死亡後の全分岐の具体案（Q75）

Q75のcは「全分岐を今具体化する」という回答であり、以下のaの詳細への承認ではない。**各aを推奨案として実装済み**。b/cは選び直すための案であり、変更する際は実装が必要になる。切替定数を未対応の値に変えると、Gameplayの読込みで明示的にエラーになる。

全案で、死亡した指導者は復活させず、Great Personにも変換しない。9.4の「仲間になった指導者のGreat Person化」は存命人物への加入であり、死亡者への例外を作らない。国の取り込みを、9.4で未採用の9状態や一方的な従属宣言に置き換えない。都市・Unitの一覧は結果が出る時点の現存資産を対象にするが、分岐・受入国・継承者・投票結果は実行時に固定する。

### 自由国家

| 案（推奨を先頭） | 都市・Unit | 外交 | 家の指導者・Great Person | プレイヤー枠・AI | 保存・読み込み |
|---|---|---|---|---|---|
| **a: 同じ国を評議会へ移す（推奨：領土と人物を失わず国家単位の自由国家を表せる）** | 都市・Unitの所有を維持。都市単位のFree Cityへ移さない | 既存の戦争・友好・条約を維持。9.4の合併・双方合意の関係宣言で再統合できる | 死亡人物を除き、存命のGreat Person・武将・Governorを維持。評議会は架空の人物名を持たない | 同じ人間／AI枠を維持。既存AIが評議会の国の都市とUnitを操作する | `State=FREE_STATE`、死亡人物・評議会・固定計画を保存。ロードで再抽選や再生成をしない |
| b: 予備枠の評議会へ移す | 都市とUnitを予備国へ移管。Unit固有情報を保存して再生成 | 旧国の対外関係を終了。新国は既存相手と和平から開始し、合意により条約を結ぶ | 存命人物は新国へ加入。死亡人物は歴史へ残す | 旧枠を退役し、開始時に確保した予備AI枠を使う。人間は同じ国へ操作枠を移す必要がある | 旧国・新国の対応、人物、都市・Unitの移管済み印を保存。ロードで移管を重ねない |
| c: 都市ごとの自治に分裂 | 都市をFree Citiesへ移管し、旧国のUnitは解散 | 国家としての外交主体を終了し、都市の再取得・合併で再統合する | 存命人物は元の家の人物登録簿に残し、加入先が決まるまで無所属 | 旧枠を退役。Civ6のFree Cities AIが操作 | 各都市の所在と旧家・人物の対応を保存。国家単位の自由国家の定義変更が必要 |

aの定数は `POST_DEATH.FreeState="SAME_SLOT_COUNCIL"`。存続期限を作らず、再統合は9.4の加入・合併経路で行う。その経路をこのPoCで新たに実装したとはしない。Civ6本体の旧指導者の肖像・特性・AI性格は残る。独自の評議会表示と、本体の指導者差替は別の実装範囲である。

### 隣国による吸収

| 案（推奨を先頭） | 都市・Unit | 外交 | 家の指導者・Great Person | プレイヤー枠・AI | 保存・読み込み |
|---|---|---|---|---|---|
| **a: 最も強い隣国へ合併し旧軍を解散（推奨：旧枠に軍を残さず、合併を都市所有へ反映できる）** | 全都市を実行時に選んだ隣国へ移す。旧国の全マップUnitを解散 | 受入国の既存外交を維持。旧国の戦争・条約を受入国へ新規コピーしない | 死亡指導者は履歴のみ。存命Great Person・武将・Governorは受入国の人物登録簿へ加入し、マップUnitとの二重所属を作らない | 旧国は都市・Unitを失って敗北。受入国の人間／AIが資産を操作し、旧AIは終了する | 受入国、移管した都市の座標、解散Unit、人物所属と旧家を保存。国の履歴はGameにも保存し、ロードで再移管しない |
| b: 同じ隣国へ都市と旧軍を引き継ぐ | aと同じ都市移管。軍・支援Unitを受入国へ再生成し、昇進・経験・個体情報を保持 | aと同じ | 存命Great Personは同じ人物としてネイティブ再付与。Governorは受入国の任命枠へ移す | aと同じ。生成できないUnitは再試行用の保存済み資産とする | 都市移管とUnitごとの再生成の完了印を保存。未完了の再生成だけ再試行 |
| c: 友好を重視して受入国を選ぶ | 隣接国のうち関係が最良の国へ都市移管し、軍はaと同じく解散 | aと同じ | aと同じ | aと同じ | aと同じ。実行時の選定基準と受入国を固定 |

aの定数は `POST_DEATH.Absorption="STRONGEST_NEIGHBOR_DISBAND"`。隣国は距離の暫定定数6以内で都市が接する存命主要国。都市数が多い順、同数なら敵対度が低い順、さらに同じならPlayer ID順で選ぶ。実行者もこの条件を満たす場合だけ候補になる。選んだ国が解決前に滅亡した場合は、`MissingRecipient="FREE_STATE"` に従って評議会へ移す。別の国を選び直さない。

人物登録簿の加入は9.4の人物管理へ渡すための保存形式であり、ネイティブGreat Personの再付与・Governorの任命やスキル移管は未接続。一般Unit・Builder・Heroも解散するaの損失は意図した案の内容である。都市の建物・区域・人口などは本体の都市移管処理に従う。最後の都市を移した後の自然敗北と、旧AIの停止は要実機確認。

### 世界会議の投票

| 案（推奨を先頭） | 都市・Unit | 外交 | 家の指導者・Great Person | プレイヤー枠・AI | 保存・読み込み |
|---|---|---|---|---|---|
| **a: 存命主要国が1国1票で処遇を決める（推奨：Favorの多寡から切り離して結果を再現できる）** | 自由国家・隣国吸収・継承のうち成立可能な案を採決し、対応するaの資産処理を実行 | 採決された処遇のaと同じ | 採決された処遇のaと同じ。候補人物や受入国も実行時に固定 | 採決された処遇のaと同じ | 有権国、各国1票、集計、処遇、受入国・候補人物を保存。最多票を採用し、同票・有効票なしは評議会へ移す |
| b: 本体のFavorを使う会議へ接続 | 同じ3案を既存世界会議で採決し、対応するaを実行 | aと同じ | aと同じ | 採決までは対象枠の評議会と既存AIを維持。投票後に採用案を実行 | 議題ID、会議日、提出票、集計と適用済み印を保存。解決の時点固定を変えるためQ73との再調整が必要 |
| c: 全会一致で吸収・継承を認める | 全有権国が同じ案を選んだ場合のみそのaを実行。それ以外は自由国家a | 対応するaと同じ | 対応するaと同じ | 対応するaと同じ | 有権国と各票、全会一致の判定を実行時に保存し、再投票しない |

aの定数は `POST_DEATH.Congress="ONE_MAJOR_ONE_VOTE"`、同票は `CongressTie="FREE_STATE"`。死亡国を含む実行時の存命主要国が有権国。不参加・未入力は棄権、無効な選択や非有権国の票は数えない。隣国なしの吸収票、候補なしの継承票も無効にする。暗殺者が実行前に知り得るかとは別に、PoCではコマンド実行時に票と採決を固定する。

**日本マップには会議がないため、通常の抽選ではこの分岐を除外する。** `TEST=1` のときだけ、対象国の `Congress={Enabled=true,Votes={['Player ID']='FREE_STATE / ABSORPTION / SUCCESSION'}}` を入力できる。これは票の保存・集計・処遇を検証するテスト用の入口であり、世界会議を開くAPIや正式議題への接続ではない。日本の朝廷を会議の代役にしない。

### 後継者の自動継承

| 案（推奨を先頭） | 都市・Unit | 外交 | 家の指導者・Great Person | プレイヤー枠・AI | 保存・読み込み |
|---|---|---|---|---|---|
| **a: 血縁を優先し1人1役で継承（推奨：人物を捏造せず、指導者と偉人の二重効果を避けられる）** | 同じ国の都市・Unitを維持。ただし指導者になるGreat PersonのマップUnitは除去 | 既存の戦争・友好・条約を維持 | 登録済み存命候補を血縁優先、同順位は人物ID順で選ぶ。Governor候補は人物登録から退役。非血縁の国名は提供済み名称がある場合だけ使用 | 同じ人間／AI枠を維持。既存AIが後継者の国を操作 | 後継者の人物ID・名前・旧役職・国名を固定保存。ロードで再選定・再除去しない |
| b: 国政への適性を優先する | aと同じ | aと同じ | 供給元が与える統治適性の最大値を優先し、同点は人物ID順。二重所属と改名はaと同じ | aと同じ | 適性、候補一覧、選定結果と役職変更済み印を実行時に保存 |
| c: 指導者とGreat Person／Governorを兼任する | 都市と全Unitを維持 | aと同じ | 血縁優先で継承し、偉人UnitやGovernorの配置・効果も維持する | aと同じ | 人物IDと兼任する各役職を保存。二重効果を許可する別仕様が必要 |

aの定数は `POST_DEATH.Succession="KIN_FIRST_SINGLE_ROLE"`。候補は `Successors` 配列または単一の `Successor` に登録する。各候補は `Identity`、`Kind=PERSON / GREAT_PERSON / GOVERNOR`、`Kin=1`（血縁）、任意の翻訳タグ `Name/CountryName` を持つ。Great Personには `Unit`、Governorには `Governor` を指定し、実在と個体トークン／人物IDを照合する。単一の人物ID文字列も既存データ互換のため受け付ける。

選んだ人物が解決前に死亡・消失した場合は `MissingSuccessor="FREE_STATE"` に従って評議会へ移す。別候補へ選び直さない。全種類で候補登録と人物登録簿の死亡印を優先し、それからUnit・Governorの実体を照合する。`PERSON` は供給元が明示的に死亡を記録した場合だけ死亡扱いとする。国名を入力しない非血縁の継承は現在の国名を維持する。独自UIは後継者と提供された国名を表示するが、Civ6本体の肖像・固有特性・AI性格とネイティブGovernor効果の交換は未接続。

## 保存と入力契約

保存単位は国別Player Property `JAC_ASN_STATE_V1`、対象国別 `JAC_ASN_NATION_V1`、Gameの国家履歴 `JAC_ASN_NATIONS_V1`、人物登録簿 `JAC_ASN_HOUSE_PEOPLE_V1`、外交台帳 `JAC_ASN_DIPLOMACY_V1`。Game Property `JAC_ASN_TICK` に最後に処理したターンを保存し、同ターン再処理・ロードで二重完成を防ぐ。外交台帳の `Applied[任務ID]` で罰則の重複を防ぐ。完成状態を通知前に保存し、通知エラーは捕捉する。終了した `Production/Mission` は `nil` で除去する。空文字にしない。実機での保存検証はこれからである。

UnitにはGame Propertyで連番を発行した個体トークンを保存する。登録済み偉人には、生死や候補の採否と独立した人物IDも `JAC_ASN_PERSON_V1` に保存する。人物IDとUnit ID・個体トークンの対応をPlayerの `JAC_ASN_PERSON_UNITS_V1` へ永続保存する。候補を再列挙しても対応を書き換えず、同IDの別個体を旧人物へ結び直さない。Unitを人物IDへ結び付けてから候補を絞り、死亡した人物をUnitトークンの別人物として登録し直さない。同ID・同型の別個体への置換でも対象をすり替えない。任務中に暗殺者が消失した場合は、Q76の回答どおり成功・失敗・発覚の全結果を取り消す。成否と発覚の組合せにかかわらず、固定済みの友好度-10・不平+25だけを一度適用し、全国待機を保持する。捕縛・死亡、開戦理由、第三国の非難は起こさない。物理的対象だけが消失してもペナルティは免除しない。通知例外からの回復はテスト済みだが、エンジンの全副作用とプロパティ保存をまたぐトランザクションは保証していない。

処理対象は存命国の列挙だけに依存しない。育成・任務を持った国のIDを `JAC_ASN_ACTORS_V1` に保存し、国家滅亡後も任務の終了と固定済み罰則を記録する。滅亡国の育成は取り消し、成功効果は適用しない。滅亡後のPlayer Propertyへのアクセス継続は要実機確認。再レビューで実行者消失の同じ領域を指摘されたため、Unitの存在確認だけを足す修正から、保存した仕事の所有国を追う設計へ改めた。

所在地は暫定で、指導者／Governorの都市は発見済み、Unitは視認中を必要とする。UIとGameplayの双方で検査し、不可視Unitを一覧へ出さない。未決のため `PROVISIONAL.Visibility` で切替可能にする。

`JAC_ASN_INPUT_V1` は後続コンテンツから入力するためのテーブルであり、プロパティを設定したという事実を「実際にその建物がある」「実際にその技能を解放した」という検証の代わりにしない。

| 所有者 | 入力 |
|---|---|
| Player | `Security`（0〜100、未設定50）、`Policy/Government`、`School/Governor/Hero/GreatPerson`（0〜1）、`Relations[対象IDの文字列]`（敵対度：-1が最良、1が最悪、戦争時1）、`Guard`（0〜1）、`Successor` または `Successors`（後述）、`Congress`（テスト専用）、`Regions.Unrest`（0〜100） |
| 育成する都市 | `Building/District/Trait/Job`（補正の単位数） |
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
| 世界会議の議題と投票 | `DLC/Expansion2/Scripts/WorldCongress.lua:5` の議題候補制御、`DLC/Expansion2/UI/Additions/WorldCongressPopup.lua:2242` のUI投票。任意の処遇議題を開くGameplay経路は未接続。PoCはテスト用の保存票を集計する |

PoC画像は公式スカウトのアイコンを参照するだけで、公式DDSを同梱しない。参照は `Base/Assets/UI/Icons/Icons_Units.xml:32`、`:133`、`Icons_UnitPortraits.xml:77`、`Icons_UnitFlags.xml:47`。新Unitの専用3Dモデルは未作成で、表示は要実機確認。借用理由は育成・任務の技術検証を先に行うため。完成版の固有画像の条件はSSOT15.2.1に従う。

## 検査と実機シナリオ

`tests/test_assassin_lua.py` はlupaを必須インポートし、Luaテストをskipしない。育成100／20、5速度、現在の補正による延長・短縮、読み込み後の同じ計算、2都市・2Unitの制限、全国ロック、全成功率、成否と発覚の4組合せ、暗殺者消失、四分岐、受入国・後継者の消失、偉人と指導者の二重所属防止、保存データの復元を検査する。スタブのgetter/setterはテーブルをコピーする。SQLと日英露の文言も検査する。

実機で実行するのは `tools/civ6-autotest/scenarios/assassin-leader-save-load.json`（79手順）。

1. 日本15 km・万世紀・22家、織田家を人間として開始。AI松平家への成功・未発覚・自由国家を強制し、育成と任務を同期操作で開始する。Unit生成APIの戻り値は使わず、生成前後のID差から見つける。
2. 育成期限を治安変更で100→120へ延長し、任務中にsave→メニュー→load。入力・開始・速度・期限・ロール・成否・国家の処理計画が同じことを検査する。読み込み後に治安を変えて80へ短縮する。
3. 評議会状態、全国コマンド待機、79／80ターンの育成境界を検査する。完成後のロック中にもsave→loadし、完成+9の拒否・+10の許可、別都市で19／20ターンの育成境界を検査する。
4. 別のAI国で継承・吸収・会議分岐を順に強制。偉人Unitの除去と後継者への照合、全都市の移管・全Unitの解散・旧国の実際の敗北、票を変えても採決が固定されることを検査する。会議の入力はテスト専用であり、日本に会議があるとは判定しない。
5. 成否と発覚の4組合せで任務中の暗殺者を削除し、対象が生きていること、固定の友好度-10・不平+25のみが一度適用されること、開戦理由・非難がないこと、全国待機が保たれることを検査する。
6. 最後に3回目のsave→loadで、全分岐の履歴・後継者・取消済み任務・罰則を保存前の値・適用済み任務一覧と比較する。ロード直後と次のターンの両方で二重適用がないことを検査し、強制テストモードを解除する。

後半の分岐と暗殺者消失の検査は待機期限だけをテスト用に解除し、実際の `StartAttempt/Tick` を通す。失われた指導者を蘇らせず、分岐ごとに別のAI国を使う。敵の自然な戦争・都市喪失・生成場所の混雑が検査へ干渉した場合は、失敗した時点のログと保存データを調べる。

今回の実機結果は未取得。外交罰則の本体表示、通常の都市生産キューへの統合、ネイティブ指導者交換、Governor効果の除去、正式世界会議の開催、3D表示、MP一致は未確認・未接続。都市移管後の旧枠敗北・AI停止は、実機シナリオが実状態を検査する。

共有ToDoは指定ツールの共有ロックを開く権限がなく登録できなかった。作業はこのworktreeに残し、コミット・pushは行わない。テスト実行時だけ必要なサンドボックス対応をリポジトリへ追加しない。

検査結果：リポジトリのunittest **386件成功**（暗殺者60件：Lua55件・包装5件を含む）、ドライバーのunittest **151件成功**。Luaのskipはなし。シナリオの `--check` は **79手順が有効**。

指定の直接実行では、Windowsの制限されたプロセスからPython3.12が作った一時フォルダへ入れず失敗した。ドライバーの `-t .` 指定では、開始ディレクトリにパッケージ初期化ファイルがなく探索に失敗した。Temp内の一度きりの実行スクリプトで、新しいテスト用フォルダを親のアクセス権を継承して作り、ドライバーは開始ディレクトリを最上位として探索して実行した。リポジトリにACL対応や新しい `__init__.py` は追加していない。

再実行で、ドライバーのログ待機テストが本体の起動状態を参照し、ゲームが起動していない場合にSteam起動へ進む問題を確認した。`test_safety.py` のプロセス状態を合成値へ固定し、実際のSteam起動を禁止するモックを追加した。テスト用の隔離であり、本体ドライバーの起動・終了処理は変更していない。

GPT-6.1-Solによる別モデルレビューで、死亡した人物の継承・別候補の保持・国名の維持・ロード検査の比較基準・偉人の同一性に関する指摘を修正した。最終の再レビューは残るMustなし。吸収時の同ID別個体への置換も、人物と個体トークンの永続した対応によって旧人物へ結び直さないことをLuaで確認した。
