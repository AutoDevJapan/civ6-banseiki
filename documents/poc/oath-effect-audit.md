# 誓約66件の強い効果を有限手番で測る

Rust生成器は[誓約カタログ](data/sengoku-oaths.json)の22家×3件を、能力診断1本と個別シナリオ66本にする。**今回の実機実行は0件で、全66件・軍事生産速度を合格とはしていない。** 産出の補正前基準を確定できない場合は実数の観測を保存し、最後の数量受入を不合格にする。生成器・形式・Luaの検査成功は、この数量受入の代わりにならない。

生成先は `tools/civ6-autotest/scenarios/oath-effects/`。[一覧](data/oath-effect-scenarios.json)には全66件の誓約、文明、期待する割合、対象と非対象の建造、各シナリオの手番数を記録する。

## 選択と分離

既存の15km日本地図の準備を再利用し、22家・56都市を実所有者つきで確認する。対象家を人間枠に置き、他の21家を残す。全都市で66件の旗が0であることを確認してから、台帳の黄金期だけを検査用に用意する。誓約は本物の選択画面から選び、3候補が対象家のものか、表示文言が正しいか、選択がゲームプレイ側へ保存されたかを検査する。自然な改元が起きたことの証明にはしない。

対象家の全所有都市と他家の全所有都市で、選択前・選択後・診断用の解除後・再適用後を測る。都市ID・所有者・都市中心の区画を照合し、選んだ誓約以外の65件が0であることも各観測時に検査する。対象外の産出、他家の都市、解除と再適用の実数が変われば失敗する。基準が0の産出、住宅で成長が完全に止まる都市だけでは効果を証明できない。

診断中の旗の切替や、手番をまたぐときの台帳の一時変更は、本物の選択とは別の証拠として記録する。最後に選択した誓約と全56都市の旗を戻す。

## 公式UIで使われる数値API

根拠はインストール済みの公式資料 `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/`。以下の行番号は、このPCの資料を調べた時点のもの。

| 観測 | APIと公式の使用箇所 | 数量受入の条件 |
|---|---|---|
| 生産・金・科学・文化・信仰・食料 | `city:GetYield`。`Base/Assets/UI/CitySupport.lua:316–332` | 実効値を記録する。補正前の総産出を返す公式の使用実績があるgetterは見つかっていないため、期待差分は `null`、数量確認は `false` |
| 成長の割合と総倍率 | `GetOtherGrowthModifier`、`GetOverallGrowthModifier`、`GetHousingGrowthModifier`、`GetHappinessGrowthModifier`。同ファイル490・557・570–574 | その他の成長補正が要求割合だけ増え、住宅を含む公式UIの式と総倍率の差分が一致すること |
| 軍事・水軍の生産 | `GetCurrentProductionTypeHash`、`GetUnitProgress`、`GetUnitCost`。同ファイル233–257 | 各段階で3回の1手番進捗を測り、進捗の合計を手番前の実都市生産量の合計で割る。解除時の比率が両側で1と観測できた場合に、適用時の比率を `1 + 要求割合/100` と厳密に比較する |
| 防壁3種類の生産 | `GetBuildingProgress`、`GetBuildingCost`。同ファイル206–215 | 古代・中世・ルネサンス防壁を個別に指定し、軍事・水軍と同じ実進捗の条件で比較する |

成長の総倍率は住宅や幸福度の影響を受ける。一律に要求割合を100で割った差とはしない。公式 `Base/Assets/UI/Panels/CityPanelOverview.lua:667` の式に従い、非占領・食料余剰が正・忠誠度倍率が1の都市で、次の差分を確認する。

`住宅倍率 × {max(1 + 幸福度補正/100 + その他の補正 + 要求割合/100, 0) − max(1 + 幸福度補正/100 + その他の補正, 0)}`

占領・食料不足・忠誠度低下で式の適用条件を満たさない都市は、観測値と理由を記録する。少なくとも1都市で実際に総倍率が増えることを要求する。

産出は、他の加算割合があると「解除時の実効値×要求割合/100」で期待差を求められない。生産キューも、解除時の比率が1でなければ既存補正の計算順を公式Luaだけでは確定できない。その場合、実進捗の増加・復元を観測しても、要求割合の数量確認には数えない。整数の残り手番、SQLのAmount、旗の値だけで数量合格にしない。

## 生産を測る手番

必要な前提技術・社会制度は、読み込まれた建造定義のものだけを用意する。全技術の解禁によるユニットの陳腐化や、防壁を建てられなくなる条件を避ける。下位防壁の用意は公式Tunerの `CreateIncompleteBuilding(..., 100)` を使う（`Base/Assets/UI/Tuner/TunerCityPanel.lua:103`）。自然な技術進行や下位防壁の建設の証明にはしない。

生産は公式の `CityManager.RequestOperation(...BUILD...)` で先頭の項目を置き換える（`Base/Assets/UI/Panels/ProductionPanel.lua:291–370, 2885–2898`）。実所有都市から、公式の開始判定が通り、9手番を測る余裕がある都市を選ぶ。

解除・適用・解除の各3手番を、公式Shift+Enterと同じ `UI.RequestAction(ACTION_ENDTURN, { REASON = "UserForced" })` で進める（`Base/Assets/UI/ActionPanel.lua:1099`）。AIへ操作を渡さず、要求は各観測につき1回だけ。次の人間手番がちょうど1手番後であることを待ち、実数の進捗を測る。途中の完成、項目や費用の変更、手番の飛び、復元失敗は不合格になる。手番後の都市生産量も記録し、手番内に変化した場合は数量未確認にする。丸めを含む割合の誤差幅が0.5パーセントポイント未満にならなければ、隣の整数割合と区別できないため数量確認をfalseにする。実機の進捗丸め幅は初回診断と実測で確認する。

軍事・防壁の非対象には労働者、水軍の非対象には陸上の槍兵を実際に生産する。同時に都市生産量が増える誓約でも、非対象の進捗をその都市生産量で割った比率が増えないことを要求する。通常進行は産出・成長のみで0手番、軍事・水軍で18手番、防壁で36手番。何百手番も進める処理はない。

## 初回の能力診断と検査

最初に `capabilities.json` を実行する。GameCore_TunerとInGameで、各APIの存在、呼出成否、戻り値の型、有限数値か、値、エラーをJSONにする。不明な `GetBaseYield`、`GetYieldModifier`、`GetProductionModifier` はこの診断だけで呼び、数値が返っても単位と適用範囲の確認が済むまで数量受入には使わない。

CargoはこのPCのゲートを通す。実機を起動しない検査は次のとおり。

```powershell
$gate = 'D:/CC/Tools/codex-jobs/cargo_gate.exe'
& $gate run --locked --manifest-path tools/Cargo.toml -p civ6-data-rs --bin generate-oath-effect-audit
& $gate run --locked --manifest-path tools/Cargo.toml -p civ6-data-rs --bin generate-oath-effect-audit -- --check
& $gate test --locked --manifest-path tools/Cargo.toml -p civ6-data-rs --lib oath_effect_audit -- --nocapture
& $gate test --locked --manifest-path tools/Cargo.toml -p civ6-autotest-rs --test oath_effect_scenarios -- --nocapture
& $gate test --locked --manifest-path tools/Cargo.toml --workspace --all-targets
```

生成した全Lua手順・待機条件と、全PoCの製品LuaをLua 5.1でコンパイルする。既存の助言画面1本にあるHavokの構造体は、13項目の型を検査し、コメントと文字列を区別するRustの字句検査で、既存の史実イベント検査と同じ構造体の変換を行う。それ以外の製品Luaは本文をそのままコンパイルする。製品のLuaは変更しない。実数の増加・住宅の抑制・割合の取り違え・非対象への漏れ・復元・誤った所有都市・手番の飛び・不明APIを、生成されたコードを実行する負例で検査する。66件と能力診断のJSONは、実際のドライバの形式検査も通す。

実機の起動・停止は統合担当がlive_chain経由で管理する。最初の有限診断は、親worktreeへ取り込んで再ビルドした実行ファイルから、次のシナリオを渡す。出力先は実行ごとに変える。

```powershell
& $gate run --locked --manifest-path tools/Cargo.toml -p civ6-autotest-rs --bin civ6-autotest -- tools/civ6-autotest/scenarios/oath-effects/capabilities.json --output tools/civ6-autotest/artifacts/oath-capability-first
```

`steps.jsonl` の2つの能力JSONを先に確認する。数量基準が未確定のまま66本を受入試験として扱わない。各シナリオは実測JSONを残した後、最後の `accepted == true` の確認で、未対応の数量基準を不合格にする。

## 親担当が実機で続ける項目

- 2コンテキストの能力診断。産出の補正前基準と既存生産補正の数値契約の確定。
- 全66件の本物の選択、全実所有都市の数値、対象と非対象の建造、診断後の復元。
- 他家の建造キューへの軍事・水軍・防壁補正の漏れ。この生成器は他家の全都市の6産出と総成長倍率を比較するが、他家の建造を操作していない。
- 保存・読み込み後の全66件の復元と、自然な改元による失効。今回の旗と台帳の復元は保存・読み込みの証明ではない。

既存PR85の織田「天下布武」の文化効果は、2城での切替と保存復元の実機確認がある。今回の66件や軍事速度への転用はしない。
