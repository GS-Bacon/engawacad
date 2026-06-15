#!/usr/bin/env bun
// loop-decision-log.ts — 重要判断を時系列追記
//
// 重要判断 (ADR draft / Issue 分割 / 失敗退避 / Phase 切替 / その他) を
// features/.loop/decisions.log.md に時系列で追記する。dashboard が最新 N 件を抜粋表示する。
//
// 使い方:
//   bun loop-decision-log.ts append --kind <kind> --message "..."
//   bun loop-decision-log.ts show [--limit N]
//
// kind: adr-draft | issue-split | needs-human | phase-transition | other

import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { dirname } from "path";

const LOG_PATH = "features/.loop/decisions.log.md";
const VALID_KINDS = new Set([
  "adr-draft",
  "issue-split",
  "needs-human",
  "phase-transition",
  "other",
]);

function appendEntry(kind: string, message: string): void {
  if (!VALID_KINDS.has(kind)) {
    throw new Error(`invalid kind '${kind}' (valid: ${[...VALID_KINDS].join("|")})`);
  }
  mkdirSync(dirname(LOG_PATH), { recursive: true });
  if (!existsSync(LOG_PATH)) {
    writeFileSync(LOG_PATH, `# /3ailoop Decision Log\n\n`, "utf-8");
  }
  const ts = new Date().toISOString();
  const line = `- [${ts}] [${kind}] ${message}\n`;
  appendFileSync(LOG_PATH, line, "utf-8");
}

function showEntries(limit: number): void {
  if (!existsSync(LOG_PATH)) {
    console.log("(no decisions yet)");
    return;
  }
  const raw = readFileSync(LOG_PATH, "utf-8");
  const entries = raw.split("\n").filter(l => l.startsWith("- "));
  const recent = entries.slice(-limit).reverse();
  console.log(recent.join("\n"));
}

if (import.meta.main) {
  const [, , cmd, ...rest] = process.argv;
  function arg(name: string): string | undefined {
    const i = rest.indexOf(name);
    return i >= 0 ? rest[i + 1] : undefined;
  }

  if (cmd === "append") {
    const kind = arg("--kind") ?? "";
    const message = arg("--message") ?? "";
    if (!kind || !message) {
      console.error("Usage: loop-decision-log.ts append --kind <kind> --message <text>");
      process.exit(2);
    }
    try {
      appendEntry(kind, message);
      console.log(`OK: appended [${kind}] ${message}`);
    } catch (e) {
      console.error(`ERROR: ${(e as Error).message}`);
      process.exit(1);
    }
  } else if (cmd === "show") {
    const limit = parseInt(arg("--limit") ?? "20");
    showEntries(limit);
  } else {
    console.error("Usage: loop-decision-log.ts (append --kind <k> --message <text> | show [--limit N])");
    process.exit(2);
  }
}
