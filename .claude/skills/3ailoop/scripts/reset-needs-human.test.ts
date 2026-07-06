// reset-needs-human.ts: scanTargets の unit test
//
// gh 呼び出し (hasNeedsHumanLabel / removeNeedsHuman / postComment) は実 API 依存で
// mock 困難なので、対象走査ロジックだけをテストする。

import { describe, expect, test } from "bun:test";
import { mkdirSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { scanTargets, DEFAULT_CATEGORY } from "./reset-needs-human.ts";

const TMP_BASE = join(tmpdir(), `reset-needs-human-test-${process.pid}`);

function freshRoot(label: string): string {
  const dir = join(TMP_BASE, label);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  return dir;
}

function writeState(
  root: string,
  issue: number,
  category: string,
  state: { count: number; last_reason?: string },
): void {
  writeFileSync(
    join(root, `${issue}-${category}.json`),
    JSON.stringify(state, null, 2),
    "utf-8",
  );
}

describe("scanTargets", () => {
  test("空ディレクトリ → 空配列", () => {
    const root = freshRoot("empty");
    expect(scanTargets(root, null)).toEqual([]);
    expect(scanTargets(root, DEFAULT_CATEGORY)).toEqual([]);
  });

  test("存在しないディレクトリ → 空配列", () => {
    expect(scanTargets("/nonexistent-path-xyz", null)).toEqual([]);
  });

  test("categoryFilter null → 全カテゴリを列挙", () => {
    const root = freshRoot("all");
    writeState(root, 276, "codex-usage-limit", { count: 3, last_reason: "rate limit" });
    writeState(root, 220, "debug-spec-pending", { count: 3 });
    writeState(root, 195, "codex-usage-limit", { count: 1 });

    const targets = scanTargets(root, null);
    expect(targets).toHaveLength(3);
    expect(targets.map((t) => t.issue).sort()).toEqual([195, 220, 276]);
  });

  test("categoryFilter 指定 → 一致カテゴリのみ", () => {
    const root = freshRoot("filter");
    writeState(root, 276, "codex-usage-limit", { count: 3 });
    writeState(root, 220, "debug-spec-pending", { count: 3 });
    writeState(root, 195, "codex-usage-limit", { count: 1 });

    const targets = scanTargets(root, "codex-usage-limit");
    expect(targets).toHaveLength(2);
    expect(targets.every((t) => t.category === "codex-usage-limit")).toBe(true);
    expect(targets.map((t) => t.issue).sort()).toEqual([195, 276]);
  });

  test("count と last_reason を state から反映", () => {
    const root = freshRoot("meta");
    writeState(root, 276, "codex-usage-limit", {
      count: 4,
      last_reason: "cycle 75: Codex CLI usage limit",
    });

    const targets = scanTargets(root, "codex-usage-limit");
    expect(targets).toHaveLength(1);
    expect(targets[0].count).toBe(4);
    expect(targets[0].last_reason).toMatch(/Codex CLI usage limit/);
  });

  test("category に - を含むケース (indexOf 最初の - で分離)", () => {
    const root = freshRoot("dashcat");
    writeState(root, 276, "codex-usage-limit", { count: 3 });
    writeState(root, 300, "adr-review-token-cap", { count: 3 });

    const all = scanTargets(root, null);
    expect(all.map((t) => `${t.issue}/${t.category}`).sort()).toEqual([
      "276/codex-usage-limit",
      "300/adr-review-token-cap",
    ]);
  });

  test("破損 JSON は skip", () => {
    const root = freshRoot("broken");
    writeFileSync(join(root, "276-codex-usage-limit.json"), "{ this is not json", "utf-8");
    writeState(root, 195, "codex-usage-limit", { count: 3 });

    const targets = scanTargets(root, "codex-usage-limit");
    expect(targets).toHaveLength(1);
    expect(targets[0].issue).toBe(195);
  });

  test(".tmp ファイルは skip (途中書き込み)", () => {
    const root = freshRoot("tmp");
    writeFileSync(
      join(root, "276-codex-usage-limit.json.tmp.12345.abc"),
      JSON.stringify({ count: 3 }),
      "utf-8",
    );
    writeState(root, 195, "codex-usage-limit", { count: 3 });

    const targets = scanTargets(root, "codex-usage-limit");
    expect(targets).toHaveLength(1);
    expect(targets[0].issue).toBe(195);
  });

  test(".json 以外のファイルは skip", () => {
    const root = freshRoot("nonjson");
    writeFileSync(join(root, "276-codex-usage-limit.txt"), "irrelevant", "utf-8");
    writeState(root, 195, "codex-usage-limit", { count: 3 });

    const targets = scanTargets(root, "codex-usage-limit");
    expect(targets).toHaveLength(1);
    expect(targets[0].issue).toBe(195);
  });

  test("issue 番号が数値でないファイル名は skip", () => {
    const root = freshRoot("noninteger");
    writeFileSync(
      join(root, "foo-codex-usage-limit.json"),
      JSON.stringify({ count: 3 }),
      "utf-8",
    );
    writeState(root, 195, "codex-usage-limit", { count: 3 });

    const targets = scanTargets(root, "codex-usage-limit");
    expect(targets).toHaveLength(1);
    expect(targets[0].issue).toBe(195);
  });
});
