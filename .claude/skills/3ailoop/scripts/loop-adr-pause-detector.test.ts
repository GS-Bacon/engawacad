// #252: loop-adr-pause-detector の rescan モード単体テスト
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { extractAdrPathFromTitle, rescanStaleGateIssues } from "./loop-adr-pause-detector";

describe("extractAdrPathFromTitle (#252)", () => {
  test("通常 title から ADR path を抽出", () => {
    expect(
      extractAdrPathFromTitle("[gate:adr-review] ADR-013: ADR 自動 accept フロー (docs/decisions/013-adr-auto-accept-flow.md)")
    ).toBe("docs/decisions/013-adr-auto-accept-flow.md");
  });

  test("末尾 whitespace を許容", () => {
    expect(
      extractAdrPathFromTitle("[gate:adr-review] ADR-016: CLI 命名 (docs/decisions/016-engawa-cli-naming.md)   ")
    ).toBe("docs/decisions/016-engawa-cli-naming.md");
  });

  test("ADR path が含まれない title → null", () => {
    expect(extractAdrPathFromTitle("[gate:adr-review] 何かのレビュー")).toBeNull();
  });

  test("docs/decisions 以外のパス → null", () => {
    expect(extractAdrPathFromTitle("[gate:adr-review] title (docs/other/foo.md)")).toBeNull();
  });

  test("ネストパスでもマッチ (ADR ファイル形式維持)", () => {
    expect(
      extractAdrPathFromTitle("[gate:adr-review] (docs/decisions/099-deep-name-with-dashes.md)")
    ).toBe("docs/decisions/099-deep-name-with-dashes.md");
  });
});

describe("rescanStaleGateIssues (#252)", () => {
  let workDir: string;
  let originalCwd: string;

  beforeEach(() => {
    originalCwd = process.cwd();
    workDir = mkdtempSync(join(tmpdir(), "rescan-test-"));
    process.chdir(workDir);
    mkdirSync(join(workDir, "docs/decisions"), { recursive: true });
  });

  afterEach(() => {
    process.chdir(originalCwd);
    rmSync(workDir, { recursive: true, force: true });
  });

  test("gh issue list が失敗 → 空配列", async () => {
    const ghFn = async () => ({ stdout: "", exit: 1 });
    const records = await rescanStaleGateIssues({ ghFn });
    expect(records).toEqual([]);
  });

  test("open Issue ゼロ → 空配列", async () => {
    const ghFn = async () => ({ stdout: "[]", exit: 0 });
    const records = await rescanStaleGateIssues({ ghFn });
    expect(records).toEqual([]);
  });

  test("title から ADR path 抽出失敗 → outcome=skip:no ADR path", async () => {
    const ghFn = async () => ({
      stdout: JSON.stringify([{ number: 100, title: "[gate:adr-review] broken title" }]),
      exit: 0,
    });
    const records = await rescanStaleGateIssues({ ghFn });
    expect(records).toHaveLength(1);
    expect(records[0].issue).toBe(100);
    expect(records[0].outcome).toContain("skip");
    expect(records[0].outcome).toContain("ADR path");
  });

  test("ADR file が存在しない → outcome=skip:ADR file missing", async () => {
    const ghFn = async () => ({
      stdout: JSON.stringify([
        { number: 200, title: "[gate:adr-review] ADR-999 (docs/decisions/999-nope.md)" },
      ]),
      exit: 0,
    });
    const records = await rescanStaleGateIssues({ ghFn });
    expect(records).toHaveLength(1);
    expect(records[0].adr).toBe("docs/decisions/999-nope.md");
    expect(records[0].outcome).toContain("ADR file missing");
  });

  test("dry-run → autoAccept を呼ばず dry-run outcome を返す", async () => {
    const adrPath = "docs/decisions/777-test.md";
    writeFileSync(join(workDir, adrPath), "# ADR-777 test", "utf-8");
    const ghFn = async () => ({
      stdout: JSON.stringify([
        { number: 777, title: `[gate:adr-review] (${adrPath})` },
      ]),
      exit: 0,
    });
    let aaCalled = false;
    const aaFn = async () => { aaCalled = true; return { kind: "accepted" as const, verdicts: [] }; };
    const records = await rescanStaleGateIssues({ ghFn, autoAcceptFn: aaFn, dryRun: true });
    expect(aaCalled).toBe(false);
    expect(records[0].outcome).toBe("dry-run");
  });

  test("複数 Issue に対して autoAccept が個別に呼ばれ、outcome.kind が記録される", async () => {
    const adr1 = "docs/decisions/801-one.md";
    const adr2 = "docs/decisions/802-two.md";
    writeFileSync(join(workDir, adr1), "# A", "utf-8");
    writeFileSync(join(workDir, adr2), "# B", "utf-8");
    const ghFn = async () => ({
      stdout: JSON.stringify([
        { number: 801, title: `[gate:adr-review] ADR-801 (${adr1})` },
        { number: 802, title: `[gate:adr-review] ADR-802 (${adr2})` },
      ]),
      exit: 0,
    });
    const calls: number[] = [];
    const aaFn = async (opts: { adrPath: string; issueNum: number }) => {
      calls.push(opts.issueNum);
      return opts.issueNum === 801
        ? { kind: "accepted" as const, verdicts: [] }
        : { kind: "regen_required" as const, reason: "review" as const, details: [], regen_count: 1 };
    };
    const records = await rescanStaleGateIssues({ ghFn, autoAcceptFn: aaFn });
    expect(calls).toEqual([801, 802]);
    expect(records.map(r => r.outcome)).toEqual(["accepted", "regen_required"]);
  });

  test("autoAccept が throw しても全 Issue 走査を継続し outcome に error 記録", async () => {
    const adr = "docs/decisions/901-err.md";
    writeFileSync(join(workDir, adr), "# E", "utf-8");
    const ghFn = async () => ({
      stdout: JSON.stringify([
        { number: 901, title: `[gate:adr-review] (${adr})` },
      ]),
      exit: 0,
    });
    const aaFn = async () => { throw new Error("boom"); };
    const records = await rescanStaleGateIssues({ ghFn, autoAcceptFn: aaFn });
    expect(records[0].outcome).toMatch(/^error:.*boom/);
  });
});
