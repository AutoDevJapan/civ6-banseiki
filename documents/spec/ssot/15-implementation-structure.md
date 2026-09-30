## 15. 実装構造

### 15.1 分離方針

- 共通コンテンツ定義: 文明、Leader、Unit、能力、装備、ジョブ、人物、宗教
- シナリオ設定: 開始位置、初期都市、初期領土、参加者、イベント、勝利条件
- ゲームモード規則: 成長、家名継承、医療、探索、コンボ、吸収、宗教、襲来、遠征、宇宙規則

```text
japan-archipelago/
├─ core-content/
│  ├─ civilizations/
│  ├─ leaders/
│  ├─ characters/
│  ├─ jobs/
│  ├─ skills/
│  ├─ artifacts/
│  ├─ religions/
│  └─ localization/
├─ game-modes/
│  ├─ japan-legacy-mode/
│  ├─ character-growth-mode/
│  ├─ exploration-mode/
│  ├─ assimilation-mode/
│  ├─ persistent-religion-mode/
│  ├─ alternate-universe-incursion-mode/
│  ├─ alternate-universe-expedition-mode/
│  └─ space-expansion-mode/
├─ scenarios/
│  ├─ sengoku-1534/
│  ├─ japan-unified/
│  ├─ world-baseline/
│  ├─ world-unified/
│  ├─ solar-system/
│  ├─ milky-way/
│  ├─ local-group/
│  ├─ laniakea/
│  └─ universe/
├─ maps/
│  ├─ japan-archipelago/
│  ├─ asia-region/
│  ├─ solar-system/
│  ├─ milky-way/
│  ├─ local-group/
│  ├─ laniakea/
│  └─ universe/
├─ services/
│  ├─ companion-app/
│  ├─ vercel-web/
│  └─ railway-api/
├─ poc/        # 16.1 の PoC 用の検証MOD（本番の構成とは分ける）
├─ tools/
└─ docs/
```

### 15.2 役割分担

| 領域 | 主な実装 |
|---|---|
| 文明・勢力・Unit・建造物・能力 | SQL / XML |
| レベル、ジョブ、宝、探索、コンボ、イベント、吸収、宗教、襲来、遠征 | Gameplay Lua |
| Notebooks、Vault、Secretaries、人物帳、法律帳、宗教帳、Next Action | UI Lua + XML |
| 現行の関係宣言・小国懐柔・大国合併 | 外交画面、再利用する都市国家画面の枠、既存通知。新規の統治画面は作らない |
| 旧9状態案の憲章・正当性・安定度・信頼 | 未採用。採用を決めるまで実装項目には含めない |
| シナリオ内状態 | Game / Player / Unit Property |
| 日本以外の勢力のターン処理 | 自勢力の操作の後、軽量なシミュレーションを裏で実行してターンを進める。まずゲーム内Luaで実装し、処理の細かさはPC性能に合わせて段階的に調整する。自前のRustプログラム（常駐アプリ、またはRailway上のサーバー）への肩代わりは、外部からゲームへ結果を戻す経路のPoCを通してから広げる |
| メタ進行・友達間共有 | Civ6のLuaは外部通信・ファイル書き込みができないため、ゲーム外の常駐アプリ（`services/companion-app/`、本リポジトリで開発）がセーブを読み、Vercel / Railwayと同期する |
| 肖像、家紋、アイコン、地図 | ArtDef / XLP / DDS / Civ6Map |

### 15.2.1 アート制作と保管

- 追加する指導者の肖像は新規生成し、統一した Civ6 風の質感・照明にする。公式画像そのものは成果物に含めない。
- 追加するすべての要素には、それぞれ新規生成した固有アイコンを実装する。指導者は円形の顔・肩アイコン、文明は家紋の白色・透明アルファマスクを必要な全サイズに展開する。ロード画面の前景と外交画面の 2D 画像も固有にする。
- 家紋は史料で確認できる紋を基本とし、確証の無い家はゲーム用の識別図案として扱う。本願寺は下り藤（明治以後の大谷家の用例で、1534年の確定紋とはしない）、アイヌは武家のような共通の家紋が無いため、伝統文様を使わない創作図案のサケの輪郭とする。
- 画像生成には、各サービスの利用規約で許可された経路のみを用い、API 従量課金を発生させない。生成には、ChatGPT のサブスクリプション枠で公式の Codex CLI の画像生成を使う。生成画像と画風の公開・再配布の条件は、公開前に [質問 D11](../../questions.md) で確認する。プロンプト・生成時刻・出力ファイルをローカルに記録する。透過が必要な画像は単色背景で生成し、Pillow で背景を除去する。
- 生成元画像は当面 Git 外の `civ6-art-local/<MOD名>/Originals/` で、Civ6 SDK Cooker で作る BLP などの出力もローカルで管理する。MOD 配置前にビルドする。再生成手順と実装側のテクスチャ参照はリポジトリに保存する。長期保管方式は `documents/questions.md` で議論する。

### 15.3 ID命名規則

```text
JAC_<SCALE>_<ERA_OR_FACTION>_<ENTITY>_<NAME>
```

例:

```text
JAC_JAPAN_SENGOKU_ODA_LEADER_NOBUNAGA
JAC_JAPAN_SENGOKU_ODA_UNIT_MATCHLOCK
JAC_TRAIT_FIERCE
JAC_ARTIFACT_KUSANAGI
JAC_WORLD_COUNTRY_FRANCE_LEADER_...
JAC_SOLAR_FACTION_AI_JAPAN_...
JAC_UNIVERSE_INCURSION_...
```

---
