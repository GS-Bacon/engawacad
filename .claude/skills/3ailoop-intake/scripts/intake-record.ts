#!/usr/bin/env bun
// intake-record.ts — intake の I-* 経緯を features/.intake/issue-<N>.yaml に追記
//
// stages: dedup / intent / phase / granularity / gate / label / created
//
// 使い方:
//   bun intake-record.ts append --issue N --stage <stage> --data '<json>'
//   bun intake-record.ts show --issue N

import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "fs";
import { dirname, join } from "path";

const INTAKE_DIR = "features/.intake";

function filePath(issue: number): string {
  return join(INTAKE_DIR, `issue-${issue}.yaml`);
}

function atomicWrite(path: string, content: string): void {
  mkdirSync(dirname(path), { recursive: true });
  const tmp = `${path}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(tmp, content, "utf-8");
  renameSync(tmp, path);
}

function loadOrInit(issue: number): string {
  const path = filePath(issue);
  if (existsSync(path)) return readFileSync(path, "utf-8");
  return `# intake record for Issue #${issue}\nissue: ${issue}\nstages:\n`;
}

function append(issue: number, stage: string, data: unknown): void {
  const existing = loadOrInit(issue);
  const ts = new Date().toISOString();
  const block =
`  - stage: ${stage}
    at: ${ts}
    data: ${JSON.stringify(data)}
`;
  atomicWrite(filePath(issue), existing + block);
}

function show(issue: number): void {
  const path = filePath(issue);
  if (!existsSync(path)) {
    console.log(`(no intake record for #${issue})`);
    return;
  }
  console.log(readFileSync(path, "utf-8"));
}

if (import.meta.main) {
  const [, , cmd, ...rest] = process.argv;
  function arg(name: string): string | undefined {
    const i = rest.indexOf(name);
    return i >= 0 ? rest[i + 1] : undefined;
  }

  const issueStr = arg("--issue");
  const issue = parseInt(issueStr ?? "");
  if (!issue || isNaN(issue)) {
    console.error("Usage: intake-record.ts (append --issue N --stage <s> --data '<json>' | show --issue N)");
    process.exit(2);
  }

  if (cmd === "append") {
    const stage = arg("--stage") ?? "";
    const dataStr = arg("--data") ?? "null";
    if (!stage) {
      console.error("--stage required");
      process.exit(2);
    }
    let data: unknown;
    try { data = JSON.parse(dataStr); } catch { data = dataStr; }
    append(issue, stage, data);
    console.log(`OK: appended stage=${stage} to ${filePath(issue)}`);
  } else if (cmd === "show") {
    show(issue);
  } else {
    console.error("Usage: intake-record.ts (append | show)");
    process.exit(2);
  }
}
