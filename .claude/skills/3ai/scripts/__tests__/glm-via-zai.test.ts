// glm-via-zai.ts: Z.AI 経由 claude -p 呼び出し primitive (#281)

import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { parseEnvFile, runGlmViaZAI } from "../glm-via-zai.ts";

let workDir: string;

beforeEach(() => {
  workDir = mkdtempSync(join(tmpdir(), "glm-via-zai-test-"));
});

afterEach(() => {
  rmSync(workDir, { recursive: true, force: true });
});

describe("parseEnvFile", () => {
  test("KEY=VAL を Record にする", () => {
    const path = join(workDir, ".env");
    writeFileSync(path, "Z_AI_API_KEY=abc123\nOTHER=xyz\n", "utf-8");
    const r = parseEnvFile(path);
    expect(r.Z_AI_API_KEY).toBe("abc123");
    expect(r.OTHER).toBe("xyz");
  });

  test("コメント行と空行はスキップ", () => {
    const path = join(workDir, ".env");
    writeFileSync(path, "# header\n\nZ_AI_API_KEY=k1\n# trailing\n", "utf-8");
    expect(parseEnvFile(path).Z_AI_API_KEY).toBe("k1");
  });

  test("値の引用符は剥がす (single / double)", () => {
    const path = join(workDir, ".env");
    writeFileSync(path, `K1="quoted"\nK2='single'\nK3=bare\n`, "utf-8");
    const r = parseEnvFile(path);
    expect(r.K1).toBe("quoted");
    expect(r.K2).toBe("single");
    expect(r.K3).toBe("bare");
  });

  test("= が無い行は無視", () => {
    const path = join(workDir, ".env");
    writeFileSync(path, "ONLYKEY\nGOOD=val\n", "utf-8");
    const r = parseEnvFile(path);
    expect(r.GOOD).toBe("val");
    expect(r.ONLYKEY).toBeUndefined();
  });
});

describe("runGlmViaZAI smoke (env unavailable paths)", () => {
  test("env file 不在 → ok=false, error に path を含む", async () => {
    const r = await runGlmViaZAI({
      prompt: "noop",
      envFilePath: join(workDir, "does-not-exist.env"),
    });
    expect(r.ok).toBe(false);
    expect(r.exitCode).toBe(-1);
    expect(r.error).toContain("Z.AI env not found");
    expect(r.result).toBe("");
  });

  test("env file あるが Z_AI_API_KEY 無し → ok=false, error が説明的", async () => {
    const path = join(workDir, ".env");
    writeFileSync(path, "OTHER=foo\n", "utf-8");
    const r = await runGlmViaZAI({ prompt: "noop", envFilePath: path });
    expect(r.ok).toBe(false);
    expect(r.exitCode).toBe(-1);
    expect(r.error).toContain("Z_AI_API_KEY missing");
  });
});
