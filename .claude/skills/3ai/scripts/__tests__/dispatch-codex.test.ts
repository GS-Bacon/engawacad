// dispatch-codex.ts: persona injection & exit code return テスト (#231)

import { describe, expect, test, beforeEach } from "bun:test";
import { mkdirSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { buildPrefix, detectCodexUsageLimit, dispatchCodex, PERSONA_HINTS } from "../dispatch-codex.ts";

const TMP_BASE = join(tmpdir(), `dispatch-codex-test-${process.pid}`);

function freshTmpRoot(label: string): { dir: string; instr: string } {
  const dir = join(TMP_BASE, label);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const instr = join(dir, "agent.md");
  writeFileSync(instr, "# Mock reviewer\n返答は yaml 1 行で。\n", "utf-8");
  return { dir, instr };
}

beforeEach(() => {
  delete process.env.CODEX_DRY_RUN;
});

describe("dispatch-codex persona injection", () => {
  test("T01: persona=architect → buildPrefix に PERSONA SCOPE ブロックと architect 役割が含まれる", () => {
    const out = buildPrefix("", "", "architect");
    expect(out).toContain("PERSONA SCOPE");
    expect(out).toContain("architect");
    expect(out).toContain(PERSONA_HINTS.architect.split("\n")[0]);
  });

  test("T02: persona 未指定 (single) → PERSONA SCOPE ブロックなし (legacy 維持)", () => {
    const out = buildPrefix("", "");
    expect(out).not.toContain("PERSONA SCOPE");
    expect(out).toBe("");
  });

  test("T02b: persona=single 明示指定でも PERSONA SCOPE ブロックなし", () => {
    const out = buildPrefix("scope-text", "", "single");
    expect(out).not.toContain("PERSONA SCOPE");
    expect(out).toContain("SCOPE PROFILE");
    expect(out).toContain("scope-text");
  });

  test("T02c: persona と scopeHint 併用時、PERSONA が先頭、SCOPE PROFILE が次", () => {
    const out = buildPrefix("scope-text", "", "contrarian");
    const personaIdx = out.indexOf("PERSONA SCOPE");
    const scopeIdx = out.indexOf("SCOPE PROFILE");
    expect(personaIdx).toBeGreaterThanOrEqual(0);
    expect(scopeIdx).toBeGreaterThan(personaIdx);
  });
});

describe("dispatch-codex exit code return", () => {
  test("T03: CODEX_DRY_RUN=1 + design mode で dispatchCodex の戻り値が 0 (process.exit しない)", async () => {
    const { dir, instr } = freshTmpRoot("t03");
    const inputFile = join(dir, "input.md");
    writeFileSync(inputFile, "design input mock", "utf-8");
    process.env.CODEX_DRY_RUN = "1";
    const code = await dispatchCodex({
      mode: "design",
      instructionFile: instr,
      resultFile: join(dir, "result.yaml"),
      inputFile,
      persona: "architect",
    });
    expect(code).toBe(0);
  });
});

describe("detectCodexUsageLimit (#250)", () => {
  test("T06: 'You\\'ve hit your usage limit, reset 6:06 PM' → true", () => {
    expect(detectCodexUsageLimit("You've hit your usage limit, reset 6:06 PM")).toBe(true);
  });

  test("T07: 大文字 'USAGE LIMIT' → true", () => {
    expect(detectCodexUsageLimit("USAGE LIMIT EXCEEDED")).toBe(true);
  });

  test("T08: 'rate limit' → true", () => {
    expect(detectCodexUsageLimit("API rate limit exceeded")).toBe(true);
  });

  test("T09: HTTP 429 単体 → true", () => {
    expect(detectCodexUsageLimit("HTTP 429 Too Many Requests")).toBe(true);
  });

  test("T10: 通常 stdout → false", () => {
    expect(detectCodexUsageLimit("review yaml generated successfully")).toBe(false);
  });

  test("T11: 空文字 → false", () => {
    expect(detectCodexUsageLimit("")).toBe(false);
  });

  test("T12: '1429' は 429 単独ではないので false", () => {
    expect(detectCodexUsageLimit("error 1429 unrelated")).toBe(false);
  });
});

describe("dispatch-codex PERSONA_HINTS 定義整合性", () => {
  test("T04: 全 persona key (single/architect/contrarian/migration) が定義済み", () => {
    expect(Object.keys(PERSONA_HINTS).sort()).toEqual(
      ["architect", "contrarian", "migration", "single"],
    );
  });

  test("T05: single hint は空文字、他 3 persona は非空", () => {
    expect(PERSONA_HINTS.single).toBe("");
    expect(PERSONA_HINTS.architect.length).toBeGreaterThan(20);
    expect(PERSONA_HINTS.contrarian.length).toBeGreaterThan(20);
    expect(PERSONA_HINTS.migration.length).toBeGreaterThan(20);
  });
});
