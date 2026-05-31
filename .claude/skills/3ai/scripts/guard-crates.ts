#!/usr/bin/env bun
// guard-crates.ts — PreToolUse フック
// Claude(オーケストレーター)が crates/** を直接編集しようとしたら deny する。
// CAD_WORKER=1 の環境（dispatch-glm.ts 経由の子プロセス）は allow する。

if (process.env.CAD_WORKER) process.exit(0);

const input = await Bun.stdin.text();
let filePath = "";

try {
  const data = JSON.parse(input);
  const ti = data?.tool_input ?? {};
  filePath = ti.file_path ?? ti.notebook_path ?? "";
} catch {
  // parse error = allow through
}

if (/(?:^|\/)crates\//.test(filePath)) {
  process.stdout.write(
    JSON.stringify({
      decision: "block",
      reason:
        "🔒 crates/** の直接編集は禁止です（/3ai フロー）。\ndispatch-glm.ts 経由で GLM-5.1 に委譲してください。\n誤って自分で実装しないこと（PoorDev 問題の再発防止）。",
    }) + "\n"
  );
  process.exit(2);
}

process.exit(0);
