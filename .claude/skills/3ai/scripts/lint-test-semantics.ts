#!/usr/bin/env bun
// lint-test-semantics.ts — テストコードの命名・実装整合チェック
// Usage: bun lint-test-semantics.ts [--dir <crates-root>]
//
// 検出するパターン:
//   - build_fuse*()   が BooleanOp::Cut / BooleanOp::Intersect を呼んでいる
//   - build_cut*()    が BooleanOp::Fuse / BooleanOp::Intersect を呼んでいる
//   - build_intersect*() が BooleanOp::Cut / BooleanOp::Fuse を呼んでいる
//
// exit 0: 問題なし
// exit 1: 不整合あり

import { readFileSync } from "fs";
import { glob } from "glob";

const args = process.argv.slice(2);
let dir = ".";
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--dir") dir = args[++i];
}

const files = await glob(`${dir}/crates/**/tests/**/*.rs`, { nodir: true });

const RULES: Array<{ fnPattern: RegExp; badOps: string[]; label: string }> = [
  {
    fnPattern: /fn\s+build_fuse\w*/,
    badOps: ["BooleanOp::Cut", "BooleanOp::Intersect"],
    label: "build_fuse*() should use BooleanOp::Fuse",
  },
  {
    fnPattern: /fn\s+build_cut\w*/,
    badOps: ["BooleanOp::Fuse", "BooleanOp::Intersect"],
    label: "build_cut*() should use BooleanOp::Cut",
  },
  {
    fnPattern: /fn\s+build_intersect\w*/,
    badOps: ["BooleanOp::Fuse", "BooleanOp::Cut"],
    label: "build_intersect*() should use BooleanOp::Intersect",
  },
];

type Finding = { file: string; line: number; rule: string; detail: string };
const findings: Finding[] = [];

for (const file of files) {
  const src = readFileSync(file, "utf-8");
  const lines = src.split("\n");

  // find each function body
  for (const rule of RULES) {
    let inFn = false;
    let fnStartLine = 0;
    let braceDepth = 0;

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];

      if (!inFn && rule.fnPattern.test(line)) {
        inFn = true;
        fnStartLine = i + 1;
        braceDepth = 0;
      }

      if (inFn) {
        braceDepth += (line.match(/\{/g) ?? []).length;
        braceDepth -= (line.match(/\}/g) ?? []).length;

        for (const badOp of rule.badOps) {
          if (line.includes(badOp)) {
            findings.push({
              file,
              line: i + 1,
              rule: rule.label,
              detail: `found '${badOp}' in function starting at line ${fnStartLine}`,
            });
          }
        }

        if (braceDepth <= 0 && i > fnStartLine) {
          inFn = false;
        }
      }
    }
  }
}

if (findings.length === 0) {
  process.stdout.write("OK: テストコードの命名・BooleanOp 整合チェック通過\n");
  process.exit(0);
}

process.stderr.write(`ERROR: ${findings.length} 件の命名不整合を検出:\n\n`);
for (const f of findings) {
  process.stderr.write(`  ${f.file}:${f.line}\n`);
  process.stderr.write(`    Rule: ${f.rule}\n`);
  process.stderr.write(`    Detail: ${f.detail}\n\n`);
}
process.exit(1);
