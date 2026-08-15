import { describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, writeFileSync } from "fs";
import { join } from "path";
import { tmpdir } from "os";
import { baseline } from "../model-eval-screen.ts";
import { completeFixtureChanges } from "../lib/evaluation-fixture.ts";

describe("model evaluation screen fixture", () => {
  test("tracks Cargo.lock in the baseline before the first grader test", () => {
    const root = mkdtempSync(join(tmpdir(), "screen-baseline-"));
    mkdirSync(join(root, "src"));
    writeFileSync(join(root, "Cargo.toml"), '[package]\nname="fixture"\nversion="0.1.0"\nedition="2021"\n');
    writeFileSync(join(root, "src/lib.rs"), "pub fn value() -> u8 { 1 }\n");
    const before = baseline(root);
    const testRun = Bun.spawnSync(["cargo", "test", "--quiet"], { cwd: root, stdout: "pipe", stderr: "pipe" });
    expect(testRun.exitCode).toBe(0);
    expect(completeFixtureChanges(root, before)).toEqual([]);
  });
});
