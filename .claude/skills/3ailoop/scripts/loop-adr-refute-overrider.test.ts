import { describe, expect, test } from "bun:test";
import { mkdtempSync, writeFileSync, rmSync } from "fs";
import { join } from "path";
import { tmpdir } from "os";
import { overrideRefutes } from "./loop-adr-refute-overrider";
import type { PersonaVerdict } from "./loop-adr-auto-accept";

const sampleAdr = `# ADR-200: Test
**Status**: Proposed
**Related**: ADR-001
## Decision
- A. opt1
- B. opt2
- C. opt3
採用: A
## Consequences
採用前提崩壊 trigger: foo
`;

const sampleRefutes: PersonaVerdict[] = [
  { persona: "architect", approved: false, raw_excerpt: "schema 矛盾あり" },
  { persona: "contrarian", approved: false, raw_excerpt: "弱い refute" },
];

describe("overrideRefutes (mock 経路)", () => {
  test("空 refute 配列 → all_overridable=true", async () => {
    const r = await overrideRefutes({ adrPath: "dummy", refutes: [] });
    expect(r.all_overridable).toBe(true);
    expect(r.judgements).toEqual([]);
  });

  test("ADR_OVERRIDE_MOCK=override-all → 全 override", async () => {
    const dir = mkdtempSync(join(tmpdir(), "adr-override-"));
    try {
      const adr = `${dir}/200-test.md`;
      writeFileSync(adr, sampleAdr);
      process.env.ADR_OVERRIDE_MOCK = "override-all";
      const r = await overrideRefutes({
        adrPath: adr,
        refutes: sampleRefutes,
        outDir: `${dir}/out`,
      });
      expect(r.all_overridable).toBe(true);
      expect(r.judgements.length).toBe(2);
      expect(r.judgements.every(j => !j.keep_refute)).toBe(true);
      expect(r.reviewer).toBe("mock");
    } finally {
      delete process.env.ADR_OVERRIDE_MOCK;
      rmSync(dir, { recursive: true, force: true });
    }
  });

  test("ADR_OVERRIDE_MOCK=keep-all → 全 keep", async () => {
    const dir = mkdtempSync(join(tmpdir(), "adr-override-"));
    try {
      const adr = `${dir}/200-test.md`;
      writeFileSync(adr, sampleAdr);
      process.env.ADR_OVERRIDE_MOCK = "keep-all";
      const r = await overrideRefutes({
        adrPath: adr,
        refutes: sampleRefutes,
        outDir: `${dir}/out`,
      });
      expect(r.all_overridable).toBe(false);
      expect(r.judgements.length).toBe(2);
      expect(r.judgements.every(j => j.keep_refute)).toBe(true);
    } finally {
      delete process.env.ADR_OVERRIDE_MOCK;
      rmSync(dir, { recursive: true, force: true });
    }
  });

  test("--dry-run → 全 keep (安全側)", async () => {
    const dir = mkdtempSync(join(tmpdir(), "adr-override-"));
    try {
      const adr = `${dir}/200-test.md`;
      writeFileSync(adr, sampleAdr);
      const r = await overrideRefutes({
        adrPath: adr,
        refutes: sampleRefutes,
        outDir: `${dir}/out`,
        dryRun: true,
      });
      expect(r.all_overridable).toBe(false);
      expect(r.judgements.every(j => j.keep_refute)).toBe(true);
      expect(r.reviewer).toBe("dry-run");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
