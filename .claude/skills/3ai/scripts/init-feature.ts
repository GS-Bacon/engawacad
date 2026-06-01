#!/usr/bin/env bun
// init-feature.ts — features/N-SLUG/ 配下に作業 stub を一括生成する
// STEP 1 で 1 回呼ぶ:
//   bun .claude/skills/3ai/scripts/init-feature.ts --issue N --slug SLUG

import { mkdirSync, writeFileSync, existsSync } from "fs";
import { initState } from "./state.ts";

const PLAN_TEMPLATE = `\
## Non-Goals
<!-- スコープ外を必ず列挙。該当なしの場合も "- 該当なし" と書くこと（空欄禁止）。 -->
<!-- 例: - フル退化検出: #34 で対応予定 -->

## 実装対象
<!-- Issue: #NNN -->
<!-- 影響クレート/ファイル: (具体パス列挙) -->
<!-- 変更する型・関数のシグネチャ -->
<!-- 既存関数の修正がある場合は STEP ごとに before/after スニペットを明記。新規追加のみの場合は不要。 -->

## 設計方針
<!-- 決定性要件: IdGenerator の使い方、同一入力→同一出力の保証方法 -->
<!-- B-rep トポロジー妥当性: Euler-Poincaré V - E + F = 2 が成立するか -->
<!-- 退化幾何の扱い: ゼロ長エッジ、面積ゼロ面などの排除・エラー条件 -->
<!-- derive 規約: Debug/Clone/Serialize/Deserialize (+JsonSchema が必要か) -->
<!-- エラーハンドリング: thiserror の使い方 -->
<!-- workspace.dependencies 規約 -->

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 同一入力を2回実行し全 ID・座標が一致 | assert_eq! |
| T02 | 正常系 | ... | ... |

## 幾何的不変条件チェックリスト
<!-- Boolean/Partition/Assemble 系の Issue のみ記述。非該当は各項目を "N/A" に書き換えること。 -->
- [ ] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか
- [ ] 各プリミティブの face ごとの outer_loop 2D 向き（CW/CCW）が文書化されているか
- [ ] flip_normals / same_sense の意味論が明確か（頂点順を変えるか vs 法線だけ変えるか）
- [ ] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか
`;

const REJECTION_TEMPLATE = `\
<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R0X: 「<指摘の要点>」を棄却 — <理由（scope 外、Non-Goals に記載済み、次 issue で対応 等）> -->
`;

const JUDGMENT_TEMPLATE = `\
<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R01: 採用 → plan の「...」節を修正 -->
<!-- - R02: 棄却 → Non-Goals に記載済みの蒸し返し -->
<!-- - R03: 部分採用 → epsilon 比較は採用。HalfEdge 循環チェックは後続 issue に委譲 -->
`;

if (import.meta.main) {
  let issueNum = "";
  let slug = "";
  const args = process.argv.slice(2);

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--issue") issueNum = args[++i];
    else if (args[i] === "--slug") slug = args[++i];
    else { console.error(`Unknown arg: ${args[i]}`); process.exit(1); }
  }

  if (!issueNum || !slug) {
    console.error("Usage: init-feature.ts --issue <N> --slug <SLUG>");
    process.exit(1);
  }

  const featureDir = `features/${issueNum}-${slug}`;
  if (existsSync(featureDir)) {
    console.error(`ERROR: ${featureDir} already exists. Remove it or use a different slug.`);
    process.exit(1);
  }

  mkdirSync(featureDir, { recursive: true });
  writeFileSync(`${featureDir}/plan.md`, PLAN_TEMPLATE);
  writeFileSync(`${featureDir}/rejection.md`, REJECTION_TEMPLATE);
  writeFileSync(`${featureDir}/judgment-summary.md`, JUDGMENT_TEMPLATE);
  initState(`${featureDir}/state.json`, parseInt(issueNum), slug);

  console.log(`Created ${featureDir}/`);
  console.log(`  plan.md, rejection.md, judgment-summary.md, state.json`);
}
