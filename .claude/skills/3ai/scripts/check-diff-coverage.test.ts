// check-diff-coverage.test.ts — Phase C テスト設計 T04-T06 + bonus (Issue #318)

import { describe, expect, test } from "bun:test";
import {
  computeDiffCoverage,
  parseDiffLines,
  parseLcov,
  runCheck,
  shouldSkipLine,
  type CheckDeps,
  type CheckOpts,
  type RunResult,
} from "./check-diff-coverage.ts";

// ---------- Helpers ----------

interface MockOverrides {
  runGit?: (args: string[]) => RunResult;
  runCargo?: (args: string[]) => RunResult;
  readLcov?: (path: string) => string | null;
}

function makeDeps(overrides: MockOverrides = {}): {
  deps: CheckDeps;
  stdout: string[];
  stderr: string[];
} {
  const stdout: string[] = [];
  const stderr: string[] = [];
  const deps: CheckDeps = {
    runGit: overrides.runGit ?? (() => ({ status: 0, stdout: "", stderr: "" })),
    runCargo:
      overrides.runCargo ??
      ((args) => {
        if (args[0] === "llvm-cov" && args[1] === "--version") {
          return { status: 0, stdout: "cargo-llvm-cov 0.5.0\n", stderr: "" };
        }
        return { status: 0, stdout: "", stderr: "" };
      }),
    readLcov: overrides.readLcov ?? (() => ""),
    writeStdout: (s) => {
      stdout.push(s);
    },
    writeStderr: (s) => {
      stderr.push(s);
    },
  };
  return { deps, stdout, stderr };
}

function baseOpts(over: Partial<CheckOpts> = {}): CheckOpts {
  return {
    base: "main",
    head: "HEAD",
    threshold: 70,
    phase: null,
    lcovPath: "/tmp/lcov.info",
    skipGenerate: true,
    ...over,
  };
}

/** N 行の code-like な追加行を持つ diff 出力を生成 (すべて let 文で shouldSkipLine を通す) */
function makeDiffOutput(file: string, startLine: number, count: number): string {
  const lines: string[] = [
    `diff --git a/${file} b/${file}`,
    `--- a/${file}`,
    `+++ b/${file}`,
    `@@ -${startLine - 1},0 +${startLine},${count} @@`,
  ];
  for (let i = 0; i < count; i++) {
    lines.push(`+    let x${i} = ${i};`);
  }
  return lines.join("\n") + "\n";
}

/** file の行 [startLine, startLine+count) のうち先頭 hits 個を hit=1、残りを hit=0 とした lcov */
function makeLcovOutput(file: string, startLine: number, count: number, hits: number): string {
  const lines: string[] = [`SF:/abs/path/${file}`];
  for (let i = 0; i < count; i++) {
    const hit = i < hits ? 1 : 0;
    lines.push(`DA:${startLine + i},${hit}`);
  }
  lines.push("end_of_record");
  return lines.join("\n") + "\n";
}

// ---------- parseDiffLines ----------

describe("parseDiffLines", () => {
  test("hunk with count: @@ -10,0 +11,3 @@ → 3 added lines starting at 11", () => {
    const diff = [
      "diff --git a/crates/foo/src/lib.rs b/crates/foo/src/lib.rs",
      "--- a/crates/foo/src/lib.rs",
      "+++ b/crates/foo/src/lib.rs",
      "@@ -10,0 +11,3 @@",
      "+let a = 1;",
      "+let b = 2;",
      "+let c = 3;",
    ].join("\n");
    const added = parseDiffLines(diff);
    expect(added).toEqual([
      { file: "crates/foo/src/lib.rs", line: 11, text: "let a = 1;" },
      { file: "crates/foo/src/lib.rs", line: 12, text: "let b = 2;" },
      { file: "crates/foo/src/lib.rs", line: 13, text: "let c = 3;" },
    ]);
  });

  test("hunk without count: @@ -22,0 +23 @@ → 1 added line at 23", () => {
    const diff = [
      "diff --git a/x.rs b/x.rs",
      "--- a/x.rs",
      "+++ b/x.rs",
      "@@ -22,0 +23 @@ import foo",
      "+use bar::baz;",
    ].join("\n");
    const added = parseDiffLines(diff);
    expect(added).toEqual([{ file: "x.rs", line: 23, text: "use bar::baz;" }]);
  });

  test("mixed - and + in same hunk: only + counted, line numbers advance only on +", () => {
    const diff = [
      "diff --git a/x.rs b/x.rs",
      "--- a/x.rs",
      "+++ b/x.rs",
      "@@ -10,2 +10,2 @@",
      "-old_a",
      "-old_b",
      "+new_a",
      "+new_b",
    ].join("\n");
    const added = parseDiffLines(diff);
    expect(added).toEqual([
      { file: "x.rs", line: 10, text: "new_a" },
      { file: "x.rs", line: 11, text: "new_b" },
    ]);
  });

  test("deletion-only file: +++ /dev/null → no added lines", () => {
    const diff = [
      "diff --git a/x.rs b/x.rs",
      "--- a/x.rs",
      "+++ /dev/null",
      "@@ -1,3 +0,0 @@",
      "-a",
      "-b",
      "-c",
    ].join("\n");
    expect(parseDiffLines(diff)).toEqual([]);
  });

  test("'\\ No newline at end of file' is ignored", () => {
    const diff = [
      "diff --git a/x.rs b/x.rs",
      "--- a/x.rs",
      "+++ b/x.rs",
      "@@ -0,0 +1 @@",
      "+let a = 1;",
      "\\ No newline at end of file",
    ].join("\n");
    expect(parseDiffLines(diff)).toEqual([{ file: "x.rs", line: 1, text: "let a = 1;" }]);
  });

  test("empty diff → []", () => {
    expect(parseDiffLines("")).toEqual([]);
  });
});

// ---------- shouldSkipLine ----------

describe("shouldSkipLine", () => {
  test("blank / whitespace-only", () => {
    expect(shouldSkipLine("")).toBe(true);
    expect(shouldSkipLine("   ")).toBe(true);
  });
  test("line comments", () => {
    expect(shouldSkipLine("// this is a comment")).toBe(true);
    expect(shouldSkipLine("    // indented")).toBe(true);
  });
  test("use / pub use / mod / extern crate", () => {
    expect(shouldSkipLine("use foo::bar;")).toBe(true);
    expect(shouldSkipLine("    use foo::bar;")).toBe(true);
    expect(shouldSkipLine("pub use crate::x::y;")).toBe(true);
    expect(shouldSkipLine("pub(crate) use crate::x::y;")).toBe(true);
    expect(shouldSkipLine("mod foo;")).toBe(true);
    expect(shouldSkipLine("pub mod foo;")).toBe(true);
    expect(shouldSkipLine("extern crate alloc;")).toBe(true);
  });
  test("single-char scope markers", () => {
    expect(shouldSkipLine("{")).toBe(true);
    expect(shouldSkipLine("}")).toBe(true);
    expect(shouldSkipLine("};")).toBe(true);
    expect(shouldSkipLine("    })")).toBe(true);
  });
  test("attributes", () => {
    expect(shouldSkipLine("#[derive(Debug)]")).toBe(true);
    expect(shouldSkipLine("#![allow(unused)]")).toBe(true);
  });
  test("real code is NOT skipped", () => {
    expect(shouldSkipLine("let x = 1;")).toBe(false);
    expect(shouldSkipLine("fn foo() -> i32 { 1 }")).toBe(false);
    expect(shouldSkipLine("    pub fn bar(&self) -> u32 {")).toBe(false);
    expect(shouldSkipLine("    if x > 0 { return 1; }")).toBe(false);
  });
});

// ---------- parseLcov ----------

describe("parseLcov (T_bonus_parse_lcov)", () => {
  test("SF/DA/end_of_record を正しく分解", () => {
    const lcov = [
      "SF:/abs/crates/foo/src/lib.rs",
      "DA:1,5",
      "DA:2,0",
      "DA:3,12",
      "end_of_record",
      "SF:/abs/crates/bar/src/lib.rs",
      "DA:10,1",
      "end_of_record",
    ].join("\n");
    const parsed = parseLcov(lcov);
    expect(parsed.size).toBe(2);
    const foo = parsed.get("/abs/crates/foo/src/lib.rs")!;
    expect(foo.get(1)).toBe(5);
    expect(foo.get(2)).toBe(0);
    expect(foo.get(3)).toBe(12);
    const bar = parsed.get("/abs/crates/bar/src/lib.rs")!;
    expect(bar.get(10)).toBe(1);
  });

  test("DA:line,hit,checksum のチェックサム付き形式でも hit だけ取る", () => {
    const lcov = ["SF:/x.rs", "DA:5,3,abc123", "end_of_record"].join("\n");
    const parsed = parseLcov(lcov);
    expect(parsed.get("/x.rs")!.get(5)).toBe(3);
  });

  test("空入力 → 空 map", () => {
    expect(parseLcov("").size).toBe(0);
  });
});

// ---------- computeDiffCoverage ----------

describe("computeDiffCoverage", () => {
  test("suffix match で lcov (絶対パス) と diff (相対パス) を突き合わせる", () => {
    const added = [
      { file: "crates/foo/src/lib.rs", line: 11, text: "let a = 1;" },
      { file: "crates/foo/src/lib.rs", line: 12, text: "let b = 2;" },
    ];
    const lcov = new Map<string, Map<number, number>>([
      [
        "/home/user/proj/crates/foo/src/lib.rs",
        new Map<number, number>([
          [11, 1],
          [12, 0],
        ]),
      ],
    ]);
    const r = computeDiffCoverage(added, lcov);
    expect(r.covered).toBe(1);
    expect(r.total).toBe(2);
    expect(r.ratio).toBeCloseTo(0.5);
    expect(r.uncovered).toEqual([{ file: "crates/foo/src/lib.rs", line: 12 }]);
  });

  test("T_bonus_skip_lines: use/comment/blank/attribute はカウントしない", () => {
    const added = [
      { file: "crates/foo/src/lib.rs", line: 11, text: "use crate::bar;" },
      { file: "crates/foo/src/lib.rs", line: 12, text: "" },
      { file: "crates/foo/src/lib.rs", line: 13, text: "// comment" },
      { file: "crates/foo/src/lib.rs", line: 14, text: "#[derive(Debug)]" },
      { file: "crates/foo/src/lib.rs", line: 15, text: "let real = 1;" },
    ];
    // lcov marks all as uncovered — but only the "let real = 1;" line should count
    const lcov = new Map<string, Map<number, number>>([
      [
        "/abs/crates/foo/src/lib.rs",
        new Map<number, number>([
          [11, 0],
          [12, 0],
          [13, 0],
          [14, 0],
          [15, 0],
        ]),
      ],
    ]);
    const r = computeDiffCoverage(added, lcov);
    expect(r.total).toBe(1);
    expect(r.covered).toBe(0);
    expect(r.uncovered).toEqual([{ file: "crates/foo/src/lib.rs", line: 15 }]);
  });

  test("lcov に無いファイルはスキップ (tests/ など非計測対象)", () => {
    const added = [{ file: "crates/foo/tests/it.rs", line: 5, text: "let a = 1;" }];
    const lcov = new Map<string, Map<number, number>>();
    const r = computeDiffCoverage(added, lcov);
    expect(r.total).toBe(0);
    expect(r.ratio).toBe(1.0);
  });

  test("lcov に無い行はスキップ (attribute のみなど計測対象外)", () => {
    const added = [
      { file: "crates/foo/src/lib.rs", line: 11, text: "let a = 1;" },
      { file: "crates/foo/src/lib.rs", line: 12, text: "let b = 2;" },
    ];
    const lcov = new Map<string, Map<number, number>>([
      ["/abs/crates/foo/src/lib.rs", new Map<number, number>([[11, 1]])],
    ]);
    const r = computeDiffCoverage(added, lcov);
    expect(r.total).toBe(1);
    expect(r.covered).toBe(1);
  });
});

// ---------- runCheck: full flow with mocked deps ----------

describe("runCheck (integration with mocked deps)", () => {
  test("T04_diff_coverage_pass: 10 lines, 8/10 hit, phase=11, threshold=70 → exit 0", async () => {
    const file = "crates/foo/src/lib.rs";
    const { deps, stdout, stderr } = makeDeps({
      runGit: () => ({ status: 0, stdout: makeDiffOutput(file, 11, 10), stderr: "" }),
      readLcov: () => makeLcovOutput(file, 11, 10, 8),
    });
    const rc = await runCheck(baseOpts({ phase: 11, threshold: 70 }), deps);
    expect(rc).toBe(0);
    expect(stderr.join("")).toBe("");
    const out = stdout.join("");
    expect(out).toContain("OK: diff coverage 80%");
    expect(out).toContain("(8/10 lines)");
    expect(out).toContain(">= 70%");
  });

  test("T05_diff_coverage_fail: 10 lines, 6/10 hit, phase=11, threshold=70 → exit 1", async () => {
    const file = "crates/foo/src/lib.rs";
    const { deps, stdout, stderr } = makeDeps({
      runGit: () => ({ status: 0, stdout: makeDiffOutput(file, 11, 10), stderr: "" }),
      readLcov: () => makeLcovOutput(file, 11, 10, 6),
    });
    const rc = await runCheck(baseOpts({ phase: 11, threshold: 70 }), deps);
    expect(rc).toBe(1);
    expect(stdout.join("")).toBe("");
    const err = stderr.join("");
    expect(err).toContain("ERROR: diff coverage 60%");
    expect(err).toContain("(6/10 lines)");
    expect(err).toContain("< 70%");
    // uncovered breakdown includes the file and 4 uncovered line numbers
    expect(err).toContain(file);
    expect(err).toContain("4 uncovered");
  });

  test("T06_diff_coverage_llvm_cov_missing: cargo llvm-cov --version fails → warn + exit 0", async () => {
    const { deps, stdout, stderr } = makeDeps({
      runCargo: (args) => {
        if (args[0] === "llvm-cov" && args[1] === "--version") {
          return { status: 101, stdout: "", stderr: "error: no such command: `llvm-cov`\n" };
        }
        return { status: 0, stdout: "", stderr: "" };
      },
    });
    const rc = await runCheck(baseOpts({ phase: 11, threshold: 70 }), deps);
    expect(rc).toBe(0);
    expect(stderr.join("")).toBe("");
    expect(stdout.join("")).toContain("WARN: cargo-llvm-cov not installed");
  });

  test("T_bonus_phase10_below_threshold: 5/10 hit + phase=10 → exit 0 (warn only)", async () => {
    const file = "crates/foo/src/lib.rs";
    const { deps, stdout, stderr } = makeDeps({
      runGit: () => ({ status: 0, stdout: makeDiffOutput(file, 11, 10), stderr: "" }),
      readLcov: () => makeLcovOutput(file, 11, 10, 5),
    });
    const rc = await runCheck(baseOpts({ phase: 10, threshold: 70 }), deps);
    expect(rc).toBe(0);
    expect(stderr.join("")).toBe("");
    const out = stdout.join("");
    expect(out).toContain("WARN: diff coverage 50%");
    expect(out).not.toContain("ERROR");
  });

  test("T_bonus_phase_null_below_threshold: phase 未指定 → warn only", async () => {
    const file = "crates/foo/src/lib.rs";
    const { deps, stdout, stderr } = makeDeps({
      runGit: () => ({ status: 0, stdout: makeDiffOutput(file, 11, 10), stderr: "" }),
      readLcov: () => makeLcovOutput(file, 11, 10, 5),
    });
    const rc = await runCheck(baseOpts({ phase: null, threshold: 70 }), deps);
    expect(rc).toBe(0);
    expect(stdout.join("")).toContain("WARN: diff coverage 50%");
  });

  test("T_bonus_no_diff: 空 diff → exit 0 + 'no changed lines to cover'", async () => {
    const { deps, stdout } = makeDeps({
      runGit: () => ({ status: 0, stdout: "", stderr: "" }),
      readLcov: () => "SF:/x.rs\nend_of_record\n",
    });
    const rc = await runCheck(baseOpts({ phase: 11 }), deps);
    expect(rc).toBe(0);
    expect(stdout.join("")).toContain("OK: no changed lines to cover");
  });

  test("T_bonus_no_lcov_with_skipGenerate: lcov 無し + --skip-generate → warn (phase<11) / error (phase>=11)", async () => {
    // phase <11 → warn + 0
    {
      const { deps, stdout, stderr } = makeDeps({
        readLcov: () => null,
      });
      const rc = await runCheck(
        baseOpts({ phase: 10, skipGenerate: true, lcovPath: "/nope.info" }),
        deps,
      );
      expect(rc).toBe(0);
      expect(stdout.join("")).toContain("WARN: lcov file not found");
      expect(stderr.join("")).toBe("");
    }
    // phase >=11 → error + 1
    {
      const { deps, stdout, stderr } = makeDeps({
        readLcov: () => null,
      });
      const rc = await runCheck(
        baseOpts({ phase: 11, skipGenerate: true, lcovPath: "/nope.info" }),
        deps,
      );
      expect(rc).toBe(1);
      expect(stderr.join("")).toContain("WARN: lcov file not found");
      expect(stdout.join("")).toBe("");
    }
  });

  test("非 crates/ ファイルはフィルタで落ちる → coverage 判定対象ゼロで OK", async () => {
    const diff = [
      "diff --git a/docs/foo.md b/docs/foo.md",
      "--- a/docs/foo.md",
      "+++ b/docs/foo.md",
      "@@ -0,0 +1,3 @@",
      "+line1",
      "+line2",
      "+line3",
    ].join("\n");
    const { deps, stdout } = makeDeps({
      runGit: () => ({ status: 0, stdout: diff, stderr: "" }),
      readLcov: () => "",
    });
    const rc = await runCheck(baseOpts({ phase: 11 }), deps);
    expect(rc).toBe(0);
    expect(stdout.join("")).toContain("OK: no changed lines to cover");
  });

  test("lcov generation を自動実行 (readLcov 初回 null → cargo llvm-cov → 再 readLcov)", async () => {
    let generated = false;
    let readCount = 0;
    const file = "crates/foo/src/lib.rs";
    const { deps, stdout } = makeDeps({
      runGit: () => ({ status: 0, stdout: makeDiffOutput(file, 11, 10), stderr: "" }),
      runCargo: (args) => {
        if (args[0] === "llvm-cov" && args[1] === "--version") {
          return { status: 0, stdout: "cargo-llvm-cov 0.5.0\n", stderr: "" };
        }
        if (args[0] === "llvm-cov" && args[1] === "--lcov") {
          generated = true;
          return { status: 0, stdout: "", stderr: "" };
        }
        return { status: 0, stdout: "", stderr: "" };
      },
      readLcov: () => {
        readCount++;
        if (readCount === 1) return null;
        return makeLcovOutput(file, 11, 10, 10);
      },
    });
    const rc = await runCheck(
      baseOpts({ phase: 11, threshold: 70, skipGenerate: false }),
      deps,
    );
    expect(generated).toBe(true);
    expect(rc).toBe(0);
    expect(stdout.join("")).toContain("OK: diff coverage 100%");
  });

  test("git diff 失敗 → exit 2", async () => {
    const { deps, stderr } = makeDeps({
      runGit: () => ({ status: 128, stdout: "", stderr: "fatal: bad revision\n" }),
      readLcov: () => "",
    });
    const rc = await runCheck(baseOpts({ phase: 11 }), deps);
    expect(rc).toBe(2);
    expect(stderr.join("")).toContain("git diff failed");
  });
});
