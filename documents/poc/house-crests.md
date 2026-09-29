# 1534年日本マップの文明家紋（22勢力）

## 実装

SSOT 15.2.1 に従い、22文明の図形を **白 RGB + 透明アルファ** の 512×512 PNG と SVG で自作した。家紋は伝統的な意匠を簡略化して幾何図形で描き直したもので、外部画像の画素や SVG パスを転用していない。画像生成サービスは使っていない。原画と生成ログは Git 外の `C:/Users/makhmeto/Desktop/dev/ADJ/civ6-art-local/JAC_PoC_CustomCivs/Originals/crests/` に置く。各 `<勢力>.svg`、`<勢力>.png`、`generation-log.json` が対応する。

`python tools/jac_art/draw_crests.py` は SVG を作り、その単純な図形を 4 倍解像度で描画して PNG に縮小する。 `python tools/jac_art/build_art.py` は PNG を 8列×3行の Civ アトラスへ並べ、Cooker で `JAC_CustomCivs_Icons.blp` を作る。同時に `tools/jac_art/build/previews/house-crests.png` に104pxと22pxの見本を保存する。BLP と中間 DDS/PNG・見本は Git 管理外。Oda/Takeda の旧生成家紋画像は来歴として残すが、**新しいビルドの入力には使わない**。両家の能力アイコン4枚は既存のものを維持する。追加20家の能力アイコン欄は、その家の文明アイコンを暫定で共用する（個別の能力絵は別件）。

必要な13サイズは公式 `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/DLC/Expansion2/Data/Expansion2_Icons_Civilizations.xml:4` ～ `:16` の文明アトラスに合わせた（22, 30, 32, 36, 44, 45, 48, 50, 64, 80, 128, 200, 256）。同じ13サイズと8列の例が Expansion1 の `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/DLC/Expansion1/Data/Expansion1_Icons_Civilizations.xml:4` ～ `:16` にあり、Base の `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/Base/Assets/UI/Icons/Icons_Civilizations.xml:4` ～ `:13` も参照した。公式 `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/Base/Assets/UI/Instances/CivilizationIcon.lua:57` ～ `:65` は `ICON_` + CivilizationType を組み立て、`IconManager:FindIconAtlas` で UI 寸法の画像を取得し、見つからなければ DataError を出す。FrontEnd と InGame の `UpdateArt`/`UpdateIcons` 登録は公式 `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/DLC/Portugal/Portugal.modinfo:82`、`:131`、`:172`、`:188` の方式に合わせ、既存 CustomCivs の登録を利用した。公式素材そのものは同梱しない。

## 画像台帳

「作者・年」は**今回のベクター作画**を指す。歴史的な紋の形はパブリックドメインの意匠として扱い、参照先の近現代の画像や図版の権利を継承しないよう、すべて座標から描き起こした。新規 SVG/PNG 自体の公開ライセンスは未指定（配布判断は SSOT の D11）。出典ページは紋の名称・家との対応を調べるための資料であり、現存の1534年当時の実物を所蔵しているという意味ではない。全行の所蔵は上記 Git 外ローカル原画フォルダ。

| 用途 | 作品・図案 | 作者・年 | 所蔵・出典 | ライセンス・利用条件 | 元ファイル |
|---|---|---|---|---|---|
| 織田文明 | 織田木瓜（簡略） | 万世紀制作チーム、2026 | ローカル原画。[織田氏](https://ja.wikipedia.org/wiki/織田氏) | 歴史意匠 PD／新規 SVG の公開条件未指定 | `oda.svg` → `oda.png` |
| 武田文明 | 武田菱 | 同上 | ローカル原画。[武田氏](https://ja.wikipedia.org/wiki/武田氏) | 同上 | `takeda.svg` → `takeda.png` |
| 伊達文明 | 竪三引両 | 同上 | ローカル原画。[伊達氏「家紋」](https://ja.wikipedia.org/wiki/伊達氏) | 同上 | `date.svg` → `date.png` |
| 蘆名文明 | 丸に三引両 | 同上 | ローカル原画。[蘆名氏](https://ja.wikipedia.org/wiki/蘆名氏) | 同上 | `ashina.svg` → `ashina.png` |
| 後北条文明 | 三つ鱗 | 同上 | ローカル原画。[後北条氏](https://ja.wikipedia.org/wiki/後北条氏) | 同上 | `hojo.svg` → `hojo.png` |
| 山内上杉文明 | 上杉笹／竹に雀（小サイズ用簡略） | 同上 | ローカル原画。[上杉氏](https://ja.wikipedia.org/wiki/上杉氏) | 同上 | `yamanouchi_uesugi.svg` → `yamanouchi_uesugi.png` |
| 長尾文明 | 九曜巴（小円は簡略） | 同上 | ローカル原画。[長尾氏](https://ja.wikipedia.org/wiki/長尾氏) | 同上 | `nagao.svg` → `nagao.png` |
| 今川文明 | 今川赤鳥（櫛状） | 同上 | ローカル原画。[今川氏](https://ja.wikipedia.org/wiki/今川氏) | 同上 | `imagawa.svg` → `imagawa.png` |
| 松平文明 | 丸に三つ葉葵 | 同上 | ローカル原画。[松平氏](https://ja.wikipedia.org/wiki/松平氏) | 同上 | `matsudaira.svg` → `matsudaira.png` |
| 朝倉文明 | 三つ盛木瓜 | 同上 | ローカル原画。[朝倉氏](https://ja.wikipedia.org/wiki/朝倉氏) | 同上 | `asakura.svg` → `asakura.png` |
| 足利文明 | 足利二つ引 | 同上 | ローカル原画。[足利氏](https://ja.wikipedia.org/wiki/足利氏) | 同上 | `ashikaga.svg` → `ashikaga.png` |
| 三好文明 | 三階菱に五つ釘抜（簡略） | 同上 | ローカル原画。[三好氏](https://ja.wikipedia.org/wiki/三好氏) | 同上 | `miyoshi.svg` → `miyoshi.png` |
| 六角文明 | 隅立て四つ目 | 同上 | ローカル原画。[六角氏](https://ja.wikipedia.org/wiki/六角氏) | 同上 | `rokkaku.svg` → `rokkaku.png` |
| 本願寺文明 | 下り藤（暫定、年代要確認） | 同上 | ローカル原画。[大谷家「家紋」](https://ja.wikipedia.org/wiki/大谷家) | 同上。後代の東西本願寺に見られる図案なので1534年への遡及は未確定 | `honganji.svg` → `honganji.png` |
| 大内文明 | 大内菱（簡略） | 同上 | ローカル原画。[大内氏](https://ja.wikipedia.org/wiki/大内氏) | 同上 | `ouchi.svg` → `ouchi.png` |
| 尼子文明 | 平四つ目結 | 同上 | ローカル原画。[尼子氏](https://ja.wikipedia.org/wiki/尼子氏) | 同上 | `amago.svg` → `amago.png` |
| 毛利文明 | 一文字三星 | 同上 | ローカル原画。[毛利氏「家紋」](https://ja.wikipedia.org/wiki/毛利氏) | 同上 | `mori.svg` → `mori.png` |
| 長宗我部文明 | 七つ酢漿草（簡略） | 同上 | ローカル原画。[長宗我部氏「家紋」](https://ja.wikipedia.org/wiki/長宗我部氏) | 同上 | `chosokabe.svg` → `chosokabe.png` |
| 大友文明 | 杏葉紋（簡略） | 同上 | ローカル原画。[大友氏](https://ja.wikipedia.org/wiki/大友氏) | 同上 | `otomo.svg` → `otomo.png` |
| 島津文明 | 島津十文字／丸に十字 | 同上 | ローカル原画。[島津氏「家紋」](https://ja.wikipedia.org/wiki/島津氏) | 同上 | `shimazu.svg` → `shimazu.png` |
| 琉球文明 | 尚家の左三巴／左御紋（簡略） | 同上 | ローカル原画。[琉球王国](https://ja.wikipedia.org/wiki/琉球王国) | 同上 | `ryukyu.svg` → `ryukyu.png` |
| アイヌ文明 | サケの輪郭（暫定の創作図案） | 同上 | ローカル原画。[国立アイヌ民族博物館・ウポポイ「アイヌ文化について」](https://ainu-upopoy.go.jp/ainu-culture/) | 新規 SVG の公開条件未指定。歴史的な「アイヌ共通家紋」だとは主張しない | `ainu.svg` → `ainu.png` |

### 暫定判断と注意

- 伊達氏の「仙台笹」は後の伊達実元への伝来・変形と説明されるため、1534年勢力には同じ[伊達氏の家紋節](https://ja.wikipedia.org/wiki/伊達氏)に挙がる竪三引両を選んだ。山内上杉の上杉笹と区別できる。
- 本願寺は東西分立より前の1534年を扱う。[大谷家の家紋節](https://ja.wikipedia.org/wiki/大谷家)は下り藤などの用例を**明治以後の両家**として説明する。今回の下り藤は見分けやすい暫定図案であり、1534年時点の本願寺の史料に基づく確定紋とはしない。オーナーに当時の紋の方針を確認したい。
- アイヌには日本の武家と同じ単一の「氏族家紋」を当てはめない。[ウポポイの文化解説](https://ainu-upopoy.go.jp/ainu-culture/)が食生活にサケを挙げることを根拠に、独自の幾何学的なサケを**ゲーム用の識別図案**として作った。伝統文様そのものを無断で代用せず、アイヌ全体の公式紋章とも表示しない。採用の是非はオーナーの判断待ち。
- 家紋は小サイズで読めるよう図形を省略した。年代・分家ごとの細部は出典だけで確証が取れないため、史料再確認と専門家・当事者レビューの余地がある。

## 検査・実機確認

- SVG/PNG 22組と SHA-256 を `generation-log.json` に記録。Cooker が利用可能なこの作業ツリーで `JAC_CustomCivs_Icons.blp` を生成できた。ビルド後の自動テストは全家のアトラス番号、13サイズ、白 RGB、透明アルファ、画像の重複なしを検査する。
- 2026-09-29 の最終静的検査：`python -m unittest discover -s tests -t .` は7件成功、`python -m unittest discover -s tools/civ6-autotest/tests` は98件成功、`python -m unittest discover -s tools/japan_map_scale/tests` は25件中23件成功・2件スキップ、`python -m unittest discover -s tools/jac_art/tests` は3件成功。autotest は Windows の通常 `tempfile` 作成ディレクトリの ACL が書き込みを拒否するため、作業ツリー内の一時領域とプロセス限定の `sitecustomize` で作成モードを調整して同じ discover コマンドを実行した。テスト内の Civ6 起動表示はモックであり、実機は起動していない。
- `python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/house-crests.json --check` は構文を検査する。実機シナリオは FrontEnd で `IconManager:FindIconAtlas` に22家×13サイズを問い合わせ、空または重複する位置を失敗とする。Advanced Setup のスクリーンショットを1枚保存する。これは**描画された各家紋の形を自動判定するものではない**。
- メインセッションでの確認：上記シナリオを実行し、特に22・45・256 px の視認性と、琉球・アイヌ・本願寺の意匠を画面と生成プレビューで見る。必要なら `python tools/jac_art/build_art.py` をメインの MOD 配置側でも実行する。Civ6/Steam の起動は本作業では行っていない。
