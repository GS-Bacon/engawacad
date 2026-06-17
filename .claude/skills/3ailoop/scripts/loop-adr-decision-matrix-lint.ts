#!/usr/bin/env bun
// loop-adr-decision-matrix-lint.ts — ADR draft の Decision Matrix を機械 lint (ADR-013)
//
// ADR draft markdown を読み、以下の必須セクション/要素を判定:
//   1. ## Decision 直下に Options A/B/C (≥3 案) の列挙
//   2. 各 option の Trade-off
//   3. 採用 option とその選択理由
//   4. 採用前提崩壊 trigger (最低 1 件)
//   5. 既存 ADR との関係 (Independent 明記も可)
//
// 使い方:
//   bun loop-adr-decision-matrix-lint.ts --adr <path>
//   exit 0 = pass, exit 2 = fail (詳細は stderr), exit 1 = 内部エラー

import { existsSync, readFileSync } from "fs";

export type LintIssue = {
  rule: string;
  message: string;
};

export type LintResult = {
  ok: boolean;
  issues: LintIssue[];
};

const SECTION_RE = /^##\s+(.+)$/;

function splitSections(md: string): Map<string, string> {
  const lines = md.split("\n");
  const sections = new Map<string, string>();
  let current = "__preamble__";
  let buf: string[] = [];
  for (const line of lines) {
    const m = line.match(SECTION_RE);
    if (m) {
      sections.set(current, buf.join("\n"));
      current = m[1].trim();
      buf = [];
    } else {
      buf.push(line);
    }
  }
  sections.set(current, buf.join("\n"));
  return sections;
}

function findOptions(text: string): string[] {
  // ADR の Options を検出。許容パターン:
  //   "- A. opt1" / "- (A) opt1" / "1. A) opt1" / "**A.**"
  //   "### (a) sentinel ..." (ADR-012 流の Alternatives Considered 見出し)
  //   大文字 A-F / 小文字 a-f
  const labels = new Set<string>();
  const re = /(?:^|\n)\s*(?:#{2,4}\s+)?(?:[-*]\s+|\d+\.\s+|\(|\[|\*\*)?(?:Option\s+)?([A-Fa-f])(?:[\)\]\.]|\b)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    labels.add(m[1].toUpperCase());
  }
  return [...labels].sort();
}

function hasTradeOff(decisionBody: string): boolean {
  // "Trade-off" / "Trade off" / "trade-off" / "トレードオフ" / "Pros" + "Cons"
  if (/trade[-\s]?off/i.test(decisionBody)) return true;
  if (/トレードオフ/.test(decisionBody)) return true;
  // Pros/Cons セットを各 option ごとに書く形式
  const pros = /(?:pros|利点|長所|正)/i.test(decisionBody);
  const cons = /(?:cons|欠点|短所|負|reject|却下)/i.test(decisionBody);
  return pros && cons;
}

function hasAdoptedOption(decisionBody: string): boolean {
  // "採用" / "Adopted" / "選択" / "選定" / "本 ADR" / "決定:"
  return /(採用|adopt|chosen|selected|決定[::]|選択肢[ABC])/i.test(decisionBody);
}

function hasTrigger(md: string): boolean {
  // 採用前提崩壊 trigger / Trigger / 見直し条件 / Revisit
  return /(採用前提崩壊\s*trigger|^\s*###?\s+.*trigger|revisit|見直し条件|前提が崩れ)/im.test(md);
}

function hasAdrRelations(md: string): boolean {
  // "既存 ADR との関係" / "Related" / "ADR-NNN" 引用が 1 つ以上、もしくは "Independent" 明記
  if (/independent/i.test(md)) return true;
  if (/ADR-\d{3}/.test(md)) return true;
  if (/(既存\s*ADR\s*との関係|related\s+ADRs?)/i.test(md)) return true;
  return false;
}

export function lintAdr(md: string): LintResult {
  const issues: LintIssue[] = [];
  const sections = splitSections(md);

  const decision = sections.get("Decision");
  if (!decision || decision.trim().length === 0) {
    issues.push({ rule: "decision_section", message: "## Decision セクションが見つかりません" });
  } else {
    // Options は Decision + Alternatives Considered の両方から検出 (ADR 慣習)
    const alternatives = sections.get("Alternatives Considered") ?? "";
    const optionsScope = decision + "\n" + alternatives;
    const options = findOptions(optionsScope);
    if (options.length < 3) {
      issues.push({
        rule: "options_count",
        message: `Options が ${options.length} 案しか検出されません (>=3 必須)。検出: ${options.join(", ") || "なし"}`,
      });
    }
    if (!hasTradeOff(optionsScope)) {
      issues.push({
        rule: "trade_off",
        message: "Trade-off / Pros-Cons の記述が見つかりません",
      });
    }
    if (!hasAdoptedOption(decision)) {
      issues.push({
        rule: "adopted_option",
        message: "採用 option (採用 / Adopted / 決定: 等) の明示が Decision セクションにありません",
      });
    }
  }

  if (!hasTrigger(md)) {
    issues.push({
      rule: "revisit_trigger",
      message: "採用前提崩壊 trigger (Revisit 条件) の明示がありません",
    });
  }
  if (!hasAdrRelations(md)) {
    issues.push({
      rule: "adr_relations",
      message: "既存 ADR との関係 (Related: ADR-NNN or Independent) の明示がありません",
    });
  }

  return { ok: issues.length === 0, issues };
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  function arg(name: string): string | undefined {
    const i = argv.indexOf(name);
    return i >= 0 ? argv[i + 1] : undefined;
  }
  const adrPath = arg("--adr");
  if (!adrPath) {
    console.error("Usage: loop-adr-decision-matrix-lint.ts --adr <path-to-adr.md>");
    process.exit(1);
  }
  if (!existsSync(adrPath)) {
    console.error(`ERROR: ADR file not found: ${adrPath}`);
    process.exit(1);
  }
  const md = readFileSync(adrPath, "utf-8");
  const result = lintAdr(md);
  if (result.ok) {
    console.log(`OK: ${adrPath} passes Decision Matrix lint`);
    process.exit(0);
  }
  process.stderr.write(`FAIL: ${adrPath}\n`);
  for (const i of result.issues) {
    process.stderr.write(`  - [${i.rule}] ${i.message}\n`);
  }
  process.exit(2);
}
