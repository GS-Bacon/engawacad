// #253: loop-should-stop の halt 検知ロジックを単体テスト
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { detectPhase21Reached, detectHaltSwitch } from "./loop-should-stop";

describe("detectPhase21Reached (#253)", () => {
  let workDir: string;

  beforeEach(() => {
    workDir = mkdtempSync(join(tmpdir(), "should-stop-test-"));
  });

  afterEach(() => {
    rmSync(workDir, { recursive: true, force: true });
  });

  test("ROADMAP 不在 → false", () => {
    expect(detectPhase21Reached(join(workDir, "no-such.md"))).toBe(false);
  });

  test("Phase 21 セクションが ✅ なしで存在 → true (UI 期到達)", () => {
    const p = join(workDir, "ROADMAP.md");
    writeFileSync(p, "## Phase 20: Quality Pass ✅\n## Phase 21: UI 期\n", "utf-8");
    expect(detectPhase21Reached(p)).toBe(true);
  });

  test("Phase 21 セクションが ✅ 付き → false (既に完了 = 次が来る)", () => {
    const p = join(workDir, "ROADMAP.md");
    writeFileSync(p, "## ✅ Phase 21: UI 期\n## Phase 22: ...\n", "utf-8");
    expect(detectPhase21Reached(p)).toBe(false);
  });

  test("Phase 20 までしかなければ false", () => {
    const p = join(workDir, "ROADMAP.md");
    writeFileSync(p, "## Phase 9: foo\n## Phase 20: Quality Pass\n", "utf-8");
    expect(detectPhase21Reached(p)).toBe(false);
  });

  test("Phase 21 の見出しに `:` がなければ拾わない (regex 仕様)", () => {
    const p = join(workDir, "ROADMAP.md");
    writeFileSync(p, "## Phase 21 以降の話 — 注意書きセクション\n", "utf-8");
    expect(detectPhase21Reached(p)).toBe(false);
  });

  test("複数の Phase 21 行があっても 1 つでも ✅ なしなら true", () => {
    const p = join(workDir, "ROADMAP.md");
    writeFileSync(p, "## ✅ Phase 21: 完了済\n## Phase 21: 別軸 (未完)\n", "utf-8");
    expect(detectPhase21Reached(p)).toBe(true);
  });
});

describe("detectHaltSwitch (#253)", () => {
  let workDir: string;

  beforeEach(() => {
    workDir = mkdtempSync(join(tmpdir(), "halt-switch-test-"));
  });

  afterEach(() => {
    rmSync(workDir, { recursive: true, force: true });
  });

  test("switch ファイル不在 → false", () => {
    expect(detectHaltSwitch(join(workDir, "halt"))).toBe(false);
  });

  test("switch ファイル存在 → true", () => {
    const p = join(workDir, "halt");
    writeFileSync(p, "halt by user", "utf-8");
    expect(detectHaltSwitch(p)).toBe(true);
  });

  test("空ファイルでも存在さえすれば true", () => {
    const p = join(workDir, "halt");
    writeFileSync(p, "", "utf-8");
    expect(detectHaltSwitch(p)).toBe(true);
  });
});
