#!/usr/bin/env bun
// check-diff-coverage.ts — 差分行 (diff) の line coverage を閾値と比較 (Issue #318)
//
// 差分 coverage の考え方:
//   全コードベースの coverage が 45% でも問題ない。
//   本 Issue で追加・変更した行の中で ≥ 70% (default) が
//   テストで実行されていれば OK とする。
//
// 使い方:
//   bun check-diff-coverage.ts [--base <ref>] [--head <ref>]
//                              [--threshold <pct>] [--phase <N>]
//                              [--lcov-path <path>] [--skip-generate]
//
// exit:
//   0 → 閾値以上、または warn モード、または cargo-llvm-cov 未導入
//   1 → 閾値未満かつ blocking モード (phase >= 11)
//   2 → 環境エラー (git repo 外 / base ref 不明 / 使い方誤り)
//
// Phase 判定:
//   phase >= 11 → blocking (未達なら exit 1)
//   phase < 11 or null → warn-only (常に exit 0)
//   Phase 11 品質基盤で cargo-llvm-cov を本格導入する前提の暫定 gate。

import { spawnSync } from "child_process";
import { existsSync, readFileSync } from "fs";

// ---------- Types ----------

export interface AddedLine {
  file: string;
  line: number;
  text: string;
}

export interface CoverageResult {
  covered: number;
  total: number;
  ratio: number;
  uncovered: { file: string; line: number }[];
}

export interface CheckOpts {
  base: string;
  head: string;
  threshold: number; // percent (integer or float)
  phase: number | null;
  lcovPath: string;
  skipGenerate: boolean;
}

export interface RunResult {
  status: number;
  stdout: string;
  stderr: string;
}

export interface CheckDeps {
  runGit: (args: string[]) => RunResult;
  runCargo: (args: string[]) => RunResult;
  readLcov: (path: string) => string | null;
  writeStdout: (s: string) => void;
  writeStderr: (s: string) => void;
}

// ---------- Diff parser ----------

/** `git diff --unified=0 base...head` の出力を + 行 (新規/変更後) の一覧に変換する。
 *  各行は `file:line:text` の 3 つ組。text は先頭の `+` を除いた本文。 */
export function parseDiffLines(diffOutput: string): AddedLine[] {
  const results: AddedLine[] = [];
  const lines = diffOutput.split("\n");
  let currentFile: string | null = null;
  let currentLine = 0;

  for (const line of lines) {
    if (line.startsWith("diff --git ")) {
      currentFile = null;
      continue;
    }
    if (line.startsWith("+++ ")) {
      const path = line.slice(4).trim();
      if (path === "/dev/null") {
        currentFile = null;
      } else if (path.startsWith("b/")) {
        currentFile = path.slice(2);
      } else {
        currentFile = path;
      }
      continue;
    }
    if (line.startsWith("--- ") || line.startsWith("index ")) {
      continue;
    }
    if (line.startsWith("@@")) {
      // @@ -a[,b] +c[,d] @@ optional_context
      const m = line.match(/^@@\s+-\d+(?:,\d+)?\s+\+(\d+)(?:,\d+)?\s+@@/);
      if (m) {
        currentLine = parseInt(m[1], 10);
      }
      continue;
    }
    if (line.startsWith("\\")) {
      // "\ No newline at end of file"
      continue;
    }
    if (line.startsWith("+") && currentFile !== null) {
      results.push({ file: currentFile, line: currentLine, text: line.slice(1) });
      currentLine++;
    } else if (line.startsWith("-")) {
      // deletion: 新ファイル側の行番号は進まない
    } else if (line.startsWith(" ")) {
      // context 行 (unified=0 では通常出ないが念のため)
      currentLine++;
    }
  }
  return results;
}

// ---------- lcov parser ----------

/** lcov.info の SF/DA/end_of_record を parse し、file -> line -> hitcount に変換 */
export function parseLcov(lcovContent: string): Map<string, Map<number, number>> {
  const result = new Map<string, Map<number, number>>();
  let currentFile: string | null = null;
  let currentMap: Map<number, number> | null = null;

  for (const line of lcovContent.split("\n")) {
    if (line.startsWith("SF:")) {
      currentFile = line.slice(3).trim();
      currentMap = new Map();
      result.set(currentFile, currentMap);
    } else if (line === "end_of_record") {
      currentFile = null;
      currentMap = null;
    } else if (line.startsWith("DA:") && currentMap) {
      const rest = line.slice(3);
      const comma = rest.indexOf(",");
      if (comma < 0) continue;
      const lineNo = parseInt(rest.slice(0, comma), 10);
      // DA:line,hit[,checksum] — checksum があっても hit 部分は 2 番目のカラム
      const rest2 = rest.slice(comma + 1);
      const comma2 = rest2.indexOf(",");
      const hitStr = comma2 < 0 ? rest2 : rest2.slice(0, comma2);
      const hit = parseInt(hitStr, 10);
      if (Number.isFinite(lineNo) && Number.isFinite(hit)) {
        currentMap.set(lineNo, hit);
      }
    }
  }
  return result;
}

// ---------- Untestable line filter ----------

/** カバレッジ判定から除外すべき行の heuristic 判定。
 *  空行、コメント、use / mod / extern crate 宣言、attribute、単独の {} など。 */
export function shouldSkipLine(sourceLine: string): boolean {
  const trimmed = sourceLine.trim();
  if (trimmed === "") return true;
  if (trimmed.startsWith("//")) return true;
  if (trimmed.startsWith("/*") || trimmed.startsWith("*/") || trimmed.startsWith("* ") || trimmed === "*") return true;
  if (/^use\s/.test(trimmed)) return true;
  if (/^pub\s+use\s/.test(trimmed)) return true;
  if (/^pub(\([^)]*\))?\s+use\s/.test(trimmed)) return true;
  if (/^mod\s+\w+\s*;/.test(trimmed)) return true;
  if (/^pub(\([^)]*\))?\s+mod\s+\w+\s*;/.test(trimmed)) return true;
  if (/^extern\s+crate\s/.test(trimmed)) return true;
  if (trimmed === "{" || trimmed === "}" || trimmed === "};" || trimmed === "})" || trimmed === "});" || trimmed === "),") return true;
  if (trimmed.startsWith("#[") || trimmed.startsWith("#![")) return true;
  return false;
}

// ---------- Coverage compute ----------

/** diff で追加された行と lcov の hitcount を突き合わせ、
 *  カバレッジ率と未カバー行一覧を返す。 */
export function computeDiffCoverage(
  added: AddedLine[],
  lcov: Map<string, Map<number, number>>,
): CoverageResult {
  // diff の相対パスと lcov の絶対パスを suffix match で結ぶ
  const uniqueFiles = new Set(added.map((a) => a.file));
  const fileToHit = new Map<string, Map<number, number>>();
  for (const file of uniqueFiles) {
    if (lcov.has(file)) {
      fileToHit.set(file, lcov.get(file)!);
      continue;
    }
    for (const [lcovFile, hitMap] of lcov) {
      if (lcovFile.endsWith("/" + file)) {
        fileToHit.set(file, hitMap);
        break;
      }
    }
  }

  let covered = 0;
  let total = 0;
  const uncovered: { file: string; line: number }[] = [];

  for (const entry of added) {
    if (shouldSkipLine(entry.text)) continue;
    const hitMap = fileToHit.get(entry.file);
    if (!hitMap) continue; // lcov に無いファイルはスキップ (tests/ など非計測対象)
    const hit = hitMap.get(entry.line);
    if (hit === undefined) continue; // llvm-cov が該当行を計測対象外にした (attribute のみなど)
    total++;
    if (hit > 0) covered++;
    else uncovered.push({ file: entry.file, line: entry.line });
  }

  const ratio = total === 0 ? 1.0 : covered / total;
  return { covered, total, ratio, uncovered };
}

// ---------- Runner ----------

export async function runCheck(opts: CheckOpts, deps: CheckDeps): Promise<number> {
  const blocking = opts.phase !== null && opts.phase >= 11;

  // 1. cargo-llvm-cov が入っていなければ warn + exit 0
  const cargoCheck = deps.runCargo(["llvm-cov", "--version"]);
  if (cargoCheck.status !== 0) {
    deps.writeStdout("WARN: cargo-llvm-cov not installed, skipping diff coverage check\n");
    return 0;
  }

  // 2. lcov 読み込み。無ければ生成 (skipGenerate=false のとき)
  let lcovContent = deps.readLcov(opts.lcovPath);
  if (lcovContent === null && !opts.skipGenerate) {
    const gen = deps.runCargo(["llvm-cov", "--lcov", "--output-path", opts.lcovPath]);
    if (gen.status !== 0) {
      const msg = `WARN: cargo llvm-cov generation failed: ${gen.stderr.trim() || "(no stderr)"}\n`;
      if (blocking) {
        deps.writeStderr(msg);
        return 1;
      }
      deps.writeStdout(msg);
      return 0;
    }
    lcovContent = deps.readLcov(opts.lcovPath);
  }
  if (lcovContent === null) {
    const msg = `WARN: lcov file not found at ${opts.lcovPath}\n`;
    if (blocking) {
      deps.writeStderr(msg);
      return 1;
    }
    deps.writeStdout(msg);
    return 0;
  }

  // 3. git diff で変更行を取得 (crates/**/*.rs に限定)
  const diffRes = deps.runGit(["diff", "--unified=0", `${opts.base}...${opts.head}`]);
  if (diffRes.status !== 0) {
    deps.writeStderr(`ERROR: git diff failed: ${diffRes.stderr.trim()}\n`);
    return 2;
  }

  const addedAll = parseDiffLines(diffRes.stdout);
  const added = addedAll.filter((a) => a.file.startsWith("crates/") && a.file.endsWith(".rs"));
  if (added.length === 0) {
    deps.writeStdout("OK: no changed lines to cover\n");
    return 0;
  }

  const lcov = parseLcov(lcovContent);
  const result = computeDiffCoverage(added, lcov);

  if (result.total === 0) {
    deps.writeStdout("OK: no coverable lines in diff (all filtered)\n");
    return 0;
  }

  const pct = Math.round(result.ratio * 1000) / 10;
  if (result.ratio * 100 >= opts.threshold) {
    deps.writeStdout(
      `OK: diff coverage ${pct}% (${result.covered}/${result.total} lines) >= ${opts.threshold}%\n`,
    );
    return 0;
  }

  // 閾値未満: uncovered 一覧をファイル別に整形
  const byFile = new Map<string, number[]>();
  for (const u of result.uncovered) {
    const arr = byFile.get(u.file) ?? [];
    arr.push(u.line);
    byFile.set(u.file, arr);
  }
  const outLines: string[] = [];
  const level = blocking ? "ERROR" : "WARN";
  outLines.push(
    `${level}: diff coverage ${pct}% (${result.covered}/${result.total} lines) < ${opts.threshold}%`,
  );
  for (const [file, ls] of byFile) {
    const shown = ls.slice(0, 10).join(", ");
    const suffix = ls.length > 10 ? ", ..." : "";
    outLines.push(`  ${file}: ${ls.length} uncovered (${shown}${suffix})`);
  }
  const output = outLines.join("\n") + "\n";
  if (blocking) {
    deps.writeStderr(output);
    return 1;
  }
  deps.writeStdout(output);
  return 0;
}

// ---------- Default branch detection (共通パターン) ----------

/** origin/HEAD が指す branch を返す。解決失敗時は null。 */
export function detectDefaultBranch(cwd?: string): string | null {
  const r = spawnSync(
    "git",
    ["symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"],
    { encoding: "utf-8", cwd },
  );
  if (r.status !== 0) return null;
  const raw = (r.stdout ?? "").trim();
  if (!raw) return null;
  return raw.startsWith("origin/") ? raw.slice("origin/".length) : raw;
}

// ---------- CLI wrapper (production deps) ----------

function realDeps(): CheckDeps {
  return {
    runGit: (args) => {
      const r = spawnSync("git", args, { encoding: "utf-8", maxBuffer: 64 * 1024 * 1024 });
      return { status: r.status ?? -1, stdout: r.stdout ?? "", stderr: r.stderr ?? "" };
    },
    runCargo: (args) => {
      const r = spawnSync("cargo", args, { encoding: "utf-8", maxBuffer: 64 * 1024 * 1024 });
      return { status: r.status ?? -1, stdout: r.stdout ?? "", stderr: r.stderr ?? "" };
    },
    readLcov: (path) => {
      try {
        if (!existsSync(path)) return null;
        return readFileSync(path, "utf-8");
      } catch {
        return null;
      }
    },
    writeStdout: (s) => {
      process.stdout.write(s);
    },
    writeStderr: (s) => {
      process.stderr.write(s);
    },
  };
}

async function mainCli(argv: string[]): Promise<number> {
  let base: string | null = null;
  let head = "HEAD";
  let threshold = 70;
  let phase: number | null = null;
  let lcovPath = "target/llvm-cov/lcov.info";
  let skipGenerate = false;

  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--base") {
      base = argv[++i] ?? "";
    } else if (a === "--head") {
      head = argv[++i] ?? "HEAD";
    } else if (a === "--threshold") {
      const v = argv[++i] ?? "";
      const parsed = parseFloat(v);
      if (!Number.isFinite(parsed)) {
        process.stderr.write(`ERROR: --threshold expects a number, got '${v}'\n`);
        return 2;
      }
      threshold = parsed;
    } else if (a === "--phase") {
      const v = argv[++i] ?? "";
      const parsed = parseInt(v, 10);
      phase = Number.isFinite(parsed) ? parsed : null;
    } else if (a === "--lcov-path") {
      lcovPath = argv[++i] ?? lcovPath;
    } else if (a === "--skip-generate") {
      skipGenerate = true;
    } else if (a === "--help" || a === "-h") {
      process.stdout.write(
        "Usage: check-diff-coverage.ts [--base <ref>] [--head <ref>] " +
          "[--threshold <pct>] [--phase <N>] [--lcov-path <path>] [--skip-generate]\n",
      );
      return 0;
    } else {
      process.stderr.write(`Unknown arg: ${a}\n`);
      return 2;
    }
  }

  if (base === null) {
    base = detectDefaultBranch() ?? "main";
  }

  return runCheck({ base, head, threshold, phase, lcovPath, skipGenerate }, realDeps());
}

if (import.meta.main) {
  mainCli(process.argv.slice(2))
    .then((code) => process.exit(code))
    .catch((e) => {
      console.error(e);
      process.exit(2);
    });
}
