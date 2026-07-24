#!/usr/bin/env bun
// lint-fuzz-corpus-updated.ts — YAML schema 変更時に fuzz corpus seed 追加を強制
//
// 意図:
//   engawa-format の `Feature` enum や `Document` 型 (= .engawa YAML schema の source
//   of truth) が拡張されたら、fuzz target が新 variant/field をカバーするための
//   seed を `fuzz/corpus/parser/` に必ず追加させる。
//   追加漏れを放置すると fuzz が新形状に到達しないまま「回っている」状態が続き、
//   parser 側 regression を捕まえられない。
//
// 使い方:
//   bun lint-fuzz-corpus-updated.ts                                # <default>...HEAD
//   bun lint-fuzz-corpus-updated.ts --base HEAD~5 --head HEAD
//   bun lint-fuzz-corpus-updated.ts --corpus-dir fuzz/corpus/parser --schema-glob "crates/engawa-format/src/**/*.rs"
//
// 挙動 (簡略化):
//   1. schema file (glob 一致) が diff に無い → OK (exit 0)
//   2. schema file はあるが実質的変更 (新 variant / 新 struct field) 無し → OK (exit 0)
//   3. fuzz/corpus/parser が存在しない (Phase 11 品質基盤未着手) → WARN (exit 0)
//   4. schema 実質変更あり + corpus 追加あり (git diff --diff-filter=A) → OK (exit 0)
//   5. schema 実質変更あり + corpus 追加無し → ERROR (exit 1) — blocking
//
// exit:
//   0 → 問題なし (OK / WARN)
//   1 → schema 実質変更に対応する fuzz corpus seed が追加されていない
//   2 → 環境エラー (git 未実行 / ref 不明 / 引数誤り)

import { spawnSync } from "child_process";
import { existsSync } from "fs";

// --- Pure functions (testable) ---

/**
 * `git diff --name-only` または `--name-status` の出力から path 列を抽出する。
 * - name-only 形式 (tab 無し): 各行がそのまま path
 * - name-status 形式 (先頭に A/M/D/R... と tab): status 列を解釈
 *   - filterAdd=true なら A のみ抽出
 *   - filterAdd=false / undefined なら全 status を抽出 (rename は new path を採用)
 */
export function parseDiffNames(diffOutput: string, filterAdd?: boolean): string[] {
  const out: string[] = [];
  for (const rawLine of diffOutput.split("\n")) {
    const line = rawLine.replace(/\r$/, "");
    if (!line.trim()) continue;
    const tabIdx = line.indexOf("\t");
    if (tabIdx < 0) {
      // name-only
      out.push(line.trim());
      continue;
    }
    const status = line.slice(0, tabIdx);
    const rest = line.slice(tabIdx + 1);
    // R100\told\tnew — take the new path
    const cols = rest.split("\t");
    const path = cols[cols.length - 1]?.trim() ?? "";
    if (!path) continue;
    if (filterAdd) {
      if (status.startsWith("A")) out.push(path);
    } else {
      out.push(path);
    }
  }
  return out;
}

/**
 * unified diff (`git diff <base>...<head> -- <schema-glob>`) の中で、
 * 新 enum variant または 新 struct field と見なせる追加行が存在するか判定する。
 *
 * 判定ヒューリスティック (誤検出より偽陽性寄り):
 *   - 新 enum variant : `^\+\s{4,}[A-Z]\w+\s*[({]`  (例: `+    NewOp {`)
 *   - 新 struct field: `^\+\s{4,}pub\s+\w+:\s+`     (例: `+    pub new_field: u32,`)
 * どちらにも該当しない ///-doc-only / whitespace-only 変更は false を返す。
 */
export function detectSubstantiveSchemaChange(diffUnifiedOutput: string): boolean {
  // 4 空白以上 (enum variant / struct field の一般的インデント)
  const enumVariantRe = /^\+\s{4,}[A-Z]\w+\s*[({]/;
  const structFieldRe = /^\+\s{4,}(?:pub(?:\([^)]*\))?\s+)\w+\s*:\s+\S/;
  for (const rawLine of diffUnifiedOutput.split("\n")) {
    const line = rawLine.replace(/\r$/, "");
    // diff header (+++ b/xxx) は除外
    if (line.startsWith("+++")) continue;
    if (!line.startsWith("+")) continue;
    if (enumVariantRe.test(line)) return true;
    if (structFieldRe.test(line)) return true;
  }
  return false;
}

/**
 * glob パターン (例 `crates/engawa-format/src/**\/*.rs`) にマッチする path のみを抽出。
 * 内部で Bun.Glob を使用。
 */
export function filterSchemaFiles(paths: string[], globPattern: string): string[] {
  const glob = new Bun.Glob(globPattern);
  return paths.filter((p) => glob.match(p));
}

// --- Runner (composable with injected deps) ---

export type CheckOptions = {
  base: string;
  head: string;
  corpusDir: string;
  schemaGlob: string;
};

export type CheckDeps = {
  /** git diff --name-status --diff-filter=... <base>...<head> の生出力 */
  runDiffNames: (base: string, head: string) => string;
  /** git diff <base>...<head> -- <schemaPaths> の unified diff 出力 */
  runDiffContent: (base: string, head: string, paths: string[]) => string;
  /** git diff --name-status --diff-filter=A -- <corpusDir> の出力 */
  runDiffCorpusAdds: (base: string, head: string, corpusDir: string) => string;
  /** dir が存在するか */
  dirExists: (path: string) => boolean;
};

export type CheckResult = {
  exitCode: 0 | 1;
  status: "ok-no-schema-change" | "ok-non-substantive" | "warn-fuzz-missing" | "ok-seed-added" | "error-seed-missing";
  message: string;
};

export function runCheck(opts: CheckOptions, deps: CheckDeps): CheckResult {
  // 1. schema file の diff (name-status; add/mod/rename 全部含む)
  const rawNames = deps.runDiffNames(opts.base, opts.head);
  const changedAll = parseDiffNames(rawNames, false);
  const schemaChanged = filterSchemaFiles(changedAll, opts.schemaGlob);

  if (schemaChanged.length === 0) {
    return {
      exitCode: 0,
      status: "ok-no-schema-change",
      message: "OK: schema 未変更、fuzz corpus 更新不要",
    };
  }

  // 2. 実質的変更判定 (unified diff を走査)
  const diffContent = deps.runDiffContent(opts.base, opts.head, schemaChanged);
  const substantive = detectSubstantiveSchemaChange(diffContent);
  if (!substantive) {
    return {
      exitCode: 0,
      status: "ok-non-substantive",
      message:
        "OK: schema file 変更あり、ただし新 variant/field なし (doc-only or refactor)",
    };
  }

  // 3. fuzz corpus dir 存在確認 (Phase 11 品質基盤前提)
  if (!deps.dirExists(opts.corpusDir)) {
    return {
      exitCode: 0,
      status: "warn-fuzz-missing",
      message: `WARN: ${opts.corpusDir} 未セットアップ (Phase 11 品質基盤前提)`,
    };
  }

  // 4. corpus dir 内の新規 seed (A entry) 追加数
  const rawCorpusAdds = deps.runDiffCorpusAdds(opts.base, opts.head, opts.corpusDir);
  const corpusAdds = parseDiffNames(rawCorpusAdds, true);

  if (corpusAdds.length > 0) {
    return {
      exitCode: 0,
      status: "ok-seed-added",
      message: `OK: fuzz corpus に ${corpusAdds.length} 件の新 seed 追加確認`,
    };
  }

  // 5. schema が実質変わったのに seed 追加が無い → blocking
  const changedList = schemaChanged.map((p) => `  - ${p}`).join("\n");
  const message = [
    "ERROR: schema 実質変更 (新 variant / 新 struct field) を検出したが、",
    `       ${opts.corpusDir} に新規 seed が追加されていません。`,
    "",
    "変更された schema ファイル:",
    changedList,
    "",
    "対処:",
    `  1. 新 variant/field を含む最小 .engawa YAML を書き起こす`,
    `  2. ${opts.corpusDir}/ に seed_<新形状名>.engawa として置く`,
    `  3. git add ${opts.corpusDir}/seed_<...>.engawa して再実行`,
    "",
    "意図的にスキップする場合は plan.md に「fuzz seed 追加スキップ: <理由>」を明記。",
  ].join("\n");
  return { exitCode: 1, status: "error-seed-missing", message };
}

// --- git 実行系 (テスト対象外; 本体でのみ使う) ---

function isInsideGitRepo(cwd?: string): boolean {
  const r = spawnSync("git", ["rev-parse", "--is-inside-work-tree"], {
    encoding: "utf-8",
    cwd,
  });
  return r.status === 0 && (r.stdout ?? "").trim() === "true";
}

function refExists(ref: string, cwd?: string): boolean {
  const r = spawnSync("git", ["rev-parse", "--verify", "--quiet", `${ref}^{commit}`], {
    encoding: "utf-8",
    cwd,
  });
  return r.status === 0;
}

/** origin/HEAD が指す branch 名を返す。解決失敗時 null。
 *  lint-regression-preserved.ts の detectDefaultBranch と同ロジック。 */
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

function runGitDiffNameStatus(base: string, head: string, cwd?: string): string {
  const r = spawnSync(
    "git",
    ["diff", "--name-status", `${base}...${head}`],
    { encoding: "utf-8", cwd, maxBuffer: 64 * 1024 * 1024 },
  );
  if (r.status !== 0) {
    throw new Error(`git diff --name-status failed (${r.status}): ${r.stderr ?? ""}`);
  }
  return r.stdout ?? "";
}

function runGitDiffContent(
  base: string,
  head: string,
  paths: string[],
  cwd?: string,
): string {
  if (paths.length === 0) return "";
  const r = spawnSync(
    "git",
    ["diff", "--unified=0", `${base}...${head}`, "--", ...paths],
    { encoding: "utf-8", cwd, maxBuffer: 64 * 1024 * 1024 },
  );
  if (r.status !== 0) {
    throw new Error(`git diff (content) failed (${r.status}): ${r.stderr ?? ""}`);
  }
  return r.stdout ?? "";
}

function runGitDiffCorpusAdds(
  base: string,
  head: string,
  corpusDir: string,
  cwd?: string,
): string {
  const r = spawnSync(
    "git",
    ["diff", "--name-status", "--diff-filter=A", `${base}...${head}`, "--", corpusDir],
    { encoding: "utf-8", cwd, maxBuffer: 64 * 1024 * 1024 },
  );
  if (r.status !== 0) {
    throw new Error(`git diff (corpus adds) failed (${r.status}): ${r.stderr ?? ""}`);
  }
  return r.stdout ?? "";
}

// --- CLI ---

async function main(argv: string[]): Promise<number> {
  let base: string | null = null;
  let head = "HEAD";
  let corpusDir = "fuzz/corpus/parser";
  let schemaGlob = "crates/engawa-format/src/**/*.rs";

  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--base") base = argv[++i] ?? "";
    else if (a === "--head") head = argv[++i] ?? "HEAD";
    else if (a === "--corpus-dir") corpusDir = argv[++i] ?? corpusDir;
    else if (a === "--schema-glob") schemaGlob = argv[++i] ?? schemaGlob;
    else if (a === "--help" || a === "-h") {
      process.stdout.write(
        "Usage: lint-fuzz-corpus-updated.ts [--base <ref>] [--head <ref>] [--corpus-dir <path>] [--schema-glob <pattern>]\n",
      );
      return 0;
    } else {
      process.stderr.write(`Unknown arg: ${a}\n`);
      return 2;
    }
  }

  if (!isInsideGitRepo()) {
    process.stderr.write("ERROR: git repo の外で実行されました\n");
    return 2;
  }

  if (base === null) {
    base = detectDefaultBranch() ?? "main";
  }

  if (!refExists(base)) {
    process.stderr.write(`ERROR: base ref '${base}' が見つかりません\n`);
    return 2;
  }
  if (!refExists(head)) {
    process.stderr.write(`ERROR: head ref '${head}' が見つかりません\n`);
    return 2;
  }

  let result: CheckResult;
  try {
    result = runCheck(
      { base, head, corpusDir, schemaGlob },
      {
        runDiffNames: (b, h) => runGitDiffNameStatus(b, h),
        runDiffContent: (b, h, paths) => runGitDiffContent(b, h, paths),
        runDiffCorpusAdds: (b, h, dir) => runGitDiffCorpusAdds(b, h, dir),
        dirExists: (p) => existsSync(p),
      },
    );
  } catch (e) {
    process.stderr.write(`ERROR: ${(e as Error).message}\n`);
    return 2;
  }

  if (result.exitCode === 0) {
    process.stdout.write(result.message + "\n");
  } else {
    process.stderr.write(result.message + "\n");
  }
  return result.exitCode;
}

if (import.meta.main) {
  main(process.argv.slice(2))
    .then((code) => process.exit(code))
    .catch((e) => {
      console.error(e);
      process.exit(2);
    });
}
