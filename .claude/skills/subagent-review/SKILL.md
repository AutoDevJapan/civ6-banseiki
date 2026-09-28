---
name: subagent-review
description: Codex などのサブエージェントにコード・MOD・ドキュメントのレビューを頼むときに使う。検証済みの事実や仕様の該当節を漏れなく渡し、安全側の懸念を遠慮なく出させ、結果を公平に評価して GTO に記録する手順。
---

# サブエージェントへのレビュー依頼

レビューの質は、渡したコンテキストで決まる。渡していない事実を知らないまま出た指摘は「誤検知」ではなく、依頼側の不足として扱う。

## 1. プロンプトを組み立てる

[review-prompt-template.md](review-prompt-template.md) を写し、`{...}` をすべて埋める。空欄のまま送らない。

必ず入れるもの：

1. **対象**：差分（`git diff origin/main...HEAD`）またはファイル・フォルダ
2. **仕様**：関係する SSOT の節のファイル（`documents/spec/ssot/…`）
3. **検証済みの事実**：実機やテストで確かめた結果と、その根拠
   - `documents/poc/README.md` の「結果」欄
   - 実機で動作済みのコード（例：`poc/JAC_PoC_Bundle0/`）
   - テストの実行結果
4. **参照してよい外部の一次資料**：ゲーム本体のフォルダ、SDK の文書など
5. **ルール**：`documents/repository-rules.md`

## 2. 実行する

- 読むだけの依頼にする：`codex exec -m <モデル> -c model_reasoning_effort='"<強度>"' -s read-only --skip-git-repo-check -C <作業フォルダ> -o <出力> - < <プロンプト>`
- 複数の対象は、並列で別々に依頼する

## 3. 結果を扱う

- 指摘が検証済みの事実と食い違う場合は、事実を優先する。ただし「要実機確認」として残し、次の実機確認で確かめる
- 採用しない指摘も、理由を PR のレビューコメントに書く

## 4. 評価して GTO に送る

GTO（`gto.exe`）がある環境では、`"kind": "model_fit_observation"` の JSON を `gto internal ingest-turn` の標準入力に渡す。

- `result_quality`（0〜5）は、**渡したコンテキストの範囲で**妥当だったかで付ける
- 安全側に倒した指摘は減点しない。依頼側の渡し方に原因があれば `notes` にそう書く
- 評価を誤ったら、`corrects` を付けた訂正の記録を送る
