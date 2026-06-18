#!/usr/bin/env bun
// loop-intent-guard.ts — intent-check の aligned:no を Issue 単位で累積、N回で needs-intent-review
//
// 専用状態ファイル: features/.loop/intent-history.json = { [issue: string]: count }
// (state.ts の failure_streak とは別系統。intent-check 専用カウンタ)
//
// 使い方:
//   bun loop-intent-guard.ts inc   --issue N    # +1、3 で needs-intent-review ラベル付与
//   bun loop-intent-guard.ts reset --issue N    # カウントとラベル削除
//   bun loop-intent-guard.ts check --issue N    # 現在値 stdout、3 以上で exit 1

import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "fs";
import { dirname } from "path";
import { withFileLockSync } from "./loop-file-lock.ts";

const HIST_PATH = "features/.loop/intent-history.json";
const THRESHOLD = 3;

interface History { [issue: string]: number }

function readHist(): History {
  if (!existsSync(HIST_PATH)) return {};
  try {
    const obj = JSON.parse(readFileSync(HIST_PATH, "utf-8"));
    return typeof obj === "object" && obj !== null ? (obj as History) : {};
  } catch {
    throw new Error(`corrupt ${HIST_PATH}`);
  }
}

function atomicWriteHist(h: History): void {
  mkdirSync(dirname(HIST_PATH), { recursive: true });
  const tmp = `${HIST_PATH}.tmp.${process.pid}.${Date.now()}`;
  writeFileSync(tmp, JSON.stringify(h, null, 2), "utf-8");
  renameSync(tmp, HIST_PATH);
}

async function runGh(args: string[]): Promise<{ exit: number }> {
  const proc = Bun.spawn(["gh", ...args], { stdout: "pipe", stderr: "pipe" });
  await proc.exited;
  return { exit: proc.exitCode ?? 0 };
}

async function addLabel(issue: number): Promise<void> {
  await runGh(["issue", "edit", String(issue), "--add-label", "needs-intent-review"]);
}
async function removeLabel(issue: number): Promise<void> {
  await runGh(["issue", "edit", String(issue), "--remove-label", "needs-intent-review"]);
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
    console.error("Usage: loop-intent-guard.ts (inc | reset | check) --issue <N>");
    process.exit(2);
  }

  try {
    if (cmd === "inc") {
      const count = withFileLockSync(HIST_PATH, () => {
        const h = readHist();
        h[String(issue)] = (h[String(issue)] ?? 0) + 1;
        atomicWriteHist(h);
        return h[String(issue)];
      });
      console.log(count);
      if (count >= THRESHOLD) {
        await addLabel(issue);
        process.stderr.write(`WARN: Issue #${issue} reached ${count} aligned:no, needs-intent-review added\n`);
      }
      process.exit(0);
    } else if (cmd === "reset") {
      withFileLockSync(HIST_PATH, () => {
        const h = readHist();
        if (h[String(issue)] !== undefined) {
          delete h[String(issue)];
          if (Object.keys(h).length === 0 && existsSync(HIST_PATH)) {
            rmSync(HIST_PATH);
          } else {
            atomicWriteHist(h);
          }
        }
      });
      await removeLabel(issue);
      console.log("0");
      process.exit(0);
    } else if (cmd === "check") {
      const h = readHist();
      const count = h[String(issue)] ?? 0;
      console.log(count);
      process.exit(count >= THRESHOLD ? 1 : 0);
    } else {
      console.error("Usage: loop-intent-guard.ts (inc | reset | check) --issue <N>");
      process.exit(2);
    }
  } catch (e) {
    console.error(`ERROR: ${(e as Error).message}`);
    process.exit(1);
  }
}
