# 戦国パックの時代（黄金期・暗黒期）第1段階（JAC_PoC_SengokuTrees）

[時代の設計](../research/sengoku-ages.md) §10 の 1（データ）・2（ゲームプレイ Lua の「武功の台帳」）・3（UI：改元の画面と時代スコアの画面。下の「改元の画面と時代スコアの画面」）・4（自動テスト）を実装した PoC。置き場は設計どおり `poc/JAC_PoC_SengokuTrees` で、条件は既存と同じ `RULESET_JAC_BANSEIKI`（地図では絞らない。戦国パックの他の仕組みと同じくルールセット全体に効く。SSOT 3.2 は時代を戦国パックの仕組みとして書いており、地図の条件は無い）。点・閾値・効果の数値はすべて**暫定**。

- データ：`Data/JAC_Sengoku_Ages.sql`（手書き。既存の `JAC_Sengoku_Unlocks.sql` などと同じく生成スクリプトは使わない。設計の表から作る生成物ではないため）、`Data/JAC_Sengoku_Ages_Icons.xml`、`Text/JAC_Sengoku_Ages_Text.xml`（ja・en・ru）。改元の表の行と元号の名前・見出しだけは生成物：`Data/JAC_Sengoku_Reigns.sql`・`Text/JAC_Sengoku_Reigns_Text.xml`（`python tools/generate_sengoku_reigns.py`、元は `documents/spec/data/sengoku-reigns.json`。`--check` で古いと失敗する）
- ゲームプレイ Lua：`Scripts/JAC_Sengoku_Ages.lua`
- UI Lua：`UI/JAC_SengokuDarkPolicy.lua`・`.xml`（暗黒期の政策の選択画面。下の「暗黒期の政策の選択」）、`UI/JAC_SengokuReignPopup.lua`・`.xml`（改元の画面）、`UI/JAC_EraProgressPanel_Sengoku.lua`（時代スコアの画面。公式の画面への差分）、`UI/JAC_SengokuAgesView.lua`（画面が共有する台帳の読み方と文の組み立て）、`UI/JAC_ActionPanel_Sengoku.lua`（右下の時代の目盛りの説明を元号にする。下の「時代の名前（元号）」）
- テスト：`tests/test_sengoku_ages.py`（公式データを読むデータの断面と、lupa で台帳と選択画面を動かす単体テスト）、`tests/test_sengoku_ages_screens.py`（lupa で改元の画面と時代スコアの画面を台帳の上で動かす単体テスト）
- 実機シナリオ：`tools/civ6-autotest/scenarios/sengoku-ages.json`（2026-09-30 に PASS。後から足した確認は未実行。下の「実機確認」）、`sengoku-dark-policy.json`（暗黒期の政策の選択。以前の版は2026-09-30 に PASS。今回の通知と受付期間は未実行）、`sengoku-ages-screens.json`（改元の画面・時代の目盛りの説明・時代スコアの画面。未実行）

## オーナー判断 L（2026-09-30、旧 Q36〜Q39）

暫定の既定だった Q36〜Q39 は、オーナー判断 L で決まった（SSOT 3.2 に反映）。

| # | 判断 | この段階での扱い |
|---|---|---|
| L-1（旧 Q36） | 通常「XX時代」と出る所を今の元号にし、「弘治の世」「Kōji Era」「Эпоха Кодзи」と出す | 右下の時代の目盛りの説明、時代スコアの画面の時代の名前、改元の通知の見出し（「改元：弘治の世」）を元号にした。改元の画面の見出しは前から元号。ツリーの段階はオーナー判断 T（元号＝時代）で元号そのものになった（[元号＝時代](era-equals-reign.md)）。下の「時代の名前（元号）」 |
| L-2（旧 Q37） | ゲーム中に新しく結んだ友好宣言も点にする（1点、同じ組は1期1回。開始時の関係は除く） | 同盟の区分（`parts.ally`）に数える。下の「友好宣言の点」 |
| L-3（旧 Q38） | 慶長の後も史実の元号で令和まで改元し、令和を100年続けた後は「遅代」で全ての家を暗黒期にする | 改元の表は47行（弘治〜令和の46と遅代）。下の「改元の表（令和・遅代まで）」 |
| L-4（旧 Q39） | 誓約は各家に固有のものを3つずつ作り、共通の誓約は置かない | 誓約はこの段階に入れない（下の「後続」）。66の中身が未設計のため |

## 時代の名前（元号、L-1）

1. **右下の時代の目盛り**（`UI/JAC_ActionPanel_Sengoku.lua`）：公式の説明は今の時代の名前と「時代の終わりまでのターン」（`DLC/Expansion2/UI/Replacements/ActionPanel_Expansion1.lua:187-216`。名前は `Base/Assets/UI/ActionPanel.lua:1161-1168` が `GameInfo.Eras` の `Name` から作る）。戦国パックではツリーの段階の名前（「統一への広域戦」など）になっていた。台帳がある間は、説明を「弘治の世」と「次の改元まで：Nターン」（遅代の後はターンなし）に替える。目盛りの向き（公式の時代の段階）は変えない。Gathering Storm の `ActionPanel` の差し替え（`Expansion2.modinfo:366-371`、LoadOrder なし）の後に効くよう、`ReplaceUIScript`（LoadOrder 100）で `ActionPanel_Expansion2` を読んでから `RealizeEraIndicator` を包む。手番の始まりにも書き直す。他の MOD が `ActionPanel` を差し替えると競合する（要確認）
2. **時代スコアの画面**：時代の名前の欄を「弘治の世（1555年〜）」にした（文だけの変更。仕組みは下の「改元の画面と時代スコアの画面」）
3. **改元の通知**：見出しを改元ごとのキー（`JAC_SengokuReigns.Notification`、「改元：弘治の世」「New era name: Kōji Era」）にした。通知には書式の引数を渡せない（各端末の言語で出すため LOC キーのまま渡す）ので、改元ごとに生成する
4. 公式の UI で今の時代の名前を出す所は、ほかに時代の終わりの予告（`NewTurnPanel_Expansion1.lua:11-24`）と公式の時代の画面（`Base/Assets/UI/Popups/EraCompletePopup.lua:55`、`DLC/Expansion2/UI/Additions/EraReviewPopup.lua:62`、`DedicationPopup.lua:53`）。オーナー判断 T（元号＝時代）で時代が元号そのものになり、これらも公式の時代の名前（`Eras.Name`）から元号の名前を出す。公式の新しい時代の画面は出さない（[元号＝時代](era-equals-reign.md)）

## 改元の表（令和・遅代まで、L-3）

1. 表の行：`documents/spec/data/sengoku-reigns.json`（出典は Wikipedia「Japanese era name」の一覧。年と月）から生成する。弘治（1555-11）〜令和（2019-05）の46と、遅代（2119-05、令和の100年後）。遅代の行は `ForcedAge = 'DARK'`
2. 各速度の判定のターン（生成した SQL の各行の末尾にもある）。暦は変えない：

| 改元 | オンライン | クイック | 標準 | エピック | マラソン |
|---|---:|---:|---:|---:|---:|
| 弘治（1555） | 33 | 44 | 66 | 88 | 132 |
| 慶長（1596） | 95 | 126 | 189 | 278 | 556 |
| 寛永（1624） | 136 | 181 | 271 | 442 | 884 |
| 明治（1868） | 503 | 670 | 1005 | 1909 | 3818 |
| 令和（2019） | 729 | 971 | 1457 | 2813 | 5625 |
| 遅代（2119） | 879 | 1171 | 1757 | 3413 | 6825 |

3. 遅代の判定：点・閾値は記録するが、年齢は点によらず全ての家が暗黒期（暗黒期の回数に数え、暗黒期の家でも英雄期にならない）。既定の政策が付き、人間の家は通常どおり1回選べる。次の改元は無いので、時代スコアの画面は残りターンと閾値を隠し、目盛りの説明もターンを出さない。令和の期の間、時代スコアの画面の見込みは点によらず暗黒期になる（`next.forced`）
4. 同じターンの改元（長さ0の期）：同じターンに入る改元は、そのうち最後のものにまとめて1回だけ判定する（間の改元は判定・通知・画面を出さない。`NextJudged`）。判定の結果には締めた期の番号 `from` を残し、改元の画面の「〇〇の期の武功」はその期の元号を出す。今の表と暦では、どの速度でも同じターンに入る改元は無い（一番短い万延の期がオンラインで1ターン）
5. 遅代まで進むのは、標準で約1757ターン（戦国パックの目安の約200ターンの9倍近く）続いたゲームだけ

## 友好宣言の点（L-2）

1. 台帳は組ごとに、同盟（`allies`）とは別に友好宣言の状態（`friends`：`none`・`on`・`start`）を持つ。数え方は同盟と同じ：組の `JAC_1534_ALLIANCE_*` が数値で友好宣言が続いている間は `start`（点にしない。プロパティが `false` になった後に残る友好宣言も点にしない）、前が `none` で今結んでいれば 1 点ずつ（キー `friend:<a>:<b>:<期>`、同じ組は1期1回）、同じチームは点にしない、台帳の初期化のときに結んでいる組は点にしない
2. 点は同盟の区分（`parts.ally`。画面の内訳は「同盟・友好宣言」）。友好宣言のあと同盟を結んだ組は 1 + 2 点
3. 判定はゲームプレイ側の `HasDeclaredFriendship`。DLL に名前はある（`DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll` の文字列 `lHasDeclaredFriendship`）が公式の使用例が無く、同じ系統の `GetDeclaredFriendshipTurn` はゲームプレイ側で nil だった（[関係の実装メモ](japan-1534-relations.md)）。無い場合は友好宣言を点にせず、Lua.log に `[JAC_AGES] no gameplay HasDeclaredFriendship: friendships are not scored` を1回出す。**要実機確認**：この行が出ないこと、AI が友好宣言を結んだ組に点が付くこと
4. 保存済みの台帳（`friends` の無い版）は、読み込んだ後の最初のターン開始で `friends` を作り、その時点で結んでいる組は点にしない

## 動き

1. **止める**（設計 §4.2）：暗黒・黄金の基準値を −9999・9999、閾値の補正をすべて 0、`Eras_XP2.EraScoreThresholdShift` を全行 0、誓約の選択数・提示数を 0、`Moments.EraScore` を全行 0 にする。時代スコアを足す型の Modifier を取り付ける行を、`ModifierId` 列を持つ公式の全25表から消す（取り付け表は `Modifiers` を外部キーで参照しないので、`Modifiers` の行を消すと宙に浮いた ID が残る。そのため取り付け行を消す）。公式の年齢の忠誠（`TRAIT_LEADER_MAJOR_CIV` の2行）と黄金時代の宣戦を消す。`ERA_RENAISSANCE` の最長ターンを 10000 にする（§4.3）
2. **台帳の初期化**：最初のターン開始（`GameEvents.OnGameTurnStarted`）に、Game プロパティ `JAC_AGES_LEDGER` が無ければ作る。開始時の技術・社会制度（`LuaEvents.NewGameInitialized` で付与）と開始時の同盟はこの時点で済んでいるので、どちらも点にならない。各家の首都を本拠にし、全都市を「最初の持ち主＝今の持ち主」で登録する。読み込み後はプロパティがあるので初期化し直さない
3. **ターン開始の取り込み**（§5.2）：城の所有者の差分（全プレイヤーの城を見る。通常の Gathering Storm 地図（SSOT 3.1）で城が都市国家・自由都市に移っても記録と履歴を残し、点は家（主要勢力）だけに付く。記録を消すのはどのプレイヤーの一覧にも無い＝破却された城だけ）（奪取は本拠 6・旧領 3・支城 2、城が 0 になった家を滅ぼした家に +8）、同盟（外交で新しく結んだ組に 2 点ずつ。1期1回）、研究（完了数の差分 1 点ずつ）、史実イベント（`JAC_EVENT_FIRED_<EventType>` と `JAC_SengokuAgeEventScores` の点。1家1回）
4. **占領・建設のイベント**：`GameEvents.CityConquered` はその場で奪取を数え、城の記録を書き換える（直後に破却されても点と滅亡の点が残り、次のターンの差分は同じ奪取を数えない）。`GameEvents.CityBuilt` は移管でも出るので点を付けず、新しいタイルの城の登録とタイルのプロパティの書き直しだけをする
5. **改元の判定**（§5.3）：改元の月を含むターン（標準速度で 66・73・110・119・178・189・246・271…1457（令和）・1757（遅代））の開始時に、取り込み（改元のターンに付いた出来事のフラグを除く）→ 全勢力の判定 → 残りの出来事の取り込み、の順に処理する。閾値は `(28 or 14 + 都市数 + 5×過去の黄金期 − 5×過去の暗黒期) × 期の長さ / (50 × 6)` を、黄金は切り上げて下限 2、暗黒は切り捨てて下限 0（6 は台帳の 1 点あたりの公式の点。根拠は設計 §5.3）。暗黒を切り捨てるのは、短い期（7〜11 ターン）の点が 0〜1 で、旧式（切り上げ・下限 1）では点 0 の家がすべて暗黒期になったため（下の「約200ターンの割合」）。結果は `history.r<番号>` に残し、各家の期の点を 0 に戻す
6. **効果**（§7.1）：タイルのプロパティを書く関数（毎回全都市を台帳から書き直す。冪等）が、都市の中心タイルに 0/1 の旗を書く。`JAC_AGE_GOLDEN`（黄金期・英雄期の家の全都市）、`JAC_AGE_POLICY_<名前>`（暗黒期の政策を持つ家の全都市）、`JAC_AGE_ANCHOR_<名前>`（その家の要の都市＝タイル番号が最小の都市だけ）。Modifier は `REQUIREMENT_PLOT_PROPERTY_MATCHES` でこの旗を読む
7. **通知**：改元のとき、人間の暗黒期の家に公式の終了ブロックを持つ `NOTIFICATION_COMMEMORATION_AVAILABLE`、その他の生存している主要勢力に `NOTIFICATION_USER_DEFINED_2` を送る。見出しは「改元：弘治の世」（改元ごとのキー。L-1）、本文はその家の結果（黄金期・英雄期・通常期、暗黒期は割り当てた政策と効果）。LOC キーのまま渡し、各端末の言語で出る。人間への主な見せ方は改元の画面（下の「改元の画面と時代スコアの画面」）で、通知は残す
8. **時代スコアの画面のための見込み**：ターン開始のたびに、次の改元のターン（`nextDue`）と、各家の今の期の閾値（`houses.p<ID>.next`。判定と同じ式に、その時点の都市数と期の長さを入れたもの）を台帳に書く。UI は式を持たず、これを読むだけ。次の改元が遅代なら見込みに `forced = "DARK"` を付ける。遅代の後（次の改元が無い）は両方とも無し

## 設計からの差分（理由つき）

| 設計 | 実装 | 理由 |
|---|---|---|
| 共通の特性 `TRAIT_JAC_AGES` を全指導者の `LeaderTraits` に足す（§7.1） | 公式の主要文明の特性 `TRAIT_LEADER_MAJOR_CIV` の `TraitModifiers` に付ける | 全主要指導者が `LEADER_DEFAULT` から継承する特性（`Base/Assets/Gameplay/Data/Leaders.xml`、公式 DB の `LeaderTraits` は `LEADER_DEFAULT` の1行）で、公式も年齢の効果をここに付けている（`DLC/Expansion2/Data/Expansion1_Leaders.xml:138-139`）。他の MOD の指導者の読み込み順に左右されない |
| タイルのプロパティ `JAC_AGE`・`JAC_AGE_POLICY`・`JAC_AGE_ANCHOR` の値が一致するか（§7.1） | 状態ごとに別の名前の 0/1 の旗（上の「動き」6） | 公式の唯一の使用例は `PropertyName` と `PropertyMinimum` だけを渡す（`DLC/CivRoyaleScenario/Data/CivRoyaleScenario_Civilizations.xml:377-403`）。「一致」を表す引数が公式に無いので、最小値 1 の旗で表す |
| 暗黒期の専用政策は改元の画面で選ぶ。AI は家ごとの順位表（§6.2、§7.2） | 改元の通知とは別の選択画面で選ぶ。家ごとの表は順位表の1位（既定の政策）だけを持つ（下の「暗黒期の政策の選択」） | 改元の画面（§8）は後続3。どの政策もいつでも選べるので、2位以下は使う場面が無い |
| 本拠：`City:GetOriginalOwner()`・`IsOriginalCapital()` で最初の首都（§5.2） | 開始時の首都、無ければ（開拓者から始める家）その家が最初に建てた城 | 2つの関数は UI 側の使用例しかない（`DLC/Expansion2/UI/CityBanners/CityBannerManager.lua:1765`）。設計が書いた代わりの方法を使う |
| 海賊衆の免許：水軍の生産 +100%（公式は時代ごと・略奪船の昇進クラスごとの行）、交易路の収益 −50%（公式は産出ごとに 22 行） | 生産はカルタゴのコトンと同じ型 `MODIFIER_CITY_ADJUST_UNIT_DOMAIN_PRODUCTION`（`Domain=DOMAIN_SEA`）で全水軍、交易路は金だけ −50% | 戦国の水軍（ガレー船・四段櫂船）は略奪船の昇進クラスではなく、公式の形では効かない。行数を抑えた暫定 |
| 誓約（§7.2） | 作らない | 下の「後続」 |
| 改元の画面は公式の時代の画面（`EraCompletePopup`／`EraReviewPopup`）と同じ作り（§8） | 絵と組み立ては `EraReviewPopup` と同じだが、`UIManager:QueuePopup` と入力の独占はしない（`ContextPtr:SetHide` の出し入れ、Esc だけ受ける） | 公式の出し方は自動進行や他の画面を止めうる。暗黒期の政策の選択画面と同じ方式にした |
| 時代スコアの画面に「本拠陥落、支城の占領、同盟など」の内訳と閾値（§8） | 公式の `EraProgressPanel` の欄を差し替える。内訳と過去の改元は下段（公式の「この時代の効果」の欄）に文で出す | 公式の画面は XML に欄を足せない（`ReplaceUIScript` は Lua だけを差し替える）。閾値の行の見出し（「暗黒時代」など）は ID の無い XML の文字列で、公式の呼び名のまま |
| 公式の時代が進んだときの画面から年齢の部分を外す（§8） | 時代スコアの画面だけ外す。`EraCompletePopup` と `EraReviewPopup` は変えない | `EraCompletePopup` は年齢が無い（通常）ときの枠を出すだけで年齢の文字は無い（`EraCompletePopup.lua:107-112`）。`EraReviewPopup` を開くのは献身の通知だけ（`NotificationPanel_Expansion1.lua:53-65`）で、献身は止めてある（§4.2）。出ることがあれば要対応 |

## 暗黒期の政策の選択（後続1、[設計](../research/sengoku-ages.md) §6.2・§7.2）

人間の家は改元のターンだけ1回選び、選ぶまで手番を終えられない。画面は改元のときに1回だけ出し、閉じた後は同じターンの改元の通知から開き直す（SSOT 3.2、2026-09-30 の回答）。

1. **家ごとの政策**：改元で暗黒期になった家には `JAC_SengokuDarkAgePolicyDefaults` の政策をすぐ適用する。表に無い文明は最小の `Priority`（背水の陣）。AI の家はこのまま、人間の家は選択が受理されるまでこの効果が続く。割り当ては SSOT 3.2 のとおりで、変更しない。
2. **画面**：改元の画面を閉じたときに政策の4候補を出す。自動表示は1つの改元に1回だけ。閉じた後の手番開始や読み込みでは自動表示しない。同じターンの改元の通知か、通知を起動する通常のターン終了ボタンから開き直せる。読み込みでも通知と判定のターンを保ち、受付期間を延ばさない。
3. **送信と受付**：画面は自分の手番が始まっているときだけ `EXECUTE_SCRIPT` で要求する（手番前の要求は落ちる。実機 court-arbitration-03）。Gameplay はエンジンが渡す送信者を使い、人間の家・暗黒期・今の期・保存した `history.r<期>.turn` が現在ターンと一致・未選択・有効な候補を確認する。受理したら政策と `chosen`、都市の旗を保存する。次のターンは、古い未選択セーブであっても拒否する。
4. **ターン終了を止める通知**：人間の暗黒期の家に、公式の誓約選択と同じ `COMMEMORATION_AVAILABLE` を送る。独自のプロパティ `JAC_DARK_POLICY_REIGN` に期を入れ、`NotificationParameterTypes.CanUserDismiss = false` にする。その他の家は従来の `USER_DEFINED_2`。送る前に台帳を保存する。UI の `JAC_NotificationPanel_Sengoku.lua` は公式パネルを読み込んだ後でこの印のある通知だけを政策画面につなぐ。通常の公式通知の処理は元のまま。
5. **解除**：同期済み台帳で選択の受理を確認した後、UI がこのMODの通知だけを消す。画面を閉じたり、拒否された要求を送ったりしても消さない。通知の全IDを調べるので、同じ型の公式通知があっても消し漏れない。古いターン・古い期の通知も消す。
6. **保存**：政策、選択済みの期、判定のターン、都市の旗は従来の台帳・タイルのプロパティを使う。読み込みで再初期化しない。改元の次のターンには選択・変更できない。次の改元で暗黒期なら、その家の政策を適用し、新しい選択を受け付ける。

以前の実装は 2026-09-30 の ages-dark-choice-01 で PASS（寺院学問の選択、都市の旗、2回目の拒否、保存・読み込み）。今回の受付期間・通知・終了ブロックは**実機未実行**。Lua から送った公式の通知型にエンジンのブロックが付くことは、単体テストの模擬APIだけでは証明できない。

実機シナリオは `sengoku-dark-policy.json`。ターン65まで自動進行し、テスト用に武田の点を −1000 にしてから、通常の手番終了で弘治の改元を迎える。自動進行中の人間の家をAIとして判定する経路では、終了ブロックを検証できないためである。

| 時点 | 検査する動き |
|---|---|
| ターン66 | 武田が暗黒期で精鋭主義、暗黒期のAIの家も各家の政策、全城の旗と1つの要の都市 |
| 改元の画面を閉じる | 政策画面が自動で1回だけ出る。閉じても繰り返し自動表示しない |
| 政策を選ぶ前 | 通知が `ENDTURN_BLOCKING_COMMEMORATION_AVAILABLE` を持つ。エンジンのブロック一覧にあり、`UI.CanEndTurn()` は false。通知は手動で消せない |
| 通知・ターン終了ボタン | 実際の通知の `Activate(true)` と通常の `DoEndTurn()` で画面を開き直し、ターン66から進まない |
| 未選択のまま保存・読み込み | 画面は自動で出ず、保存した通知から開き直せる。終了ブロックは続く |
| 寺院学問を選ぶ | 台帳と全城の旗が替わり、選択の通知と専用の終了ブロックが消える。2回目の要求は拒否される |
| ターン67 | 画面を開けず変更できない。テスト用に未選択の古いセーブを模した状態でも、遅い要求は Gameplay が拒否する |
| 選択済みで保存・読み込み | 寺院学問・選択済みの印・旗が残り、政策の通知も画面も出ない |

**要実機確認**：上の通知ブロックとその保存・解除、三言語の本文の折り返し、マルチプレイ。通知型で止まらなければシナリオが失敗する。ボタンを無効にするだけの代替動作へは切り替えず、公式型で止められない場合はオーナーの判断を求める。

## 改元の画面と時代スコアの画面（後続3、[設計](../research/sengoku-ages.md) §8）

どちらも台帳（Game プロパティ）を読むだけで、ゲームの状態を書かない。時代の名前は元号（L-1。上の「時代の名前（元号）」）。家固有名の差し替え（`poc/JAC_PoC_BanseikiRuleset/UI/JAC_HouseNames.lua`）は両方の画面に効く（時代スコアの画面は公式のコンテキストに取り付け済みの包みをそのまま使い、改元の画面は自分で `JAC_InstallHouseNames` を呼ぶ）。

1. **改元の画面**（`UI/JAC_SengokuReignPopup.lua`・`.xml`）：改元のターンに、自分の家の判定を出す。見出しは「改元」「天文から弘治へ（1555年）」の2行（公式 `LOC_ERA_REVIEW_POPUP_TITLE` と同じく `[NEWLINE]` で分ける。1行では英語・ロシア語が枠を超えて切れた）、帯はその家の年齢（黄金期・英雄期・通常期・暗黒期とアイコン）、「天文の期の武功：N点」と閾値（黄金期は何点以上、暗黒期は何点未満）、年齢の効果（暗黒期は割り当てた政策の効果）。絵と組み立ては公式の時代の振り返り `EraReviewPopup` と同じで、年齢ごとに羊皮紙・枠・帯（`Ages_Parchment*`・`Ages_Frame*`・`Ages_BannerLong*`）を替え、英雄期は光（`Ages_FrameGlow`）を出す
2. **出す時機**：自分の手番の始まり（`Events.LocalPlayerTurnBegin`。公式の `EraCompletePopup` と同じ）と自動進行の終わり（`LuaEvents.AutoPlayEnd`）に、今の期の判定のターンが今のターンなら出す。1つの改元はこのセッションで1回だけ。読み込み直後には出さない（公式の `EraCompletePopup` と同じ）。手番の終わりに閉じる
3. **暗黒期の政策の選択との順番**：改元の画面が出ている間は選択画面を出さない。閉じると（選択待ちなら）選択画面が出る。暗黒期で選択待ちの家には、閉じるボタンが「暗黒期の政策を選ぶ」になる（`LuaEvents.JAC_SengokuReignPopup_Shown`・`_Closed`）。手番の終わりで閉じたときは選択画面を出さない（手番の外では送れない）
4. **時代スコアの画面**（`UI/JAC_EraProgressPanel_Sengoku.lua`）：入口は公式のまま（画面上部の時代のボタン、`PartialScreenHooks_Expansion1.lua:40-56`）。家固有名の差し替え `JAC_PoC_BanseikiRuleset/UI/JAC_EraProgressPanel.lua` が公式の `EraProgressPanel` を読んだ後に `include("JAC_EraProgressPanel_", true)` で読む（別の差し替えを作らない。§8）。台帳がある間は、公式の `Refresh` の後で次の欄を台帳の中身に置き換える：題（「武功と改元」。英語・ロシア語は Merit and Eras・Заслуги и эпохи。枠の題は公式の「Era Progress」「Ход эпохи」程度の長さしか入らず、Merit and Era Names は切れた）、時代の名前（「弘治の世（1555年〜）」）、次の改元までのターン、今の期の武功、自分の家の年齢（帯）、他の家の年齢（公式の一覧を、その呼び出しの間だけ `Game.GetEras()` を台帳の年齢に替えて作り直す。公式の年齢は止めてあり、そのままでは全家が通常になる。家紋は公式 `CivilizationIcon.lua:42-65` が `ICON_` + 文明種別を36pxで引く。家紋の画像が BLP に無いと「?」になる：実機 ages-screens-01 は、MOD を置いた作業フォルダの `JAC_CustomCivs_Icons.blp` が家紋（#54）より前に作られていたため全家が「?」だった。`tools/civ6-autotest/scenario.py` は実行前に、有効な MOD のアイコンアトラスの画像がその MOD の BLP に全部あるかを確かめ、無ければ `python tools/jac_art/build_art.py` での作り直しを求めて止まる）、次の改元の閾値と今の見込み（チェックの印。暗黒期の家は黄金の行が英雄期になる）、下段（年齢の効果、今の期の内訳：他家の本拠の奪取・旧領の奪還・城の奪取・家を滅ぼす・同盟・技術と社会制度・史実の出来事、これまでの改元：元号と年・年齢・武功・閾値）。遅代の後は次の改元までのターンと閾値を隠す
5. **自動テストの driver**：`scenario.py` の `POPUP_CLOSERS` に `JAC_SengokuReignPopup` の `OnClose()` を足した（選択画面より先に閉じる。閉じると選択画面が出るので、同じ回に続けて閉じる）。どちらの画面も入力を奪わないので、閉じなくても autoplay は止まらない
6. 自動テスト用に、改元の画面は最後に出した改元を `JAC_REIGN_POPUP_LAST`（閉じた後も残る）に、時代スコアの画面は最後に出した中身を `JAC_AGES_PANEL_VIEW` に残す。`JAC_ShowReignPopup(true)` は最後の改元を出し直す

実機シナリオ `sengoku-ages-screens.json`（設定は `sengoku-dark-policy.json` と同じ。15 km 史実領土版、22勢力、武田が人間。改元直前に武田の点を −1000 にするテスト用の操作で暗黒期にし、通常の手番終了で改元する）。**未実行**：

| 時点 | 確かめること |
|---|---|
| ターン66（64ターンの autoplay と通常の手番終了の後） | 弘治の判定がターン66、次の改元が73、武田の閾値の見込みがある |
| 同上 | 改元の画面が弘治・ターン66・台帳どおりの年齢と点で出た（autoplay の途中で driver が閉じていれば出し直す）。見出し・帯・武功の文・ボタン（暗黒期で選択待ちなら「暗黒期の政策を選ぶ」）。画面を撮る |
| 同上（`ActionPanel`） | 右下の時代の目盛りの説明が「弘治の世」と「次の改元まで：7ターン」（`JAC_ERA_INDICATOR_TEXT`。L-1） |
| 同上 | 改元の画面を閉じると政策の画面が1回出る。閉じても自動再表示せず、エンジンの通知ブロックが続く。通知から開き直して寺院学問を選ぶと専用のブロックが消える |
| ターン67 | 選択画面を開けず、政策を変えられない |
| 時代スコアの画面 | 上部の時代のボタンと同じ `LuaEvents.PartialScreenHooks_Expansion1_ToggleEraProgress()` で開く。期の名前・武功・次の改元までのターン（7）・帯・閾値・内訳の7行・弘治の行が台帳と一致。画面を撮る。閉じる |

**要実機確認**：autoplay の終わりに `LuaEvents.AutoPlayEnd` か `LocalPlayerTurnBegin` が出て改元の画面が開くか（シナリオの `JAC_REIGN_POPUP_LAST` で分かる）。`EraProgressPanel`・`ActionPanel` の状態名で FireTuner から触れるか。目盛りの説明の見た目（マウスを乗せたときの文）は人が見る。画面の見た目（文の長さ・折り返し、下段のスクロール、帯の文字色）は人が見る。

## 台帳の中身（Game プロパティ `JAC_AGES_LEDGER`）

キーはすべて文字列（§6.1）。

| キー | 中身 |
|---|---|
| `version`・`reign`・`reignStart` | 版（1）、今の期の番号（0 = 天文）、今の期が始まったターン |
| `nextDue` | 次の改元のターン（遅代の後は無し）。時代スコアの画面と目盛りの説明用 |
| `houses.p<ID>` | `civ`、`score`（今の期の点）、`parts`（内訳：`home`・`regain`・`castle`・`ko`・`ally`・`research`・`event`）、`golden`・`dark`（過去の回数。英雄期は黄金期に数える）、`age`（`NONE`・`GOLDEN`・`HEROIC`・`NORMAL`・`DARK`）、`policy`、`chosen`（暗黒期の政策を選んだ期の番号。選ぶまで無し）、`research`（完了数）、`home`（本拠のタイルのキー）、`next`（今の期の `golden`・`dark` の閾値の見込みと、次の改元が遅代なら `forced`。時代スコアの画面用） |
| `cities.plot<番号>` | `owner`・`original`（最初の持ち主）・`n`（奪取の回数） |
| `allies.<小さいID>-<大きいID>` | `none`・`on`・`start`（古い台帳の `ally` は `on` と同じ扱い） |
| `friends.<小さいID>-<大きいID>` | 友好宣言の `none`・`on`・`start`（L-2） |
| `keys` | 数えたキー（`cap:<タイル>:<n>`・`ko:<家>`・`ally:<a>:<b>:<期>`・`friend:<a>:<b>:<期>`・`evt:<EventType>:<家>`） |
| `history.r<番号>` | 判定したターン・期の長さ・締めた期の番号 `from`・遅代なら `forced`、家ごとの `age`・`score`・`golden`・`dark`（閾値） |

## テスト

```powershell
python -m unittest tests.test_sengoku_ages
```

- データの断面（公式の Civ6 が入っている環境だけ）：§4.2 の最終値、`ModifierId` 列を持つ全表で時代スコアの Modifier の取り付けが 0 件、公式の年齢の忠誠・暗黒/黄金の政策（実在する政策に限る）・黄金時代の宣戦が無い、改元の表が JSON の47行と同じ順・年月、効果の Modifier・要件・能力が実在し、要件が読む旗は台帳が書く名前だけ、テキストが3言語そろう
- 暗黒期の政策の選択（lupa）：家ごとの既定と表に無い文明の既定、選択で旗が切り替わり要の都市が1つ、同じ期の2回目・暗黒期でない家・表に無い政策・前の期の画面・台帳に無いプレイヤー・引数なしを捨てる、判定前の選択を捨てる、次の期にまた既定に戻り選べる、読み込み後も選択が残る。画面（UI Lua を同じ Lua に読み込む）：判定前は出ない、判定後に4行と今の政策、「選ぶ」で要求が届いて台帳と旗が変わり画面が閉じる、選んだ後は出ない（ネイティブの通知ブロックは模擬APIであり、実機シナリオで検査する）、手番の前は送らず開いたまま、閉じた後は同じターンの通知から再表示、次のターン・暗黒期でない家には出ない
- 自動テストの driver（`tools/civ6-autotest/tests/test_diplomacy.py`）：autoplay の途中で出た選択の画面を `OnClose()` で閉じて最後まで進む。改元の画面を閉じ、それで開いた選択の画面も同じ回に閉じる
- 改元の画面と時代スコアの画面（`tests/test_sengoku_ages_screens.py`、lupa。台帳は本物のゲームプレイ Lua、画面はコンテキストごとのグローバルで読み込む）：改元の前は出ない、弘治のターンに1回だけ出て中身（見出し・年齢の絵・武功と閾値・効果）が台帳どおり、自動進行の終わりでも出る、出し直しと `OnClose()`、暗黒期の家はボタンが替わり閉じると選択画面が出る（出ている間は選択画面が出ない）、手番の終わりでは選択画面を出さない、暗黒の後の英雄期の絵、操作中のプレイヤーがいなければ出さない、台帳を書かない。時代スコアの画面：台帳の前は公式のまま、天文の期の欄（元号・次の改元まで・武功・閾値・見込み・内訳）、弘治の後（過去の改元の行、他の家の年齢を台帳から、英雄期の見出し、`Game` が元に戻る）、遅代の後は残りターンと閾値を隠す、令和の期の見込みは点によらず暗黒期、まとめた期の改元の画面は締めた期の元号を出す、台帳を書かない。時代の目盛りの説明：台帳の前は公式のまま、天文と弘治の元号と次の改元までのターン、遅代はターンなし、台帳を書かない。見込みの閾値が次の判定と一致すること（`test_sengoku_ages.py`）
- 台帳の単体テスト（lupa。設計 §10 の 4 の一覧）：奪取の区分、キーの重複防止、同じ城を同じターンに A→B→A、A→B→A→B と奪い合ってそのつど点、閾値の式（期の長さ・都市数・過去の年齢・暗黒の切り捨てと下限 0・黄金の下限 2）と英雄期、同盟の1期1回と同じチームの 0 点、開始時の同盟（ターン32の期限切れを台帳が先・後の両方で 0 点、そのターンに外交で結び直せばどちらの順でも 2 点、プロパティが `false` の後に残る同盟も 0 点、エンジンが解いた後に外交で結べば 2 点）、出来事の点（1家1回、改元のターンのフラグは新しい期、どちらのスクリプトが先でも同じ）、読み込み後に数え直さない、占領してすぐ破却した城の点と滅亡の点、自由都市に移った城の記録が残り、元の家が取り戻すと旧領・都市国家から他家の本拠を奪うと本拠の点（主要勢力以外は点を持たず、その城の旗は 0）、占領イベントと差分で二重に数えない、移管の `CityBuilt` で点を付けない、開拓者から始めた家の本拠、プロパティを書く関数の冪等と要の都市が家ごとに1つ、改元のターンが暦どおり、通知、実機（2026-09-29）の弘治の点と都市数の22家が黄金 3・通常 19・暗黒 0 に分かれる（K=1 では全家が暗黒期）、実機 health-200-01 の6回の改元の点で、旧式が記録どおりの割合を返し、今の式ではどの期も暗黒期が 1/3 以下で通常期が最多。友好宣言（新しく結んだ組に1点ずつ・1期1回、同盟と合わせて 1 + 2 点、開始時の関係と同じチームと初期化時の組は 0 点、ゲームプレイ側に呼び出しが無ければ 0 点でログ1回）、改元の表（47行、日付の順、令和の100年後に遅代）、全5速度で改元のターンが生成スクリプトと一致し同じターンに入らない、同じターンの改元をまとめて1回だけ判定、遅代で全家が暗黒期（点 1000 でも。英雄期にならず、政策を選べ、以後は判定しない）、生成物が最新

## 実機確認（メインセッションで実行）

```powershell
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/sengoku-ages.json --check
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/sengoku-ages.json --output tools/civ6-autotest/artifacts/sengoku-ages-01
```

実行の記録（2026-09-29〜30）：

| 実行 | 結果 |
|---|---|
| ages-impl-01 | 確認はすべて PASS。弘治の22家の点と都市数は下の単体テストに写した（旧式の閾値に当てると全家が暗黒期になるため、閾値を台帳の点に合わせて縮めた：`OFFICIAL_PER_POINT`） |
| ages-impl-02 | 通常期の家の政策を `""` で持つと、次のターンに Game プロパティから読んだ値が `""` でなくなる問題が見つかった。政策なしを `nil` にして直した |
| ages-impl-03 | K=6（`OFFICIAL_PER_POINT = 6`）で全確認 PASS。弘治の判定は黄金 6・通常 15・暗黒 0 |

ages-impl-03 の後に足した次の2つは未実行：城が主要勢力以外に移っても記録を残す修正（日本マップのこのシナリオでは起きにくい）と、下の表の「ターン67（効果）」「2回目のセーブ→ロード」の確認。

MOD と設定は `ages-probe.json` と同じ（15 km 史実領土版、22勢力、武田が人間）。判定は `generic`。確認はすべて GameCore_Tuner と InGame の状態で行い、`log_line` の待ちは使わない。

| 時点 | 確かめること |
|---|---|
| 開始直後 | 公式の閾値の値が上書きされ、改元の表が47行で遅代の行が2119年・`ForcedAge = 'DARK'`（47行への更新は未実行） |
| ターン2 | 台帳に22家・22の本拠。公式の時代スコアが全勢力 0（今の時代の内訳の合計と `GetPlayerCurrentScore`）、公式の年齢が無く、暗黒の閾値 ≤ 点 ＜ 黄金の閾値 |
| ターン31・34 | ターン31：開始時の同盟8組がすべて `start` で、同盟の点のキーが無い。ターン34：8組ともターン32の期限切れでプロパティが `false`（結び直さない。オーナー判断K）、台帳は同盟・友好宣言とも `start` でなく、外交で結び直していない組に同盟・友好宣言の点のキーが無い。公式の時代スコアも再確認 |
| セーブ→メニュー→ロード | 台帳を決まった順に文字列にしたもの（セーブ前に `JAC_AGES_TEST_SNAPSHOT` へ書いた）とロード後が一致 |
| ロード後に1ターン | 各家の研究の完了数が台帳とエンジンで一致（数え直していない） |
| ターン67 | 弘治の判定が `history.r1` にターン66・期の長さ65で残る。生存する各家の結果が黄金期・通常期・暗黒期のどれか。全家が同じ年齢でなく、暗黒期が半分以下。各都市の `JAC_AGE_GOLDEN` と政策の旗が台帳と一致し、暗黒期の家の要の都市が1つ |
| ターン67（効果） | 黄金期の家の生産 2 以上の都市（最大3つ）で `JAC_AGE_GOLDEN` を 0 にすると `City:GetYield` の生産が下がり、1 に戻すと元に戻る（黄金期の生産 +10% の Modifier が旗で入り切りする） |
| 2回目のセーブ→メニュー→ロード | ターン開始の書き直しより前に、各都市の黄金・政策の旗が台帳と一致（タイルのプロパティがセーブに入る）。続けて上の効果の確認をもう一度。公式の時代スコアと年齢を再確認 |

**要実機確認（シナリオでは確かめない。設計の要実機確認をそのまま引き継ぐ）：**

1. `REQUIREMENT_PLOT_PROPERTY_MATCHES` を都市の Subject・Owner の要件に使ったとき、都市の中心タイルを見るか、プロパティの変更に追従するか（設計 §7.1）。シナリオの「ターン67（効果）」で生産だけを確かめる（未実行）。旗を変えてすぐ `GetYield` に出ず次のターン処理で反映される作りなら、この確認はターンをまたぐ形に直す。`City:GetYield` の公式の使用例は UI 側だけ（`Base/Assets/UI/CitySupport.lua:316`）で、ゲームプレイ側で呼べるかも実行で確かめる。金は画面で見る
2. `MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER` で付けた家向けの Modifier が `OwnerRequirementSetId` で入り切りするか、占領で新しい持ち主に効くか（同）
3. タイルのプロパティがセーブに入るか（同 §6.1。入らなくても次のターン開始で書き直す）。シナリオの「2回目のセーブ→ロード」で確かめる（未実行）
4. 通知の見た目（アイコンは公式の誓約の通知と同じ絵）と、22勢力分の通知が AI の手番に影響しないか
5. ~~約200ターンの黄金期・通常期・暗黒期の割合~~ → 下の「約200ターンの割合」（health-200-01）。今の式で200ターンを回し直すのと、開拓者から始める設定は未確認

## 約200ターンの割合（health-200-01、2026-09-30）

全 MOD で 200 ターン（AI 22 家。家 13 は元亀の前に滅亡）。`tools/civ6-autotest/artifacts/health-200-01/Lua.log` の `[JAC_AGES] judged` 128 行。黄金・英雄・通常・暗黒の数：

| 改元 | ターン | 期の長さ | 点 | 旧式（暗黒は切り上げ・下限 1） | 今の式（再計算） |
|---|---|---|---|---|---|
| 弘治 | 66 | 65 | 2〜15 | 4/0/13/5 | 4/0/17/1 |
| 永禄 | 73 | 7 | 0〜1 | 0/0/8/14 | 0/0/22/0 |
| 元亀 | 110 | 37 | 0〜18 | 0/8/6/7 | 5/0/13/3 |
| 天正 | 119 | 9 | 0〜5 | 1/1/9/10 | 1/1/19/0 |
| 文禄 | 178 | 59 | 2〜11 | 5/5/7/4 | 6/0/10/5 |
| 慶長 | 189 | 11 | 0〜6 | 4/0/6/11 | 4/0/15/2 |

短い期は期待される点が 1 点未満で、旧式では点 0 の家が暗黒期になり、暗黒期が半数を超えた。今の式の列は、記録の点と、記録の閾値から逆算した都市数（ログに都市数は無い）で判定を順に再計算した値（過去の年齢の補正も今の式の結果で数え直す）。単体テスト `ShortReignDistributionTests` に写した。今の式で実機の200ターンは未実行。

## 後続

1. ~~暗黒期の専用政策を選ぶ画面と受け付け、AI の家ごとの順位表~~ → 上の「暗黒期の政策の選択」（実機 PASS：ages-dark-choice-01、2026-09-30）
2. 誓約（L-4：各家に固有の誓約を3つずつ、共通の誓約なし。22家 × 3 = 66 の中身を別の表で作る）と、誓約の達成の点。織田家の3つのうち1つは「天下布武」（オーナー判断 U-1、2026-09-30。それまでは表示しない）
3. ~~改元の画面・時代スコアの画面（§8）~~ → 上の「改元の画面と時代スコアの画面」（実機は未実行）
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
| ゲームプレイ側の呼び出し | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:136`（`GetCities():Members()`）、`:1103`（`GetCapitalCity`）、`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:1198`（`GetCities():GetCount()`）、`DLC/CivRoyaleScenario/Scripts/CivRoyaleScenario_UnitCommands.lua:157`（`GetTeam`）、`DLC/NubiaScenario/Scripts/NubiaScenario.lua:83`（`PlayerManager.GetAlive`。都市国家・自由都市を含む全プレイヤー） |
| 暗黒期の政策の選択 | `DLC/BlackDeathScenario/UI/BlackDeathScenario_PopupHandler.lua:21-28`・`Scripts/BlackDeathScenario_StateUtils.lua:64-74`（UI から `EXECUTE_SCRIPT` で GameEvent を呼ぶ）、`DLC/Expansion2/UI/Additions/DisloyalCityChooser.lua`（`ContextPtr:SetHide` と `OnClose` で出し入れする画面）、`Base/Assets/UI/ActionPanel.lua:688,1334`（`IsTurnActive`、`LocalPlayerTurnBegin`）、`Base/Assets/UI/Popups/PlayerChange.lua:298`（`LoadScreenClose`） |
| 暗黒期の選択の終了ブロック | `DLC/Expansion2/Data/Expansion1_Notifications.xml:70`（終了で消えない）、`DLC/Expansion2/UI/Replacements/ActionPanel_Expansion1.lua:29`（専用のブロック型）、`Base/Assets/UI/ActionPanel.lua:500-519,553-602`（自動終了・ボタンが通知を確認）、`Base/Assets/UI/Panels/NotificationPanel.lua:1637-1646,1691-1703,1896`（起動と読み込み）、`DLC/PiratesScenario/Scripts/PiratesScenario_StartScript.lua:340-343`・`PiratesScenario_PropKeys.lua:85-89`（任意プロパティと手動消去の禁止）、`DLC/PiratesScenario/UI/Replacements/NotificationPanel_PiratesScenario.lua:74-84`（IDを列挙して消す） |
| 改元の画面・時代スコアの画面 | `DLC/Expansion2/UI/Additions/EraReviewPopup.xml`・`.lua:62-124`（組み立て、年齢ごとの絵、指導者の肖像）、`EraProgressPanel.xml`・`.lua:212-321,387-407`（欄、`Refresh`、他の家の一覧、`Open`）、`EraCompletePopup.lua:107-112,120,268-274`（通常の枠、音、手番の始まりに出す）、`DLC/Expansion2/UI/Replacements/PartialScreenHooks_Expansion1.lua:40-56,73`（上部の時代のボタン）、`Base/Assets/UI/ActionPanel.lua:1335,1350`（`LocalPlayerTurnEnd`、`LuaEvents.AutoPlayEnd`）、`Base/Assets/UI/Civilopedia/CivilopediaScreen.lua:13`（ワイルドカード include）、`DLC/Expansion2/UI/Additions/DisloyalCityChooser.lua`（`ContextPtr:SetHide` の出し入れ） |
| 時代の名前を出す所（L-1） | `Base/Assets/UI/ActionPanel.lua:343-364,1161-1168`、`DLC/Expansion2/UI/Replacements/ActionPanel_Expansion1.lua:181,187-216`、`ActionPanel_Expansion2.lua:6`、`DLC/Expansion2/Expansion2.modinfo:366-371`、`DLC/Expansion2/UI/Additions/EraProgressPanel.lua:60-68`、`NewTurnPanel_Expansion1.lua:11-24`、`Base/Assets/UI/Popups/EraCompletePopup.lua:55`、`DLC/Expansion2/UI/Additions/EraReviewPopup.lua:62`・`DedicationPopup.lua:53` |
| 通知とアイコン | `Base/Assets/Gameplay/Data/Notifications.xml:52-60`（`NOTIFICATION_USER_DEFINED_2`）、`DLC/Expansion2/Data/Expansion1_Icons_Notifications.xml:36`（誓約の通知の絵）。`USER_DEFINED_1` は史実の出来事が使う |
