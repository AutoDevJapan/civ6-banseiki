# PR86: 加入人物の本体任命資格（共有ToDo 853）

## ToDo853: B1と本人読取を同じ試験用leaseへ接続

費用1のCore受入は親1595の待ち列、費用2/3は親880、中和は548が所有する。この接続の実機成功はまだない。通常のmode6と観測mode7を変更せず、controllerの明示的な `--mode paid-assignment-experimental` と同期Gameplayの `JAC_TEST853_PAID_ASSIGNMENT_V1=true` を両方使う場合だけ試せる。通常シナリオへ追加しない。内部設定version `0x08530001` はmode6だけに適用され、CanEver・自然property・CanAssignを一つのbounded controller leaseで設置、失敗時および終了時に復元する。ロードしただけでは設置しない。

Rustの `paid_glue::source()` がGameplay接続を生成する。生成器は `cargo run --locked --manifest-path tools/Cargo.toml -p civ6-native-bridge-rs --bin generate-paid-glue -- <repository-root>`。既存の同期台帳、実偉人の照合、GameInfoの22定義を束縛し、費用・Governor Index・BASE/TITLE_1/TITLE_2 Indexを転送する。UIの読取結果や総Spentを配置資格へ使わない。配置時には現在のGovernor引数を再読し、所属リスト・owner・Index・本人昇進配列とこの定義を照合する。予約中の任命資格trueだけでは配置を許可しない。

schemaはGame property `JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA=1`。904000..904255がmanifest、905000..905255が支払照合要求、906000..906255が閉鎖、907000..907063がowner reset。追加領域905256..905999と906256..906523は22件×46bitの定義転送、904999は全定義のcommit。数値型のnative表現を推測せず、転送にも検証済みのtyped boolを使う。1定義はIndex8bit、費用2bit、BASEとTITLE各12bit。未使用TITLE欄は0。全22件のIndex・昇進が一意で、全bitが同owner/receiver/sessionのreset後かつcommit前のticketを持つまで全資格を閉じる。任意のbitの再公開は古いcommitを無効化する。支払照合要求trueはcommit後に必要で、現在の本人が費用分の正式昇進を持つ場合のみ原ALを維持する。原falseはtrueへ変えない。

初回は全予約領域の既存値を照合し、衝突時は既存値を上書きしない。再読込は保存済みtrueを使わずowner reset、22件のfalse ACK、定義commitをやり直す。現在は試験専用で全定義を再同期するため、通常有効化前に本体受入と所要時間の計測が必要。合成検査と実機の結果は区別して記録する。

試験用設定は専用の共有traceを確保し、mode7のlayoutを変えずにCanAssignの原RAXと実際に返したRAXを保存する。終了後は三入口の元bytesを確認し、配置traceの未完了呼出しと容量超過があれば成功記録を出さない。記録identityは `paid-assignment-experimental`、schema 1、明示した全予約範囲を持ち、既存mode6判定器へ渡さない。専用の `check-paid-artifact <JSONL> <PID> <created-filetime>` で順序、三入口の復元、予約キー、各sequenceと件数、AL以外の保持を検査する。出力 `capture_valid=true` は記録の整合性だけを示し、`product_acceptance=false` を必ず併記する。実験用シナリオを通常一覧へ追加していない。

169fbc30の全件検査は117対象、1815成功、2失敗、ignored/filtered 0、exit101。新しい三入口Win64子と旧mode7子は各1成功（各48 filteredを別集計）で、未払い・本人費用3・別owner・reset・三入口復元・第3入口失敗の巻戻しを確認した。失敗2件は生成Luaの更新漏れと、使わないTITLEを重複させた異常fixtureで、2916cbbeで再生成と実BASEの重複へ修正した。全体成功へ読み替えない。以後の配置trace追加版は別候補として検証する。

## ToDo1433再開：配置資格の独立受信処理B1

**配置の共通経路は親の実機144で確認済み。今回のB1は独立した支払済み資格の受信処理と合成拒否試験であり、製品の未払い配置拒否B全体は未完成である。** 元の観測Aの差分b848fba5を保持した担当用作業場所を、親のc21b145fと通常資格修正51cb6b42まで早送りした。親・実機担当の作業場所と本体メモリには接続しない。

実機144の根拠は本書「実機141〜144」の記録とd49d485e。UI戻り先0xBA3E2が2行、本体更新0x2DD375が1行、owner0・Index4・City ID65536・座標134/29・owner0・原AL1で一致した。これは私的な費用1の配置観測で、費用2/3・AI・支払い本人の全昇進の受入へ拡張しない。

`windows/paid_assignment.rs` の `PaidAssignmentConsumer` は、既存の任命資格と同じowner binding、session、一度だけ採番するgetter入口ticket、128bit CASを使う。manifest 904000+Index、paid bool 905000+Index、閉鎖906000+Index、owner reset 907000+ownerの別領域を解析する。各Indexは0..255、ownerは0..63で、producerによるこの範囲の予約・保存・公開はまだ行わない。任命資格901000+Indexのtrueはpaidへ転用しない。

閉鎖はgetter入口で即時に発効する。その後のtyped bool false ACK、同じ本人のmanifest、最後のtyped paid trueが同じreceiver/sessionと新しいticketで揃った場合だけ有効とする。未知receiver、複数ownerへ重複したreceiver、不正型・flags・payload、未払い、古い完了、reset・rebind・cancel・ticket上限は既知の人物を閉じる。owner resetは通常資格と同じbinding閾値を先に進め、二つの資格を同時に失効させる。返値は一回分の非複製トークンへreceiver・key・世代・ticketを束縛し、別人物や別getterへ返値を取り違えない。

現在のCanAssign呼出しへ渡すadapterは、同じ二つの入力ポインターから既存の本人読取器を使い、owner・Index・本体リスト所属・昇進列を照合する。任意getterやLuaは呼ばず、ポインターを保存しない。原RAXを受け取り、拒否時だけALを0へ制限する。元のfalseをtrueに変えず、分類済みの通常総督は人物用の昇進配列を要求せず、原RAXを保持する。人物manifestが揃う前の未分類Indexと通常総督を区別する製品の初期化処理は次工程で必須であり、このadapterを単独で実ゲームへ設置しない。

合成Win64子では、既存の4引数gatewayを検査時だけこのadapterへ接続する。通常のmode7には切替手段を追加せず、原RAXを保持する観測モードを維持する。拒否時の原関数一回、4入力・RAX/R10/R11・XMM0..5、上位RAX保持、全額後の原true保持と原false保持、通常総督、閉鎖後の拒否、trace容量上限でも拒否継続、復元後の原動作を検査する。製品controller・新mode・trace版・三入口の設置処理は未実装である。

### 次の工程と受入条件

1. 親853の現在の本人読取器について、同一実SessionのCore昇進とそのゲームのGameInfo定義を実機で照合する。診断の呼出元が渡したPromotion Indexだけでpaidをtrueにしない。
2. 同期Gameplayの人物台帳と、実Governor/base/title1/title2/公式Spentの人物別会計からRustでpaidを派生するproducerを接続する。全22人の任命producerを複製せず、範囲の衝突検査・保存schema移行・全owner閉鎖を同じ初期化へ追加する。予約残高とEVOLVING状態だけを支払い証拠にしない。
3. 完全な人物catalogの初期化前は人物を閉じる共通gateを、CanEver・自然Core getter・4引数CanAssignの三入口として一Job・leaseで設置する。入力が未分類の間にabsenceを通常総督の証拠にしない。第二/第三設置失敗、遅延callback、取り消し、各trace版の終端、三入口の復元を合成試験で確認する。
4. 同一候補の別モデルレビュー・全件検査・releaseは親が行う。費用2/3の公式APPOINT後の未払いUI/本体ASSIGN拒否、PROMOTEを一個ずつ支払った本人会計、全額後の配置、途中保存/再開、AIの正式支払いと配置、複数端末を親のlive_chainで実証する。B1の合成成功を実機成功へ読み替えない。

### B1の検査結果

最終候補は専用targetで `cargo test --locked --manifest-path tools/Cargo.toml --workspace --all-targets --no-fail-fast` が115実行対象・1,809件成功、失敗・ignored・measured・filtered outが0件だった。Lua5.1を全件実行した。Cargoの各Running区間の最後のsummaryだけを集計し、別Win64子の1件成功・44 filtered outは二重加算しない。最終native45件には受信処理の追加6件と、4引数gatewayの合成拒否・通常総督の昇進配列欠落の直接確認を含む。

native crateは `cargo clippy --locked --manifest-path tools/Cargo.toml -p civ6-native-bridge-rs --all-targets --no-deps -- -D warnings` が成功した。初回は既存の本人読取器の末尾byteループでneedless_range_loopを検出したため、同じoffset・一byteずつの読取をiteratorへ変更して修復した。初回45件の検査に出た試験用メモリのunused_assignmentsもvolatile書込みと読取結果の確認へ修復した。さらに通常総督へ人物用昇進配列を要求しないよう判定順を直し、修正前の全件1,809件とは別に最終全件を再実行した。Rust2021の対象整形・差分検査も成功した。依存先まで含むClippy、別モデル、CI、release、製品の配置拒否と追加実機はこの担当では未実行である。

親へ渡すのは51cb6b42を基準にした6ファイルのcommit差分だけ。lib/windows/assignment/新paid_assignment/本人読取器のloop/本書が対象で、任命producer・既存シナリオ・UI・実機の保存は変更しない。検査ログ、集計、候補指紋は担当用 `.task/assignment1433` へ保全し、commitには含めない。

## ToDo1433の配置観測Aと未払い拒否Bの接続契約

**今回追加するのは原戻り値を保持する配置観測Aで、費用2・3の未払い配置拒否Bは未実装・未実機である。** 基準a29a8717の専用作業フォルダーでmode ordinal 7 `AssignCalibration`を追加する。通常資格のmode 6は担当1399/1447が修正中であり、この差分に複製しない。mode 5、数値getter、ticket/session、128bit原子更新と既存の終了処理は保持する。mode 7だけでは人物を任命可能にしない。

### Aの実装と測定対象

固定GameCore SHA-256は `324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c`。配置可否RVA `0x2DD880` のRCXはGovernors、RDXはGovernor **ポインター**、R8はCity **ポインター**、R9は意味を確定していないraw引数である。公式Expansion2のGovernorAssignmentChooser.lua:179はCanAssignGovernorに第三引数trueを渡しており、R9を理由出力ポインターと断定しない。`0x2DD8B9` がGovernor+4のIndexを、`0x2DD941` がGovernors+0x70のPlayerを使う。入口の最初の10 byteは `48 89 5c 24 08 48 89 6c 24 10` と一致を要求する。任命資格用の2引数gatewayは流用せず、専用の4引数gatewayを使う。

静的には公式UI `0xBA350` が `0xBA3DD` から `0x2DDB30` を呼び、後者の `0x2DDBAB` が `0x2DD880` へ末尾分岐する。観測するUIの戻り先は `0xBA3E2`。本体配置更新 `0x2DD340` は `0x2DD36D` でR9を0にし、`0x2DD370` から同じ検証へ呼び、戻り先 `0x2DD375` でALが偽なら `0x2DD6DB` へ進む。これは固定DLLの読取・逆アセンブルによる根拠であり、実手番やAIの通過を証明しない。`0x2DD880` はBOOL検証で、配置setterそのものではない。

専用gatewayは4引数とRAX/R10/R11、XMM0..5を原呼出し前に復元し、原関数を一度だけ呼ぶ。戻ったRAX全体を退避・復元し、ALも改変しない。Win64のshadow space、16 byteの呼出し時stack alignment、SEH unwind情報を持つ。入口5 byteの中継分岐は既存の8 byte原子交換で設置・復元する。先頭命令だけをtrampolineへ移すため、二番目の命令の途中を分岐先にしない。復元後もDLL・trace・中継コードを所有ゲームの終了まで保持し、mode 7の再設置は同一プロセスで拒否する。実行器の事前検証も配置観測を単回に制限し、停止後のrestart_gameだけで使用済み状態を戻す。安定したsnapshotは遅れて中継へ入るthreadの不存在を証明しない。

呼出し中に `this+0x70`、Player+0xD8、Governor+4、および本体検証が使うCity+0xA8/+0xC0/+0xC4/+0xD8のraw値だけをコピーする。読取成功bitを独立して記録し、null・不正アドレス・欠落を許可へ変換しない。R9の先や任意getter、Lua VMは呼ばない。アドレスは数値の観測記録にし、別の呼出しで参照しない。City各fieldの意味は親が同じ場面のLua city ID・座標・ownerと照合するまで実測済みとしない。

traceは独立版1・容量1,024。上限では行を上書きせず欠落を加算し、原戻り値を保持する。controllerは不完全行、欠落、旧版、未復元を成功としない。実行器は終端のquiescence_proven=falseとcounter_scope=snapshot_observation_not_complete_call_historyを必須とし、所有者0・実Index4のUI/本体更新双方の行と、両経路のthis/Governor/City/PlayerおよびCity raw値の一致を要求する。この数値比較も、別呼出し間のポインター寿命の証明ではない。合成Windows検査は途中のpanicでもworkerを全件回収してから入口を復元し、traceを解放する。

### 親が実行する有限シナリオ

既存の私的費用1の手動任命シナリオを基に `leader-persona-assign-calibration-15km.json` を追加した。正規Join、本人の偉人、称号全額予約、公式APPOINT一度、Governor/base/Spent+1、通常Finalizeを確認する。mode 5を復元してから、同じ所有ゲームにmode 7を一度設置し、公式UIのCanAssignと通常のJAC_AssignLeaderを一度送る。実Cityへの配置と称号使用数不変を確認し、入口を復元する。観測窓は90秒、配置待ちは30秒。strict Luaと既存のgeneric固定Judgeを使い、新テストMOD・map・rulesetは追加しない。通常の新規開始の試験MOD分離を保持する。

親853が別モデルレビュー後、所有ゲームのない状態で同一候補からDLL・controller・実行器をrelease buildして配置し、次を実行する。子1433は実Civ6・実メモリ・共有ロックに接続しない。

```powershell
D:/CC/Tools/civ6-live/live_chain.exe leaderperson:leader-persona-assign-calibration-15km.json --stop-on-failure
```

`assign-calibration-cost1.jsonl` と同試行の `ASSIGN1433_UI/REQUEST/AFTER`・steps・Lua.logを照合する。最初のAは費用1だけなので、費用2/3途中の拒否や本体AIの自然な配置試行を合格に数えない。AI用の寿命未証明な `0x30CBE0/0x30C100` は呼ばない。

### Bの具体的な実装契約（A実測と1447確定後）

1. mode 6の `901000+Index` は全額予約中のEVOLVINGにも真となる**任命**資格であり、配置資格へ共用しない。`903000+Index` は任命の閉鎖に使う。配置の候補は対応表 `904000+Index`、本人の支払済みbool `905000+Index`、閉鎖 `906000+Index`、所有者失効 `907000` とする。全runtime定義・保存初期化で既存範囲との衝突を検査し、同じowner・人物Index・Core receiver・session・ticket・typed false ACKで公開する。番号は未実装で、まだ製品へ公開しない。
2. 同期Gameplayの正本から、同じactor・人物・runtime Index・保存された費用、実Governor、base、必要なTITLE_1/TITLE_2、公式Spentと人物別の全支払の会計一致を確認してpaidを作る。予約残高、UI cache、EVOLVING/GOVERNORという文字列だけを全支払の証明にしない。未払い時は偉人と支払済みの実Governorを保持する。追加称号や完了stateを偽造しない。
3. producerは任命資格とは独立してpaid falseを最初に公開し、manifest/owner/Index/世代の確定と閉鎖値のCore自然getterによる確認後、全支払の確定点で最後にtrueを公開する。死亡・取消・所有者変更・保存読込・実体欠落・会計不一致では最初に閉じる。1399/1447の初期化/reload、receiver切替、closed ACK、manifest競合の契約へ合流し、全22人のproducerを複製しない。
4. consumerにはactor×Indexのpaid用128bit cellを追加し、同一Core receiver・session・入口で一度採番したticket・派生paidを一体でCAS更新する。manifest/owner/resetにも同じ世代照合を要求する。UI receiver・不正型・flags・欠損・旧世代・古いtrueの遅延復帰は閉鎖。資格consumerと同じ受信先切替/取消で両cellを閉じ、任命allow trueからpaid trueを導出しない。未確定の1447のatomic形式を別実装で固定しない。
5. A実測後の共通CanAssign callbackは、その呼出し中にコピーしたowner/Indexで確定manifestを照合し、原関数を一度呼ぶ。人物ならpaidだけで原ALを制限し、偽を真にはしない。通常の非人物総督は原RAXを返す。manifest未初期化時は全人物を識別できるまで人物を閉じる設計を1447のmanifest契約と統合する。無効owner・不正型・旧世代・未払いは閉じ、引数ポインターは保持しない。R9の意味はAで未証明のため書き換えず、記録名はraw_r9とする。
6. controllerはCanEver/自然getter/CanAssignを同一Job・lease・sessionで管理し、paidを先に閉じて全入口を復元する。第二/第三設置失敗のatomic rollback、遅延callbackと取消の競合、各版のsnapshot読取、所有ゲーム終了を合成で検査する。mode 7は観測専用として維持し、製品のmode 6との接続はレビュー確定後の別deltaにする。
7. 合成の拒否/許可/元一回/通常総督保持と、親の費用2・3の公式APPOINT→未払いUI/本体ASSIGN拒否→PROMOTE一個ずつ→実Spent/base/title照合→全額後の配置、保存読込・複数端末・AI実手番を必須にする。AIの公式PROMOTE選択・支払い経路は親の別工程で未完了のまま残す。

Aの観測sourceと合成検査だけを引き渡し、B完成やPR86の製品完成とは報告しない。

### 1433の検査結果と引き継ぐ差分

専用targetで `cargo test --locked --manifest-path tools/Cargo.toml --workspace --all-targets --no-fail-fast` が112実行対象・1,712件成功、失敗・ignored・filtered 0。Lua 5.1の全件を含む。別実行のworkspace文書検査10対象も成功した。新シナリオは実行器の `--check` で30手順VALIDだった。初回の新規開始件数133→134、配布固定一覧への新シナリオ登録漏れ、基準版の手動任命回帰1件の登録漏れを検出して修復した。元の119件と追加5件の人物検査一覧は、親の公開済み `9c34d443` と同一の差分を取り込み、削除・省略しない。

追加Clippyでは依存先を含む既存のスタイル警告で停止したため、任意の別領域へ機能変更を広げず、Aを載せるnative crateを `--all-targets --no-deps -- -D warnings` で検査した。この範囲のscope初期化3箇所、bool検査、無効ポインターの検査記法を等価に修正し、成功した。依存先を含めた厳格Clippy全体の成功は主張しない。修正後はnative 34件、実行器と関係シナリオ126件、人物一覧2件の計162件を再実行し、すべて成功した。整形と `git diff --check` も成功した。ソースの別モデルレビュー、同一候補release、CIと実機Aは親の担当で、子1433では未実行である。

対象はnative bridgeのlib/windows/独立assignment module/controller、実行器のnative_bridge/runner/scenario、assignment回帰と配布・MOD分離・人物一覧の検査、30手順シナリオ、本節の13ファイルだけ。親は基準a29a8717からのcommit deltaを3-way統合し、mode 6と新しい親修正を保持する。実機Aの同一call内の数値・原ALを確認し、1447確定後のpaid契約Bへ進む。AIの公式支払い工程は別の未完了条件である。

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
3. **予約と正式会計**: Join直後に開くとAIが費用2/3の人物を1称号だけで先取りし得る。`EVOLVING` は費用全額の予約が成立した後に限る。変更前の `PrepareEvolution` は人物別 `native.canAppoint` を遷移前に確認していたため、資格を `EVOLVING` で開く案と循環していた。task853ではGameplay内で実人物・所有者・公式残高・予約を先に検査して遷移する順序へ直した。人間のUIだけでなく、AI所有者も同期Gameplayで人物ID順に同じ予約を確定する必要がある。その処理が本体AIの総督分析より先に走るかは未測定であり、遅ければ本体の同期操作入口に移す。APPOINTを1回、費用2/3では効果なしの公式PROMOTEを1/2回だけ実行し、公式 `GetGovernorPointsSpent` と実Promotionから未払いを復元する。Gameplayの功績購入、5ターン定着、忠誠8、活動不能と回復の判定をUIの擬似総督へ移さない。
4. **本体との同期接続**: `CanEver` が参照する所有者Playerから本体Player propertyへ到達できることが静的に分かった（下記）。費用1の最初の橋は、同期Gameplayで確定したPlayer側の入場許可を同じ所有者の本体propertyから読む候補を優先して測る。Game property台帳の `OWNER` や `STATE` をPlayer側へ複製して二重正本にしない。許可を派生値として持つなら、加入・予約・取消し・実Great Person死亡・完了・読込の各確定点で台帳から更新し、再構築前は閉じる。既存のGame propertyを本体から直接読む案も残すが、UIスレッドの読取とGameplay更新の競合は未測定である。両案とも実在のC++アクセサの完全ABI・値寿命・同期を証明する前にRustから呼ばない。
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

技術実証を行う段階の最短順序は、(1) 未加入484組の偽・21AIの1実手番・使用済み0、(2) 費用1の正式Join→予約→資格真→公式APPOINT一度→同期barrier→本体実体と使用済み1、(3) 費用2/3の公式PROMOTEと途中保存、(4) AIの同経路と全額会計、(5) 5ターン定着・忠誠8・都市喪失の回帰、(6) 公式スパイ中和・回復、(7) 複数端末の順である。実機41の都市喪失55手順成功と42のスパイ派遣不受理は別の証跡として保持し、この資格変更の合格へ読み替えない。この設計を記した時点では、新たな実機接続、DLLの改変、runtime patch、注入、配布を行っていなかった。技術実証後に配布・権利の判断が必要ならオーナー事項とする。

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

## Player/Game propertyの本体経路を追加調査（共有ToDo 853）

前節のDLLとSHA-256を維持し、`tools/civ6-pe-rs --ascii-xrefs` のASCII文字列参照候補をLLVMの実命令で照合した。`GetProperty` の文字列RVAは `0x9BCF38`、`SetProperty` は `0x9BCF78`。DLLに含まれる `Cache_Lua_IPlayer.cpp` と `Lua_IPlayer.cpp` の出典文字列は、それぞれ登録領域 `0x6A93E0` と `0x6ED4F0` から参照される。これにより同名のLua APIをUI cacheとGameplayで取り違えずにwrapperを特定した。

| 入口・命令 | 実命令から読めるreceiver / 結果 | 限界 |
| --- | --- | --- |
| 本体資格 `0x2DDDEC`、`0x2DDDFD` | `RCX=this` の `+0x70` からPlayer候補ポインタを取り、その `+0xD8` の32 bit IDを使う。`0x2DE930` はIDから本体Player配列を引き、`0x72E3D0` はPlayer内のcomponent配列から総督componentを返す | `this+0x70` がGameplay getterと同じ生存中のPlayerインスタンスであることはobserverで照合する |
| Gameplay Player getter登録 `0x6ED3CF-0x6ED3F8` → wrapper `0x6EDC60` | `0x6E8120` が本体Playerを解決し、`Player+0x528` を `0x152F0` へ `R8` で渡す。`Player+0xD8` はGameplay setter `0x6EE43E` でもIDとして読む | `0x6E8120` はLua stateを受けるwrapper専用。AIや別スレッドから呼ぶnativeAccessorではない |
| Gameplay Player setter登録 `0x6ED3FD-0x6ED426` → wrapper `0x6EE3F0` | `Player+0x518` から `0x72A920` を通ると `+0x528` の同じpropertyインターフェースに達する。`0x153A0` の後、`0x5FC360` へ所有者IDとキー・値を渡す | `0x153A0` は値の削除・更新を仮想関数経由で行うと読める。通知の確定順と他端末への反映は未測定 |
| Gameplay Game getter `0x6D7690`、setter `0x6D7F50` | DLL内の本体Gameポインタ `0xB8AA68` の `+0x1B8` が読取先。setterは `+0x1A8` と `0x72A920` を通じて同じ `+0x1B8` に達し、続いて `0x5FC4F0` を呼ぶ | 同じ台帳へ届く候補だが、`CanEver` からの安全な呼出しスレッドは未証明 |
| UI cache Player getter登録 `0x6A92F1-0x6A931A` → wrapper `0x6A9E70` | cache側のPlayerを別経路で得て、その `+0x58` を同じ汎用 `0x152F0` へ渡す | `+0x528` と `+0x58` は別オブジェクト。現在値への反映時刻は未測定 |
| UI cache Game getter `0x68AF10` | cache側のGame object `+0x910` のpropertyインターフェースから読む | 本体Game `+0x1B8` と同一のポインタではない |
| cache側へのコピー候補 `0xBE3B9-0xBE3CE` | `rbx+0x58` をreceiver、`rdi+0x528` をsourceとして、propertyインターフェース仮想関数 `+0x178` を呼ぶ | 両offsetが上記wrapperと一致するため本体Player→UI cacheのコピーと推定。実時刻とスレッドを測る |

**nativeAccessorの最短候補**は `ownerPlayer = *(NativePlayerGovernors.this + 0x70)`、`ownerID = *(ownerPlayer + 0xD8)`、`properties = ownerPlayer + 0x528` である。ここまでのfieldとIDは命令に現れる。文字列キーの場合、汎用getter `0x152F0:0x1533E-0x1535E` は `properties` の仮想関数 `+0x18` から名前表を得て、その表の仮想関数 `+0x8` でキーIDを解決し、`properties` の仮想関数 `+0x60` に `RCX=properties, EDX=keyID` を渡す。戻る `RAX` は**Lua値ではなく内部値へのポインタ候補**で、wrapperは直ちに変換器の仮想関数 `+0xA0` へ渡す。数値キーならLua整数が直接キーIDになる。`0x153A0:0x15433/0x15442` ではnilと非nilでproperty側の仮想関数 `+0x140` / `+0x148` が分かれる。これらは呼出し候補と引数レジスタの静的観測であり、型名、可変長文字列の所有権、戻り値のタグ・長さ・有効期限を確定したC++ ABIではない。Lua stateを要求する `0x152F0` / `0x153A0` 自体をAIスレッドから呼ばない。

数値と文字列の**値**はいずれも、getter内で同じ `+0x60` の戻りポインタからLua変換器へ渡る。Luaで読める `number` / `string` がそのポインタの先頭バイト表現と一致するとは限らない。特に文字列を `const char*` と仮定せず、nativebridgeは型タグ・所有権・長さと、設定直後・UI cache反映後・ロード後の寿命を実測するまで直接解釈しない。

公式Luaも両側の境界を分けている。`G/DLC/BlackDeathScenario/Scripts/BlackDeathScenario_StateUtils.lua:63-95` はUIから `EXECUTE_SCRIPT` を送り、GameCore側で `SetProperty` を呼ぶ。`G/DLC/PiratesScenario/Scripts/PiratesScenario_UnitCommands.lua:74-84` は本体Playerへ数値propertyを書き、同シナリオのUI `WorldTracker_PiratesScenario.lua:41` はPlayer propertyを読む。標準の同期Lua経路と画面での読取が実在する根拠になる。ただし、これらのLuaファイルだけで本体関数からUIスレッド上で `Player+0x528` を安全に読むこと、または再読込直後のcache反映時刻は証明できない。

### 親のruntime observerへ渡す検証順

各組を少数のhardware breakpointで順に測る。命令位置はRVAなので、観測時は対象DLLのSHA-256を照合して実際のロード基底を足す。試験用キー `JAC_ACCESSOR_853_NUMBER=853271` と `JAC_ACCESSOR_853_TEXT="ACCESSOR_853"` を同期Gameplayの `Player:SetProperty` で設定し、公式Lua getterと比較する。人物台帳の `OWNER` / `STATE` / `EVOLVING` を試験用に上書きせず、Governor実体・公式使用済み称号も変更しない。

1. **所有者の同一性**: `0x2DDDEC` のRCX、`[RCX+0x70]`、`[owner+0xD8]` を採り、同一playerIDの `0x6EDC69` の戻りRAXと比較する。AI分析 `0x4A8114`、UI getter `0xBA51E`、公式APPOINT `0x30DA56/0x2DCFFE` で呼出し元RVAとスレッドIDを分けて記録する。
2. **値と寿命**: `0x6EDC76` の `R8=Player+0x528`、`0x1535E` の仮想呼出し前後の `RCX,EDX,RAX`、`0x6EE424` の設定後、`0x5FC360` の通知を記録する。`[properties]` の仮想表とslot `+0x18/+0x60/+0x140/+0x148` の実行先RVAも採る。数値と文字列をそれぞれ公式Lua getterで照合し、内部値ポインタが更新・次ターン・ロードで無効化されるかを見る。ポインタを保存して後で参照しない。
3. **cacheとスレッド**: `0xBE3CE` のsource/destinationを `0x6A9E70` のcache getterと照合し、両propertyインターフェースの仮想表が同型か調べる。Gameplay更新完了、UI最初のread、AI判断の順序とスレッドIDを記録する。別スレッドならGameCore propertyの同時読取が保護される根拠、または同期snapshotの確定点が要る。
4. **保存・取消し**: 加入前、実Great Person確認後、費用1を全額予約した後、任命後の保存・読込で、同じキーの本体Player値とUI cache値を初回readから比較する。新Playerポインタを再取得し、古いアドレスを流用しない。実Great Person死亡・予約取消し・進化完了でも許可を閉じる/維持する時点を測る。`0xB8387` には本体Player `+0x528` を汎用処理へ渡す命令があるが、これを保存・復元関数と断定する呼出し履歴はない。

この測定が通るまでは、Player側の入場許可をGame台帳の**派生値**として同じ同期Gameplay確定点で管理する案に留める。Game台帳の所有者・状態・費用の写しを第二の正本にせず、ロード後の再構築前、実Great Person欠落、取消しでは資格を閉じる。もし初回AI/UI判定が再構築より先なら、この案はそのまま使えない。Game propertyを本体から直接読む案も、同じスレッド・snapshot検証なしには採らない。

許可を実装する段階では、加入済みの実Great Personと全額予約を確定してからPlayer許可を**最後に**立て、取消しや実体喪失では許可を**最初に**消す。進化完了時は実Governorと全支払いを確認して許可の意味を更新する。Game台帳の変更とPlayer propertyの変更が一つの同期処理・保存境界として観測できること、途中保存やロード直後に片方だけ古くならないことを先に証明する。証明できなければ派生permissionを永続の資格入力に採らない。

### 費用未払い時のAssign境界

資格が開いてAPPOINTが通っても、費用2/3の追加PROMOTE前に公式Governor実体ができる。UIの `CanAssignGovernor` はwrapper `0x710090` →本体 `0xBA350` → `0x2DDB30` →末尾分岐 `0x2DD880` を通る。公式操作ID `ASSIGN_GOVERNOR=0xF97C278B` を比較命令から追うと、配置は `0x30E070` → `0x30E0D5` → `0x30E1AC` → `0x2DD6F0` → `0x2DD340` であり、反映前の `0x2DD370` → `0x2DD880` で可否を読む。UIと公式配置が合流する `0x2DD880` が未払い時の配置禁止の静的候補になる。実手番での通過と拒否はまだ未実測で、UIだけを隠して支払いを保証する方式は採らない。

以前の記録で配置とした `0x30EC0F/24` は、操作ID `PROMOTE_GOVERNOR=0x7D3DA99D` の分岐だった。正式昇進はGovernor IndexとPromotion Indexを渡し、`0x30EC0F` → `0x2DDBB0` が可否を調べ、真なら `0x30EC24` → `0x2DF340` が取得済み昇進を反映する。`GetGovernorPointsSpent` の本体 `0x2DEA90` は取得済み昇進フラグを数える。別の使用済み数を直接加算して支払いを代替しない。AIが効果なしの追加称号昇進を順に選び、配置より前に全額を払うことは、この静的解析だけでは未証明である。公式昇進、Spent、活動不能とスパイの実体をpropertyで偽造しない。

### 昇進可否候補の入口観測を止めた理由（2026-10-05）

固定GameCoreを読み取り専用のRust製`civ6-pe-rs`で再確認した。候補`0x2DDBB0`はファイル位置`0x2DCFB0`、`.pdata`範囲`[0x2DDBB0, 0x2DDC9F)`、先頭8バイト`40 53 56 57 48 83 EC 30`だった。これは`push rbx`、`push rsi`、`push rdi`、`sub rsp, 30h`の4命令である。直前の`0x2DDBAB`からの5バイトは`E9 D0 FC FF FF`という前の処理の分岐であり、入口用の未使用領域ではない。`0x30EC0F`はこの候補へのE8呼出しパターンとして見つかるが、その事実だけでは引数の型・数、戻り値、AI実手番での通過を証明しない。

既存のnative bridgeの8バイト原子的な入口交換は、`CanEver`の先頭5バイトが**一つの命令**であることを前提とする。`0x2DDBB0`に同じ方式で5バイトのジャンプと3バイトのNOPを置くと、既に2・3・4バイト目から実行しているスレッドは元の命令列ではなくジャンプの変位やNOPへ進み、スタック操作を飛ばし得る。原子的交換だけではそのスレッドを保護できない。また既存の戻り値ラッパーは元の関数を変更後のスタック位置から呼ぶ。候補のスタック引数の有無が不明なため、これを使った戻り値記録も安全といえない。

このため候補へのフック、入口行、戻り値行、時刻・スレッド・Indexの推定は追加していない。入口だけを記録する場合も、途中実行中のスレッドを保護できる別の設置・解除方式と、その方式の合成呼出し試験が先に必要である。現行の`civ6-native-probe-rs`のhardware breakpoint経路には実機53・82で停止位置の障害記録があり、代用品として検証済みとは扱わない。AIの費用2・3の正式昇進と使用済み称号は引き続き未実測である。

### 固定PROMOTE呼出し1箇所の受動観測（2026-10-05）

入口を変更する方式は復活させず、`civ6-native-bridge-rs`に明示起動の`promotion-call`（mode 8）を追加した。固定SHA256のGameCoreについて、PEの`.text`からRVA `0x30EC0F`→ファイル位置`0x30E00F`、呼出し先RVA `0x2DDBB0`→`0x2DCFB0`を再計算する。呼出し全体と後続命令の16バイトは`E8 9C EF FC FF 84 C0 0F 84 75 01 00 00 44 8B C7`で、`E8`の変位は`0x2DDBB0`へ復号される。変更対象はRVA `0x30EC10`からの4バイト変位だけで、4バイト境界かつ同一64バイトライン内にある。元の`E8`、後続命令、呼出し先先頭とその直前5バイトを変更しない。

設置・解除は`VirtualProtect`の下でこの変位を32ビットのcompare-exchangeで交換し、保護の復元、`FlushInstructionCache`、16バイトの読戻しを確認する。[AMD64 Architecture Programmer’s Manual 3.11](https://kib.kiev.ua/x86docs/AMD/AMD64/24592_APM_v1-r3.20.pdf)は、変更範囲が自然な8バイト境界の中に収まり、1回の原子的な書込みで更新する場合の非同期コード変更を許容し、変位4バイトの変更を例示している。固定位置はこの条件を満たす。Intel側の非同期変更を同じ根拠で許可せず、CPU vendorがAMD以外なら設置前に拒否する。元の命令を先取りしたスレッドは元関数へ、変更後を読むスレッドは近傍relayへ進む。relay・gateway・traceは解除後もゲーム終了まで保持するため、解除直前に変更後の行き先を読んだスレッドも有効な経路を通る。gatewayは元の戻り先`0x30EC14`を保持し、入場時のRCX/RDX/R8/R9、XMM0～5、volatile整数レジスタ、flags、stackを復元してから元関数へ末尾ジャンプする。Rustから元関数を呼ばず、戻り値も記録しない。トレースの4引数は生レジスタ名だけで、所有者・Index・型に解釈しない。対象はこの公式PROMOTE呼出し1箇所だけで、他のPROMOTE呼出しやAI全体の経路を示さない。

固定DLLのSHAとPE位置の検査、および合成の並列4スレッドで設置前・設置中・解除後の元関数への到達、戻り先、4つの生引数、XMM0～5とcarry flagを確認した。2,048行超過、途中行、進行中、magic・版・mode・容量違いは読取側で拒否する。観測に失敗した場合は既存controllerの失敗時規則により、専有したゲームを終了する。これは固定イメージと合成呼出しに対する安全確認であり、実ゲームの観測結果ではない。命令を事前に取り込んだスレッドを観測対象に必ず含める保証はなく、解除時にまだgatewayの記録処理へ達していないスレッドもあり得る。したがって成果物は`quiescence_proven=false`の入口観測で、全呼出し履歴や正式昇進の成否・支払いの証明には用いない。AIの費用2・3、正式支払いと使用済み称号のMustは残る。

## task853の実装候補と実機待ちの境界（2026-10-03）

人物MODのSQLは、22人にだけ所有者へ付与しない有効な `TRAIT_JAC_PERSONA_ADMISSION_CLOSED` を設定した。sidecarが存在しなければ未加入の本体AI任命は閉じる。Gameplayの `PrepareEvolution` は、実Great Person・所有者・公式の獲得数と使用済み数・他人物の未払い予約を先に検査して `EVOLVING` を確定し、その後に本体任命可否を読む。人間の画面も予約前の `CanAppointGovernor` を要求しなくなった。AIは自分の手番のGameplayで人物ID順に同じ予約を行う。任命と昇進は従来の公式操作で1/2/3称号を払い、功績・定着・中和の規則を変更しない。sidecarが不在または本体が拒否したときは予約状態の人物と称号を保持し、公式任命を発行しない。

`tools/civ6-native-admission-rs` は、本体のGovernor Index 22件を実行中の `GameInfo` から受け取る判定器を実装した。対象以外の総督は元の判定を使う。対象人物は未加入・所有者不一致・実体欠落・残高不足・他人物との予約重複で拒否する。`EVOLVING` は全額予約と実Great Personを必要とし、途中の実Governorは配置を拒否する。`GOVERNOR` は実任命、基礎昇進、必要な追加昇進がすべてある場合だけ、公式の配置判定へ通す。過去の人物の公式支払いと進化中の観測値を合わせて使用済み称号と照合し、合わないときは未払い予約を減らさない。任命前に偉人が失われた場合は、次の手番まで二度観測してからその人物の未払い予約だけを解除する。入力の `Snapshot` はGame propertyの加入台帳と本体の人物・総督・称号を同期して読み取る**契約**である。現時点では実Civ6からその値を作る安全なadapterと、本体の戻り値を変えるsidecarは未接続であり、加入後の本体任命はまだ成立しない。AIが費用2/3の公式PROMOTEを順に払い直す操作経路も未実装なので、AI予約の検査を支払い完了の証明に読み替えない。Rust試験のJSON保存・読込は判定器の直列化の試験で、実ゲームの保存・読込の証明ではない。

既存の `civ6-native-probe-rs` に所有者ポインタ・ID・Player propertyインターフェースの読取を追加した。静的な実命令 `0x2DDDEC/0x2DDDFD` と `0x6EDC76` を根拠に、synthetic Windows子プロセスで `+0x70`、`+0xD8`、`+0x528` を読み、Playerオブジェクトの入替え後に古いポインタを再使用しない試験を通した。シナリオが指定する `mode=property` では、`0x6EDC76`、文字列キー `0x15339`、property getter前後 `0x1535E/0x15361` の4点だけをhardware breakpointで観測する。試験用の2キーに限り、同一スレッドのPlayer getterからproperty getterまで一致した場合に戻りポインタの先頭16 byteを記録する。値のタグ・長さ・所有権や別スレッドからの安全性はまだ不明であり、16 byteを数値や文字列と断定しない。RVAとfile offsetは `civ6-pe-rs`、命令境界はLLVMで照合した。観測器のcleanupは登録済みthreadの元handleだけを再利用し、debug registerを変更していないthreadを停止対象から外した。これは実機53の `SuspendThread: Access is denied` に対する候補修正で、実機での解消は未確認である。

実機82では固定GameCoreのproperty停止位置 `0x15339` で `SINGLE_STEP` 異常終了が記録された。旧観測器は初回attach breakpointを`ready`に変換しており、後続のproperty・例外行はない。実機53の `0x2DDDD0` も当時の停止位置であり、同じ種類の失敗が続いた。原因が分かるまで実機向けhardware breakpointを停止し、attachのみの`lifecycle`モードで切り分ける。元の9手順property ABIシナリオは証跡として残し、そのまま再実行しない。

親担当がレビュー後に新しいrelease版観測器を配置した場合の**次の1コマンド**は次のとおり。`live_chain` が共有ロック、起動、attach、停止を管理する。こちらのwriterは実機へ接続しない。

```powershell
D:/CC/Tools/civ6-live/live_chain.exe leaderperson:leader-persona-lifecycle-15km.json --stop-on-failure
```

新シナリオ `tools/civ6-autotest/scenarios/leader-persona-lifecycle-15km.json` は7手順。実機82と同じ設定と最初の数値・文字列property Luaを使い、観測器だけをbreakpointなしに変える。20秒以内にdebug event、thread状態、process exitを記録し、通常detachを検査する。観測器はGameCoreのプロセスメモリや値の先頭16 byteを読まない。正常終了もproperty ABIや資格の合格ではない。詳細は[既存の観測記録](pr86-native-runtime-probe.md)にまとめた。保存直後とロード直後、AI実手番での読取順、費用2/3途中のASSIGN拒否、スパイ中和・回復は残る実機Mustである。

取込候補単体は103試験群・1501件、統合コミット `f3cd5e4` までは103試験群・1516件、次のレビュー修正後は103試験群・1522件、実機82後のno-breakpoint診断追加時は103試験群・1530件、attach出所と対象終了の区別を加えた段階では103試験群・1531件成功していた。今回の `cargo test --locked --manifest-path tools/Cargo.toml --workspace --all-targets -q` は、このworktree専用の `tools/target-native-integrate` で103試験群・1534件成功、失敗0・省略0だった。Lua 5.1の73人物ケース、SQLの22件gate、Windows専用Rust子プロセスでの実開始位置・停止中RIPの照合、先行アプリ例外のdispatcher注入、重大終了コードと終端版の検査を含む。整形と差分の空白検査、7手順シナリオの`--check`も通った。実機でのproperty ABIとnative adapterは未証明・未接続のままである。共有Git indexへの書込みは親担当が行う。

統合前の実機55では、敵AI1の費用1について実Great Personと公式称号1、予約後の本体任命・配置、使用済み称号1、進化完了時の偉人消失まで確認した。実機61では定着、63ではスパイの派遣受理まで進んだ。これは敵側の試験経路の証跡であり、今回の静的閉鎖gateで加入後の資格が開く証明ではない。実機56の後続ロードは保存名の書換えで失敗した。親側の新checkpoint修正と実機確認は今回の観測器修正の合格に含めない。未加入21AIを含む全113ケース、費用2/3の追加昇進、資格の本体adapterと戻り値への接続、propertyの型・寿命・スレッド安全性、配置拒否、保存・読込、複数端末、スパイ中和と回復はMustとして残る。

### 追加した読取コマンド

```powershell
$d = "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll"
$env:CARGO_TARGET_DIR = 'C:/Users/makhmeto/Desktop/dev/ADJ/civ6-banseiki-wt/pr86-native-admission-853/tools/target/native-admission-853-investigate'
cargo run --locked --manifest-path tools/Cargo.toml -p civ6-pe-rs -- --ascii-xrefs $d GetProperty
cargo run --locked --manifest-path tools/Cargo.toml -p civ6-pe-rs -- --ascii-xrefs $d SetProperty
llvm-objdump -d --no-show-raw-insn --x86-asm-syntax=intel --start-address=0x1806edc60 --stop-address=0x1806edc87 $d
llvm-objdump -d --no-show-raw-insn --x86-asm-syntax=intel --start-address=0x1800152f0 --stop-address=0x18001539b $d
llvm-objdump -d --no-show-raw-insn --x86-asm-syntax=intel --start-address=0x1800be3b9 --stop-address=0x1800be3d4 $d
```

## 持続するGameplay APIと所有者別特性付与の実機検証

実機89では、テスト専用`AddGameplayScripts`の同期イベントを公式`EXECUTE_SCRIPT`から呼んだ。9手順と6ログ検査に合格し、`leaderperson-89/steps.jsonl`の8手順目に`source=persistent_gameplay`の結果が残った。本体Gameplayの`Player:AttachModifierByID`、総督の`GetGovernorPoints`と`GetGovernorPointsSpent`は関数だった。一方、同じGameplayコンテキストでは総督の`Appoint`・`Promote`・`Assign`、Playerの`GetTraits`・`GrantTrait`・`AddTrait`、Gameの`RequestPlayerOperation`はnilだった。イベント表の生の要素は見つからなかったが、`__index`から生成されるイベントの有無は判定していない。UIの公式操作APIの有無と混同しない。

実機89の後、`poc/JAC_Test_PersonaTraitGrant`を追加した。これは15kmマップで明示的に有効化するテストMODで、本番の22人のSQLは変更しない。蘆名盛舜1人だけに未割当の専用Traitを設定し、所有者の同期Gameplayが正規Joinと費用全額予約に成功した後、`Player:AttachModifierByID`で`COLLECTION_OWNER`の効果候補を接続する。候補の`EFFECT_GRANT_TRAIT`は公式データの写しに実装例がなく、SQL行が通っても本体の効果handlerが存在する証拠にはならない。偽人事・称号残高・公式総督は書き換えない。

有限の実機シナリオ`tools/civ6-autotest/scenarios/leader-persona-trait-grant-15km.json`は、加入前に22プレイヤー×22人物の484組が閉じていることを確認する。その後、正規Join・称号1個の全額予約・所有者への効果接続を同期操作で行い、UIから本体`CanEverAppointGovernor`の変化を読む。所有者0の蘆名盛舜だけ真で残る483組が偽なら公式`APPOINT_GOVERNOR`を一度要求し、実Governor・基礎昇進・本体の使用済み称号+1を確認してから通常の進化完了へ進む。効果が存在しない場合は9手順目の30秒以内に失敗し、8手順目の実`CanEver`値を残す。成功しても費用2・3の追加公式昇進、AI側の同経路、消失時の資格閉鎖と保存読込は未証明であり、本番の資格を開かない。親担当がこの作業ツリーをsourceRootとして配置した後の1回の実行は、`D:/CC/Tools/civ6-live/live_chain.exe leaderperson:leader-persona-trait-grant-15km.json --stop-on-failure`である。
## 所有者別特性付与候補の実機結果（leaderperson-92）

PR先端7af4686をlive_chainで実行し、再起動を含む18手順のうち9手順が成功した。加入前は22プレイヤー×22人物の484組すべてを本体UIのCanEverAppointGovernorで照合して偽だった。正規Joinで実偉人を作り、称号1個を全額予約した後、所有者0にAttachModifierByIDを実行したことを同期receiptとLua.logで確認した。使用済み称号は0、獲得済みは1だった。その後30秒待っても、対象人物のUI側の資格は偽のままだった。正式なAPPOINT要求は送っていないため、この結果からGameplay本体の要求拒否や効果handlerの不在までは断定しない。実ターン後の効果更新と、本体の公式要求の受理を分けて調べる必要がある。

成果物は共有live作業フォルダのtools/civ6-autotest/artifacts/leaderperson-92にある。SQLとGameplayスクリプトの適用、同期receipt、実測CanEver=falseを保存した。これを加入資格アダプタの成功や、理想の方式が実現不可能である証拠とは扱わない。続くleaderperson-93では人物化と試験MODを無効にした比較を実施し、22勢力で6手順と5種類のログ判定が成功した。両試行後にlive_chainがゲームを閉じた。新しい試験MODは93のenable_modsで無効化を確認した。

## UI資格と公式任命要求を分ける比較

leader-persona-trait-grant-native-control-15km.jsonは、正規Joinと費用予約後に1実ターンを進める。483組の他所有者・他人物は引き続きUI資格の偽を要求し、対象人物のUI資格は値を記録する。未任命と未使用の正式称号を確認してから、対象人物の公式APPOINTを一度だけ要求する。さらに1実ターンを完了し、同期Gameplayから本体の獲得済み・使用済み称号を読む。そのreceiptとUIの実Governor・基礎昇進を照合してNATIVE_REQUEST_ACCEPTEDまたはNATIVE_REQUEST_REFUSEDを分類する。診断の完走は任命成功ではない。称号だけを直接消費する処理、総督を偽造する処理、未加入者の任命要求は含まない。この比較は実機未実行で、費用2・3、AI、保存と回復の製品条件を解消しない。

## 正式任命要求の実測（leaderperson-94）

b025c32の比較は再起動を含む17手順と6種類のログ判定に成功した。正規Joinと称号1個の予約後、実ターン1→2を進めてもUI資格は偽だった。対象本人の公式APPOINTを一度だけ要求し、実ターン2→3の完了後に同期Gameplayの称号会計とUIの実Governorを照合した。使用済み称号は0のまま、実Governorはなく、結果はNATIVE_REQUEST_REFUSEDだった。観測ターンが準備より2以上進み、現在のゲームターンと一致することも確認した。これで、この特性付与候補は実ターン更新と正式要求を通しても任命を開けないと実測した。効果handlerの不存在や、あらゆる本体接続の不可能性は断定しない。

成果物はtools/civ6-autotest/artifacts/leaderperson-94。診断の成功を人物化の成功に数えない。次はRustで本体の共通資格判定へ接続する技術実証を行う。既存の読み取り専用観測ツールの契約は維持し、危険が再現したハードウェアブレークポイントは使わない。現在のGameCoreのABI・値寿命・同期を実測してから、加入と全額予約に結び付く本人だけを許可する。AIの費用2・3の公式昇進と未払い時の配置禁止は別の必須条件として残る。接続未完成の静的閉鎖版を製品合格にしない。

## 実機94の拒否と独立native bridgeの最初の境界

### 加入資格の所有者・property照合（A実機118確認、B実機待ち）

#### 実機118のA段階と費用1の私的B接続

ROOT853の実機118は22手順・6ログ検査に成功し、最終の484組もすべて元の本体資格が偽だった。加入した所有者0の蘆名盛舜だけ試験用派生ID `853304` がtrueとなり、ほかはfalseを維持した。`joined-admission-property.jsonl` のCore thread 146072では、本体getterのreceiver `0x152557b98` が前後のCanEver呼出しから得た所有者0の `Player+0x528` と一致した。同じreceiverでboolタグ1・flags0のpayloadが0から1へ移り、UI thread 148772のcache receiver `0x18061fba8` とは別だった。GameCore固定SHA、両入口の復元、欠落0も同試行で確認した。これは別呼出し間のポインタ寿命やロード時の同期を証明しない。

この結果を受け、`joined-admission` を私的な費用1専用モードとして追加した。固定GameCoreの数値getter入口とCanEver入口を同じPID・作成FILETIME・共有lease・300秒上限のもとで同時に設置する。CanEverは元関数を一度だけ呼び、`this+0x70` のPlayer IDが0、Indexが実行時検査済みの4である呼出しからCore property receiverを登録する。自然に呼ばれたgetterが同じreceiver・固定キー `853304` の24バイトboolを返した時だけ、receiverと許可bitを一つの原子的な値で更新する。Core以外のUI cache receiver、欠損、不正タグ・flags・payload、所有者違いは許可を作らない。所有者0のIndex 4以外は元のCanEver結果を返す。controllerはtrueを構成から渡さない。終了時は許可を先に閉じてから両入口を復元し、property前置領域と中継コードをゲーム終了まで保持する。

試験用の同期GameplayはGame台帳の正規Join、実Great Person、費用1の全額予約を確認した確定点で派生boolを公開する。完了時は偉人を消す前にfalseへ戻し、本体Governor・基礎昇進・称号使用済み数と偉人消失を確認した後にtrueを再公開する。初期値は全484組falseで、数値IDの衝突は試験開始時に拒否する。この公開処理は試験MODの明示flagで有効になり、通常の人物MODだけでは数値資格を公開しない。

ROOT853が同一releaseビルドを配置・レビューした後に実行するBの有限シナリオは次である。24手順の内容は、未加入484組偽、21AIに公式称号を与えた実ターン後の使用済み0・未任命、正規Join後に所有者0の蘆名盛舜だけ真、ほか483組偽、公式APPOINT一度、実Governor・基礎昇進・Spent+1、通常Finalize後の偉人消失である。証跡は`joined-admission-cost1.jsonl`、同期receipt、`steps.jsonl`と各ログで判断する。

```powershell
D:/CC/Tools/civ6-live/live_chain.exe leaderperson:leader-persona-joined-admission-cost1-15km.json --stop-on-failure
```

合成Windows子プロセスでは元関数一回、所有者だけの偽→真、別所有者と別Index、不正なnative値による閉鎖、第二hook失敗時の第一hook復元を確認した。実機でのB成功は未確認である。稼働中の保存・ロード、即時の偉人死亡通知、費用2/3の公式PROMOTEと未払い時ASSIGN拒否、全22人・AI資格の製品接続は残る。Bセッション中の保存・ロード・ターン変更はシナリオ検証が拒否する。AI実ターン検査はhook設置前に行う。

`tools/civ6-native-bridge-rs` に `can-ever-calibration` を追加した。固定SHAの `CanEver` 入口では従来と同じ先頭5バイトだけを置き換え、元の関数を**一度だけ**呼んで `AL` をそのまま返す。呼出し中に `this+0x70` のPlayer、`Player+0xD8` のIDを限定してコピーし、`Player+0x528` をproperty receiver候補として記録する。保存したPlayerポインタを次回呼出しやロード後へ流用しない。読取失敗は失敗状態として残す。別の `property-admission-scoped` は既存の本体getter観測器を数値ID `853304` に限定し、元のgetterが自然に呼ばれた際の24バイトの値をコピーする。これらは資格を開かず、未知のC++ accessorも呼ばない。

`poc/JAC_Test_PropertyObserver` の試験用同期Gameplayイベントは、22人の実行中Governor Indexから `853300+Index` を作り、Game/Playerでの既存値との衝突を全484組について拒否してから、全員をfalseにする。蘆名盛舜（実行時Index 4、費用1）だけを正規Joinし、実Great Personが1体であることと公式称号1個の全額予約、Game台帳の所有者・EVOLVINGを確認してから、所有者0の派生値だけをtrueにする。Game propertyの加入台帳を正本とし、試験用のPlayer値やreceiptから加入・称号を作らない。別の人物・所有者はfalseのままにする。

ROOT853はAの有限シナリオを実機118で実行した。ROOT側で加入後の484組を追加したため実行版は22手順だった。元の資格判定のfalseと所有者0/1をJoin前後に観測し、その間に派生propertyのfalse→trueをGameplay/UI getterで観測した。共有ロックとゲーム終了はROOT853の`live_chain`が所有した。

```powershell
D:/CC/Tools/civ6-live/live_chain.exe leaderperson:leader-persona-joined-admission-calibration-15km.json --stop-on-failure
```

証跡は `joined-admission-before.jsonl`、`joined-admission-property.jsonl`、`joined-admission-after.jsonl`、同期Game receiptと`steps.jsonl`で照合する。前後のnative行にIndex 4、所有者0/1、元のAL=0、UI戻り先RVA `0xBA523`、Player候補と`+0x528` receiverの整合が必要である。property行はID `853304`、boolタグ1、payload 0/1、flags 0、Core/UIの別threadを必要とする。property traceのreceiverと各CanEver行のreceiverが一致しても、別呼出し・別threadのポインタ寿命や同時更新の安全性は証明されない。ROOT853はその差と同期Gameplay receiptの順序を実際の証跡で判断する。

Aの実測に基づく費用1の私的B接続は、CanEver内から新たにC++ getterを呼ばず、同じ所有者のCore receiverに対して本体が自然に呼んだgetterの結果だけを使う。B合格には蘆名盛舜本人だけのnative資格真、他483組偽、公式APPOINT1回による実Governor・基礎昇進・本体Spent+1、Finalize後の実Great Person消失が必要である。費用2/3のPROMOTE、未払い時ASSIGN拒否、保存とスパイ回復はその後のMustであり、Aの成功に含めない。

### 元の判定を保つ接続の実測（実機100）

73aa00dの実機100は10手順と6ログ検査に成功した。新しい共有ロックで、人間0とAI1の同じ人物Governor Index 4について公式UIの資格を設置前と設置中に読み、両方とも偽を維持した。実際の本体入口から2件を記録し、呼出し元の戻り先はともにRVA `0xBA523`（UI wrapperの`0xBA51E`からの呼出しの直後）だった。installとuninstallは0、欠落・処理中件数は0で、元の入口の復元を確認した。安定したスナップショットであり、全呼出し履歴の完全性やAI実手番での呼出しを示す結果ではない。

成果物は`tools/civ6-autotest/artifacts/leaderperson-100/can-ever-original.jsonl`。DLLのSHA-256は`8a4c0fcfd9052ccb27b159b8101767a0dbb9275f006aa653b68e34a818ed8888`。共有実行器はゲームを閉じ、ゲームとロックの不在を確認した。シナリオの結果は成功だが、実行器は完了表示後にファイル使用中のエラー32を返したため、この終了処理の原因確認は別に残す。正式任命の受理・本体propertyの値形式・AIの費用2/3・中和と回復は未達のまま維持する。

このソースのRust/Lua全件検査は106対象・1,609件が成功し、失敗・ignored・filtered outは0件だった。終了保護の別モデル再レビューはMust 0件、Suggestion 1件だった。Suggestionはlease末尾で起動時間分が不足する点で、実機100は新しいleaseから開始して十分な余裕を確保した。今後の短いlease開始に対する起動時間予算の検査は残る。

実機94は17手順と6ログ検査に合格したが、機能の成功ではない。正規Join・実Great Person・公式称号1の予約・`AttachModifierByID`の接続後、実ターンを1回進めても資格は偽だった。対象の公式`APPOINT_GOVERNOR`を一度送った後に、本体使用済み称号0・Governor不在・`CanEver=false`を同期GameplayとUIで読んだため、この操作は拒否と分類した。成果物は`leaderperson-94/result.json`と`steps.jsonl`である。`EFFECT_GRANT_TRAIT`候補を製品の22人へ展開しない。効果handlerの登録成否はこの記録だけでは確定せず、別の公式接続の全否定でもない。

`tools/civ6-native-bridge-rs`は既存の読取専用debuggerとは別のRust DLLと、期限付きの専用controllerである。**現段階は受理を一切変えない。** 固定DX11実行ファイル、GameCoreのSHA-256、DLLファイルとメモリの関数先頭8バイト、実PID・作成FILETIME、標準`live_chain`のleaseを照合してから、対象RVA `0x2DDDD0`の先頭5バイトだけを同じ長さのjumpへ原子的に置く。元の5バイトを近傍trampolineに置き、入口ではRCX・EDX・戻り先だけを上限4096行の専用領域へ記録してから、元の処理へtail jumpする。Lua状態、Player propertyの値や未知のC++値型は読まない。停止時は元の8バイトを原子的に復元して照合する。DLLとtrampolineはゲーム終了まで保持する。ハードウェアbreakpoint、debug register、DRM、Steamファイル、kernel/driverは扱わない。

Windows上の自分の合成関数では、先頭5バイトの差替え前後でRCX/EDX由来の戻り値が等しく、実呼出し元とIndexが記録され、4096件の上限超過が数えられ、復元後も元の関数が動いた。複数スレッドが同じ合成関数を呼ぶ間の原子的な復元も検査した。専用controllerは自分のテストプロセスへのDLL読込を検査し、bridge本体のinstallは実Civ6以外を拒否する。これらは実ゲームのABI・スレッド安全性を証明しない。Controllerは別の`start_native_can_ever_trace`/`stop_native_can_ever_trace` actionでのみ動き、最大300秒、無関係なプロセスメモリは読まない。

レビュー後の修正では、300秒の上限をOS共通のcrate定数に移し、DLL入口でconfigとtraceの全領域・配置・アクセス権を`VirtualQuery`で検査する。gatewayの処理中件数を数え、復元後に最大2秒の静穏期間を取り、未完了slotまたは上限超過を正常なtraceとして受け取らない。JSONLの`snapshot_stable=true`は取得時点の安定した行集合を指し、全呼出し履歴の完全性は主張しない。停止スレッドが記録開始前に後からrelayへ入る可能性は残る。

失敗時の保護は待機時間に依存しない。controllerは遠隔確保・DLL読込・入口変更より前に、照合済みの対象PIDだけを匿名の[Windows Job Object](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)へ割り当てる。所属PIDが1件と確認できなければ遠隔変更せず失敗する。Jobには`KILL_ON_JOB_CLOSE`と子プロセスの`SILENT_BREAKAWAY_OK`を設定し、controllerの異常終了時も最後のJobハンドルが閉じると対象だけが終了する。remote call timeoutや復元失敗ではJobを終了・閉鎖し、保持した対象プロセスハンドルで終了を確認する。共有leaseに30秒の復旧枠を予約する。正常時だけ、install/uninstallの両遠隔スレッドの終了と元の入口8バイトの読戻し、記録の検査を経てからKill-on-closeを解除する。実機ABI・並行動作の安全性はこの合成試験で証明できない。

親担当がsourceRootを組み込む際、`cargo build --locked --release --manifest-path tools/Cargo.toml -p civ6-native-bridge-rs --bin civ6-native-bridge --lib --target-dir tools/target`でDLLとcontrollerを作り、実行器の隣へ置く。固定テストは`tools/civ6-autotest/scenarios/leader-persona-bridge-original-15km.json`で、未加入の蘆名盛舜について人間0とAI1の公式UI判定を設置前と設置中に各1回読む。設置中も資格が元の偽を保つこと、入口・復元の成功と呼出し元RVAを確認する。実行は親の共有leaseで`D:/CC/Tools/civ6-live/live_chain.exe leaderperson:leader-persona-bridge-original-15km.json --stop-on-failure`の1回とする。正常完走後に `can-ever-original.jsonl` の `identity/install/entry/uninstall/stop`、Index、戻り先RVA、行数と欠落数を検査する。実機での資格真、AIの費用2・3の公式PROMOTE、GameCore property値のABI・寿命・同時実行安全性は未確認である。この確認を経る前に資格を真へ変えるhookは実装しない。

### 本体propertyの値を測る次の接続

実機113では数値キー853271・853272だけを記録し、24手順・6判定が成功した。60行を記録し欠落0、除外69,220件だった。公式Luaの真偽値はタグ1・payload 0/1、整数はタグ2・payload下位32 bitの853271と-853271、flags14は0として一致した。欠損は戻りポインタ0だった。実機111で削除後のgetterが戻り値なしを返し、tostringへ渡すログが失敗した点は、変数へ受けてnilに正規化することで直した。112・114の開始比較は各6手順・5判定が成功し、試験MODを無効にしてゲームとロックを閉じた。

実機118の加入資格照合は22手順・6判定が成功した。Governor Index 4の元のCanEverは加入・費用予約前後ともAL=0で、player ID 0/1とPlayer addressから+0x528のproperty receiverを記録した。player0のreceiverはGameplayで853304をfalse→trueへ変えた受信先と一致した。資格関数の呼出しthreadとUIのproperty読取threadは同じだったが、Gameplayのproperty読取threadは別であり、UI cacheのreceiverも別だった。所有者の構造とproperty受信先の対応を実測できたが、資格関数から追加のgetterを呼ぶ安全性を証明した結果ではない。今後の接続は、同期Gameplayが元のgetterを通じて公開した派生値だけを本体側の値へ反映し、資格判定の途中でLua VMや未検証のgetterを呼ばない方式を採る。

加入後も元の資格484組がすべて偽で、派生値は加入した所有者0の蘆名盛舜だけ真と確認した。beforeの校正は2行、afterは18行、propertyは8行で欠落0、入口復元はすべて成功した。118の成果物を保存し、119の開始比較6手順・5判定で試験MODを無効にしてゲームを閉じた。正式な任命の受理、AIの費用2/3と配置の制御は未達である。

実機107は公式Luaの欠損、真、偽、853271、-853271、削除後の欠損をGameとPlayerで確認したが、停止時に記録容量を超えて失敗した。全getterの117,129呼出し中、4,096行を記録し113,033件が欠落した。非欠損の31行はすべてタグ10で、試験値を含まなかった。入口復元は成功し、後始末も成功した。これは部分的な生記録で、値の型や試験キーとの対応を確認した結果ではない。実機108の開始比較は6手順・5判定が成功し、試験MODを無効にしてゲームとロックを閉じた。

d958059では、固定DLLのproperty getter RVA `0x99F5C0`を観測する別モードを追加した。元の関数を一度だけ呼び、戻る値ポインタを変えずに返す。タグ・値・フラグ・呼出しスレッド・受信先・キーIDを有限のバッファへコピーする。これは加入資格を書き換える接続ではない。Actorの人物台帳や称号数は変更しない。

テストMOD `JAC_Test_PropertyObserver`（UUID `d8532710-f853-4c53-8853-271000000001`）は自動変更を行わず、同期操作でGameとPlayerの専用キーだけを欠損→真→偽→853271→-853271→削除へ切り替える。専用シナリオは`leader-persona-property-observer-15km.json`の23手順で、観測中は保存・ロード・実ターンを行わない。各状態を公式Lua getterで読み、本体で記録した型と値、キーIDと受信先を実機で照合する。入口の命令境界と合成Win64関数の返値保持は検査済みだが、このモードの実機結果、値の寿命・同時読取・AIの読取順は未確認である。

## 実機123の資格開放と任命前の停止（2026-10-05）

`dc6347e`の実機123は16/25手順が成功し、17手順目の任命前確認で停止した。未加入484組の本体資格が偽で、21AIに正式な社会制度から称号を与えた実ターン1→2の後も、使用済み称号0と未任命を確認した。フック設置後、正規Join・本人の偉人1体・称号1個の全額予約を受理し、同期receiptのEVOLVINGと所有者0の蘆名の派生値・本体CanEverが真になったことを16手順目で確認した。

17手順目は、所有者と人物状態、22勢力、全484組の資格、CanAppoint、HasGovernor、使用済み称号の照合を一つのLua要求で行っていた。エラーは説明なしのassertion failedであり、任命前のどの条件が不一致だったかは確定していない。正式APPOINTは次の18手順目なので未実行。公式Governor・基礎昇進・Spent+1・Finalize・偉人消失を合格とは数えない。任命前の最終照合は、各getterを一度だけ読み、可否・実体有無・使用済み数とreceiptの値を別名で報告する形へ修正した。

終了時の記録読取も、作成側のCalibrationTraceが版2なのにcontrollerが版1を固定していたため失敗した。DLL SHA-256はE9302C45413B9F4C0DAED1D2E28CF0D6A9489857EE98EB092BA5651B9A7BCEA5で、用意した同一ビルドと一致していた。JSONLはidentity/install/uninstallの3行だけで、calibration/propertyの返値群は保存されていない。元の入口の復元は成功し、controllerの対象終了とlive_chainのゲーム・ロック解放を確認した。

読取側は全3種のtraceで、作成側と同じ型のVERSION定義を使う形へ修正した。現在のCalibrationTraceを自分のプロセスから読み取る回帰検査は修正前に版不一致で失敗し、修正後に成功した。旧版を拒否する検査も保持する。修正後のnative crate全30件と、任命シナリオの形式検査・関連シナリオ検査を行った。実機の再確認は4試行の上限に達したため次の担当へ引き継ぐ。

今回の作業で実機120（診断成功）、121（地図条件の不一致で建設前停止）、122（正式BUILD・進捗・保存成功）、123（上記の任命前停止）を行った。最終実機用dc6347eの全件検査は109対象・1,700件成功、失敗・ignored・filtered outは0で、Luaを全件実行した。変更58シナリオの--checkも成功した。実機123後の版定義修正と診断表示は、この先行版の全件成功へ混ぜず、統合版の全件・再レビュー・release再ビルドと実機を次の担当が確認する。

資格読取の競合は、古いtrueが新しいfalseを戻す問題と、古い受信先のCAS再試行が新しい受信先を戻す問題を、それぞれ修正前に失敗する検査で捕捉した。受信先・session・ticket・許可を一つの128bit状態で公開し、getterと受信先観測の入口で一度だけ採番して再試行では番号を変えない。追補レビューは新Mustなし。採番位置の将来の移動を直接検出する追加検査は、現行コードの位置が正しいことをレビューで確認済みであり、今回の実害修正へ追加せず提案として残す。

全22人の通常の資格接続、AIの費用2・3の公式PROMOTE、未払い時の公式ASSIGN拒否、保存・読込と複数端末、元の人物化通し、スパイ中和成功・回復は引き続きMustである。未加入21AIが任命しなかった今回の1実ターンだけで、加入後のAI経路まで解決したとは扱わない。

## 私的な費用1の正式任命（実機124・126・130）

713c7b88の実機124では、記録版の不一致が解消し、入口復元と20行の読み取りが成功した。一方、任命前の17手順目は再び停止した。全停止条件に説明を付けた1da94640の実機126で、実値は所有者0・人物状態GOVERNOR・準備receiptの段階EVOLVINGだった。人物画面が進化中の人物へ自動で公式任命と進化完了を送るため、試験の手動操作より先に進んでいた。試験自身の18手順目は未実行であり、これらの途中観測だけでは称号使用数や偉人除去までの成功とは数えない。

私的な費用1試験の初期化だけにManualAppoint=1を追加した。試験用のACTIVEとMANUAL_APPOINTがともに1の場合、人物画面の自動送信だけを停止する。通常の自動進化と手動関数、公式可否、人物状態と称号会計は保持する。初期化待ちでも両方のフラグがUIに届いたことを確認する。実際のGameplay・Rules・Viewを読むLua回帰は、修正前に試験側のAPPOINT所有権を失って失敗し、修正後には両フラグ時の停止、片方だけの場合と解除後の通常自動処理、重複送信なし、正式費用と人物除去を確認した。

09999c2cの実機130は25手順・6ログ判定すべて成功した。未加入484組は閉鎖され、正式称号を持つ21AIの実ターン1→2後も未任命・使用0だった。正規Join・本人の偉人1体・全額予約の後、本人所有者0の蘆名盛舜だけの資格とCanAppointを確認し、18手順目で公式APPOINTを一度送った。19・20手順目で本体Governor・基礎昇進・使用済み称号+1を確認した。21手順目で通常のFinalizeを要求し、23手順目でGOVERNOR、偉人0体、人物UNIT記録なし、ほか483組閉鎖を照合した。

DLLのSHA-256はC87FC1D16D4C93CCEB8AB0515114764259916A3B58D8D16DDD9E51A269AD6CEA、controllerはCB4606496D894D2A21F2067D39711D3286B31972C516BF1284A24F77B1EC44F0。54校正行と9property行を取得し、欠落0・復元成功・stop結果okだった。記録は安定したスナップショットであり、全呼出し履歴やスレッドの静穏を証明しない。成果物はtools/civ6-autotest/artifacts/leaderperson-130。ゲームと共有ロックを終了した。

713c7b88の全件検査は109実行対象と10文書検査対象、1,701件成功・失敗/ignored/filtered 0、同headのWindows・Ubuntu・native-driver・CodeQL成功だった。後続の手動所有修正と政府準備の統合版の全件検査では、政府準備シナリオに関する2件が失敗した。準備手順の追加で検査側の固定した手順番号がずれたため、操作と段階の意味で一意に選ぶ形へ修正し、関係する96件は成功した。最終統合版の全件成功とはまだ数えない。全22人とAI、費用2・3、配置拒否、保存と複数端末、元の通し、中和・回復は未達のまま維持する。

## 開始比較と検査の再確認（実機137）

コミットc3102fd8の通常構成で、実機137は開始比較の6手順・5ログ判定に成功した。22勢力、人間0の手番、本体の処理待ちなしを確認した。ターン進行・保存・人物加入・支払いは行っていない。成果物は`tools/civ6-autotest/artifacts/leaderperson-137`で、共有実行器がゲームを閉じ、ロックを解放した。

同コミットに対する親担当の全件検査は、111実行対象と10文書対象、1,707件成功、失敗・ignored・filtered outは0件だった。別の作業フォルダーの先行検査へ読み替えず、記録`D:/CC/Work/pr86-redesign-853/workspace-c3102fd8-summary.json`と対応する実行ログで照合した。mainとの差分に含まれる58シナリオも、Rust実行器の`--check`ですべて成功した。形式検査は実機の完走を意味しない。

AIが費用2・3の支払い用昇進を自発的に選ぶ経路は、引き続き未実証である。公式昇進定義にはAI選好の列がなく、汎用のAI選好設定から総督昇進へ届く公式の接続も確認していない。人間のアクティブ手番だけで動く人物画面を、AIの支払い実装として扱わない。次の確認は通常の加入資格と配置観測の接続後、伊達稙宗（費用2）と上杉憲政（費用3）のAI所有者について、画面から昇進要求を送らず本体の可否・取得反映・使用済み称号を有限の実手番で照合する。取得しない場合も、正式な昇進会計を通る本体操作入口を実測するまで、架空の減算や未確認の関数呼出しで成功を作らない。
## 通常22人の派生資格（ToDo1399・1447）

通常Gameplayの権限源は同期Game台帳、実Great Person、Coreの公式称号である。UI cache・私的Bの853304・UI人物資格は入力にしない。Governor Indexは実行中のGameInfoから求め、22人の重複・欠損・範囲外を拒否する。

公開キーは、対応表900000+Index、資格901000+Index、所有者別の失効902000+owner、人物セルの閉鎖903000+Indexである。初回は各公開範囲と所有者別失効キーの衝突を確認する。死亡中も含む既存の主要文明を調べ、復活後に別MODの値を上書きしない。同期Gameのschema値3がこの範囲を所有する。旧保存の値1では新閉鎖範囲、値2では新しい所有者別失効キーを確認してから移す。衝突または既存キーの読取失敗なら、全所有者を失効して公開を中断し、既存値を上書きしない。所有者一覧は0〜63の重複しない整数を持つ連続配列と確認し、穴や不正値があれば全所有者を失効して公開を止める。Luaの揮発性スナップショットは前回公開値の比較だけに使い、権限源にしない。所有者の操作はその所有者を、各PlayerTurnはその所有者を、全体Turnは全員を再計算する。変化のないセルの数値propertyは読み書きしない。加入直後・EVOLVING前後・正式Finalize前後も同じ処理である。

初回・Lua再作成・所有者オブジェクトの変更は、各所有者の失効キーを自然なCore property getterから読む。そのキーに含むownerと今回の受信先をnativeが原子的に結び、同じ入口で旧世代を閉じる。Gameplayに存在するか未実測のCanEverAppointGovernorへ依存しない。全所有者の失効値trueの読取を確認してから公開を開始する。GameInfo対応表が変わった場合と所有者の消滅時も以前の所有者を失効させる。保存されたtrueだけでは再公開しない。native接続がLua初期化より遅かった場合、比較用スナップショットから自動的には再公開しないため、親は接続を先に用意してGameplayを初期化する必要がある。任意の途中接続を製品完成とは扱わない。

変更されたセルは、閉鎖キーの読取、資格falseの読取確認、対応表trueの読取確認、必要な場合だけ資格trueの読取確認の順で公開する。全変更セルを閉じてから新しい値を公開する。falseまたは対応表の確認に失敗すれば所有者全体を失効し、その自然読取も確認して中断する。失敗したスナップショットは捨てる。失効の読取が不正型でもnative側は入口で失効する。古いtrueがfalseの直後に戻る場合、nativeは新しい閉鎖世代のfalse確認がないため許可しない。

nativeは各owner/Indexに受信先・session・ticketを持つ閉鎖、false確認、対応表、許可の有界セルを使う。所有者別の失効getter入口では、まずownerの原子状態を閉じ、その後に同じ受信先の他owner結合を整理する。同時CanEverがこの間に走っても古い許可を返さない。人物セルの閉鎖もgetter入口でticketを進め、返値の正誤に依存しない。新世代の正しいbool falseを確認した後でだけ対応表を公開でき、その後の資格trueだけを許可とする。旧世代・遅れた読取・所有者再利用・旧session・不正tag/flags/payloadを拒否する。許可を返す直前に、所有者・閉鎖・対応表・資格が変わっていないことを再確認する。別MODの通常Governorは元ALを保ち、対応済み人物の未知の所有者は閉じる。

22所有者×22人の再ロード初期化は、失効22回と3段階×484回で1,474回の対象getterになる。初回の名前空間確認も含めると2,948回で、property観測4,096件以内である。変更のない300ターンで全体と全22 PlayerTurnを実行しても追加の対象getter・公開・Core binding anchorは0回である。所有者の初回bindingは失効getterへ統合した。この数値はLua 5.1合成試験の測定で、実機時間の測定ではない。

観測と資格判定を分離した。通常mode6の校正は1,024行、propertyは4,096行の先頭サンプルだけを保存する。超過呼出しも原関数を1回呼び、資格判定を行う。原関数の前後の本人情報は各呼出しのスタックに持ち、サンプル行がなくても同じactor/sessionで判定する。自然なproperty返値はサンプルの残量に関係なく消費する。容量不足で許可を閉じたりcontrollerを失敗させたりしない。ticketの実際の枯渇は別であり、許可を閉じる。

校正traceは版4、property traceは版3であり、DLLとcontrollerは同じ型のVERSIONとサイズを照合する。mode6のcounter_scopeはbounded_prefix_samples_not_complete_call_history、sample limitは1,024/4,096、droppedは未保存の呼出し数である。全履歴の完全性・全22人の動作成功をこのサンプルだけから主張しない。他の診断・私的Bモードの厳密な記録契約は保持する。有限接続時間は最大300秒のままで、原関数1回・全入口復元・所有プロセス回収の契約を変えない。

不正型の公開行には、その自然返値を消費した直後の同じ所有者・Indexについてnative consumerが閉鎖を確認した結果を添える。成果物検査はその結果とCore受信先の所有者対応を照合する。さらに、同じCore受信先・所有者の型付きtrue失効読取が先頭サンプルにあることを要求する。該当Indexや所有者が違う閉鎖、確認の欠落、完全履歴を装う上限超過は受理しない。サンプルにない後続イベントまで閉鎖済みとは扱わない。

### Core Governor getterの実機診断と残り

GOVERNOR段階の本人への支払照合は未解決である。Gameplay HasGovernorは実機54/89で不在、GetGovernorの存在は未確認である。CorePaymentCompleteは両正式bindingと本人の基礎・必要な追加昇進を読める場合だけ許可し、UIの値で代用しない。fixtureで両メソッドがあることは本体の証明ではない。HasGovernor欠損のnegativeを維持する。

親の限定診断は、live_chain所有セッションで費用1の本人が正式APPOINTされた直後とFinalize直後に、Gameplay内の次の関数を1回ずつ呼ぶ。任命・称号・配置・記録の書換えは行わない。

    local r = ExposedMembers.JAC_LeaderPersona.CoreAdmissionCapabilities(0, "LEADER_JAC_ASHINA_MORIKIYO")
    for _,k in ipairs({"context","owner","leader","index","state","ledgerOwner","available","spent","hasGovernorBinding","hasGovernorOK","hasGovernor","getGovernorBinding","getGovernorOK","governorReturned","baseOK","base","paymentOK1","payment1","paymentOK2","payment2"}) do
        print("JAC_CORE_GOV_BINDING " .. k .. "=" .. tostring(r[k]))
    end

診断は自然な正式Lua bindingのHasGovernor/GetGovernorを各1回だけ呼び、取得できた本人の昇進を読む。bindingがない場合もその不在を返し、別のgetter・未確認RVAを呼ばない。両bindingが存在するなら費用2・3の同じ本人について支払昇進を実測する。存在しなければ、親は本体の正式イベントまたは自然なnative queryの本人情報を用いた支払証明へ次段の設計を進める。総使用済み数だけ、UI cache、未確認30CBE0/30C100の呼出しでは紐付けを作らない。

配置担当1433への契約: 901000+Indexは未任命EVOLVINGの全額予約もtrueになり、追加称号の支払済み配置資格ではない。PaidAssignmentは別の派生bool/manifest範囲と閉鎖キーを設け、同じCore actor/Index・session・getter入口ticket・false ACKの原子的な公開契約を使う必要がある。今回その配置範囲を予約・公開しない。配置の4引数ABIへ任命のIndex入口を流用しない。

実機の通常22人、AI追加公式PROMOTE、未払いAssign、保存/MP、元113通し、中和・回復、常時の製品接続、別モデルの固定全差分レビューは親853/1399/1433の未完了である。04b単体を統合せず、今回の修正も合成成功だけで製品完了・マージ可とは扱わない。私的manual fixtureの親8d/099修正と最新spy修正は、親が3-way統合で保持する。

単回起動の実行制約に合わせ、形式検査でも同じゲームプロセス内の2回目の昇進観測を拒否する。property観測と私的な加入観測も、同じproperty入口を共有するためまとめて単回とする。観測停止後のrestart_gameだけが再起動を許し、観測中の再起動は拒否する。入口記録の終端ではquiescence_proven=falseと、完全な呼出し履歴ではない範囲の明示も必須にした。

leader-persona-promotion-call-calibration-15km.jsonは、既存の私的な費用1の任命シナリオを最後の終了以外すべて保持する。元の資格観測を解除し、公式Civicによる予備称号を用意してから新観測を開始する。既取得の基礎昇進に正式PROMOTEを一度要求し、CoreとUIの使用済み称号が任命の1個のままなのを照合する。停止時に観測行0件なら失敗する。4つのRust検査が新しい実Luaの正負、誤所有者、未取得の基礎昇進、支払い増加、前の観測を閉じない場合、再起動なしの2回目を拒否する。これはAIの支払い実装ではなく、既取得昇進の拒否経路を測る校正である。

## 実機137〜140と昇進呼出しの校正

この担当の4試行を終了した。137は通常構成の開始比較6手順・5判定に成功した。138は元のスパイ655368の検索が別個体786440を返す保存を読み、元の実体をpresent=falseと判定し、別個体の操作・座標・終了ターンを混ぜない照合を6手順・6判定で確認した。139は私的な費用1の正式任命・Finalizeと元の観測解除まで通過したが、25/34の予備称号準備で画面側の制度変更APIがnilだったため停止した。新しい昇進観測は設置していない。

修正後d1a07dfeの140は34手順・6判定すべて成功した。予備称号をCoreの正式SetCivicと必要な前提の取得で用意し、既取得の基礎昇進に正式PROMOTEを一度要求した。PlayerGovernorsの可否はfalseで、CoreとUIで使用済み称号は任命の1個のまま、予備称号と基礎昇進を保持した。入口観測は1行、欠落0、記録処理中0、元のCALL変位の復元成功、終端result=okだった。quiescence_proven=falseであり、戻り値の直接観測・全呼出し履歴・AIの支払いを示さない。

実機のcontrollerはSHA256 34323348C27B6B1E9AA2F3A917BC198494E966413C8F5072871E394E6E68DD8C、DLLは9CDFE71A15E35F7D9C8A2401BAE76F4BCE8497DEA4E365863E551B21038E7124で、配布先と一致した。4回とも共有実行器がゲームを閉じた。追加の実機はこの担当では行わない。

d1a07dfeで固定したRustワークスペース全112実行対象・1,723件は成功し、失敗・ignored・filtered outは0件だった。Luaを全件実行し、変更59シナリオの--checkと整形・差分検査も成功した。別モデルの修正差分レビューは新Mustなし。複数段のCivic前提・共有親を持つfixture拡張と、盤上にいないスパイがUI要求とCore処理の間に現れる競合の確認は、提案・確認項目として残す。通常22人の資格、AIの費用2/3、未払い配置の拒否、途中保存・複数端末、人物化の通し、中和成功と回復の未達は、この成功に含めない。

## 実機141〜144：開始比較、捕虜API不足、配置の4引数校正

この担当の4試行を終え、PR #86は未マージである。141はeddd2674の通常構成で6手順・5判定に成功した。142はe3c60a96の追加スパイ採用で5/9手順が成功した後、Gameplayの捕虜・offMap API不足で拒否された。元の失敗イベント0・ターン23と、実機134のチェックポイントc1a6eacce7004f01b1ff123aac70581aを保持し、保存は更新していない。

143はe3c60a96で26/31手順を通過した。私的な費用1の正式Join・APPOINT・基礎昇進・使用済み称号1・Finalize・偉人除去と元の観測解除を確認した。配置観測はUIのCanAssign呼出し1行を記録したが、次の正式要求がInGameにないJAC_AssignLeaderを呼んで停止した。人物画面の関数はJAC_LeaderPersona_Viewにある。入口の復元とstop結果okを確認したが、本体の配置更新は未実行だった。

呼出し先を宣言元の人物画面へ直したd49d485eの144は、31手順・6判定すべてに成功した。正式ASSIGNを一度送り、本体Governorが所有者0の都市65536（134/29）へ配置されたことを確認した。使用済み称号は任命の1個のままで、基礎昇進も保持した。

配置観測の3行はUI戻り先RVA 0xBA3E2が2行、本体の配置更新戻り先0x2DD375が1行だった。全行がowner0・Governor Index4・同じGovernors/Player/Governor/Cityのポインターを持ち、原ALと返却ALはともに1だった。Cityのraw値は+0xA8=65536、+0xC0=134、+0xC4=29、+0xD8=0でUIの都市ID・座標・所有者と一致した。raw_r9はすべて0で、この観測からポインターや引数全般の意味を推定しない。

欠落0・記録処理中0・元の入口復元成功・終端okを確認した。quiescence_proven=falseで、全呼出し履歴や遅延した呼出しの不存在、保存後のポインター寿命は証明しない。実機controller SHA-256はCB87F471A744BBC2D3F90A2246E458BEE73156D27CDB4317B7B6C9BD96F646D2、DLLはB7C1137A5565742131206F0E98254598453ECBB797BB985063354667B2A3919Fで、用意した同一ビルドと一致した。

d49d485eの指定Rustワークスペースはトップレベル114対象・1,765件が成功し、失敗・ignored・filtered outは0件、Luaは全件実行した。別プロセスのWin64 fixture1件も成功した。その子プロセス内部の29 filtered outは隔離fixtureの選択で、Luaの省略や成功の二重加算には使わない。変更60シナリオの形式、整形、差分検査も成功した。画面環境の回帰検査は修正前にInGameと宣言元の不一致で失敗し、修正後に成功した。

4回ともlive_chainがゲームを閉じた。この担当は5回目を行わない。通常22人の資格と製品接続、AIの費用2・3、未払い配置拒否B、途中保存・複数端末、元の人物化通し、本体中和・功績停止・回復は未達である。144は原判定を保持する私的な費用1の配置観測Aであり、これらの受け入れ結果へ広げない。スパイのAPI不足への接続修正は別作業フォルダーで準備中で、今回の成功に含めない。

### 支払い途中の予約額を人物別に計算する

全差分の別モデルレビューで、EVOLVINGの各人物の費用全額を再予約する新しいMustが見つかった。費用3の人物に正式任命と追加昇進1回の計2称号を払い、費用1の別人物も予約した場合、獲得4・使用2の残り2で両者の未払い分を払える。それでも全額4を要求し、後者を任命候補から閉じていた。

bbaf3494では本人の正式Core総督・基礎昇進・順序どおりの支払い昇進を読める場合、支払い済み分だけ予約から除く。正式な本人支払いの合計が使用済み称号より多い場合、昇進の順序が違う場合、本人の読取結果が矛盾する場合は許可しない。Coreの本人getterがない場合は全額予約を保ち、称号使用総数やUIの値から支払いを推定しない。任命資格と全額支払い後の配置資格は別であり、この変更は未払い配置を許すものではない。

費用3と費用1の同時予約を正式操作で再現する検査は修正前に失敗し、修正後の人物資格15件は成功した。人物の基本動作74件、既存加入10件、本体任命条件12件も成功した。不足残高、使用済み数と本人支払いの矛盾、Core getter欠損時にUIの観測が残る場合、昇進順序の違い、再読込後の予約再計算を含む。これは模擬Core bindingの検査で、実機で不足している本人支払いの証明を解決した結果ではない。

スパイの名前に対するレビューの提案も反映し、正式なSAVE_TRAVELとRETURNへ空文字・数字に見える名前の送信と保存の型変換を与えた。敵側のLua122件が成功した。実機148の後にゲームを起動しておらず、通常22人・AI追加支払い・未払い配置・途中保存/複数端末・通し・中和と回復の実機未達は保持する。
bbaf3494で固定した指定Rustワークスペースは114対象・1,775件が成功し、トップレベルの失敗・ignored・filtered outは0件、Luaは全件実行した。別プロセスのWin64 fixture1件も成功した。子プロセスの29 filtered outは隔離fixtureの選択であり、Luaの省略や成功件数の二重加算に使わない。専用ビルド先で削除済みの別作業フォルダーへの参照を除き、全件を検査した。変更60シナリオの形式検査、整形、差分検査も成功した。この予約修正の追加実機は行っていない。
別モデルの再レビュー20261005-113435-rev-leaderpersonは、人物別のCore支払いを読める場合の予約計算が直ったと確認した。Core getterがない実機では全額予約が残り、この問題の実機解消には本人支払いの正式な証明が引き続き必要である。通常22人の本体資格・有料配置などの既知Mustも残るため、PR全体は未承認・未マージとする。送信形式ec64e672と保存形式358c24fdの各レビューでは新しいMustなしだった。帰還と派遣記録の検査不足の提案は追加検査で対応した。仮確認中の手動保存の復旧、通常資格の本人一致したfalse→trueの実機観測は確認項目として保持する。

### 同じ配置呼出しで本体本人の昇進を読む

853の再設計では、Gameplayに存在しないgetterを模擬Coreで補う検査を増やす前に、正式Governorが持つ昇進配列を直接観測する。固定GameCore SHA-256 `324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c` の `0x259980` はGovernor+0x68の配列をPromotion Indexで読む。`0x2598F0` は同じ配列のGovernor+0x78個の非ゼロ値を数え、`0x2DEA90` が各Governorの値を合計する。`0x259990` は容量+0x70を確保してから長さ+0x78を公開する。`0x2DF340` は人物Indexが一致する本体Governorの同じ配列へ正式昇進の1を置く。これらは逆アセンブルによる静的根拠であり、今回の実機受入ではない。

Rustの `governor_payment` は現在のCanAssignの二つの引数だけを使い、Governors+0x70のPlayer本人と+0x58の本体リスト、Governor+0x18の所有者と+4のIndexを照合する。256件以内の循環しないリストと4096件以内の昇進列を読む。長さ、容量、本人、所有者、全昇進byteを再読取し、不一致・読取不能・0/1以外を記録上の拒否として返す。未知の関数は呼ばず、ゲームのメモリへ書かず、ポインターを次の呼出しへ持ち越さない。二重読取は大域的な静止の証明ではない。

本体リストの次リンク+0x10は、正式PROMOTEの走査 `0x2DF366–0x2DF38A` と本人lookupの `0x2DE960–0x2DE984` が同じ位置を読むことを命令境界から確認した。純Rust検査では本人が先頭・中間・末尾にある三つの配置と別所有者を扱う。昇進配列は256 byte単位で二度読み、残りだけ8 byte以下で読む。4096件なら配列のOS読取は32回で、先頭本人の構造照合を含めても64回以内である。この回数はfixtureの測定上限で、実機の処理時間ではない。

mode7のtrace版2は `promotion_snapshot_status`、本人所有者とIndex、昇進数、取得済みPromotion Indexを追加する。status 0だけが読取完了、1は読取不能、2は構造不正、3は本人不一致、4は本体リストの非所属、5は読取中の変化、6は不正なbyte、7は未観測である。失敗時は本人値を未知、件数0、取得一覧を空にする。検証器は型・順序・重複・本人一致を検査する。全行の `payment_authority` はfalseであり、原ALとRAXは従来どおり保持する。

この観測を費用1の実Governorと正式基礎昇進へ照合し、その後費用2・3のTITLE_1/2の実Index、公式PROMOTE前後の本人昇進とSpentを照合する。実機で未証明の段階ではPaidAssignmentの904–907名前空間を公開しない。通常22人、正式な本人支払い、未払い配置拒否B、保存・複数端末、人物化通し、中和成功と回復のMustは継続する。

支払数を読む純Rust関数は `diagnostic_paid` とし、要求された所有者とsnapshot本人が違えば拒否する。費用と基礎・TITLEのIndexの形や順序は検査するが、呼出側が渡すIndexの出所までは証明しない。そのためこれは診断用の解釈に限定し、製品からは呼ばず、snapshotと同様に支払権限へ使わない。製品接続では、そのゲームの同期GameInfoに対応する本人・所有者・各Promotion Indexを照合する生成元を先に完成する。この確認項目を、名前だけの検証済み定義で済ませない。

### 通常の費用2・3と任命途中の保存を確認する検査

ToDo880の実機`agent-todo-880-4c6129c40c8c-01`は、e4f76aeaで21/37手順まで成功した。Coreの閉鎖記録とUIの未加入484組、通常Joinと全額予約、伊達稙宗だけの任命許可（他483組は拒否）、公式APPOINT一回、費用2のうち本人支払い1と偉人一体の保持、観測復元、途中保存を確認した。22番の読込は、保存側がcheckpointの一意な物理名を発行したのに読込側のcheckpoint指定が欠けていたため停止した。費用2全額やCore本人昇進の観測成功には数えない。

生成器は保存と読込の両方でcheckpointを使うよう修正した。既に作った途中保存からは`--resume-partial`へ成功したsave記録の物理名（拡張子なし）を渡す。新規ゲーム設定・Join・Evolve・APPOINTを再送せず、読込直後に本人・所有者・費用・既払1・残った偉人を照合してから未払い昇進を一度要求する。今回の再開元は`JAC-PR86-Payment1720-Cost2-Par-ac0b49e106ae4347b2041107afa1a313-0000000000000001`で、原保存のハッシュを確認して保持した。中和任務のターン45保存とは別の新規診断である。

検査用の監視はGameplayの初期化後に設置されるため、設置直後にCoreから既存の閉じた資格を読む。初期化済みの値は、通常の同期処理では変化しない限り再送されない。Rust生成器はschema 3、22人全員の未加入、22所有者と人物Indexの一意性を確認し、所有者reset・閉鎖・boolのfalse・manifestの順で自然なGetPropertyを呼ぶ。その後に484組のCanEverを検査する。保存値の書換え、資格の新規発行、手番進行は行わない。古い許可や欠けた記録は失敗とし、加入・任命へ進まない。これは新規ゲームの検査を開始する手順で、常時接続や保存復元の製品受入の代用ではない。

ToDo1720では、Rustのciv6-persona-payment-scenarioから、伊達稙宗の費用2と山内上杉憲政の費用3の有限シナリオを作る。従来の人物化シナリオと同じ22人の新規設定を使う。私的な蘆名用資格を使わず、通常のproduction-admissionと実行時の人物定義を読む。公式社会制度を完成して全額の称号を用意し、同期した通常Join・Evolveを一度ずつ要求する。任命前は他483組が閉じていることも照合する。既存の二つの検査用フラグで画面の自動操作だけを停止し、送信元を一つにする。

公式APPOINTの後は本人総督と基礎昇進、使用済み称号+1、本人の偉人が一体残ることを要求する。資格の観測を復元してから別名保存を作り、その保存を再読込する。通常Join・Evolve・APPOINTは再送せず、同じ基礎昇進と未払いの追加昇進、残った同じ偉人を照合する。TITLE_1、費用3ならTITLE_2も公式PROMOTEで一度ずつ支払う。各段階で本人の昇進の順序と称号使用数を照合し、全額の後にだけ通常Finalizeと偉人の除去を要求する。

mode7の観測は、ownerとleaderを指定すると、そのゲームのUI定義と実Governorから本人Indexを読む。読取前に確認したゲームのPID・起動日時を、観測Sessionの実FILETIMEへ束縛する。subject.jsonにもその組を残す。この指定はhost側で照合する対象だけを変更し、DLL・本体の原AL・関数呼出し・昇進配列は変更しない。所有者・本人Index・都市所有者と同じポインター組がUIと本体更新の両方にあることを必須にする。指定を省いた従来の検査は所有者0・蘆名Index4のままである。

最終UI受領は1595担当の検証器と同じ14項目である。実行時の基礎と支払昇進のIndex、全取得一覧、最初にCoreで保存した称号使用数との差、同じ首都の座標を含む。観測中に、その首都へ全額支払い済み本人の正式配置を一度送る。シナリオの合格だけではCore本人支払いの証明としない。同じ候補から作った1595の独立した検証器で、同じSession・プロセスと全ての本人観測行を照合する必要がある。

生成器と対象指定の接続は実機未受入である。通常22人全員の肯定側資格、AIの実手番・本人支払い、未払い配置拒否B、複数端末、元113手順、中和成功と回復は別の未完了条件として保持する。人物の費用と本体原残高を変更せず、ターン45の二つの失敗保存にも触れない。

### 本人の昇進を画面の実定義と照合する診断

ToDo1595は、追加スパイ採用の後続結果を統合し、費用1の本体本人昇進を独立した画面読取と照合する診断を追加する。追加スパイの分離はToDo1461の実機138・148・149と別モデルの監査で達成した。正式中和の最新状態は実機160のターン45・失敗・捕虜2であり、その保存と23/45の履歴を保持する。巻戻しや過去の任務の再送はしない。

Rustの `civ6-persona-payment-check generate <出力JSON>` は既存の費用1配置シナリオの全手順を保持し、配置観測の直後に独立した読取だけを挿入する。同じゲームの画面側GameInfoから本人のGovernor Index、基礎Promotion Index、昇進表の長さと取得済み一覧を読む。都市は元の配置要求と同じく、自国の都市をID順に並べた先頭を使う。公式称号の使用数と費用1の加入記録も照合する。読取は本体の値や人物台帳を書き換えない。

`check <本体trace.jsonl> <steps.jsonl>` は同じ成果物フォルダーの記録だけを受け付ける。実行器が開始時に保持するOSのPID・作成FILETIMEを、UIを読む前後の現在のゲームと本体traceの識別へ照合する。現在のUI読取が同じ観測元であることを実行器が確認してから、その識別を読取結果へ付ける。開始、観測中の画面読取、同じ実ファイルへの観測復元、メニューへの終了、成功した最終判定の全記録を要求する。本人の所有者・Index・都市・全昇進をUIとCoreで照合し、昇進の順序と使用済み称号の差を確認する。本人を読めなかった行を番兵の所有者で除外せず、読取失敗が一つでもあれば、この小さい観測全体を不合格にする。正常行だけを取り出したり、最後の正常行で失敗を覆ったりしない。

出力は有界な読取診断であり、支払い権限には使わない。費用2・3の合成fixtureは実機の支払い証明ではない。任命途中の追加支払い、未払い配置拒否、通常22人、AI、保存・複数端末、元の113手順、中和成功・功績停止・回復の必須条件は別々に保持する。新しい実機の成功は、成果物を取得して本検証器で照合した後に追記する。

### 明示runtimeの寿命と追加修正の検査範囲

`paid-assignment-runtime` は観測を終えても三入口、判定状態、中継コードと固定記録領域を同じゲームプロセスに保持する。加入した人物のCanEverはmode 6と同じく、本体の原ALが0でも同期済み任命資格を1として反映する。実機118で元の本体資格が偽だった事実を変更しない。元falseをtrueにしない制限はCanAssignに適用し、現在の呼出し本人の本体昇進を毎回確認する。

resetはsessionと開始ticketを比較するCASで所有者の束縛を更新する。状態も同じsession・ticketを保持し、世代を更新してから再構築中へ進む。新しいresetが全カタログを公開した後に古いresetが復帰しても、所有者・ready・呼出し世代を変更しない。検査では旧resetをticket取得直後と世代更新直前の二か所で止め、新resetの自然getterを通した全公開とreadyを確認してから旧resetを再開する。世代更新待ちの束縛では任命・配置とも閉じる。合成検査は実機ロードの証明ではない。

runtimeのCOMMIT権威は所有者ごとの単一transaction wordへ統合した。128bit CAS値にbindingstamp（sessionとreset ticketの58bit）、begin ticketの31bit、latest commit ticketの31bit、phaseの2bitを保持する。payloadの受理、begin・latest・phase更新を同じCASで行い、COMMITセルと状態の二段更新を廃止した。legacy COMMITはbounded経路と記録だけに使い、runtimeの判定からは読まない。falseはそのticketをbeginとして直ちにRebuildingへ進み、invalidはbeginを消してClosedへ進む。trueはRebuildingの同じtransaction snapshotで全22定義のbeginより後・trueより前のticketを照合し、copy前後の同wordと全bitの一致を確認してReadyへCASする。Readyを新たなtrueの開始状態として使わず、重複trueはClosedへ進めて別falseを要求するため、前のcommit境界を広げて後発bitを借用しない。reset、世代更新と判定もこのwordに束縛し、旧returnはlatest ticketで拒否する。

固定並行検査ではowner resetを挟まず、旧COMMITのfalse・true・invalidを、受理前・原子的受理直後・世代更新前で止める9順序を維持する。同じbindingで別の定義を含む全カタログと新trueを公開してreadyを確認してから旧callbackを再開し、権威word・世代・owner bindingと任命・本人配置判定が変わらないことを要求する。追加検査ではfalseとinvalidの受理直後を止め、別falseを挟まず部分bitとtrueを送って拒否を要求する。eceの二段更新ではこの検査が失敗した。Ready中の重複true、旧境界を越えるbit、Closedへの連続trueも拒否し、別falseと全bitコピー後だけ回復する。falseの受理時に旧beginを残すmutationでも失敗することを確認する。

旧callの拒否は遅延し得るepoch callbackから独立させた。両入口の開始時にsessionと独立したentry ticketを取得し、その呼出しのownerとCore receiverをstackへコピーする。共通caller fenceは終了時に現在のowner/receiverを読み直し、同一transactionのreset・begin・latest commitが開始ticketより新しければ拒否する。観測行や保存pointerはこのfenceの権威にしない。CanEverは既存のstack scratchにticketを保持し、CanAssignはshadow/引数とSIMD退避から独立した24byteのstack fenceを渡す。四引数gatewayのstack allocationに合わせてSEH unwind metadataも更新する。元falseをtrueにしない制限は引き続きCanAssignだけに適用する。

検査は旧call開始→falseの原子的受理直後で停止→別false/resetなしで全bit・true・本人支払いを公開→epochが旧Gのまま旧call終了、という順を固定する。旧CanEver/CanAssignはAL0、新callはAL1、原CanAssignのAL0は0のままであることを対で要求する。観測中と観測期限後の両方で実行し、遅れたfalse callbackが破棄されても同じ結果を確認する。これは実機SEH離脱の証明ではない。

通常runtimeの設置時は固定SHAのGameCoreとbridgeをWindowsローダーにpinし、通常のFreeLibraryによる寿命切れを防ぐ。一方、実機のGameCore unload、強制unmap、別baseへの再ロードと再設置は未証明であり、対応済みとは扱わない。property prefixと中継コードは一度だけ設置する契約を維持する。Close/Rebuildは判定状態の閉鎖・再構築であり、三入口の再設置APIではない。ロード開始前の実在フックへの接続は別Mustとして保持する。

通常起動担当へ渡すABIは`BridgeRuntimeInstall`・`BridgeRuntimeStopObservation`・`BridgeRuntimeClose`・`BridgeRuntimeRebuild`の四exportで、いずれもWindows x64の`extern "system" fn(*mut c_void) -> u32`である。Installの入出力は`#[repr(C)] BridgeConfig`（40byte・align8）を使う。フィールド順はversion/size/game_pid/max_seconds（各u32）、created_filetime/trace_address（各u64）、trace_size/mode（各u32）。version=`0x0324_0002`（ToDo324の別領域プロパティ記録で`0x0324_0001`から変更）、size=40、mode=6、入力trace_address/trace_size=0、max_seconds=1..300は観測時間だけである。PIDと作成FILETIMEは現在のゲームへ完全一致させる。返値0のときだけ同じ設定領域にゲーム所有のtraceアドレスとサイズが返る。property生行ログの位置はdata export `BRIDGE_RUNTIME_PROPERTY_LOG`（`AtomicPtr`）の現在値で公開する。その他は1入力不正、2プロセス不一致、3GameCore不一致、4入口不一致、5設置失敗、6重大失敗、99panicである。Closeは本体オブジェクト破棄前に呼ぶ必要があり、Rebuild後も所有者ごとの自然resetと公開が必要になる。このABIの記述を、通常起動や実在するロード前フックへの接続成功とは扱わない。

controllerはrel32の接続先と中継命令を確認する。ロード済みbridgeのdata exportから入口ごとの期待gatewayを読み、DLL内の別functionでも拒否する。停止後は入口8byte、property prefix5byteと中継14byteを設置時と完全一致で比較する。callbackが終了へ到達せずcounterが残った場合は観測停止を不完全として拒否し、判定を解除しない。これは未終了callbackを使った合成検査の範囲であり、実機SEHがcallbackをどのように離脱させるかの証明ではない。通常Luaの試験フラグ依存を外す接続、本人会計、AIの正式支払いと通常runtimeの実機証明は引き続き未達である。

### 費用2の全額保存と本体の所有者を読む位置

実機agent-todo-1690-fb7308f2658d-01は4faae9f1で19手順・6判定が成功した。共有実行器が元の18手順へ最初の再起動を加えている。原任命途中保存を読んで本人の既払1と残る偉人を照合し、正式PROMOTEを一度だけ要求して既払2、通常Finalizeによる偉人の除去、全額保存、正式配置を確認した。公式UIを含むLuaエラーは0件だった。一方、三つの本体観測はいずれもpromotion_snapshot_status=3で、独立した検証器は識別不能として拒否した。この試験をCore本人の全額支払いの証明には含めない。

固定SHAのGameCoreで、正式APPOINTのRVA 0x2DD069は管理側の識別値をGovernor+0へ書き、0x2DD071–0x2DD07CはPlayer+0xD8の所有者をGovernor+0x18へ書く。人物Indexは+4であり、CanAssignの0x2DD8B9もこのIndexを読む。+0の識別値は0x2DCC29–0x2DCC35で加算とスロット番号から作る。公式UIのCacheGovernorは別の型で、GetOwnerの+0x28とGetTypeの+0x14をCoreへ転用しない。旧読取は+0を所有者と誤解した。修正では同じ+0x18の定義を診断と試験用の配置拒否に使い、識別値・所有者・Index・所属・昇進列を二度読む。ゲームの値や原RAXは変更しない。

全額保存はJAC-PR86-Payment1720-Cost2-Pai-02d96cd7bf28491a8b9719228d276c12-0000000000000001.Civ6Save、SHA-256 B7062F934A3852BF71236DD7EFE7DCE7C9CE477E54C9E29A31398592E5635A61で保全した。次はこの保存から本人の全額を再照合し、本体の昇進を観測する。Join・Evolve・APPOINT・PROMOTE・Finalizeを再送しない。保存は最初の配置要求前なので、未配置を確認してから既払の正式配置を一度だけ要求してUIとCoreの二つの呼出しを採る。修正後の実機、本体費用3、通常22人とAI、未払い配置、通常の保存と複数端末、元113手順、中和停止と回復は未達として保持する。

### 支払照合情報の公開に失敗した場合の閉鎖

任命資格と支払照合情報は、いずれも保存済みの許可をそのまま再使用せず、全所有者を閉じてから再構築する。任命資格側だけにあった例外処理を、Rust生成の支払照合情報にも適用する。死亡した大国も保存キーの衝突検査に含める。キーを読めなければ未使用とは扱わない。初期の閉鎖呼出し、保存キー検査、書込後の読戻しのいずれかが失敗したら、後続所有者の公開を止め、先に公開した所有者も閉じ直す。

投影全体の例外でも、支払照合と任命資格の両方を閉じ直す。次の通常の調整処理は、保存台帳から再構築を試みる。ここでは加入・任命・昇進・支払い要求を送らず、称号の使用済み数を変更しない。初期閉鎖は失敗した所有者の後も継続し、既存の全所有者に試みる。閉鎖不能な所有者があれば新しい公開は始めない。

旧生成物で、初期閉鎖の例外、死亡した大国の保存キーの読取例外、途中の公開における書込・読戻し例外をRustのLua 5.1検査で再現した。追加3件が失敗し、既存4件は成功した。最初の修正後は全7件が成功した。別モデルの指摘を受け、無効化側で閉鎖失敗を無視する問題と、閉鎖メソッド取得の例外が後続所有者を飛ばす問題も追加2件の失敗で再現した。閉鎖処理は、所有者と二つの名前空間ごとに独立して保護する構成へ統一した。最終の全9件が成功した。

書込・読戻し例外の検査は、例外発生時に閉鎖回数を保存し、その後に全所有者を閉じ直すことを確認する。通常の調整処理による再構築、未加入人物の拒否、使用済み称号の不変も確認する。書込失敗後の閉鎖呼出しだけを取り除く試験では、閉鎖の断言が失敗した。試験後の生成済みソースは元のバイト列へ復元した。

この修正は試験用接続の失敗処理である。通常runtimeの三入口と観測の寿命分離、ロード開始時の世代閉鎖、同期Gameplayの本人会計、AIの正式支払いは別の必須条件として保持する。試験controllerの時間制限を外すだけで通常利用の完成とは扱わない。

統合レビューでは、任命資格側が例外を出さずに公開を拒否しても、支払照合側の公開が続く経路を確認した。任命資格の同期は成功時だけtrue、拒否時はfalseを返す契約へ統一した。後続所有者で読戻しが不一致になった場合も、先行所有者を含む全所有者を閉じる。支払照合側は明示的な成功結果を受け取るまで公開を進めない。任命資格の処理もRustの生成元へ移し、両ブロックの一致を検査する。

RustのLua 5.1検査では、任命と支払照合の許可を別々の読取側キャッシュへ反映する。先行所有者の全額支払いと後続所有者の任命予約を用意してから、不正schema・名前空間衝突・後続所有者の読戻し不一致を注入する。旧版は不正schemaで許可を再公開して失敗した。修正版は三経路で全許可の取消しと、復旧後だけの再公開、正式な支払数の不変を確認した。関連31件とnative55件は成功した。独立Win64子2本の各1成功・54内部filteredは別集計である。これは合成検査で、実機の許可取消しや製品全体の受け入れ結果ではない。

## ToDo880・既存1466：AIの追加称号操作の有限候補（2026-10-06）

AI所有者が伊達稙宗の追加1称号、山内上杉憲政の追加2称号を正式昇進へ渡すための入口を、基点 `4cd8026e` から再調査した。**実機0、AI受入未達。** 新規Rust bin `ai-operation-audit` は固定イメージの読取と候補の合成検査だけを持つ。runtime・既存callee・支払照合・bootstrapには接続していない。型を整数からポインタに直したことを呼出し許可とは扱わない。

一次ファイルは上記のSteam `G/DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll`、SHA-256 `324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c`。ImageBaseは `0x180000000`。`llvm-readobj --sections` と既存 `civ6-pe` で `.text` のRVA `0x1000` / raw `0x400` を再照合した。以下のコード位置は **file offset = RVA - 0xC00**。新binはDLL全体のSHAが一致した後だけこの固定対応を使い、相対分岐と引数命令を照合する。PE本体の変更・ロード・プロセス接続はしない。DLL exportの最寄り名は関数名の証拠に使わない。

公式 `G/DLC/Expansion2/UI/Additions/GovernorPanel.lua:547-577` は人間の `GetLocalPlayer()` からIndexを公式操作へ送っている。SDKの `Sid Meier's Civilization VI SDK/Documentation` のmd/htmlにも今回検索した操作名の公開native ABIは見つからなかった。これは内部操作が存在しないという結論ではない。古い1466領域の `pr86-native-promote-abi-853` は読むだけに留めた。指定された `8055b866` の実際の差分はproperty記録容量超過の追記で、昇進呼出しの校正成功を示すcommitではなかった。既存の既取得基礎昇進の拒否校正も、AI追加支払いの成功とは数えない。

### 実命令で照合した正式経路

| RVA（代表file offset） | 静的に確認した引数・処理 | 残る境界 |
| --- | --- | --- |
| `0x401FA0`、`0x40263C`、`0x402649`（`0x401A49`） | 所有者側のコールバック集合 `+0x1B8` を構築。`0x4A7F70` でreceiverを得て、`LEA`で `0x4A7AD0` を登録する。`0x402672 -> 0x404770` が集合へコピーする | 外部から勝手に登録するABI・解除・寿命は未検証 |
| vtable RVA `0x9FB4B8`（file `0x9FA0B8`）、slot `+0x10 -> 0x404170`（file `0x403570`） | 間接呼出し口は `EDX=[RDX]`、`R8D=byte[R8]`、`RCX=[callable+8]` に変換し `[callable+0x10]` へ末尾分岐する。登録された宛先が `0x4A7AD0` | callerの生の第2・第3引数はポインタ。AI側で受ける32bit段階値・byteと混同しない |
| `0x4A7AD0`（`0x4A6ED0`） | `EDX==2` の段階だけ本体へ進む。`RCX` はAI Governors receiver、`[receiver+0x98]` がplayer。`0x4A7B1B -> 0x2DE920` が本人Governorsを取得する | 段階2の正式名称、自然なスレッド、同期Gameplay確定点との前後関係は未実測。R8byteを任意の許可flagと解釈しない |
| `0x4A7D54 -> 0x2DE960`、`0x4A7D67 -> 0x2DDBB0`（`0x4A7167`） | 本人管理リストからGovernor Indexで実体を再取得し、`RCX=Governors, RDX=Governor*, R8D=Promotion Index` を検査する | **CanPromoteの第2引数はGovernor Indexではない**。R9やスタックに入力を読む命令は、この本体全経路では確認されなかった |
| `0x2DDBB0..0x2DDD81`（`0x2DCFB0`） | null/-1、Spent対 `Governors+0x118`、Promotion定義範囲・存在、既取得、Governor定義との対応、前提昇進のいずれか、後続の条件検査を経てALを返す | `.pdata` 最初の範囲終端 `0x2DDC9F` は関数全体の終わりではない。後続条件の型名は未確定。ownerリスト検査をこのcallee単体が代替すると仮定しない |
| `0x4A7E9E/EB3`、`0x4A7ED3 -> 0x30CBE0`（`0x4A72D3`） | `0x9A64F0` でGovernor/Promotionの整数Indexを格納する。キーは `0xAF5E1625` / `0x6D4BD4EA`、操作は `0x7D3DA99D`。構築関数の `ECX=playerID, EDX=operation hash, R8=input parameter object*, R9=output operation object*`、戻りはAL | 戻りRAXを操作ポインタとするのは誤り。構造体は既存constructor/copy/destructorを伴い、Rustの生バイト構築は未許可 |
| `0x30D0E3 -> 0x30D2FB`、`0x30CFF4..0x30CF5D` | PROMOTEはGovernorキーの存在検査後、ヘッダとparameter objectをコピーする経路へ進む | 構築成功は本人の昇進可否や支払い成功ではない |
| `0x4A7EF0 -> 0x30C100`（`0x4A72F0`） | `ECX=playerID, RDX=operation*, R8B=0, R9=0`。playerの存在・内部状態と `0x05B5D0` 等を検査。PROMOTEの分岐にも拒否がある | **キュー送信・実行ではなく検査**。第三引数をtrueにすると検査を飛ばすため、この候補は変更しない。各player状態flagの正式名称は未確定 |
| `0x4A7F0F -> 0x038990`、`0x4A7F18 -> 0x038C80`、`0x038C87 -> 0x30D330`（`0x38087`） | 検査が真ならownerと操作をwrapperへコピーし、その場で共通dispatcherへ到達する。`0x038C80` は `[wrapper+0x18]` をECX、`wrapper+0x20` をRDXへ渡す。dispatcherはplayerID<64・player存在・操作側検査を再確認 | このAI経路は同期的な直接呼出しであり、UI要求を送っただけの経路ではない。ただし任意threadから呼べること、MP全端末の一致を証明しない |
| `0x30EBF8 -> 0x2DE960`、`0x30EC0F -> 0x2DDBB0`（`0x30E00F`）、`0x30EC24 -> 0x2DF340`（`0x30E024`） | checkerはRDX=実体、次のmutationは **EDX=Governor Index**。どちらもRCXは同一Governors、R8DはPromotion Index。checkerのALが真のときだけ正式反映する | 隣接する二つのABIを一つの関数型にしない。mutationを直接呼ばない |
| `0x2DF3A3..0x2DF3AA`、`0x2DF42E`、`0x2DEA90 -> 0x2DECC0 -> 0x2598F0` | 正式反映は本人の取得済みPromotion byteを更新し、効果・通知を伴う。SpentはGovernorリスト内の実Promotion数を合計する | Spent用の別カウンタや新規の支払flagを足してはならない |

AI chooserは `0x4A7C1F` と `0x4A7D01/0x4A7D98` で評価値を既存の最良値と比較する。初期最良値は0。効果なしTITLEが可否検査を通っても選ばれる保証はない。追加支払いは評価関数の戻り値を偽るのではなく、同じ自然なAI処理段階で、予約済み本人の次のTITLEを決定して正式操作へ接続する候補とする。

### Rust候補と既存受理への接続条件

新bin内の `ai_operation/protocol.rs` は外部へexportしない純粋な再生モデルであり、送信関数・関数ポインタ・本体setter・Lua VM・ゲームメモリ書込みを持たない。既存 `governor_payment::PromotionSnapshot::diagnostic_paid` を再利用して、本人のBaseと順序付きTITLEを照合する。22行を人物ID順に扱い、Governor/Promotion Indexの重複、本人prefix不一致、部分列の飛び越し、PID・作成時刻・読込世代の不一致を拒否する。入力値の型やこの検査の合格は、入力が同期Gameplay由来であることの証明ではない。

予約の式は既存 `native-admission::secured` と同じ保守的な条件、すなわち所有者の人物別paid合計=公式Spent、公式残高>=全EVOLVING人物の未払い合計とする。他人物分の予約を先に差し引き、伊達はTITLE_1だけ、山内上杉はTITLE_1確認後にTITLE_2だけを候補にする。任命前は候補を作らない。一つの再生単位は同一所有者・最大2提案に限定する。

候補発行時は未確定として保持する。呼出しの戻り値やUIのsend行で支払済みにせず、後続の同期観測で本人の次のPromotionだけが増え、他人物の全Promotion・所有者・定義が変わらず、公式Spentが1だけ増えたときに一段を確認する。同じ観測内では次段を提案しない。変化なしは再送せず待つ。不一致・巻戻り・曖昧な結果はその再生単位を閉じる。欠落した結果を成功扱いして予約を減らさない。

実装接続時は、同期台帳のOWNER/STATE/保存費用と、完全な当該ゲームのGameInfo catalog、`governor_payment::read_current` 相当の本人観測、公式Spentを同じ境界で取得する。今回の再生入力に正式なGameInfo出所やCoreポインタの寿命を証明する機能はない。`CoreGovernorMethods.HasGovernor` 不在、および一部Gameplayで `GetGovernor` / `HasPromotion` 不在の既知制約を、偽APIで補わない。通常のpaid producer/consumerへの接続は324の既存所有領域を使い、全額確認後だけ通常のFinalizeと配置受理へ進める。予約済み任命許可を支払済み配置許可に読み替えない。

読込開始では任命・配置の派生許可を閉じ、旧世代の候補を破棄する。新しいPID作成時刻/読込世代と同期境界で、保存台帳・本体Base/TITLE・Spentから残予約を再計算する。旧要求の実行終了を確認する前に再生モデルを作り直してはいけない。純粋な再生成検査は実save/load、旧要求の排出、MP同期の証明ではない。

### 次に必要な1回の有限校正（今回未実施）

一つの専有試行で、自然なAI callback段階2の入口 `0x404170 -> 0x4A7AD0`、正式操作構築・検査 `0x4A7ED3/0x4A7EF0`、実行 `0x4A7F18`、共通checker `0x30EC0F` と反映後を同一thread・owner・PID作成時刻・読込世代・同期イベント順で結ぶ。費用2の伊達と費用3の山内上杉について、加入と全額予約→本体任命の1→本人TITLEを一段ずつ払った2/3→全額後だけ配置、を記録する。山内上杉はpaid2の途中保存を一度読み、残予約1とTITLE_1再送0を照合する。各段でCore本人Base/TitlePrefix/Indexと公式Spent、既存受理側の閉鎖/公開を突き合わせる。上限は2人物・追加3操作・途中読込1回とし、自然なcallbackを観測できなければ結果を未達で止める。

校正の前提は、通常callbackへの安全な観測設置・解除と同期producerの接続が既存runtime側で検証されていること。今回見つけた入口へ未検証のpatchや外部thread呼出しを追加して実施するという意味ではない。pure property getter内で操作を実行せず、UIのlocal-humanガードを無効化しない。これでMP、通常startup/load、113通し、スパイ中和・回復まで受け入れたとは扱わない。

### 観測専用callback本体の候補と校正出力

追加の `ai_operation/capture.rs` は、既存mode 8の `PromotionObservation` と `governor_payment::read_current` を再利用するWindows向け観測callback本体である。`ReadProcessMemory(GetCurrentProcess())` で現在の自然呼出しのオブジェクトだけをコピーする。OSのPID・作成時刻・threadを毎回照合し、渡された読込世代も開始時と比較する。所有者はAI receiver `+0x98` →player `+0xD8` と、Governors `+0x70` →player、本人Governor `+0x18` の三経路で突き合わせる。ゲームへの書込み、原関数の追加呼出し、操作の送信は持たない。

記録する順序は、自然callback段階2→AI CanPromote入口→原Spentの入口と戻り→AI checkerの原AL→owner filter入口と原AL→AI Execute直前のowner/操作hash→正式CanPromote入口と原AL→正式反映の入口と原AL→新しい原Spentと本人Promotionである。実際には最初のSpentはCanPromote内部 `0x2DDBCB -> 0x2DEA90` の原戻り `0x2DDBD0` で採れるため、AI checkerのALより先に記録される。checkerはRDX=Governorポインタ、反映入口はEDX=Indexの差を維持する。R9は生値のまま残す。所有者の全Governorリストを各観測で二度読み、本人の次TITLEだけを追加した期待値と反映後の全体指紋も照合する。原Spentと全体の本体Promotion数が一致しなければ結果を拒否する。エラーが一度出た観測単位を、後続の良さそうな行で回復させない。

`ai-operation-audit <固定DLL>` のJSONへ、次の1回で必要なcall/return/RVA・許容件数・生レジスタ・未接続条件を加えた。`0x4A7D67`、`0x2DDBCB`、`0x30EC0F` は変位が4byte境界にある。一方、filter `0x4A7EF0`、Execute `0x4A7F18`、反映 `0x30EC24` の変位は揃っていない。既存mode 8の原子的な変位交換をそのまま転用してはならない。mode 8は入口だけなので、原AL/原Spentとこれらの追加地点は、別途検証された通常runtimeの観測経路が必要である。

**観測callback本体はコンパイル候補であり、gatewayの新規設置・通常runtimeへの登録は未実装。** 既存native callee/bodyは編集していない。将来のgatewayから自然呼出しの生値を渡す接続口はあるが、fixture値を渡しても実際の同期callback由来になったとは扱わない。校正receiptにも `payment_authority=false` を付ける。原反映後のSpentが自然に読まれない場合はその項目を欠損として止め、任意threadからgetterを呼んで補完しない。今回のWindows検査は自身の試験プロセス内のメモリ配置を実OS APIで読み、記録と拒否を検査するもので、Civ6の成功ではない。

並列一次調査では `0x05B5D0` がECXのownerを読まずグローバル `0xB617C0` 等を参照することも確認した。`0x30C100` はplayer `+0x288` とこのグローバル検査の組合せによって拒否リストへ進む。従って、PROMOTEが拒否リストにある事実だけで「AIは常に拒否」とは言えない。`0x30CBE0` 冒頭の動的登録表とslot `+0x28` の実体、出力operationのコピー寿命も校正項目に残す。`0x30C2A0` には入口RSP+0x28の第5引数があるため、4引数の代替入口として呼ばない。

共通callerまで追加照合した。`0x4014F0` は登録時と同じ所有者側 `+0x1B8` の集合を取り出し、`0x401513` は `+0x108` の段階値を読む。`0x40152C/30` がbyteと32bit段階値のアドレスをR8/RDXへ渡し、**`0x401534: call [rax+0x10]`（file `0x400934`）→ `0x404170` → `0x4A7AD0`** と進む。元の戻り先は `0x401537` のまま。段階値は `0x401544..0x40155E` で減らしながら反復する。観測候補の開始時もこの元戻り先を必須にした。これにより関数名や近傍文字列だけに頼らず、登録された所有者receiverを使う自然callbackの共通callerを特定できた。ただし、その実thread・同期Gameplay更新順・保存境界の実行時照合は未達である。
追加レビューのSpent出所指摘を修正した。Spentの入口と原戻り先を同じ観測単位で対にし、生RCX/RDX/R8/R9・原RAX・順番をreceiptへ保存する。前Spentを取得する前のAI checker完了を拒否し、対の欠落・世代違い・thread違いで観測単位を消費して終える。filterのR8は下位byteで照合し、上位bitをABI上の別条件にしない。再生中の新規称号獲得はこの有限校正では対象外として、従来どおり曖昧な結果で閉じる。正規22人の置換と引数キーの個別命令照合も追加した。

今回の検証は、capture追加前のworkspace全対象が139群・1980件成功（独立Win64子2群の各1成功・54内部filteredを加えると141群・1982件）、失敗・ignored0。Lua 5.1全件を含む。captureと費用欠損拒否を含む最終追加binは17件成功、失敗・ignored・filtered0。待機後の最初のコンパイルでは試験連番の型推論が失敗し、u32を明示してから17件を再実行した。対象Rust整形とgit diff --checkは成功。capture再レビューは原Spentの対保存とR8byteの修正後にMust/Suggestion/Nit各0。これらは有限候補の検証であり、ゲームの起動・操作・実受入は0である。親が扱うGameplayの進化完了・追加費用欠落修正には触れていない。
完成版の監査binをインストール済み固定DLLに実行し、全SHA・18個の分岐・9個の引数/共通caller命令を再照合して終了0。出力はexecution_enabled=false、runtime_installed=false、receipt_complete=falseを維持した。実機のreceiptを生成したわけではない。

## ToDo880・1466：自然AI callbackの4地点gateway登録候補

`cc7525ae` 基点の独立writerで、`ai_operation/gateway.rs` と検査、任意実行bin `ai-gateway-observer` を追加した。親 `runtime-integration` の `32af6210` および `0051068e` のCallFence・所有者識別・loader helperは読むだけで、既存runtime/composite/bootstrapを変更していない。通常runtime登録・実AI観測は0のまま。

登録候補は、現在のAI owner `+0x1B8` の集合（上限256、要素幅0x40、callableポインタ+0x38）から一意の本人callableを確認する。vtable `base+0x9FB4B8`、receiver+8、元関数slot+0x10=`base+0x4A7AD0`、owner+0x90とreceiver+0x98の同一playerを照合し、slot+0x10の整列64bit CASで観測gatewayへ切り替える。共通caller `0x401534` の原戻り先 `0x401537` とEDX段階2が一致した自然呼出しから、既存 `Capture::begin` へ生値を渡す。

同じ登録単位に、AI CanPromote `0x4A7D67`、正式CanPromote `0x30EC0F`、原Spent `0x2DDBCB` の3つのE8変位を加える。すべてoperandは4byte整列。signed rel32の上下限を検査して近傍relayを確保し、元E8を保持して変位だけCASする。CPU条件は既存mode8と同じAMD判定を再利用する。filter `0x4A7EF0`、Execute `0x4A7F18`、反映 `0x30EC24` の非整列変位は変更しない。操作hash `0x7D3DA99D` と二つのparameter keyは前節の一次命令が正本であり、不明なenum番号53へ置換しない。

4つのgatewayは原関数を一度だけ呼ぶ。真callerのRSP+8から0x60byteを `ReadProcessMemory(GetCurrentProcess())` で有限コピーし、observerのshadow領域とは別に保存した後、原call前にhome領域を含めて復元する。読み取れなければ元stackへ戻して原calleeへtail jumpし、観測は採用しない。RCX/RDX/R8/R9/RAX/R10/R11・XMM0..5・RFLAGSを観測前後に保存し、returnの元結果を復元する。gatewayは元calleeから見たreturn addressを自身のreturnラベルへ変えるため、ログでは元callerのreturn addressを別フィールドに保持する。無加工の元return addressをcalleeが読むという保証はしない。

SEHのpdata/xdataを持たせ、巻戻り時の終了handlerでそのthreadの呼出し情報・深さ・Captureを破棄する。親の原例外処理へ検索を続け、例外を成功にしない。再入呼出しは外側の所有者文脈を借りず、観測拒否にする。元targetは登録前に一度だけ確定し、停止後も書き換えない。GameCoreとgateway moduleをpinし、relayもプロセス終了まで保持する。途中失敗は逆順復元を試し、失敗した場合は `CriticalRestore` と静的保持したRegistrationを残す。`recover_failed_registration` で元所有者が再復元できる。

`RuntimeLease` は親内部で実lease/ARMとCallFenceへつなぐunsafeなRust契約であり、実装済の許可ではない。開始時のPID/作成時刻/owner/generation/session/ticket/arm tokenと、現在のowner transactionを原呼出し前後に検査する必要がある。今回読んだ親ソースに `PRODARM` という公開型は見つからず、数値propertyや常にtrueを返すアダプターは作っていない。世代変更後に古いcallableを再利用しない。親が安全な現caller registrationと同一leaseを保証するまで、CLIは状態JSONを表示するだけで、登録を実行しない。

この4地点だけではfilterの原AL・実Execute・反映後の原Spentを揃えた完全receiptにはならない。実測できていない点を架空のstatusや機械アドレス0で埋めず、将来の3操作・途中読込1回の校正では前節の追加地点と合わせる。property getter内からOP/VM/Lua getterを呼ばず、Spent・本体昇進flag・追加許可flagを生成しない。
親ソースの実enumはProductionAdmission=6、PromotionCall=8である。新候補は既存enumへ架空の53や9を足さず、独立binと任意のRust登録関数に留めた。最新SEH試験ではWindows SDK準拠のFFIを試験内だけに宣言し、Rust activationを飛び越さない明示Win64の例外発生元を使う。

最終source `6f7dba43` の対象bin検査は24件成功、失敗・ignored・上位filtered0、コンパイル警告0。内訳は既存のcapture/protocol/pinned17件＋gateway7件で、gatewayの7件に2つの独立Win64子検査を含む。各子は1件成功・23内部filteredであり、上位24件へ重ねて加算しない。原関数4gateway各1回、homeを含む0x60真caller領域とvolatile GP/XMM/flags、実RtlVirtualUnwindのRIP/RSP、実RtlUnwindExでgatewayを跨いだ深さ・invocation解除、rel32両端、非整列拒否、原子的slot復元、読取不能frame、所有者/世代/session/arm/ticket差を検査した。

初回の実unwind fixtureはアクセス違反で失敗した。診断付きRust発生元では成功したが、その結果を最終成功には使わず、Rust activationを飛び越さない明示Win64発生元へ変更し、診断なしの全24件で成功した。FFIのfeature不足によるコンパイル失敗も記録し、共有Cargo.tomlへ変更を広げず試験内SDK宣言で解消した。最終ログは `D:/CC/Work/todo880/ai-gateway-final-test.log`。再レビューはGPT-6-Sol/maxで新Must0・Nit0。unwind fixtureの状態設定を補強するSuggestionに対し、INVOCATIONへ値を設定して解除確認を追加した。CAPTUREは空の解除確認で、非空CaptureのSEH破棄を実証したとは主張しない。親指定のAstra等による最終統合レビューは親側で実施する。全workspaceの追加実行はしていない。

実所有leaseを使う通常registration、callable集合の実ゲーム寿命、ゲーム内並行owner/load競合、完全な支払いreceiptは未実測。unsafe RuntimeLeaseの架空true実装はなく、単体のfence値比較を本体authorityの成功とは扱わない。実機起動・Steam操作・共有DLL変更・通常登録・実AI観測はいずれも0。親で既存lease/CallFenceアダプターの実装と所有権確認を終えた後にのみ、前節の有限校正へ進める。

### 解除の直列化と未完了状態の保持（Jasonレビュー対応）

解除同士が競合して一方のVirtualProtectがRXへ戻した直後に他方がCASする問題と、bytesだけ元に戻った状態を復旧済みと誤判定する問題を修正する。登録・公開close・復旧は一つのINSTALL lockを使う。登録失敗中は既にlockを持つためclose_lockedを使い、再取得しない。

PatchStateは全地点の初期ページ保護を最初の変更前に保存する。共有RegistrationはArcで同じ状態を参照し、bytes/protection/cacheの未完了を独立に保持する。元bytesが戻っていても、保護・FlushInstructionCacheが未完了なら残りだけ再試行する。RWX化後の属性を元属性へ上書きせず、同じページにある複数地点でも初回のRX等へ戻す。すべての状態がRestoredになって初めて成功を返す。relayと原targetの寿命は従来どおりプロセス終了まで保持する。

検査は実際の実行可能RXページに対するMakeWritable/保護復元/flush失敗を注入し、retry後の属性と実行結果を検査する。barrierで一方をRWX化後・CAS前に止め、もう一方の解除が共通lockで待つことを確かめる。非空Captureを作る正常段階の子検査では、動的にowner/thread/generationを照合する試験専用leaseを使う。再入・別thread・誤owner・途中世代変更を拒否しても原関数は各1回で、合計7回・14行の順番を確認する。常にtrueを返すadapterは作らず、この合成leaseを本体authorityの証明には使わない。

Jason指摘修正後の対象binは29件成功（失敗・ignored・上位filtered0、警告0）。内訳は前回24件から旧slot単発検査1件を共有状態のRX復旧5件へ置き換え、正常Captureの動的owner/generation子検査1件を追加したもの。独立子は原ABI、非空Captureの実SEH、正常観測、並行解除の4本で、各1成功・28内部filteredを上位29件へ重ねて加算しない。実ロード済gatewayのRET直前の仮想unwindも成功した。ログは `D:/CC/Work/todo880/ai-patchstate-final.log`。

GPT-6-Sol/max再レビューはMust0。Suggestionの導入途中flush失敗と公開recover_failed_registration経由の再試行を検査へ追加し、Nitの名称と実処理の差も解消した。この最終差分は復旧5件を再実行して5成功・24filtered・失敗ignored0・警告0（`ai-patchstate-recovery-final.log`）。29+5を別の34ケースとして数えない。全workspaceの追加実行はしていない。親runtimeの実lease adapterは未接続、通常登録・実AI観測・ゲーム起動は0のまま。

Jason追加提案のphase不一致検査を確定した。試験用機械語callerをphase引数付きにし、同じowner/thread/ARM/世代のphase2→phase1→phase2を原gateway経由で呼ぶ。phase1は原関数へそのまま届き、原callは1回だけ増え、受理receiptは0、Capture/INVOCATION/DEPTHは解除済み。続くphase2は正常に再受理される。誤owner・再入・別thread・世代変更も含め、子試験の合計を9原call・18行・入口連番1..9で検査した。世代を巻き戻してphase検査を通していない。

変更はgateway_tests.rsと本書のみ。対象子検査を共有cargo_gateと専用targetで再コンパイルし、1成功・28filtered・失敗ignored0・警告0（D:/CC/Work/todo880/ai-phase-final.log）。直前の対象29成功、復旧API5成功・24filteredと合わせて検証範囲を保持し、重複加算しない。全workspaceの追加実行は0。source上のproduction ABI/RuntimeLease契約に変更はなく、本体の実adapter登録・AI支払い実観測・通常接続成功は依然0である。
### Root324: 通常Gameplay成功通知を使う有限会計

基点0051068eの費用2診断は、実機agent-todo-324-a39639587c09-01で39/39手順、6/6判定が成功した。原steps.jsonl、result.jsonとtodo324-events-cost2.jsonlを照合した。所有者0・伊達稙宗Index6ではBASE成功seq1/Spent1、任命成功seq2/Spent1、TITLE_1成功seq3/Spent2となり、保存再読込で取得集合・定義・本人支払数・Spent・確定後Spentが一致した。功績・全通知履歴・保存前の確定列番号の完全比較は未達である。判定対象のRuntime Error等は0件だが、ログには公式Mods検索のcontext作成失敗と翻訳重複がある。再読込後の正式操作再送は0である。この事実は同診断だけの証明で、通常22人の実機受入ではない。

本有限実装はtools/civ6-persona-events-rs/src/normal.rsを生成元とする。既存生成ブロック、GameplayのNativeGovernor・CorePaymentRead、Gameplay内の予約計算呼出しへ接続する。通常APIは私的なcost2のStart・Scope・Paidを呼ばず、明示したprivateフラグのときだけ従来の診断へ分岐する。成功済みの診断は生成38手順に初回再起動を加えた実機39手順である。新しい診断ではFinalize後の比較元の取得を追加して生成39手順とし、その手順数と保存比較の順序を生成検査に反映した。新しい診断の実機成功はまだ主張しない。

開始はスクリプト初期化の一回だけである。schema取得前にターン0、22生存大国、各所有者のSpent0、全人物の未加入、未使用のイベント名前空間、私的フラグなしを確認して揮発候補を保持する。加入元の設定は全22人の人物と文明へ一意に一致させる。その後、既存SyncAppointmentAdmissionが衝突検査を受理し、schema3の全所有者reset、全22人物のclose・型付きfalse・manifestを自然GetPropertyで読んだ成功境界でだけ同期Game propertyへ保存する。失敗や例外で候補を破棄し、後のReconcile、readyや設定変更で再作成しない。所有者・人物・文明・偉人・総督・必要昇進のIndex/Hash/type・費用を、長さ付きの正確な定義列へ束縛する。これは設定による保存値の受理ではない。

このcommitのnativeClosedAll22は、既存の自然Core getterへ全22閉鎖セルを公開し型付き応答を得たという契約である。Rust nativeの内部状態を新しいLua APIで読み返した証明ではない。基点の実機診断では後付けのmode6設置後に再公開し484組の閉鎖を別に確認している。通常loaderを最初から接続した新規ゲームで同じ境界と484組を実機確認することはRoot/880の別Mustである。owned JOIN/EVOLUTION definitionは既存SQLの全22人物、費用1/2/3、正式BASE/TITLE定義と既存同期Join/Evolveの書込を使う。この全22閉鎖と定義が揃わない保存・初期化からreceiptを開始しない。

JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1は所有者ごとの人物取得集合と、その人物の加入元、正式GovernorAppointed/GovernorPromotedの生引数、context、順序、通知時Spentを同期保存する。BASEと任命の前後逆・重複を許し、最初の成功証拠を変更しない。費用1ならBASE、2ならBASE/TITLE_1、3ならBASE/TITLE_1/TITLE_2の連続prefixだけを会計に使う。未知追加昇進は支払いにも功績にも加えない。join前・wrong owner・他人物・死亡大国・不正引数を拒否する。加入前の本人成功は未任命という前提と矛盾するため、その人物の新規Joinも閉じる。

通知時のSpentはnil/0もそのまま保存する。予約前の公式Spentを保存し、以後の正規支出連番が要求するSpentへ後のGameplay queryが達したことを確認する。通知数や総Spentから人物の支払数を作らない。各人物の集合から未払予約を計算し、複数の進化を互いに区別する。Spentが不明・遅延中、任命/BASE欠落、TITLEの穴は不明として全額予約を保持する。通常NativeGovernorとCorePaymentReadはこの集合を読み、既存CompleteEvolutionだけが全額証拠後に偉人を消す。Events callbackは取得記録の保存だけを行い、任命許可やpaid flagを作らず、支払い要求・Finalize・偉人除去を呼ばない。

途中保存を新Lua contextへ移した後も同じ集合だけを読む。Appointed前の欠けた通知をUI full-paid、総Spent、後readyから補完しない。receipt導入前のold paid/old partialはunknown closedを維持する。既存Core本人観測を使う正式bootstrapは別Mustであり、本変更でUI受領を転記しない。通常AI自動支払い、default loader、複数端末、Runtime2の実機受入はこの有限実装の達成条件に含めず、未達のまま保持する。workerは実機を予約・起動せず、Rootが後で全件検査・別モデルレビュー・実機受入を行う。

検査は出荷Gameplay全体をmluaのLua5.1で読み込む。全22所有者と22人物の非自己加入462組、疎Index、費用1/2/3、通知前後逆・重複、複数予約、保存途中・再送、callback nil/0とSpent遅延、TITLEprefix穴、schema衝突・閉鎖読取失敗・死亡大国、Index/Hashと証拠束縛の変化、UI全額でも通知不足なら偉人保持を扱う。元callbackを空にするmutationでは成功側が失敗し、通常Finalizeでも偉人が残る。これは合成検査であり新しい実機成功数には加算しない。

取得集合の通常読取は、進化中と進化済みの保存EVOLUTION_SURCHARGEが人物定義の費用-1へ一致することも要求する。欠落・過小・別費用なら未知として偉人を保持する。880担当が変更するCompleteEvolution本体のpredicateは編集せず、会計モジュールの入力束縛だけでこの条件を守る。通常APIで私的Paid/Native/Scope/Startを例外に置換しても費用2が完了する検査を追加し、public modeの独立性を直接確認する。

実機01のsnapshot(2)はFinalize前のacquired・definition・paid・spent・afterSpentを比較する範囲である。merit、全receipt履歴、Finalize後の最終sequenceと台帳・UnitNilを含む完全保存比較の証明へ拡大しない。Rootの別担当が成功済み01の物理保存を読取り、新しい現在値baselineを採り別名保存・再読込する有限診断を作る。workerはsrc/scenario.rsとCLI出力を編集しない。通常モジュールの新Lua contextテストでは同期取得store全体を深い比較で照合するが、この合成検査を実機01の不足範囲の証明には使わない。

最終の対象検査はLua5.1 Gameplay 31件と診断生成2件が成功した。前者には全22人物のSQL定義と費用式を照合した462組、通常APIから私的cost2 APIへ接続しない検査、callback removalで偉人を保持するmutationを含む。専用targetはD:/CC/BuildCache/todo324-normal-events。コマンドはcargo test --locked --manifest-path tools/Cargo.toml -p civ6-persona-events-rs（Gameplay 31件成功）と、手順数の期待値を生成38へ訂正した後の同-p --test scenario（2件成功）である。Rust CLIのembedを再実行して出荷LuaのSHA-256が変わらないことも確認した。Native crate・paid/appointment生成ブロック・UI/shared・CompleteEvolution・src/scenario.rs・src/main.rsは基点から変更していない。全体検査、別モデルレビューと新しい実機受入はRootで行う。

### Root324: 支出の確認基準と購読の連続性

Astraの二つのMustを受け、所有者の支出会計を予約前の基準と正規の支出連番へ組み直した。通常のPrepareEvolutionは、残額検査より前にRust生成のBeforeEvolutionを呼ぶ。人物・加入元・費用と保存された状態を照合した上で、その時点のSpent、所有者の期待Spent、支出連番と通知列番号を同期保存する。保存の費用と追加費用は引き続き人物定義との完全一致が必要である。CompleteEvolution本体と880担当のpredicateは変更しない。

BASEとTITLEの最初の成功通知だけが所有者の期待Spentを1進める。任命とBASEの前後逆、同じ通知の重複では追加計上しない。人物の取得集合は支出連番・期待Spentと同じ正式Index/Hashの支出記録へ束縛する。費用2の予約前に無関係な支出が5あれば、本人BASEとTITLE_1の成功通知はSpent6と7を要求する。通知時Spent5はそのまま保存し、後queryがSpent7になるまでは本人が全取得していても完了を拒否する。複数人物の通知順が混ざっても、所有者の連番を共有して全ての既知の未反映支出を確認する。

人物以外の正式消費も、実定義のGovernorPromotionSetsを使って区別し、支払い確認の連番へ加える。普通の総督のGovernorAppointedは一意なBaseAbility定義へ解決し、GovernorPromoted(BASE)と同じ支出記録にする。正式な追加昇進の支出も確認基準へ加えるが、人物のBASE/TITLE取得や功績には転用しない。基本昇進を解決できない普通の任命は、その所有者の会計を不明として閉じる。通知が欠けた本人の取得集合を総Spentや普通の総督の消費から作らない。

後の予約前に、記録した正規支出を超えるSpentが見つかった場合は、基準を更新した世代を保存する。新しい予約にはその時点の基準を使えるが、以前の予約の欠けた通知や未反映支払いを、この新しい基準で再び有効にしない。既に実Spentとの一致を確認した支出は、その時点の所有者・定義・連番・世代・Spentを保存して保持する。基準変更では未確定の支出だけを無効にし、確定済み人物の支払数を失わせない。人物の取得集合が空のときの未任命という判断も、同じ基準の世代と連続購読、所有者の期待Spentとの一致、完了済み人物の取得証拠が揃う場合だけに限定する。これによりAの完了支出1があっても未任命Bの偉人消失を二回確認してBだけの予約を解放できる。Aの旧支払証拠が欠ける場合はBの不明状態と全額予約を保持する。

支出の記録と人物への取得認定は分ける。正式な成功通知が未加入の人物を指していても、その国の消費には数え、加入済みの別人の未払いを埋めるSpentとして使わせない。人物の加入資格は引き続き拒否する。消費対象は公式GovernorPromotionSetsと一意な基本昇進に限定し、名前の接頭辞で所属を補わない。確定処理は通常のGameplay照合で期待Spentと現在値が一致したときだけ行い、Statusは保存を書き換えずに読む。

現在のLua contextで両方のEvents.Addが成功したことをstate/read/admissionの条件にする。復元した台帳で一方でも失敗したら、通常の公開より前に同期保存へcontinuityTaintedと失敗した購読・時点・通知列番号を刻む。次のLua contextで購読が成功してもこの欠測を解除しない。保存に全額支払いの人物がいても、その資格と新しい人物の加入を閉じる。UIの現在値や設定による復旧入口は作らない。通常台帳はversion3/source v3で、支出確定の証拠を持たない旧version1/2はunknown closedのままにし、正式Core bootstrapを別Mustとして保持する。私的な費用2の台帳・フラグ・診断は従来どおりである。

前候補のGameplay Lua5.1の40件は成功したが、Rootのnormal-events-integrated.logでは診断生成2件中1件が失敗した。Finalize後の比較元追加に合わせた39手順へ期待値を修正した。現在の候補はnormal-v3-fixed-tests.logでGameplay44件・診断生成2件・paid glue10件が全て成功し、失敗・ignored・filteredは0だった。過去支出5/新費用2/Spent5遅延から7への確認、複数人物と普通の総督の前後逆・重複、本人通知欠落、購読失敗から別contextへの復旧、全額保存と新人物加入、Aの支出1とB消失、後の不明支出による旧欠測の再利用拒否、支出基準と連番の改変を扱う。支出確認と購読条件をそれぞれ外すmutationでは、正常側の保持・閉鎖の断言が失敗した。全体検査と次の別モデルレビューはRootで行う。この合成結果は、通常22人・AI・MPの実機成功数には加算しない。

### Root324: 既払セーブからの通知履歴・功績の追加比較

0051068eの実機agent-todo-324-a39639587c09-04は11/11手順・6/6判定が成功した。元の成功セーブ（SHA-256 71E9A0DB5F54BF33699CA6D57F5DE6FFBA06511F62BF3B9DF99E2D5193C2C925）を読み、全receipt、生引数を含む取得集合、定義、本人paid2、Spent2、afterSpent2、確定sequence3、数値の功績0とその存在・型を比較元として取得した。新しいcheckpointを保存して読み直し、同じ全文を二回取得して完全一致を確認した。Join/Evolve/APPOINT/PROMOTE/Finalizeの送信は0で、元セーブのハッシュも不変だった。判定対象のLuaエラーは0、CHAIN DONE/exit0と所有ゲームの終了を確認した。

これは今回の比較元から新checkpointを経た追加検査の結果であり、元の39手順で採取しなかった項目へ遡って成功を補わない。通常22人、AI、複数端末、常駐runtimeの未払い拒否の証拠にも転用しない。生記録と照合値はD:/CC/Work/todo324/live-full-save-04およびlive-full-save-proof.jsonに保全した。

### Root324: 通常イベントの準備順と既存試験の契約

旧試験の3人・ターン1開始とUI読取による会計は、通常イベント会計の初期化条件を満たさなかった。Rustのpersona_engine試験補助で、全22人、ターン0の初期化、一意な総督・昇進の定義と公式の所属表、加入前の全閉鎖を先に用意する。元の3人のIndex/Hashは保持する。費用3を試す設定は定義を固定する前へ移し、初期化後の定義変更を製品が受理するようにはしない。

試験の正式APPOINT/PROMOTEは既存のエンジン処理で支払いを行い、その実際の成功からGameplay通知を送る。直接生成する既払総督の試験用準備も、対応する支出を伴う。候補オブジェクトだけの生成は別のNATIVE_CANDIDATEを使い、通知と支出がないまま偉人を消さないことを引き続き検査する。UIの総督getterを会計の確認元にしていた試験は、正規に作ったGameplay確認データの欠落・同じデータの復元・他人の予約・追加支払いへ更新する。UI側の種類別getterがない場合の操作送信拒否は保持する。旧関数をskip/ignoreにしたり、失敗した呼出しを成功へ置き換えたりしない。

別モデルが指摘した、通常総督の成功通知の後にSpentの読取が遅れている間の予約過剰は、独立した追加試験で再現した。BeforeEvolutionは現在のSpentが所有者の期待Spentより小さい間、新しい人物の予約を拒否する。Spentが追いついて残高が0ならNO_TITLEとなり、いずれも偉人を保持する。

修正版のnormal-engine-migration-03.logは任命10件、境界25件、人物80件、通常任命21件の136件が成功した。normal-v3-related-all-02.logでは関連2パッケージの全対象を実行し、33対象・906件、失敗・ignored・filtered 0、exit0を確認した。Lua 5.1を省略していない。予約過剰の追加試験を含むGameplay45件とscenario2件も同じ検査に含む。これは当候補の関連検査であり、親の固定67fの全2184件へ足したり、当候補の全workspace成功として扱ったりしない。通知に対して実Spentが先行する場合の本体順序保証は、別モデルの実機確認項目として残る。

再レビューのSuggestionには、昇進定義を残したまま公式所属だけを削除する場合と、所属先を別の実在総督へ変える場合の負例を追加した。どちらも通常の初期化と加入が閉じ、偉人を生成しない。別人のSpentがその通知より先に見える順序も追加し、本人のTITLE通知がない間は完了できず、本人の正式通知と追加支出が揃って初めて完了することを確認した。normal-membership-late-callback.logのGameplay47件は全成功・省略0である。前の全関連906件から製品コードは変わらず、追加はこの2試験だけである。本人の通知が全てある場合に本体がどの順で集計値を公開するかの実機保証は、引き続き未確認として区別する。

### ToDo1665: 実機04の予約保存からの返還再開

実機04（8ca69057）は本体の予約と物理保存まで成功し、画面で交渉を開始した後、10手順目の待機で停止した。元の22記録行には10件のSTARTと9件の成功があり、EqualizeOnce・AcceptOnceのSTARTはない。途中のingame_unblockは本体のOnLocalPlayerTurnEndとOnCloseを呼んでいる。前者は画面の状態に応じてセッション終了や拒否を行うため、「本体への操作が一切なかった」とは扱わない。原Lua.logと失敗も保全する。返還・中和の成功はまだ確認していない。

再開生成器 `tools/civ6-autotest-rs/examples/resume-group-exchange-62.rs` は、この原記録だけを受け付ける。原シナリオ・steps・Lua.log・resultのSHA-256を固定し、全STARTのコードと文脈を原固定契約へ照合する。途中の行の欠損、追加、送信コードの入替えを拒否する。保存は `b4daf733336c42d682acf021ce8b2dd0-0000000000000001`、SHA-256 `58A796AD65643DE332AC2DF703FCFF0D16B4ADC2ED09040F6F31CC2D5EE40253`、2,149,795バイト。元のPrepare・本体予約・予約保存を再送せず、保存されたnonceと履歴を新しいUIの変数へ復元する。古いSessionIDやDealIDは復元しない。

生成したloadのsave_sha256は実際の読込直前にも検査する。Windowsでは同ファイルの読取専用共有ハンドルを読込完了まで保持し、書換えと置換を拒否する。resume_onceは同じ保存領域にSHAごとのcreate_new記録を先に同期保存し、二度目の読込を拒否する。途中停止した場合もこの記録を消さず、新たな実行記録と最新保存を調査する。原04の未送信の根拠を、次の実行後の再送許可には使わない。

返還が不明な場合も観測と保存を残し、RETURNEDの確認で止める。別の技能検査へ進んだことを返還成功の代わりにしない。元の通常22人、本人の正式支払い、未払い配置、マルチプレイ、113手順、中和による効果停止と保存回復の受入条件は変えていない。

この候補の全ワークスペース検査は194結果行、親2,534件成功、Lua 5.1全件実行、失敗・上位除外0、終了0。Windows子8本は各1件成功・各67件の内部絞込みを別計数する。補助入口1件のignoredは親から実際に実行する。その後のT9入力固定と保存ハンドルの寿命検査は、関連35件で成功した。保護ハンドルをLoadGameの戻り直後に解放する変異は、後続のInGame確認で書込禁止のassertが失敗し、退行を検出した。製品実装は原バイトへ復元して関連検査を再実行した。

T9技能は生成器の `--skill-turn9 元41手順 実保存 出力` で用意する。原手順のSHA-256 `92E546605630B0EF50F0537F4F66616D1F7001B3F71EFC5DD97339AB5AFDE296` と、実保存の `95B8BE0EEE90EAE2C58CEDF7836C6089B83E9CA7059EE8758227003470B59090` を照合し、最初の読込だけを同じ保存保護で固定する。購入後のcheckpoint読込を含む元41操作は保持する。これは実機の成功を示すものではない。

### ToDo1665: 固定統合版での返還と技能の実測

固定f8396615の実機 `todo880-reserved-05-01` は24/26手順で停止した。原予約の復元、MAKE_DEAL声明の確認、EQUALIZE一回は成功したが、相手の応答はDealAction 10／RespondingToDealAction 6で、診断はAI_REFUSEDとなった。受諾は送らず、本体にQUOTED_UNKNOWNを保存した。最後のRETURNED必須判定は期待どおり失敗し、未返還を成功にしなかった。最新保存は `02e9748e7497429e82af5ca10b890da7-0000000000000005`、2,151,180バイト、SHA-256 `2E7744102442B852C5851D79F0C86C73DBC52FD4865B646C20FE90CB0C9FDBE6`。旧58A保存と未送信の証明は、この先の再送許可には使わない。

独立した技能の実機 `todo880-reserved-05-02` は42/42手順・6/6判定、非test_modeで成功した。実ターン9→10→11で功績4→6→8となり、ENVOYS_1を費用8で一回購入して0になった。本人owner0/source1/index6/hash912436062、正式支払い2・使用称号2、都市131073を保持した。重複購入と別系統購入の拒否で全状態が不変だった。Coreの実都市効果も確認し、再起動して実保存を読み直した。

`verify-reserved-skill` は固定された41操作と準備起動の全STARTを照合する。購入後・保存前・再読込後の全166,725バイトが同一で、SHA-256 `6BB37F91139299F9591E9C531301A5162E789164CDBB39BCECB2120FE0C24BCC` も必須にした。実保存 `46037a45ff174987b8a3d9fc46a7ee92-0000000000000001` は1,740,554バイト、SHA-256 `CB6F4004136F0E4B41B9A47D90E8F1472E9C17D8AD5D2F28B413B6CDF06DA33D`。親の6つの受入条件全体や、中和・返還の成功には広げない。

次の `read-group-refusal-62` は最新の拒否保存から5手順で読む。保存時の応答と、現在の取引項目・金額・容量を分けて記録する。APIごとの欠測と途中結果を残し、待機中も外交画面を保護する。項目を観測する前のValidate呼出しは行わない。本体のEFFECT_GRANT_SPYを含む容量付与制度の取得済み状態・進行可否・費用を確認する。文明別CitizenNames件数は仮説検証用であり、旧捕虜への名前やIDの割当てには使わない。

関連検査は読み取り55件、技能照合18件が成功し、最終別モデルは全指摘0。原技能rawの独立照合も終了0。新しい読み取りの実機は未実施。原ログ・全実保存・ハッシュ・proofは共有ToDo1665の `20261007-current` に保全した。ゲームと各チェーンを回収し、次の枠は親880と調整する。

### ToDo1665: 通常の市民名候補の不足を修正する

拒否後の読み取りは6/6手順・6/6判定で成功した。北条と伊達のCivilizationCitizenNamesはUI/Coreとも0件、3捕虜は名前空欄・NameIndex=-1のままだった。応答10がEQUALIZE_FAILEDであることも両文脈で確認した。所持金61.6953125と220.19140625はUI/Coreで一致し、現在の取引は空、保留なし。拒否時に記録されなかった対価は復元せず、名前不足との因果も未確定として扱う。

Rustの文明生成器から、全22文明へ本体の日本用40候補を性別・時代フラグごと登録する。元からある名の行は置換せず、日本の元行も変えない。Oda/Takedaの定義より後に読む既存の生成SQLに追加する。旧捕虜や保存されたユニットへ名前・NameIndex・IDを設定する処理はない。SQLの生成、全22×40件の一致、再適用時のrowid維持、既存名の保持を確認した。

関連helpers9件とpersona14件が成功し、名前データ候補の指定全ワークスペースは196結果行・親2,549件・全Lua5.1・失敗/上位除外0・終了0。補助入口1件は親内で実行し、Windows子8本の各1成功/各67内部除外は別計数する。その後の生産読取修正は関連59件で成功した。再レビューは全指摘0である。

次の読み取りは、名前登録後の件数に加え、公式ProductionPanelと同じHash/Formation/Indexで各都市のスパイ生産可否・理由・費用・進捗と諜報局の所有・建設可否を見る。各APIを別々に捕捉し、進捗欠測で可否を消したり、生産可否欠測で建物所有状態を消したりしない。容量表示だけで生産不能と決めず、実際の判定から次の合法な操作を選ぶ。新規スパイへの名前割当、旧捕虜の扱い、次の中和は未実機である。

### ToDo1665: 名前番号の本体実装と既存保存の互換性

対象はインストール済みの `GameCore_XP2_FinalRelease.dll`（12,940,384バイト、SHA-256 `324C51E9EA3531758842E16C69E6CDDBEFBB226C5B675C3D6E60111646C2E98C`）である。以下は既存PE読取器とLLVMによる静的解析で、実行時の通過を証明したものではない。アドレスはimage base 0x180000000からのRVAを示す。逆アセンブラが表示する近隣export名を関数名として採用しない。

- 捕虜のLua読取（0x6af610以降）は記録の+8を名前番号として読み、0x6af6ddから0x3094a0へ渡す。同じ+8をLuaのNameIndexへ渡す箇所は0x6af75c/0x6af761にある。
- 0x3094a0の解決処理は所有文明の名前collectionを引き、負数または範囲外なら空文字へのポインターを返す。有効な番号はcollection内の添字であり、SQLite全体のrowidではない。
- 0x309640の候補選択はcollectionが空なら-1へ進む。候補数を保存された使用回数配列の長さでも制限し、候補のflagと使用回数を使って選ぶ。0x309991に本体の `Choosing a Citizen Name` 文字列参照があり、選択後は対応する使用回数を増やす。0x1f5cafの呼出し元は記録の+4を比較し、条件を満たす場合だけ戻った番号を+8へ格納する（0x1f5cb4/0x1f5cbf）。この静的経路だけで、新規スパイが実際に通過したとは主張しない。
- `PlayerNameManager` の復元末尾0x30940bは0x309c50を呼ぶ。同処理は対象の文明段階を確認し、現在の名前collectionが保存された使用回数配列より大きい場合に配列を拡張し、新しい末尾だけを0で初期化する。既存値を全面リセットする処理は追加しない。
- 段階判定0x2f9860の比較値0x253718b0は `CIVILIZATION_LEVEL_FULL_CIV` の未finalize IEEE CRC32と一致する。このhash方式は実測済みの伊達稙宗の総督hash912436062でも照合した。これは静的な対応付けであり、今回の保存で復元経路を実測した証拠とは区別する。

公式 `Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql` の `CitizenNameCollection` は、文明ごとの行を `ORDER BY T1.rowid ASC` で列挙する。既存の行を置換せず候補を末尾へ追加すれば、同じDB内の既存collectionの添字を維持できる。限定検査ではアイヌと琉球についても名前順と異なる順序で入れた既存3候補のrowid・各添字・フラグがすべて不変であることを確認した。DB再生成全体の順序や既存保存上の挙動まで、この合成検査だけで保証しない。

SSOT `03-map-and-scale.md` の独立したアイヌ・琉球の文明、各指導者、FULL_CIV区分は保持する。公式日本の名表は本体が名前を選ぶための互換候補として利用しており、アイヌ・琉球の史実の人名資料という意味ではない。SSOTは両勢力用の市民名表を定めていない。旧3捕虜の-1を推測番号に置換する処理も、既存ユニットを命名し直す処理もない。

追加helpers検査9件は成功、失敗・ignored・filteredは0。静的資料と原検査ログはToDo1665の `20261007-current` にSHA付きで保全する。次の実機では現在の2E774保存から登録件数・旧捕虜の生値・公式の生産可否を読み、新規正規生産時の名前割当はその後の実測として確認する。EQUALIZE_FAILEDとの因果、中和成功、回復の受入は依然未達である。

### ToDo1665: 名前登録後のT62保存の実機読取

親固定5a792461の `todo880-citizen-12-01` は6/6手順・6/6判定で成功し、終了0とゲーム回収を確認した。所有lockと実行中のlive HEADを記録した。最新2E774保存のみを読み、取引・任務・ターン送り・新保存は送信していない。実保存のSHAは読取前後で一致した。

北条・伊達の市民名候補はUI/Coreとも40件になった。旧3捕虜は空名・NameIndex=-1、取引候補はForType=-1を保持する。UIの保存済みquoteとspyRosterは前のread07と全文一致し、容量3と所持金61.6953125/220.19140625も不変だった。CoreのrosterはAPI欠測のERRORであり、両文脈で捕虜一覧が一致したという主張はしない。

UIでは全3都市のスパイ生産可否がfalse、費用450、進捗0で、スパイの拒否理由は返らなかった。江戸（65536）の諜報局はUI/Coreとも建設済み。他2都市の諜報局建設は政府広場不足で拒否された。CoreのCanProduceはNot Implemented、費用・進捗も欠測として個別に保存した。諜報局の再建設や既存制度の再取得を追加枠の根拠にしない。

独立Rust照合で全6件のSTARTを固定シナリオと全文比較した。旧実記録を使った正常読取・追加送信・Lua改変・欠落の4検査も成功した。この結果は名前データのロード成功を示す。新規スパイへの名前番号割当、返還、均衡化失敗の原因解消、中和と回復は未達のままである。生記録とファイルSHA、UI/Coreの部分欠測を含むproofは共有ToDo1665の `20261007-current/citizen-live-read-08` と `citizen-read-proof.json` に保全した。

### ToDo324: 観測停止後の全書き込みを閉じる

実機175の原観測は、停止後の二回の読取で対象外呼び出し数が一つ増え、全バイト比較に失敗した。コードを照合すると、任命・プロパティ・配置の処理中件数も観測の受付と独立して更新されていた。閉鎖フラグと受付済み件数を一つの原子状態へまとめ、受付に成功した処理だけが観測の本体とヘッダーへ書く。停止は新規受付を閉じ、受け付けた最後の書き込みが完了するまで待つ。件数の減算を含め、観測への書き込みを終えてから受付を返す。

人物の資格・支払い判定は観測に依存しない。任命は呼出しごとのスタック上で結果を判定し、観測が停止していても自然なプロパティ読取と本人の昇進照合を続ける。停止前に始まった任命が停止後に戻る場合も、停止済みの領域を更新しない。既存の期限付き観測モードは保持する。

独立したWin64子プロセスで、停止を跨ぐ任命の終了、停止後の配置の開始中、対象外キーの読取、本人の資格変更、費用3の未払い拒否と支払済み配置を検査した。32回繰り返し、917584バイトの全観測領域が不変だった。保留中の受付がある場合は停止がCriticalを返し、処理完了後にだけ成功する既存検査も保持する。この検査は実機の成功ではない。

実機175で同時に確認されたプロパティ記録4096件の満杯と115355件の欠落は別の未解決事項である。この停止修正だけでは容量不足は解消しない。従来の4096件の形式と、欠落0・原バイトの完全一致という検査条件を保持する。通常起動、同一プロセスの読込、AI本人の費用2/3、途中保存を含むマルチプレイ、中和後の功績と効果停止・回復は引き続き実機での受入を要する。

起動用の記録も同じ原子受付へ統一する。閉鎖フラグと受付件数を別々に更新する経路を残さず、二つの実装は共通のObservationGateを使用する。競合検査は実スレッドをチャネルで止め、閉鎖後の新規受付拒否、先行書き込みの未完了、完了後の可視性と排出を個別に断言する。起動処理と人物処理の本体判定を観測受付で止めない。

### ToDo324: 常駐runtimeのプロパティ生行を別領域へ全件記録する

実機175の原観測（`city-startup-native.native-first.bin`、SHA-256 `90D17D33…CADB4E`）は、受け付けたプロパティ呼出し119,451件のうち先頭4,096件だけを残し、115,355件を欠落として数えた。残った4,096件も欠測2,600件・読取済1,496件、受信先22件で、catalog配信の途中で満杯になっている。1回の全員への配信はcatalogのbit部分だけで22人×1,012bit＝22,264件あり、reset・commit・資格の読取が別に加わる。4,096行ではこのbit部分だけでも入らない。

常駐runtimeだけ、設置時にプロパティ生行の専用ログを別領域に確保する。容量は2^20行（48MiB＋ヘッダー80byte）で、実機175の受付件数の約8.8倍にあたる有限の診断予算である。この比率は配信回数や観測時間の保証ではない。無限には保持しない。VirtualAllocで確保した全ページを0で書いてから公開し、確保に失敗した場合はフックを置く前に設置失敗（5）を返す。確保した領域はプロセス終了まで解放しない。行は既存の48byte `PropertyRow` のまま全項目を残し、圧縮・間引き・消去・上書きはしない。満杯後は行を書かずに欠落数を加算し、検証を失敗させる。ヘッダーはmagic `JACL`・version 1・容量・行幅48・総byte長・PID・作成FILETIME・next・dropped・in_flight・filteredを持つ。

フィルター、24byteの一回読取、任命と支払の判定は従来の経路と共通の関数を使い、書き込み先だけが異なる。ログへの書き込み（件数を含む）は観測の受付と終了の間だけで行う。判定は行の空きや観測の停止と無関係に続く。フック内で確保・ロック・Luaの呼出しはしない。

CompositeTraceの917,584byteとprefixの4,096行の配置は変えない。runtimeでは、prefixのPropertyTraceのversionを4、reservedを`JACL`にし、行と件数は0のまま残す。version 3だけを受け付ける旧検査はこのprefixを拒否し、新しい検査はログが無いと拒否するので、空のprefixだけで合格することはない。共通の定数と検証は`civ6-bootstrap-rs`の`protocol`に置き、native側は構造体の大きさとoffsetを静的に照合する。runtimeの設定versionは`0x0324_0002`、bootstrapのPrepare versionは`0x0880_0003`へ上げた（`todo880-bootstrap-primary.md`の旧値を置き換える）。

bootstrap controllerは停止後、Compositeとログを各2回すべて読み、比較や検証の前に`native-first/second.bin`と`property-log-first/second.bin`をcreate_newで保存する。成功には、2回の一致、PIDと作成FILETIME、容量・行幅・総byte長、公開済み行のsequence、未使用の行がすべて0、droppedとin_flightが0を要求する。JSONLへ`bootstrap_property_log`行を出す。実行器の終了検査は`runtime.bin`と2本のログを読み直して同じ検査を行う。直接のruntime controllerも同じ4本を保存し、prefixの印とログを同じ基準で検査して`property_log`行を出す。任命と配置の記録枠の検査はこのcontrollerへ新たに加えていない。既存の成功条件は弱めていない。bootstrap DLLの途中件数は、ログのnextとin_flightを読む。印の無いprefixの0は件数として扱わず、不明（u64::MAX）にする。

独立したWin64子プロセスで、実際のgetter gatewayを通して次を確認した。

1. 22人分のcatalog全公開と、欠測・読取不可・tag 2・flags付きの読取を記録し、4,096件を超える全行のkey・受信先・戻り値・thread・読取状態・tag・payload・flagsが呼出し順に一致した。元のgetterの呼出し回数も一致し、対象外キーは行にせずfilteredだけに数えた。
2. 停止後のreset、対象外キー、任命と配置の判定は続き、Compositeとログの全byteは変わらなかった。実際のbyteを新しい検査に通し、FILETIME違いとログ無しは拒否した。停止をまたぐ既存の32回検査も、ログの全byteが不変であることを確認する。
3. 2^20＋1回の実呼出しで、next=2^20・dropped=1となり、検査は失敗した。容量が尽きても未払いの拒否と支払済みの配置は続いた。
4. 改変検査として、欠落の加算を消す変更とprefixの印を消す変更を入れると、それぞれ検査が失敗することを確認した。

これは合成検査の結果で、実機ではまだ確認していない。既知の上限として、観測中の対象呼び出しが2^20件を超えると溢れ、検査は失敗する。再配信の頻度や300秒までの観測で何件に達するかは、実機で確認する。そのときは容量を勘で増やさず、設計を見直す。1回の成果物はログ2本で約96MiBになる。作業集合から外れたページへの再アクセスで起きるソフトページフォルトは防がない（VirtualLockは使わない）。

観測は未公開・受付中・閉鎖済みの三段階として扱う。未公開に入ったcallbackはINSTALLEDをAcquire読取した結果を保持し、後から初期化が終わっても期限判定や受付・閉鎖をしない。sample_beginと停止APIの両方が公開を確認してからGateへ触る。初期化は期限とGateを用意した後、最後にINSTALLEDをRelease公開する。独立Win64子で、未公開の読取直後にcallbackを停止し、別スレッドで初期化してから再開した。古いcallbackの受付拒否と、その後の受付成功、正式停止後の拒否を確認した。公開確認を外す改変では、同じ検査が失敗した。

### ToDo324: 観測停止修正後の都市喪失診断が完走

修正版026a0f830c344eb467b5daedd7757263e761b05eで、明示した冷間bootstrapによる都市喪失診断 `todo324-observation-stop-01` が58/58手順・6/6判定に成功した。Luaエラーは0、チェーンの終了コードは0だった。通常製品の自動起動を確認した結果ではない。先の「実機ではまだ確認していない」は専用ログ導入時の記録であり、この一件で下記の範囲を確認した。

- プロパティ生行は119,451件、欠落0、処理中0、対象外618,523件だった。917,584byteのprefixと50,331,728byteの専用ログを各二回採取し、それぞれ全byteが一致した。元の4096行では失われていた記録を全件保持した。この件数は将来の観測時間や再配信回数を保証しない。
- 元の都市喪失54操作は、内容・順序とも完全一致した。追加はMOD反映用再起動、bootstrapの開始・停止、実行器の準備再起動だけである。観測を終了した後も元の技能・都市移管・功績と効果停止・保存読込の判定を通過した。
- 実保存は1,783,326byte、SHA-256は `B09C363E0E1B486DF88BDF8668F59B444DF63A7FDB0AD1A4A2FB9FBDDEC584DC`。保存前53番と読込後58番の返却文字列全体を原stepsから独立照合し、一致した。読込後の新しい加入資格の再接続を独立に実証したものではない。
- 原成果14ファイルと実保存を `D:/CC/Work/todo324/live-city-sidecar-01/` に保全した。集計と各SHAは `D:/CC/Work/todo324/live-city-sidecar-01-proof.json` に記録し、product_acceptance=falseを維持した。ゲームPID74360の作成FILETIMEは134358462812664796で、ゲームとチェーンは終了済み。成功した実行を再試行しない。

静的検査はc80b739a固定で全workspace・全targetが終了0、203結果群・親2678件成功。Luaは実行exeの先頭依存を照合した32群・897件で、失敗・ignored0だった。親のignored2件は子プロセス入口であり、独立子11群を別計数する。後続026ではnative/bootstrap関連検査、全workspace Clippy、releaseビルドが成功した。026で全workspaceテストを再実行したとは扱わない。最終Luna追審はMust/Suggestion/Nitすべて0、公開026の必須CI7項目も成功した。Bugbotは利用枠による未実行で成功に数えない。

通常の自動起動、通常22人の資格、AI本人の費用2/3の正式支払い、余分な昇進と通常未払い配置の拒否、保存読込、途中支払いを含むMP、元の通しシナリオ、正式なスパイ中和後の功績・効果停止と回復は元の受入条件として保持する。今回の限定成功で未達条件を完了へ変えず、PR86とToDo324を未完了のまま継続する。
