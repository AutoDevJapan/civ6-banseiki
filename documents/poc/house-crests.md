# 1534年日本マップの文明家紋（22勢力）

実行コマンドは Rust 版に更新している。移行前の検証結果は旧実行器での記録として残す。

## 実装

SSOT 15.2.1 に従い、22文明の図形を **白 RGB + 透明アルファ** の 512×512 PNG と SVG で自作した。家紋は伝統的な意匠を簡略化して幾何図形で描き直したもので、外部画像の画素や SVG パスを転用していない。画像生成サービスは使っていない。原画と生成ログは Git 外の `C:/Users/makhmeto/Desktop/dev/ADJ/civ6-art-local/JAC_PoC_CustomCivs/Originals/crests/` に置く。各 `<勢力>.svg`、`<勢力>.png`、`generation-log.json` が対応する。

`cargo run --locked --manifest-path tools/Cargo.toml --bin jac-draw-crests --` は SVG を作り、その単純な図形を 4 倍解像度で描画して PNG に縮小する。`cargo run --locked --manifest-path tools/Cargo.toml --bin jac-build-art --` は PNG を 8列×3行の専用アトラス `JAC_Crests<サイズ>` に並べ、Cooker で `JAC_CustomCivs_Icons.blp` に入れる。同時に `tools/jac_art/build/previews/house-crests.png` に104pxと22pxの見本を保存する。BLP と中間 DDS/PNG・見本は Git 管理外。

22家の文明アイコンは家紋ファイルだけで定義する。

1. `Data/JAC_Crests_Icons.xml`：アトラス `ICON_ATLAS_JAC_CRESTS`（13サイズ）と、22家の `ICON_CIVILIZATION_JAC_<家>` の `<Row>`。番号は `draw_crests.NAMES` の順。FrontEnd・InGame とも `UpdateIcons` で読み込む
2. `Config/JAC_Crests_Players.xml`：追加20家の能力アイコン欄（`CivilizationAbilityIcon`・`LeaderAbilityIcon`）を `<Update>` でその家の家紋にする。`UpdateDatabase` の最後に読み込む。織田・武田は専用の能力アイコン4枚を維持する
3. `JAC_CustomCivs_Icons.xml` と `JAC_Japan1534_Icons.xml`（`tools/civ6-assets-rs/src/write_custom_civs.rs` の生成物）には文明アイコンの行を置かない。公式 `Base/Assets/Database/IconManager.sql:31` ～ `:38` の `IconDefinitions` は主キーが `(Name, Atlas)` なので、別アトラスの同名行を `<Replace>` しても古い行は消えずに2行残り、実機（crests-01）では FrontEnd の織田が旧アトラス `JAC_Civs22` のままだった。`tools/civ6-assets-rs/tests` は、MOD のアイコン定義に `<Row>` 以外の操作や重複名が無いこと、文明アイコンが家紋ファイルにしか無いことを確かめる

旧アトラス `ICON_ATLAS_JAC_CIVILIZATIONS`（`JAC_Civs<サイズ>`、織田・武田の旧生成家紋 `oda_mokko_generated.png`・`takeda_bishi_generated.png`）は削除した。`jac-build-art` も作らない

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
| アイヌ文明 | サケの輪郭（ゲーム用の創作図案） | 同上 | ローカル原画。[国立アイヌ民族博物館・ウポポイ「アイヌ文化について」](https://ainu-upopoy.go.jp/ainu-culture/) | 新規 SVG の公開条件未指定。歴史的な「アイヌ共通家紋」だとは主張しない | `ainu.svg` → `ainu.png` |

### 暫定判断と注意

- 伊達氏の「仙台笹」は後の伊達実元への伝来・変形と説明されるため、1534年勢力には同じ[伊達氏の家紋節](https://ja.wikipedia.org/wiki/伊達氏)に挙がる竪三引両を選んだ。山内上杉の上杉笹と区別できる。
- 本願寺は東西分立より前の1534年を扱う。[大谷家の家紋節](https://ja.wikipedia.org/wiki/大谷家)は下り藤などの用例を**明治以後の両家**として説明する。下り藤は1534年時点の本願寺の史料に基づく確定紋とはせず、見分けやすいゲーム用の識別図案として使う（オーナー判断で確定。SSOT 15.2.1）。
- アイヌには日本の武家と同じ単一の「氏族家紋」を当てはめない。[ウポポイの文化解説](https://ainu-upopoy.go.jp/ainu-culture/)が食生活にサケを挙げることを根拠に、独自の幾何学的なサケを**ゲーム用の識別図案**として作った。伝統文様そのものを無断で代用せず、アイヌ全体の公式紋章とも表示しない。この図案の採用はオーナー判断で確定（SSOT 15.2.1）。
- 家紋は小サイズで読めるよう図形を省略した。年代・分家ごとの細部は出典だけで確証が取れないため、史料再確認と専門家・当事者レビューの余地がある。

## 検査・実機確認

- SVG/PNG 22組と SHA-256 を `generation-log.json` に記録。Cooker が利用可能なこの作業ツリーで `JAC_CustomCivs_Icons.blp` を生成できた。ビルド後の自動テストは全家のアトラス番号、13サイズ、白 RGB、透明アルファ、画像の重複なしを検査する。
- 2026-09-30 の静的検査（main 取り込み後）：`cargo test --locked --manifest-path tools/Cargo.toml --workspace --all-targets` は80件成功（7件スキップ）、`cargo test --locked --manifest-path tools/Cargo.toml --workspace --all-targets` は130件成功、`cargo test --locked --manifest-path tools/Cargo.toml --workspace --all-targets` は43件成功（2件スキップ）、`cargo test --locked --manifest-path tools/Cargo.toml --workspace --all-targets` は3件成功（BLP 再ビルド後）。Civ6 は起動していない。
- `cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-autotest -- tools/civ6-autotest/scenarios/house-crests.json --check` は構文を検査する。実機シナリオは次を行う。描画された家紋の形そのものは自動判定しない。
  1. FrontEnd で `IconManager:FindIconAtlas` に22家×13サイズを問い合わせ、`JAC_Crests<サイズ>` 以外のシート、公式の日本（`ICON_CIVILIZATION_JAPAN`）と同じ位置、重複位置を失敗とする
  2. Create Game 画面のスクリーンショット `house-crests-advanced-setup.png` を保存する
  3. 日本マップ 15 km（織田が人間、22家）で開始し、ゲーム内の生存メジャー22家に同じ検査を行う。外交リボンのスクリーンショット `house-crests-diplomacy-ribbon.png` を保存する
  4. 判定（`civ6-judge` の共通チェック `Generic: JAC missing textures`）で、`Lua.log` の JAC を含む `Could not find icon`（公式 `CivilizationIcon.lua:62`）、`We are lacking a texture`・`Failed to load background image texture`（公式 `LoadScreen.lua:248`・`:218`）を失敗とする
- 実機確認の前に、MOD を配置する作業ツリー（Civ6 が読むフォルダ）で BLP を作り直す。原画は `civ6-art-local` から読み、Civ6 SDK の Cooker を使う（Civ6 本体は起動しない）。

  ```
  cargo run --locked --manifest-path tools/Cargo.toml --bin jac-build-art --
  cargo test --locked --manifest-path tools/Cargo.toml --workspace --all-targets
  ```

  `OK JAC_CustomCivs_Icons.blp ... 47 textures` と出れば、家紋の13枚が入っている（指導者8・旧文明13・能力13・家紋13）。
- 画面で見る点：22・45・256 px の視認性と、琉球・アイヌ・本願寺の意匠。
