// loop-context-bootstrap.test.ts — #230 diff モード + cache の単体テスト
//
// - diffIssues: added / closed / labels changed / unchanged の検出
// - checkFileChange: mtime + sha による未変更検出
// - extractCurrentPhase: ✅ 済み Phase スキップ
// - integration: bun spawn で連続 2 回呼び (--full → diff) 出力 line 数が大幅に減ることを確認

import { describe, expect, test, beforeEach, afterEach } from "bun:test";
import { mkdirSync, rmSync, writeFileSync, existsSync, readFileSync } from "fs";
import { join } from "path";
import { tmpdir } from "os";
import {
  diffIssues,
  checkFileChange,
  extractCurrentPhase,
  classifyIssues,
} from "./loop-context-bootstrap.ts";

const SCRIPT = join(import.meta.dir, "loop-context-bootstrap.ts");

let tmp: string;
beforeEach(() => {
  tmp = join(tmpdir(), `ctx-bootstrap-test-${Date.now()}-${Math.floor(Math.random() * 1e6)}`);
  mkdirSync(tmp, { recursive: true });
});
afterEach(() => {
  rmSync(tmp, { recursive: true, force: true });
});

describe("diffIssues", () => {
  test("T01: 完全一致なら unchanged のみ", () => {
    const prev = [{ number: 1, title: "a", labels: ["x"] }];
    const cur = [{ number: 1, title: "a", labels: ["x"] }];
    const d = diffIssues(prev, cur);
    expect(d.added).toHaveLength(0);
    expect(d.closed).toHaveLength(0);
    expect(d.labelsChanged).toHaveLength(0);
    expect(d.unchanged).toBe(1);
  });

  test("T02: 新規 issue は added に入る", () => {
    const prev = [{ number: 1, title: "a", labels: [] }];
    const cur = [
      { number: 1, title: "a", labels: [] },
      { number: 2, title: "b", labels: ["new"] },
    ];
    const d = diffIssues(prev, cur);
    expect(d.added).toHaveLength(1);
    expect(d.added[0].number).toBe(2);
    expect(d.closed).toHaveLength(0);
    expect(d.unchanged).toBe(1);
  });

  test("T03: 消えた issue は closed に入る", () => {
    const prev = [
      { number: 1, title: "a", labels: [] },
      { number: 2, title: "b", labels: [] },
    ];
    const cur = [{ number: 1, title: "a", labels: [] }];
    const d = diffIssues(prev, cur);
    expect(d.closed).toHaveLength(1);
    expect(d.closed[0].number).toBe(2);
    expect(d.added).toHaveLength(0);
    expect(d.unchanged).toBe(1);
  });

  test("T04: ラベル変更は labelsChanged に入る (順序非依存)", () => {
    const prev = [{ number: 1, title: "a", labels: ["x", "y"] }];
    const cur = [{ number: 1, title: "a", labels: ["y", "x"] }];
    const d = diffIssues(prev, cur);
    expect(d.labelsChanged).toHaveLength(0);
    expect(d.unchanged).toBe(1);

    const cur2 = [{ number: 1, title: "a", labels: ["x", "z"] }];
    const d2 = diffIssues(prev, cur2);
    expect(d2.labelsChanged).toHaveLength(1);
    expect(d2.labelsChanged[0].before).toEqual(["x", "y"]);
    expect(d2.labelsChanged[0].after).toEqual(["x", "z"]);
  });

  test("T04b: ラベルに `|` が含まれても誤検出しない (#230 Codex r2)", () => {
    const prev = [{ number: 1, title: "a", labels: ["a|b"] }];
    const cur = [{ number: 1, title: "a", labels: ["a", "b"] }];
    const d = diffIssues(prev, cur);
    expect(d.labelsChanged).toHaveLength(1);
    expect(d.labelsChanged[0].before).toEqual(["a|b"]);
    expect(d.labelsChanged[0].after).toEqual(["a", "b"]);
  });

  test("T05: 空 prev (cold start) なら全件 added", () => {
    const cur = [
      { number: 1, title: "a", labels: [] },
      { number: 2, title: "b", labels: [] },
    ];
    const d = diffIssues([], cur);
    expect(d.added).toHaveLength(2);
    expect(d.closed).toHaveLength(0);
  });
});

describe("checkFileChange", () => {
  test("T10: prev 無し → changed=true", () => {
    const p = join(tmp, "f.md");
    writeFileSync(p, "hello");
    const r = checkFileChange(p, "hello", undefined);
    expect(r.changed).toBe(true);
    expect(r.entry.sha).toHaveLength(12);
  });

  test("T11: sha 一致 → changed=false (mtime は無視)", () => {
    const p = join(tmp, "f.md");
    writeFileSync(p, "hello");
    const r1 = checkFileChange(p, "hello", undefined);
    // mtime を意図的にずらしても sha が同じなら未変更
    const r2 = checkFileChange(p, "hello", { mtime: 0, sha: r1.entry.sha });
    expect(r2.changed).toBe(false);
  });

  test("T12: sha 不一致 → changed=true", () => {
    const p = join(tmp, "f.md");
    writeFileSync(p, "hello");
    const r1 = checkFileChange(p, "hello", undefined);
    const r2 = checkFileChange(p, "world", { mtime: r1.entry.mtime, sha: r1.entry.sha });
    expect(r2.changed).toBe(true);
    expect(r2.entry.sha).not.toBe(r1.entry.sha);
  });
});

describe("extractCurrentPhase", () => {
  test("T20: ✅ なしの Phase ヘッダを採用", () => {
    const r = extractCurrentPhase(
      "## ✅ Phase 0 — done\nsome\n## Phase 8 — current\nbody\n## ✅ Phase 9 — later",
    );
    expect(r).toContain("Phase 8");
    expect(r).toContain("body");
    expect(r).not.toContain("Phase 9");
  });

  test("T21: 全 Phase 完了 → 説明文", () => {
    const r = extractCurrentPhase("## ✅ Phase 0\n## ✅ Phase 1\n");
    expect(r).toContain("全 Phase が完了済み");
  });
});

describe("classifyIssues", () => {
  test("T30: gate / needs-* を分類", () => {
    const c = classifyIssues([
      { number: 1, title: "a", labels: [] },
      { number: 2, title: "b", labels: ["gate:adr-review"] },
      { number: 3, title: "c", labels: ["needs-human"] },
      { number: 4, title: "d", labels: ["type: feature"] },
    ]);
    expect(c.total).toBe(4);
    expect(c.loopActionable).toBe(2); // #1, #4
    expect(c.gateBreakdown["gate:adr-review"]).toBe(1);
    expect(c.needsBreakdown["needs-human"]).toBe(1);
  });
});

describe("integration (bun spawn)", () => {
  // diff モード: 連続呼び出しで stdout 行数が減ること、issue-cache.json が生成されることを確認
  // gh issue list は実環境のみ動くため、ROADMAP / CLAUDE.md / Memory / dashboard / state.json
  // の cache 部分のみ確認。gh 失敗時は "*gh issue list failed*" が出るが script 自体は exit 0。

  test("T40: --full → diff の 2 回連続で context-cache.json が作られ、2 回目は ROADMAP 全文が抑制される", async () => {
    const cwd = tmp;
    mkdirSync(join(cwd, "features/.loop"), { recursive: true });
    writeFileSync(
      join(cwd, "ROADMAP.md"),
      "## Phase 8 — current\nbody line 1\nbody line 2\nbody line 3\n",
    );
    writeFileSync(join(cwd, "CLAUDE.md"), "# rules\nline\nline\n");
    const memPath = join(cwd, "memory.md");
    writeFileSync(memPath, "# mem\nentry 1\nentry 2\n");
    writeFileSync(join(cwd, "features/.dashboard.md"), "# dash\nx\n");
    writeFileSync(
      join(cwd, "features/.loop/state.json"),
      JSON.stringify({
        cycle: 5,
        last_cycle_at: "2026-06-18T00:00:00Z",
        recent_cycles: [
          { cycle: 3, closed: [], raised: [], merged_commits: 0 },
          { cycle: 4, closed: [], raised: [], merged_commits: 1 },
          { cycle: 5, closed: [10], raised: [11], merged_commits: 2 },
        ],
      }),
    );

    // 1 回目: --full
    const r1 = Bun.spawnSync(
      ["bun", SCRIPT, "--memory-index", memPath, "--cache-dir", join(cwd, "features/.loop"), "--full"],
      { cwd, stdout: "pipe", stderr: "pipe" },
    );
    expect(r1.exitCode).toBe(0);
    const out1 = new TextDecoder().decode(r1.stdout);
    expect(out1).toContain("Mode: full");
    expect(out1).toContain("Phase 8 — current");
    expect(out1).toContain("body line 1"); // 全文展開
    expect(out1).toContain("# rules");

    // context-cache.json が出来ている
    expect(existsSync(join(cwd, "features/.loop/context-cache.json"))).toBe(true);

    // 2 回目: diff モード (デフォルト)
    const r2 = Bun.spawnSync(
      ["bun", SCRIPT, "--memory-index", memPath, "--cache-dir", join(cwd, "features/.loop")],
      { cwd, stdout: "pipe", stderr: "pipe" },
    );
    expect(r2.exitCode).toBe(0);
    const out2 = new TextDecoder().decode(r2.stdout);
    expect(out2).toContain("Mode: diff");
    expect(out2).toContain("unchanged (cache:"); // ROADMAP / CLAUDE.md / Memory が抑制された
    // body line 1 は ROADMAP の中身。未変更なら出ない
    expect(out2).not.toContain("body line 1");

    // 出力サイズが減っていること (期待効果)
    expect(out2.length).toBeLessThan(out1.length);

    // recent cycle は 1 件のみ (diff モード)
    const recentMatches = out2.match(/Cycle #\d+:/g);
    expect(recentMatches?.length).toBe(1);
  });

  test("T41: --full で再度呼ぶと cache 破棄され全文復活", async () => {
    const cwd = tmp;
    mkdirSync(join(cwd, "features/.loop"), { recursive: true });
    writeFileSync(join(cwd, "ROADMAP.md"), "## Phase 8\nbody\n");
    writeFileSync(join(cwd, "CLAUDE.md"), "# rules\n");
    const memPath = join(cwd, "memory.md");
    writeFileSync(memPath, "# mem\n");

    // 初回 + diff で cache が温まる
    Bun.spawnSync(
      ["bun", SCRIPT, "--memory-index", memPath, "--cache-dir", join(cwd, "features/.loop")],
      { cwd, stdout: "pipe", stderr: "pipe" },
    );
    Bun.spawnSync(
      ["bun", SCRIPT, "--memory-index", memPath, "--cache-dir", join(cwd, "features/.loop")],
      { cwd, stdout: "pipe", stderr: "pipe" },
    );
    // --full で再度: 全文復活
    const r3 = Bun.spawnSync(
      ["bun", SCRIPT, "--memory-index", memPath, "--cache-dir", join(cwd, "features/.loop"), "--full"],
      { cwd, stdout: "pipe", stderr: "pipe" },
    );
    const out3 = new TextDecoder().decode(r3.stdout);
    expect(out3).toContain("Mode: full");
    expect(out3).toContain("Phase 8");
    expect(out3).toContain("body"); // 全文復活
    expect(out3).not.toContain("unchanged (cache:"); // 抑制マーカーは出ない
  });

  test("T42b: 破損 issue-cache.json (wrong shape) なら cold-start にフォールバック", async () => {
    const cwd = tmp;
    mkdirSync(join(cwd, "features/.loop"), { recursive: true });
    writeFileSync(join(cwd, "ROADMAP.md"), "## Phase 8\n");
    writeFileSync(join(cwd, "CLAUDE.md"), "# rules\n");
    const memPath = join(cwd, "memory.md");
    writeFileSync(memPath, "# mem\n");
    // 不正なシェイプ: number が string、labels が無い
    writeFileSync(
      join(cwd, "features/.loop/issue-cache.json"),
      JSON.stringify({ fetched_at: "x", issues: [{ number: "abc", title: "bad" }] }),
    );
    const r = Bun.spawnSync(
      ["bun", SCRIPT, "--memory-index", memPath, "--cache-dir", join(cwd, "features/.loop")],
      { cwd, stdout: "pipe", stderr: "pipe" },
    );
    const out = new TextDecoder().decode(r.stdout);
    expect(r.exitCode).toBe(0);
    // 破損 cache は drop されて cold start (Diff since cache が出ない)
    expect(out).not.toContain("Diff since cache");
  });

  test("T42: issue-cache.json は存在しない場合 cold start として扱う (diff 出ず classification 全表示)", async () => {
    // gh issue list は実環境依存だが、出力フォーマットの cold-start 経路を確認するため
    // ROADMAP / Memory のみ用意し、cache 無しで diff モード起動 → "Loop-actionable" 行が出る
    const cwd = tmp;
    mkdirSync(join(cwd, "features/.loop"), { recursive: true });
    writeFileSync(join(cwd, "ROADMAP.md"), "## Phase 8\n");
    writeFileSync(join(cwd, "CLAUDE.md"), "# rules\n");
    const memPath = join(cwd, "memory.md");
    writeFileSync(memPath, "# mem\n");

    const r = Bun.spawnSync(
      ["bun", SCRIPT, "--memory-index", memPath, "--cache-dir", join(cwd, "features/.loop")],
      { cwd, stdout: "pipe", stderr: "pipe" },
    );
    const out = new TextDecoder().decode(r.stdout);
    expect(r.exitCode).toBe(0);
    // cold start: "Diff since cache" は出ない。失敗時は "*gh issue list failed*" の可能性あり
    if (!out.includes("gh issue list failed")) {
      expect(out).toContain("Loop-actionable");
    }
  });
});
