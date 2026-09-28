# Civ6 MOD 実装着手の準備度調査（2026-09-28）

> 2026-09-28 追記：本調査を受けてオーナーが決めた事項（アジア圏マップと Civ6 既定の世界マップ、外部連携用の常駐アプリ、勢力体とプレイヤー枠、レベル上限、ゲーム速度の倍率）は SSOT に反映済み。本文中の「Q1〜Q3」などの番号は調査時点のもので、現在の質問リストとは一致しない。

SSOT（[`documents/spec/ssot/`](../spec/ssot/)）と手元の Civ6 環境、Civ6 MOD の技術情報を突き合わせ、実装に着手できるかを判定した調査記録。仕様の決定はここに書かない。決定が必要な事項は [質問リスト](../questions.md) に集約した。

## 結論

| 対象 | 判定 | 理由 |
|---|---|---|
| 19.3 の最小技術検証（読み込み・保存、指導者の人物化、暗殺者のタイマー、架空タイルでの領土関係） | **着手できる** | 手元に Civ6 と全拡張がそろっている。参考になる既存 MOD とゲーム本体のソースがある。未決仕様に触れない範囲で検証できる |
| 16.1 PoC の全項目 | **一部は着手できない** | 外部保存・宇宙・襲来・遠征などは成功条件が無い。外部保存は Civ6 の制約で構成の見直しが要る |
| 16.2 MVP | **着手できない** | 勢力・城・武将の名簿、地図範囲、経験値式、コンボ・探索・装備の中身、勝利判定が未定 |
| 16.4 Phase 2 世界完全版 | **現行仕様のままでは実現できない** | 851主体を個別のプレイヤーにする前提が、エンジンの上限62を大きく超える |

19.3 の検証を先に進めつつ、[質問リスト](../questions.md) の「PoC前に必須」「MVP前に必須」を順に決めていけば、MVP 実装へ移れる。

## 1. 手元の環境（実機で確認済み）

| 項目 | 状態 |
|---|---|
| Civ6 本体 | Steam 版（AppID 289070）。`C:\Program Files (x86)\Steam\steamapps\common\Sid Meier's Civilization VI`。実行ファイルの版は 1.0.12.68（2024-06-26） |
| 拡張・DLC | Rise and Fall（`Expansion1`）、Gathering Storm（`Expansion2`）、New Frontier Pass 系と各種シナリオ・リーダーパックを含む37件 |
| 公式開発ツール（SDK、AppID 404350） | 導入済み（2026-09-28 時点）。Cooker（`AssetModTools\Cooker`）をコマンドラインで動かして BLP を作れることを確認した（`tools/jac_art/build_art.py`） |
| ユーザーデータ | `Documents\My Games\Sid Meier's Civilization VI\`（`Mods`・`Saves` あり。`Mods` は空） |
| ログ・設定 | `%LOCALAPPDATA%\Firaxis Games\Sid Meier's Civilization VI\`（`Logs\`、`AppOptions.txt`、`Mods.sqlite`） |
| デバッグ設定 | `AppOptions.txt` は `EnableTuner 0`、`EnableDebugMenu 0`、`EnableWorldBuilder 1` |
| Steam Workshop | 28件を購読中。Lua で暗殺者を実装した MOD が2件ある（下記） |
| 空き容量 | C: 351GB、D: 504GB |
| その他 | Civ7 もインストール済み（本プロジェクトの対象外） |

Web 上の資料にはログの場所を `Documents\My Games\...\Logs` とするものがある。しかしこの環境では、ログは `%LOCALAPPDATA%` 側に出ていた。

### 参照に使えるローカル資料

| 資料 | 場所 | 使い道 |
|---|---|---|
| ゲームデータ定義（XML 82件） | `Base\Assets\Gameplay\Data\`、`DLC\Expansion1\Data\` など | テーブル構造・既存 ID の確認。Governor は Expansion1 側 |
| 公式 UI・ゲームプレイ Lua（641件） | `Base\Assets\UI\`、各 DLC の `UI\`・`Scripts\` | Lua API の実際の使い方。公開 API の網羅的な公式文書は無いので、これが一番確かな資料 |
| 既存マップ（`.Civ6Map`） | `Base\Assets\Maps\EarthMaps\` など | `.Civ6Map` は SQLite。`Plots`・`PlotOwners`・`StartPositions`・`Cities`・`Players` などのテーブルを持つ（`WorldMapHuge` は 130×66、8,580 タイル） |
| 暗殺者 MOD（Workshop 3662073624「Assassin Unit」） | `steamapps\workshop\content\289070\3662073624\` | UI から `UI.RequestPlayerOperation(..., PlayerOperations.EXECUTE_SCRIPT, ...)` で命令を出し、ゲームプレイ側の `GameEvents` で受けて `UnitManager.Kill` で倒す。状態は `Game:SetProperty` で保存している。マルチプレイでも使われる形の UI→ゲームプレイ連携の実例 |
| 暗殺者 MOD（Workshop 2889676774） | 同上 `2889676774\` | `ExposedMembers` 経由で UI からゲームプレイ側の関数を呼ぶ別方式 |
| その他 | `3409803446`（City and Town Mode）、`3102595288`（Civilization Paragons）、`3170879482`（日本の指導者追加） | 都市・町の区別、人物系の仕組み、日本の指導者追加の実例 |

## 2. Civ6 MOD の技術的制約（SSOT への影響順）

「確認済み」は Firaxis の一次情報・手元のゲームファイル、または複数の信頼できる情報源で裏付けたもの。「推定」は単一または古い情報源。

### 2.1 プレイヤー数の上限は62（確認済み）

- エンジンのプレイヤー枠は64。蛮族と自由都市に1枠ずつ予約されているので、メジャー文明と都市国家の合計上限は62（[YnAMP ConfigText.xml](https://github.com/Gedemon/Civ6-YnAMP/blob/master/Configuration/ConfigText.xml)、[CivFanatics](https://forums.civfanatics.com/threads/the-max-amount-of-civs-city-states.637230/)）。DLL 側の制約で、SQL/XML では変えられない
- 素のゲームの Huge サイズは、メジャー文明20・都市国家24が上限（手元の `Base\Assets\Configuration` で確認済み）
- 20〜60文明規模での定量的な処理時間データは見つからなかった（推定：ターン処理が大幅に遅くなる）

**SSOT への影響**：16.4「各国最低1 Leader」、3.3、14.4 の「851主体を国として扱う」前提は、1主体＝1プレイヤーでは実現できない。17.16 で「PoC で検証」とされている事項は、結論が先に出た。大半の主体を**プレイヤー以外のデータ層**（都市・地域・関係データ）で表す設計が必要（[質問 Q1](../questions.md)）。

### 2.2 地図の大きさの上限（確認済み）

- 公式の最大は Huge 106×66（手元の `Maps.xml` で確認済み）。付属の `WorldMapHuge.Civ6Map` は 130×66
- YnAMP の区分では、Enormous 128×80 が安定、Giant 180×94 は終盤が不安定、230×116 は作者が「フルサイズではロードできない」と警告している。Gathering Storm の海面上昇処理は、大きい地図で回復不能なクラッシュを起こす（[YnAMP ConfigText.xml](https://github.com/Gedemon/Civ6-YnAMP/blob/master/Configuration/ConfigText.xml)）

**SSOT への影響**：3.3・16.4 の「47都道府県を表現する超巨大世界地図」は、128×80 級の上限と両立しにくい。日本だけで数十タイル幅が必要になる（[質問 Q2](../questions.md)）。

### 2.3 ゲーム内 Lua からの外部通信とファイル書き込みは不可（確認済み）

- Civ6 の Lua はサンドボックスで、`io`・`os`・`require` が無効（[Lua Environment and Sandboxing](https://forums.civfanatics.com/resources/lua-environment-and-sandboxing.25425/)、[require の可否](https://forums.civfanatics.com/threads/require-a-lua-package.624171/)）
- 実例がある回避策は、ゲーム外のアプリでセーブファイルを解析する方法だけ（[civ6-savegame-parser](https://github.com/bestander/civ6-savegame-parser)）

**SSOT への影響**：15.1 の `services/vercel-web`・`railway-api`、15.2「メタ進行・友達間共有」、16.1「外部保存」、17.3 の接続と同期は、ゲーム外の常駐アプリがセーブを読んで同期する構成でしか実現できない。MOD 本体とは別製品になる（[質問 Q3](../questions.md)）。

### 2.4 ツールチェーン

- 公式 SDK（AppID 404350）には ModBuddy・FireTuner・WorldBuilder・Asset Editor が入っている。ModBuddy は Visual Studio 2013 Isolated Shell に依存し、新しい環境では入らない報告が続いている（推定。[ModBuddy: Missing Dependency](https://forums.civfanatics.com/threads/modbuddy-missing-dependency.670628/) ほか）
- ModBuddy は必須ではない。`.modinfo`・SQL・XML・Lua をテキストで書き、`Documents\My Games\...\Mods\` に置けば読み込まれる（確認済み）
- 必要になる場面：FireTuner（実行時の Lua コンソール。`EnableTuner 1` が必要）、Asset Editor（アイコン・肖像を XLP/BLP に変換する）、WorldBuilder（地図の手修正）
- ID の重複は `Database.log` に `UNIQUE constraint failed` として記録され、MOD の読み込みが失敗する（確認済み）。SSOT 15.3 の `JAC_` 接頭辞はこの対策として妥当

### 2.5 保存・同期

- `Game`・`Player`・`Unit`・`City`・`Plot` の `SetProperty` / `GetProperty` は、セーブ・ロードをまたいで値を保持する。表（テーブル）も保存できる。書き込めるのはゲームプレイ側のスクリプトだけで、キーは全 MOD 共通の名前空間（確認済み。[sukritact KB](https://sukritact.github.io/Civilization-VI-Modding-Knowledge-Base/About_Get-SetProperty)）
- マルチプレイでは、Lua 内の `math.random` などが各端末でばらばらに評価されると同期ずれ（デシンク）が起きる（確認済み。[CivFanatics](https://forums.civfanatics.com/threads/gameplay-related-lua-scripts-in-multiplayer.626600/)）
- UI からゲームの状態を変える操作は、`UI.RequestPlayerOperation(..., PlayerOperations.EXECUTE_SCRIPT, ...)` でゲームプレイ側へ渡し、`GameEvents` で受けるのが定石（手元の暗殺者 MOD 3662073624 の実装で確認。マルチプレイでの挙動は未検証）

**SSOT への影響**：4〜5章の武将の成長データ（レベル・性格・特性・装備）は `Unit` / `Player` の Property で持てる見込み。マルチプレイに対応するかどうかで、乱数と操作の設計が変わる（[質問 Q5](../questions.md)）。

### 2.6 Governor・偉人・都市国家（確認済み／一部推定）

- Governor は `Governors` / `GovernorPromotions` テーブルで追加できる（12人に増やした MOD の前例がある）。人数の上限は GlobalParameters で変えられる。必要なのは Rise and Fall 以降
- 偉人を起動すると Governor の称号を得る効果は、素のゲームに前例がある（Irene of Athens など）。「特定の偉人を同名の Governor に進化させる」実装例は見つかっていない（推定：データと Lua の組み合わせで作れる）
- 都市国家は MOD で削除できる。「小国懐柔」への置き換えは前例が無く、都市国家の枠を流用するかは PoC で確かめる
- 忠誠度（Loyalty）は Modifier で実質無効にできる。自由都市化の抑止も可能（推定）

### 2.7 人物の殺害・排除

- `UnitManager.Kill` でユニット（偉人ユニットを含む）を消せる（確認済み。手元の暗殺者 MOD）
- Governor を消す、指導者（プレイヤー）を直接倒す書き込み系 API の確証は無い。都市をすべて消して間接的に脱落させる方法だけが議論されている（推定）

**SSOT への影響**：9.5 の「指導者暗殺→四分岐」の実装方法は PoC で確かめる必要がある。Civ6 の Lua だけで「指導者が死亡し、国は存続する」状態を作れない場合は、ゲーム側で指導者を置き換える方式などの代替案が要る。

### 2.8 配布

- Steam Workshop は1ファイル約100MB が上限。大きい MOD は複数に分けて配布する（確認済み。[CivFanatics](https://forums.civfanatics.com/threads/maximum-steam-workshop-size.609188/)）
- マルチプレイでは全員が同じ版の MOD を持っている必要がある（確認済み）
- 実在の国名・宗教名の使用に関する 2K/Firaxis の公式指針は見つからなかった

## 3. SSOT の不足・矛盾（実装前に解消したいもの）

| 節 | 内容 |
|---|---|
| 3.1 / 4.1 / 16.2 | 3.1 は主な勢力に6家（織田・武田・上杉・北条・毛利・島津）を挙げるが、4.1 の Civilization ID 例は北条を除く5家。16.2 の MVP は「6勢力」 |
| 5.1 / 16.2 | 通常のレベル上限は100、MVP は1〜30。MVP 用の上限なのか、式そのものが違うのかが書かれていない |
| 16.1 / 19.3 | 19.3 の暗殺者・領土関係・小国指導者は 16.1 に無い。16.1 の外部保存・宇宙・襲来・遠征には成功条件が無い |
| 17.4 / 19.3 | 17.4 は Governor への進化経路を「確定」とする一方、19.3 は成立するかを検証するとしている。仕様としての決定と、技術的に成立するかは分けて書く必要がある |
| 1.3 / 9.5 | ゲーム速度ごとの「倍率換算」の表が無いが、9.5 の暗殺者のターン数はそれに依存している |
| 16.2 全般 | 城・武将の名簿、地図の範囲とタイル配置、経験値式、ジョブ・スキルツリーのノード、コンボの数値、探索地点と報酬、装備・Artifact・英雄の名簿、吸収方式、宗教の最小版、UI の項目、統一の判定が未定 |

## 4. 推奨する進め方

1. **19.3 の最小技術検証を始める**（判断待ちにならない範囲）
   1. 空の MOD を `Mods\` に置き、読み込み・`Database.log` / `Modding.log` の確認手順を確立する
   2. `JAC_` 接頭辞の文明・指導者を2つ追加し、ID の衝突が無いこととセーブ・ロード後の状態を確かめる
   3. `Unit:SetProperty` で武将のレベル・性格・特性を保存し、ロード後に UI から読めることを確かめる
   4. 暗殺者のタイマー（全国で共有する待機・生産ロック）だけをダミーの任務で確かめる
   5. 偉人を同名の Governor に置き換える経路を1人分だけ確かめる
2. 並行して [質問リスト](../questions.md) の「PoC前に必須」を決める
3. MVP のデータ（勢力・城・武将）は、決まったものから SSOT に表として追加していく

## 調査の方法と限界

- 手元の環境は、レジストリ・Steam のライブラリ・ゲームフォルダ・Workshop・`AppOptions.txt` を読み取って確認した。ゲームの起動や MOD の読み込みはまだ行っていない
- Web の情報は2系統の調査を並列に行い、出典の URL を付けた。公式 SDK の同梱文書は SDK が未導入のため読めていない
- SSOT の過不足は、別のモデルによるレビューと突き合わせ、指摘の中身を原文で確認した
