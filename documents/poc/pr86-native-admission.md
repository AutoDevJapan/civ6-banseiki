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

公開キーは、対応表900000+Index、資格901000+Index、所有者別の失効902000+owner、人物セルの閉鎖903000+Indexである。初回は各公開範囲と所有者別失効キーの衝突を確認する。同期Gameのschema値3がこの範囲を所有する。旧保存の値1では新閉鎖範囲、値2では新しい所有者別失効キーを確認してから移す。衝突なら元の値を上書きせず、全所有者を失効して中断する。所有者一覧は0〜63の重複しない整数を持つ連続配列と確認し、穴や不正値があれば全所有者を失効して公開を止める。Luaの揮発性スナップショットは前回公開値の比較だけに使い、権限源にしない。所有者の操作はその所有者を、各PlayerTurnはその所有者を、全体Turnは全員を再計算する。変化のないセルの数値propertyは読み書きしない。加入直後・EVOLVING前後・正式Finalize前後も同じ処理である。

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

Rustの `governor_payment` は現在のCanAssignの二つの引数だけを使い、Governors+0x70のPlayer本人と+0x58の本体リスト、Governorの所有者とIndexを照合する。256件以内の循環しないリストと4096件以内の昇進列を読む。長さ、容量、本人、所有者、全昇進byteを再読取し、不一致・読取不能・0/1以外を記録上の拒否として返す。未知の関数は呼ばず、ゲームのメモリへ書かず、ポインターを次の呼出しへ持ち越さない。二重読取は大域的な静止の証明ではない。

本体リストの次リンク+0x10は、正式PROMOTEの走査 `0x2DF366–0x2DF38A` と本人lookupの `0x2DE960–0x2DE984` が同じ位置を読むことを命令境界から確認した。純Rust検査では本人が先頭・中間・末尾にある三つの配置と別所有者を扱う。昇進配列は256 byte単位で二度読み、残りだけ8 byte以下で読む。4096件なら配列のOS読取は32回で、先頭本人の構造照合を含めても64回以内である。この回数はfixtureの測定上限で、実機の処理時間ではない。

mode7のtrace版2は `promotion_snapshot_status`、本人所有者とIndex、昇進数、取得済みPromotion Indexを追加する。status 0だけが読取完了、1は読取不能、2は構造不正、3は本人不一致、4は本体リストの非所属、5は読取中の変化、6は不正なbyte、7は未観測である。失敗時は本人値を未知、件数0、取得一覧を空にする。検証器は型・順序・重複・本人一致を検査する。全行の `payment_authority` はfalseであり、原ALとRAXは従来どおり保持する。

この観測を費用1の実Governorと正式基礎昇進へ照合し、その後費用2・3のTITLE_1/2の実Index、公式PROMOTE前後の本人昇進とSpentを照合する。実機で未証明の段階ではPaidAssignmentの904–907名前空間を公開しない。通常22人、正式な本人支払い、未払い配置拒否B、保存・複数端末、人物化通し、中和成功と回復のMustは継続する。

支払数を読む純Rust関数は `diagnostic_paid` とし、要求された所有者とsnapshot本人が違えば拒否する。費用と基礎・TITLEのIndexの形や順序は検査するが、呼出側が渡すIndexの出所までは証明しない。そのためこれは診断用の解釈に限定し、製品からは呼ばず、snapshotと同様に支払権限へ使わない。製品接続では、そのゲームの同期GameInfoに対応する本人・所有者・各Promotion Indexを照合する生成元を先に完成する。この確認項目を、名前だけの検証済み定義で済ませない。

### 通常の費用2・3と任命途中の保存を確認する検査

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
