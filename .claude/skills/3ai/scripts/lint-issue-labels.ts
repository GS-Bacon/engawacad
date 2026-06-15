#!/usr/bin/env bun
// lint-issue-labels.ts — Issue 起票時のラベル組み合わせ検証
//
// CLAUDE.md / ADR-006 §1 / ADR-002 のラベル運用に基づき、以下を検証する:
//   1. type 軸ラベルが必ず 1 つ含まれること
//      (type:feature / type:refactor / type:foundation / bug / docs)
//   2. type:feature 以外なら batch:* も必須
//      (batch:kernel / batch:data / batch:viewer / batch:skill)
//   3. enhancement は警告対象 (ADR-002 正規外、type:foundation 推奨)
//
// 使い方:
//   bun lint-issue-labels.ts --labels "bug,batch:kernel"
//   bun lint-issue-labels.ts --labels "bug" --labels "batch:kernel"  # 繰り返しも可
//   bun lint-issue-labels.ts --issue 153                              # 既存 Issue を検査
//
// 出力:
//   ok の場合 stdout に "OK: ..." + exit 0
//   ng の場合 stderr にエラー詳細 + exit 1

const TYPE_AXIS_LABELS = new Set([
  "type:feature", "type: feature",
  "type:refactor", "type: refactor",
  "type:foundation", "type: foundation",
  "bug",
  "docs",
]);

const FEATURE_LABELS = new Set(["type:feature", "type: feature"]);

const BATCH_PREFIX = "batch:";

// 既知ラベルセット — 誤字検出 (未知ラベルは error 扱い)
// batch:* / gate:* は prefix 一括許可せず明示列挙 (Issue #168: typo `batch:kerenl` を
// 既知扱いにしないため)。prefix 許可は parent-blocked-by-split:* (Issue 番号 free-form) のみ。
const KNOWN_LABELS = new Set([
  // type 軸
  "type:feature", "type: feature",
  "type:refactor", "type: refactor",
  "type:foundation", "type: foundation",
  "bug",
  "docs",
  // batch 軸 (明示列挙)
  "batch:kernel", "batch:data", "batch:viewer", "batch:skill",
  // 3ailoop gate ラベル (明示列挙)
  "gate:human-feel", "gate:adr-review",
  // 3ailoop needs/blocked ラベル
  "needs-triage", "needs-phase", "needs-human", "needs-intent-review",
  "blocked-by-split",
  // 既存慣習
  "needs-review",
  "enhancement",
  "cli", "format", "kernel", "viewer", "documentation",
  // GitHub default
  "duplicate", "good first issue", "help wanted", "invalid", "question", "wontfix",
]);

const KNOWN_PREFIXES = [
  "parent-blocked-by-split:", // Issue 番号 free-form
];

function isKnownLabel(label: string): boolean {
  if (KNOWN_LABELS.has(label)) return true;
  return KNOWN_PREFIXES.some(p => label.startsWith(p));
}

interface LintResult {
  ok: boolean;
  errors: string[];
  warnings: string[];
}

export function lintLabels(labels: string[]): LintResult {
  const errors: string[] = [];
  const warnings: string[] = [];

  const set = new Set(labels);
  const typeAxis = [...set].filter(l => TYPE_AXIS_LABELS.has(l));
  const batchLabels = [...set].filter(l => l.startsWith(BATCH_PREFIX));
  const isFeature = [...set].some(l => FEATURE_LABELS.has(l));

  // Rule 1: type 軸ラベル必須
  if (typeAxis.length === 0) {
    errors.push(
      `type 軸ラベルがありません。次のいずれかを必ず付けてください: ` +
      `type: feature / type: refactor / type: foundation / bug / docs`,
    );
  } else if (typeAxis.length > 1) {
    warnings.push(
      `type 軸ラベルが複数あります: [${typeAxis.join(", ")}]。通常は 1 つに絞ってください`,
    );
  }

  // Rule 2: feature 以外は batch:* 必須
  if (!isFeature && batchLabels.length === 0) {
    errors.push(
      `type:feature 以外の Issue には batch:* ラベルが必要です。` +
      `内容に応じて batch:kernel / batch:data / batch:viewer / batch:skill のいずれかを付けてください ` +
      `(CLAUDE.md / ADR-006 §1)`,
    );
  }

  // Rule 3: enhancement は ADR-002 正規外
  if (set.has("enhancement")) {
    warnings.push(
      `enhancement は ADR-002 type 軸の正規ラベルではありません。` +
      `機能拡張系は type: foundation を使うことを推奨します`,
    );
  }

  // Rule 4: 未知ラベル検出 (誤字検出、または新ラベル登録漏れ)
  const unknown = [...set].filter(l => !isKnownLabel(l));
  if (unknown.length > 0) {
    errors.push(
      `未知ラベルが含まれています: [${unknown.join(", ")}]。` +
      `誤字の可能性、または新規ラベルなら lint-issue-labels.ts の KNOWN_LABELS / KNOWN_PREFIXES への追加が必要です`,
    );
  }

  return { ok: errors.length === 0, errors, warnings };
}

// --- CLI 引数解析 ---
async function main() {
  const args = process.argv.slice(2);
  const collectedLabels: string[] = [];
  let issueNum = "";

  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case "--labels": {
        const val = args[++i] ?? "";
        // カンマ区切り or 単一値を許容
        for (const l of val.split(",").map(s => s.trim()).filter(Boolean)) {
          collectedLabels.push(l);
        }
        break;
      }
      case "--issue": {
        issueNum = args[++i] ?? "";
        break;
      }
      default:
        process.stderr.write(`Unknown arg: ${args[i]}\n`);
        process.exit(2);
    }
  }

  if (!collectedLabels.length && !issueNum) {
    process.stderr.write(
      "Usage: lint-issue-labels.ts (--labels <csv> | --issue <num>)\n" +
      "  --labels bug,batch:kernel\n" +
      "  --issue 153\n",
    );
    process.exit(2);
  }

  let labels = collectedLabels;
  if (issueNum) {
    const proc = Bun.spawn(
      ["gh", "issue", "view", issueNum, "--json", "labels", "-q", ".labels[].name"],
      { stdout: "pipe", stderr: "pipe" },
    );
    const out = await new Response(proc.stdout).text();
    await proc.exited;
    if (proc.exitCode !== 0) {
      process.stderr.write(`ERROR: Issue #${issueNum} のラベル取得に失敗しました\n`);
      process.exit(2);
    }
    labels = out.split("\n").map(l => l.trim()).filter(Boolean);
  }

  const result = lintLabels(labels);

  if (result.warnings.length > 0) {
    for (const w of result.warnings) {
      process.stderr.write(`WARN: ${w}\n`);
    }
  }

  if (!result.ok) {
    for (const e of result.errors) {
      process.stderr.write(`ERROR: ${e}\n`);
    }
    process.stderr.write(`\n対象ラベル: [${labels.join(", ")}]\n`);
    process.exit(1);
  }

  process.stdout.write(`OK: labels [${labels.join(", ")}] は規約を満たしています\n`);
  process.exit(0);
}

// このファイルが直接実行された場合のみ main を呼ぶ (import される場合は呼ばない)
if (import.meta.main) {
  main().catch(e => { console.error(e); process.exit(2); });
}
