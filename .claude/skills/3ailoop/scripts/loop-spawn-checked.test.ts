// loop-spawn-checked.test.ts — #227 runChecked / LoopCommandError 検証

import { describe, expect, test } from "bun:test";
import { LoopCommandError, runChecked } from "./loop-spawn-checked";

describe("runChecked", () => {
  test("returns stdout/stderr/exitCode on success", async () => {
    const r = await runChecked(["sh", "-c", "echo hello; echo err 1>&2"]);
    expect(r.exitCode).toBe(0);
    expect(r.stdout).toBe("hello\n");
    expect(r.stderr).toBe("err\n");
  });

  test("throws LoopCommandError on non-zero exit (default allowFailure=false)", async () => {
    try {
      await runChecked(["sh", "-c", "echo problem 1>&2; exit 7"]);
      throw new Error("expected throw");
    } catch (e) {
      expect(e).toBeInstanceOf(LoopCommandError);
      const lce = e as LoopCommandError;
      expect(lce.exitCode).toBe(7);
      expect(lce.stderr).toContain("problem");
      expect(lce.message).toContain("Command failed");
      expect(lce.message).toContain("stderr");
      expect(lce.message).toContain("problem");
    }
  });

  test("returns instead of throwing when allowFailure=true", async () => {
    const r = await runChecked(["sh", "-c", "echo failmsg 1>&2; exit 3"], {
      allowFailure: true,
    });
    expect(r.exitCode).toBe(3);
    expect(r.stderr).toContain("failmsg");
  });

  test("preserves stdout even on failure (for debugging)", async () => {
    try {
      await runChecked(["sh", "-c", "echo partial-result; echo bad 1>&2; exit 2"]);
      throw new Error("expected throw");
    } catch (e) {
      expect(e).toBeInstanceOf(LoopCommandError);
      expect((e as LoopCommandError).stdout).toBe("partial-result\n");
    }
  });

  test("truncates very long stderr in error message", async () => {
    const longErr = "x".repeat(1000);
    try {
      await runChecked(["sh", "-c", `printf '%s' '${longErr}' 1>&2; exit 1`]);
      throw new Error("expected throw");
    } catch (e) {
      const lce = e as LoopCommandError;
      expect(lce.message).toContain("truncated");
      // Full stderr should still be available on the error object.
      expect(lce.stderr.length).toBe(1000);
    }
  });

  test("rejects empty cmd", async () => {
    await expect(runChecked([])).rejects.toThrow(/non-empty/);
  });

  test("supports cwd option", async () => {
    const r = await runChecked(["pwd"], { cwd: "/tmp" });
    expect(r.stdout.trim()).toBe("/tmp");
  });

  test("merges env on top of process.env", async () => {
    const r = await runChecked(["sh", "-c", "echo $LOOP_TEST_VAR"], {
      env: { LOOP_TEST_VAR: "hello-env" },
    });
    expect(r.stdout.trim()).toBe("hello-env");
  });
});
