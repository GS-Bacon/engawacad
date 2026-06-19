import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { autoAccept, isAllCodexFailed, type PersonaVerdict } from "./loop-adr-auto-accept";

let workDir: string;
let originalCwd: string;
let adrDir: string;

const GOOD_ADR = [
  "# ADR-X: テスト用",
  "## Decision",
  "- A. opt1",
  "- B. opt2",
  "- C. opt3",
  "Trade-off: A 速い B 遅い C 中間",
  "採用: A",
  "## Alternatives Considered",
  "### (a) opt1 详细",
  "### (b) opt2 详细",
  "### (c) opt3 详细",
  "## Consequences",
  "採用前提崩壊 trigger: foo が観測されたら見直し",
  "Related: ADR-001",
].join("\n");

const BAD_ADR = [
  "# ADR-Y: 不完全",
  "## Decision",
  "- A. opt1",
  "## Consequences",
].join("\n");

beforeEach(() => {
  originalCwd = process.cwd();
  workDir = mkdtempSync(join(tmpdir(), "adr-autoaccept-test-"));
  process.chdir(workDir);
  adrDir = join(workDir, "docs/decisions");
  mkdirSync(adrDir, { recursive: true });
});

afterEach(() => {
  process.chdir(originalCwd);
  rmSync(workDir, { recursive: true, force: true });
  delete process.env.ADR_AUTOACCEPT_MOCK;
  delete process.env.ADR_AUTOACCEPT_MOCK_CODEX;
});

describe("loop-adr-auto-accept", () => {
  test("良い ADR + mock=pass で accepted を返す (dry-run)", async () => {
    const path = join(adrDir, "999-test.md");
    writeFileSync(path, GOOD_ADR, "utf-8");
    process.env.ADR_AUTOACCEPT_MOCK = "pass";
    const r = await autoAccept({ adrPath: path, issueNum: 99999, dryRun: true });
    expect(r.kind).toBe("accepted");
  });

  test("不完全な ADR は regen_required (reason=lint)", async () => {
    const path = join(adrDir, "998-bad.md");
    writeFileSync(path, BAD_ADR, "utf-8");
    const r = await autoAccept({ adrPath: path, issueNum: 99998, dryRun: true });
    expect(r.kind).toBe("regen_required");
    if (r.kind === "regen_required") {
      expect(r.reason).toBe("lint");
      expect(r.details.length).toBeGreaterThan(0);
    }
  });

  test("lint fail を 4 回繰り返すと retired (reason=regen_cap)", async () => {
    const path = join(adrDir, "997-cap.md");
    writeFileSync(path, BAD_ADR, "utf-8");
    let last;
    for (let i = 0; i < 4; i++) {
      last = await autoAccept({ adrPath: path, issueNum: 99997, dryRun: true });
    }
    expect(last!.kind).toBe("retired");
    if (last!.kind === "retired") expect(last!.reason).toBe("regen_cap");
  });

  test("review で refute されると regen_required (reason=review)", async () => {
    const path = join(adrDir, "996-refute.md");
    writeFileSync(path, GOOD_ADR, "utf-8");
    process.env.ADR_AUTOACCEPT_MOCK = "refute";
    const r = await autoAccept({ adrPath: path, issueNum: 99996, dryRun: false });
    expect(r.kind).toBe("regen_required");
    if (r.kind === "regen_required") expect(r.reason).toBe("review");
  });

  test("token 予算を意図的に超過させると retired (reason=token_cap)", async () => {
    const path = join(adrDir, "995-tok.md");
    writeFileSync(path, GOOD_ADR, "utf-8");
    process.env.ADR_AUTOACCEPT_MOCK = "pass";
    const r = await autoAccept({
      adrPath: path,
      issueNum: 99995,
      dryRun: true,
      estimatedReviewTokens: 300_000,
    });
    expect(r.kind).toBe("retired");
    if (r.kind === "retired") expect(r.reason).toBe("token_cap");
  });

  // #251: Codex usage limit fallback (GLM 3 persona)
  test("#251: Codex 全員 fail で GLM fallback ルートに乗り、glm-refute なら regen_required", async () => {
    const path = join(adrDir, "994-glm-fb.md");
    writeFileSync(path, GOOD_ADR, "utf-8");
    process.env.ADR_AUTOACCEPT_MOCK_CODEX = "fail";  // Codex 全 persona を usage limit fail に
    process.env.ADR_AUTOACCEPT_MOCK = "glm-refute"; // GLM fallback 側は refute
    const r = await autoAccept({ adrPath: path, issueNum: 99994, dryRun: false });
    expect(r.kind).toBe("regen_required");
    if (r.kind === "regen_required") expect(r.reason).toBe("review");

    // outDir に Codex / GLM 両方の verdicts が記録されている
    const slug = "994-glm-fb";
    const outDir = join(workDir, `features/.loop/adr-review/${slug}`);
    const codexVer = JSON.parse(readFileSync(join(outDir, "verdicts.codex.json"), "utf-8"));
    const glmVer = JSON.parse(readFileSync(join(outDir, "verdicts.glm.json"), "utf-8"));
    const combined = JSON.parse(readFileSync(join(outDir, "verdicts.json"), "utf-8"));
    expect(codexVer.length).toBe(3);
    expect(glmVer.length).toBe(3);
    expect(combined.fallback_used).toBe(true);
  });
});

describe("isAllCodexFailed (#251)", () => {
  const v = (approved: boolean, raw: string): PersonaVerdict => ({ persona: "x", approved, raw_excerpt: raw });

  test("空配列 → false", () => {
    expect(isAllCodexFailed([])).toBe(false);
  });

  test("3 個とも [codex exit=...] crash → true", () => {
    expect(isAllCodexFailed([
      v(false, "[codex exit=1] no stderr"),
      v(false, "[codex exit=137] killed"),
      v(false, "[codex exit=2] panic"),
    ])).toBe(true);
  });

  test("3 個とも usage limit シグナル → true", () => {
    expect(isAllCodexFailed([
      v(false, "you've hit your usage limit"),
      v(false, "rate limit reached"),
      v(false, "HTTP 429 too many requests"),
    ])).toBe(true);
  });

  test("crash + usage limit mix → true", () => {
    expect(isAllCodexFailed([
      v(false, "[codex exit=1] hit your usage limit"),
      v(false, "rate limit"),
      v(false, "[codex exit=137]"),
    ])).toBe(true);
  });

  test("1 個でも approved があれば false (= refute 経路に乗る)", () => {
    expect(isAllCodexFailed([
      v(true, "verdict: approved"),
      v(false, "[codex exit=1] usage limit"),
      v(false, "[codex exit=1] usage limit"),
    ])).toBe(false);
  });

  test("通常 refute (Codex 動作) → false (= fallback 不発)", () => {
    expect(isAllCodexFailed([
      v(false, "verdict: refuted — 採用 option は X で破綻する"),
      v(false, "verdict: refuted — 既存 ADR-002 と矛盾"),
      v(false, "verdict: refuted — 後方互換性が壊れる"),
    ])).toBe(false);
  });
});
