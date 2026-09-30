# 史実の出来事の仕組み（JAC_PoC_HistoricalEvents）

SSOT 3.2「史実の出来事は史実の年に起き、先に関係する研究を終えた勢力はその年より早く使える」「史実の年が来たとき、関係する研究を終えていない勢力には、その研究へのひらめき（ブースト）だけを与える」「史実イベントは、暦の月までできるだけ合わせて起こす」「家名の切り替えは、長尾→上杉、松平→徳川、本能寺の変の後の織田→羽柴→豊臣を同じ仕組みで行う」と、3.1「因縁は和睦を促すイベント（ひらめきを与え、助言役のポップアップで知らせる）で解消へ導く」を、データ駆動で実装した PoC。`RULESET_JAC_BANSEIKI` のときだけ有効になる。地図では絞らない：史実の出来事は戦国パックの機能（SSOT 3.2）で、戦国パック（ツリー・暦。`JAC_PoC_SengokuTrees`）も万世紀ルールセットだけを条件にどの地図でも動くため、それに合わせる。日本マップ限定なのは資源の予約地点だけで、それは下の「資源を伴う出来事」のとおり資源 MOD が置き、出来事は置かない。

出来事の一覧は [`spec/data/sengoku-events.json`](../spec/data/sengoku-events.json)（61件。設計は [research/sengoku-events.md](../research/sengoku-events.md)）が正本で、`tools/generate_sengoku_events.py` がそこから SQL と文言を生成する（手で編集しない）。

- MOD：`poc/JAC_PoC_HistoricalEvents`（ID `f6398d3b-707c-4d0a-8b19-e68333efa200`）
- 依存：Gathering Storm、万世紀ルールセット、CustomCivs（長尾家などと家名表示の上書き）、戦国ツリー（前提技術・社会制度の ID）
- 生成：`python tools/generate_sengoku_events.py`（`--check` で生成物が最新かを確かめる）。出力は `Data/JAC_HistoricalEvents_Sengoku.sql` と `Text/JAC_HistoricalEvents_Text.xml`
- 実機シナリオ：`tools/civ6-autotest/scenarios/historical-events-15km.json`（採用縮尺の 15 km 地図。SSOT 3.4）
- 静的テスト：`tests/test_historical_events.py`。Lupa のテスト：`tests/test_historical_events_lua.py`（ゲームプレイ側と助言ポップアップの UI）

## 動き

1. 毎ターンの開始（`GameEvents.OnGameTurnStarted`）に、全出来事 × 生存中の主要勢力を調べる
2. 対象勢力（`JAC_HistoricalEventTargets`。一覧の `affected_factions`。`ALL` は行なしで全主要勢力）のうち、まだ効果を受けていない勢力を、次の順に判定する
   - 前提の技術・社会制度（`JAC_HistoricalEventPrereqTechs`／`JAC_HistoricalEventPrereqCivics`。一覧の `earlier_by`）の**いずれか1つ**を持っている → 出来事の効果を与える（`reason=prereq`。研究を先に終えた勢力は年より早い）
   - 前提の無い出来事（6件：伊達氏天文の乱・大友二階崩れ・大寧寺の変・駿河侵攻・朝倉氏の滅亡・武田氏の滅亡）で、史実の年（と月）のターンに達した → 効果を与える（`reason=due`）
   - 前提があり、年に達したが未研究 → 出来事のひらめき（`BoostTech`／`BoostCivic`。一覧の `boost`）**だけ**を与える（`reason=year`。1勢力1回）。効果は、その勢力が後で前提のどれかを研究したターンに `reason=prereq` で与える
3. 効果を与えるとき（`prereq`／`due`）
   - フラグ：プレイヤーのプロパティ `JAC_EVENT_FIRED_<EventType>` に与えたターンを書く。これが「1勢力1回」の記録で、セーブに残る
   - 効果（モディファイア）：`JAC_HistoricalEventModifiers` のモディファイアを付ける。一覧の `gameplay_effect` はまだ文章だけで数値が無いため、今は行が無い。発生回数 `JAC_EVENTS_FIRE_COUNT` を1増やす
4. ひらめきだけのとき（`year`）
   - 出来事のひらめきを発動する。ログの `boost=` に `tech=boosted`／`civic=boosted`（発動）、`already`（既に発動済み）、`no_boost`（`Boosts` の行が無い）か `error:…` を出す
   - 発動できた（`boosted`／`already`／`no_boost`）ときだけ、プロパティ `JAC_EVENT_BOOSTED_<EventType>` に年が来たターンを書く（効果のフラグは書かない）。`error` のときは書かず、次のターンにもう一度試す。通知も発動できたターンに1回だけ送る
5. どちらの場合も行うこと（出来事そのものは史実の年に起きるため）
   - 家名：下の「家名の段階」
   - 資源：置かない。資源 MOD が地点の年か地点の研究で置き、出来事はその研究へのひらめきを与える（下の「資源を伴う出来事」）
   - 通知：`NotificationTypes.USER_DEFINED_1` で本人に送る（`NotifyAll=1` なら全主要勢力。生成した61件はすべて0）。見出しは出来事名、本文は助言の文（助言の無い出来事は共通の「史実の出来事が起きた。」）。家固有の出来事（`House`。一覧の `house_specific`）は、その家には固有名、ほかの家には一般名（`GenericName`）を出す（SSOT 3.2）。ひらめきの後に研究して効果を受けたときは、同じ通知を2度は送らない。文面は LOC キーのまま渡し、各端末の言語で表示する
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
| `JAC_HistoricalEventRenames` | `EventType`、`CivilizationType`、`Name`、`CivicType` | 文明の表示名の切替。文明 ID は変えない。`CivicType` が空なら出来事が起きたとき（研究か年）、あれば家がその社会制度を研究したときだけ切り替える（下の「家名の段階」） |
| `JAC_HistoricalEventResources` | `EventType`、`SiteId`、`Action`、`OwnerTo` | すべて**記録だけ**で、Lua は使わない。`reveal`：資源 MOD が開示する予約地点（資源 MOD の地点 ID。出来事は開示しない：SSOT 3.2）。`trade_access`（平戸の硝石）は研究（C19）で解放する決まりで、交易アクセスの仕組みは未実装。`ownership`（石見銀山・奥出雲の鉄を毛利へ）は所有を自動では移さない（旧領は AI の攻略目標だけ。下の「旧領と AI の攻略目標」） |

生成の対応：`affected_factions` → 対象、`earlier_by` → 前提、`boost` → `Boost*`、`house_specific`・`house`・`generic_name` → `House`・`GenericName`、`advisor_popup` → `Description`・`Advisor=1`、`rename` → 家名（`rename.by` → `CivicType`。出来事の `boost` と同じ社会制度でなければ生成を止める）、`resource` → 資源（地点 ID は `japan-map-resources.json` の座標で引き、`reveal` は資源 MOD の予約地点に無ければ生成を止める）。`gameplay_effect`・`year_end`・`regions`・`sources` などは生成に使わない。

開始時の知識（[sengoku-starting-knowledge.md](sengoku-starting-knowledge.md)の技術4・社会制度3）は、全勢力が1534年の開始時から持つため、前提にも Boost にも使わない。前提にすると全勢力でターン1に `reason=prereq` で起きてしまい（実機 sengoku-events-list-01：島津貴久の薩摩統一戦と天文の大飢饉が、シナリオの強制より前にターン1で22家に発生）、Boost にすると無駄になる。一覧（JSON）から外し、生成スクリプトは開始時の知識を付与する `JAC_Sengoku_StartingKnowledge.lua` から同じ一覧を読んで、含む出来事があれば生成を止める。外した出来事は次のとおり。

| 出来事 | 外した前提 | 残った前提 | Boost |
|---|---|---|---|
| 島津貴久の薩摩統一戦 `EV_SHIMAZU_UNIFY_1` | C02 国人盟約 | C10 軍役帳 | C02 → C10 |
| 天文の大飢饉 `EV_TENBUN_FAMINE` | T01 用水普請 | T07 兵糧保存 | T07（変更なし） |
| 伊達氏天文の乱 `EV_TENBUN_DATE` | C02 国人盟約 | なし（史実の年だけで起きる） | C02 → なし |
| 大寧寺の変 `EV_OUCHI_DAINEIJI` | C02 国人盟約 | なし（史実の年だけで起きる） | C02 → なし |
| 四万十川の戦い `EV_SHIMANTO` | C02 国人盟約 | C10 軍役帳、T10 | C10（変更なし） |
| 上杉謙信の死去と御館の乱 `EV_KENSHIN_DEATH` | C02 国人盟約 | C11 | C11（変更なし） |

ゲームプレイ側の Lua には、開始時に持っていた前提を無視する仕組みは入れていない（開始時の知識を各勢力のプロパティに記録して保存・ロードをまたいで持つ必要があり、データ側で防げるため）。`tests/test_historical_events_lua.py` が実際の付与スクリプトを動かしてターン1に何も起きないことを確かめる。

`Boosts` の行が無いとひらめきは発動できないため、ツリー側に行の無い29件の技術・社会制度に仮の行（`BOOST_TRIGGER_NONE_LATE_GAME_CRITICAL_TECH`、40%、`LOC_JAC_EVENT_BOOST_PLACEHOLDER`）を足している（**暫定**）。この種類は公式でも自然には満たされない（`Base/Assets/Gameplay/Data/Technologies.xml:440,505`）ので、出来事のひらめき以外で発動しない。以前の仮の行（ガレー船の生産で発動）は、まとめてガレー船1隻で発動してしまうため替えた。ツリー側に行がある T09・T10 は足さない。実際のひらめき条件はツリー側で決める。

## 暦との関係

Civ6 の公式 Lua には年を数値で返す API が無く、暦は表示用の文字列（`Calendar.MakeYearStr`）しか出さない。公式の Pirates シナリオもターン数から年を計算している。そこで、暦を作っている表そのものから年をターンに換算する。

- 暦は main にマージ済みの戦国パックの暦（[sengoku-calendar.md](sengoku-calendar.md)、PR #34）。`ERA_ANCIENT` の開始年が1534、ゲーム速度ごとに `GameSpeed_Turns`（`MonthIncrement` か月/ターンを `TurnsPerIncrement` ターン）が順に並ぶ
- 換算（`Scripts/JAC_HistoricalEvents.lua` の `TurnForYear`）：ターン1を開始年の1月とし、ターン `t` の手前までに経過した月数が `(年 - 開始年) × 12 + (月 - 1)` に達する最初のターン。開始年は `GameInfo.StartEras['ERA_ANCIENT'].Year`、速度は `GameConfiguration.GetGameSpeedType()`、区間は `GameInfo.GameSpeed_Turns()` を速度で絞り、`MonthIncrement` の大きい順（同じなら `TurnsPerIncrement` の小さい順）に並べ替えて読む（速度は `InGameTopOptionsMenu.lua:442` と同じ取り方）。エンジンは区間を行の挿入順（`GameSpeedTurnCollection` の `ORDER BY rowid`、`01_GameplaySchema.sql:3337`）で使うが、Lua からは rowid が見えず、`GameInfo` の列挙順も保証されない。公式の `GameSpeeds.xml` も戦国の暦も、各速度の区間を1ターンの月数が長い順に並べているので、この並べ替えは挿入順と一致する（静的テストで戦国の暦の並びを固定し、Lupa のテストで行を逆順に渡しても同じターンになることを確かめる）。月数が後で長くなる区間を持つ暦を足すと、この前提が崩れる
- エンジンの現在ターンは1から始まらない。1534年開始の暦では `Game.GetCurrentGameTurn()` と `GameConfiguration.GetStartTurn()` がともに 16603 から始まる（実機、2026-09-29。暦の文字列は「1534 AD」）。そこでスクリプトは、公式の上部パネルが「ターンを1から数える」ために行う `現在 - GetStartTurn() + 1`（`Base/Assets/UI/TopPanel.lua:390`）を `GameTurn()` に取り、上の換算・発生ターンのプロパティ・ログはすべてこの経過ターンで扱う。開始ターンはセーブに含まれる設定なので、セーブ・ロードでも速度が違っても変わらない
- 標準速度（4か月/ターン＝1年3ターン）では、月が無い出来事は `ターン = (年 - 1534) × 3 + 1`。例：天文の大飢饉1540年はターン19、鉄砲伝来1543年9月はターン30、長尾→上杉1561年4月はターン83、本能寺の変1582年6月はターン147、豊臣姓の下賜1586年10月はターン160。61件はすべて200ターン以内
- ほかの速度も同じ表で自動的に換算する。1年あたりのターン数は Online 1.5、Quick 2、Epic 4（200ターンまで。以後6）、Marathon 6（200ターンまで。以後12）
- 暦の月（SSOT 3.2）：`Month` は一覧の `month`。一覧は史料の和暦の日付を西暦に換算した月（1582年10月4日まではユリウス暦、以後はグレゴリオ暦。一覧の `_schema.month_note`）を持つので、そのまま使う。画面は年だけを表示する（SSOT 3.2）ので、月は発生ターンにだけ効く
- 静的テスト（`tests/test_historical_events.py`）は、暦の XML から同じ換算を Python で再計算し、全速度で「換算したターンの暦上の年が出来事の年で、その前のターンは前の年」と「月を指定したら、そのターンで初めてその月に達する」を確かめる

## 家名の段階

長尾→上杉（`EV_NAGAO_TO_UESUGI`、1561年4月）、松平→徳川（`EV_MATSUDAIRA_TOKUGAWA`、1567年1月）、織田→羽柴（`EV_HONNOJI`）→豊臣（`EV_TOYOTOMI_SURNAME`）。

- どの家も研究による襲名で切り替える（`CivicType` は必須）。史実の年は切り替えず、その社会制度への着想だけを与える。出来事の別の前提で出来事が起きても、襲名の社会制度が無ければ切り替えない
- 長尾・松平：社会制度 C16（朝廷・将軍への取次）を研究したら上杉・徳川（オーナー判断 2026-09-30、Discord の確認事項Q「1a」。以前の 2a「1561年、または社会制度を先に研究」を置き換えた）。C16 は両方の出来事の既存の前提（開始時の知識ではない）で、年のひらめきの対象でもある。長尾→上杉の別の前提（C11・C30）で出来事が起きても切り替えない
- 織田：研究による襲名（オーナー判断 2026-09-30、SSOT 3.2「Civ6 のような流れで、研究で襲名を解放したら変わる」）。社会制度 C16（朝廷・将軍への取次）を研究したら羽柴、C24（停戦裁定。関白としての惣無事）を研究したら豊臣。本能寺の変（1582年6月）と豊臣姓の下賜（1586年10月）の年は切り替えず、その社会制度への着想だけを与える。本能寺の変の別の前提（C11）で出来事が起きても、C16 が無ければ切り替えない。ターン開始ごとに、出来事の判定の後で各家の研究を見て切り替える（`RenameByResearch`。切り替えたときだけ `[JAC_EVENTS] renamed civ=… civic=…` をログに出す）
  - 社会制度の選び方：ツリー設計案に襲名そのものの節点は無いので、既存の社会制度から選んだ。C16 は朝廷・将軍への使節で権威を得る節点（秀吉の羽柴期の朝廷工作）、C24 は惣無事（関白の権能）を表し、前提に C16 を持つので羽柴を飛ばして豊臣にならない。新しい「襲名」の仕組みは足していない
- どちらも Game プロパティ `JAC_CIV_NAME_<CivilizationType>` に表示名の LOC キーを書く。CustomCivs の `UI/JAC_CivNameOverride.lua` がこれを読み、外交リボン・外交画面・ワールドランキング・都市バナーの文明名を差し替える（既存の仕組み。[house-names.md](house-names.md)）
- 段階は研究の順番で前後しうる（例：C24 を C16 より先に得た場合）。そこで Game プロパティ `JAC_CIV_NAME_STAGE_<CivilizationType>` に表示中の段階の予定ターン（出来事の年月のターン）を残し、それより前の段階では上書きしない。最後の段階が残る
- 家名は日英露：上杉 Uesugi Уэсуги、徳川 Tokugawa Токугава、羽柴 Hashiba Хасиба、豊臣 Toyotomi Тоётоми（一覧は日本語だけなので、生成スクリプトに対応表を置く）
- 豊臣期の出来事「大坂城の築城」「北野大茶湯」は織田家の固有名で、ほかの家には一般名（巨大城郭の築城・大茶会）を出す。織田が羽柴・豊臣に切り替わる前でも後でも同じ（オーナー判断 2026-09-30。旧 Q40 の①）

## 旧領と AI の攻略目標

オーナー判断（2026-09-30、SSOT 3.2）：出来事で移った旧領（武田領の分割、北条領の徳川への移封など）は、城・資源の所有を自動で移さず、AI の攻略目標にだけ使う。

- 一覧の `ai_targets`（家 → その家の AI が攻略目標にする国）に8件を記録した：江口の戦い（三好：摂津・山城）、上杉憲政の越後亡命（北条・長尾：上野）、石見銀山（毛利：石見）、尼子氏の滅亡（毛利：出雲・伯耆）、上洛（織田：近江・山城・摂津）、朝倉氏の滅亡（織田：越前）、武田氏の滅亡（織田：甲斐・信濃、松平：駿河）、関東移封（松平：武蔵・相模）。資源の `ownership` の移転先は必ずこの中にある（テストで照合）
- **AI への反映は今は実装していない。** 公式の AI データに、特定の城（都市）を攻略目標として優先させる仕組みが無いため
  - 都市攻撃の作戦（`Base/Assets/Gameplay/Data/Operations.xml:138-139` の `Attack Enemy City`／`Wartime Attack Enemy City`）は目標の種類 `TARGET_ENEMY_COMBAT_DISTRICT` で、目標の選び方は距離・勝算（`MaxTargetDistInArea`、`MinOddsOfSuccess`）だけ。都市を名指しする列は無い
  - 疑似産出（`Base/Assets/Gameplay/Data/Yields.xml:16-19` の `PSEUDOYIELD_CITY_BASE`／`PSEUDOYIELD_CITY_ORIGINAL_CAPITAL` など。使い方は `Leaders.xml:818`、`Victories.xml:280`）は指導者ごとの「都市全般・元の首都全般」の重みで、特定の都市や国は選べない
  - 目標を Lua で渡す `TARGET_SCRIPT_SUPPLIED`（`Operations.xml:22`「シナリオ用。作戦の定義の Lua を呼ぶ」）はあるが、公式の使用例は2つだけで、どちらもシナリオ専用の行動ツリーで1地点へ移動させるもの（`DLC/CivRoyaleScenario/Data/CivRoyaleScenario_AI.xml:13` と `Scripts/CivRoyaleScenario_StartScript.lua:755-786,1607`、`DLC/PiratesScenario/Data/PiratesScenario_AI.xml:41`）。関数は `PlotX`／`PlotY`／`Extra` を返すだけで、どの AI の作戦かを受け取らないため、家ごとの目標にできない。都市攻撃の行動ツリーと組み合わせた公式例も無い
- 仕組みが見つかったとき（または独自の AI 誘導を作るとき）に、この一覧を使う。城・資源の所有を出来事で移すことは今後もしない

## 助言ポップアップ

オーナー判断 E5a（和睦などを薦める出来事は、ひらめきを与え、助言役のポップアップで知らせる）。一覧の19件が助言の文を持つ（`Advisor=1`）。

- 公式の助言ポップアップ（`Base/Assets/UI/Popups/AdvisorPopup.lua`）をそのまま使う。`TutorialUIRoot.xml:21` がこのコンテキストを置き、通常のゲームでも読み込まれる。`LuaEvents.TutorialUIRoot_AdvisorRaise`（`AdvisorPopup.lua:521`）に `AdvisorItem`（`:25-38`）を渡すと `ShowOrQueuePopup` → `ShowAdvisorPopup`（`:94`）で出る。表示中に別の助言が来たら公式のキューに入り、閉じた後に出る
- 変換（アダプター）は `UI/JAC_HistoricalEventsAdvisor.lua`（`InGame` に足す、部品の無いコンテキスト）。公式のチュートリアルと同じく `hstructure AdvisorItem` を宣言して `hmake` で組み立てる（`TutorialUIRoot.lua:38-52,216-229`。公式の FIXME のとおり、宣言は使う側の各ファイルに同じものを置く）。本文は「出来事名＋改行＋助言の文」を `Locale.Lookup` した文字列（`TutorialUIRoot.lua:366` と同じく引いてから渡す）。ボタンは1つ（「承知した」）で、押すと閉じるだけ。コールバックを自前で持つので、チュートリアルの `AdvisorPopup_ClearActive` は呼ばれない。助言役の顔（`ShowPortrait=true`）付きの吹き出しで出る
- 出すのはローカルのプレイヤーだけ：`Game.GetLocalPlayer()` のプレイヤーのフラグ（`JAC_EVENT_FIRED_`／`JAC_EVENT_BOOSTED_` の早い方）が今のターンの出来事だけを出す。フラグは対象の勢力にしか付かないので、AI の出来事やほかの端末のプレイヤーの出来事は出ない。UI はゲームの状態を書かない
- 出すきっかけは `Events.LocalPlayerTurnBegin`（ターン開始の処理でフラグが付いた後）と `Events.LocalPlayerChanged`（オートプレイから人間に戻ったとき）。公式の `ActionPanel.lua:1333-1334` と同じ2つ。同じ出来事は1セッションで1回だけ出す。オートプレイ中はローカルのプレイヤーがいない（`-1`）ので出さない
- 自動操作を止めない：ポップアップは公式のコンテキストなので、ドライバの既存の閉じ処理（`scenario.py` の `_dismiss_advisor`。`IsBlockingInput()` のとき `OnHideAdvisorDialog()`）がオートプレイ・ターン終了・ロードの前後で閉じる。新しいポップアップのコンテキストは足していないので、ドライバの変更は要らない
- ひらめきは、年が来たときに未研究の勢力へ `reason=year` で与える（上の「動き」）。先に研究していた勢力には、ひらめきは無く、ポップアップと通知だけが出る
- 和睦を薦める10件を1534年の因縁に対応づけて因縁を解消する処理は、因縁の実装側で行う（設計 §2.6）。この PoC は知らせるだけ

## 資源を伴う出来事

オーナー判断（2026-09-30、SSOT 3.2）：出来事は資源を置かない。出来事の年月が来たら、その資源が現れる研究へのひらめきを与え、資源は研究の完了か指導者の能力などで現れる。資源は main の資源 MOD（[resources.md](resources.md)、`poc/JAC_PoC_JapanMapScale/Scripts/JAC_Resources.lua`）だけが、予約した地点に、地点の年か、いずれかの勢力の地点の研究（資源配置の `REVEAL_TECH`）の完了（初期知識を除く）で置く。出来事のスクリプトは `RevealSite` を呼ばず、`JAC_HistoricalEventResources` も読まない（記録だけ）。

| 出来事 | 地点 | 年（出来事／地点） | ひらめき（地点の研究） |
|---|---|---|---|
| 生野銀山 `JAC_EVENT_IKUNO_SILVER` | `silver-ikuno` | 1542／1542 | T09 灰吹法（以前は T06） |
| 佐渡平定 `JAC_EVENT_SADO_SUBJUGATION` | `gold-sado-nishimikawa`（15 km のみ。SSOT 3.2） | 1589年8月／1593 | T14 鉱山経営（変更なし） |
| 会津の漆 `JAC_EVENT_AIZU_LACQUER` | `lacquer-aizu` | 1590年9月／1590 | 地点に研究が無いので C15 のまま。漆は1590年に現れる |
| 相川金銀山 `JAC_EVENT_AIKAWA` | `gold-sado-aikawa` | 1596／1596 | T14 鉱山経営（以前は T09） |

- 読み方の選択：オーナーの答えは佐渡平定についてだが、「砂金が現れる研究にひらめき、研究の完了か指導者の能力などで現れる」は資源の出し方の決まりとして読み、資源を伴う4件すべてに当てた。出来事の年と地点の年が同じ3件でも、出来事は前提（`earlier_by`）のどれかを先に研究した勢力に早く起きるため、以前は地点の研究より前に資源を置けた（例：T06 だけを得た家の生野銀山）。これは[史実イベント一覧](../research/sengoku-events.md)の「`earlier_by` のうち地点の研究以外は資源を早く出さない」とも食い違っていた。すべてを「ひらめきだけ」にすると、どの資源も地点の年か地点の研究より早くは現れない
- 地点の研究がある地点では、出来事のひらめきをその研究にする（テストで照合）。そのため生野銀山と相川金銀山のひらめきを替えた。地点の研究はどれも出来事の前提にも入っている
- 資源 MOD は日本マップ4種でだけ読み込まれる。ほかの地図でも出来事の動きは変わらない（ひらめきと通知だけ）
- 石見銀山（1526年）は開始時から地図に置く（SSOT 3.2、資源 MOD）ので、開示の対象ではない（毛利への所有は記録だけ）
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
| 2a | 長尾→上杉は「1561年、または関連する社会制度を先に研究」の条件を維持する | 下の 2026-09-30（確認事項Q）で置き換え。年では切り替えない |
| 6a | 松平→徳川の改名も、長尾→上杉と同じ仕組みの出来事にする | 実装済み（1567年1月） |
| 2026-09-30 | 松平の改名も長尾と同じく研究で早まる（SSOT 3.2） | 下の確認事項Q で研究だけに変更 |
| 2026-09-30 | 資源を伴う出来事は資源を置かず、資源が現れる研究へのひらめきを与える（SSOT 3.2） | 実装済み（上の「資源を伴う出来事」） |
| 7a | 本能寺の変の後、織田家の家名表示を羽柴から豊臣へ切り替える（同じ仕組み） | 実装済み。切替の時期は下の 2026-09-30（研究による襲名） |
| 8a | 室町幕府が終わる1573年、足利家の勢力は残り、将軍として裁定する権能だけを失う | 出来事 `JAC_EVENT_MUROMACHI_END` は通知とひらめきだけ。勢力は消さない。裁定の権能は停戦裁定の MOD 側（[court-arbitration.md](court-arbitration.md)）が扱う |
| 10a | 関ヶ原の戦いの出来事は置かない | 一覧に無い（テストで固定） |
| E6b | 史実イベントは暦の月までできるだけ合わせる（SSOT 3.2） | 実装済み（`Month` 列） |
| E5a | 和睦などを薦める出来事は、ひらめきを与え、助言役のポップアップで知らせる（SSOT 3.1） | 実装済み（上の「助言ポップアップ」）。因縁の解消は因縁の実装側 |
| 2026-09-30（旧 Q40） | 大坂城の築城・北野大茶湯は織田家の固有名、ほかの家は一般名 | 実装済み（`House`） |
| 2026-09-30（旧 Q41） | 楽市は六角の石寺新市（1549年12月、C15）と織田の加納（1567年11月、C17）の別の出来事にする | 実装済み（`EV_ROKKAKU_RAKUICHI`・`EV_RAKUICHI`） |
| 2026-09-30（旧 Q42） | カトリックは研究（C19／C13）した家から広がる。地域の固定順は使わない | 実装済み（キリスト教の伝来は前提のある家に早く起き、年には未研究の家に C19 の着想だけ）。宗教の中身は 9章の実装側 |
| 2026-09-30（旧 Q43） | 旧領は AI の攻略目標にだけ使い、所有は自動で移さない | 記録だけ（`ai_targets`、`Action='ownership'`）。AI への反映は公式の仕組みが無く未実装（上の「旧領と AI の攻略目標」） |
| 2026-09-30（旧 Q55） | 硝石の交易アクセスは Civ6 のように研究で解放する | データだけ（資源配置の `unlock_civic` が C19。平戸の出来事の着想も C19）。交易アクセスの仕組みは未実装 |
| 2026-09-30（旧 Q56） | 織田→羽柴→豊臣は研究による襲名で切り替え、史実の年は着想だけ | 実装済み（C16・C24。上の「家名の段階」） |
| 2026-09-30（Discord の確認事項Q「1a」） | 長尾→上杉・松平→徳川も研究による襲名（C16）だけで切り替え、史実の年は着想だけ。2a を置き換える | 実装済み（上の「家名の段階」。テストで固定） |

## マルチプレイとセーブ

- 状態はすべて Player／Game のプロパティにあり、Lua の変数にターンをまたいで持たない。ロード後もプロパティから同じ判定になり、同じ出来事は2度起きない（Lupa のテストでスクリプトを読み直して確かめ、実機シナリオでもセーブ→ロード→1ターンで確かめる）
- 判定はゲームプレイ側のターン開始イベントだけで行い、出来事は「予定ターン→ID」、勢力は ID 順に処理する。乱数・ローカルプレイヤー・端末の言語に依存しない
- 家名は Game プロパティ（同期済みの共有状態）から UI が読む。端末ごとの設定値は使わない（SSOT 3.1）
- 助言ポップアップは各端末が自分のプレイヤーのフラグを読んで出す（UI だけで、ゲームの状態は変えない）。どの出来事を出したかは端末のメモリにだけあるので、出来事が起きたターンのセーブをロードすると、そのターンの助言はもう一度出る。既読の印はプレイヤーごとなので、ホットシートで同じターンに同じ出来事が起きた2人目の人間にも助言が出る
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
| ゲーム中の資源配置 | 資源 MOD だけが置く（根拠は [resources.md](resources.md)）。出来事は地点の研究へのひらめきだけ |
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
| 開始直後 | `Lua.log` に `[JAC_EVENTS] loaded events=61 startYear=1534 speed=GAMESPEED_STANDARD`。ターン1で出来事は1件も起きていない |
| ターン1に強制 | 織田（人間）に技術「兵糧保存」、長尾家に社会制度「朝廷・将軍への取次」、島津家（AI）に社会制度「軍役帳」を与える（「国人盟約」は開始時の知識のため前提から外した）（GameCore_Tuner） |
| 1ターン進めた後 | 織田に天文の大飢饉（助言あり）、長尾家に上杉継承、島津家に薩摩統一戦（助言あり）がターン2で起きる。長尾家の表示名が上杉（強制した C16 による襲名）。織田は改名しない。読み込んだ `JAC_HistoricalEventRenames` の4段階がすべて `CivicType` を持つ（研究だけで切り替わる）。発生回数と全フラグの数が一致（2度起きていない） |
| 助言ポップアップ | `JAC_HistoricalEventsAdvisor` の `JAC_EVENTS_ADVISOR_SHOWN` に `JAC_EVENT_TENBUN_FAMINE@2#0`（出来事@ターン#プレイヤー）があり、島津・長尾（AI）の助言は無い。公式の `AdvisorPopup` が入力を止めて表示中で、本文が大飢饉（または天正地震）の助言の文。スクリーンショット `advisor-popup.png` を撮ってから閉じる |
| セーブ→メニュー→ロード | 同じ確認がロード直後にも通る |
| ロード後に1ターン | 各フラグがターン2のまま（2度起きていない）。家名の切替も残る |
| ターン19まで進める | 暦が1540年。兵糧保存を未研究の勢力に `JAC_EVENT_BOOSTED_JAC_EVENT_TENBUN_FAMINE`=19 と兵糧保存のひらめき。織田はターン2の効果のまま。伊達家は塵芥集（1536年5月＝ターン8）で社会制度「分国法」の着想（`GetCulture():TriggerBoost`）を受けたか、先に研究して効果を受けている |

判定は `generic`（`Lua.log` の JAC 由来のエラーが無いこと）。

**実機の結果（2026-09-30、sengoku-events-list-03）：** 上の手順すべて合格。60件を読み込み、強制した前提で織田に天文の大飢饉が起き、人間に公式の `AdvisorPopup` で出来事の文が出た（スクリーンショットで確認）。長尾家の上杉への改名が世界ランキングに出て、島津家（AI）の出来事では助言が出なかった。セーブ→ロード後に再発火は無し。ターン19（1540年）で16勢力に年のひらめき。伊達家はターン8に社会制度の着想を受けた（`PlayerCulture:TriggerBoost` の初の実機確認）。この実行の後、既読の印をプレイヤーごとにした（`@2` → `@2#0`）ので、シナリオの確認は新しい形で次の実行で通す

**要実機確認（シナリオで確かめること・確かめないこと）：**

1. （2026-09-30 合格）助言ポップアップが、オートプレイから人間に戻ったとき（`Events.LocalPlayerChanged`／`LocalPlayerTurnBegin`）に実際に出るか。自前の `hstructure AdvisorItem` を公式のコンテキストに渡して型の不一致にならないか（公式のチュートリアルも別ファイルの同じ宣言で渡している）。シナリオの「助言ポップアップ」の手順で確かめる
2. （2026-09-30 合格）社会制度の `GetCulture():TriggerBoost` と、仮の `Boosts` 行（`BOOST_TRIGGER_NONE_LATE_GAME_CRITICAL_TECH`）を社会制度に使えるか。シナリオの伊達家（ターン8）で確かめる（伊達家が先に研究した場合は通らない）
3. 通知の見た目。`ICON_NOTIFICATION_USER_DEFINED_1` に `ICON_ATLAS_NOTIFICATIONS` の15番（公式 Black Death と同じ）を当てたが、絵柄が出来事に合うかは画面で見る
4. 資源の開示は資源 MOD の実機確認（`japan-resources.json`）と同じ。シナリオは資源の出来事を通らない
5. 標準以外の速度での暦の換算（区間の並び順の前提。上記「暦との関係」）
6. マルチプレイでの同期。2クライアントで、出来事の発生（フラグ・ひらめき）と家名の切替が両方の端末で一致するか、セーブ→ロード後も一致するかを確かめる（単独プレイのシナリオと Lupa のテストでは確かめられない）
7. 織田の研究による改名（C16 で羽柴、C24 で豊臣）。シナリオはターン19までなので、読み込んだ表に `CivicType` があることだけを確かめる。切り替えは Lupa のテストで確かめている
