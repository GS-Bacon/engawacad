// loop-tmux-watcher.ts: ring buffer + 30s heartbeat 削減テスト (#233)

import { describe, expect, test } from "bun:test";
import {
  POLL_SEC_DEFAULT,
  RING_BUFFER_MAX_ENTRIES,
  trimRingBuffer,
} from "./loop-tmux-watcher.ts";

describe("trimRingBuffer", () => {
  test("T01 basic: existing=50 + 1 new → 全 51 件残る (max=60)", () => {
    const existing = Array.from({ length: 50 }, (_, i) => `line-${i}`);
    const out = trimRingBuffer(existing, "line-new", 60);
    expect(out.length).toBe(51);
    expect(out[out.length - 1]).toBe("line-new");
    expect(out[0]).toBe("line-0");
  });

  test("T02 overflow: existing=60 + 1 new → 末尾 60 件、最古 (line-0) drop", () => {
    const existing = Array.from({ length: 60 }, (_, i) => `line-${i}`);
    const out = trimRingBuffer(existing, "line-new", 60);
    expect(out.length).toBe(60);
    expect(out[0]).toBe("line-1");
    expect(out[out.length - 1]).toBe("line-new");
    expect(out).not.toContain("line-0");
  });

  test("T03 large overflow: existing=100 + 1 new → 末尾 60 件", () => {
    const existing = Array.from({ length: 100 }, (_, i) => `line-${i}`);
    const out = trimRingBuffer(existing, "line-new", 60);
    expect(out.length).toBe(60);
    expect(out[0]).toBe("line-41");
    expect(out[out.length - 1]).toBe("line-new");
  });

  test("T04_boundary_empty: existing=[] + 1 new → [newLine]", () => {
    const out = trimRingBuffer([], "only-line", 60);
    expect(out).toEqual(["only-line"]);
  });

  test("T04b: existing 中の空行を除外", () => {
    const out = trimRingBuffer(["", "kept-1", "", "kept-2", ""], "new", 60);
    expect(out).toEqual(["kept-1", "kept-2", "new"]);
  });

  test("T05_degen_zero_max: maxEntries=0 → 結果は []", () => {
    const out = trimRingBuffer(["a", "b", "c"], "d", 0);
    expect(out).toEqual([]);
  });

  test("T05b: maxEntries=1 → 末尾 1 件のみ", () => {
    const out = trimRingBuffer(["a", "b", "c"], "d", 1);
    expect(out).toEqual(["d"]);
  });

  test("T05c: 負値 maxEntries → 結果は []", () => {
    const out = trimRingBuffer(["a", "b"], "c", -1);
    expect(out).toEqual([]);
  });
});

describe("watcher 定数", () => {
  test("T06: POLL_SEC_DEFAULT === 30 (heartbeat noise 削減で 10→30 に緩和)", () => {
    expect(POLL_SEC_DEFAULT).toBe(30);
  });

  test("T07: RING_BUFFER_MAX_ENTRIES === 60 (30 min @ 30s)", () => {
    expect(RING_BUFFER_MAX_ENTRIES).toBe(60);
  });
});
