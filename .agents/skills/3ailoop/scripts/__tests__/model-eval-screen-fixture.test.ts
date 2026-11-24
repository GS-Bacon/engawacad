import { afterEach, describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "fs";
import { join } from "path";
import { tmpdir } from "os";
import { baseline, createScreenFixture } from "../model-eval-screen.ts";
import { completeFixtureChanges } from "../lib/evaluation-fixture.ts";

describe("model evaluation screen fixture", () => {
  let root: string;
  afterEach(() => rmSync(root, { recursive: true, force: true }));

  test("tracks Cargo.lock in the baseline before the first grader test", () => {
    root = mkdtempSync(join(tmpdir(), "screen-baseline-"));
    createScreenFixture(root);
    const before = baseline(root);
    expect(Bun.spawnSync(["git", "cat-file", "-e", `${before}:Cargo.lock`], { cwd: root }).exitCode).toBe(0);

    // The worker starts with a deliberately failing test. Generate Cargo's local
    // artifacts even when the developer normally uses a shared CARGO_TARGET_DIR.
    const testRun = Bun.spawnSync(["cargo", "test", "--quiet", "--target-dir", "target"], { cwd: root });
    expect(testRun.exitCode).toBe(101);
    expect(testRun.stdout.toString()).toContain("tests::ids_are_zero_based_and_deterministic --- FAILED");
    expect(completeFixtureChanges(root, before)).toEqual([]);
  });
});
