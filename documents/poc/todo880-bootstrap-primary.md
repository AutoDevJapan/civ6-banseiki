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

### 実機07後の4候補再点検とstop時の記録

親の実機07（actual `d21`、observer source `d8591f10…1fb0`）は7/7手順と6判定が成功した。Binkの6語通知はfirst-chanceでDBG_CONTINUEとなり、second-chanceは0。原コピー `D:/CC/Work/todo880/order-fixed-07-artifacts` の13ファイルを `order-fixed-07-hashes.json` と再照合した。ただしloadの4候補は全hit0で、製品Close/Rebuild境界は未証明である。

固定SHAのEXEとGameCoreをLLVMで再読した。候補は次の範囲に限られる。

| 候補（全てEXE RVA） | 確認できた対象 | 証明できないこと |
|---|---|---|
| `0xF7110` / `0xF7830` | EXEの保持オブジェクト+0x450からContextの仮想+0x1A0・+8を呼ぶ。GameCore factoryの固定仮想表0xA101E8では、それぞれCore0x58FF70→0x07BBF0とCore0x8972D0（ret） | Gameplay Luaや324 runtimeが参照するnative objectの破棄そのもの |
| `0x971B11` / `0x971CB7` | EXEの所有者+0x10をRCXとしてthunk0xDBD83C→Havok `lua_close` IAT0xF962F0へ渡す | RCXがGameplay stateであること。UIを含むEXE側Lua経路との区別 |

GameCoreの直接importに `lua_close` / `lua_newstate` はない。これは間接呼出しの不在を意味しない。EXE0xF8E30→CoreのDllDestroyGameContext（0x58FD10）には内部オブジェクトの仮想+0x10とdestructorがあるが、その前のEXE0x29C2A0にも破棄callがある。現4候補を「Gameplay破棄4点」と呼ばず、export閉鎖を旧Luaより前と確定しない。再点検の命令・仮想表は `bootstrap-live07-pe-recheck.txt` / `.json` に記録した。

07のrawは命名通知threadのDR一致を示すが、stop直前の保持済み116thread全体は記録していなかった。そこで既存cleanupのfreeze直後、TF解除・DR復元の前に `pre_restore_registers` を1行追加する。保持済みhandleごとに所有PID/TID、wait、exit、held、現在DR0〜3/6/7、期待site、世代、pendingを記録する。既存の比較条件は変更せず、不一致・終了済み・読取不明・pendingを分ける。記録I/O失敗でも復元を先に完了させ、出力失敗を結果へ残す。read-only記録は再armせず、ゲームへの新しい書込を追加しない。

この採取は[GetThreadContextの停止thread条件](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getthreadcontext)に従う。freeze失敗時はDR一致を判定しない。全件一致もstopの瞬間だけの証拠で、期間中の一時的消失・既に退出したthread・未処理CREATEイベントを覆わない。`coverage_over_interval=unknown` と `zero_hits_proves_unreached=false` を常に保持する。既存解析器はこの診断行を命令完了として数えない。

所有OS子の既存lifecycle検査を共用し、停止した所有threadのDRを実際に0へ消去する否定ケースを追加した。実読取で不一致1件を検出し、採取後も0のまま（再armなし）、既存cleanupで原DRへ復元・子回収する。これは故障注入であり、Civ6のDR消失原因を再現したとの主張ではない。

次の1シナリオは同じ `tools/civ6-pe-rs/tests/fixtures/order-fixed-load-leave.json` の7手順を使い、source固定したhelperだけ更新する。新しいsiteやphase切替は足さない。不一致ならstop時の観測装置の欠測を実証したものとして保持し、自動再armやhit0からの未到達判定をしない。全一致かつhit0でも「未到達確定」とせず、この4点の再試行を繰り返さずCore側native owner破棄への静的call連鎖同定へ戻る。hitが得られた場合だけ、そのRCX・時系列を324の旧native objectと照合する。通常Install/Close/Rebuildの製品接続は引き続き未達である。

### 実機04のdumpで確認した6語通知

追加一次証拠 `D:/CC/Work/todo880/order-fixed-04-windows-events.json` のApplication1000/1001はPID53272・開始FILETIME `0x1DD55D24B88AD90` と同runへ一致する。原dump `C:/Users/makhmeto/AppData/Local/CrashDumps/CivilizationVI.exe.53272.dmp` は85,471,399byte、SHA-256 `8588bdc1fc4146dea482e3a1340bc4c026913b8f524defb3d60302f78430b1a4`。SDK minidumpapiset.hのExceptionStream/ModuleList/MemoryList定義に従い読み取り、前後SHA不変を確認した。ExceptionStreamはTID29092、code0x406d1388、flags0x80、KernelBase+0xC41CA、NumberParameters=6である。

ExceptionInformationは `[0x1000,0x7ffae1e09b28,0x71a4,0,0x7ffae1e204a0,0x7ffae1df5bfc]`。先頭3語のtype・名前pointer・対象TID29092・payload flags0は正規THREADNAME_INFOに一致する。名前pointerはdump内のBink module+0x49B28に属するが、そのページはdumpに含まれない。同timestamp・SizeOfImageの実ファイル `Base/Binaries/Win64Steam/bink2w64.dll`（SHA-256 `8c72bcb0a08f722a15afca6b0e3395e278b10e0b318b696182d5625a4375ee93`）の同RVAは `Bink IO\0` である。文字列は実ファイル由来であり、dumpからの復元と偽らない。

同BinkのRVA0x35AE0は24byteの構造体をstackへ作る。0x35B98はR8D=6、0x35BA4はRaiseExceptionを呼ぶ。先頭24byteに対し48byteを引数として渡す実装で、後半3語は構造体外の値である。これにより旧3語限定候補a27fffeeがこの実通知を拒否することが判明した。新候補は3語または確認済み6語だけを認識し、後半3語を未解釈のrawとして保持する。先頭field・終端付き名前の実メモリ読取・所有thread・pending条件は変更しない。4/5語などを名前通知として推測しない。

所有Rust子は3語・6語の両方で、旧first-chance透過→second-chanceと、修正first-chance継続→second-chanceなしを比較する。6語fixtureは配列全体を初期化し、実Binkの構造体外読み込みをテスト側で再現しない。04のownedゲームが命名例外でAPPCRASHしたことは確定したが、2プロセス目の正体は未確定、4load site hit0は境界不在の証明ではない。dump解析結果は `D:/CC/Work/todo880/bootstrap-live04-minidump-proof.json` に記録する。

固定load実機04は5/7手順まで成功し、LeaveGame中に2プロセス検出で失敗した。原本は `D:/CC/Work/todo880/order-fixed-04-artifacts`。sequence120/121は同TID29092、code `0x406D1388` のfirst/second chanceで、旧observerは両方を透過した。4つのload siteはhit0、observerの復元・detach・回収は成功している。当初のobserver rawにはExceptionInformationがなく、payloadは未確定だった。追加dumpによる命名例外のAPPCRASH確認は上記のとおり。2プロセス目の正体と破棄境界は引き続き未確定である。

Microsoftの[thread名通知](https://learn.microsoft.com/en-us/visualstudio/debugger/tips-for-debugging-threads?view=vs-2022)はpack8のTHREADNAME_INFOをRaiseExceptionへ渡す。固定EXE SHA `e7450823…d63` をLLVMで読むとRVA `0x98BF30`、`0xC896E0`、`0xF63060`にtype0x1000・名前pointer・thread ID・flags0を組み立て、3 ULONG_PTRで同コードをRaiseException IAT `0xF96788`へ送る命令がある。DWORD typeの上位は構造体paddingなので0を要求しない。これはゲームPEに実在する通知経路の証拠であり、04の例外callsiteの同定ではない。

observerは3引数のWin64形式または上記の6引数形式の先頭3語、type0x1000、payload flags0、256byte以内に終端する読み取り可能な名前、送信元と対象の保持済み同所有process/thread・生存を照合した場合だけDBG_CONTINUEとする。対象IDのDWORD -1は送信threadを表す。未知・不正なpayloadは透過し、TIDから不明handleを再取得しない。ExceptionFlagsは0またはSDK winnt.hの `EXCEPTION_SOFTWARE_ORIGINATE=0x80` のみ許す。実OS子ではRaiseExceptionへflags0を渡しても受信レコードに0x80が付いた。[EXCEPTION_RECORD](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-exception_record)が区別する非継続bit等は許可しない。同じthreadにpending命令がある場合は完了を作らず停止し、別threadのpendingは変更しない。

例外のraw記録にExceptionFlags、NumberParameters、ExceptionInformation（最大15語）、名前のbyte列と対象TID、拒否理由を追加した。継続statusを記録I/Oより前に決定し、通知受理後にdisk書込が失敗しても、そのstatusを既存cleanupへ明示して維持する。`order_capture_thread_name_real_second_chance_and_fixed_continue` は既存所有Rust子に同じRaiseExceptionを実行させ、旧first-chance透過→second-chanceを観測後に救済する対照と、修正first-chance継続を比較する。不正payload・pending台帳の検査は実イベントの否定用変形で、ゲームがその値を送った証拠にはしない。

実機03（`D:/CC/Work/todo880/order-phase-03-artifacts`、actual `e5d0f16a`、observer digest `4042d7d53608461316402809f0325b75bdd13a4bb69829fbe8e948c7d0b69171`）は、保存loadとGameplay読取まで成功した後、phase切替で失敗した。TID41456はPID65240の生存handle（wait258、exit259）でheld=true、generation0、pending0だがDR0〜3/6/7が全0だった。終了thread仮説ではこの値を説明できない。結果はerror、復元・detach・子回収は成功として別に保持し、phase問題の解決やrun成功へ読み替えない。

次の有限観測は `tools/civ6-pe-rs/tests/fixtures/order-fixed-load-leave.json` を使う。7手順はrestart→同じ保存load→Gameplay読取→load固定のobserver開始→退出前の読取印→LeaveGame→stop。phase切替は0回、install/Join/支払い/保存の再送は0回。専用DLLは旧記録のc11255eb…c775を明示し、共有DLLを置換しない。親の既存runner/helperで実行でき、bootstrap workerは予約しない。

load4siteの実hitとcompletion、RCXの旧context/Lua候補、TID・sequenceを退出操作の時間帯へ対応付ける。正常cleanupでもhit0を境界証明にしない。固定phaseでは観測外threadのDR保持全数を証明できないため、無hit・一部hitを「該当callなし」と断定しない。取得できた順序はLeaveGameの当該世代だけの証拠で、初回startup、Load/Continue/Quickload共通境界やGameplay Lua同定を証明しない。退出siteを得た場合だけresident Close候補を絞り、Rebuildは旧破棄後・新RESET前、許可公開は自然COMMITとする。loader/factory側の常時接続は別の未達条件として維持する。

`order_capture_fixed_thread_lifecycle_retains_debug_registers` は既存の所有Rust子と本番thread登録を使い、CREATE_THREAD直後のreadback、新規threadの実関数入口、停止したidle時のDRを記録する。phase変更・レジスタ改変注入・EXIT遅延注入はしない。これはCiv6を模擬成功させる検査ではなく、Windowsによる一律初期化リセット仮説を限定的に比較する検査である。

公式[CREATE_THREAD_DEBUG_INFO](https://learn.microsoft.com/en-us/windows/win32/api/minwinbase/ns-minwinbase-create_thread_debug_info)は開始addressが近似であり得ることを記す。[SetThreadContext](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setthreadcontext)はOS管理bitの補正を記すが、後続初期化によるDR0〜3の一律消去を保証していない。全0を自分の設定とみなす比較緩和や自動再armは追加しない。

`civ6-order-observer`をfeature `bootstrap-order`の明示指定時だけビルドする。実装は `tools/civ6-pe-rs/src/order_capture.rs`、`order_windows.rs`、`bin/civ6-order-observer.rs`。既存native-probe側はCargoのfeature/bin/dependency宣言とmodule入口だけを追加し、同じソースのDebugger、attach識別、thread HANDLE、CleanupGateを再利用する。既存governor/propertyの実機hardware禁止を解除していない。324 runtimeは変更していない。旧probe一式のcloneは0。

```powershell
$env:CARGO_TARGET_DIR='D:/CC/Work/todo880/bootstrap-target'
& D:/CC/Tools/codex-jobs/cargo_gate.exe build --locked --offline --manifest-path tools/Cargo.toml -p civ6-native-probe-rs --features bootstrap-order --bin civ6-order-observer
& D:/CC/Work/todo880/bootstrap-target/debug/civ6-order-observer.exe snapshot $exe
```

snapshotはPEをファイルとして読むだけ。SHA・opcode・実行sectionを確認し、ソースファイル一覧と各SHA、コンパイル時source digest、生成時checkout HEADをJSONに出す。コンパイル後にソースが変わっていれば拒否する。`live_runs=0`、未観測counterはnull、共通境界とGameplay同定はunknown。checkout HEADとcompiled source digestを分け、未コミット編集をHEADだけで同一と扱わない。

`order-capture-contract` の既定Cargo検査3件は所有tempだけを使う。不正byte列を既存`disk_sites`と実CLIのsnapshot/summaryへ渡す拒否検査、manifestの構造検査、無効manifest・引数・未存在入力のCLI拒否を実行する。Civ6未導入時にreturn成功やignoreへ逃がす分岐はない。実物EXE/GameCoreの肯定照合は上記の明示 `snapshot <Steam EXE>`／`summary <Steam EXE>` が担い、Cargo件数へ加算しない。未導入・SHA不一致・source不一致はこの明示実行の失敗である。

実PCではsnapshotの終了0、8 sites、`checkout_sources_match_binary=true`、`gamecore_identity_verified=true`、`live_runs=0`、各siteの未観測counter=nullを確認し、JSONと実行EXEのSHAを別receiptへ保存する。既存 `D:/CC/Work/todo880/bootstrap-phase-8ac3b847-snapshot.json` と `bootstrap-phase-8ac3b847-results.json` はその候補の実物照合として保持し、新CI候補の合格へ読み替えない。snapshotは起動・attachを行わない。

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

## review19749944の終了プロトコル修正

`order_protocol.rs`にChildGuardを追加し、入力・出力失敗、panic、通常STOPを同じ終了トランザクションへ集約した。STOPは一度だけ送り、復元済みreceiptとChild終了を待つ。最初の終了要求から絶対20秒を超えて期限を作り直さない。ready待ちは起動8秒、ready失敗の回収も起動から20秒以内。通常capture時間はmanifestの上限内であり、終了要求後の20秒とは分ける。復元が確認できない場合だけ同じ保持HANDLEによる対象終了へ進む。閉鎖要求のackも一度だけ送る。期限時に回収できなければ `cleanup_confirmed=false` を返し、成功とは表示しない。

親のstdoutはすべて `io::Result` で受け、BrokenPipeをpanicにしない。panicが起きてもcatchとRAIIが同じ期限・同じChildハンドルでSTOP／復元／回収を行う。stdout読取threadは廃止し、OS pipeを有界で読む。入力readerは終了時にjoinする。

debug exceptionの所有判定・継続状態保存を最初のログ書込より前に行う。ディスクへの書込が失敗してもowned例外はDBG_CONTINUEを保つ。復元済み `order_stop` はtraceへの書込失敗と独立して親pipeへ送る。証拠の保存失敗は明示し、復元失敗と同一視しない。

失敗テストは自分が起動したRust helperを使う。unknown/空行/reader InvalidData/実OS BrokenPipe/panicは同じChildGuardでSTOPと資源削除・waitを検査する。helperの復元は `helper_resource_restored`、Civ6の復元は `native_debug_restored=false`として区別する。実hardware例外とOSファイル書込拒否の検査も、既存probeの自生成Rust helperだけを使う。固定Civ6 identityを偽装して通さない。ゲーム本体・保存ファイル・DLLは変更しない。console lease generation・Civ6 parent cleanup・native orderは実機成功前のfalse/unknownを維持する。

review19749944の対象検査は13件成功（lib 10件＋実CLI 3件）。helper用subprocess入口1件は直接実行対象から除外し、各親テストから実際に起動した。unknown/empty/reader/BrokenPipe/panicの5ケースはSTOP1回・資源復元・子回収を確認し、強制終了0。実hardware例外の書込失敗ではDBG_CONTINUEとDR復元後、同じRust子を明示終了・waitした。6件の実測行は `D:/CC/Work/todo880/review19749944-owned-helper-receipts.json`。残存helperは0。Civ6の実機復元・lease世代は未検証。

Clippyは実行したが、依存先native-admissionの`question_mark`2件、既存probeの`unnecessary_lazy_evaluations`/`too_many_arguments`/`single_match`/`collapsible_match`/`collapsible_if`計6件で未通過。今回追加部の2件は修正した。元probeのunion読取を含むため、自動提案の`then_some`置換は行っていない。既存native領域はこのworkerの書込対象外であり、Clippy全通過とは報告しない。親による修正版レビューは未受領。

## EDEの許可境界と回収責任

旧`done`と`attach_attempted`の組合せを廃止した。所有状態は `Ungranted / Granted / Attached / Restored / TargetExited`、実行状態は `Active / Finishing / CriticalRecovery / Reaped`で表す。20秒の終了APIが切れた場合の戻り値は`CriticalRecovery`であり、終了成功のreceiptを取得できない。ChildGuardが元のChild・pipe・対象HANDLEを持ち続け、明示回収またはRAIIでOS終了確認まで保持する。新規ポーリングdaemonは作らない。既存native critical cleanupの有界試行の後も未終了なら、OSの終了event待ちで同じ所有stackを残す。wait失敗もハンドルを解放する理由にせず、回収状態を保持する。

attachは次の順序に固定する：子がSHA/opcode等の読取検査を終える → `attach_ready`を送る → 親が保留入力と出力エラーを処理する → まだActive/Ungrantedの場合だけleaseと元HANDLEのidentityを再照合 → 親がGrantedへ遷移してtoken付きGOを送る → 子が正しいGOとPID/FILETIMEを再確認 → `Debugger::attach`。tokenは親のPID/FILETIME・対象identity・source hashに結び付け、readyにはchain/controller/対象のPID/FILETIMEとobserver PIDを含む。GO送信結果が曖昧な場合はGrantedの回収責任を保持する。GO前のキャンセル、入力不正、readinessの遅延、同一性拒否では対象終了APIを呼ばず、子だけSTOP／終了／waitする。

終了責任を検査するhelperもGO前には資源を作らない。GO前拒否とGO後復元を別々に実測し、部分的な終了失敗では20秒APIを期限切れにした後も、2つの自生成OS子が生存したまま保持されることを確認する。明示回収後にのみtarget/observerをwaitする。この検査の対象はRust helperでありCiv6ではない。

ログ失敗の検査は本番と同じ`classify_then_record`を呼ぶ。実hardware eventを旧handle_eventで先に分類してから別のwrite関数だけを検査する形をやめ、共通関数が所有とDBGSTATUSを確定した後にOS書込拒否を発生させる。write側はDBG_CONTINUEが先に保存済みであることを検査するため、順序を逆にすると失敗する。固定Civ6 identityは偽装しない。

追加費用、APIキー呼出し、新しい外部保管先、実機Civ6起動は0を維持する。

EDE修正の先行対象検査は18件成功（lib 15件＋実CLI 3件）。GO前4ケースはpermit=false・対象終了要求0、GO後の入力／実OS BrokenPipeはrestore→reap成功。部分終了失敗では20秒API後も対象helperとobserver helperの両方が生存していることを確認し、明示回収で両方を終了・waitした。共通classify_then_recordの実hardware例外＋OS書込拒否検査も成功。結果行は `D:/CC/Work/todo880/ede-owned-os-receipts.json`、残存helper0。

最後に追加した遅延ready拒否テストを含む再検査は `D:/CC/Work/todo880/ede-final-tests.log`／`.exit`。Clippyの通常実行（`--no-deps`、`-D warnings`なし）は終了0で、今回の書式指摘1件は修正済み。既存probeの6警告は書込範囲外として残し、警告ゼロとは報告しない。親の同一レビューセッションでの受入はまだ未確認。

## GOとSTOPの同時受信・GO後の拒否

本番とOS helperは同じbuffer付き`commands`読取と`Admission`を使う。GO＋STOPを1回のpipe writeで送った場合は、同じbatch全体を評価してattach前キャンセルとする。GO後のlease/identity再照合で失敗しても、`Admission`が対象PID/FILETIME・chain/controller identity・source hash・tokenを含む`NEVER_ATTACHED` terminalを送る。親は完全一致した通知だけを受理し、GrantedからNeverAttachedへ移して子だけ回収する。

二段目の`ATTACH_ENTER`／`ENTER_ACK`を追加した。親がENTERを受信して所有状態をAttachedへ進める前に、子はnative APIを呼べない。親がSTOP中ならACKを出さず、Grantedは対象未変更として子だけ回収できる。子がENTERを送った後はNEVER_ATTACHEDを出さず、正しいACK後にnative-call-attempted状態を先に確定してDebuggerへ渡す。ACKとSTOPが同時なら、許可済みattach後の即時STOPとして復元する。ENTER後の失敗を「未attach」と偽って責任解除しない。

`go-stop-tests.log`／`.exit`には、実OS子と本番と同じ一括読取経路によるGO＋STOP、GO後再照合拒否、ENTER後の偽NEVER_ATTACHED拒否を含む対象テストを保存する。Civ6実機観測は0、追加費用0。親レビュー前の成功宣言はしない。

## 統合時点の照合と順序ログ解析

新主担当の再開時に、writerの`c2d6700e`とbootstrap-integrationの`be17406a`で観測器一式の差分0を確認した。レビューセッション`01a10e39-1740-7d62-b952-4ffd4ab569dd`の最後の判定は、この差分のMust/Suggestion/Nit各0。親の `bootstrap-final-be17-focused.log`／`.exit` は対象22件成功（helper subprocess入口を直接実行する1件は除外）。`323613a3/main-integration`にはまだ観測器一式がないため、公開履歴の統合とbootstrap成果の統合を区別する。

次段階として `civ6-order-observer analyze <capture.jsonl>` を追加した。ゲームやDLLに接続せず、取得済みJSONLを読む。source/PE hash、所有PID/FILETIME、連続sequence、時間、phaseと承認prefix、before/completedの対、元命令の到達先、最終counter・復元結果を照合する。factory call/returnは同じphase区間・thread・RSPで対応付け、phase切替を越えて結び付けない。静的snapshot、途中で切れたログ、古いPIDの終了通知、実行回数の自己申告は合格にしない。旧版identityにphase/prefix記録がない場合は明示エラーとなり、推測で補わない。

このコマンドの終了0は**渡されたログ内部の整合性**だけを意味する。実機receiptの出所を認証するものではなく、mock JSONを実測に昇格させない。`runtime_activation_allowed=false`、`common_load_boundary_proven=false`を維持し、Gameplay Luaの同定・全ロード共通境界・runtime設置の不足を出す。テストの手書きJSONも構造検査fixtureとしてのみ扱う。

親が常時接続へ進める順序は、初回Gameplayより前のInstall、旧オブジェクト破棄より前のClose、破棄後かつ新RESETより前のRebuild、その後の既存COMMIT CASによる許可公開である。解析結果から世代境界が実測できた箇所を絞り込み、324 runtimeへの本接続は親側で行う。ここではexport呼出し・自動install・実機操作を追加していない。

### 親runnerへ接続する契約

親の `native_trace::Session` は最初の非ready行を拒否し、旧probe形式を検査するため、observer parentの前置通知をそのまま渡せない。親側でstart/phase/stopの小さいadapterを追加する。既存trace/bridge/runtimeと同時起動しない。

- 起動: `civ6-order-observer.exe parent <absolute input.json>`。CREATE_NO_WINDOW、stdin/stdoutをpipeにし、stderrも排出する。live_chainから継承した `CIV6_LIVE_CHAIN_PID` と `CIV6_LIVE_LOCK_PATH` を維持する。
- 入力: schema=`todo880-order-v1`、pid/created_filetime、chain_pid/chain_created_filetime、controller_pid/controller_created_filetime、source_dll/source_dll_sha256、output、max_seconds(1..300)、max_hits(1..256)、front_prefix/load_prefix(1..4)、phase(front/load)の15フィールド。ゲームのFILETIMEは既存pin済みhandleから取得する。chainも同一leaseの実値を使う。controllerはrunner自身の実値でよく、parent CLIが自身へ再束縛する。DLLは同sourceビルドの絶対パスと実SHAで、installを要求しない。
- 入力を `order.input.json`、outputを `order.jsonl` にすれば、parent生成の `order.request.json` と衝突しない。既存outputを使い回さず、1実行1組にする。
- `attach_ready` / `ATTACH_ENTER` は前置通知。GO/ENTER_ACKはparent内部で処理し、外側runnerから送らない。`{"ready":true,"pid":1234,"phase":"front","capture_only":true}` が初期ready（PIDは説明例）。stdinを開いたまま保持する。
- `phase load\n` または `phase front\n` を送り、同PIDの `{"phase_ready":"load","pid":1234}` を待ってから操作する。phase切替は最大8回。
- stopは `stop\n` を1回送り、同game PID/FILETIMEの `order_parent_receipt` を読む。正常観測終了は child_reaped=true、cleanup_confirmed=true、restored=true、child_exit_code=0、error=null。その後parent自体の終了0をwait/reapし、readerを回収する。復元の個別証拠は先行する同identityの `order_stop`。
- never_attached=trueは未接続での回収であり復元成功にしない。critical_recovery_retainedは回収責任継続。20秒経過だけでparentをkill/dropしない。標準出力断を成功扱いしない。native_order_observedとconsole_lease_generation_verifiedは実測受入前false。

親は既存ビルド対象の `civ6-startup-diagnostics.exe` に `order-parent <input.json>` と `observe <request.json>` を追加する。autotestのnative-probe依存でbootstrap-orderを有効にし、parentはJSON Valueを `windows::order_capture::parent`、observeは `Request::parse` の結果を `windows::order_capture::observe` へ渡す。parentの関数引数はRequestではなくValue。同一EXEを `observe <generated request.json>` で起動するため、子側の入口名を変えない。GO/ACKをadapterで再実装しない。

この方法ならlive_chainの `driver::prepare` が必ずビルドするdiagnosticsを再利用でき、live toolingへの追加build処理は不要。compiler-artifactのmanifest_path、target.name、executableの既存確認を維持する。worker検査用の別名 `civ6-order-observer` は引き続き共有cargo_gate経由でbuildできる。

```text
D:/CC/Tools/codex-jobs/cargo_gate.exe build --locked --offline --release --manifest-path <worktree>/tools/Cargo.toml --target-dir <dedicated target> -p civ6-native-probe-rs --features bootstrap-order --bin civ6-order-observer
<worktree>/tools/target/release/civ6-startup-diagnostics.exe order-parent <absolute order.input.json>
```

実行器はdiagnosticsのSHA、wrapper・runner・scenarioを含むソースの識別値とcheckoutパスをdriver.jsonへ保存する。候補コミットは親の別検証記録へ残し、観測記録だけでGit候補を認証したとは扱わない。helper本体は同一性確認より前から子の回収まで読取り専用で保持し、途中の差し替えを防ぐ。Steamフォルダへの配置や共有DLLの置換は不要。
次の1実機は、同じowned lease下でfront ready後に初回factory/Lua初期化を取り、load phase_ready後に旧context/Lua破棄を取る。既にfactoryが通過した状態なら初回未観測と記録する。4slotの非選択phaseは欠測であり呼出し0としない。初期receipt前に解析器や自動Installを追加しない。

### 固定AMD版chainからの観測

観測器のchain許可対象は標準 `D:/CC/Tools/civ6-live/live_chain.exe` と、追加の固定 `D:/CC/Tools/civ6-live/live_chain_pr86_amd.exe`（SHA-256 `267ee55fdbe47e507bbefcbb8154780bb2a4070412802f923e0762764d7dc82b`）だけとする。AMD版は開始時に読み取りhandleで書換え・削除を防いでhashを検査し、親／子それぞれが回収完了まで保持する。PID/FILETIME、生存、標準lock、30分lease、祖先関係と継承PIDの検査は維持する。共有lock検査は期待imageを受け取る内部関数へ分け、既存probe呼出しには従来imageだけを渡す。任意pathをrequestに追加せず、548の別コピーも拒否する。固定AMD版のdisk hash検査は起動・実機観測ではない。

### 324 ABIの後続接続条件

`D:/CC/Work/todo324/runtime-bootstrap-abi.json`（source `7d6a1628`、2026-10-06 09:43:37）を参照する。Install configは40byte・align8・version `0x03240001`・mode6、入力trace address/sizeは0。Closeは旧native object/Lua破棄前、Rebuildは破棄後かつ新RESET前。COMMIT後Rebuildは状態を消すので行わない。未接続のABIラッパーは今回追加せず、最初の実機順序receiptから親のnative境界実装を進める。
