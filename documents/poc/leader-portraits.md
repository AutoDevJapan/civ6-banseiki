# 1534年日本22勢力の指導者肖像

## 制作方針と史料上の留保

対象は `LEADER_JAC_*` の22人。人物は [1534年開始状態の調査](../research/japan-1534-start.md#22勢力当主本拠支配の中核)と SSOT 3.1 の開始年に合わせる。島津はゲームで採用した**貴久**、アイヌは全域の単一当主と断定せずゲーム代表の**タリコナ**を描く。年齢は生年が分かる場合に `1534−生年` の概数とした。肖像を制作するための年齢・服装・室内の推定は**暫定**であり、人物の実際の容貌・衣服・居室を確定する史料とは扱わない。

旧 [PoC 記録](README.md#画像)にある「20人は織田画像を共用」は、この差し替え前の状態の記録である。現行の22人の画像参照と出典は本書に示す。

歴史的肖像の明示的な参考は、既存の [PoC 画像記録](README.md#画像)に掲載した織田信秀木像の1934年写真と、大泉寺所蔵・1574年の武田信虎像。後者は1534年より40年後の老年像なので、生成画では約40歳へ戻した。その他20人について確実に同定できる同時代肖像の所在・制作年・利用条件までは確認できなかったため、特定の古画から顔を複写したとは主張しない。照会した Wikimedia Commons の検索は一部で HTTP 429 となった。肖像原本の個別照合はオーナーの確認事項とする。

| 指導者 | 1534年の描画年齢 | 衣装・画面上の区別（暫定） |
|---|---|---|
| 織田信秀 | 約23歳 | 尾張の若い武家、烏帽子・小袖 |
| 武田信虎 | 約40歳 | 甲斐の壮年大名。1574年の老年肖像をそのまま使わない |
| 伊達稙宗 | 約46歳 | 南奥の武家、落ち着いた小袖 |
| 蘆名盛舜 | 約44歳 | 会津の武家、深緑の小袖 |
| 北条氏綱 | 約47歳 | 小田原の壮年大名、濃紺の礼装 |
| 上杉憲政 | **約11歳説**。生年不詳説もある | 少年の武家装束。成人像にしない |
| 長尾為景 | 生年不詳、壮年～老年を推定 | 越後の武家。正確な年齢は未断定 |
| 今川氏輝 | 約21歳 | 駿河の若い当主、都風の小袖 |
| 松平清康 | 約23歳 | 三河の若い武家、実用的な小袖 |
| 朝倉孝景 | 約41歳 | 1493年生の孝景。1428年生の同名人物と区別 |
| 足利義晴 | 約23歳 | 若い将軍、室町将軍家の礼装 |
| 三好長慶 | **約12歳** | 少年の装束。後年の成人大名像にしない |
| 六角定頼 | 約39歳 | 近江の当主、烏帽子・小袖 |
| 証如 | **約18歳** | 本願寺門主の僧衣。老僧像にしない |
| 大内義隆 | 約27歳 | 山口の都風文化を反映した礼装 |
| 尼子経久 | **約76歳** | 老年の出雲大名 |
| 毛利元就 | 約37歳 | 安芸国人の控えめな武家装束 |
| 長宗我部国親 | 約30歳 | 土佐の武家、実用的な小袖 |
| 大友義鑑 | 約32歳 | 豊後の当主、青緑の礼装 |
| 島津貴久 | 約20歳 | 採用済みの島津代表、薩摩の若い武家 |
| 尚清王 | 約37歳 | 琉球の王服と冠。日本の武家装束と区別 |
| タリコナ | 生年不詳、成人として暫定描画 | アイヌの植物繊維衣。全道共通当主という史実主張はしない |

## 適用範囲（公式 Gathering Storm と万世紀）

肖像・アイコン・ロード画面・外交画面の画像は、万世紀（`RULESET_JAC_BANSEIKI`）だけでなく、公式 Gathering Storm（`RULESET_EXPANSION_2`）でも有効になる。この範囲は本書の差し替えより前から `JAC_PoC_CustomCivs` の設計であり、本書の変更では広げていない。

1. ゲーム内の画像・データは `poc/JAC_PoC_CustomCivs/JAC_PoC_CustomCivs.modinfo` の条件 `JAC_CustomCivs_GatheringStorm`（`RuleSetInUse` が `RULESET_EXPANSION_2,RULESET_JAC_BANSEIKI`）で読み込む。PoC 束1（#20、`bf1c617`）で `RULESET_EXPANSION_2` として作られ、万世紀ルールセットの PoC（#28、`3f3c1ad`）で万世紀を足した
2. 設定画面の登録は `Players:Expansion2_Players` ドメインで行い、公式 GS と万世紀が `RulesetDomainOverrides` でこのドメインを選ぶ（同 modinfo の FrontEndActions の注記、[PoC 記録](README.md#jac_poc_customcivsカスタム勢力)）。標準ルールセットでは候補0件、GS では JAC の指導者が候補に入ることを実機で確かめている（同記録の「ルールセット境界の再確認」）
3. CustomCivs の実機検証（`clans-notebooks.json`・`clans-both-ai.json`・旧[画像の実機表示](README.md#新しい画像の実機表示)）は公式 GS で公式指導者と並べて行っており、GS での比較実行は意図した使い方である

22勢力が必要な日本マップだけは万世紀専用で、素の GS の地図候補には出ない（[PoC 記録](README.md#jac_poc_japanmapscale1534年開始状態現行)）。画像の適用範囲を万世紀だけに絞るかは CustomCivs 全体の範囲の問題として扱い、この差し替えでは変えない。

## 生成と組み込み

生成元は Git 外の `civ6-art-local/JAC_PoC_CustomCivs/Originals/<leader>/` に置く。全画像は生成画像（本リポジトリで生成）で、SSOT 15.2.1 で決めた生成手段だけを使い、API キーと他の画像サービスを使わない。各 PNG の実際のプロンプト、生成開始・完了時刻、出力名・SHA-256 は同じローカルストアの `generation-log.jsonl` に記録する。立ち姿のマゼンタ背景を `tools/jac_art/build_art.py` が Pillow で透過し、頭部アイコン・ロード画面前景・外交フォールバックを作る。背景はロード画面用 1920×1080 と外交用 1600×1000 に切り出す。生成元 PNG と Cooker 出力 BLP は Git 管理外で、公開・再配布の可否は既存の[質問 D11](../questions.md)の判断待ち。

画像参照の公式根拠：指導者アイコンの `Index` とアトラス形状は `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/Base/Assets/UI/Icons/Icons_Leaders.xml:4-10,21-24` と同じ方式。FrontEnd の `Portrait`・`PortraitBackground` は `DLC/Expansion1/Config/Expansion1_Players.xml:22`。`LoadingInfo` の前景・背景は `DLC/WarMachineScenario/Data/WarMachineScenario_LoadingInfo.xml:3-8`、実際の読込は `Base/Assets/UI/FrontEnd/LoadScreen.lua:210-219,239-248`。外交背景は `DLC/PolandScenario/Data/PolandScenario_DiplomacyInfo.xml:3-8` と `Base/Assets/UI/LeaderScene.lua:58-65`。2D 専用指導者の空の 3D BLP 欄は `DLC/WarMachineScenario/ArtDefs/Leaders.artdef:15-19,39-43`、人物フォールバックは `DLC/WarMachineScenario/ArtDefs/FallbackLeaders.artdef:21-32` に従う。UI 用 BLP の `UITexture` ライブラリ参照は `DLC/Portugal/Portugal.dep:429-437`、FrontEnd の `UpdateArt` は `DLC/Portugal/Portugal.modinfo:172-174` と同じ方式。これらの `DLC/` と `Base/` のパスはすべて上記 Civ6 インストールディレクトリからの相対パス。公式画像そのものは成果物に収録しない。

プロンプト要約（全員共通。人物ごとの差は上の表の年齢・衣装と、居室の地域だけ）：

1. 頭肩肖像：正方形・1024px 以上。1534年時点の年齢と役職で、顔を中央に、円形切り抜き用の余白を取る。背景は暖色の無地
2. 立ち姿：縦長・1024×1536 以上。手足まで入る全身に近い姿で、後で透過するため背景は一様なマゼンタ（#FF00FF）。顔・衣装・配色は頭肩肖像とそろえる
3. 背景：横長 3:2 以上・1536×1024 以上。本拠の地域に合わせた無人の室内で、中央を人物の重ね用に空ける
4. 共通条件：後年の姿や時代に合わない甲冑を避け、現存肖像で顔を確定できると仮定せず独自の顔を作る。Civ6 風の厚塗りで暖色の方向光。公式画像の人物や画素を写さない。文字・ロゴ・透かしを入れない

再生成：`python tools/jac_art/generate_portraits.py` は人物ごとに SSOT 15.2.1 の生成手段で3点の生成を要求し、PNGを検証してローカルログに追記する。続けて `python tools/jac_art/build_art.py` で DDS・XLP・BLP を作り、Git 除外の `tools/jac_art/build/previews/portrait-sheet.png` に22人の顔を並べる。生成途中で止まった人物の画像は調査し、欠けた出力を揃えてから再実行する。失敗時に別サービスへ切り替えない。

## 画像台帳

表の「所蔵と出典」は生成物の保管場所であり、顔の史料上の同定を意味しない。「ライセンス」は公開条件を確定していないため全件 D11 待ち。元ファイルは `civ6-art-local/JAC_PoC_CustomCivs/Originals/` からの相対パス。

| 用途 | 作品 | 作者と年 | 所蔵と出典 | ライセンス | 元ファイル |
|---|---|---|---|---|---|
| 指導者円形アイコン | 織田信秀の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `oda_nobuhide/oda_nobuhide_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 織田信秀の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `oda_nobuhide/oda_nobuhide_figure_codex.png` |
| ロード画面・外交画面背景 | 織田信秀の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `oda_nobuhide/oda_nobuhide_background_codex.png` |
| 指導者円形アイコン | 武田信虎の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `takeda_nobutora/takeda_nobutora_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 武田信虎の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `takeda_nobutora/takeda_nobutora_figure_codex.png` |
| ロード画面・外交画面背景 | 武田信虎の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `takeda_nobutora/takeda_nobutora_background_codex.png` |
| 指導者円形アイコン | 伊達稙宗の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `date_tanemune/date_tanemune_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 伊達稙宗の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `date_tanemune/date_tanemune_figure_codex.png` |
| ロード画面・外交画面背景 | 伊達稙宗の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `date_tanemune/date_tanemune_background_codex.png` |
| 指導者円形アイコン | 蘆名盛舜の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `ashina_morikiyo/ashina_morikiyo_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 蘆名盛舜の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `ashina_morikiyo/ashina_morikiyo_figure_codex.png` |
| ロード画面・外交画面背景 | 蘆名盛舜の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `ashina_morikiyo/ashina_morikiyo_background_codex.png` |
| 指導者円形アイコン | 北条氏綱の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `hojo_ujitsuna/hojo_ujitsuna_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 北条氏綱の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `hojo_ujitsuna/hojo_ujitsuna_figure_codex.png` |
| ロード画面・外交画面背景 | 北条氏綱の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `hojo_ujitsuna/hojo_ujitsuna_background_codex.png` |
| 指導者円形アイコン | 上杉憲政の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `yamanouchi_uesugi_norimasa/yamanouchi_uesugi_norimasa_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 上杉憲政の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `yamanouchi_uesugi_norimasa/yamanouchi_uesugi_norimasa_figure_codex.png` |
| ロード画面・外交画面背景 | 上杉憲政の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `yamanouchi_uesugi_norimasa/yamanouchi_uesugi_norimasa_background_codex.png` |
| 指導者円形アイコン | 長尾為景の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `nagao_tamekage/nagao_tamekage_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 長尾為景の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `nagao_tamekage/nagao_tamekage_figure_codex.png` |
| ロード画面・外交画面背景 | 長尾為景の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `nagao_tamekage/nagao_tamekage_background_codex.png` |
| 指導者円形アイコン | 今川氏輝の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `imagawa_ujiteru/imagawa_ujiteru_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 今川氏輝の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `imagawa_ujiteru/imagawa_ujiteru_figure_codex.png` |
| ロード画面・外交画面背景 | 今川氏輝の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `imagawa_ujiteru/imagawa_ujiteru_background_codex.png` |
| 指導者円形アイコン | 松平清康の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `matsudaira_kiyoyasu/matsudaira_kiyoyasu_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 松平清康の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `matsudaira_kiyoyasu/matsudaira_kiyoyasu_figure_codex.png` |
| ロード画面・外交画面背景 | 松平清康の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `matsudaira_kiyoyasu/matsudaira_kiyoyasu_background_codex.png` |
| 指導者円形アイコン | 朝倉孝景の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `asakura_takakage/asakura_takakage_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 朝倉孝景の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `asakura_takakage/asakura_takakage_figure_codex.png` |
| ロード画面・外交画面背景 | 朝倉孝景の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `asakura_takakage/asakura_takakage_background_codex.png` |
| 指導者円形アイコン | 足利義晴の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `ashikaga_yoshiharu/ashikaga_yoshiharu_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 足利義晴の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `ashikaga_yoshiharu/ashikaga_yoshiharu_figure_codex.png` |
| ロード画面・外交画面背景 | 足利義晴の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `ashikaga_yoshiharu/ashikaga_yoshiharu_background_codex.png` |
| 指導者円形アイコン | 三好長慶の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `miyoshi_nagayoshi/miyoshi_nagayoshi_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 三好長慶の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `miyoshi_nagayoshi/miyoshi_nagayoshi_figure_codex.png` |
| ロード画面・外交画面背景 | 三好長慶の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `miyoshi_nagayoshi/miyoshi_nagayoshi_background_codex.png` |
| 指導者円形アイコン | 六角定頼の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `rokkaku_sadayori/rokkaku_sadayori_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 六角定頼の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `rokkaku_sadayori/rokkaku_sadayori_figure_codex.png` |
| ロード画面・外交画面背景 | 六角定頼の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `rokkaku_sadayori/rokkaku_sadayori_background_codex.png` |
| 指導者円形アイコン | 証如の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `honganji_shonyo/honganji_shonyo_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 証如の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `honganji_shonyo/honganji_shonyo_figure_codex.png` |
| ロード画面・外交画面背景 | 証如の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `honganji_shonyo/honganji_shonyo_background_codex.png` |
| 指導者円形アイコン | 大内義隆の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `ouchi_yoshitaka/ouchi_yoshitaka_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 大内義隆の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `ouchi_yoshitaka/ouchi_yoshitaka_figure_codex.png` |
| ロード画面・外交画面背景 | 大内義隆の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `ouchi_yoshitaka/ouchi_yoshitaka_background_codex.png` |
| 指導者円形アイコン | 尼子経久の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `amago_tsunehisa/amago_tsunehisa_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 尼子経久の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `amago_tsunehisa/amago_tsunehisa_figure_codex.png` |
| ロード画面・外交画面背景 | 尼子経久の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `amago_tsunehisa/amago_tsunehisa_background_codex.png` |
| 指導者円形アイコン | 毛利元就の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `mori_motonari/mori_motonari_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 毛利元就の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `mori_motonari/mori_motonari_figure_codex.png` |
| ロード画面・外交画面背景 | 毛利元就の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `mori_motonari/mori_motonari_background_codex.png` |
| 指導者円形アイコン | 長宗我部国親の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `chosokabe_kunichika/chosokabe_kunichika_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 長宗我部国親の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `chosokabe_kunichika/chosokabe_kunichika_figure_codex.png` |
| ロード画面・外交画面背景 | 長宗我部国親の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `chosokabe_kunichika/chosokabe_kunichika_background_codex.png` |
| 指導者円形アイコン | 大友義鑑の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `otomo_yoshiaki/otomo_yoshiaki_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 大友義鑑の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `otomo_yoshiaki/otomo_yoshiaki_figure_codex.png` |
| ロード画面・外交画面背景 | 大友義鑑の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `otomo_yoshiaki/otomo_yoshiaki_background_codex.png` |
| 指導者円形アイコン | 島津貴久の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `shimazu_takahisa/shimazu_takahisa_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 島津貴久の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `shimazu_takahisa/shimazu_takahisa_figure_codex.png` |
| ロード画面・外交画面背景 | 島津貴久の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `shimazu_takahisa/shimazu_takahisa_background_codex.png` |
| 指導者円形アイコン | 尚清王の新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `ryukyu_sho_sei/ryukyu_sho_sei_portrait_codex.png` |
| ロード画面前景・外交フォールバック | 尚清王の新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `ryukyu_sho_sei/ryukyu_sho_sei_figure_codex.png` |
| ロード画面・外交画面背景 | 尚清王の新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `ryukyu_sho_sei/ryukyu_sho_sei_background_codex.png` |
| 指導者円形アイコン | タリコナの新規生成頭肩肖像 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `ainu_tarikona/ainu_tarikona_portrait_codex.png` |
| ロード画面前景・外交フォールバック | タリコナの新規生成立ち姿 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `ainu_tarikona/ainu_tarikona_figure_codex.png` |
| ロード画面・外交画面背景 | タリコナの新規生成地域別室内 | 生成画像（本リポジトリで生成）、2026-09-29 | ローカルアートストア（`generation-log.jsonl` にプロンプト・生成時刻・ハッシュ） | 公開・再配布は質問 D11 待ち | `ainu_tarikona/ainu_tarikona_background_codex.png` |

## 静的検査と実機確認

2026-09-29 の静的結果：元 PNG は **22人×3点＝66点**、生成ログも66件で各 SHA-256 が一致し、66点相互に同一ハッシュはない。Civ6 SDK Asset Cooker は `JAC_CustomCivs_Icons.blp`（34テクスチャ、7,530,496 B）、`JAC_CustomCivs_Leaders.blp`（66テクスチャ、343,945,216 B）、`JAC_CustomCivs_Fallback.blp`（22テクスチャ、51,407,872 B）を生成した。BLP と `portrait-sheet.png` は Git 除外。`python -m unittest discover -s tests -t .` は10件、`python -m unittest discover -s tools/civ6-autotest/tests` は98件、`python -m unittest discover -s tools/japan_map_scale/tests` は25件（2件スキップ）、`python -m unittest discover -s tools/jac_art/tests -v` は2件成功した。下記シナリオの `--check` も6手順で成功。これはゲーム内表示の証拠ではない。

`python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/leader-portraits.json --check` はシナリオJSONの形式を検査する。実行時には万世紀ルールセットと15 km史実領土地図で22人を開始し、全員のロード前景・ロード背景・外交背景が各自のテクスチャ名を参照すること、22人が重複なく存在することを Lua で調べ、指導者リボンのスクリーンショットを保存する。続けて織田と松平を接触済みにし、松平の外交画面を開いて `LeaderScene` の背景レイヤーがテクスチャを持つこと、上下のレターボックスが隠れていることを調べ、スクリーンショットを保存する。初期画面だけを対象とし、AI 自動進行はしない。一般判定（`generic`）は、公式 `Base/Assets/UI/FrontEnd/LoadScreen.lua:218,248` の `Failed to load background image texture: …`・`We are lacking a texture for …` のうち `JAC_` を含むテクスチャ名の行があれば失敗にする。外交画面の手順（`LeaderScene` コンテキストへの Lua 実行を含む）は実機で未確認。`HostGame` の設定組み立ては公式 `Base/Assets/UI/FrontEnd/GameSetupLogic.lua:689-725` と `Base/Assets/UI/FrontEnd/MainMenu.lua:455-463` の手順に合わせた。画像の**画素や見た目そのもの**はこの Lua 判定では検査できない。

メインセッションの目視確認：ロード時に若年の上杉憲政・三好長慶、老年の尼子経久、琉球王・アイヌ代表の容貌と装束を見て、各指導者のアイコンに頭部欠け・他人の顔・色抜き縁・背景の欠損がないか確かめる。外交画面を開ける各相手について2D人物と固有背景、黒帯や `We are lacking a texture` エラーの有無を確認する。ゲームと Steam はこの作業ツリーでは起動していない。

20勢力の文明アイコン（家紋）は公式アトラスの index 12（日本）を仮に参照しており、固有家紋への差し替えは PR #54 で扱う。本書の範囲は指導者の肖像・立ち絵・背景だけ。

## オーナーへの確認事項

- D11：生成画像を含む PoC の公開・再配布条件。
- 暫定の20人について、同時代肖像の原本同定と服装・顔立ちの史実照合をどの程度必須とするか。特に上杉憲政の出生年説、長尾為景・タリコナの年齢、尚清王の王服の細部。
