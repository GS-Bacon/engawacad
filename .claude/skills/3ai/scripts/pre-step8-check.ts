#!/usr/bin/env bun
// pre-step8-check.ts — STEP 8 前に crates/ の状態を検査する
// Usage: bun pre-step8-check.ts [--feature-dir <path>]
//
// チェック項目:
//   (a) crates/ 配下の unstaged/untracked ファイル
//   (b) crates/<crate>/tests/*.rs に「すべて #[ignore] + todo!() の腐敗 stub」
//       または「temporary/removed を含む 1〜3 行 stub」が残っていないか (#139)
//
// exit 0: 問題なし
// exit 1: いずれかが検出された (ガード違反)

import { readFileSync } from "fs";

export type StaleStub = { path: string; reason: "all_todo_ignore" | "oneline_stub" };

async function main(): Promise<number> {
  const cliArgs = process.argv.slice(2);
  let _featureDir = "";

  for (let i = 0; i < cliArgs.length; i++) {
    if (cliArgs[i] === "--feature-dir") _featureDir = cliArgs[++i];
    // --auto-raise は廃止。フラグが渡されても無視する（後方互換）
    else if (cliArgs[i] === "--auto-raise") { /* no-op */ }
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

  // (b) stale test stub 検出 (#139): crates/<crate>/tests/*.rs を走査
  const stales = await detectStaleTestStubs();

  let exitCode = 0;

  // (a) unstaged/untracked 検出
  if (unstaged.length > 0 || untracked.length > 0) {
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
    process.stderr.write(`    git add ${paths.join(" ")}\n\n`);
    exitCode = 1;
  }

  // (b) stale test stub 警告
  if (stales.length > 0) {
    process.stderr.write(
      `ERROR: crates/<crate>/tests/ 配下に腐敗 stub テストファイルが残っています (#139 ガード)。\n` +
      `  STEP 8 直前のこのチェックを通過すると CI green が実態より広いカバレッジを示唆する状態になります。\n` +
      `  実装が完了したファイルは #[ignore] を外し、不要なファイルは削除してください。\n\n`
    );
    for (const s of stales) {
      const reasonLabel =
        s.reason === "all_todo_ignore"
          ? "すべての #[test] 関数が #[ignore] + todo!() のみ"
          : "1〜3 行の temporary/probe/safely deleted stub";
      process.stderr.write(`    ${s.path}  (${reasonLabel})\n`);
    }
    process.stderr.write(`\n`);
    exitCode = 1;
  }

  if (exitCode === 0) {
    process.stdout.write("OK: crates/ に unstaged/untracked ファイルおよび腐敗 stub はありません\n");
  }

  return exitCode;
}

if (import.meta.main) {
  process.exit(await main());
}

// ---------------------------------------------------------------------------
// stale test stub 検出ロジック
// ---------------------------------------------------------------------------

export async function detectStaleTestStubs(): Promise<StaleStub[]> {
  const lsProc = Bun.spawn(
    ["git", "ls-files", "--cached", "--others", "--exclude-standard", "crates/"],
    { stdout: "pipe", stderr: "pipe" },
  );
  const lsOut = await new Response(lsProc.stdout).text();
  await lsProc.exited;

  // crates/<crate>/tests/ 直下の .rs ファイルだけ対象 (integration test)
  // src/ 配下の単体テスト inline mod tests は対象外 (TEST_HARNESS = single source)
  const files = lsOut
    .split("\n")
    .map((l) => l.trim())
    .filter((p) => /^crates\/[^/]+\/tests\/[^/]+\.rs$/.test(p));

  const results: StaleStub[] = [];
  for (const path of files) {
    try {
      const text = readFileSync(path, "utf-8");
      if (isOnelineStub(text)) {
        results.push({ path, reason: "oneline_stub" });
      } else if (isAllTodoIgnoreStub(text)) {
        results.push({ path, reason: "all_todo_ignore" });
      }
    } catch {
      // ファイル読み取り失敗は無視 (削除済み等)
    }
  }
  return results;
}

// 1〜3 行 stub: 非空行が ≤ 3 でかつ
//   - "temporary" + ("probe" | "diagnostic" | "debug") のいずれかが共起、または
//   - "safely deleted" / "can be safely delet" の自己破棄表現
// を含む場合のみ検出。"Removed: covered by X" のような正当な tombstone は除外。
export function isOnelineStub(text: string): boolean {
  const nonEmpty = text
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.length > 0);
  if (nonEmpty.length === 0 || nonEmpty.length > 3) return false;
  const joined = nonEmpty.join(" ").toLowerCase();
  const hasTemporaryDiagnostic =
    /\btemporary\b/.test(joined) && /\b(probe|diagnostic|debug)\b/.test(joined);
  const hasSafelyDeleted = /safely (deleted|delete)\b|can be (safely )?delet/.test(joined);
  return hasTemporaryDiagnostic || hasSafelyDeleted;
}

// 「すべての #[test] が #[ignore] + 関数本体が実質 todo!() のみ」
// 偽陽性ガード: 本体に他の assertion/return/expr があれば対象外
export function isAllTodoIgnoreStub(text: string): boolean {
  // #[test] 個数
  const testFnCount = (text.match(/#\[test\]/g) || []).length;
  if (testFnCount === 0) return false;

  // #[test] と #[ignore...] の隣接ペア数
  const ignoreTestPattern = /#\[ignore[^\]]*\]\s*\n\s*#\[test\]|#\[test\]\s*\n\s*#\[ignore[^\]]*\]/g;
  const ignoreCount = (text.match(ignoreTestPattern) || []).length;
  if (ignoreCount !== testFnCount) return false;

  // 各 fn 本体が todo!() のみで構成されているかを波括弧ベースで簡易判定
  // 関数定義 fn xxx(...) { ... } を順に抽出
  const fnBodies = extractFnBodies(text);
  if (fnBodies.length !== testFnCount) return false;
  return fnBodies.every((body) => isBodyEffectivelyTodo(body));
}

// 関数本体を抽出 (波括弧バランスで対応)
// `fn name(...) { ... }` の `{` から対応する `}` までを返す
function extractFnBodies(text: string): string[] {
  const bodies: string[] = [];
  const fnRegex = /\bfn\s+\w+\s*\([^)]*\)\s*(?:->\s*[^{]+)?\{/g;
  let m: RegExpExecArray | null;
  while ((m = fnRegex.exec(text)) !== null) {
    const openIdx = m.index + m[0].length - 1; // `{` の位置
    let depth = 1;
    let i = openIdx + 1;
    while (i < text.length && depth > 0) {
      if (text[i] === "{") depth++;
      else if (text[i] === "}") depth--;
      i++;
    }
    if (depth === 0) bodies.push(text.slice(openIdx + 1, i - 1));
  }
  return bodies;
}

// 本体が空白・コメント・todo!()/unimplemented!() のみで構成されているか
function isBodyEffectivelyTodo(body: string): boolean {
  // コメント (// ... と /* ... */) を除去
  let stripped = body.replace(/\/\/[^\n]*/g, "").replace(/\/\*[\s\S]*?\*\//g, "");
  // 空白を圧縮
  stripped = stripped.replace(/\s+/g, "");
  // 終止セミコロンを許容
  stripped = stripped.replace(/;$/, "");
  return stripped === "todo!()" || stripped === "unimplemented!()";
}
