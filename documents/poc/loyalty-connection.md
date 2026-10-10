# 忠誠心の単発接続診断（PR116・ToDo995、2026-10-10）

実書込み先から画面の現在値へ届く接続は、まだ特定できていない。製品の受入は候補7dの2/5・40%のまま、小国AI・合併AI・公式都市国家の残り3件を保持する。Luna045814の未特定も解消したとは扱わない。この差分は有限診断であり、製品Lua・共有DLL・二公開の停止条件を変更しない。

## 固定した根拠

Steamの原本を読み、DLLのSHA-256 `324C51E9EA3531758842E16C69E6CDDBEFBB226C5B675C3D6E60111646C2E98C`、EXEのSHA-256 `E7450823CC8E00468CFF7B9D7B97C63140EAE38AE1D774BA4EFA437556C42D63`を照合した。命令列は必要な関数範囲だけを逆アセンブルした。DLLについては診断の開始前にもPEのセクションからRVAを変換し、32組の命令列・符号付き相対参照先・3つのLua型文字列を照合する。読み込まれた命令列も8バイト以下に分割して照合する。

| 境界 | 命令から確認したこと |
|---|---|
| 変更命令 | `713E15 → 025040` はLua型 `City` を解決する。`City+1630` を `115210` へ渡し、現在値枠は受け手+98、差分値枠は+ B0。現在値は各枠+10にある。City型であることから本体の正本であるとは断定しない。 |
| getterの生成元 | `710FE1 → 024EF0` は別のLua型 `CacheCity` を解決する。`710FE9` はその+6D8を生成処理 `710980` へ渡し、`710C55` は同じポインターをLuaのインスタンス欄へ格納する。現在値をここでコピーする処理とは呼ばない。 |
| getterの読取り | `70AF41 → 707E60` は `CacheCityCulturalIdentity` を解決し、`70AF4A` がその+8を読む。したがって生成元の現在値欄はCacheCity+6E0。ここを誰が埋めるかは静的には未特定。 |
| 同一都市 | `690CE8 → 024EF0` の後、`690CF2` はCacheCity+18のu16とCacheCity+0のu32から所有者・都市のキーを作る。診断はこの命令に基づいて両キーを照合する。忠誠値の数値一致やスレッド一致を都市対応の根拠にしない。 |
| City側のキー | City型を解決した後の`713E22`はRAXをRBXへ保存し、`713E2D`がRBX+1630をRCXへ渡して`713E3D → 115210`を呼ぶ。calleeの`11521A`はこのRCXをRBXへ保存する。`11531A`は受け手+90の参照先をRAXへ読み、`115332/115338`がその+A8のu32と+D8のu16を通知候補へ使う。診断は実際の受け手=City+1630、受け手+90の参照先=Cityを要求し、この2つの読取り値をCacheCityのキーと比較する。参照の不一致・読取り失敗は都市確定に使わず終了する。 |
| 書込み登録 | `2CA10D` は枠+8の自己参照を読み、`2CA11A` はその+38の木コンテナを使う。`3F37D` は枠のアドレスをノード+20へ格納し、`3F388` はコンテナ+8を増やす。実機100のself+40=14は、この件数欄でありproperty番号14とは特定できない。`2CA200` は入力を差分枠+10へコピーする。登録後の消費・配送先は未特定。 |

実機100のJSONLでは、City側+16D8が25600→24320、+16F0が24697→-1280に変わり、getterの別アドレス+8は25600だった。所有者3・都市65536の通知候補はキー89DE2474・kind2。二公開後のUIは100→100、対象の通知0件だった。新しい都市対応はこの旧記録へ遡って付与しない。

外部インターフェースの実アドレスからEXEのRVA1CA220を追った。キーと8バイトの引数を渡し、1C90F0で種別0x113のバッファーへ格納する命令を確認した。9898C0の参照先IAT F96728は、原本ではLeaveCriticalSectionである。FE3CF8の文字列参照と3D79C0のLuaハンドル生成経路も確認したが、このバッファーの消費が対象都市のCityLoyaltyChangedへ届くことは未証明。FB629F・FE3D34の文字列だけでは役割を確定しない。既存出力の「slot未呼出し」は観測器自身が呼ばないという宣言で、GameCoreの未呼出しの証拠ではない。

製品のAddGameplayScriptsはEntryからGameplayを読み込み、ChangeLoyaltyを呼ぶ。公式BlackDeathもAddGameplayScriptsでPacify/Purgeから同じAPIを呼ぶ。独自rulesetのGameCoreはExpansion2。日本マップの毎手番+100はMODIFIER_PLAYER_CITIES_ADJUST_IDENTITY_PER_TURNである。これらの実物は確認したが、同じ手番内で反映されない原因とは断定していない。

## 親が次の一回で採るもの

既存の `--native-observer` シナリオとPID・作成FILETIME・新しいJSONL名を使い、観測器の引数に **`--connection-city 3:65536`** を加える。対象が変わった場合はシナリオの実キーを指定する。ready後にシナリオが既に行う「before-setter」の生成処理を捕捉する。その実引数と、続くgetterのポインター・Lua状態・スレッドを一致させて基準値を捕捉する。基準値が捕捉できないままsetterへ来た場合は診断を失敗終了する。setterの追加送信は行わない。

| 時点 | 4枠の用途 |
|---|---|
| ready | setter直前・setter復帰・getter生成元・通知候補 |
| 同じ都市の基準値捕捉後 | 生成元の枠を一度getterへ替え、実ポインター・Lua状態・スレッドが一致した後、その現在値4バイトの書込み監視へ替える |
| 最初のsetter直前 | setter直前の枠を、同じCity+16D8の書込み監視へ替える。復帰と通知候補は保持する |
| setter復帰後 | 通知候補の枠を生成処理とgetterの照合へ替える。公開後に同じ都市の生成元が入れ替われば記録し、監視先を更新する |

書込み監視は全ての存命対象スレッドと新しいスレッドに適用する。書込み後のRIP、DLL内のRVA、直前16バイト、レジスター、スタック先頭、両方の値枠の登録先と木への所属を採る。RIPを命令の先頭や正本の役割とは呼ばない。木の検索は2つの枠のアドレスだけ、各32ノードまで。128生成・getter標本・32書込み標本・4生成元・既存の90秒以内で終わる。二公開後に既存のfinishを使い、時間や公開回数を延ばさない。

切替前に他スレッドで発生済みの例外は、旧アドレス・旧制御値・RIP・DR6・flagsの一致を確認して旧用途で扱う。新しい書込みとは数えない。終了済みスレッドへのGetThreadContext失敗は、生存状態の再確認を行う。復元、保留イベントの続行、デタッチは既存の経路を使う。

一回の記録では、同じ都市の2つの現在値欄、City側の最初のwriter、その後の上書き、キャッシュ側のwriterまたは生成元の入替え、登録の前後、元の通知候補を照合する。キャッシュへ書込みがない結果から、GameCore全体で配送がないとは断定しない。通知の画面到達と製品の受入は、親の実機ログで別に判定する。

## 検査

終了JSONの`connection_capture`は、同一都市のgetter基準値、setter、setter復帰の各捕捉と、3つがそろったかを明示する。後始末成功の終了コードだけでは診断完了と判定しない。捕捉がそろっても、Core正本・UIの実変更・都市通知・製品受入の成功を意味しない。

前版（子e3922321・親3b206391）の観測器の関連nextestは49件成功。生成元の古いLua状態を待ち続けない最後の修正後も、変更した経路の8件が成功した。Clippyの警告なし検査、Windows CLIのビルドとfmtも成功した。ignore指定の子fixture1件は専用の親検査から実際に起動するため、独立実行の対象外である。新しいWindows検査は、自分で起動した子の2つの4バイト書込みを実ハードウェアで捕捉し、書込み後のRIP、元のレジスターへの復元、生存中のデタッチ、子の正常終了を確認した。ゲームや既存の他プロセスには接続していない。実DLLの32組の照合もファイルの読取りだけで成功した。

初回の新しい検査は、切替時に既に終了した隠しfixtureスレッドへのGetThreadContextがWindows error31で失敗した。生存状態を前後で確認して終了済みの対象を飛ばす修正後、該当検査と関連全件が成功した。これを忠誠心の実機成功とは扱わない。

Cityキーの根拠を求めた090359レビュー後、7組の命令と相対call先を追加し、受け手+90の実参照がCityと一致することを要求した。初回の関連検査は、合成子がこの自己参照を準備していなかったため5成功・1失敗。合成子の準備を直した後、実DLLの32組照合を含む関連6件が成功し、44件は対象外だった。Cityキーの読取りオフセットを変えて再hashした合成DLLも拒否する。ClippyとWindows CLIビルドも終了0。原ログは`D:/CC/Work/todo995/connection-identity-review-tests-0925.log`と修正後の`connection-identity-review-tests-fixed-0927.log`、同系列のClippy/buildログに保持した。

## 実機101（native-1743-live08）

候補`8b65cfb8b3fb6221284566bc43e5778fc351f792`、観測器exe SHA `8ECC69350138A3151409F600083F3AAD1970C723ECA35A75B15FD78A730E861B`、原v5 JSONを標準チェーンから一度実行した。City側キーの根拠不足は限定Luna追審091807でMust0。検査結果を前版と区別するSuggestionを文書へ反映した。

14/16手順まで成功し、実owner3・city65536・座標146,38・turn1へ一度だけ-5を発行した。Game PID86464/FILETIME134360653776002940、observer PID81172/FILETIME134360655957086300。変更前のgetter生成元と変更後の生成元はCacheCity834728528、member834730280、current住所834730288で一致し、両キーはowner3/city65536だった。各生成処理とgetterのポインター・Lua状態・スレッドも一致した。City13617720496、receiver13617726176、receiver=City+1630とreceiver+90の参照先=Cityを実測した。

City+16D8は25600→24320、+16F0は0→-1280だった。書込み後のRIPのDLL RVAは64A7E9で、直前に`8B 02 / 89 43 10`がある。同一都市のCacheCity currentは前後25600、getterは前後100だった。getter生成元の入替えとCacheCity currentの書込みはこの観測範囲では0件。これをゲーム全体の書込み不在や正本変更の証明へ広げない。

受け手の登録木は変更前からcurrent/delta両wrapperを含み、変更後もsize14だった（探索各3/2ノード）。登録済みだったことと、変更がCacheCityや画面へ配送されたことは別の事実である。通知候補は1件、生成処理2件・getter2件、City側の書込み1件を記録した。

15番のUI要求は45秒後にFireTunerの接続終了で応答不明となった。再送せず、最後のUI値と都市通知数は未測定として保持する。観測器は約50.947秒で対象の終了を検知し、reason=process_exited、unknown first/second chance各0（記録された件数であり、未取得の例外がないことは示さない）、115スレッド設置、復元0・終了済み1、continue/detach成功・後始末エラー0、observer終了0だった。生存中の復元・90秒の正常観測終了とは扱わない。保持handleの終了をWaitForDebugEventの期限切れ経路で先に検知して戻ったため、target_exitのOS終了コードは記録されていない。対象の終了原因は未確定で、実機98のSTATUS_SINGLE_STEPを101に転用しない。

controllerはobserverの終了0をdoneへ公開し、実行器の失敗経路が完了記録を読んだ後にchain終了1を記録した。ゲーム・観測器・controller・chainの終了と共有lease不在を確認した。ソフトウェア描画設定は元bytes・SHA `C234884BD45BCD371F0D5DC263A61FAA3C4C5BF84B1185FFDED12413A0C647D8`へ復元した。artifact9件とcontrol原本16件の計25件をowned `9bef32ac6025f6c756231a36e724b095/live101`へ上書きせずコピーし、全サイズ・SHA一致を確認した。証跡は`D:/CC/Work/todo995/native-1743-live08-copy-proof.json`とlaunch記録に保持する。製品のCore現在値・都市通知・保存onceと残り3受入は未達である。

## 101後の限定修正候補

Luna093402の限定調査で、終了イベントより先に保持handleの終了を検知するとOS終了コードを取り逃す診断欠落を確認した。後始末で対象終了を確認した場合は、その保持process handleからGetExitCodeProcessでコードを読む。`target_exit_status`には実値か読取エラーを出し、後始末成功・observer終了0と対象の正常終了を区別する。これで101の欠けた終了コードを遡って補うことはできない。

各観測枠の切替後はDEBUGとCONTROLを実際に読み戻す。変更した全対象のRIP・RF/TFの変化数を集計し、保持中の対象と最大15個の他スレッドの前後を記録する。省略数を明示し、128レコードの上限を維持する。DR6のOSによる予約ビットの正規化を、アプリ例外がなかった証拠とはしない。

診断生成器は、UIで作ったgetter関数をGameplayの同期ハンドラーから呼ぶ処理を除いた。観測器がreadyになった後、UI要求内で基準値を読んでから一度の変更操作を要求し、GameplayはCityの当事者・手番・再送を検査してsetterだけを呼ぶ。二公開後のgetterは元のUI手順で読む。これは呼出しコンテキストの分離で、101の終了原因を証明した修正とは扱わない。製品Luaは変更していない。

生成器3件が成功・省略0。終了コードの実取得を加えたWindows子を含む観測器関連8件が成功・42件は対象外だった。この8件の任意DLL監査fixtureは環境変数を設定していないため実原本を読んでおらず、前の6件の実DLL32組照合を新しい結果へ付け替えない。切替後RIP・RF/TFの実読戻しの検証を加えた最後の所有子1件も成功・49件は対象外。この1件は8件と重複するため件数を合算しない。Clippyの初回は遅延評価の不要なclosureで失敗し、同値の`Option::and`へ直した後のClippyと両CLIビルドは終了0。原ログは`D:/CC/Work/todo995/connection-separated-ui-generator-tests-0945.log`、`connection-exit-and-context-tests-0948.log`、`connection-control-readback-child-0952.log`と同系列のClippy/buildログに保持する。

## 実機102（native-1743-live09）と隠しスレッドの有限対策

親`781ca2ef5868f53b138706db81a98a94e582b79b`、v6のUI分離sceneは12/16まで成功した。ready後の13番のUI要求でbefore getterを呼び、接続エラー10054で結果が不明になった。約26.403秒で対象が終了し、保持process handleのOS終了コードは2147483652（`STATUS_SINGLE_STEP`）だった。基準getter・setter・native triggerの捕捉は0。未知first/second chanceの記録も0で、復元0・終了済み1、observer終了0・chain終了1だった。原JSONLの設置数と`probe_install`の実件数は108であり、依頼要約の115とは区別する。101の欠けた終了コードは102から遡って確定しない。

原本25件の保全は既存の`D:/CC/Work/todo995/native-1743-live09-copy-proof.json`（count=25）に記録されている。artifact102のLua.logは所有ID`26ca61c2797f4e083dd92d3b055921b0`の`live102/artifact/Lua.log`へ保全済みで、同記録のSHAは`CFDF3F8E3F062496CA728229CBF4DF4544ACC545EB7E561D6D4191C3AC5DFEA3`。今回、ゲーム・共有DLL・scene・保全原本は変更せず、新しい実機試行も行っていない。

隠れたUIスレッドへ全スレッドのexecution/dataプローブを置いたという仮説は、終了コードと配送された例外がない結果に整合する。ただし実機102のスレッドの隠し状態は未測定で、この仮説を実機の確定原因とは呼ばない。

所有Win64子で修正前の失敗を先に再現した。新しいworkerへ旧0x45計画を設置した後、worker自身が既存`NtSetInformationThread(0x11)`で隠れ、待機する。通常スレッドのmarkerで全スレッドの0x55 getter計画へ切り替えてから、隠しworkerのgetterを実行した。OS終了コードと終了イベントはともに2147483652、子の終了確認はtrue、未知first/second chance記録は0だった。親のkillや期限切れではない。`hidden-thread-red-01.log`と対応metaを成功後も保持する。

照会は保持済みのローカルntdll関数だけを使い、context操作と同じthread handleへ`NtQueryInformationThread` class17を渡す。BOOLEANをRustの`u8`で受け、長さ1と返却長を照合する。所有workerの同じハンドルで、通常はstatus=0・length=1・value=0、hide後はstatus=0・length=1・value=1を実測した。`THREAD_QUERY_INFORMATION`（0x40）を持たない限定照会ハンドルは`STATUS_ACCESS_DENIED`（0xC0000022）。4バイト出力の実照会は`STATUS_INFO_LENGTH_MISMATCH`（0xC0000004）だった。初回の照会検査はfixtureがprocess pseudo handleを独立所有ハンドルとして複製しようとして失敗した。自身のprocessを独立ハンドルで開くfixtureへ直し、実照会を確認した。両方のログを保持する。class9のstart address照会は変更していない。ABIの参照は[Microsoftの関数仕様](https://learn.microsoft.com/en-us/windows/win32/api/winternl/nf-winternl-ntqueryinformationthread)と[phntのBOOLEAN定義](https://github.com/winsiderss/phnt/blob/master/ntpsapi.h)であり、動作の証拠は今回のOS実測である。

preflight・新しいスレッドの登録・設置直前・接続計画の切替・trigger getter増設が同じ判定を使う。hiddenと、NTSTATUS失敗・返却長不一致・BOOLEAN不正によるunknownは新規設置0として理由を記録する。既設の自前プローブがあれば、同じハンドルを停止して既存のrestore経路で自前の枠だけを復元し、所有した停止回数を1回だけ戻す。以後そのスレッドへ再設置しない。後からアプリが変更した別枠・制御・未分類DR6状態を保持し、CONTROLは書き換えない。

隠しgetterを捕捉できない結果からCacheCityの値を推定しない。所有子でも基準値なし・setterなしを維持し、基準値なしのsetter要求を拒否する。都市キー・同じtrigger thread・例外配送の条件、128 trace・90秒・既存標本数の上限は維持する。

この対策の範囲は、照会時点でhiddenまたはunknownと判定できるスレッドへの設置を防ぎ、検出できた既設の自前プローブを外すことに限る。照会と設置の間、または設置後にスレッドがhideへ変わり、次の照会より先にプローブを踏む競合は残る。CREATEイベントではunhiddenだったworkerが設置後にhideする場合も同じ限界がある。解除・kernel flag変更・例外handler注入・追加hookで埋め合わせず、完全解決とは断定しない。

これは診断による対象終了事故を防ぐ有限修正であり、Core現在値変更成功ではない。親の製品受入は候補7dの2/5・40%のままで、小国AI・大国AI・公式都市国家の3件が未達である。

検査中の失敗も保全した。`hidden-thread-nextest-final-01.log`は、事前検査のfixtureを自身へ向けて自分のスレッドを停止したため、11成功・1停止失敗・46未実行だった。該当する所有検査プロセスだけを、パスと生成時刻を照合して終了した。事前検査を別の所有子へ向けた後、その隠しgetter実行とOS終了0を確認した。`hidden-thread-fixture-fixed-02.log`の復元後配送fixtureは、分類前に復元してDR6読戻し不一致となった。実経路と同じく分類を先に行う順序へ直した。`hidden-thread-queued-fixed-03.log`の残った失敗は、そのfixtureがアプリ例外の前で止まることを共通の検査条件へ反映していなかったためで、修正後の`hidden-thread-queued-fixed-04.log`は成功した。これらを実機102の原因や修正前の隠しプローブ再現とは混同しない。

最終sourceで観測器の関連nextest59件が成功、独立実行をしない子fixture1件は対象外だった。事前検査、設置直前、登録時、getter増設、動的切替、実照会失敗の各経路を含む。隠しgetterを実行した全5モードと事前検査の所有子はOS終了0・終了確認trueだった。前者では別のアプリ用DR枠を保持し、RF/TFの変化0と自前プローブの復元を実読戻しで確認した。登録時の検査は既に隠れた実OSスレッドの保持handleを再登録するものであり、CREATEイベント後にhideする競合を解決した証拠ではない。基準getter・setterの捕捉はfalseのまま。元のconnection/writewatch/route/restore、復元後の保全snapshotからの自前例外配送、実DLLの32組監査も成功した。Clippy（warnings拒否）、fmt、Windows CLI buildは終了0。所有子の残存は0だった。

今回の原ログとコマンド・開始終了時刻・終了コード・source SHAを含むmetaは、子の生成先`C:/Users/makhmeto/AppData/Local/Temp/codex-owned-6bfd169444d1814547e40876dbdba279/target/`に保持する。最終検査は`hidden-thread-nextest-final-02.log`、`hidden-thread-clippy-final.log`、`hidden-thread-fmt-final.log`、`hidden-thread-build-final.log`。先行失敗は`hidden-thread-red-01.log`、実照会は`hidden-thread-query-02.log`。検査済みobserver source SHA-256は`050BFCDC60AA85287210D3E6FC0F484A68F0CCF16FC561A1B2E9E58663DF461F`、Windows exe SHA-256は`F764EF495E350F5E546AFAD7DED9DF04747B19F15567E79319321F4D6C1DADF6`。親受領commit`9c9216fbc12b6d62c1759084c914e1f4f6115892`は旧e392と親781を両祖先に持ち、返却用の有限修正commitとは分けた。

## 同一アドレスで制御が変わった枠の復元（112508レビューMust1）

親は子47c0ebb0をf1ed1671へ受領した。追加レビューは、同じ設置アドレスのDR7をアプリが変えた場合、その活性が残っていても読戻し一致だけでarmed=false・復元完了へ進む問題を指摘した。この子ではobserverと本書だけを修正し、生成器・identity/型/文字列のmetadata・製品処理は変更していない。共有ToDoの状態・担当・場所・noteも変更していない。

復元計画は、所有を照合して戻した枠と、観測器が実際に使った枠に同じアドレスの監視が有効なまま残る場合を別に返す。後者はL/Gのいずれかが有効なら競合であり、アプリのアドレス・DR7・未分類DR6を消さず、実読戻しの残存mask・枠番号・L/G・RW/LEN・理由・元/変更前/要求/実値を記録する。読戻しが成功しても残存maskがあればarmedを解除せず、復元完了はfalseになる。無効化済みの同一アドレス、別アドレスのアプリ監視、元から使っていない枠へ追加された監視は活性残存と区別する。

競合はAPI失敗とは別の復元結果で返す。cleanupは他スレッドの復元、保留イベントの続行、デタッチ、所有した停止回数の解除を最後まで行う。照会失敗時のskip経路で競合を検出した場合も、所有した停止を戻してから観測を終える。既に保持する停止回数をfreezeで再び増やさない。APIの後始末と復元完了を分け、競合を成功数へ加算しない。

競合履歴は復元試行時の事実として保持する。対象が後から終了すれば現在のarmed・残存maskは終了済みへ更新するが、元の競合履歴は消さず、診断の全体完了はfalseのままである。終了済みの現在API事実と過去の有効監視を混同しない。履歴は最大128件、超過は省略数を明示する。既存traceの128件上限、90秒・二公開・都市対応・32組監査の条件は変えない。

先行ログ`restore-control-red-01.log`で、所有Win64子の同一アドレスをexecutionからwrite4へ実変更し、別の未使用枠へアプリ用data watchを追加した。復元後も同一アドレスの監視が有効だったが、旧コードはarmed=false・cleanup_complete=trueを返し、新しい検査が失敗した。子のOS終了は0である。途中の`restore-control-green-01.log`はfixtureの借用エラーによるビルド失敗で、実検査は未実行。修正後の`restore-control-green-02.log`では競合の2件が成功した一方、execution例外を保留していた対照子はOS終了2147483652となった。後者の配送原因は未確定で、再実行成功へ置き換えず原ログを保全した。

最終fixtureはexecution監視やアプリ例外handlerを使わない。所有子が読み書きも解放もしない専用領域にwrite4監視を設置し、通常の停止中に同じアドレスをwrite1へ変える。別slotへ追加したアプリ用write4監視も実OSで読み戻す。子はgoまで待ち、復元後の状態を確認してから終了0となる。残存検出を一時的に外した`restore-control-safe-red-02.log`でも、同一アドレスが有効なのに完了trueとなって検査が失敗した。検出を戻し、元sourceのSHA一致を確認した後、新規4件が成功した。対象外60件は旧59件と独立起動しない子fixture1件であり、旧59全件・全workspace・CIは再実行していない。

実測した3つの所有子は、直接cleanupの競合、照会失敗によるskip中の競合、未変更自前枠の正常復元である。すべて保持spawn handleと生成時刻で対象を限定し、OS終了0・残存プロセス0を確認した。競合時の残存mask=1・slot0・armed=true・cleanup_complete=false、API後始末trueを確認した。アプリ用slot2は復元後も保持し、他スレッドは復元、continue/detach・所有した停止解除も成功した。終了後は現在mask=0・process_exited=trueだが元の競合履歴と完了falseは残る。正常対照は元の自前枠だけが除去され、追加アプリ監視が残り、完了trueだった。

pure policyの9条件は、未変更、Gのみ、L/G両方、LのみのRW変更、RW/LEN変更、LENだけの変更、制御変更後の無効化、元制御への無効化、別アドレスを区別する。未分類DR6と追加アプリ枠を保持することも固定値で確認した。Gのみ・L/G両方・RWだけの変更・無効化の全ケースを実OSで測ったとは主張しない。実測はLのみの安全なdata watchとLEN変更であり、非zeroのアプリDR6保持はpolicy検査の範囲である。ゲームや実機には接続せず、設置後hideと保留execution例外の配送の限界を解決した修正とは呼ばない。Core現在値の変更成功・製品受入の更新ではなく、製品2/5・40%・残り3件は維持する。

最終検査は`restore-control-green-03.log`（4成功・60対象外）、`restore-control-clippy.log`（warnings拒否、終了0）、`restore-control-build.log`（Windows CLI、終了0）。それぞれのmetaにコマンド・開始終了時刻・実行元commitとsource SHAを保存した。場所は既存の子生成先`C:/Users/makhmeto/AppData/Local/Temp/codex-owned-6bfd169444d1814547e40876dbdba279/target/`。最終observer source SHA-256は`CBA9E5C3DCBB7BB262C9B720324726675FFA62EC996706C05714B21DA8591B08`。旧hidden-thread系列の原ログ、102の`D:/CC/Work/todo995/native-1743-live09-copy-proof.json`と保全25件にも触れていない。

## 監視に依存しないUI・Gameplayの単発比較

実機103はUIにrawgetがないため情報記録で失敗した。実機104は通常のtable field lookupへ修正後、10/10手順とログ5/5が成功した。identityの実型はtable、__instanceの実型はuserdata、getmetatableはfunction、metatableとcustom formatterはnilだった。表示文字列は保全するが、これだけをメモリアドレスや正本の根拠にしない。両実機はsetter・操作要求・観測器が0で、忠誠心変更の受入ではない。ゲーム終了・描画設定の元SHA一致を標準chainで確認した。

生成器の --single-setter-ui は、UIで実値の基準を読んでから同期操作を一度要求する。Gameplayは基準の取得と当事者・手番・再送を検査し、変更命令だけを実行する。UI getterのclosureをGameplayから呼ばない。観測器のreadyを偽に用意する処理はなく、この方式は観測器を起動しない。二公開後の実getterと都市通知数を記録し、ターン・公開回数・停止条件を延長しない。元の --native-observer の検証付きready/finishは維持する。この追加方式も診断であり、Core正本の更新や製品成功をCLI完走で代用しない。

実機105（5c4cfca0）の通常UI要求からの単発比較は14/14手順・ログ5/5成功、owner3/city65536/座標146,38/turn1でUI100→95・CityLoyaltyChanged1件を確認した。setter CALL/RETURNは各1、観測器0。待機条件は元の二公開以上で、最終記録の実カウンターは11だった。公開数を増やすコード変更やsetter再送はしていない。この成功を製品5条件へ付け替えない。

--single-setter-publish-ui は、同じ一度の要求を次のGameCoreEventPublishComplete callback内から送る比較である。登録時には要求せず、再入や応答不明でもUIRequestIssuedを先に保持し、再送しない。変更先・基準・ターン・二公開の条件は105と同じで、送信する時点の違いだけを比べる。製品の公開callbackが同様に同期要求を送っていることは静的に確認したが、それが原因とはこの追加実機を終えるまで断定しない。

実機106（cde9e81d）は公開callback内からの要求でも14/14・ログ5/5成功、105と同じowner3/city65536/146,38/turn1でUI100→95・CityEvents1、pubCounter10、CALL/RETURN各1だった。要求時点だけでは失敗を再現しないので、公開callback自体を原因とする仮説は採用しない。原11filesをownfe29634f/live106へ全SHA一致で保全し、standardclose/profile元C234復元を確認した。

--single-setter-ai-ui は送信者をactive human3のまま、対象だけ既存AI0の非首都へ変える。ActorとOwnerを別に保持して、Coreはactive senderと対象city owner/id/turnを別々に検査する。変更と再送の回数・二公開・ターン条件は105と同じ。これでAI所有・非手番条件と、未達製品での都市移管後という別の条件を分ける。AI対象のsetterやUI結果は実機前に成功としない。

実機107（ae9b8051）の既存AI0非首都/131073/129,39/turn1も、sender3の一回の要求でUI100→95・対象CityEvents1、pubCounter11、14/14・ログ5/5だった。既存都市のAI所有・非手番だけでは旧失敗を再現しない。原11filesをowna9caf719/live107へ全SHA一致で保全し、standardclose/profile元C234復元を確認した。次の製品108は元smallAI38手順で未払い保存/読込まで成功し、自然な資格成立を待っている。

--single-setter-ai-cross-ui は107からGameplayのsetter直前にUIのReadCurrent closureを1回呼ぶ条件だけを戻す。旧native診断100に含まれていたこの処理と、正常107との差を監視無しで比べる限定診断である。製品へUI closure呼出しを追加しない。actor/owner/都市キー/実before/一回setter/二公開/ターンは107と同じ。再送や観測回数延長で通さず、結果が出るまで原因を断定しない。

実機108（ae9）の元smallAI製品は27/38成功後、owner0/city196610/120,40/turn15でbefore100/target95、issuedPublication1403の後1404/1405でも100となりBLOCKEDした。物理保存2本を含む原本14filesをown3d27215d/live108へ保全。実機109（25a）のCore→UI getter一回を戻した対照も、14/14・ログ5/5、既存AI都市131073のUI100→95・CityEvents1・pubCounter11だった。追加したgetterだけでも旧失敗を再現せず、その境界を単独の根因にする変更は採用しない。109原11filesはown413e5cce/live109へ全SHA一致で保全。両標準chainはgame終了・profile元C234復元を確認した。

--single-setter-ai-capital-ui は107の対象選択だけを本体のGetCapitalCityへ変える。残り4手順は107と一致し、sender3/owner0/実before/一回setter/二公開/手番を維持する。移管後の失敗都市が元の首都だったことと、都市移管そのものの条件を分ける比較である。首都の変更可否や移管の根因は実機前に断定しない。


## 移管して公開した都市への一回のsetter（実機前の有限対照）

実機110の既存AI0首都への比較は14/14手順・ログ5/5成功、UI100→95・対象CityEvents1、CALL/RETURN各1、観測器0だった。105の通常UI送信、106の公開callback送信、107の既存AI0非首都、109のCoreからUI getterを一回呼ぶ比較も同じ陽性結果だった。既存都市の送信者・AI所有・非手番・首都・VM境界だけを失敗の原因とする仮説は採用しない。

実機108の元smallAI製品は27/38成功後、移管都市owner0/id196610/120,40/turn15でbefore100/target95、発行公開1403の後1404/1405も100となりBLOCKEDした。保存2本を含む原14filesは保全済みで、この結果を110の成功へ付け替えない。

新しい内部CLI `--single-setter-transfer-ui` は原8手順の場面準備を維持し、owner1（ODA）の本体GetCapitalCityだけをAI0へ移す。送信者はactive human3のまま、送信者と対象所有者を別に保持する。元都市owner/id/座標/turn/Tag、手番、未発行、元所有者に他都市があることを同期callbackで検査し、TransferIntentを記録してからCityManager.TransferCityを一度だけ呼ぶ。Coreで座標から再取得し、owner0・座標・同turnを照合して新IDを保存する。移管や要求の応答が不明でも再送せず、他の資産・部隊・通常admission・費用には操作を加えない。

移管完了後の実GameCoreEventPublishCompleteで、UIが新owner/ID/座標のCacheCityと実getterを照合する。その後のUI要求で新キーを参照するgetter closureから実値を読み直し、これを忠誠心の基準にしてCoreのChangeLoyalty(-5)を一度だけ要求する。CoreはUI getter closureを呼ばない。元のBaselineIdentityを保持し、移管後のidentityも通常のtable field lookupで型と__instanceの型を記録する。文字列からポインターを推定せず、メモリを読まない。忠誠心の都市通知・公開カウンターはsetter要求時に初期化し、移管通知を成功数へ加えない。setter後は元と同じ実二公開条件でUI値と都市通知・手番を記録して終える。移管直後のsetterを同じgameで追加せず、ターン進行・待機延長・保存・再送・観測器のready偽造はない。

これは移管と移管後の公開を条件に加えた未実機の対照であり、根因の断定や製品修正ではない。実Lua 5.1 fixtureでは登録callbackを動かし、Actor3・Owner1→0・ID変更、UIだけのgetter、Coreだけの移管、副作用1+1の順序、応答不明・再入の再送拒否、キーや手番の不一致拒否、UI公開基準前のsetter要求0を検査する。旧Native observer、製品Lua、受入37手順は変更せず、旧4件・59件の検査は再実行しない。人間製品と移管save-onceは未達のまま、親の製品2/5・40%・残3件を維持する。次の製品修正は親が実機結果に基づいて決める。

生成器に限定した14件は成功・省略0だった。Coreでの登録と公開後の基準未取得をfixtureで明示した後は、新5件だけを再検査して成功・旧9件は対象外とした。最終Clippy（warnings拒否）・build・15手順の生成JSONに対する既存CLIのcheckは終了0。新旧の原ログと時刻・コマンド・source SHAを記録したmeta、生成JSONとexeのSHAは既存6bfdのtarget内の`transfer-generator-*`に保全する。Lua 5.1構文検査は新7手順と原8手順内のLuaを対象にした。game・公開push・製品の受付は親だけが実行する。

## Windows CIのcleanup resume終了競合（有限修正）

Windows job 114150730563の失敗原本は `D:/CC/Work/todo995/ci-6c79-windows-failed.log`。UncertainDr7のresumeでcount 0が2件あり、DR復元・continue・detachは成功、競合履歴は空だった。原本には保持スレッドの終了確認がなく、終了競合が原CIの原因とは断定しない。親6c79b814のmerge後、旧検査を1回実行すると成功した。旧判定を抽出した新しい所有子の回帰検査では、子自身の終了後に同じ保持ハンドルがcount 0を返し、旧判定のエラーを再現した。

修正は同じ保持ハンドルのWaitForSingleObjectをAPI前後に使う。終了確認時は時点・raw count・期待値を残し、生存中の0・不正count・Wait失敗はエラーのままにする。追加resumeは行わない。新4件と旧関連4件はnextestで8成功・60選外、CIに合わせた同一プロセス内の並列libtestも8成功・60選外だった。元のignored入口は通常検査の成功数に含めず、新回帰の所有子として起動し、終了0を別に確認した。DR競合履歴がある場合のcleanup未完了は維持した。Clippy・build・書式確認は終了0。原ログと指紋は既存6bfdのtarget内の `resume-race-*` に残す。未知GETTERのoriginal context配送は未証明のまま。generator・ゲーム・製品の受入・共有ToDo・公開pushは変更していない。
