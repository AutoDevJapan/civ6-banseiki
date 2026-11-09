# 通常起動とロード境界のnative接続

通常bootstrapからruntimeへの接続は実装済みです。固定した従来版では、通常Prepareと保存読込→メニュー→同一process再読込の8手順が成功し、runtimeの設置も確認しています。これは全native外側呼出しの排出や製品受入の証明ではありません。`boundary_order_proven=false`と`product_acceptance=false`を保持します。

現候補はSteam build26407122を対象とします。以下の完全SHAと接続する元命令・完全な関数本体・unwind情報を別々に照合します。

| 対象 | SHA-256 |
|---|---|
| DX11 EXE | `d1a0cfd439e7451be515f47595cfa0b6b983f56d1faf4ea434a4ce3b21491ebe` |
| XP2 Core | `092abfbb2176f8df04b5212f819dc269dd83aea866ff05aaf489904a1ff7c41f` |
| Base menu Core | `160875db284b825ec2711d23ba425dab99e10a386b7ad0cbd4a33e40e75bcc05` |

署名・ハッシュの一致は、実行順序やnativeの停止証明とは区別します。対象の許容範囲を広げたり、別版の成功を流用したりしません。

## 起動とWorld出生

MainMenuのcold状態でHost側の追跡を準備し、元loaderがXP2を返した同じthreadで、factoryより前にruntimeを設置します。DllMainや期限切れの観測から設置しません。

| 接続 | EXE RVA |
|---|---|
| holder release / replace | `F68D0` / `F6FF0` |
| loader / verified caller | `F7CD0` / `24A90D` |
| context / holder globals | `22D70C0` / `22D7070` |
| module / factory / destroy slots | `22D70D8` / `22D74E8` / `22D7548` |

Coreの元Context factory `58FB40`、World factory `7504C0`の正常returnから、公開前のContext→CGame→World出生を保持します。raw pointerや、遅れて入ったcallbackで読んだ現在Worldだけから、出生や入力の世代を作りません。

## 呼出しと非同期仕事の寿命

Coreの同期入口 `35F70`、`34840`、`4AFC0`、`36180`、`39920`、`4182B0`を、元の正常returnまたはterminal SEHまで保持します。inner bridge呼出しが0でも外側frameが残れば排出成功にしません。

HostのPush、allocator正常return、元T execute、元destructorを区別し、T/Pの公開前から予約します。元Tの実行がPushのreturnより早くても、発行元の寿命を残します。AddJob `9858B0`、Push `985F40`、再発行 `986010`、公開 `E3E8C0`、typed dispatcher `E42470/E427F0`も完全な呼出しを保持します。DestroyJobSetやFreeJobListのreturnだけを、全仕事の取消やproducer停止にしません。

serialized operationでは、enqueue `373E0`、保存データの再読込 `37690`、複製 `36520`、実行・解放 `372F0`、複製元の後処理 `37F70`、終了時 `380B0`を追跡します。allocator `994970`が元ノードを返してから共有リストへ公開される前に識別を保存し、元の解放 `36E20`が正常returnするまで保持します。同じアドレスが再利用されても、旧識別で新ノードを精算しません。

履歴側の消去 `36950`も完全な呼出しを保持します。payload解放の戻り `3699F`とnode解放の戻り `369B4`を区別し、nodeの予約は元の解放から正常returnするまで残します。最後のsentinel更新と消去関数の復帰より前に排出成功にしません。Worldの元の破棄処理内では、Wait後のinert storageだけを扱う専用の寿命を保持し、新しい実行・製品操作の受付を開きません。

World資格の不明な仕事も元の処理を続けます。不明の印を発行元frame、既発行の兄弟の仕事、複製、worker、consumerへ伝え、その完全な復帰まで製品側の許可を拒否します。workerで不明状態が判明した場合も、保存した発行元の対応を使って逆方向へ伝播します。bridge呼出し自身も入口から最後の借用・returnまで所有を保持します。

## session・入力・ロード要求

native registry、Session、transport、network、threadの出生を公開前から別々に保持します。transportのdisconnectと、registryからの正式なSession削除を区別し、元thread待機、destructor、ノード解放、外側の全returnまで終了扱いにしません。

受信packetは、World ticketが不明でも独立した識別を持ちます。元poll `C66350`、event構築 `856E80`、コピー・拡張・元解放を通して、識別と不明状態を保存します。出生時Worldが不明なtransportや退役済みWorldの入力を、後の現在Worldから補完しません。

遅延キューでは、serialize `847510`、move `527440`、pump `83D570`、decode `7F9C30`、元storage freeの正常return、dispatchを区別します。packetの識別と製品用ticketを別々に保持し、不明なslotが無関係な外側のticketを借りないようにします。

Snapshot `273F40`から元request `16B5C0`、所有stream `166B40`、command36 `162480`、bank consumer `5F470`、`EF220`→`DB810`→`6B7D0`、View `82E50`、Host job、worker `82DA0`へ元要求を渡します。元requestのAL1はcommand公開より後なので、発行と成功を別々に保持します。Host allocatorがNULLを返す元分岐は透明に通し、未公開の予約だけを取り消します。

通常のpathname読込では、同じ元requestが持つUTF-16名と読込設定を入口で保存します。発行済みcommandが選んだViewで、その保存値と元の読込設定が一致する場合に要求を引き継ぎます。古いglobal streamを借りず、pathname要求から現在Sessionの資格を補完しません。元のAL0は、公開前なら通常の拒否として保持します。

NetSnapshot `80F230`からsnapshot ownerへの実呼出しでは、元messageの値、payload、長さ、metadataをコピーします。SessionへWorld関係を渡すには、同じ受信・dispatchで既に発行されたWorld ticketを必要とし、live Sessionの現在値を代用しません。turn/sequenceやローカルに解決されたplayer番号をWorld nonceとして扱いません。

初回のWorldがまだ無いSessionには、出生時に観測した次の未公開Worldの識別候補を保持します。正確なsnapshot ownerとSessionを持つ元load要求が受理され、その要求のworker内で一致するWorldが出生した場合だけ初回関係を確定します。閉鎖中のWorld、古い候補、未観測のregistry、kind 4は採用しません。この候補は入力ticketではなく、結合前に発行されたpacketやtransportを新Worldへ付け替えません。実load gatewayの試験では初回結合と古い候補の拒否を確認しましたが、Session出生・Core設置を含む全体の肯定受入は未成立です。

gatewayごとの入口、元関数の正常復帰、terminal SEHと最初のfatal箇所を、別の常駐診断表へ記録します。bootstrapは元のバイト列と解析結果を別々に保存し、読取・保存・形式の失敗を区別します。実行中の独立したatomic値なので、件数の一致や空集合をnative停止の証明には使いません。

## 自然shutdownと未達

Closeは製品側の許可を失効させ、元の自然shutdownを進めます。World `63E160`→manager `4A1150`→同setのWait `9922C0`をreceiver・caller・stackで照合します。元Waitから後続の破壊処理へ戻す前に実行中の仕事と同期frameを排出し、元の `380B0`で保存用ノードを解放してから最終排出を確認します。現在のshutdown制御stackを自己待ちに含めません。

managerは元の初期化 `49C7C0`の正常returnで世代を作ります。閉鎖時のlist識別と、そのmanager自身による `4A1209`のFreeJobList呼出し・一度だけの正常returnを記録します。Wait後の同setへのAdd/Pushを拒否し、閉鎖済みの記録は実際の再初期化まで残します。list識別だけを使うSpawnは別domainでの再利用を区別できないため、遅いSpawnを検出した場合は受領を未完にします。World閉鎖外のmanager再初期化経路を、World停止の証明には使いません。

list解放は元の戻り番地と、元shutdownがRBXに保持するmanagerを組み合わせて帰属します。解放済みの識別を別managerが再利用しても旧managerへ帰属せず、同じmanagerによる二重解放とは区別します。

製品側の呼出しを閉鎖・排出し、同一WorldのWait・cleanup・外側の終了と全所有hookの保持を確認できた場合には、全domainの証拠不足によるRebuild拒否を致命的な排出失敗と区別します。この場合のstatus 7は、製品の許可を閉じたまま診断追跡だけを次の世代へ進める結果です。未精算の使用、別Worldの終了、所有hookの欠落・改変があればこの復帰を許しません。後の完全なRebuildが成功すれば通常状態へ戻れる構成で、恒久的な無効化を製品受入の代わりにはしません。status 7を含むbootstrap traceは引き続き受入失敗として解析します。

この方針の単体テストは、制御した終了の事実と所有条件を与えたもので、本番設置を通る肯定受入ではありません。全hook所有と実World終了を結んだ実ゲームでの復帰は、変更後にはまだ確認していません。

現在の接続は全producerの自然停止、全scheduler domain、コピー・再発行・取消の全alias、OS受信済みqueueやremote送信元の世代切替を証明していません。`native_coverage_complete()`はfalseのまま、Rebuildを拒否します。正常Waitや追跡件数0だけを全域の許可へ変換しません。

## 確認方法と結果の範囲

所有する試験用メモリで、元処理の一回実行、入口の原子公開と復元、引数・戻り値・LastError・flags、Win64 unwind、SEH、アドレス再利用、発行前と発行後の競合を確認します。試験用命令の成功を、同じscheduleが実ゲームで発生した証明にしません。

ライセンス済みローカルのEXE/Coreを必要とする一次照合は、通常のCIと分けてWindowsの実機作業先で明示実行します。

```powershell
cargo nextest run --locked --manifest-path tools/Cargo.toml -p civ6-native-bridge-rs -E 'test(native_frames::)' --run-ignored only
```

通常CIで省略される一次照合を合格とみなさず、対象件数と実結果を別に保持します。ゲーム原本や必要な利用権のないfixtureを、このコマンドで取得・配布することはありません。

後続候補では通常起動・Prepare・原保存読込・ゲーム内の確認・観測停止の5手順と5判定が成功し、終了・自動保存の復元も確認しています。通常のnative getterでは22人×22ownerの484組でHash/Indexの結果が一致しました。その保存状態では任命可能が0、任命不可が484であり、全員の加入・進化・支払い成功を示す結果ではありません。

さらに後の8手順の実行では、Prepare後の保存読込中の異常終了、Host準備の拒否、Prepare成功後もLoadの成立を確認できない結果が残っています。最後の実行は2/8で、実保存読込の合格には数えていません。今回の初回結合と診断表の変更後は、画面・実ゲームでの再実行を行っていません。

元の製品受入7条件は未達です。通常22人の加入・進化・任命、起動の自動接続と同一process読込、AI本人の費用2・3支払いと余分な昇進拒否、未払い配置拒否、途中支払いMPの保存読込、元113手順と保存比較6組・両AI、正式中和後の功績・効果停止と保存回復・再配置を、短い診断へ置き換えません。
