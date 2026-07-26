#!/usr/bin/env bun
// guard-crates.ts — PreToolUse フック
// Claude(オーケストレーター)が crates/** を直接編集・revert しようとしたら deny する。
// CAD_WORKER=1 の環境（dispatch-glm.ts 経由の子プロセス）は allow する。
// tests/ 配下のみ Claude による Write が許可される（STEP 5.5 acceptance skeleton 用）。
//
// #325 (2026-07-26): settings.json の matcher を Edit|Write|NotebookEdit のみに narrow。
// Bash 系操作 (git reset / stash pop / rebase) は素通り。理由:
//   - loop-mode worker が git ツリー整合を保つのに必須
//   - Bash 引数の文字列 pattern match は Issue body 等の説明テキストも
//     誤爆する (2026-07-26 に本 Issue #325 起票時に発火して観測)
//   - 本来目的は Claude の Edit/Write による直接編集の防止であり、Bash は対象外
// Bash 系のブロックロジックは互換のため残置 (matcher で Bash が来なくなるので dead code)。

if (process.env.CAD_WORKER) process.exit(0);

const raw = await Bun.stdin.text();
let toolName = "";
let filePath = "";
let command = "";
let parseFailed = false;

try {
  const data = JSON.parse(raw);
  toolName = data?.tool_name ?? "";
  const ti = data?.tool_input ?? {};
  filePath = ti.file_path ?? ti.notebook_path ?? "";
  command = ti.command ?? "";
} catch {
  parseFailed = true;
}

function blockEdit(): never {
  process.stdout.write(
    JSON.stringify({
      decision: "block",
      reason:
        "🔒 crates/** の直接編集は禁止です（/3ai フロー）。\ndispatch-glm.ts 経由で GLM-5.1 に委譲してください。\n誤って自分で実装しないこと（PoorDev 問題の再発防止）。",
    }) + "\n"
  );
  process.exit(2);
}

function blockBash(detail: string): never {
  process.stdout.write(
    JSON.stringify({
      decision: "block",
      reason: `🔒 crates/** への破壊操作は禁止です（/3ai フロー）。\n${detail}\ndispatch-glm.ts 経由か、明示的にユーザー承認を得てください。`,
    }) + "\n"
  );
  process.exit(2);
}

if (toolName === "Bash") {
  // Bash の parse 失敗は安全側に倒してブロック
  if (parseFailed) blockBash("コマンドを解析できませんでした。");

  // git reset --hard はコマンド境界で始まる場合に無条件ブロック
  // （commit -m "..." 等のコマンド引数内に含まれる文字列は除外）
  if (/(?:^|;\s*|&&\s*|\|\|\s*|\n\s*)git\s+reset\s+--hard\b/.test(command)) {
    blockBash("`git reset --hard` は crates/ の変更を破壊する可能性があります。");
  }

  // Phase 1: crates/ をパス引数として含むか
  const touchesCrates = /(?:^|[\s=])crates\//.test(command);
  if (touchesCrates) {
    // Phase 2: 破壊的 verb か
    const isMutating =
      /\bgit\s+(?:checkout|restore|clean|rm|mv)\b|\brm\b|\bmv\b|\bcp\b|sed\s+-i|\btruncate\b|>\s*crates\//.test(
        command
      );
    if (isMutating) {
      blockBash(
        "crates/ を対象とした `git checkout/restore/clean` や `rm/mv/cp` などの破壊操作が検出されました。"
      );
    }
  }

  process.exit(0);
}

// Edit | Write | NotebookEdit（または tool_name 不明）
// parse 失敗は後方互換のため allow
if (parseFailed) process.exit(0);

if (/(?:^|\/)crates\//.test(filePath)) {
  // crates/<crate>/tests/ 配下は Claude も書ける（STEP 5.5 acceptance test skeleton 用）
  if (/(?:^|\/)crates\/[^/]+\/tests\//.test(filePath)) process.exit(0);
  // xtask はビルドツールであり CAD カーネルコードではないため直接編集を許可
  if (/(?:^|\/)crates\/xtask\//.test(filePath)) process.exit(0);
  blockEdit();
}

process.exit(0);
