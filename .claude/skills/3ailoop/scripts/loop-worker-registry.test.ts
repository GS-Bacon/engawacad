// loop-worker-registry.test.ts — Phase D-1 tmux N=3 worker 台帳の単体テスト
//
// 検証観点:
// - lifecycle: register → assign → mark-merging → release → next-free の一貫性
// - next-free: 全 busy → null
// - max_workers: N+1 人目の register は失敗
// - release 冪等: idle 状態への release は no-op
// - atomic write: tmp+rename パターンで partial write が残らない
// - serialization: write → read で往復同一

import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import {
  assignIssue,
  firstIdle,
  markError,
  markMerging,
  readRegistry,
  registerWorker,
  releaseWorker,
  unregisterWorker,
  writeRegistry,
  type Registry,
} from "./loop-worker-registry";

let workDir: string;
let registryPath: string;

beforeEach(() => {
  workDir = mkdtempSync(join(tmpdir(), "loop-worker-registry-test-"));
  registryPath = join(workDir, "worker-registry.json");
});

afterEach(() => {
  if (existsSync(workDir)) rmSync(workDir, { recursive: true, force: true });
});

function seedThreeIdle(): Registry {
  let reg: Registry = { workers: {}, max_workers: 3 };
  reg = registerWorker(reg, "worker-1", "%12", "/tmp/w1");
  reg = registerWorker(reg, "worker-2", "%13", "/tmp/w2");
  reg = registerWorker(reg, "worker-3", "%14", "/tmp/w3");
  return reg;
}

describe("T03_worker_registry_lifecycle", () => {
  test("register → assign → mark-merging → release → next-free の流れが期待通り推移する", () => {
    let reg = seedThreeIdle();
    expect(firstIdle(reg)).toBe("worker-1");

    // assign worker-1 に issue 295
    reg = assignIssue(reg, "worker-1", 295);
    expect(reg.workers["worker-1"].state).toBe("busy");
    expect(reg.workers["worker-1"].current_issue).toBe(295);
    expect(reg.workers["worker-1"].assigned_at).not.toBeNull();
    // 次の idle は worker-2
    expect(firstIdle(reg)).toBe("worker-2");

    // mark-merging に遷移
    reg = markMerging(reg, "worker-1");
    expect(reg.workers["worker-1"].state).toBe("merging");
    expect(reg.workers["worker-1"].current_issue).toBe(295); // まだ issue は保持
    expect(firstIdle(reg)).toBe("worker-2");

    // release で idle 復帰
    reg = releaseWorker(reg, "worker-1");
    expect(reg.workers["worker-1"].state).toBe("idle");
    expect(reg.workers["worker-1"].current_issue).toBeNull();
    expect(reg.workers["worker-1"].assigned_at).toBeNull();
    // 次の idle は worker-1 (最初に戻る、ID 昇順)
    expect(firstIdle(reg)).toBe("worker-1");
  });
});

describe("T_bonus_next_free_none", () => {
  test("全 worker が busy なら next-free は null", () => {
    let reg = seedThreeIdle();
    reg = assignIssue(reg, "worker-1", 100);
    reg = assignIssue(reg, "worker-2", 101);
    reg = assignIssue(reg, "worker-3", 102);
    expect(firstIdle(reg)).toBeNull();
  });

  test("merging / error 状態も idle とはみなさない", () => {
    let reg = seedThreeIdle();
    reg = assignIssue(reg, "worker-1", 100);
    reg = markMerging(reg, "worker-1");
    reg = assignIssue(reg, "worker-2", 101);
    reg = markError(reg, "worker-2", "rebase conflict");
    // worker-3 だけが idle
    expect(firstIdle(reg)).toBe("worker-3");
    reg = assignIssue(reg, "worker-3", 102);
    expect(firstIdle(reg)).toBeNull();
  });
});

describe("T_bonus_register_max_workers", () => {
  test("N+1 人目の register は throw (max_workers=3 で 4 人目 NG)", () => {
    let reg = seedThreeIdle();
    expect(() => {
      reg = registerWorker(reg, "worker-4", "%99", "/tmp/w4");
    }).toThrow(/max_workers=3/);
    // 既存 3 人は無傷
    expect(Object.keys(reg.workers).sort()).toEqual(["worker-1", "worker-2", "worker-3"]);
  });

  test("既存 worker-id の再 register は max チェック対象外 (上書きリセット)", () => {
    let reg = seedThreeIdle();
    reg = assignIssue(reg, "worker-1", 500);
    // 同 id を再 register: state は idle にリセット
    reg = registerWorker(reg, "worker-1", "%20", "/tmp/w1-new");
    expect(reg.workers["worker-1"].state).toBe("idle");
    expect(reg.workers["worker-1"].pane_id).toBe("%20");
    expect(reg.workers["worker-1"].worktree).toBe("/tmp/w1-new");
    expect(reg.workers["worker-1"].current_issue).toBeNull();
    expect(Object.keys(reg.workers).length).toBe(3);
  });
});

describe("T_bonus_release_when_idle", () => {
  test("既に idle な worker への release は no-op (エラーにしない)", () => {
    const reg = seedThreeIdle();
    const same = releaseWorker(reg, "worker-1");
    expect(same).toBe(reg); // 参照同一 (no-op)
    expect(same.workers["worker-1"].state).toBe("idle");
  });

  test("未登録 worker への release は throw", () => {
    const reg = seedThreeIdle();
    expect(() => releaseWorker(reg, "worker-99")).toThrow(/not registered/);
  });

  test("error 状態への release は idle にリセット (人手回収後の再投入経路)", () => {
    let reg = seedThreeIdle();
    reg = assignIssue(reg, "worker-1", 300);
    reg = markError(reg, "worker-1", "ci red");
    expect(reg.workers["worker-1"].state).toBe("error");
    reg = releaseWorker(reg, "worker-1");
    expect(reg.workers["worker-1"].state).toBe("idle");
    expect(reg.workers["worker-1"].error_reason).toBeUndefined();
  });
});

describe("T_bonus_atomic_write", () => {
  test("writeRegistry は tmpfile+rename パターンで partial JSON を残さない", () => {
    const reg = seedThreeIdle();
    writeRegistry(registryPath, reg);
    // 書き込み後: registry 本体だけが残り、tmp ファイルは存在しない
    const files = readdirSync(workDir);
    expect(files).toContain("worker-registry.json");
    const leftoverTmps = files.filter(f => f.includes(".tmp."));
    expect(leftoverTmps).toEqual([]);

    // ファイルは valid JSON
    const raw = readFileSync(registryPath, "utf-8");
    expect(() => JSON.parse(raw)).not.toThrow();
  });

  test("破損 JSON があっても readRegistry は empty を返して例外にしない", () => {
    mkdirSync(workDir, { recursive: true });
    writeFileSync(registryPath, "{ this is not: valid json", "utf-8");
    const reg = readRegistry(registryPath);
    expect(reg.workers).toEqual({});
    expect(reg.max_workers).toBeGreaterThan(0);
  });

  test("ファイル不在なら empty registry", () => {
    const missing = join(workDir, "does-not-exist.json");
    const reg = readRegistry(missing);
    expect(reg.workers).toEqual({});
  });
});

describe("T_bonus_serialization", () => {
  test("write → read で内容が往復同一", () => {
    let orig = seedThreeIdle();
    orig = assignIssue(orig, "worker-1", 42);
    orig = markMerging(orig, "worker-1");
    orig = markError(orig, "worker-2", "some reason");

    writeRegistry(registryPath, orig);
    const loaded = readRegistry(registryPath);

    expect(loaded.max_workers).toBe(orig.max_workers);
    expect(Object.keys(loaded.workers).sort()).toEqual(Object.keys(orig.workers).sort());
    for (const id of Object.keys(orig.workers)) {
      expect(loaded.workers[id].pane_id).toBe(orig.workers[id].pane_id);
      expect(loaded.workers[id].worktree).toBe(orig.workers[id].worktree);
      expect(loaded.workers[id].state).toBe(orig.workers[id].state);
      expect(loaded.workers[id].current_issue).toBe(orig.workers[id].current_issue);
      expect(loaded.workers[id].assigned_at).toBe(orig.workers[id].assigned_at);
      expect(loaded.workers[id].error_reason).toBe(orig.workers[id].error_reason);
    }
  });

  test("unregisterWorker は entry を削除する", () => {
    let reg = seedThreeIdle();
    reg = unregisterWorker(reg, "worker-2");
    expect(Object.keys(reg.workers).sort()).toEqual(["worker-1", "worker-3"]);
    // 未登録 id への unregister は no-op
    const same = unregisterWorker(reg, "worker-99");
    expect(same).toBe(reg);
  });
});
