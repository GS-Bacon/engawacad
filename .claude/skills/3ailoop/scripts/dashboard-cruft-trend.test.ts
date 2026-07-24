// dashboard-cruft-trend.ts の単体テスト (Issue #313 T05/T06 + bonus)
//
// snapshot 側は一時ディレクトリに mock crates を作って git init → 実際に git grep が
// 走る形で E2E 相当を確認する。純関数系 (parseGitGrepCount / computeDelta / renderMarkdown)
// は in-memory のみ。

import { describe, expect, test, afterAll } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import {
  appendSnapshot,
  collectSnapshot,
  computeDelta,
  parseGitGrepCount,
  readSnapshots,
  renderMarkdown,
  type CruftSnapshot,
} from "./dashboard-cruft-trend.ts";

const TMP_ROOTS: string[] = [];

function freshTmp(label: string): string {
  const dir = mkdtempSync(join(tmpdir(), `cruft-trend-${label}-`));
  TMP_ROOTS.push(dir);
  return dir;
}

afterAll(() => {
  for (const d of TMP_ROOTS) rmSync(d, { recursive: true, force: true });
});

describe("parseGitGrepCount", () => {
  test("T_bonus_parse_grep: 'foo.rs:3\\nbar.rs:5\\n' → 8", () => {
    expect(parseGitGrepCount("foo.rs:3\nbar.rs:5\n")).toBe(8);
  });

  test("空文字列 → 0", () => {
    expect(parseGitGrepCount("")).toBe(0);
  });

  test("末尾改行なし・単一行 → その数値", () => {
    expect(parseGitGrepCount("crates/a/b/c.rs:7")).toBe(7);
  });

  test("複数コロンを含むパス (Windows 風) でも末尾コロン後を採用", () => {
    expect(parseGitGrepCount("crates/a:b/c.rs:2\ncrates/x.rs:3\n")).toBe(5);
  });

  test("壊れた行 (数値部が非数値) は 0 扱いで sum を汚さない", () => {
    expect(parseGitGrepCount("foo.rs:NaN\nbar.rs:4\n")).toBe(4);
  });
});

describe("computeDelta", () => {
  test("T_bonus_ignore_increase_bad: (42, 38, ignore) → +4 ⚠️", () => {
    expect(computeDelta(42, 38, "ignore")).toEqual({ value: 4, symbol: "+4 ⚠️" });
  });

  test("ignore 減少は改善: (38, 42, ignore) → -4 ✓", () => {
    expect(computeDelta(38, 42, "ignore")).toEqual({ value: -4, symbol: "-4 ✓" });
  });

  test("allow_clippy 増加は悪: (10, 5, allow_clippy) → +5 ⚠️", () => {
    expect(computeDelta(10, 5, "allow_clippy")).toEqual({ value: 5, symbol: "+5 ⚠️" });
  });

  test("T_bonus_regression_increase_good: (7, 6, regression_files) → +1 ✓", () => {
    expect(computeDelta(7, 6, "regression_files")).toEqual({ value: 1, symbol: "+1 ✓" });
  });

  test("T_bonus_regression_decrease_bad: (5, 7, regression_files) → -2 ⚠️", () => {
    expect(computeDelta(5, 7, "regression_files")).toEqual({ value: -2, symbol: "-2 ⚠️" });
  });

  test("変化なし: (5, 5, ignore) → 0 (symbol も '0')", () => {
    expect(computeDelta(5, 5, "ignore")).toEqual({ value: 0, symbol: "0" });
    expect(computeDelta(5, 5, "regression_files")).toEqual({ value: 0, symbol: "0" });
  });
});

describe("renderMarkdown", () => {
  test("T_bonus_empty: 0 entries → 'No data yet'", () => {
    const md = renderMarkdown([]);
    expect(md).toContain("## Cruft Trend");
    expect(md).toContain("_No data yet");
  });

  test("T_bonus_single: 1 entry → '- (baseline)' が Δ 列に入る", () => {
    const only: CruftSnapshot = {
      sha: "abc123",
      timestamp: "2026-07-24T08:30:00Z",
      ignore: 26,
      allow_clippy: 4,
      regression_files: 0,
    };
    const md = renderMarkdown([only]);
    expect(md).toContain("- (baseline)");
    expect(md).toContain("| 26 |");
    expect(md).toContain("Latest snapshot: abc123 at 2026-07-24T08:30:00Z");
  });

  test("T06_cruft_trend_render: 4 snapshot で ignore が単調増加 → +N ⚠️ 形式", () => {
    // 4 件なら past = 最新から 3 つ前 = entries[0]
    const entries: CruftSnapshot[] = [
      { sha: "s1", timestamp: "t1", ignore: 20, allow_clippy: 5, regression_files: 3 },
      { sha: "s2", timestamp: "t2", ignore: 25, allow_clippy: 5, regression_files: 3 },
      { sha: "s3", timestamp: "t3", ignore: 30, allow_clippy: 5, regression_files: 3 },
      { sha: "s4", timestamp: "t4", ignore: 42, allow_clippy: 5, regression_files: 3 },
    ];
    const md = renderMarkdown(entries);
    // 42 - 20 = +22 ⚠️
    expect(md).toContain("| `#[ignore]` | 42 | 20 | +22 ⚠️ |");
    expect(md).toContain("| `#[allow(clippy::)]` | 5 | 5 | 0 |");
    expect(md).toContain("| `regression_*.rs` files | 3 | 3 | 0 |");
    expect(md).toContain("Latest snapshot: s4 at t4");
  });

  test("2-3 entries は最古と比較 (past = entries[0])", () => {
    const entries: CruftSnapshot[] = [
      { sha: "s1", timestamp: "t1", ignore: 10, allow_clippy: 0, regression_files: 0 },
      { sha: "s2", timestamp: "t2", ignore: 15, allow_clippy: 0, regression_files: 0 },
    ];
    const md = renderMarkdown(entries);
    expect(md).toContain("| `#[ignore]` | 15 | 10 | +5 ⚠️ |");
  });
});

describe("snapshot / append / read (E2E)", () => {
  test("T05_cruft_trend_snapshot: mock crates で snapshot → JSONL に 1 行増える", () => {
    const dir = freshTmp("t05");
    // mock crates layout: 1 個の ignore, 1 個の allow_clippy, regression_*.rs は作らない
    mkdirSync(join(dir, "crates/foo/tests"), { recursive: true });
    mkdirSync(join(dir, "crates/foo/src"), { recursive: true });
    writeFileSync(
      join(dir, "crates/foo/tests/a.rs"),
      `#[test]\n#[ignore]\nfn t() {}\n\n#[test]\n#[ignore = \"slow\"]\nfn u() {}\n`,
    );
    writeFileSync(
      join(dir, "crates/foo/src/lib.rs"),
      `#[allow(clippy::too_many_arguments)]\npub fn f() {}\n`,
    );

    // git 初期化 (git grep は tracked file しか見ないので add まで必要)
    const run = (cmd: string[]) => Bun.spawnSync(cmd, { cwd: dir });
    expect(run(["git", "init", "-q"]).exitCode).toBe(0);
    expect(run(["git", "config", "user.email", "t@t"]).exitCode).toBe(0);
    expect(run(["git", "config", "user.name", "t"]).exitCode).toBe(0);
    expect(run(["git", "add", "-A"]).exitCode).toBe(0);
    expect(run(["git", "commit", "-q", "-m", "init"]).exitCode).toBe(0);

    // collectSnapshot は cwd 依存なので process.chdir で切り替え
    const origCwd = process.cwd();
    process.chdir(dir);
    try {
      const snap = collectSnapshot();
      expect(snap.ignore).toBe(2);
      expect(snap.allow_clippy).toBe(1);
      expect(snap.regression_files).toBe(0);
      expect(typeof snap.sha).toBe("string");
      expect(snap.sha.length).toBeGreaterThan(0);
      expect(snap.timestamp).toMatch(/^\d{4}-\d{2}-\d{2}T/);

      const jsonl = join(dir, "features/.loop/cruft-trend.jsonl");
      appendSnapshot(snap, jsonl);
      expect(existsSync(jsonl)).toBe(true);
      const lines = readFileSync(jsonl, "utf-8").trim().split("\n");
      expect(lines).toHaveLength(1);
      const parsed = JSON.parse(lines[0]);
      expect(parsed).toMatchObject({
        ignore: 2,
        allow_clippy: 1,
        regression_files: 0,
      });
      expect(parsed).toHaveProperty("sha");
      expect(parsed).toHaveProperty("timestamp");

      // 二度目 append で 2 行になること (append-only)
      appendSnapshot(snap, jsonl);
      const lines2 = readFileSync(jsonl, "utf-8").trim().split("\n");
      expect(lines2).toHaveLength(2);

      // readSnapshots で 2 件戻る
      const rows = readSnapshots(jsonl);
      expect(rows).toHaveLength(2);
    } finally {
      process.chdir(origCwd);
    }
  });

  test("readSnapshots: 存在しないファイル → []", () => {
    const dir = freshTmp("read-missing");
    expect(readSnapshots(join(dir, "does-not-exist.jsonl"))).toEqual([]);
  });

  test("readSnapshots: 壊れた行はスキップ、正常行だけ返す", () => {
    const dir = freshTmp("read-corrupt");
    const path = join(dir, "cruft-trend.jsonl");
    writeFileSync(
      path,
      `{"sha":"a","timestamp":"t","ignore":1,"allow_clippy":0,"regression_files":0}\n` +
        `broken line\n` +
        `{"sha":"b","timestamp":"t","ignore":2,"allow_clippy":0,"regression_files":0}\n`,
      "utf-8",
    );
    const rows = readSnapshots(path);
    expect(rows).toHaveLength(2);
    expect(rows[0].sha).toBe("a");
    expect(rows[1].sha).toBe("b");
  });
});
