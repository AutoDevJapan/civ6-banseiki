# 通常起動・ロード前閉鎖のPE調査（880）

通常接続は未実装である。この有限成果は、Steam配布PEから起動候補と破棄候補を再現できるRust解析器と、未確定の接続条件である。実hook・reload・通常userpathの実測はすべて0回。ゲーム起動、DLL配置、ゲームファイル変更、プロセスメモリ操作は行っていない。324のnative/runtime/composite/paid/eventsは変更していない。

## 対象と再現

`G = C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI`。

| 一次資料 | SHA-256 / 版 |
|---|---|
| `G/Base/Binaries/Win64Steam/CivilizationVI.exe` | `e7450823cc8e00468cff7b9d7b97c63140eae38ae1d774ba4efa437556c42d63`、ImageBase `0x140000000` |
| `G/DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll` | `324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c`、ImageBase `0x180000000` |
| 324の参照作業フォルダ | `C:/Users/makhmeto/Desktop/dev/ADJ/civ6-banseiki-wt/agent-todo-324-a39639587c09`、`50f9c14565da5d7ef8a1ff2f0406cd7c8b1c426b` |
| 本workerの開始点 | `4cd8026e0db2f7f2c422dca0206b9ae2261a359c` |

以下のRVAは上記EXEまたはDLLだけに対応する。DX12のEXEには適用しない。LLVMが付ける「直前のexport名+変位」は対象関数の名前ではない。本書ではexport以外の関数をRVAで記述する。

```powershell
$env:CARGO_TARGET_DIR='D:/CC/Work/todo880/bootstrap-target'
& D:/CC/Tools/codex-jobs/cargo_gate.exe build --locked --offline --manifest-path tools/Cargo.toml -p civ6-pe-rs
$g="C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI"
$exe="$g/Base/Binaries/Win64Steam/CivilizationVI.exe"
$core="$g/DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll"
$pe='D:/CC/Work/todo880/bootstrap-target/debug/civ6-pe.exe'
& $pe --bootstrap-audit $exe $core
& $pe --bootstrap $exe DllCreateGameContext
& $pe --bootstrap $exe DllDestroyGameContext
& $pe --bootstrap $exe lua_close
& $pe --bootstrap $exe 0xf7110
llvm-readobj --coff-imports --coff-exports --coff-tls-directory $core
llvm-objdump -d --x86-asm-syntax=intel --start-address=0x1400f8510 --stop-address=0x1400f868d $exe
llvm-objdump -d --x86-asm-syntax=intel --start-address=0x14024b450 --stop-address=0x14024b5f9 $exe
llvm-objdump -d --x86-asm-syntax=intel --start-address=0x1400f7110 --stop-address=0x1400f7168 $exe
llvm-objdump -d --x86-asm-syntax=intel --start-address=0x1400f7830 --stop-address=0x1400f78d4 $exe
llvm-objdump -d --x86-asm-syntax=intel --start-address=0x1400f8e30 --stop-address=0x1400f8eb3 $exe
```

`--bootstrap`は文字列、import名の部分一致、またはRVAから参照を探す。RIP相対mov/lea、間接call/jump、絶対VA、E8/E9を出す。**バイト走査は命令境界を証明しない**。`.pdata`は関数の断片を含む。採用した位置はLLVMでも照合した。

`--bootstrap-audit`は両ファイルのSHA、分岐先、命令バイトを再検査する。成功は静的資料の一致だけを意味する。出力は常に `execution_ready=false`。入力を実行可能moduleとしてロードしたり、WinAPIの成功を模擬したりしない。

## 起動側で確定した呼出し順

| EXE RVA | 一次資料の命令と意味 |
|---|---|
| `0x24B523` → `0x0F8E30` | 既存Contextの破棄処理を呼ぶ |
| `0x24B52D` → `0x0F8510` | RCXは変換済みパス。DLL読込処理を呼ぶ |
| `0x0F8572` → `0x989970` | `LoadLibraryW`へのthunkを呼ぶ。RCXはUTF-16ファイル名、戻り値RAXはHMODULE |
| `0x989970` | `48 FF 25 D1 CF 60 00`、IAT `0xF96948` の `KERNEL32!LoadLibraryW`へjump |
| **`0x0F8577`** | `48 89 05 5A DB 1D 02`、戻ったRAXを `0x22D60D8` に保存。**選んだbootstrap挿入候補** |
| `0x0F8583` / `0x0F858D` | RDXに文字列 `DllCreateGameContext`、RCXにHMODULEを置き `GetProcAddress` thunk `0x9898F0`を呼ぶ |
| `0x0F8592` | factoryポインターを `0x22D64E8`へ保存 |
| `0x0F85BC` / `0x0F85C3` / `0x0F85C8` | `DllDestroyGameContext`を同じmoduleから解決し `0x22D6548`へ保存 |
| `0x24B545` → `0x0F8430` | loaderが成功して戻った後、Contextを取得する |
| `0x0F8451` / `0x0F845D` | factoryをRCXへ読み `call rcx`。既存Context `0x22D60C0`があればこのcallを通らない |
| `0x24B5C2` | Contextの仮想表 `+0x60`を呼ぶ。RCX=Context、RDX=別インターフェース |
| `0x24B5D5` → `0x0F7830` | RCX=保持側オブジェクト、RDX=Contextを渡して保持先を切り替える |

GameCore側のexport `DllCreateGameContext` はRVA `0x752D50`から `0x58FC60`へjumpする。calleeは入力レジスターを引数として使わず、内部heapから16 byteを確保して `0x58FB40`を呼び、ContextポインターをRAXで返す。Contextは `+0` に仮想表 `0xA101E8`、`+8` に `0x7503E0`で構築するオブジェクトを保持する。これはこの配布版の静的ABI観察であり、公開C++ヘッダーの契約ではない。

`0x0F8577`は、上記の同一スレッドのfactory生成より前で、GameCoreのLoadLibraryが戻った位置である。従って単なる「EXEがLoadLibraryをimportする」より具体的な入口になった。ただし**最初のGameplay Lua初期化より必ず前とはまだ言えない**。GameCoreのPE entrypointは `0x9B0B58`、TLS directoryは `0xAC0DA8`、callback配列 `0x9BACE0`の先頭は0だった。CRT初期化の全関数・生成threadの順序は未追跡である。別スレッドの生成や初期化を、export呼出し順だけから否定しない。

## 破棄側で確定したことと残る穴

EXEの次の4か所では、条件付きの `0x0F7110`呼出しの直後に `0x0F8E30`を呼ぶ。

| 保持解除call | Context破棄call |
|---|---|
| `0x245CEA` | `0x245CEF` |
| `0x24B51E` | `0x24B523` |
| `0x24CAB6` | `0x24CABB` |
| `0x24D1D2` | `0x24D1D7` |

`0x0F7110`のRCXは、`+0x450`にContextを保持するEXE側オブジェクト。`0x0F7123`で `0x159390`を呼び、`0x0F7139`でContextの仮想表 `+0x1A0`をEDX=0で呼ぶ。保持先を0にした後、`0x0F715E`で仮想表 `+8`へ末尾jumpする。戻り値の意味は確定していない。

**この4か所を4種類のユーザー操作に対応付けてはならない。** 別処理 `0x0F7830`にも古いContextに対する同じ処理があり、`0x0F7867`で仮想表 `+0x1A0`、`0x0F7887`で `+8`を呼ぶ。従って `0x0F7110`一か所だけを共通境界とする根拠は足りない。

Contextの固定仮想表 `0xA101E8`を読むと `+8`は `0x8972D0`（`ret 0`）、`+0x1A0`は `0x58FF70`（DLをECXにして `0x07BBF0`へjump）だった。つまり、上記の保持解除をLua破棄そのものと呼ぶのも誤りである。`0x159390`の推移、旧Luaが実際に消える順序、全利用経路の合流を確定する必要がある。

`0x0F8E30`は先に `0x29C2A0`を呼ぶ。このcalleeにも仮想destructor呼出しがある。その後 `0x0F8E60`が `DllDestroyGameContext`を呼ぶ（RCX=`0x22D60C0`のContext）。GameCore export `0x752D60` → `0x58FD10`はContext内 `+8` のオブジェクトへ仮想 `+0x10`とdestructorを呼び、Contextを解放する。**exportに閉鎖を置くだけで旧Luaより前になることは未証明**。

別に、EXEのHavok import `?lua_close@@YAXPEAUlua_State@@@Z`をIAT `0xF962F0`まで確定した。thunk `0xDBD83C`へのcallは `0x971B11`と `0x971CB7`。どちらもRCXへ保持オブジェクト `+0x10`のLua stateを渡す。直前には保持オブジェクト `+0x60`のcallbackを `+0x58`を引数として呼ぶ経路がある。これはUIを含むLua所有者の破棄点であり、Gameplay世代との一致をまだ確認していない。GameCoreは直接の `lua_close` importを持たない。

公式 `Base/Assets/UI/FrontEnd/LoadGameMenu.lua:96–112` は `LeaveGame` → `LoadGame`と旧Lua破棄の注記を持つ。一方、同ファイル `:424–440` のQuickLoadは `LoadGame`を直接呼ぶ。`MainMenu.lua:71–72,143–146`はContinue系でもLeave→Loadを使う。よってLeaveGameのLua登録関数一か所を覆うだけでは足りない。EXE内のLua登録も `LeaveGame` → `0x26C100`、`LoadGame` → `0x26C150`まで追えたが、それらから世代破棄までの全経路は未確定。

## 選んだ方式と採らない候補

実装に向けて調べる候補は **live_chainが所有する起動からresident bootstrapを導入し、EXEのGameCore loaderが戻る位置で同期して324 runtimeを設置する方式**の1つに絞る。挿入候補は `0x0F8577`。単一経路での候補であり、全ロード経路に共通か、最初のGameplay Luaより前かは未確認。通常のSteamクリックへ自動接続できたという意味ではない。

| 候補 | 一次資料に基づく判断 |
|---|---|
| `CREATE_SUSPENDED`だけでremote LoadLibrary | 主threadだけが停止する。loader初期化完了やDLLロード完了は保証しない。これだけではGameCore待ちと設置順を解決しない |
| `DEBUG_ONLY_THIS_PROCESS`のDLL-load event中に設置 | debug event中は対象の全threadが停止する。remote thread終了待ちは進行しない。イベントを継続する前の同期installとして使わない |
| `LdrDllNotification`内でruntimeを呼ぶ | 他モジュールの関数を呼べない。runtimeはmodule pin、heap、mutexなどを使うため不適合。通知からworkerへ渡すだけではGameplayより先の保証がない |
| `DllMain`、一般sideload/config変更 | loader lock下の設置やworker待機にはdeadlockの問題がある。配置・ユーザー通常経路も未検証。本workerは配置しない |
| **EXE loaderの復帰直後** | **調査候補として選択**。単一経路でGameCoreロード完了→export取得→factory呼出しの同一thread上の順序がある。全ロード共通・初回Gameplay前・resident導入は未証明 |

Microsoft一次資料：[Process Creation Flags](https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags)、[Debugging Events](https://learn.microsoft.com/en-us/windows/win32/debug/debugging-events)、[LdrDllNotification](https://learn.microsoft.com/en-us/windows/win32/devnotes/ldrdllnotification)、[DLL Best Practices](https://learn.microsoft.com/en-us/windows/win32/dlls/dynamic-link-library-best-practices)。これらはWindowsの制約であり、Civ6のthread順序を証明するものではない。

初回起動の制御候補はdebug creationでEXE側の境界を捕捉する方法だが、eventの処理中にinstallの完了を待たない。loaderから戻ったthreadを安全な継続点に保持し、debug eventをpumpしてresident設置を完了してからfactoryへ戻す実装には、例外・thread順序の実証が要る。ABI未確定のコード移植やメモリ書換えを先に追加しない。

## 324 runtimeへの接続で不足する最小条件

1. **初回接続**：`0x0F8577`へ到達する前にresidentを導入し、loader lockを持たずに `BridgeRuntimeInstall`（実在名。`RustBridgeRuntimeInstall`ではない）を呼べる所有起動経路。PID・作成FILETIME・同一DLL hashは既存controllerの契約を引き継ぐ。現在のdriverは `steam://launch/289070/option2`経由で、作成前のthreadを所有していない。
2. **世代境界**：Load/Continue/QuickLoad/LeaveGameの全経路を、旧Gameplay Luaと本体オブジェクトが生きている時点で閉鎖する共通native処理へ対応付ける。上記2か所の保持変更と、Lua破棄callbackの関係が最小の未確定部分。引数レジスターの観察と所有threadの証拠が必要。
3. **再武装と公開の区別**：324の `BridgeRuntimeClose` はproductを非活性化してSTATEを消す。`reset_owner` は活性session以外を拒否する。`BridgeRuntimeRebuild` は再度closeしてからactivateするため、**新Gameplay COMMITの後にRebuildを呼ぶと、そのCOMMITを消す**。既存APIのままなら、破棄完了後かつ新Gameplay RESETより前にRebuildで再武装し、許可の公開は既存単一CASのCOMMIT成功に任せる必要がある。この「新Lua前」のnative境界も未確定。324側を本workerで書き換えない。

観測の秒数は設置後の有限trace用であり、製品leaseの寿命ではない。residentはゲーム終了まで残す。同期pipeline、paid receipt、公式AI支払の問題は別の未完了条件である。

## 次に必要な実機receiptは1件

親の予約と統合後、**所有する1ゲームプロセスの起動から各ロード経路までを記録する、通しの順序receipt**を取る。記録にはPID/FILETIME、EXE/GameCore/bridge hash、thread IDと連番、loader-return、install完了、最初のGameplay RESET/COMMIT、旧世代close、対象旧Lua破棄、新Lua RESET、COMMIT公開を含める。Load/Continue/QuickLoad/LeaveGameを操作名と呼出しスタックで対応付け、古い世代の許可が新世代へ残らないことを確認する。未知のhardware breakpointを再利用しない。これは本workerで実行する予約ではなく、親が次に集める1件の受入証拠である。

現時点のRust成果は解析・候補照合まで。resident activation、実hook、runtimeの自動接続は未実装。これを通常起動の完成や実機合格に数えない。

## この有限成果の検査

共有 `D:/CC/Tools/codex-jobs/cargo_gate.exe` と専用targetで `cargo test --locked --offline --manifest-path tools/Cargo.toml -p civ6-pe-rs` を実行し4件成功。実配布PEに対するauditも成功し、EXEとDLLを逆に渡す試験はSHA不一致で終了コード1になった。Rustの形式検査と差分の空白検査を実施。Luaや324のruntimeは編集しておらず、全workspace・Lua全件の検査は今回実行していない。

Clippyの `--all-targets -- -D warnings` は先行検査で成功した。その後の走査末尾6 byte対応と表示修正は再ビルド・4テスト・実PE照合を通した。Clippyの重複再検査は共有枠待ちで終了し、最終差分に対するClippy成功とは数えない。

別モデル `gpt-6-luna` の読み取り専用レビューはMustなし。「選択」を実装確定と誤読される点のSuggestionを反映した。実機懸念は上記の不足条件として残す。

## 追加のcapture-only候補

`civ6-order-observer`をfeature `bootstrap-order`の明示指定時だけビルドする。実装は `tools/civ6-pe-rs/src/order_capture.rs`、`order_windows.rs`、`bin/civ6-order-observer.rs`。既存native-probe側はCargoのfeature/bin/dependency宣言とmodule入口だけを追加し、同じソースのDebugger、attach識別、thread HANDLE、CleanupGateを再利用する。既存governor/propertyの実機hardware禁止を解除していない。324 runtimeは変更していない。旧probe一式のcloneは0。

```powershell
$env:CARGO_TARGET_DIR='D:/CC/Work/todo880/bootstrap-target'
& D:/CC/Tools/codex-jobs/cargo_gate.exe build --locked --offline --manifest-path tools/Cargo.toml -p civ6-native-probe-rs --features bootstrap-order --bin civ6-order-observer
& D:/CC/Work/todo880/bootstrap-target/debug/civ6-order-observer.exe snapshot $exe
```

snapshotはPEをファイルとして読むだけ。SHA・opcode・実行sectionを確認し、ソースファイル一覧と各SHA、コンパイル時source digest、生成時checkout HEADをJSONに出す。コンパイル後にソースが変わっていれば拒否する。`live_runs=0`、未観測counterはnull、共通境界とGameplay同定はunknown。checkout HEADとcompiled source digestを分け、未コミット編集をHEADだけで同一と扱わない。

| phase | slot 0 | slot 1 | slot 2 | slot 3 |
|---|---|---|---|---|
| `front` | loader復帰 `0xF8577` | factory call `0xF845D` | factory復帰 `0xF845F` | lua_newstate復帰 `0x972BBD` |
| `load` | 保持解除 `0xF7110` | 保持変更 `0xF7830` | Lua close call `0x971B11` | Lua close call `0x971CB7` |

新しいLua生成位置の根拠は、EXEの `lua_newstate` import IAT `0xF962B8` → thunk `0xDBD848`、直接call `0x972BB8`の `E8 8B AC 44 00`、復帰 `0x972BBD`の `44 8B C6`。RAXは返されたLua state、RBXは次の `0x972BC0`でstateを `+0x10`へ格納する所有オブジェクト。UI Luaを含むためGameplayだと断定しない。初期画面ですでにGameCoreロード済みでもfuture factory/newstateは捕捉できるが、attach以前の初回factoryはunknownのまま残す。

### 親から渡すmanifestと実行契約

`validate <request.json>`は構造だけを検査し、プロセス・lock・ゲームを開かない。`parent <request.json>`は現在のlive_chain子孫である親からのみ起動する。controllerのPID/FILETIME欄は親stub自身の値で埋め直し、出力と同じ場所へ新規 `*.request.json`を保存する。直接 `observe` を起動する場合は実際のOS親controllerの値を渡す。

必須キーは `schema`=`todo880-order-v1`、`pid`、`created_filetime`、`chain_pid`、`chain_created_filetime`、`controller_pid`、`controller_created_filetime`、`source_dll`、`source_dll_sha256`、`output`、`max_seconds`、`max_hits`、`front_prefix`、`load_prefix`、`phase`。未知のキーを拒否する。PID/FILETIMEは正の整数、各prefixは1–4、秒数1–300、hit数1–256。source DLLは絶対パスと64桁SHAを照合して書換え・削除を防ぐ読み取りハンドルを保持する。DLLをロードもinstallもしない。

親は標準入力pipeを開いたまま保持する。stdoutの `ready=true`後に対象操作を開始する。`phase load\n` / `phase front\n`を送った場合、`phase_ready`を受け取ってから次の操作へ進む。最大8回。切替はpending instructionがなく、全追跡threadを停止している時だけ行い、切替の欠測期間を `phase_gap`に記録する。上限は固定集合のprefix選択であり、任意address・opcodeは受け付けない。`stop\n`またはEOFは復元を要求する。現在のcatalog予約とは同時実行しない。このworkerはobserve/parentを実行していない。

`order_identity`にPID/FILETIME、source DLL SHA、EXE base、初期Context値とGameCore baseを出す。`instruction_before`は元のRAX/RBX/RCX/RDX/R8/R9/RSP/RIP、thread ID、pointer candidate、単調増加sequence/capture_seqを記録する。保持オブジェクトの `+0x450`を読むが、owner IDやGameplay stateと断定しない。attach時のロード済みmodule列挙は `module_attach_snapshot`、ready後の実ロード/アンロードは別のeventとして記録する。

### 元命令と例外の扱い

breakpointはDR0–3だけに設定する。固定SHAと命令窓に加え、対象pageがMEM_IMAGE・同じEXEのAllocationBase・実行可能・非GUARDであることを照合する。`WriteProcessMemory`、注入、RIP飛越、NOP化、元命令のemulationは行わない。

最初の停止は例外address=RIP=設定DR、DR7の実行breakpoint設定、対応DR6 B-bitを要求する。不明thread、先行breakpoint、TF、DRの不一致は拒否する。欠落DR6を推測でowned扱いしない。既存probeがDR6欠落を許容する経路は、この新しい実機未検証候補には適用しない。

そのthreadへRFとTFを設定し、元命令をCPUに実行させる。次のfirst-chance single-stepでDR6.BSと静的な次のRIPを照合できた時だけ `original_instruction_executions=1`を出す。factory callの次RIPは固定GameCoreのexport `base+0x752D50`。Lua close callの次RIPはEXEのthunk `base+0xDBD83C`。**この1はcall命令1回を意味し、calleeの完了ではない。** factoryのcallee完了は別のfactory-return siteで観測する。Lua close完了はこの8点では証明しない。

無関係な例外は `DBG_EXCEPTION_NOT_HANDLED`。ownedか曖昧な例外は正常detachへ進まず、既存critical cleanupに従って所有ゲームの終了を親へ要求する。正常終了では全追跡threadのDRを保存値へ戻し、照合してからdetachし、追加した停止回数を戻す。pending TFも除去する。observerのreader threadはjoinし、親stubはChild終了と復元済み `order_stop`の両方を確認する。復元済み記録のない子の終了では、保持中の同じゲームHANDLEの終了確認まで回収を続ける。ゲーム終了を確認できない失敗は成功receiptにせず、ハンドルを保持して通知する。観測に失敗しても復元とdetachを確認できた場合は、失敗を返してゲームを残せる。

この順序は [Microsoft Debugging Events](https://learn.microsoft.com/en-us/windows/win32/debug/debugging-events)、[SetThreadContext](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setthreadcontext)、[ContinueDebugEvent](https://learn.microsoft.com/en-us/windows/win32/api/debugapi/nf-debugapi-continuedebugevent) の停止・継続契約に基づく。TF/RFの実配送と全thread復元は実機未検証。実測なしで将来の例外配送成功を宣言しない。

Microsoftの [x86 Flags](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/x86-architecture#x86-flags) はTFによる1命令後のSTATUS_SINGLE_STEPを説明し、[x64 Architecture](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/x64-architecture) はx86由来のflagsを示す。ただしWindowsが本対象のCONTEXTでTF/DR6をどう配送するかは未実測。固定条件に合わなければ拒否し、欠落bitを推測で補って反復しない。命令完了はDR6の原因bitがBSのみであることを要求し、混在を消費しない。phase切替では停止後の保留debug eventを旧phaseで処理してから再武装する。

### 親が取る3種類の順序receipt候補

1. 既知の初期画面でfrontを準備してから新規/Continueへ進む。初期module/context値、最初に捕捉したfactory、Lua生成を照合する。すでに完了していたloader/factoryは欠測。
2. 既知のゲーム中状態でloadの準備完了後にLoad/QuickLoadを行う。保持変更と各Lua closeのthread・pointer・sequenceを記録する。
3. 同じ所有セッションでLeaveGameを行い、必要なら初期画面でfrontへ戻して次のfactoryを取る。全操作の合流点・Gameplay同定は実データから判定し、phase外の呼出しを0件とはみなさない。

これらは親の予約内で行う候補であり、workerによる起動・予約追加は0。product installやCOMMIT公開の受入とは別である。

構造確認専用のmanifestは `tools/civ6-pe-rs/tests/fixtures/order-request-no-live.json`。PID・FILETIME・DLL SHAは明示的なテスト値であり、`validate`以外に渡さない。実機identityの成功を模擬する用途ではない。

追加候補の別モデルレビューでは、親自身のchain確認、子の異常終了時の回収、DR6混在原因、phase切替時の旧イベント競合を修正した。最後の読み取り専用再レビューはMustなし。既存critical cleanupはstdout通知を持つため、「通知がない」という初回指摘は撤回された。実機のthread/例外配送は未検証のままである。

追加候補の最終検査：共有cargo_gate・専用targetで `cargo test --locked --offline --manifest-path tools/Cargo.toml -p civ6-native-probe-rs --features bootstrap-order --lib --test order-capture-contract order_capture` を実行し、純粋な否定テスト4件と実CLI/ディスク照合3件が成功。生成済みの同じlib test実行ファイルで `windows::tests::live_hardware_modes_are_rejected_before_attach_or_process_access --exact` も1件成功した。合計8件。いずれもゲーム起動・debug attachは0。request fixtureはvalidateだけに渡し、`manifest_valid=true, process_identity_verified=false`を確認した。形式・差分検査も成功。capture追加差分のClippyおよびworkspace全件は未実行。

commit後の成果物は `D:/CC/Work/todo880/order-observer-snapshot.json`（全8sites、source一覧、生成時HEAD）、`D:/CC/Work/todo880/order-observer-summary.json`（短い同一性結果）、`D:/CC/Work/todo880/order-observer-manifest-validation.json`（構造確認）。いずれもRust CLIの出力をそのまま保存し、runtime counterを作らない。親stubの実際の起動・子回収・例外配送の検証は親の実機receiptに残る。
