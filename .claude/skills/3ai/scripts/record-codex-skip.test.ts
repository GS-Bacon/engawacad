// record-codex-skip.test.ts — Codex gate スキップ台帳の append / read / atomic write 検証

import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { appendFileSync, mkdtempSync, readFileSync, rmSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import {
  appendSkip,
  buildSkipEntry,
  readLedger,
  resolveLedgerPath,
  writeLedger,
  DEFAULT_LEDGER,
} from "./record-codex-skip.ts";

let workDir: string;
let ledger: string;

beforeEach(() => {
  workDir = mkdtempSync(join(tmpdir(), "record-codex-skip-"));
  ledger = join(workDir, "codex-skips.jsonl");
});

afterEach(() => {
  rmSync(workDir, { recursive: true, force: true });
});

describe("record-codex-skip", () => {
  test("buildSkipEntry は resolved_at:null + ISO recorded_at を持つ", () => {
    const e = buildSkipEntry(42, "my-slug", "3.5", "usage limit hit", new Date("2026-07-06T01:02:03Z"));
    expect(e.issue).toBe(42);
    expect(e.slug).toBe("my-slug");
    expect(e.step).toBe("3.5");
    expect(e.reason).toBe("usage limit hit");
    expect(e.resolved_at).toBeNull();
    expect(e.recorded_at).toBe("2026-07-06T01:02:03.000Z");
  });

  test("appendSkip → readLedger で往復する", () => {
    appendSkip(buildSkipEntry(1, "a", "3.5", "r1"), ledger);
    appendSkip(buildSkipEntry(2, "b", "7.5", "r2"), ledger);
    const entries = readLedger(ledger);
    expect(entries.map((e) => e.issue)).toEqual([1, 2]);
    expect(entries[1].step).toBe("7.5");
  });

  test("readLedger は破損行を読み飛ばす", () => {
    appendSkip(buildSkipEntry(1, "a", "3.5", "r1"), ledger);
    appendFileSync(ledger, "not json at all\n");
    appendFileSync(ledger, "\n"); // 空行も無視
    appendSkip(buildSkipEntry(2, "b", "7.5", "r2"), ledger);
    const entries = readLedger(ledger);
    expect(entries.map((e) => e.issue)).toEqual([1, 2]);
  });

  test("readLedger は不在ファイルで空配列", () => {
    expect(readLedger(join(workDir, "missing.jsonl"))).toEqual([]);
  });

  test("writeLedger は atomic に全体を書き換える (resolved_at 更新)", () => {
    appendSkip(buildSkipEntry(1, "a", "3.5", "r1"), ledger);
    appendSkip(buildSkipEntry(2, "b", "7.5", "r2"), ledger);
    const entries = readLedger(ledger);
    entries[0].resolved_at = "2026-07-06T00:00:00.000Z";
    entries[0].resolution = "reviewed-clean";
    writeLedger(entries, ledger);

    const reread = readLedger(ledger);
    expect(reread[0].resolved_at).toBe("2026-07-06T00:00:00.000Z");
    expect(reread[0].resolution).toBe("reviewed-clean");
    expect(reread[1].resolved_at).toBeNull();
    // 末尾改行つきで壊れないこと
    expect(readFileSync(ledger, "utf-8").endsWith("\n")).toBe(true);
  });

  test("writeLedger は空配列でも壊れない", () => {
    writeLedger([], ledger);
    expect(readLedger(ledger)).toEqual([]);
  });

  test("resolveLedgerPath: 明示 > env > default の優先順", () => {
    const prev = process.env.CODEX_SKIPS_LEDGER;
    try {
      delete process.env.CODEX_SKIPS_LEDGER;
      expect(resolveLedgerPath()).toBe(DEFAULT_LEDGER);
      process.env.CODEX_SKIPS_LEDGER = "/env/path.jsonl";
      expect(resolveLedgerPath()).toBe("/env/path.jsonl");
      expect(resolveLedgerPath("/explicit.jsonl")).toBe("/explicit.jsonl");
    } finally {
      if (prev === undefined) delete process.env.CODEX_SKIPS_LEDGER;
      else process.env.CODEX_SKIPS_LEDGER = prev;
    }
  });
});
