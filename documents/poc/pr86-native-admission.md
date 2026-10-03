# PR86: 加入人物の本体任命資格（共有ToDo 853）

## 技術判断

**機能本体は未実装。** 現行のSteam版Gathering Stormで、標準Lua・SQLから加入後の人物だけに `Governors.TraitType` の資格を動的に与える接続は確認できなかった。公式任命操作の内部経路は本体 `CanAppointGovernor` 相当のRVA `0x2DD820` を通り、RVA `0x2DDDD0` の人物別資格判定へ末尾分岐する。`AI::Governors` の内部経路にも後者への直接呼出しがある。ただし、実手番でその呼出しがAIの任命判断に使われたことは未証明である。したがって製品用のRust hookや架空のLua setterは書かない。次の技術実証に渡す契約と測定点を以下に固定する。

対象は `G = C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI` の `G/DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll`。SHA-256は `324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c`、PE64のImageBaseは `0x180000000`。以下のRVAはこのハッシュのDLL専用であり、別ビルドには流用しない。

## 一次資料と今回の静的測定

| 箇所 | 観測 | 判断の範囲 |
| --- | --- | --- |
| `G/DLC/Expansion2/UI/Additions/GovernorPanel.lua:73-100` | 公式候補一覧は `CanEverAppointGovernor(governorHash)` を読む | UIの読取。AIの判断を示す資料ではない |
| 同 `:547-577` | 公式画面はGovernor Indexを渡す `APPOINT_GOVERNOR` / `PROMOTE_GOVERNOR` を要求する | MODの公式操作もこれに合わせる |
| `G/DLC/Expansion2/Data/Expansion2_Schema.sql:337-352`、`Expansion2_Governors.xml:16`、`Expansion2_Leaders_Major.xml:91,287` | `Governors.TraitType` は `Traits` 参照。Ibrahimの資格特性をSuleimanに静的に割り当てる | 加入時に変更する列や操作の存在は示さない |
| DLL `0xBA4D0` → `0xBA51E` → `0x2DDDD0` | UI getterはHash/Indexを既存変換RVA `0x1B0810` に通して本体判定を呼ぶ | 変換は[既存調査](leader-persona.md#一次資料と採用できなかった方式)を参照 |
| DLL `0x2DD820:0x2DD82F-0x2DD861` | 共有の称号可否、任命済みの検査後、`0x2DD861` から `0x2DDDD0` へ末尾分岐 | `CanAppointGovernor` の本体経路。直接 `call` だけを探すとこの分岐を見落とす |
| DLL `0x30DA05-0x30DA75` | 操作を分岐する内部経路は `0x30DA56` で `0x2DD820` を呼び、真の場合に `0x30DA70` で任命処理候補 `0x2DCFB0` を呼ぶ | 操作IDと公式Luaの対応は実行時照合が残るが、資格より前に本体任命を行う経路ではない |
| DLL `0x2DCFB0:0x2DCFFE-0x2DD005` | 任命処理候補自体も `0x2DDDD0` を呼び、偽なら先へ進まない | 公式要求の拒否と整合する二重の本体境界 |
| DLL `0x21B9E4`、`0x4A8114` | `0x2DDDD0` への直接 `call`。後者は `0x4A8530` から反復処理される `0x4A7F80` 系の内部経路にある | 前者の役割は未分類。後者の周辺はAI総督分析領域だが、AI実手番での実行は未観測 |
| DLLの埋込文字列と参照 `0x4A5F64`、`0x4A73A1`、`0x4A8644` | 近接するコードに `AI::Governors`、`G:\BuildAgent\work\26d31e7d2eca7bbe\Civ6\Src\GameCore\XP2\Common\AI\Governors.cpp`、`Game Turn, Player, Governor, Assignment, Promotions` の参照がある。`0x4A8567` は `0x4A7F80` を呼び、同処理の `0x4A8114` が資格関数を呼ぶ | 逆アセンブルとソースパス文字列でAI分析領域を特定。イベント時の呼出しスタックは未測定 |
| DLL `0x2DDDD0:0x2DDDEC-0x2DDE0A, 0x2DDE70-0x2DDE87, 0x2DDF95-0x2DE076` | 第2引数をIndexとして総督定義を引き、所有者側の特性集合を照合して真偽を返す | 既存の静的Trait gateと整合。加入台帳への参照は見えない |

Win64の呼出し規約と命令から、**このビルドの観測可能な入口ABI**は `RCX = IPlayerGovernors相当のthisポインタ`、`EDX = Governor Index (32 bit)`、戻り値は `AL = 0/1` と読める。`0x2DDDEC` は `RCX+0x70` を参照し、その先の `+0xD8` をプレイヤー識別子として補助呼出しへ渡す。UIからのHashをこの入口へ直接渡してはいけない。構造体の型、所有権、呼出しスレッド、他ビルドのオフセットは未証明である。

読み取り専用ツール `tools/civ6-pe-rs` でPEヘッダ、`.text`、`.pdata`、E8/E9相対分岐を読んだ。`0x2DDDD0` のファイルオフセットは `0x2DD1D0`、`.pdata` 範囲は `[0x2DDDD0, 0x2DE07D)`。直接呼出し候補は `0xBA51E`、`0x21B9E4`、`0x2DCFFE`、`0x4A8114`、末尾分岐は `0x2DD861`。`0x2DD820` のファイルオフセットは `0x2DCC20`、直接呼出しは `0xBA30E` と `0x30DA56`。これらの命令境界は `llvm-objdump -d --no-show-raw-insn` で照合した。RustツールのE8/E9走査単独では、命令の途中に現れたバイト列を排除できない。`.pdata` の範囲も分割された関数片を含み、ソース上の関数名を証明しない。DLLの名前付きexportにも該当する総督資格関数はなかった。

## 標準MOD接続の到達点

インストール済みのBase・Expansion1/2と全DLCのXML/SQL/Luaについて、`GRANT_TRAIT`、`SetTrait`、`AddTrait`、`AttachTrait`、総督資格setterを対象に再検索した。`G/Base/Assets/Gameplay/Data/Modifiers.xml:8` に `MODIFIER_ALL_PLAYERS_GRANT_TRAIT` の **Types行** が1件ある。一方、対応する `DynamicModifiers` 行、作用するEffect、公式の使用例は見つからなかった。DLLのASCII登録名も `CanEverAppointGovernor`、`CanAppointGovernor`、`ChangeGovernorPoints` は確認できるが、調べたTrait setter名は見つからない。型名だけを `AttachModifierByID` へ渡して資格が開くとは扱わない。この調査は本ビルドの公開MOD接続についての否定であり、未知の内部実装すべてが存在しないとの証明ではない。

[実機43の証跡](native-slots-diagnostic.md#人物22種の静的資格と公式任命要求を分ける診断実機43) は静的gateの限界を示す。22勢力×22人物の `CanEver` はすべて偽、各3称号の21AIは実1手番後も人物任命・称号使用とも0。正規Joinと実Great Personを確認した織田信秀へ公式APPOINTを1回だけ要求し、後続の同期 `EXECUTE_SCRIPT` の完了を確認した後も本体Governorなし・称号使用0・資格偽だった。`C:/Users/makhmeto/Desktop/dev/ADJ/civ6-banseiki-wt/live/tools/civ6-autotest/artifacts/leaderperson-43/result.json` は17/18手順後 `passed=false`。操作拒否と処理未観測の内訳までは断定しない。比較対象の未制限登録では実機23の21AIに未加入の任命が生じた（[記録](leader-persona.md#公式費用功績6回保存の実機成功とai任命の残課題)）。この差分はAIも本体のTrait制限を受けるという強い根拠だが、特定RVAをAIが通過した呼出し履歴ではない。

## 最小のRust engine拡張の契約案

以下はnativebridge向けの設計条件であり、実装済みの関数名や現存するLua APIではない。

1. **状態の所有者**: 同期Gameplayの `Game:SetProperty` 台帳を唯一の正本にする。現行の加入は `poc/JAC_PoC_LeaderPersona/Scripts/JAC_LeaderPersona_Gameplay.lua:55-104` で実Great Personの生成後に `OWNER`、`EVOLUTION_COST`、`STATE=GREAT_PERSON` を保存する。費用予約は同 `:117-141` が称号獲得数−使用済み数と他人物の未払い予約を検査し、`STATE=EVOLVING` に遷移する。UIのLua値、非同期ログ、別スレッドから同じLua VMを呼ぶ方式を資格の入力にしない。
2. **論理述語 `CanEver(playerID, govIndex)`**: 22人物のGovernor Indexだけに限定して、総督Indexから世界共通の人物IDを引く。加入前と他の所有者は偽。加入後も `GREAT_PERSON` の段階では偽とし、当該所有者の実Great Personが存在し、費用全額を公式残高から予約済みの `EVOLVING` 段階で真にする。任命済みの `GOVERNOR` は同じ所有者の実Governorを確認して真を維持する。その他の公式Governorは元の判定に渡す。静的gateだけを維持しても加入後を開けないため、22人物に限り本体の特性判定結果をこの同期資格で置き換える構成を調べる。
3. **予約と正式会計**: Join直後に開くとAIが費用2/3の人物を1称号だけで先取りし得る。`EVOLVING` は費用全額の予約が成立した後に限る。現在の `PrepareEvolution` は人物別 `native.canAppoint` を遷移前に確認するため、資格を `EVOLVING` で開く案とそのままでは循環する。実装時はGameplay内で実人物・所有者・公式残高・予約を先に検査して遷移し、その**後**に本体の人物別資格と任命可否を読む。人間のUIだけでなく、AI所有者も同期Gameplayで人物ID順に同じ予約を確定する必要がある。その処理が本体AIの総督分析より先に走るかは未測定であり、遅ければ本体の同期操作入口に移す。APPOINTを1回、費用2/3では効果なしの公式PROMOTEを1/2回だけ実行し、公式 `GetGovernorPointsSpent` と実Promotionから未払いを復元する。Gameplayの功績購入、5ターン定着、忠誠8、活動不能と回復の判定をUIの擬似総督へ移さない。
4. **本体との同期接続**: 最小案は、GameCore自身が保存する同じGame propertyを、`0x2DDDD0` の実行スレッドからLuaを経ずに同期読取すること。Rust側の変換層は所有者IDと総督Indexを検証し、読み取れない場合は22人物だけ偽で失敗する。これは現時点でアクセサのアドレス・ABI・スレッド安全性が未測定の**設計案**である。実Game propertyへ安全に到達できない場合は、同期操作の確定点にネイティブ状態と保存・ロード処理を設ける必要があり、単純な判定関数hookより変更範囲が増える。別の状態を非同期ログから追従させない。
5. **保存・複数端末**: 同じ保存データのGame propertyからロード直後、最初のUI読取とAI判断より前に資格を得る。総督Index対応はそのゲームの定義から再構築し、古いポインタやプロセス固有のアドレスを保存しない。操作順・人物順を固定し、時刻・UIフレーム・乱数・端末固有値を判定に混ぜない。もし本体側にキャッシュが必要なら、Gameplayと同じ同期確定点で更新し、保存読込時に同じデータから再構築する。複数端末で各確定点の台帳、資格484組、称号使用済み数、実総督と保存ハッシュを照合する。

この境界だけではAIの費用2/3の追加PROMOTEは保証できない。AIがどの操作入口から任命するか、AIに対する公式PROMOTEの実行・順序・使用済み会計が成立するかは別に測る。両者を証明するまで、AIに開いた資格を製品化しない。既存の[称号会計](../spec/ssot/09-assimilation-religion.md#指導者の進化)と実総督のスパイ中和・回復を維持する必要がある。

## nativebridgeに渡す未実測点と最短の実機順序

| 未実測点 | 必要な観測 |
| --- | --- |
| AI入口 | 同じDLLのAI実手番で `0x2DDDD0` の呼出し元RVA、RCXの所有プレイヤー、EDXの人物Index、戻りALを記録。`0x21B9E4` / `0x4A8114` の役割を確定し、公式APPOINT処理の `0x30DA56` / `0x2DCFFE` と比較する。GameplayによるAI所有者の費用予約と本体AI総督分析の前後関係も測る |
| 同期状態とABI | `Game:SetProperty` の本体格納先、読取入口、オブジェクト寿命と呼出しスレッドを特定。Join/予約の同期操作がAIとAPPOINTより前に確定することを測る。読み取り専用静的調査だけではC++型と完全なABIは確定しない |
| 保存 | 加入後、予約後、任命後、追加昇進途中、完了後で保存・再読込し、初回判定までに資格と公式使用済み数が一致することを測る |
| 複数端末 | 同じ同期操作順で各端末の資格・公式総督・称号・功績とゲーム同期状態が一致することを測る |
| AI会計とスパイ | AIが費用2/3の全公式昇進を払うこと、実Governorが敵スパイの公式中和と回復を受けることを測る |

技術実証を行う段階の最短順序は、(1) 未加入484組の偽・21AIの1実手番・使用済み0、(2) 費用1の正式Join→予約→資格真→公式APPOINT一度→同期barrier→本体実体と使用済み1、(3) 費用2/3の公式PROMOTEと途中保存、(4) AIの同経路と全額会計、(5) 5ターン定着・忠誠8・都市喪失の回帰、(6) 公式スパイ中和・回復、(7) 複数端末の順である。実機41の都市喪失55手順成功と42のスパイ派遣不受理は別の証跡として保持し、この資格変更の合格へ読み替えない。現段階では実機、DLLの改変、runtime patch、注入、配布を行っていない。技術実証後に配布・権利の判断が必要ならオーナー事項とする。

## 再現した読取コマンド

```powershell
$d = "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll"
Get-FileHash -Algorithm SHA256 -LiteralPath $d
cargo run --locked --manifest-path tools/Cargo.toml -p civ6-pe-rs -- $d 0x2dd820 0x2dddd0 0x2dcfb0
llvm-readobj --file-headers --sections --coff-exports $d
llvm-strings -t x $d | Select-String 'AI\\Governors.cpp|AI::Governors|Game Turn, Player, Governor'
llvm-objdump -d --no-show-raw-insn --start-address=0x18030da00 --stop-address=0x18030da75 $d
llvm-objdump -d --no-show-raw-insn --start-address=0x1802dddd0 --stop-address=0x1802de07d $d
llvm-objdump -d --no-show-raw-insn --start-address=0x1804a80ea --stop-address=0x1804a8121 $d
```

この読取だけで本体資格が動的になったわけではない。今後、実在する標準接続が確認できた場合はこの案より先に実証する。
