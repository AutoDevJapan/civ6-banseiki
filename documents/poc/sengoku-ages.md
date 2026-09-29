# 戦国パックの時代（黄金期・暗黒期）第1段階（JAC_PoC_SengokuTrees）

[時代の設計](../research/sengoku-ages.md) §10 の 1（データ）・2（ゲームプレイ Lua の「武功の台帳」）・4（自動テスト）と、3（UI）のうち結果を見せる最小限（改元の通知）を実装した PoC。置き場は設計どおり `poc/JAC_PoC_SengokuTrees` で、条件は既存と同じ `RULESET_JAC_BANSEIKI`（地図では絞らない）。点・閾値・効果の数値はすべて**暫定**。

- データ：`Data/JAC_Sengoku_Ages.sql`（手書き。既存の `JAC_Sengoku_Unlocks.sql` などと同じく生成スクリプトは使わない。設計の表から作る生成物ではないため）、`Data/JAC_Sengoku_Ages_Icons.xml`、`Text/JAC_Sengoku_Ages_Text.xml`（ja・en・ru）
- ゲームプレイ Lua：`Scripts/JAC_Sengoku_Ages.lua`
- テスト：`tests/test_sengoku_ages.py`（公式データを読むデータの断面と、lupa で台帳を動かす単体テスト）
- 実機シナリオ：`tools/civ6-autotest/scenarios/sengoku-ages.json`（未実行）

## 暫定の既定（オーナー確認事項 Q36〜Q39）

[`questions.md`](../questions.md) の推奨案を**暫定**の既定にした。オーナーの判断で変わりうる。

| # | 暫定の既定 | この段階での扱い |
|---|---|---|
| Q36 | ① 元号は改元の画面と時代スコアの画面にだけ出す。画面上部の年表示は年のまま | 改元の通知の見出しにだけ元号を出す（「改元：天文から弘治へ（1555年）」）。暦の表示は変えない |
| Q37 | ① 友好宣言は点にしない | 台帳は `HasAllied` の同盟だけを数える |
| Q38 | ① 元和（1615）・寛永（1624）でも判定し、寛永の期をゲーム終了まで続ける | 改元の表は8行（弘治〜寛永） |
| Q39 | ① 家に固有の誓約1つ＋共通から2つ | 誓約はこの段階に入れない（下の「後続」）。22家分の固有誓約の中身が未設計のため |

## 動き

1. **止める**（設計 §4.2）：暗黒・黄金の基準値を −9999・9999、閾値の補正をすべて 0、`Eras_XP2.EraScoreThresholdShift` を全行 0、誓約の選択数・提示数を 0、`Moments.EraScore` を全行 0 にする。時代スコアを足す型の Modifier を取り付ける行を、`ModifierId` 列を持つ公式の全25表から消す（取り付け表は `Modifiers` を外部キーで参照しないので、`Modifiers` の行を消すと宙に浮いた ID が残る。そのため取り付け行を消す）。公式の年齢の忠誠（`TRAIT_LEADER_MAJOR_CIV` の2行）と黄金時代の宣戦を消す。`ERA_RENAISSANCE` の最長ターンを 10000 にする（§4.3）
2. **台帳の初期化**：最初のターン開始（`GameEvents.OnGameTurnStarted`）に、Game プロパティ `JAC_AGES_LEDGER` が無ければ作る。開始時の技術・社会制度（`LuaEvents.NewGameInitialized` で付与）と開始時の同盟はこの時点で済んでいるので、どちらも点にならない。各家の首都を本拠にし、全都市を「最初の持ち主＝今の持ち主」で登録する。読み込み後はプロパティがあるので初期化し直さない
3. **ターン開始の取り込み**（§5.2）：城の所有者の差分（奪取は本拠 6・旧領 3・支城 2、城が 0 になった家を滅ぼした家に +8）、同盟（外交で新しく結んだ組に 2 点ずつ。1期1回）、研究（完了数の差分 1 点ずつ）、史実イベント（`JAC_EVENT_FIRED_<EventType>` と `JAC_SengokuAgeEventScores` の点。1家1回）
4. **占領・建設のイベント**：`GameEvents.CityConquered` はその場で奪取を数え、城の記録を書き換える（直後に破却されても点と滅亡の点が残り、次のターンの差分は同じ奪取を数えない）。`GameEvents.CityBuilt` は移管でも出るので点を付けず、新しいタイルの城の登録とタイルのプロパティの書き直しだけをする
5. **改元の判定**（§5.3）：改元の月を含むターン（標準速度で 66・73・110・119・178・189・246・271）の開始時に、取り込み（改元のターンに付いた出来事のフラグを除く）→ 全勢力の判定 → 残りの出来事の取り込み、の順に処理する。閾値は `ceil((28 or 14 + 都市数 + 5×過去の黄金期 − 5×過去の暗黒期) × 期の長さ / (50 × 6))`、暗黒の下限 1・黄金の下限は暗黒 + 1（6 は台帳の 1 点あたりの公式の点。根拠は設計 §5.3）。結果は `history.r<番号>` に残し、各家の期の点を 0 に戻す
6. **効果**（§7.1）：タイルのプロパティを書く関数（毎回全都市を台帳から書き直す。冪等）が、都市の中心タイルに 0/1 の旗を書く。`JAC_AGE_GOLDEN`（黄金期・英雄期の家の全都市）、`JAC_AGE_POLICY_<名前>`（暗黒期の政策を持つ家の全都市）、`JAC_AGE_ANCHOR_<名前>`（その家の要の都市＝タイル番号が最小の都市だけ）。Modifier は `REQUIREMENT_PLOT_PROPERTY_MATCHES` でこの旗を読む
7. **通知**（§8 の最小限）：改元のとき、生存している全主要勢力に `NOTIFICATION_USER_DEFINED_2` を送る。見出しは改元（元号と年）、本文はその家の結果（黄金期・英雄期・通常期、暗黒期は割り当てた政策と効果）。LOC キーのまま渡し、各端末の言語で出る

## 設計からの差分（理由つき）

| 設計 | 実装 | 理由 |
|---|---|---|
| 共通の特性 `TRAIT_JAC_AGES` を全指導者の `LeaderTraits` に足す（§7.1） | 公式の主要文明の特性 `TRAIT_LEADER_MAJOR_CIV` の `TraitModifiers` に付ける | 全主要指導者が `LEADER_DEFAULT` から継承する特性（`Base/Assets/Gameplay/Data/Leaders.xml`、公式 DB の `LeaderTraits` は `LEADER_DEFAULT` の1行）で、公式も年齢の効果をここに付けている（`DLC/Expansion2/Data/Expansion1_Leaders.xml:138-139`）。他の MOD の指導者の読み込み順に左右されない |
| タイルのプロパティ `JAC_AGE`・`JAC_AGE_POLICY`・`JAC_AGE_ANCHOR` の値が一致するか（§7.1） | 状態ごとに別の名前の 0/1 の旗（上の「動き」6） | 公式の唯一の使用例は `PropertyName` と `PropertyMinimum` だけを渡す（`DLC/CivRoyaleScenario/Data/CivRoyaleScenario_Civilizations.xml:377-403`）。「一致」を表す引数が公式に無いので、最小値 1 の旗で表す |
| 暗黒期の専用政策は改元の画面で選ぶ。AI は家ごとの順位表（§6.2、§7.2） | 暗黒期の家は全員 `Priority` が最小の政策（背水の陣）になる。選択の受け付け（`EXECUTE_SCRIPT`）は作らない | 選択の画面が後続のため。送り手の無い受け付けだけを先に作らない |
| 本拠：`City:GetOriginalOwner()`・`IsOriginalCapital()` で最初の首都（§5.2） | 開始時の首都、無ければ（開拓者から始める家）その家が最初に建てた城 | 2つの関数は UI 側の使用例しかない（`DLC/Expansion2/UI/CityBanners/CityBannerManager.lua:1765`）。設計が書いた代わりの方法を使う |
| 海賊衆の免許：水軍の生産 +100%（公式は時代ごと・略奪船の昇進クラスごとの行）、交易路の収益 −50%（公式は産出ごとに 22 行） | 生産はカルタゴのコトンと同じ型 `MODIFIER_CITY_ADJUST_UNIT_DOMAIN_PRODUCTION`（`Domain=DOMAIN_SEA`）で全水軍、交易路は金だけ −50% | 戦国の水軍（ガレー船・四段櫂船）は略奪船の昇進クラスではなく、公式の形では効かない。行数を抑えた暫定 |
| 城の差分は全都市 | 生存している主要勢力の都市だけ | 日本マップには都市国家が無く、忠誠による離反も起きない（SSOT 3.1）。主要勢力以外が持つ城は破却と同じに見え、家が取り戻すと点なしで登録し直す（コードに `ponytail:` で記録） |
| 誓約（§7.2）、改元の画面・時代スコアの画面（§8） | 作らない | 下の「後続」 |

## 台帳の中身（Game プロパティ `JAC_AGES_LEDGER`）

キーはすべて文字列（§6.1）。

| キー | 中身 |
|---|---|
| `version`・`reign`・`reignStart` | 版（1）、今の期の番号（0 = 天文）、今の期が始まったターン |
| `houses.p<ID>` | `civ`、`score`（今の期の点）、`parts`（内訳：`home`・`regain`・`castle`・`ko`・`ally`・`research`・`event`）、`golden`・`dark`（過去の回数。英雄期は黄金期に数える）、`age`（`NONE`・`GOLDEN`・`HEROIC`・`NORMAL`・`DARK`）、`policy`、`research`（完了数）、`home`（本拠のタイルのキー） |
| `cities.plot<番号>` | `owner`・`original`（最初の持ち主）・`n`（奪取の回数） |
| `allies.<小さいID>-<大きいID>` | `none`・`ally`・`start` |
| `keys` | 数えたキー（`cap:<タイル>:<n>`・`ko:<家>`・`ally:<a>:<b>:<期>`・`evt:<EventType>:<家>`） |
| `history.r<番号>` | 判定したターン・期の長さ、家ごとの `age`・`score`・`golden`・`dark`（閾値） |

## テスト

```powershell
python -m unittest tests.test_sengoku_ages
```

- データの断面（公式の Civ6 が入っている環境だけ）：§4.2 の最終値、`ModifierId` 列を持つ全表で時代スコアの Modifier の取り付けが 0 件、公式の年齢の忠誠・暗黒/黄金の政策（実在する政策に限る）・黄金時代の宣戦が無い、改元の表が8行、効果の Modifier・要件・能力が実在し、要件が読む旗は台帳が書く名前だけ、テキストが3言語そろう
- 台帳の単体テスト（lupa。設計 §10 の 4 の一覧）：奪取の区分、キーの重複防止、同じ城を同じターンに A→B→A、A→B→A→B と奪い合ってそのつど点、閾値の式（期の長さ・都市数・過去の年齢・下限 1）と英雄期、同盟の1期1回と同じチームの 0 点、開始時の同盟（ターン32の期限切れと結び直しを台帳が先・後の両方で 0 点、プロパティが `false` の後に残る同盟も 0 点、エンジンが解いた後に外交で結べば 2 点）、出来事の点（1家1回、改元のターンのフラグは新しい期、どちらのスクリプトが先でも同じ）、読み込み後に数え直さない、占領してすぐ破却した城の点と滅亡の点、占領イベントと差分で二重に数えない、移管の `CityBuilt` で点を付けない、開拓者から始めた家の本拠、プロパティを書く関数の冪等と要の都市が家ごとに1つ、改元のターンが暦どおり、通知、実機（2026-09-29）の弘治の点と都市数の22家が黄金 3・通常 15・暗黒 4 に分かれる（旧式では全家が暗黒期）

## 実機確認（メインセッションで実行。未実行）

```powershell
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/sengoku-ages.json --check
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/sengoku-ages.json --output tools/civ6-autotest/artifacts/sengoku-ages-01
```

MOD と設定は `ages-probe.json` と同じ（15 km 史実領土版、22勢力、武田が人間）。判定は `generic`。確認はすべて GameCore_Tuner と InGame の状態で行い、`log_line` の待ちは使わない。

| 時点 | 確かめること |
|---|---|
| 開始直後 | 公式の閾値の値が上書きされ、改元の表が8行 |
| ターン2 | 台帳に22家・22の本拠。公式の時代スコアが全勢力 0（今の時代の内訳の合計と `GetPlayerCurrentScore`）、公式の年齢が無く、暗黒の閾値 ≤ 点 ＜ 黄金の閾値 |
| ターン31・34 | 開始時の同盟8組がすべて `start` で、同盟の点のキーが無い。ターン34ではターン32の結び直し（`JAC_1534_ALLIANCE_LAPSES` ≥ 1）が起きている。公式の時代スコアも再確認 |
| セーブ→メニュー→ロード | 台帳を決まった順に文字列にしたもの（セーブ前に `JAC_AGES_TEST_SNAPSHOT` へ書いた）とロード後が一致 |
| ロード後に1ターン | 各家の研究の完了数が台帳とエンジンで一致（数え直していない） |
| ターン67 | 弘治の判定が `history.r1` にターン66・期の長さ65で残る。生存する各家の結果が黄金期・通常期・暗黒期のどれか。全家が同じ年齢でなく、暗黒期が半分以下。各都市の `JAC_AGE_GOLDEN` と政策の旗が台帳と一致し、暗黒期の家の要の都市が1つ。公式の時代スコアと年齢を再確認 |

**要実機確認（シナリオでは確かめない。設計の要実機確認をそのまま引き継ぐ）：**

1. `REQUIREMENT_PLOT_PROPERTY_MATCHES` を都市の Subject・Owner の要件に使ったとき、都市の中心タイルを見るか、プロパティの変更に追従するか（設計 §7.1）。黄金期の都市の生産・金が実際に +10% になるかを画面で見る
2. `MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER` で付けた家向けの Modifier が `OwnerRequirementSetId` で入り切りするか、占領で新しい持ち主に効くか（同）
3. タイルのプロパティがセーブに入るか（同 §6.1。入らなくても次のターン開始で書き直す）
4. 通知の見た目（アイコンは公式の誓約の通知と同じ絵）と、22勢力分の通知が AI の手番に影響しないか
5. 約200ターンの黄金期・通常期・暗黒期の割合（数値を決めるため。設計 §10 の 5）。滅んだ家・開拓者から始める設定

## 後続

1. 暗黒期の専用政策を選ぶ画面と、`UI.RequestPlayerOperation(..., PlayerOperations.EXECUTE_SCRIPT, ...)` の受け付け（`DLC/BlackDeathScenario/Scripts/BlackDeathScenario_StateUtils.lua:64-74`）。AI の家ごとの順位表
2. 誓約（Q39 ①の暫定。家に固有1つ＋共通から2つ）と、誓約の達成の点
3. 改元の画面・時代スコアの画面（§8）。家固有名の差し替え（`poc/JAC_PoC_BanseikiRuleset/UI/JAC_Era*.lua`）に足す。公式の時代の画面から年齢の部分を外す
4. 数値の調整（設計 §5.3、§10 の 5）

## 公式の根拠

| 事柄 | 公式ファイル |
|---|---|
| 公式の判定と閾値 | `DLC/Expansion2/UI/Additions/EraProgressPanel.lua:117-133`、`DLC/Expansion2/Data/Expansion2_GlobalParameters.xml:62-66,126,248-264`、`DLC/Expansion2/Data/Expansion2_Eras.xml:65` |
| 誓約を止める先例 | `DLC/Byzantium_Gaul/Data/Byzantium_Gaul_DramaticAges_MODE.xml:6-18`。モードの読み込み順（LoadOrder 無し＝0）：`DLC/Byzantium_Gaul/Byzantium_Gaul.modinfo:99-104` |
| 時代スコアの Modifier の取り付け | `DLC/Expansion2/Data/Expansion1_Moments.xml`・`Expansion2_Moments.xml:149`（誓約）、`DLC/Expansion1/Data/Expansion1_Buildings_Major.xml:78`（タージ・マハル）、`Byzantium_Gaul_DramaticAges_MODE.xml:96-100` |
| 年齢の忠誠・黄金時代の宣戦 | `DLC/Expansion2/Data/Expansion1_Leaders.xml:138-139`、`Expansion1_DiplomaticActions.xml:5` |
| 時代の長さ | `DLC/Expansion2/Data/Expansion1_Eras.xml:3-13` |
| タイルのプロパティの要件と書き込み | `DLC/CivRoyaleScenario/Data/CivRoyaleScenario_Civilizations.xml:377-403`、`DLC/CivRoyaleScenario/Scripts/CivRoyaleScenario_StartScript.lua:496-519` |
| 暗黒政策の型と値 | `DLC/Expansion2/Data/Expansion1_Policies.xml:274-290,1609-1627,1657-1761`（寺院学問・背水の陣・海賊衆の免許・精鋭主義の元） |
| 水軍の生産の型 | カルタゴのコトン `COTHON_NAVAL_UNIT_PRODUCTION`（公式 DB。`MODIFIER_CITY_ADJUST_UNIT_DOMAIN_PRODUCTION`、`Domain=DOMAIN_SEA`） |
| 都市全体へ付ける型 | `Base/Assets/Gameplay/Data/Modifiers.xml:780`（`MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER`） |
| イベント | `DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:246`（ターン開始）、`DLC/NubiaScenario/Scripts/NubiaScenario.lua:937-969`（占領。イベントの時点で城は奪った側の一覧にある）、`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:56,255`（建設） |
| ゲームプレイ側の呼び出し | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:136`（`GetCities():Members()`）、`:1103`（`GetCapitalCity`）、`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:1198`（`GetCities():GetCount()`）、`DLC/CivRoyaleScenario/Scripts/CivRoyaleScenario_UnitCommands.lua:157`（`GetTeam`） |
| 通知とアイコン | `Base/Assets/Gameplay/Data/Notifications.xml:52-60`（`NOTIFICATION_USER_DEFINED_2`）、`DLC/Expansion2/Data/Expansion1_Icons_Notifications.xml:36`（誓約の通知の絵）。`USER_DEFINED_1` は史実の出来事が使う |
