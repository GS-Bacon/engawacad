// Exercise the real dispatcher without starting an external reviewer.
import { expect, test } from "bun:test";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";

const script = resolve(import.meta.dir, "../dispatch-glm-review.ts");

for (const [scenario, expectedCode, expectedVerdict, calls] of [
  ["turn-limit-then-review", 0, "pass", 2],
  ["turn-limit-twice", 2, "error", 2],
  ["transport-failure", 2, "error", 1],
] as const) {
  test(`final review: ${scenario}`, () => {
    const temp = mkdtempSync(join(tmpdir(), "final-review-"));
    try {
      const repo = join(temp, "repo");
      const agents = join(repo, ".claude/skills/3ai/agents");
      const feature = join(temp, "feature");
      mkdirSync(agents, { recursive: true });
      mkdirSync(feature);
      writeFileSync(join(agents, "glm-reviewer-final.md"), "agent instructions");
      writeFileSync(join(feature, "plan.md"), "acceptance plan");
      writeFileSync(join(temp, "env"), "Z_AI_API_KEY=fake-test-key\n");
      const git = (...args: string[]) => {
        const result = Bun.spawnSync(["git", ...args], { cwd: repo });
        expect(result.exitCode).toBe(0);
      };
      git("init", "-q", "-b", "main");
      git("add", ".");
      git("-c", "user.name=Test", "-c", "user.email=test@example.org", "commit", "-qm", "base");
      git("update-ref", "refs/remotes/origin/main", "HEAD");
      git("symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/main");
      // Larger than Linux's per-argument limit; it must reach both calls via stdin.
      writeFileSync(join(repo, "implementation.txt"), "diff-marker-" + "x".repeat(150_000));
      git("add", ".");
      git("-c", "user.name=Test", "-c", "user.email=test@example.org", "commit", "-qm", "implementation");

      writeFileSync(join(temp, "gh"), "#!/bin/sh\necho '{\"title\":\"test\",\"body\":\"intent-marker\",\"number\":326}'\n");
      writeFileSync(join(temp, "claude"), `#!/usr/bin/env bun
import { appendFileSync, existsSync } from "node:fs";
const retry = existsSync(process.env.CALLS);
const input = await Bun.stdin.text();
appendFileSync(process.env.CALLS, JSON.stringify({args: process.argv.slice(2), hasCode: input.includes("diff-marker-"), hasIssue: input.includes("intent-marker"), hasPlan: input.includes("acceptance plan")}) + "\\n");
if (process.env.SCENARIO === "transport-failure") process.exit(23);
if (retry && process.env.SCENARIO === "turn-limit-then-review") {
  console.log("issues: []\\nverdict: pass");
  process.exit(0);
}
console.log("Error: Reached max turns (30)");
process.exit(1);
`);
      for (const name of ["gh", "claude"]) chmodSync(join(temp, name), 0o755);
      const callsFile = join(temp, "calls");
      const result = Bun.spawnSync([
        process.execPath, script, "--persona", "final", "--issue", "326",
        "--feature-dir", feature, "--result", join(feature, "result.yaml"),
      ], {
        cwd: repo,
        env: { ...process.env, PATH: temp + ":" + process.env.PATH, ZAI_ENV: join(temp, "env"),
               CALLS: callsFile, SCENARIO: scenario, GLM_MAX_TURNS: "30" },
      });
      expect(result.exitCode).toBe(expectedCode);
      const verdict = JSON.parse(readFileSync(join(feature, "result.verdict.json"), "utf8"));
      expect(verdict.verdict).toBe(expectedVerdict);
      if (scenario === "turn-limit-twice") expect(verdict.reason).toBe("max-turns");
      if (scenario === "transport-failure") expect(verdict.reason).toBe("claude-exit-23");
      const attempts = readFileSync(callsFile, "utf8").trim().split("\n").map(line => JSON.parse(line));
      expect(attempts).toHaveLength(calls);
      for (const attempt of attempts) {
        expect(attempt.hasCode).toBe(true);
        expect(attempt.hasIssue).toBe(true);
      }
      if (calls === 2) {
        const { args, hasPlan } = attempts[1];
        expect(hasPlan).toBe(true);
        expect(args).toContain("--tools=");
        expect(args).toContain("--strict-mcp-config");
        expect(args[args.indexOf("--max-turns") + 1]).toBe("1");
        expect(readFileSync(join(feature, "result.yaml.log"), "utf8")).toContain("Error: Reached max turns");
      }
    } finally {
      rmSync(temp, { recursive: true, force: true });
    }
  }, 30_000);
}
