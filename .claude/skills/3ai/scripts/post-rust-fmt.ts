#!/usr/bin/env bun
// post-rust-fmt.ts — PostToolUse フック (#149)
// Edit/Write ツールで .rs ファイルを編集した後に cargo fmt --all を自動実行する。
// stdin から JSON を読み、file_path が *.rs なら fmt を実行。
// 失敗しても flow を block しない（stderr にログ出力して exit 0）。

import { resolve } from "node:path";

// repo root は本スクリプトから 4 階層上 (.claude/skills/3ai/scripts/ → repo root)
// 絶対パス固定を避けて任意のチェックアウト先で動作させる (Codex B-6 round 3 F01 指摘対応)
const REPO_ROOT = resolve(import.meta.dir, "../../../..");

/**
 * stdin JSON から .rs ファイルパスを抽出する純関数。
 * @param stdin - PostToolUse hook に流される JSON 文字列
 * @returns .rs ファイルパス または null（非対象・異常時）
 */
export function extractRustFilePath(stdin: string): string | null {
  try {
    const data = JSON.parse(stdin);
    const ti = data?.tool_input ?? {};
    const filePath = ti.file_path ?? "";
    if (typeof filePath === "string" && filePath.endsWith(".rs")) {
      return filePath;
    }
    return null;
  } catch {
    return null;
  }
}

async function main(): Promise<void> {
  const raw = await Bun.stdin.text();
  const rustPath = extractRustFilePath(raw);

  if (!rustPath) {
    process.exit(0);
  }

  const proc = Bun.spawn(["cargo", "fmt", "--all"], {
    cwd: REPO_ROOT,
    stderr: "pipe",
    stdout: "pipe",
  });

  const stderr = await new Response(proc.stderr).text();
  const exited = await proc.exited;

  if (exited !== 0) {
    console.error(`[post-rust-fmt] cargo fmt failed: ${stderr}`);
  }

  process.exit(0);
}

if (import.meta.main) {
  main();
}
