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
