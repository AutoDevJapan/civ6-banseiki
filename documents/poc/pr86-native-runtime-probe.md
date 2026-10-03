# PR86: 本体任命資格の実行時観測器

本体資格を変更せず、標準モードでは`CanEver`と`CanAppoint`相当の入口を観測し、propertyモードではPlayerの所有者候補と試験用property getterを観測する。**初回実機53は失敗した。今回の例外分類とpropertyモードの統合後は実Civ6へ再attachしていない。** 専用Rust子プロセスの合格を、加入資格の修正やAIの任命成立へ読み替えない。

## 初回実機53の失敗と今回の切り分け

親の`leaderperson-53/eligibility-ui.jsonl`には、ゲームPID `71856`、作成FILETIME `134354995302818060`、GameCoreロードbase `0x1a49b0000`、DLL SHA-256 `324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c`のidentityと、141 threadのreadyだけがある。`entry`と`return`は0件。2026-10-03 20:14:31.824 JSTのWindows Application Error 1000は、GameCore内RVA `0x2DDDD0`の`0x80000004`（`SINGLE_STEP`）でCiv6が失敗したと報告した。最初のUI Lua判定は接続切断で停止した。後続の`critical_cleanup`はthread `7180`の`SuspendThread`がAccessDeniedを返した記録であり、初発の例外と同じ原因とは扱わない。`stop`行の`detached=true/result=error`は実験成功を示さない。

旧dispatchはthread contextのDR6 bitとRIPが同時に一致しないと`SINGLE_STEP`を未処理で渡した。一方、実機53ではRIP・DR6・DR7を記録していないため、どちらが一致しなかったかは確定できない。修正では`OwnedProven`・`Unrelated`・`Ambiguous`を分ける。登録済みthreadに自分が設定したDR0〜DR3のアドレス・enable bitがあり、例外アドレスとRIPも同じslotに一致した場合だけ`OwnedProven`とする。DR6 bitだけが0でもこの所有判定は失わない。未所有の`SINGLE_STEP`はRIP・DR6・DR7・例外アドレス・first chanceをJSONLに記録して元の例外handlerへ渡す。

RIP不一致・debug registerの不一致・context取得失敗は両モードとも`Ambiguous`である。ABIを読まず、通常の復元→detach→ゲーム再開には進めない。pendingの例外を保ったまま`critical_cleanup`を親へ通知し、同じゲームの終了要求に成功した旨を親から受け取ってから終了イベントを処理する。終了前にその例外を`DBG_CONTINUE`で処理済みにしない。元のdebug registerは可能なら戻すが、復元成功を曖昧な例外の正常終了と混同しない。実機53の`SuspendThread: Access is denied`を受け、後始末ではdebug registerを書き換えたthreadの保存済みhandleを使う。未登録threadは停止せず、生存中の登録済みthreadすべてで元値への復元を確認する。実機での解消は未確認である。

## 固定した対象と境界

- 観測器: `tools/civ6-native-probe-rs`。Windows x86-64でのみ実行できる読取専用CLI。Linuxでは実行不可を明示し、呼出し元の分類など共通処理だけを検査する。
- 対象: 標準 `live_chain` の共有ロック `D:/CC/Tools/civ6-live/civ6.lock` が示す生存中のchain PID。chain実行ファイルは同ディレクトリの`live_chain.exe`と完全パスで照合する。controllerのPIDと作成時刻に加えてOS上の実親PIDを照合し、ゲームのPID・作成時刻・完全パスを固定して再照合する。
- GameCore: `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll` が対象プロセスへロード済みで、SHA-256が `324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c` の場合だけattachする。ASLR後のロードbaseへRVAを足す。入口の16バイトをDLLファイルと比較し、既存のコード改変がある場合は拒否する。
- 関数入口: `CanEver`候補RVA `0x2DDDD0`、`CanAppoint`候補RVA `0x2DD820`。名前とWin64 ABIは静的解析からの候補である。実機でのRCX=this、EDX=総督Index、AL=真偽の確認が残る。
- 操作: `DebugActiveProcess`、`DebugSetProcessKillOnExit(FALSE)`、threadごとのハードウェア実行breakpointを使う。DLLファイル・ゲームの命令・人物台帳・本体会計は書かない。debug registerを変更したthreadの元値を保存し、停止・期限切れ・guard失敗時に元のhandleでそのthreadだけを停止する。生存threadのdebug registerを再読取して元値と一致した場合だけdetachし、停止したthreadを再開する。復元時は現在のRIP・RSP・EFlags・一般registerを古いsnapshotで上書きしない。元からdebug breakpointが使用中なら拒否する。
- 標準モードのメモリ読取は指定DLL入口16バイト、指定DLL内の呼出し命令5バイト、入口時stackのreturn address 8バイトと、静的に識別した `this+0x70`、`Player+0xD8`、`Player+0x528` の先頭vtable候補8バイトに限る。別の `property` モードでは同じPlayerの公式Lua getterから到達した値ポインタの先頭16バイトを追加で読む。いずれも候補値として記録し、Lua値の型と解釈しない。Steam認証や環境変数などの無関係なメモリは読まない。
- 観測は最大300秒・20,000 JSONL行・共有lease30分内。controllerの終了、ロック所有者の変更、対象DLLのunloadで停止する。

## JSONLの読み方

`identity`にゲーム実行ファイル、作成時刻、ロードbase、GameCore SHAを記録する。`ready`の後に`entry`と`return`をIDで対応付け、`stop`にdetach結果を記録する。`entry`にはthread ID、関数RVA、`this`の生ポインタ、32ビットIndex、stack由来の`return_site_rva`を別項目として保存する。直前の5バイトが対象への直接`E8`である場合だけ`caller_rva`を設定する。`0x2DD861`からの末尾分岐は新しいreturn addressを作らないため、return addressとstack位置の両方が同じ`CanAppoint`入口だけを`tail_from_can_appoint`で補助分類し、架空の直接callerを作らない。`return`にはALの生値と、0/1だった場合だけの真偽候補を保存する。戻り値を観測できない呼出しは`entry`の`return_breakpoint_armed=false`または対応する`return`欠落で分かる。ポインタからowner IDを確定しない。

`path_candidate`は既存の逆アセンブルで得た呼出し箇所との照合名であり、AI実手番の文脈を単独で証明しない。主な候補はUI getter `0xBA51E`、公式要求 `0x30DA56`と`0x2DCFFE`、AI解析領域 `0x4A8114`、未分類 `0x21B9E4`。`owner_id_candidate`と`property_interface_candidate`も実機での同一性と寿命を確定した値ではない。親のシナリオがAI実手番・公式要求・UI getterを別の区間で起動し、同じ本体関数の実呼出しと戻り値を突き合わせる。

シナリオで `mode=property` を指定すると、runnerは子プロセスへ `--mode property` を渡し、identityにも同じmodeを要求する。4つの固定breakpointをPlayer getter `0x6EDC76`、試験用文字列キー `0x15339`、property getter前後 `0x1535E/0x15361` に割り当てる。このモードは本体資格の戻りALを採らない。Player getterとproperty getterが同じスレッド・同じPlayerのinterfaceで一致した2つの試験用キーだけ、値ポインタの先頭16バイトを記録する。停止後に各キーの読取・更新・削除の欠測があれば失敗する。実機診断と次コマンドは[加入資格の実装記録](pr86-native-admission.md#task853の実装候補と実機待ちの境界2026-10-03)にある。

## live_chainからの接続

Rust runnerのシナリオに次を追加できる。observer実行時には`CIV6_LIVE_CHAIN_PID`と`CIV6_LIVE_LOCK_PATH`が必須で、共有ロックの一致がなければ開始しない。`civ6-native-probe.exe`はrunner実行ファイルと同じディレクトリに配置する。runnerはゲームのprocess handleを保持し、渡された作成時刻とOSから読み直したFILETIMEを既存runnerと同じ式でUnix時刻へ変換して照合する。許容差は0秒。不一致ならobserverを起動しない。保持した同じPID・FILETIME・実行ファイルパスをobserverの引数と、失敗時の終了対象に使う。

起動前に単一の絶対期限を定める。親の**起動全体は20秒**、ready待ちは最初の10秒、子のattach ready待ちは8秒、復元再試行は6秒、親による同一ゲームの終了要求に最後の2秒を確保する。これらは`civ6-native-probe-rs::startup_contract`の共通定数で管理する。子は初回Windows breakpointを受け取る時点とready通知の直前にも8秒期限を確認する。通常の停止はその要求時から別の20秒期限とし、`stop`とDropが共有する。共有ロックの残りが起動20秒に足りなければ起動前に拒否する。readyを得られず停止へ移った場合、遅れて届いたreadyを成功に変えない。

生命周期は`attaching → ready → stopping → detached`を正常経路とする。ready待ち失敗やOS異常は`stopping → critical → target_exited`へ進む。子が終了しても標準出力の終端まで重大エラー通知を読み、JSONL末尾の`stop/detached=true/result=ok`と終了コードを照合できなければ正常終了にしない。期限内にdetachも同一ゲームの終了も確認できない場合は`unresolved`で失敗する。runnerのエラー時とDropも同じ期限を使い、終了要求を重複しない。observer自身もcontrollerの生存を監視する。

復元・detach・thread再開でOSが失敗した場合は、6秒以内で再試行する。復元値を確認できない間は通常detachへ進まない。期限後は`critical_cleanup`をJSONLとcontrollerの標準出力へ通知し、observerはdebuggerに留まって終了イベントを処理する。runnerは同じPID・作成時刻のゲームだけを既存の終了関数で閉じ、試験を失敗として記録する。ゲーム終了の確認を最大12秒待つ。`DebugSetProcessKillOnExit(FALSE)`の初回失敗も、detachを確認するか終了時のkillを無効にできるまで再試行し、どちらもできない場合は同じ重大エラー経路に入る。ゲーム終了まで確認できないOS障害は`critical_cleanup unresolved`で報告し、安全な正常停止とは扱わない。

```json
{"action":"start_native_governor_trace","name":"pr86-ai-turn","max_seconds":300}
{"action":"stop_native_governor_trace"}
```

runnerは開始と停止の対、名前、300秒上限、観測中のゲーム再起動禁止を検証する。親の実機シナリオでは、静的gateの484組、21AIの最初の実手番、正式Join前後の公式要求とUI getterの区間を分けて使う。observerが存在するだけでは実Civ6を起動しない。

## 独立検査と限界

- Windowsの専用Rust子プロセスで、標準モードの入口・戻り値、新thread、正常stop、期限切れ、guard失敗、記録上限到達時の例外処理を検査した。各停止後に子プロセスが実行を続け、debug registerがattach前と同値であることを確かめた。`SetThreadContext`拒否時の再試行と、永続拒否時にcontrollerが同じ子プロセスを閉じる経路も区別した。実際のWindowsハードウェアbreakpointで、テスト用`CONTEXT.Dr6`を0にした所有例外、RIP不一致とcontext取得失敗の保留、別の`SINGLE_STEP`の子側handlerへの引き渡し、期限後のready拒否を検査した。propertyモードでも所有・未所有・曖昧な例外、終了済みthreadと生存threadの後始末を検査する。いずれも修正後の実Civ6成功を示さない。
- runnerのRust検査で開始・停止のdispatch、名前と時間の上限、未対の開始・停止と観測中の再起動の拒否を確認した。試験用子プロセスでは作成時刻の不一致、誤ったprocess identityでの終了拒否、ready期限前後の重大エラー、期限より遅い通知、厳密な終端JSON、`stop`とDropの共通期限、一度だけの終了要求を確認した。実機53のreadyは記録されたが、正常detachと30分leaseの完走は未確認。
- 統合後はこのworktree専用の`tools/target-native-integrate`で全ワークスペースの`--all-targets`を実行し、103試験群・1516件成功、失敗0・省略0を確認した。最後にdebug registerの実行条件を照合する判定を足し、観測器の28件を再実行して成功した。Lua 5.1とWindows実子プロセスの検査を含む。全93シナリオの`--check`とRustの整形検査も通った。実Civ6でのproperty観測は未実行。
- `tools/civ6-pe-rs`はこのworktreeにある。観測器は既知の6停止位置のファイルオフセットを固定してバイト比較し、PE parserを複製しない。別ビルドやRVA追加に対応する段階で、既存PE parserの必要部分を共通libraryに移す。
- 静的gateの実機43では484組が偽、21AIは称号各3・使用0・任命0、正式Join後の公式APPOINT1回も同期barrier後に本体任命なし・使用0だった。この観測器はその原因を切り分ける測定器であり、本体の動的資格機能は未実装のまま。

前提と元の静的解析は、親担当のworktreeにある`documents/poc/pr86-native-admission.md`と[枠診断](native-slots-diagnostic.md)に記録されている。SSOT 4・9・16の挙動は変更していない。

Windows APIの根拠はMicrosoftの[稼働中プロセスへのattachとdetach](https://learn.microsoft.com/en-us/windows/win32/debug/debugging-a-running-process)、[終了時のdetach設定](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-debugsetprocesskillonexit)、[debug eventで受け取るhandleの扱い](https://learn.microsoft.com/en-us/windows/win32/api/debugapi/nf-debugapi-waitfordebugeventex)、[例外レコードのアドレスとfirst chance](https://learn.microsoft.com/en-us/windows/win32/api/minwinbase/ns-minwinbase-exception_debug_info)、[続行状態による例外処理の違い](https://learn.microsoft.com/en-us/windows/win32/api/debugapi/nf-debugapi-continuedebugevent)に従う。DR6のbitの意味は[Intelのプロセッサ文書](https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developers-manual.pdf)を参照したが、実機53でWindowsが返したDR6の値は不明である。
