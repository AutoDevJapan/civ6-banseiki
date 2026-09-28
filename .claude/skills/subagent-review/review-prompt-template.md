You are reviewing {対象の説明（例：Civ6 MOD の poc/JAC_PoC_GrowthUI と、その検証手順）}. Read-only: do NOT modify files. Answer in Japanese.

## Target
{差分のファイルパス、またはレビュー対象のフォルダ・ファイル}

## Spec (read these sections)
{関係する SSOT のファイル。例：documents/spec/ssot/16-roadmap.md（16.1 の束1）}

## Verified facts (take these into account)
{実機やテストで検証済みの事実と根拠。例：
- documents/poc/README.md の JAC_PoC_Bundle0 の結果：ゲームプレイ Lua で Events.LoadScreenClose と Events.TurnBegin を登録し、実機で動作した
- Game:SetProperty のテーブルはセーブ・ロード後に復元された（シングルプレイ）}

## Primary references you may read
{例：C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI（Base/Assets, DLC/Expansion1, DLC/Expansion2）}

## Rules
documents/repository-rules.md

## What to look for
{観点。例：読み込みを妨げる誤り、API の使い方、成功条件と実装・ログの一致、マルチプレイでの同期}

## How to report
- Classify findings as Must / Suggestion / Nit, each with file:line and a concrete fix.
- If a concern contradicts a verified fact above, still report it, label it "要実機確認", and explain why you are still concerned.
- Cautious, safety-side findings are welcome and are never penalized. Do not withhold a concern because you are unsure; say how confident you are instead.
- Keep the answer under {文字数} Japanese characters.
