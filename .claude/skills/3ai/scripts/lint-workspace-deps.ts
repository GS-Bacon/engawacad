#!/usr/bin/env bun
// lint-workspace-deps.ts — crates/*/Cargo.toml の [dependencies] を workspace 集約に強制
//
// CLAUDE.md の規約:
//   "新しい依存は必ず [workspace.dependencies] に追加し、
//    各クレートは { workspace = true } で参照"
//
// 検出パターン:
//   OK: foo = { workspace = true }         (通常形)
//   OK: foo = { workspace = true, features = [...] }
//   OK: foo.workspace = true                (dotted-key 形)
//   OK: [dependencies.foo] + workspace = true  (subtable 形)
//   NG: foo = "1.2.3"                       (裸のバージョン文字列)
//   NG: foo = { version = "1.2.3", ... }    (inline table にバージョン、workspace なし)
//   NG: foo = { git = "..." }               (git dep、workspace なし)
//   NG: foo = { path = "..." }              (path dep、workspace なし)
//
// スコープ (Phase A):
//   - [dependencies] のみを検査対象とする
//   - [dev-dependencies] / [build-dependencies] は誤検出回避のため対象外
//   - target.<cfg>.dependencies (platform 固有) も対象外
//   - ワークスペースルート Cargo.toml (glob 対象外)
//
// Usage: bun lint-workspace-deps.ts [--dir <workspace-root>]
// exit 0: 問題なし
// exit 1: 違反あり

import { readFileSync } from "fs";

export type Violation = {
  file: string;
  line: number;
  name: string;
  detail: string;
};

type DepEntry = {
  firstLine: number;
  detail: string;
  hasWorkspaceTrue: boolean;
};

// 行末コメント (#...) を除去する。ただし文字列リテラル内の # は保持する。
export function stripInlineComment(line: string): string {
  let inQuote = false;
  let quoteChar = "";
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (inQuote) {
      if (c === quoteChar) {
        inQuote = false;
      }
    } else {
      if (c === '"' || c === "'") {
        inQuote = true;
        quoteChar = c;
      } else if (c === "#") {
        return line.slice(0, i);
      }
    }
  }
  return line;
}

export function scanCrateCargoToml(path: string): Violation[] {
  const src = readFileSync(path, "utf-8");
  const lines = src.split("\n");

  let currentSection = "";
  let currentSubtableDep = ""; // [dependencies.<name>] 形式
  const deps = new Map<string, DepEntry>();

  for (let i = 0; i < lines.length; i++) {
    const rawLine = lines[i];
    const noComment = stripInlineComment(rawLine);
    const stripped = noComment.trim();

    // section header
    const secMatch = stripped.match(/^\[([^\]]+)\]$/);
    if (secMatch) {
      currentSection = secMatch[1].trim();
      currentSubtableDep = "";
      const subMatch = currentSection.match(/^dependencies\.([A-Za-z0-9_-]+)$/);
      if (subMatch) {
        currentSubtableDep = subMatch[1];
        if (!deps.has(currentSubtableDep)) {
          deps.set(currentSubtableDep, {
            firstLine: i + 1,
            detail: rawLine.trim(),
            hasWorkspaceTrue: false,
          });
        }
      }
      continue;
    }

    // [dependencies.<name>] subtable の body
    if (currentSubtableDep) {
      if (/^workspace\s*=\s*true\b/.test(stripped)) {
        deps.get(currentSubtableDep)!.hasWorkspaceTrue = true;
      }
      continue;
    }

    // [dependencies] のみを対象とする
    if (currentSection !== "dependencies") continue;
    if (!stripped) continue;

    // dotted key: name.subkey = value
    const dottedMatch = stripped.match(
      /^([A-Za-z0-9_-]+)\.([A-Za-z0-9_-]+)\s*=\s*(.+)$/,
    );
    if (dottedMatch) {
      const [, name, subkey, valRaw] = dottedMatch;
      let entry = deps.get(name);
      if (!entry) {
        entry = {
          firstLine: i + 1,
          detail: rawLine.trim(),
          hasWorkspaceTrue: false,
        };
        deps.set(name, entry);
      }
      if (subkey === "workspace" && /^true\b/.test(valRaw.trim())) {
        entry.hasWorkspaceTrue = true;
      }
      continue;
    }

    // 通常代入: name = value
    const assignMatch = stripped.match(/^([A-Za-z0-9_-]+)\s*=\s*(.*)$/);
    if (!assignMatch) continue;

    const [, name, valStart] = assignMatch;
    const firstLine = i + 1;
    const detail = rawLine.trim();

    if (valStart.startsWith('"') || valStart.startsWith("'")) {
      // 裸のバージョン文字列: name = "1.2.3"
      if (!deps.has(name)) {
        deps.set(name, { firstLine, detail, hasWorkspaceTrue: false });
      }
      continue;
    }

    if (valStart.startsWith("{")) {
      // inline table (1 行または複数行にまたがる可能性)
      let accumulated = valStart;
      let braceDepth =
        (valStart.match(/\{/g) ?? []).length -
        (valStart.match(/\}/g) ?? []).length;
      let j = i;
      while (braceDepth > 0 && j + 1 < lines.length) {
        j++;
        const nextNoComment = stripInlineComment(lines[j]);
        accumulated += " " + nextNoComment;
        braceDepth +=
          (nextNoComment.match(/\{/g) ?? []).length -
          (nextNoComment.match(/\}/g) ?? []).length;
      }
      const hasWs = /\bworkspace\s*=\s*true\b/.test(accumulated);
      if (!deps.has(name)) {
        deps.set(name, { firstLine, detail, hasWorkspaceTrue: hasWs });
      } else {
        const e = deps.get(name)!;
        e.hasWorkspaceTrue = e.hasWorkspaceTrue || hasWs;
      }
      i = j;
      continue;
    }
  }

  const violations: Violation[] = [];
  for (const [name, entry] of deps) {
    if (!entry.hasWorkspaceTrue) {
      violations.push({
        file: path,
        line: entry.firstLine,
        name,
        detail: entry.detail,
      });
    }
  }
  violations.sort((a, b) => a.line - b.line);
  return violations;
}

async function listCrateCargoTomls(dir: string): Promise<string[]> {
  const g = new Bun.Glob("crates/*/Cargo.toml");
  const files: string[] = [];
  for await (const rel of g.scan({ cwd: dir, onlyFiles: true })) {
    files.push(`${dir}/${rel}`);
  }
  files.sort();
  return files;
}

export async function scanWorkspace(dir: string): Promise<Violation[]> {
  const files = await listCrateCargoTomls(dir);
  const all: Violation[] = [];
  for (const file of files) {
    all.push(...scanCrateCargoToml(file));
  }
  return all;
}

async function main() {
  const args = process.argv.slice(2);
  let dir = ".";
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--dir") dir = args[++i];
  }

  const files = await listCrateCargoTomls(dir);
  const violations = await scanWorkspace(dir);

  if (violations.length === 0) {
    process.stdout.write(
      `OK: workspace.dependencies 集約チェック通過 (${files.length} crates checked)\n`,
    );
    process.exit(0);
  }

  for (const v of violations) {
    process.stderr.write(`${v.file}:${v.line}\n`);
    process.stderr.write(
      `    dep '${v.name}' should use { workspace = true }\n`,
    );
    process.stderr.write(`    Detail: ${v.detail}\n\n`);
  }
  process.stderr.write(
    `ERROR: ${violations.length} 件の workspace 非集約 dep を検出\n`,
  );
  process.exit(1);
}

if (import.meta.main) {
  main().catch((e) => {
    console.error(e);
    process.exit(2);
  });
}
