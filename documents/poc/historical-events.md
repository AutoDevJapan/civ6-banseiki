# 史実の出来事の仕組み（JAC_PoC_HistoricalEvents）

SSOT 3.2「史実の出来事は史実の年に起き、先に関係する研究を終えた勢力はその年より早く使える」「史実の年が来たとき、関係する研究を終えていない勢力には、その研究へのひらめき（ブースト）だけを与える」「史実イベントは、暦の月までできるだけ合わせて起こす」「家名の切り替えは、長尾→上杉、松平→徳川、本能寺の変の後の織田→羽柴→豊臣を同じ仕組みで行う」と、3.1「因縁は和睦を促すイベント（ひらめきを与え、助言役のポップアップで知らせる）で解消へ導く」を、データ駆動で実装した PoC。`RULESET_JAC_BANSEIKI` のときだけ有効になる。地図では絞らない：史実の出来事は戦国パックの機能（SSOT 3.2）で、戦国パック（ツリー・暦。`JAC_PoC_SengokuTrees`）も万世紀ルールセットだけを条件にどの地図でも動くため、それに合わせる。日本マップ限定なのは資源の予約地点だけで、それは下の「資源を置く出来事」のとおり資源 MOD が無ければ何もしない。

出来事の一覧は [`spec/data/sengoku-events.json`](../spec/data/sengoku-events.json)（60件。設計は [research/sengoku-events.md](../research/sengoku-events.md)）が正本で、`tools/generate_sengoku_events.py` がそこから SQL と文言を生成する（手で編集しない）。

- MOD：`poc/JAC_PoC_HistoricalEvents`（ID `f6398d3b-707c-4d0a-8b19-e68333efa200`）
- 依存：Gathering Storm、万世紀ルールセット、CustomCivs（長尾家などと家名表示の上書き）、戦国ツリー（前提技術・社会制度の ID）
- 生成：`python tools/generate_sengoku_events.py`（`--check` で生成物が最新かを確かめる）。出力は `Data/JAC_HistoricalEvents_Sengoku.sql` と `Text/JAC_HistoricalEvents_Text.xml`
- 実機シナリオ：`tools/civ6-autotest/scenarios/historical-events-15km.json`（採用縮尺の 15 km 地図。SSOT 3.4）
- 静的テスト：`tests/test_historical_events.py`。Lupa のテスト：`tests/test_historical_events_lua.py`（ゲームプレイ側と助言ポップアップの UI）

## 動き

1. 毎ターンの開始（`GameEvents.OnGameTurnStarted`）に、全出来事 × 生存中の主要勢力を調べる
2. 対象勢力（`JAC_HistoricalEventTargets`。一覧の `affected_factions`。`ALL` は行なしで全主要勢力）のうち、まだ効果を受けていない勢力を、次の順に判定する
   - 前提の技術・社会制度（`JAC_HistoricalEventPrereqTechs`／`JAC_HistoricalEventPrereqCivics`。一覧の `earlier_by`）の**いずれか1つ**を持っている → 出来事の効果を与える（`reason=prereq`。研究を先に終えた勢力は年より早い）
   - 前提の無い出来事（4件：大友二階崩れ・駿河侵攻・朝倉氏の滅亡・武田氏の滅亡）で、史実の年（と月）のターンに達した → 効果を与える（`reason=due`）
   - 前提があり、年に達したが未研究 → 出来事のひらめき（`BoostTech`／`BoostCivic`。一覧の `boost`）**だけ**を与える（`reason=year`。1勢力1回）。効果は、その勢力が後で前提のどれかを研究したターンに `reason=prereq` で与える
3. 効果を与えるとき（`prereq`／`due`）
   - フラグ：プレイヤーのプロパティ `JAC_EVENT_FIRED_<EventType>` に与えたターンを書く。これが「1勢力1回」の記録で、セーブに残る
   - 効果（モディファイア）：`JAC_HistoricalEventModifiers` のモディファイアを付ける。一覧の `gameplay_effect` はまだ文章だけで数値が無いため、今は行が無い。発生回数 `JAC_EVENTS_FIRE_COUNT` を1増やす
4. ひらめきだけのとき（`year`）
   - 出来事のひらめきを発動する。ログの `boost=` に `tech=boosted`／`civic=boosted`（発動）、`already`（既に発動済み）、`no_boost`（`Boosts` の行が無い）か `error:…` を出す
   - 発動できた（`boosted`／`already`／`no_boost`）ときだけ、プロパティ `JAC_EVENT_BOOSTED_<EventType>` に年が来たターンを書く（効果のフラグは書かない）。`error` のときは書かず、次のターンにもう一度試す。通知も発動できたターンに1回だけ送る
5. どちらの場合も行うこと（出来事そのものは史実の年に起きるため）
   - 家名：下の「家名の段階」
   - 資源：`JAC_HistoricalEventResources` のうち `Action='reveal'` の予約地点を、その出来事が誰かに初めて起きたときに1回だけ開示する（下の「資源を置く出来事」）
   - 通知：`NotificationTypes.USER_DEFINED_1` で本人に送る（`NotifyAll=1` なら全主要勢力。生成した60件はすべて0）。見出しは出来事名、本文は助言の文（助言の無い出来事は共通の「史実の出来事が起きた。」）。家固有の出来事（`House`。一覧の `house_specific`）は、その家には固有名、ほかの家には一般名（`GenericName`）を出す（SSOT 3.2）。ひらめきの後に研究して効果を受けたときは、同じ通知を2度は送らない。文面は LOC キーのまま渡し、各端末の言語で表示する
   - 助言ポップアップ：下の「助言ポップアップ」
6. `Lua.log` に `[JAC_EVENTS] fired event=… reason=prereq|due …`（効果）か `[JAC_EVENTS] boosted event=… reason=year … boost=…`（ひらめきだけ）を出す

## データ形式

コードを変えずに行を足すだけで出来事を増やせる。一覧を変えるときは JSON を直して生成し直す。

| テーブル | 列 | 意味 |
|---|---|---|
| `JAC_HistoricalEvents` | `EventType`（主キー。`EV_TEPPO` → `JAC_EVENT_TEPPO`）、`Pack`、`Year`、`Month`、`Turn`、`BoostTech`、`BoostCivic`、`Name`、`House`、`GenericName`、`Description`、`Advisor`、`NotifyAll` | `Year` と `Turn` のどちらかは必須。両方あれば `Turn` を使う。`Month`（1〜12、任意）は一覧の月。無ければ1月扱い。`Boost*` は年が来たときに未研究の勢力へ与えるひらめき（どちらか1つ）。`House` を持つ行は、その文明に `Name`、ほかに `GenericName` を出す。`Description` は通知の本文で、`Advisor=1` なら助言の文 |
| `JAC_HistoricalEventPrereqTechs`／`JAC_HistoricalEventPrereqCivics` | `EventType`、`TechnologyType`／`CivicType` | 早く起きる条件。どれか1つを持てばよい。行が無ければ年だけで起きる |
| `JAC_HistoricalEventTargets` | `EventType`、`CivilizationType` | 対象勢力。行が無ければ全主要勢力 |
| `JAC_HistoricalEventModifiers` | `EventType`、`ModifierId` | 起きた勢力に付けるプレイヤーのモディファイア（今は行なし） |
| `JAC_HistoricalEventRenames` | `EventType`、`CivilizationType`、`Name` | 文明の表示名の切替。文明 ID は変えない |
| `JAC_HistoricalEventResources` | `EventType`、`SiteId`、`Action`、`OwnerTo` | `reveal`：開示する資源の予約地点（資源 MOD の地点 ID）。`trade_access`（平戸の硝石）と `ownership`（石見銀山・奥出雲の鉄を毛利へ）は**記録だけ**で、Lua は使わない（交易アクセスは未実装。所有の自動移転はしない：Q43） |

生成の対応：`affected_factions` → 対象、`earlier_by` → 前提、`boost` → `Boost*`、`house_specific`・`house`・`generic_name` → `House`・`GenericName`、`advisor_popup` → `Description`・`Advisor=1`、`rename` → 家名、`resource` → 資源（地点 ID は `japan-map-resources.json` の座標で引き、`reveal` は資源 MOD の予約地点に無ければ生成を止める）。`gameplay_effect`・`year_end`・`regions`・`sources` などは生成に使わない。

`Boosts` の行が無いとひらめきは発動できないため、ツリー側に行の無い30件の技術・社会制度に仮の行（`BOOST_TRIGGER_NONE_LATE_GAME_CRITICAL_TECH`、40%、`LOC_JAC_EVENT_BOOST_PLACEHOLDER`）を足している（**暫定**）。この種類は公式でも自然には満たされない（`Base/Assets/Gameplay/Data/Technologies.xml:440,505`）ので、出来事のひらめき以外で発動しない。以前の仮の行（ガレー船の生産で発動）は、30件がガレー船1隻でまとめて発動してしまうため替えた。ツリー側に行がある T09・T10 は足さない。実際のひらめき条件はツリー側で決める。

## 暦との関係

Civ6 の公式 Lua には年を数値で返す API が無く、暦は表示用の文字列（`Calendar.MakeYearStr`）しか出さない。公式の Pirates シナリオもターン数から年を計算している。そこで、暦を作っている表そのものから年をターンに換算する。

- 暦は main にマージ済みの戦国パックの暦（[sengoku-calendar.md](sengoku-calendar.md)、PR #34）。`ERA_ANCIENT` の開始年が1534、ゲーム速度ごとに `GameSpeed_Turns`（`MonthIncrement` か月/ターンを `TurnsPerIncrement` ターン）が順に並ぶ
- 換算（`Scripts/JAC_HistoricalEvents.lua` の `TurnForYear`）：ターン1を開始年の1月とし、ターン `t` の手前までに経過した月数が `(年 - 開始年) × 12 + (月 - 1)` に達する最初のターン。開始年は `GameInfo.StartEras['ERA_ANCIENT'].Year`、速度は `GameConfiguration.GetGameSpeedType()`、区間は `GameInfo.GameSpeed_Turns()` を速度で絞り、`MonthIncrement` の大きい順（同じなら `TurnsPerIncrement` の小さい順）に並べ替えて読む（速度は `InGameTopOptionsMenu.lua:442` と同じ取り方）。エンジンは区間を行の挿入順（`GameSpeedTurnCollection` の `ORDER BY rowid`、`01_GameplaySchema.sql:3337`）で使うが、Lua からは rowid が見えず、`GameInfo` の列挙順も保証されない。公式の `GameSpeeds.xml` も戦国の暦も、各速度の区間を1ターンの月数が長い順に並べているので、この並べ替えは挿入順と一致する（静的テストで戦国の暦の並びを固定し、Lupa のテストで行を逆順に渡しても同じターンになることを確かめる）。月数が後で長くなる区間を持つ暦を足すと、この前提が崩れる
- エンジンの現在ターンは1から始まらない。1534年開始の暦では `Game.GetCurrentGameTurn()` と `GameConfiguration.GetStartTurn()` がともに 16603 から始まる（実機、2026-09-29。暦の文字列は「1534 AD」）。そこでスクリプトは、公式の上部パネルが「ターンを1から数える」ために行う `現在 - GetStartTurn() + 1`（`Base/Assets/UI/TopPanel.lua:390`）を `GameTurn()` に取り、上の換算・発生ターンのプロパティ・ログはすべてこの経過ターンで扱う。開始ターンはセーブに含まれる設定なので、セーブ・ロードでも速度が違っても変わらない
- 標準速度（4か月/ターン＝1年3ターン）では、月が無い出来事は `ターン = (年 - 1534) × 3 + 1`。例：天文の大飢饉1540年はターン19、鉄砲伝来1543年9月はターン30、長尾→上杉1561年4月はターン83、本能寺の変1582年6月はターン147、豊臣姓の下賜1586年10月はターン160。60件はすべて200ターン以内
- ほかの速度も同じ表で自動的に換算する。1年あたりのターン数は Online 1.5、Quick 2、Epic 4（200ターンまで。以後6）、Marathon 6（200ターンまで。以後12）
- 暦の月（SSOT 3.2）：`Month` は一覧の `month`。一覧は史料の和暦の日付を西暦に換算した月（1582年10月4日まではユリウス暦、以後はグレゴリオ暦。一覧の `_schema.month_note`）を持つので、そのまま使う。画面は年だけを表示する（SSOT 3.2）ので、月は発生ターンにだけ効く
- 静的テスト（`tests/test_historical_events.py`）は、暦の XML から同じ換算を Python で再計算し、全速度で「換算したターンの暦上の年が出来事の年で、その前のターンは前の年」と「月を指定したら、そのターンで初めてその月に達する」を確かめる

## 家名の段階

長尾→上杉（`EV_NAGAO_TO_UESUGI`、1561年4月）、松平→徳川（`EV_MATSUDAIRA_TOKUGAWA`、1567年1月）、織田→羽柴（`EV_HONNOJI`、1582年6月）→豊臣（`EV_TOYOTOMI_SURNAME`、1586年10月）。織田の2段階の時期は Q56 の推奨①（オーナー未回答の暫定）。

- 出来事がその家に起きたとき（効果でも、ひらめきだけの年でも）、Game プロパティ `JAC_CIV_NAME_<CivilizationType>` に表示名の LOC キーを書く。CustomCivs の `UI/JAC_CivNameOverride.lua` がこれを読み、外交リボン・外交画面・ワールドランキング・都市バナーの文明名を差し替える（既存の仕組み。[house-names.md](house-names.md)）
- 段階は研究で早まるため、順番どおりに来るとは限らない（例：年で羽柴・豊臣になった後、織田が本能寺の変の別の前提を研究して `reason=prereq` でもう一度起きる）。そこで Game プロパティ `JAC_CIV_NAME_STAGE_<CivilizationType>` に表示中の段階の予定ターンを残し、それより前の段階では上書きしない。最後の段階が残る
- 家名は日英露：上杉 Uesugi Уэсуги、徳川 Tokugawa Токугава、羽柴 Hashiba Хасиба、豊臣 Toyotomi Тоётоми（一覧は日本語だけなので、生成スクリプトに対応表を置く）
- 松平・織田の改名も長尾と同じく前提の社会制度（C16 など）で早まる。C16 を早く得た織田は本能寺の変より前に豊臣になりうる（Q66）

## 助言ポップアップ

オーナー判断 E5a（和睦などを薦める出来事は、ひらめきを与え、助言役のポップアップで知らせる）。一覧の19件が助言の文を持つ（`Advisor=1`）。

- 公式の助言ポップアップ（`Base/Assets/UI/Popups/AdvisorPopup.lua`）をそのまま使う。`TutorialUIRoot.xml:21` がこのコンテキストを置き、通常のゲームでも読み込まれる。`LuaEvents.TutorialUIRoot_AdvisorRaise`（`AdvisorPopup.lua:521`）に `AdvisorItem`（`:25-38`）を渡すと `ShowOrQueuePopup` → `ShowAdvisorPopup`（`:94`）で出る。表示中に別の助言が来たら公式のキューに入り、閉じた後に出る
- 変換（アダプター）は `UI/JAC_HistoricalEventsAdvisor.lua`（`InGame` に足す、部品の無いコンテキスト）。公式のチュートリアルと同じく `hstructure AdvisorItem` を宣言して `hmake` で組み立てる（`TutorialUIRoot.lua:38-52,216-229`。公式の FIXME のとおり、宣言は使う側の各ファイルに同じものを置く）。本文は「出来事名＋改行＋助言の文」を `Locale.Lookup` した文字列（`TutorialUIRoot.lua:366` と同じく引いてから渡す）。ボタンは1つ（「承知した」）で、押すと閉じるだけ。コールバックを自前で持つので、チュートリアルの `AdvisorPopup_ClearActive` は呼ばれない。助言役の顔（`ShowPortrait=true`）付きの吹き出しで出る
- 出すのはローカルのプレイヤーだけ：`Game.GetLocalPlayer()` のプレイヤーのフラグ（`JAC_EVENT_FIRED_`／`JAC_EVENT_BOOSTED_` の早い方）が今のターンの出来事だけを出す。フラグは対象の勢力にしか付かないので、AI の出来事やほかの端末のプレイヤーの出来事は出ない。UI はゲームの状態を書かない
- 出すきっかけは `Events.LocalPlayerTurnBegin`（ターン開始の処理でフラグが付いた後）と `Events.LocalPlayerChanged`（オートプレイから人間に戻ったとき）。公式の `ActionPanel.lua:1333-1334` と同じ2つ。同じ出来事は1セッションで1回だけ出す。オートプレイ中はローカルのプレイヤーがいない（`-1`）ので出さない
- 自動操作を止めない：ポップアップは公式のコンテキストなので、ドライバの既存の閉じ処理（`scenario.py` の `_dismiss_advisor`。`IsBlockingInput()` のとき `OnHideAdvisorDialog()`）がオートプレイ・ターン終了・ロードの前後で閉じる。新しいポップアップのコンテキストは足していないので、ドライバの変更は要らない
- ひらめきは、年が来たときに未研究の勢力へ `reason=year` で与える（上の「動き」）。先に研究していた勢力には、ひらめきは無く、ポップアップと通知だけが出る
- 和睦を薦める10件を1534年の因縁に対応づけて因縁を解消する処理は、因縁の実装側で行う（設計 §2.6）。この PoC は知らせるだけ

## 資源を置く出来事

資源は、main の資源 MOD（[resources.md](resources.md)、`poc/JAC_PoC_JapanMapScale/Scripts/JAC_Resources.lua`）が生成器で予約した地点に置く。出来事の行は地点 ID だけを持ち、発生時に `ExposedMembers.JAC_Resources.RevealSite(SiteId)` を呼ぶ。置いたタイルは Game プロパティ `JAC_EVENT_RESOURCES_<EventType>` に残す。ログは `resource event=… site=… plot=<タイル>`。

| 出来事 | 地点 | 年 |
|---|---|---|
| 生野銀山 `JAC_EVENT_IKUNO_SILVER` | `silver-ikuno` | 1542 |
| 佐渡平定 `JAC_EVENT_SADO_SUBJUGATION` | `gold-sado-nishimikawa`（15 km のみ。SSOT 3.2） | 出来事1589、地点1593（Q67） |
| 会津の漆 `JAC_EVENT_AIZU_LACQUER` | `lacquer-aizu` | 1590 |
| 相川金銀山 `JAC_EVENT_AIKAWA` | `gold-sado-aikawa` | 1596 |

- 資源 MOD は自分でも、地点の年か、いずれかの勢力の研究完了（初期知識を除く）で同じ地点を開示する。`RevealSite` は開示済みなら何もしないので、どちらが先でも同じタイルに1つだけ置かれる。地点の研究は出来事の前提のどれかに入っている（テストで照合）
- 資源 MOD は日本マップ4種でだけ読み込まれる。ほかの地図ではログに `resource event=… skipped=resources_mod_not_loaded` を出し、何も置かず、`JAC_EVENT_RESOURCES_<EventType>` も書かない。20 km 地図には西三川の地点が無いので、資源 MOD が `unknown site` を出し、出来事は `plot=not_placed` と記録する
- 石見銀山（1526年）は開始時から地図に置く（SSOT 3.2、資源 MOD）ので、出来事では開示しない（毛利への所有は記録だけ）
- 以前のサンプルの足尾銅山の出来事は一覧に無いので無くなった。足尾の銅は資源 MOD が1550年か鉱脈採掘の研究で置く

## ひらめきの公式 API の根拠

- 技術：`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:761-784`。`player:GetTechs():HasBeenBoosted` で未発動を確かめ、`player:GetTechs():TriggerBoost(techIndex, 1)`（コメント「ゴーディハット由来として発動」）。UI 側の判定は `Base/Assets/UI/Panels/TechBoostsPanel.lua:91`（`HasTech`／`HasBoostBeenTriggered`／`CanTriggerBoost`）
- 社会制度：`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:556`（ゲームプレイ側）が `player:GetCulture():HasBoostBeenTriggered` を使う。`GetCulture():TriggerBoost(civicIndex, 1)` を呼ぶ公式の Lua 例は見つからなかった（**要実機確認**。実機シナリオの伊達家の塵芥集（ターン8）で確かめる）
- ゲームプレイ側では `CanTriggerBoost` が無い（UI 側だけ）。実機（events-06）で `function expected instead of nil` になり、1勢力目で止まって誰にもひらめきが入らなかった。ひらめきの有無は `GameInfo.Boosts` の行で判定し、未発動かは `HasBoostBeenTriggered` で確かめ、呼び出しは `pcall` で囲む（エラーはログの `boost=tech=error:...` に残し、ほかの勢力・出来事の処理は続ける）。`tests/test_historical_events_lua.py` がゲームプレイ側のメソッドだけのスタブでターンを進めて確かめる
- 代替（不採用）：モディファイア `MODIFIER_PLAYER_GRANT_SPECIFIC_TECH_BOOST`（`Base/Assets/Gameplay/Data/Modifiers.xml:2499-2503`）は技術のひらめきだけで、社会制度用の同種は無い。両方を同じ経路で扱うため Lua の API にした

## 決定事項（オーナー回答）

| 番号 | 決定 | 状態 |
|---|---|---|
| 1a | 年が来たとき、前提の技術・社会制度を未研究の勢力には、それ自体ではなくひらめき**だけ**を与える（SSOT 3.2）。研究済みの勢力は影響なし | 実装済み（一覧の `boost`） |
| 2a | 長尾→上杉は「1561年、または関連する社会制度を先に研究」の条件を維持する | 実装済み（テストで固定） |
| 6a | 松平→徳川の改名も、長尾→上杉と同じ仕組みの出来事にする | 実装済み（1567年1月） |
| 7a | 本能寺の変の後、織田家の家名表示を羽柴から豊臣へ切り替える（同じ仕組み） | 実装済み（時期は Q56 の推奨①で暫定） |
| 8a | 室町幕府が終わる1573年、足利家の勢力は残り、将軍として裁定する権能だけを失う | 出来事 `JAC_EVENT_MUROMACHI_END` は通知とひらめきだけ。勢力は消さない。裁定の権能は停戦裁定の MOD 側（[court-arbitration.md](court-arbitration.md)）が扱う |
| 10a | 関ヶ原の戦いの出来事は置かない | 一覧に無い（テストで固定） |
| E6b | 史実イベントは暦の月までできるだけ合わせる（SSOT 3.2） | 実装済み（`Month` 列） |
| E5a | 和睦などを薦める出来事は、ひらめきを与え、助言役のポップアップで知らせる（SSOT 3.1） | 実装済み（上の「助言ポップアップ」）。因縁の解消は因縁の実装側 |
| Q43 | 城・資源の所有移転は自動で行わない（推奨①。未回答） | 記録だけ（`Action='ownership'`） |

## マルチプレイとセーブ

- 状態はすべて Player／Game のプロパティにあり、Lua の変数にターンをまたいで持たない。ロード後もプロパティから同じ判定になり、同じ出来事は2度起きない（Lupa のテストでスクリプトを読み直して確かめ、実機シナリオでもセーブ→ロード→1ターンで確かめる）
- 判定はゲームプレイ側のターン開始イベントだけで行い、出来事は「予定ターン→ID」、勢力は ID 順に処理する。乱数・ローカルプレイヤー・端末の言語に依存しない
- 家名は Game プロパティ（同期済みの共有状態）から UI が読む。端末ごとの設定値は使わない（SSOT 3.1）
- 助言ポップアップは各端末が自分のプレイヤーのフラグを読んで出す（UI だけで、ゲームの状態は変えない）。どの出来事を出したかは端末のメモリにだけあるので、出来事が起きたターンのセーブをロードすると、そのターンの助言はもう一度出る
- マルチプレイでの同期・決定性は未検証（[PoC README「マルチプレイ同期と決定性」](README.md#マルチプレイ同期と決定性)と同じ扱い）

## 公式ファイルの根拠

パスは `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/` からの相対。

| 使っている仕組み | 公式の例 |
|---|---|
| ターン開始の処理と現在ターンの取得 | `DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:1440-1455`（`GameEvents.OnGameTurnStarted` と `Game.GetCurrentGameTurn()`）、`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:64` |
| プレイヤーのプロパティ | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:373`（`SetProperty`）、`DLC/CivRoyaleScenario/Scripts/CivRoyaleScenario_UnitCommands.lua:137`（`GetProperty`） |
| モディファイアを Lua で付ける | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:1204`（`pPlayer:AttachModifierByID`） |
| 通知（LOC キーを渡す） | `DLC/PiratesScenario/Scripts/PiratesScenario_Shared_Script.lua:318-338`、`Base/Assets/Gameplay/Data/Notifications.xml:52,165`（`NOTIFICATION_USER_DEFINED_1`）、`Base/Assets/UI/Panels/NotificationPanel.lua:181,231` |
| 通知アイコン | `DLC/BlackDeathScenario/Data/BlackDeathScenario_Icons_Notifications.xml:4-6`（本体には `ICON_NOTIFICATION_USER_DEFINED_1` が無い） |
| 自然には発動しないひらめきの種類 | `Base/Assets/Gameplay/Data/Technologies.xml:440`（`BOOST_TRIGGER_NONE_LATE_GAME_CRITICAL_TECH`）、`:505`（使用例） |
| 助言ポップアップ | `Base/Assets/UI/Popups/AdvisorPopup.lua:25-38`（`AdvisorItem`）、`:94`（`ShowAdvisorPopup`）、`:521`（`TutorialUIRoot_AdvisorRaise`）、`Base/Assets/UI/TutorialUIRoot.lua:216-229`（`hmake AdvisorItem`）、`:1565`（発火）、`Base/Assets/UI/TutorialUIRoot.xml:21`（コンテキスト） |
| ローカルのターン開始・プレイヤー切替の UI イベント | `Base/Assets/UI/ActionPanel.lua:1333-1334` |
| ゲーム中の資源配置 | 資源 MOD の `RevealSite` に任せる（根拠は [resources.md](resources.md)） |
| 研究済みの判定 | `DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:670`（`HasTech`）、`:555`（`HasCivic`） |
| ゲームプレイ側の文明 ID | `DLC/WarMachineScenario/Scripts/WarMachineScenario.lua:27`（`GetCivilizationTypeName`） |
| 独自テーブルの追加 | `DLC/Expansion2/Data/Expansion2_Schema.sql:2`（`CREATE TABLE` を UpdateDatabase で読む） |
| 年の表示と計算 | `Base/Assets/UI/TopPanel.lua:407`（`Calendar.MakeYearStr` のみ）、`DLC/PiratesScenario/UI/Replacements/TopPanel_PiratesScenario.lua:10-18`（ターンから年を計算） |
| シナリオでの研究の強制 | `DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:38`（`SetTech`）、`DLC/PiratesScenario/Scripts/PiratesScenario_Shared_Script.lua:956`（`SetCivic`） |

## 実機確認（メインセッションで実行）

```powershell
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/historical-events-15km.json --output tools/civ6-autotest/artifacts/historical-events-run-03
```

有効にする MOD は `japan-resources.json` と同じ5つに、この MOD を足したもの。15 km 史実領土版・22勢力・織田が人間（設定は `build_setup` で組み立てる）。ログの待ち合わせは前の手順より後の行しか見ないため、確認は状態（プロパティ・UI の表）で行う。

| 手順 | 確かめること |
|---|---|
| 開始直後 | `Lua.log` に `[JAC_EVENTS] loaded events=60 startYear=1534 speed=GAMESPEED_STANDARD`。ターン1で出来事は1件も起きていない |
| ターン1に強制 | 織田（人間）に技術「兵糧保存」、長尾家に社会制度「朝廷・将軍への取次」、島津家（AI）に社会制度「国人盟約」を与える（GameCore_Tuner） |
| 1ターン進めた後 | 織田に天文の大飢饉（助言あり）、長尾家に上杉継承、島津家に薩摩統一戦（助言あり）がターン2で起きる。長尾家の表示名が上杉。発生回数と全フラグの数が一致（2度起きていない） |
| 助言ポップアップ | `JAC_HistoricalEventsAdvisor` の `JAC_EVENTS_ADVISOR_SHOWN` に `JAC_EVENT_TENBUN_FAMINE@2` があり、島津・長尾（AI）の助言は無い。公式の `AdvisorPopup` が入力を止めて表示中で、本文が大飢饉（または天正地震）の助言の文。スクリーンショット `advisor-popup.png` を撮ってから閉じる |
| セーブ→メニュー→ロード | 同じ確認がロード直後にも通る |
| ロード後に1ターン | 各フラグがターン2のまま（2度起きていない）。家名の切替も残る |
| ターン19まで進める | 暦が1540年。兵糧保存を未研究の勢力に `JAC_EVENT_BOOSTED_JAC_EVENT_TENBUN_FAMINE`=19 と兵糧保存のひらめき。織田はターン2の効果のまま。伊達家は塵芥集（1536年5月＝ターン8）で社会制度「分国法」の着想（`GetCulture():TriggerBoost`）を受けたか、先に研究して効果を受けている |

判定は `generic`（`Lua.log` の JAC 由来のエラーが無いこと）。

**要実機確認（シナリオで確かめること・確かめないこと）：**

1. 助言ポップアップが、オートプレイから人間に戻ったとき（`Events.LocalPlayerChanged`／`LocalPlayerTurnBegin`）に実際に出るか。自前の `hstructure AdvisorItem` を公式のコンテキストに渡して型の不一致にならないか（公式のチュートリアルも別ファイルの同じ宣言で渡している）。シナリオの「助言ポップアップ」の手順で確かめる
2. 社会制度の `GetCulture():TriggerBoost` と、仮の `Boosts` 行（`BOOST_TRIGGER_NONE_LATE_GAME_CRITICAL_TECH`）を社会制度に使えるか。シナリオの伊達家（ターン8）で確かめる（伊達家が先に研究した場合は通らない）
3. 通知の見た目。`ICON_NOTIFICATION_USER_DEFINED_1` に `ICON_ATLAS_NOTIFICATIONS` の15番（公式 Black Death と同じ）を当てたが、絵柄が出来事に合うかは画面で見る
4. 資源の開示は資源 MOD の実機確認（`japan-resources.json`）と同じ。シナリオは資源の出来事を通らない
5. 標準以外の速度での暦の換算（区間の並び順の前提。上記「暦との関係」）
6. マルチプレイでの同期
