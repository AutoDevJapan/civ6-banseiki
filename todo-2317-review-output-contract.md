# ToDo2317 成果物保存先衝突レビュー

対象: e5853efd34a4a5d649028907876982a3a81670eb（tree 289c1488472778fb10080493bc818e634b0051bd）。前4eb455b0との差分は指定3ファイルのみ。

結論: 追加コードMust 0件、Suggestion 0件、Nit 0件。

- 通常実行はshipのrootを先にcreate_dirで予約し、autosave archiveをroot、driver出力をroot/runに分けます。driverの厳格なcreate_dirとも衝突せず、既存rootなら失敗して既存物を保ちます。失敗後もrootが残り、次回のship番号を進めます。[chain.rs:1511-1521, 1572-1595] [runner.rs:2908-2912]
- begin前に復旧対象を不確定扱いにし、失敗・応答不明時の復旧では互換state/argvを維持します。通常一回の原本復元・不確定時のreceipt保持・連続二回のarchive分離は回帰にあります。reliabilityの10回経路と条件も差分で変わりません。[chain.rs:1572-1607] [parity.rs:4735-4888, 3138-3224]
- RESULTのdriver-error/close-error/通常行は、driverに渡したroot/runの実パスを返します。[chain.rs:1624-1628, 1681-1698]
- 元134件のcold/通常数量とsave/load条件、SSOT、他writerの範囲は変更されていません。公開PR143のbec82e72にもroot/run分離と厳格create_dirの同じ修正があります。[bec82e72: chain.rs:1372-1384, parity.rs:2266-2268]
- Renderer.logはdriverと同じLOCALAPPDATA配下を読み、開始後に更新されたログの最後のSelectedブロックでDevice名と8cを照合します。Available一覧だけでは成功せず、最終証明もrendererとSteam解放を要求します。[todo2317_proof.rs:32-43, 102-106, 250-256, 305-318]

実機未達Must 2件（コードMustとは別）:
1. 原一回は既存出力dirをdriverが検出してexit 2となり、ゲーム開始前に停止しました。実ゲームとSteam解放は未証明です。最新candidateの割当済み一回で再実証し、原失敗は成功扱いしないでください。[chain.rs:1511-1521; todo2317_proof.rs:305-318]
2. 今回の開始後に選択されたCPU rendererの証拠が未取得です。次の一回で新しいRenderer.logのSelected GPU deviceがMicrosoft Basic Render Driver/8cであることを記録してください。[todo2317_proof.rs:250-256, 317-318]

原本autosave・AppOptions・seedはbaselineと一致し、receipt/leaseも消失したとの原記録は維持します。ただしゲーム/Steam/CPU実証の成功には数えません。候補196件成功を新規observer込み197件へ付け替えず、197件の確認は未完了として残します。コード変更・検査再実行・コミットは行っていません。
