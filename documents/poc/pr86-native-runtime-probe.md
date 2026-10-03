# PR86: 本体任命資格の実行時観測器

本体資格を変更せず、`CanEver` と `CanAppoint` 相当の入口を通る実呼出しを記録する。**Civ6実機への接続は未実行**。現時点の合格は専用Rust子プロセスのdebugger試験だけであり、加入資格の修正やAIの任命成立を意味しない。

## 固定した対象と境界

- 観測器: `tools/civ6-native-probe-rs`。Windows x86-64でのみ実行できる読取専用CLI。Linuxでは実行不可を明示し、呼出し元の分類など共通処理だけを検査する。
- 対象: 標準 `live_chain` の共有ロック `D:/CC/Tools/civ6-live/civ6.lock` が示す生存中のchain PID。chain実行ファイルは同ディレクトリの`live_chain.exe`と完全パスで照合する。controllerのPIDと作成時刻に加えてOS上の実親PIDを照合し、ゲームのPID・作成時刻・完全パスを固定して再照合する。
- GameCore: `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll` が対象プロセスへロード済みで、SHA-256が `324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c` の場合だけattachする。ASLR後のロードbaseへRVAを足す。入口の16バイトをDLLファイルと比較し、既存のコード改変がある場合は拒否する。
- 関数入口: `CanEver`候補RVA `0x2DDDD0`、`CanAppoint`候補RVA `0x2DD820`。名前とWin64 ABIは静的解析からの候補である。実機でのRCX=this、EDX=総督Index、AL=真偽の確認が残る。
- 操作: `DebugActiveProcess`、`DebugSetProcessKillOnExit(FALSE)`、threadごとのハードウェア実行breakpointを使う。DLLファイル・ゲームの命令・人物台帳・本体会計は書かない。各threadの元debug registerを保存し、停止・期限切れ・guard失敗時にthreadを停止する。生存threadのdebug registerを再読取して元値と一致した場合だけdetachし、停止したthreadを再開する。復元時は現在のRIP・RSP・EFlags・一般registerを古いsnapshotで上書きしない。元からdebug breakpointが使用中なら拒否する。
- メモリ読取は指定DLL入口16バイト、指定DLL内の呼出し命令5バイト、入口時stackのreturn address 8バイトに限る。`this`先のfieldやowner IDは読まない。Steam認証や環境変数などの無関係なメモリは読まない。
- 観測は最大300秒・20,000 JSONL行・共有lease30分内。controllerの終了、ロック所有者の変更、対象DLLのunloadで停止する。

## JSONLの読み方

`identity`にゲーム実行ファイル、作成時刻、ロードbase、GameCore SHAを記録する。`ready`の後に`entry`と`return`をIDで対応付け、`stop`にdetach結果を記録する。`entry`にはthread ID、関数RVA、`this`の生ポインタ、32ビットIndex、stack由来の`return_site_rva`を別項目として保存する。直前の5バイトが対象への直接`E8`である場合だけ`caller_rva`を設定する。`0x2DD861`からの末尾分岐は新しいreturn addressを作らないため、return addressとstack位置の両方が同じ`CanAppoint`入口だけを`tail_from_can_appoint`で補助分類し、架空の直接callerを作らない。`return`にはALの生値と、0/1だった場合だけの真偽候補を保存する。戻り値を観測できない呼出しは`entry`の`return_breakpoint_armed=false`または対応する`return`欠落で分かる。ポインタからowner IDを確定しない。

`path_candidate`は既存の逆アセンブルで得た呼出し箇所との照合名であり、AI実手番の文脈を単独で証明しない。主な候補はUI getter `0xBA51E`、公式要求 `0x30DA56`と`0x2DCFFE`、AI解析領域 `0x4A8114`、未分類 `0x21B9E4`。親のシナリオがAI実手番・公式要求・UI getterを別の区間で起動し、同じ本体関数の実呼出しと戻り値を突き合わせる。

## live_chainからの接続

Rust runnerのシナリオに次を追加できる。observer実行時には`CIV6_LIVE_CHAIN_PID`と`CIV6_LIVE_LOCK_PATH`が必須で、共有ロックの一致がなければ開始しない。`civ6-native-probe.exe`はrunner実行ファイルと同じディレクトリに配置する。開始時はJSONのready応答を最大20秒待ち、停止時はobserverの終了を待って`<name>.jsonl`を回収する。runnerのエラー時とDropでも停止を試みる。`stop`とDropは同じ20秒期限を共有し、終了要求と重大エラーへのゲーム終了要求を重複しない。observer自身もcontrollerの生存を監視する。

復元・detach・thread再開でOSが失敗した場合は、6秒以内で再試行する。復元値を確認できない間は通常detachへ進まない。期限後は`critical_cleanup`をJSONLとcontrollerの標準出力へ通知し、observerはdebuggerに留まって終了イベントを処理する。runnerは同じPID・作成時刻のゲームだけを既存の終了関数で閉じ、試験を失敗として記録する。ゲーム終了の確認を最大12秒待つ。`DebugSetProcessKillOnExit(FALSE)`の初回失敗も、detachを確認するか終了時のkillを無効にできるまで再試行し、どちらもできない場合は同じ重大エラー経路に入る。ゲーム終了まで確認できないOS障害は`critical_cleanup unresolved`で報告し、安全な正常停止とは扱わない。

```json
{"action":"start_native_governor_trace","name":"pr86-ai-turn","max_seconds":300}
{"action":"stop_native_governor_trace"}
```

runnerは開始と停止の対、名前、300秒上限、観測中のゲーム再起動禁止を検証する。親の実機シナリオでは、静的gateの484組、21AIの最初の実手番、正式Join前後の公式要求とUI getterの区間を分けて使う。observerが存在するだけでは実Civ6を起動しない。

## 独立検査と限界

- Windowsの専用Rust子プロセスで、入口のRCX/EDX、ALのtrue/false、attach後の新thread、正常stop、期限切れ、guard失敗、記録上限到達時の例外処理を検査した。各停止後に子プロセスが同じ関数を実行でき、debug registerがattach前と同値であることを子プロセス側から確かめた。`SetThreadContext`拒否を注入した試験では再試行後の復元と、永続拒否時の重大エラー通知からcontrollerが専用子プロセスを終了する経路を区別した。実Civ6では未検査。
- runnerのRust検査で開始・停止のdispatch、名前と時間の上限、未対の開始・停止と観測中の再起動の拒否を確認した。試験用observerが停止しない場合も、`stop`とDropの合計待機が単一の期限を超えず、重大エラーが重複しても終了要求が1回であることを確認した。実ゲームのready handshakeと30分leaseは未検査。
- このworktree専用の`CARGO_TARGET_DIR`で`cargo test --locked --manifest-path tools/Cargo.toml --workspace --all-targets`が成功した。Lua 5.1の検査を含み、表示された検査は省略0件だった。最後のprocess-handle権限とDLL debug event handleの修正後に、観測器単体の`--all-targets`も再実行して成功した。
- `tools/civ6-pe-rs`は別worktreeの読取専用PE調査器で、今回のworktreeにはない。今回のツールは既知の2入口のファイルオフセットを固定してバイト比較するだけで、PE parserを複製しない。別ビルドやRVA追加に対応する段階で、既存PE parserの必要部分を共通libraryに移し、この観測器から参照する。
- 静的gateの実機43では484組が偽、21AIは称号各3・使用0・任命0、正式Join後の公式APPOINT1回も同期barrier後に本体任命なし・使用0だった。この観測器はその原因を切り分ける測定器であり、本体の動的資格機能は未実装のまま。

前提と元の静的解析は、親担当のworktreeにある`documents/poc/pr86-native-admission.md`と[枠診断](native-slots-diagnostic.md)に記録されている。SSOT 4・9・16の挙動は変更していない。

Windows APIの根拠はMicrosoftの[稼働中プロセスへのattachとdetach](https://learn.microsoft.com/en-us/windows/win32/debug/debugging-a-running-process)、[終了時のdetach設定](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-debugsetprocesskillonexit)、[debug eventで受け取るhandleの扱い](https://learn.microsoft.com/en-us/windows/win32/api/debugapi/nf-debugapi-waitfordebugeventex)に従う。
