// dispatch-codex-3persona.ts: 3 persona 並列 spawn & merge テスト (#231)

import { describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import {
  dispatchCodex3Persona,
  mergePersonaResults,
  PERSONAS,
  type PersonaResult,
} from "../dispatch-codex-3persona.ts";

const TMP_BASE = join(tmpdir(), `dispatch-codex-3p-test-${process.pid}`);

function freshTmpRoot(label: string): { dir: string; instr: string; result: string } {
  const dir = join(TMP_BASE, label);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const instr = join(dir, "agent.md");
  writeFileSync(instr, "# Mock reviewer\n", "utf-8");
  return { dir, instr, result: join(dir, "codex-final.yaml") };
}

function mockResult(
  persona: PersonaResult["persona"],
  verdict: "pass" | "fail" | "unknown",
  counts: { critical?: number; high?: number; medium?: number; low?: number } = {},
): PersonaResult {
  const sc = {
    critical: counts.critical ?? 0,
    high: counts.high ?? 0,
    medium: counts.medium ?? 0,
    low: counts.low ?? 0,
  };
  return {
    persona,
    exitCode: 0,
    yamlPath: `/tmp/mock-${persona}.yaml`,
    verdict,
    severity_counts: sc,
    blocking: sc.critical + sc.high,
  };
}

describe("mergePersonaResults (pure)", () => {
  test("T04: 3 persona 全 verdict=pass → merged verdict=pass, blocking=0", () => {
    const r = mergePersonaResults([
      mockResult("architect", "pass"),
      mockResult("contrarian", "pass"),
      mockResult("migration", "pass"),
    ]);
    expect(r.verdict).toBe("pass");
    expect(r.blocking).toBe(0);
    expect(r.severity_counts.critical).toBe(0);
  });

  test("T05: 1 persona critical=1 → merged verdict=fail, blocking=1", () => {
    const r = mergePersonaResults([
      mockResult("architect", "pass"),
      mockResult("contrarian", "fail", { critical: 1 }),
      mockResult("migration", "pass"),
    ]);
    expect(r.verdict).toBe("fail");
    expect(r.blocking).toBe(1);
    expect(r.severity_counts.critical).toBe(1);
  });

  test("T06: 3 persona 全 critical=1 → merged verdict=fail, blocking=3", () => {
    const r = mergePersonaResults([
      mockResult("architect", "fail", { critical: 1 }),
      mockResult("contrarian", "fail", { critical: 1 }),
      mockResult("migration", "fail", { critical: 1 }),
    ]);
    expect(r.verdict).toBe("fail");
    expect(r.blocking).toBe(3);
    expect(r.severity_counts.critical).toBe(3);
  });

  test("T07_boundary_empty_issues: 全 persona verdict=pass + issues=[] → merged verdict=pass, blocking=0", () => {
    const r = mergePersonaResults([
      mockResult("architect", "pass"),
      mockResult("contrarian", "pass"),
      mockResult("migration", "pass"),
    ]);
    expect(r.verdict).toBe("pass");
    expect(r.blocking).toBe(0);
    expect(r.severity_counts.high).toBe(0);
  });

  test("T07b: severity mix (high + medium) を集計", () => {
    const r = mergePersonaResults([
      mockResult("architect", "pass", { medium: 2 }),
      mockResult("contrarian", "fail", { high: 1, low: 3 }),
      mockResult("migration", "pass", { low: 1 }),
    ]);
    expect(r.severity_counts).toEqual({ critical: 0, high: 1, medium: 2, low: 4 });
    expect(r.blocking).toBe(1);
    expect(r.verdict).toBe("fail");
  });

  test("T09_degen_all_unknown: 全 persona verdict=unknown → silent fail 防止で merged=fail", () => {
    const r = mergePersonaResults([
      mockResult("architect", "unknown"),
      mockResult("contrarian", "unknown"),
      mockResult("migration", "unknown"),
    ]);
    expect(r.verdict).toBe("fail");
    expect(r.blocking).toBe(0);
  });

  test("T09b: empty results → merged verdict=pass, blocking=0 (no-op)", () => {
    const r = mergePersonaResults([]);
    expect(r.verdict).toBe("pass");
    expect(r.blocking).toBe(0);
  });
});

describe("dispatchCodex3Persona parallel mock", () => {
  test("T08a: mockMode=pass → 3 persona 全 yaml 生成 + 統合 yaml/verdict.json 生成", async () => {
    const { instr, result } = freshTmpRoot("t08a");
    const merged = await dispatchCodex3Persona({
      instructionFile: instr,
      resultFile: result,
      mockMode: "pass",
    });
    expect(merged.verdict).toBe("pass");
    expect(merged.blocking).toBe(0);
    expect(merged.per_persona).toHaveLength(3);
    for (const p of PERSONAS) {
      const yamlPath = result.replace(/\.yaml$/, `-${p}.yaml`);
      expect(existsSync(yamlPath)).toBe(true);
      expect(existsSync(`${yamlPath}.verdict.json`)).toBe(true);
    }
    expect(existsSync(result)).toBe(true);
    expect(existsSync(`${result}.verdict.json`)).toBe(true);
    const v = JSON.parse(readFileSync(`${result}.verdict.json`, "utf-8"));
    expect(v.verdict).toBe("pass");
    expect(v.per_persona).toHaveLength(3);
  });

  test("T08b: mockMode=fail → merged blocking>=3 (各 persona critical=1)", async () => {
    const { instr, result } = freshTmpRoot("t08b");
    const merged = await dispatchCodex3Persona({
      instructionFile: instr,
      resultFile: result,
      mockMode: "fail",
    });
    expect(merged.verdict).toBe("fail");
    expect(merged.blocking).toBeGreaterThanOrEqual(3);
  });

  test("T08c: mockMode=mixed → architect/migration pass, contrarian fail → merged blocking=1", async () => {
    const { instr, result } = freshTmpRoot("t08c");
    const merged = await dispatchCodex3Persona({
      instructionFile: instr,
      resultFile: result,
      mockMode: "mixed",
    });
    expect(merged.verdict).toBe("fail");
    expect(merged.blocking).toBe(1);
    const byPersona = Object.fromEntries(merged.per_persona.map(p => [p.persona, p]));
    expect(byPersona.architect.verdict).toBe("pass");
    expect(byPersona.contrarian.verdict).toBe("fail");
    expect(byPersona.migration.verdict).toBe("pass");
  });

  test("T08d: merged yaml 中の issue id に persona プレフィックス (A-/C-/M-) が付く", async () => {
    const { instr, result } = freshTmpRoot("t08d");
    await dispatchCodex3Persona({
      instructionFile: instr,
      resultFile: result,
      mockMode: "fail",
    });
    const mergedYaml = readFileSync(result, "utf-8");
    expect(mergedYaml).toMatch(/id:\s*A-F01/);
    expect(mergedYaml).toMatch(/id:\s*C-F01/);
    expect(mergedYaml).toMatch(/id:\s*M-F01/);
  });

  test("T08e (codex review F-mig-02): 全 persona issues=[] 時、merged yaml に `issues: []` を明示出力 (後方互換)", async () => {
    const { instr, result } = freshTmpRoot("t08e");
    await dispatchCodex3Persona({
      instructionFile: instr,
      resultFile: result,
      mockMode: "pass",
    });
    const mergedYaml = readFileSync(result, "utf-8");
    expect(mergedYaml).toMatch(/^issues:\s*\[\]\s*$/m);
    expect(mergedYaml).toMatch(/^verdict:\s*pass\s*$/m);
  });
});
