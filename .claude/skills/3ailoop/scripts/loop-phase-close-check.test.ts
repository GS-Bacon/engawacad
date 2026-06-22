// #271: split parent auto-close を単体テスト
import { describe, expect, test } from "bun:test";
import { autoCloseFinishedSplitParents } from "./loop-phase-close-check";

type IssueLite = { number: number; title: string; labels: { name: string }[]; state: string };

describe("autoCloseFinishedSplitParents (#271)", () => {
  test("blocked-by-split ラベルなしの親 → スキップ (close されない)", async () => {
    const parents: IssueLite[] = [
      { number: 100, title: "normal", labels: [{ name: "type: feature" }], state: "OPEN" },
    ];
    let called = false;
    const ghFn = async () => { called = true; return { stdout: "", exit: 0 }; };
    const closed = await autoCloseFinishedSplitParents(parents, ghFn);
    expect(closed).toEqual([]);
    expect(called).toBe(false);
  });

  test("split parent で全子 closed → 親を auto-close", async () => {
    const parents: IssueLite[] = [
      { number: 194, title: "Phase 9 起点", labels: [{ name: "blocked-by-split" }], state: "OPEN" },
    ];
    const calls: string[][] = [];
    const ghFn = async (args: string[]) => {
      calls.push(args);
      if (args[0] === "issue" && args[1] === "list") {
        return {
          stdout: JSON.stringify([
            { number: 255, state: "CLOSED" }, { number: 256, state: "CLOSED" },
            { number: 257, state: "CLOSED" }, { number: 258, state: "CLOSED" },
            { number: 259, state: "CLOSED" }, { number: 260, state: "CLOSED" },
          ]),
          exit: 0,
        };
      }
      if (args[0] === "issue" && args[1] === "close") {
        return { stdout: "closed", exit: 0 };
      }
      return { stdout: "", exit: 0 };
    };
    const closed = await autoCloseFinishedSplitParents(parents, ghFn);
    expect(closed).toEqual([194]);
    // 子検索 + close の 2 回呼ばれている
    expect(calls.length).toBe(2);
    expect(calls[0]).toContain("parent-blocked-by-split:194");
    expect(calls[1]).toContain("close");
  });

  test("split parent で 1 件でも子 open → 親を close しない", async () => {
    const parents: IssueLite[] = [
      { number: 194, title: "Phase 9 起点", labels: [{ name: "blocked-by-split" }], state: "OPEN" },
    ];
    const ghFn = async (args: string[]) => {
      if (args[0] === "issue" && args[1] === "list") {
        return {
          stdout: JSON.stringify([
            { number: 255, state: "CLOSED" }, { number: 256, state: "OPEN" },
          ]),
          exit: 0,
        };
      }
      return { stdout: "", exit: 0 };
    };
    const closed = await autoCloseFinishedSplitParents(parents, ghFn);
    expect(closed).toEqual([]);
  });

  test("子 Issue ゼロ件 → 親を close しない (split が起票失敗した状態を保護)", async () => {
    const parents: IssueLite[] = [
      { number: 999, title: "lonely parent", labels: [{ name: "blocked-by-split" }], state: "OPEN" },
    ];
    const ghFn = async () => ({ stdout: "[]", exit: 0 });
    const closed = await autoCloseFinishedSplitParents(parents, ghFn);
    expect(closed).toEqual([]);
  });

  test("gh issue list 失敗 → スキップして次に進む", async () => {
    const parents: IssueLite[] = [
      { number: 194, title: "p", labels: [{ name: "blocked-by-split" }], state: "OPEN" },
      { number: 195, title: "q", labels: [{ name: "blocked-by-split" }], state: "OPEN" },
    ];
    const ghFn = async (args: string[]) => {
      if (args.includes("parent-blocked-by-split:194")) return { stdout: "", exit: 1 };
      if (args.includes("parent-blocked-by-split:195")) {
        return { stdout: JSON.stringify([{ number: 300, state: "CLOSED" }]), exit: 0 };
      }
      if (args[0] === "issue" && args[1] === "close") return { stdout: "ok", exit: 0 };
      return { stdout: "", exit: 0 };
    };
    const closed = await autoCloseFinishedSplitParents(parents, ghFn);
    // #194 はスキップ、#195 は close 成功
    expect(closed).toEqual([195]);
  });

  test("gh issue close 失敗 → close 配列に含まれない", async () => {
    const parents: IssueLite[] = [
      { number: 194, title: "p", labels: [{ name: "blocked-by-split" }], state: "OPEN" },
    ];
    const ghFn = async (args: string[]) => {
      if (args[0] === "issue" && args[1] === "list") {
        return { stdout: JSON.stringify([{ number: 255, state: "CLOSED" }]), exit: 0 };
      }
      if (args[0] === "issue" && args[1] === "close") return { stdout: "fail", exit: 1 };
      return { stdout: "", exit: 0 };
    };
    const closed = await autoCloseFinishedSplitParents(parents, ghFn);
    expect(closed).toEqual([]);
  });

  test("複数 split parent が同時に成熟 → 全部 close", async () => {
    const parents: IssueLite[] = [
      { number: 194, title: "a", labels: [{ name: "blocked-by-split" }], state: "OPEN" },
      { number: 195, title: "b", labels: [{ name: "blocked-by-split" }], state: "OPEN" },
    ];
    const ghFn = async (args: string[]) => {
      if (args[0] === "issue" && args[1] === "list") {
        return { stdout: JSON.stringify([{ number: 500, state: "CLOSED" }]), exit: 0 };
      }
      if (args[0] === "issue" && args[1] === "close") return { stdout: "ok", exit: 0 };
      return { stdout: "", exit: 0 };
    };
    const closed = await autoCloseFinishedSplitParents(parents, ghFn);
    expect(closed).toEqual([194, 195]);
  });

  test("非 split parent と split parent が混在 → split parent だけ処理", async () => {
    const parents: IssueLite[] = [
      { number: 100, title: "normal", labels: [{ name: "type: feature" }], state: "OPEN" },
      { number: 194, title: "split", labels: [{ name: "blocked-by-split" }], state: "OPEN" },
    ];
    const ghFn = async (args: string[]) => {
      if (args[0] === "issue" && args[1] === "list") {
        return { stdout: JSON.stringify([{ number: 500, state: "CLOSED" }]), exit: 0 };
      }
      if (args[0] === "issue" && args[1] === "close") return { stdout: "ok", exit: 0 };
      return { stdout: "", exit: 0 };
    };
    const closed = await autoCloseFinishedSplitParents(parents, ghFn);
    expect(closed).toEqual([194]);
  });
});
