#!/usr/bin/env bun
// pre-step8-check.ts — STEP 8 前に crates/ の状態を検査する
// Usage: bun pre-step8-check.ts [--feature-dir <path>] [--allow-skeleton]
//
// チェック項目:
//   (a) crates/ 配下の unstaged/untracked ファイル
//   (b) crates/<crate>/tests/*.rs に「すべて #[ignore] + todo!()/unimplemented!() の腐敗 stub」
//       または「temporary/probe/safely-deleted を含む 1〜3 行 stub」が残っていないか (#139)
//
// 注 (Codex F01 review #139): (b) は /3ai STEP 8 文脈での実行を前提とする厳格判定。
// この文脈では STEP 5.5 acceptance skeleton は STEP 6 までに実装完了 (#[ignore] 解除)
// しているのが正常状態であり、STEP 8 時点で `#[ignore] + todo!()` が残る = 腐敗。
// /3ai 外の文脈 (進行中状態の確認等) で実行する場合は --allow-skeleton で (b) のうち
// all_todo_ignore 検出を抑止できる (oneline_stub は引き続き対象)。
//
// exit 0: 問題なし
// exit 1: いずれかが検出された (ガード違反)

import { readFileSync } from "fs";

export type StaleStub = { path: string; reason: "all_todo_ignore" | "oneline_stub" };

async function main(): Promise<number> {
  const cliArgs = process.argv.slice(2);
  let _featureDir = "";
  let allowSkeleton = false;

  for (let i = 0; i < cliArgs.length; i++) {
    if (cliArgs[i] === "--feature-dir") _featureDir = cliArgs[++i];
    else if (cliArgs[i] === "--allow-skeleton") allowSkeleton = true;
    // --auto-raise は廃止。フラグが渡されても無視する（後方互換）
    else if (cliArgs[i] === "--auto-raise") { /* no-op */ }
  }

  const proc = Bun.spawn(["git", "status", "--porcelain"], { stdout: "pipe", stderr: "pipe" });
  const output = await new Response(proc.stdout).text();
  const procExit = await proc.exited;
  if (procExit !== 0) {
    const err = await new Response(proc.stderr).text();
    process.stderr.write(`ERROR: \`git status --porcelain\` failed (exit ${procExit}):\n${err}\n`);
    return 1; // fail-closed
  }

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
  // --allow-skeleton 指定時は all_todo_ignore 検出を抑止 (oneline_stub は維持)
  let stalesRaw: StaleStub[];
  try {
    stalesRaw = await detectStaleTestStubs();
  } catch (e) {
    process.stderr.write(`ERROR: stale stub 検出失敗 (fail-closed): ${(e as Error).message}\n`);
    return 1;
  }
  const stales = allowSkeleton
    ? stalesRaw.filter((s) => s.reason !== "all_todo_ignore")
    : stalesRaw;

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
  const lsExit = await lsProc.exited;
  if (lsExit !== 0) {
    const err = await new Response(lsProc.stderr).text();
    throw new Error(`git ls-files failed (exit ${lsExit}): ${err.trim()}`);
  }

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

// 「すべての #[test] が #[ignore] + 関数本体が実質 todo!()/unimplemented!() のみ」
// helper 関数 (#[test] なし) は判定から除外。属性挿入 (#[cfg(...)] 等) や
// メッセージ付き todo!("msg") / unimplemented!("msg") も検出する。
export function isAllTodoIgnoreStub(text: string): boolean {
  const testFns = extractTestFunctions(text);
  if (testFns.length === 0) return false;
  return testFns.every((f) => f.hasIgnore && isBodyEffectivelyTodo(f.body));
}

type TestFn = { hasIgnore: boolean; body: string };

// `#[test]` または `#[*::test]` (例: tokio::test) が付いた関数のみを抽出。
// 各 fn の直前に並ぶ連続属性ブロック (#[ignore], #[cfg(...)] 等) を集めて
// hasIgnore を判定する。helper fn (test 属性を含まない、または無属性) は無視。
// async fn にも対応 (Codex F02 #139)。
// pub fn / pub(crate) fn 等の可視性修飾子も許容 (Codex F01 #139 round 5)。
// 関数本体抽出は string-aware / comment-aware brace matcher を使用
// (todo!("}") のような文字列内 brace で誤マッチしない)。
function extractTestFunctions(text: string): TestFn[] {
  const results: TestFn[] = [];
  // 連続する #[...] 属性ブロック + 可視性修飾子? + async? fn name(...) {
  // 属性間の改行・空白は許容
  const fnHead =
    /((?:#\[[^\]]*\]\s*)+)(?:pub(?:\(\s*(?:crate|super|self|in\s+[\w:]+)\s*\))?\s+)?(?:async\s+)?fn\s+\w+\s*\([^)]*\)\s*(?:->\s*[^{]+)?\{/g;
  let m: RegExpExecArray | null;
  while ((m = fnHead.exec(text)) !== null) {
    const attrBlock = m[1];
    // #[test] / #[<path>::test] / #[<path>::test(args...)] を許容
    // 例: #[tokio::test(flavor = "multi_thread")] / #[smol::test(...)]
    if (!/#\[(?:\w+::)*test(?:\([^\]]*\))?\]/.test(attrBlock)) continue;

    const hasIgnore = /#\[ignore(?:\s*=\s*[^\]]*)?\]/.test(attrBlock);

    // string/comment-aware brace matcher で関数本体を抽出
    const openIdx = m.index + m[0].length - 1;
    const closeIdx = findMatchingClose(text, openIdx);
    if (closeIdx !== -1) {
      results.push({ hasIgnore, body: text.slice(openIdx + 1, closeIdx) });
    }
  }
  return results;
}

// `{` の位置 openIdx から対応する `}` のインデックスを返す。
// 文字列リテラル (" .." と raw r#?"..."#?)、行コメント (//)、ブロックコメント (/* */)
// の中の brace は depth に影響させない。マッチが見つからなければ -1。
// (Codex F01 #139 round 5)
function findMatchingClose(text: string, openIdx: number): number {
  let i = openIdx + 1;
  let depth = 1;
  while (i < text.length && depth > 0) {
    const c = text[i];

    // ブロックコメント /* ... */
    if (c === "/" && text[i + 1] === "*") {
      const end = text.indexOf("*/", i + 2);
      if (end === -1) return -1;
      i = end + 2;
      continue;
    }
    // 行コメント // ...
    if (c === "/" && text[i + 1] === "/") {
      const nl = text.indexOf("\n", i + 2);
      if (nl === -1) return -1;
      i = nl + 1;
      continue;
    }
    // 文字 literal '..' (escape も考慮、ライフタイム ' は除外)
    if (c === "'") {
      // ライフタイム 'a 等: ' 直後が識別子で次が ' でない場合
      if (/[A-Za-z_]/.test(text[i + 1] || "") && text[i + 2] !== "'") {
        i++;
        continue;
      }
      // char literal '\\n' 等
      const end = scanCharLit(text, i);
      if (end === -1) return -1;
      i = end;
      continue;
    }
    // raw string r#?"..."#?
    if ((c === "r" || c === "b") && (text[i + 1] === '"' || text[i + 1] === "#")) {
      const end = scanRawString(text, i);
      if (end !== -1) {
        i = end;
        continue;
      }
    }
    // 通常の文字列リテラル "..."
    if (c === '"') {
      const end = scanString(text, i);
      if (end === -1) return -1;
      i = end;
      continue;
    }

    if (c === "{") depth++;
    else if (c === "}") depth--;
    i++;
  }
  return depth === 0 ? i - 1 : -1;
}

// "..." をスキャン。\" でエスケープされた " は文字列の一部として扱う。
function scanString(text: string, startIdx: number): number {
  let i = startIdx + 1;
  while (i < text.length) {
    const c = text[i];
    if (c === "\\") {
      i += 2;
      continue;
    }
    if (c === '"') return i + 1;
    i++;
  }
  return -1;
}

// '...' (char literal) をスキャン。
function scanCharLit(text: string, startIdx: number): number {
  let i = startIdx + 1;
  while (i < text.length) {
    const c = text[i];
    if (c === "\\") {
      i += 2;
      continue;
    }
    if (c === "'") return i + 1;
    i++;
  }
  return -1;
}

// r"..." / r#"..."# / b"..." / br#"..."# 等の raw / byte string をスキャン。
function scanRawString(text: string, startIdx: number): number {
  let i = startIdx;
  if (text[i] === "b") i++;
  if (text[i] === "r") i++;
  // # の連続を数える
  let hashes = 0;
  while (text[i] === "#") {
    hashes++;
    i++;
  }
  if (text[i] !== '"') return -1;
  i++;
  const closer = '"' + "#".repeat(hashes);
  const end = text.indexOf(closer, i);
  if (end === -1) return -1;
  return end + closer.length;
}

// 本体が空白・コメント・todo!(...)/unimplemented!(...) のみで構成されているか
// メッセージ付き呼び出し (todo!("WIP")) と全マクロ delimiter 形式
// (todo!() / todo!{} / todo![]) を検出対象 (Codex F02 #139 round 3)。
// 文字列リテラル内の // や /* はコメントとして扱わない
// (Codex F01 #139 round 5)。
function isBodyEffectivelyTodo(body: string): boolean {
  // string-aware にコメントを除去
  let stripped = stripCommentsStringAware(body);
  // 空白を圧縮
  stripped = stripped.replace(/\s+/g, "");
  // 終止セミコロンを許容
  stripped = stripped.replace(/;$/, "");
  // todo!() / todo!{} / todo![] (および unimplemented!) のすべての delimiter 形式を許容
  return /^(?:todo|unimplemented)!(?:\(.*\)|\{.*\}|\[.*\])$/.test(stripped);
}

// 文字列リテラル・char literal・raw string を保持したまま // と /* */ コメントを除去。
function stripCommentsStringAware(text: string): string {
  let out = "";
  let i = 0;
  while (i < text.length) {
    const c = text[i];

    // ブロックコメント /* ... */
    if (c === "/" && text[i + 1] === "*") {
      const end = text.indexOf("*/", i + 2);
      if (end === -1) { i = text.length; continue; }
      i = end + 2;
      continue;
    }
    // 行コメント // ...
    if (c === "/" && text[i + 1] === "/") {
      const nl = text.indexOf("\n", i + 2);
      i = nl === -1 ? text.length : nl;
      continue;
    }
    // 文字列リテラル "..."
    if (c === '"') {
      const end = scanString(text, i);
      const stop = end === -1 ? text.length : end;
      out += text.slice(i, stop);
      i = stop;
      continue;
    }
    // raw string r#?"..."#? / byte string b"..." / br#?"..."#?
    if ((c === "r" || c === "b") && (text[i + 1] === '"' || text[i + 1] === "#")) {
      const end = scanRawString(text, i);
      if (end !== -1) {
        out += text.slice(i, end);
        i = end;
        continue;
      }
    }
    // char literal '...' (ライフタイム除外)
    if (c === "'") {
      if (/[A-Za-z_]/.test(text[i + 1] || "") && text[i + 2] !== "'") {
        out += c;
        i++;
        continue;
      }
      const end = scanCharLit(text, i);
      const stop = end === -1 ? text.length : end;
      out += text.slice(i, stop);
      i = stop;
      continue;
    }

    out += c;
    i++;
  }
  return out;
}
