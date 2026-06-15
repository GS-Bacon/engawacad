#!/usr/bin/env bun
// loop-failure-tracker.ts — Issue 単位の連続失敗カウントを管理し、N回で needs-human 退避
//
// 内部で state.ts の inc-failure / reset-failure / get-failure を呼び (= features/.loop/failure-streak/<N>.json
// を操作)、閾値超過で gh label add --add-label needs-human を実行する。
//
// 使い方:
//   bun loop-failure-tracker.ts inc   --issue N      # +1、閾値超過で needs-human 付与
//   bun loop-failure-tracker.ts reset --issue N      # カウントと needs-human ラベル削除
//   bun loop-failure-tracker.ts check --issue N      # 現在値 stdout、閾値超過で exit 1

const FAILURE_THRESHOLD = 3;
const STATE_TS = ".claude/skills/3ai/scripts/state.ts";

async function runBun(args: string[]): Promise<{ stdout: string; exit: number }> {
  const proc = Bun.spawn(["bun", ...args], { stdout: "pipe", stderr: "pipe" });
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  return { stdout: out.trim(), exit: proc.exitCode ?? 0 };
}

async function runGh(args: string[]): Promise<{ stdout: string; exit: number }> {
  const proc = Bun.spawn(["gh", ...args], { stdout: "pipe", stderr: "pipe" });
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  return { stdout: out.trim(), exit: proc.exitCode ?? 0 };
}

async function addNeedsHumanLabel(issue: number): Promise<void> {
  await runGh(["issue", "edit", String(issue), "--add-label", "needs-human"]);
}

async function removeNeedsHumanLabel(issue: number): Promise<void> {
  await runGh(["issue", "edit", String(issue), "--remove-label", "needs-human"]);
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
    console.error("Usage: loop-failure-tracker.ts (inc | reset | check) --issue <N>");
    process.exit(2);
  }

  if (cmd === "inc") {
    const r = await runBun([STATE_TS, "inc-failure", "--issue", String(issue)]);
    if (r.exit !== 0) {
      console.error(`state.ts inc-failure failed: ${r.stdout}`);
      process.exit(1);
    }
    const count = parseInt(r.stdout);
    console.log(count);
    if (count >= FAILURE_THRESHOLD) {
      await addNeedsHumanLabel(issue);
      process.stderr.write(`WARN: Issue #${issue} reached ${count} failures, needs-human added\n`);
    }
    process.exit(0);
  } else if (cmd === "reset") {
    const r = await runBun([STATE_TS, "reset-failure", "--issue", String(issue)]);
    if (r.exit !== 0) {
      console.error(`state.ts reset-failure failed: ${r.stdout}`);
      process.exit(1);
    }
    await removeNeedsHumanLabel(issue);
    console.log("0");
    process.exit(0);
  } else if (cmd === "check") {
    const r = await runBun([STATE_TS, "get-failure", "--issue", String(issue)]);
    if (r.exit !== 0) {
      console.error(`state.ts get-failure failed: ${r.stdout}`);
      process.exit(1);
    }
    const count = parseInt(r.stdout || "0");
    console.log(count);
    process.exit(count >= FAILURE_THRESHOLD ? 1 : 0);
  } else {
    console.error("Usage: loop-failure-tracker.ts (inc | reset | check) --issue <N>");
    process.exit(2);
  }
}
