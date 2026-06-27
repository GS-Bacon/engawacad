import { describe, expect, test } from "bun:test";
import { mkdtempSync, writeFileSync, mkdirSync, rmSync } from "fs";
import { join } from "path";
import { tmpdir } from "os";
import {
  collectAcceptedAdrSummaries,
  collectRoadmapPhaseSummaries,
  crossRefCheck,
} from "./loop-adr-cross-ref-check";

describe("collectAcceptedAdrSummaries", () => {
  test("Accepted ADR のみ抽出、自分自身は除外", () => {
    const dir = mkdtempSync(join(tmpdir(), "adr-cross-ref-"));
    try {
      mkdirSync(`${dir}/decisions`, { recursive: true });
      writeFileSync(`${dir}/decisions/001-foo.md`, [
        "# ADR-001: Foo",
        "**Status**: Accepted",
        "**Related**: ADR-002",
      ].join("\n"));
      writeFileSync(`${dir}/decisions/002-bar.md`, [
        "# ADR-002: Bar",
        "**Status**: Proposed",
        "**Related**: -",
      ].join("\n"));
      writeFileSync(`${dir}/decisions/003-baz.md`, [
        "# ADR-003: Baz",
        "**Status**: Accepted",
        "**Related**: ADR-001",
      ].join("\n"));
      const summaries = collectAcceptedAdrSummaries(
        `${dir}/decisions/003-baz.md`,
        `${dir}/decisions`,
      );
      expect(summaries.length).toBe(1);
      expect(summaries[0].number).toBe(1);
      expect(summaries[0].title).toBe("Foo");
      expect(summaries[0].relatedLine).toBe("ADR-002");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  test("存在しない dir → 空配列", () => {
    expect(collectAcceptedAdrSummaries("nonexistent.md", "no-such-dir")).toEqual([]);
  });
});

describe("collectRoadmapPhaseSummaries", () => {
  test("Phase 見出し直後の summary 行を抽出", () => {
    const dir = mkdtempSync(join(tmpdir(), "adr-roadmap-"));
    try {
      const roadmap = [
        "# ROADMAP",
        "## ✅ Phase 0: 起点",
        "完了済み",
        "## Phase 9: 履歴編集",
        "**外から見た成果**: 履歴 CRUD と Variable",
        "## Phase 19: データ連携",
        "STEP / IGES の入出力",
      ].join("\n");
      const path = `${dir}/ROADMAP.md`;
      writeFileSync(path, roadmap);
      const phases = collectRoadmapPhaseSummaries(path);
      expect(phases.length).toBe(3);
      expect(phases[0]).toMatch(/Phase 0/);
      expect(phases[1]).toMatch(/Phase 9/);
      expect(phases[1]).toMatch(/履歴 CRUD/);
      expect(phases[2]).toMatch(/Phase 19/);
      expect(phases[2]).toMatch(/STEP/);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe("crossRefCheck (mock 経路)", () => {
  test("ADR_CROSSREF_MOCK=aligned → aligned=true", async () => {
    const dir = mkdtempSync(join(tmpdir(), "adr-check-"));
    try {
      const adr = `${dir}/100-test.md`;
      writeFileSync(adr, "# ADR-100\n## Decision\nfoo");
      process.env.ADR_CROSSREF_MOCK = "aligned";
      const r = await crossRefCheck({ adrPath: adr, outDir: `${dir}/out` });
      expect(r.aligned).toBe(true);
      expect(r.reviewer).toBe("mock");
    } finally {
      delete process.env.ADR_CROSSREF_MOCK;
      rmSync(dir, { recursive: true, force: true });
    }
  });

  test("ADR_CROSSREF_MOCK=misaligned → aligned=false + missing_refs", async () => {
    const dir = mkdtempSync(join(tmpdir(), "adr-check-"));
    try {
      const adr = `${dir}/100-test.md`;
      writeFileSync(adr, "# ADR-100\n## Decision\nfoo");
      process.env.ADR_CROSSREF_MOCK = "misaligned";
      const r = await crossRefCheck({ adrPath: adr, outDir: `${dir}/out` });
      expect(r.aligned).toBe(false);
      expect(r.missing_refs.length).toBeGreaterThan(0);
      expect(r.reviewer).toBe("mock");
    } finally {
      delete process.env.ADR_CROSSREF_MOCK;
      rmSync(dir, { recursive: true, force: true });
    }
  });

  test("--dry-run → aligned=true with reviewer=dry-run", async () => {
    const dir = mkdtempSync(join(tmpdir(), "adr-check-"));
    try {
      const adr = `${dir}/100-test.md`;
      writeFileSync(adr, "# ADR-100\n## Decision\nfoo");
      const r = await crossRefCheck({ adrPath: adr, outDir: `${dir}/out`, dryRun: true });
      expect(r.aligned).toBe(true);
      expect(r.reviewer).toBe("dry-run");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
