import { describe, expect, test } from "bun:test";
import { buildPaneScript, shouldRetainPane } from "../visible-command.ts";

describe("visible-command pane cleanup policy", () => {
  test("releases completed panes by default", () => {
    expect(shouldRetainPane([], {})).toBe(false);
  });

  test("retains only through an explicit debug setting", () => {
    expect(shouldRetainPane(["--retain-pane"], {})).toBe(true);
    expect(shouldRetainPane([], { LOOP_RETAIN_WORKER_PANE: "1" })).toBe(true);
  });

  test("worker command arguments are outside the option slice", () => {
    const args = ["--label", "worker", "--", "tool", "--retain-pane"];
    const separator = args.indexOf("--");
    expect(shouldRetainPane(args.slice(0, separator), {})).toBe(false);
  });
});

describe("visible-command dashboard wiring", () => {
  test("pipes the raw process stream through the pane renderer", () => {
    const script = buildPaneScript({
      cwd: "/tmp/work tree",
      label: "model lab",
      command: ["opencode", "run", "hello world"],
      logPath: "/tmp/raw.log",
      exitPath: "/tmp/result.exit",
      channel: "done-channel",
      rendererPath: "/skills/pane-render.ts",
      retainPane: false,
    });

    expect(script).toContain("pane-render.ts");
    expect(script).toContain("--log '/tmp/raw.log'");
    expect(script).toContain("rc=${PIPESTATUS[0]}");
    expect(script).not.toContain("tee -a");
    expect(script).toContain("tmux wait-for -S 'done-channel'");
  });

  test("enables the live-score renderer explicitly for model evaluation", () => {
    const script = buildPaneScript({
      cwd: "/tmp/work", label: "selection", command: ["bun", "screen.ts"],
      logPath: "/tmp/raw.log", exitPath: "/tmp/result.exit", channel: "done",
      rendererPath: "/skills/pane-render.ts", retainPane: false,
      evaluationModels: "opencode/a-free,opencode/b-free",
    });
    expect(script).toContain("--evaluation-models 'opencode/a-free,opencode/b-free'");
  });
});
