#!/usr/bin/env bun
// pre-step8-check.ts — STEP 8 前に crates/ の unstaged/untracked ファイルを検出する
// Usage: bun pre-step8-check.ts [--auto-raise] [--feature-dir <path>]
//
// exit 0: 問題なし（staged または追跡済みの変更のみ）
// exit 1: crates/ 配下に unstaged 変更または untracked ファイルあり

const cliArgs = process.argv.slice(2);
let autoRaise = false;
let featureDir = "";

for (let i = 0; i < cliArgs.length; i++) {
  if (cliArgs[i] === "--auto-raise") autoRaise = true;
  else if (cliArgs[i] === "--feature-dir") featureDir = cliArgs[++i];
}

const proc = Bun.spawn(["git", "status", "--porcelain"], { stdout: "pipe", stderr: "pipe" });
const output = await new Response(proc.stdout).text();
await proc.exited;

const lines = output.split("\n").filter((l) => l.trim() !== "");

// unstaged 変更: " M", " D", "??" など（先頭が空白または ??）
const unstaged = lines.filter((l) => {
  const xy = l.slice(0, 2);
  const path = l.slice(3);
  return (xy[1] !== " " || xy === "??") && path.startsWith("crates/");
});

// 新規 untracked ファイル（??）
const untracked = lines.filter((l) => l.startsWith("??") && l.slice(3).startsWith("crates/"));

if (unstaged.length === 0 && untracked.length === 0) {
  process.stdout.write("OK: crates/ に unstaged/untracked ファイルはありません\n");
  process.exit(0);
}

process.stderr.write(
  `ERROR: STEP 8 前に crates/ 配下の未ステージングファイルが見つかりました。\n` +
  `  git add してからもう一度 STEP 8 を実行してください。\n\n`
);

if (untracked.length > 0) {
  process.stderr.write(`  Untracked files:\n`);
  for (const l of untracked) process.stderr.write(`    ${l.slice(3)}\n`);
}

const modifiedUnstaged = unstaged.filter((l) => !l.startsWith("??"));
if (modifiedUnstaged.length > 0) {
  process.stderr.write(`  Unstaged modifications:\n`);
  for (const l of modifiedUnstaged) process.stderr.write(`    ${l.slice(3)}\n`);
}

process.stderr.write(`\n  Suggested command:\n`);
const paths = [...new Set([...untracked, ...modifiedUnstaged].map((l) => l.slice(3).trim()))];
process.stderr.write(`    git add ${paths.join(" ")}\n`);

if (autoRaise && featureDir) {
  const errorSummary = `STEP 8 前に crates/ の未ステージングファイルを検出: ${paths.join(", ")}`;
  try {
    Bun.spawnSync(
      [
        "bun",
        import.meta.dir + "/raise-issue-on-failure.ts",
        "--step", "STEP 8 pre-check (untracked)",
        "--feature-dir", featureDir,
        "--error-summary", errorSummary,
      ],
      { stdout: "inherit", stderr: "inherit" }
    );
  } catch {
    // ベストエフォート: 起票失敗してもメインの exit code には影響させない
  }
}

process.exit(1);
