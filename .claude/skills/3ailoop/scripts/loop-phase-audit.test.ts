// loop-phase-audit.test.ts — Issue #319
import { describe, expect, test } from "bun:test";
import { join } from "path";
import {
  type AggregatedFindings,
  type DispatchDeps,
  type Finding,
  type RunDeps,
  aggregateFindings,
  buildInputMarkdown,
  categorizeFindings,
  computeScopeRange,
  parseAdrChanges,
  parseImplDiff,
  renderAuditIssues,
  runAudit,
} from "./loop-phase-audit.ts";

const SCRIPT = join(import.meta.dir, "loop-phase-audit.ts");

// --- helpers ----------------------------------------------------------------

function mkFinding(opts: Partial<Finding> & { source: string; severity: Finding["severity"] }): Finding {
  return {
    source: opts.source,
    severity: opts.severity,
    location: opts.location ?? "crates/foo/src/lib.rs:12",
    summary: opts.summary ?? "some issue",
    detail: opts.detail,
  };
}

function stubDispatch(findings: Finding[]): (input: string) => Promise<Finding[]> {
  return async () => findings;
}

function emptyDispatch(): DispatchDeps {
  const empty = async (): Promise<Finding[]> => [];
  return {
    opus: empty,
    codex: { architect: empty, contrarian: empty, migration: empty },
    glm: { architect: empty, contrarian: empty, migration: empty },
  };
}

function inMemoryDeps(overrides: Partial<RunDeps> = {}): {
  deps: RunDeps;
  writtenFiles: Map<string, string>;
  appendedLog: Array<Record<string, unknown>>;
  createdIssues: Array<{ title: string; labels: string[] }>;
} {
  const writtenFiles = new Map<string, string>();
  const appendedLog: Array<Record<string, unknown>> = [];
  const createdIssues: Array<{ title: string; labels: string[] }> = [];
  let counter = 400;
  const deps: RunDeps = {
    dispatch: emptyDispatch(),
    runGitAdrLog: async () => "",
    runGitImplLog: async () => "",
    readJournalExcerpt: async () => "",
    createIssue: async (draft) => {
      counter += 1;
      createdIssues.push({ title: draft.title, labels: draft.labels });
      return counter;
    },
    appendLog: (entry) => {
      appendedLog.push(entry);
    },
    writeInputFile: (path, body) => {
      writtenFiles.set(path, body);
    },
    ...overrides,
  };
  return { deps, writtenFiles, appendedLog, createdIssues };
}

// --- pure functions ---------------------------------------------------------

describe("computeScopeRange", () => {
  test("T_bonus_scope: phase=14, depth=3 → {from:12, to:14}", () => {
    expect(computeScopeRange(14)).toEqual({ fromPhase: 12, toPhase: 14 });
  });
  test("depth clamps at phase 1", () => {
    expect(computeScopeRange(2, 5)).toEqual({ fromPhase: 1, toPhase: 2 });
  });
  test("depth=1 → single phase", () => {
    expect(computeScopeRange(11, 1)).toEqual({ fromPhase: 11, toPhase: 11 });
  });
});

describe("parseAdrChanges", () => {
  test("複数 commit + docs/decisions 配下のみ拾う", () => {
    const log = [
      "abc1 refactor ADR-001",
      "docs/decisions/001-foo.md",
      "src/other.rs",
      "",
      "def2 ADR-002 追加",
      "docs/decisions/002-bar.md",
      "docs/decisions/002-bar.md.snap",
    ].join("\n");
    const changes = parseAdrChanges(log, { fromPhase: 9, toPhase: 11 });
    expect(changes.length).toBe(3);
    expect(changes[0]).toEqual({ sha: "abc1", path: "docs/decisions/001-foo.md", message: "refactor ADR-001" });
    expect(changes[2].path).toBe("docs/decisions/002-bar.md.snap");
  });

  test("空 log → 空配列", () => {
    expect(parseAdrChanges("", { fromPhase: 1, toPhase: 1 })).toEqual([]);
  });
});

describe("parseImplDiff", () => {
  test("各行の sha + subject を拾う", () => {
    const log = ["aaa1 feat: X", "bbb2 fix: Y", ""].join("\n");
    const out = parseImplDiff(log, { fromPhase: 9, toPhase: 11 });
    expect(out).toEqual([
      { sha: "aaa1", message: "feat: X" },
      { sha: "bbb2", message: "fix: Y" },
    ]);
  });
});

describe("buildInputMarkdown", () => {
  test("3 セクションを含む markdown を返す", () => {
    const md = buildInputMarkdown(
      11,
      { fromPhase: 9, toPhase: 11 },
      [{ sha: "abc1", path: "docs/decisions/010-x.md", message: "add" }],
      [{ sha: "def2", message: "feat: foo" }],
      "cycle 32: ok",
    );
    expect(md).toContain("Phase 11 監査入力");
    expect(md).toContain("Phase 9〜11");
    expect(md).toContain("## ADR 変更");
    expect(md).toContain("docs/decisions/010-x.md");
    expect(md).toContain("## 実装 commit");
    expect(md).toContain("def2 feat: foo");
    expect(md).toContain("## Loop 運用メトリクス");
    expect(md).toContain("cycle 32: ok");
  });

  test("空データでも「該当なし」を出す", () => {
    const md = buildInputMarkdown(11, { fromPhase: 9, toPhase: 11 }, [], [], "");
    expect(md).toContain("(該当なし)");
  });
});

// --- T03: aggregateFindings dedupe / merge ----------------------------------

describe("aggregateFindings", () => {
  test("T03_aggregate_findings: 同じ location+summary は dedupe + source 連結", () => {
    const opus = [mkFinding({ source: "opus", severity: "high", location: "a.rs:1", summary: "dup" })];
    const codex = [
      [mkFinding({ source: "codex-architect", severity: "critical", location: "a.rs:1", summary: "dup" })],
      [],
      [mkFinding({ source: "codex-migration", severity: "low", location: "b.rs:2", summary: "unique-codex" })],
    ];
    const glm = [
      [mkFinding({ source: "glm-architect", severity: "medium", location: "a.rs:1", summary: "dup" })],
      [],
      [],
    ];
    const agg = aggregateFindings(opus, codex, glm);
    // dedupe: a.rs:1::dup は 1 件 (source 3 つ連結), b.rs:2::unique-codex は 1 件
    expect(agg.findings.length).toBe(2);
    const dup = agg.findings.find(f => f.location === "a.rs:1");
    expect(dup).toBeDefined();
    // severity は max (critical)
    expect(dup!.severity).toBe("critical");
    // source に 3 系統ぶん含まれる
    expect(dup!.source).toContain("opus");
    expect(dup!.source).toContain("codex-architect");
    expect(dup!.source).toContain("glm-architect");
    // raw count
    expect(agg.counts_raw).toEqual({ opus: 1, codex: 2, glm: 1 });
  });

  test("空入力 → 空結果", () => {
    const agg = aggregateFindings([], [[], [], []], [[], [], []]);
    expect(agg.findings).toEqual([]);
    expect(agg.counts_raw).toEqual({ opus: 0, codex: 0, glm: 0 });
  });
});

// --- T04: categorize --------------------------------------------------------

describe("categorizeFindings", () => {
  test("T04_categorize: mixed severity を bucket に振り分ける", () => {
    const agg: AggregatedFindings = {
      findings: [
        mkFinding({ source: "opus", severity: "critical", location: "a:1", summary: "c1" }),
        mkFinding({ source: "opus", severity: "critical", location: "a:2", summary: "c2" }),
        mkFinding({ source: "opus", severity: "high", location: "b:1", summary: "h1" }),
        mkFinding({ source: "opus", severity: "medium", location: "c:1", summary: "m1" }),
        mkFinding({ source: "opus", severity: "low", location: "d:1", summary: "l1" }),
        mkFinding({ source: "opus", severity: "low", location: "d:2", summary: "l2" }),
      ],
      counts_raw: { opus: 6, codex: 0, glm: 0 },
    };
    const cat = categorizeFindings(agg);
    expect(cat.critical.length).toBe(2);
    expect(cat.high.length).toBe(1);
    expect(cat.medium.length).toBe(1);
    expect(cat.low.length).toBe(2);
  });
});

// --- T05: renderAuditIssues cap + ordering ----------------------------------

describe("renderAuditIssues", () => {
  test("T05_render_dry_run: 3 crit + 2 high + 1 med + 5 low → 5 Issues (cap), crit/high 優先", () => {
    const cat = {
      critical: [
        mkFinding({ source: "opus", severity: "critical" as const, location: "c:1", summary: "crit1" }),
        mkFinding({ source: "opus", severity: "critical" as const, location: "c:2", summary: "crit2" }),
        mkFinding({ source: "opus", severity: "critical" as const, location: "c:3", summary: "crit3" }),
      ],
      high: [
        mkFinding({ source: "codex", severity: "high" as const, location: "h:1", summary: "high1" }),
        mkFinding({ source: "codex", severity: "high" as const, location: "h:2", summary: "high2" }),
      ],
      medium: [mkFinding({ source: "glm", severity: "medium" as const, location: "m:1", summary: "med1" })],
      low: [
        mkFinding({ source: "opus", severity: "low" as const, location: "l:1", summary: "low1" }),
        mkFinding({ source: "opus", severity: "low" as const, location: "l:2", summary: "low2" }),
        mkFinding({ source: "opus", severity: "low" as const, location: "l:3", summary: "low3" }),
        mkFinding({ source: "opus", severity: "low" as const, location: "l:4", summary: "low4" }),
        mkFinding({ source: "opus", severity: "low" as const, location: "l:5", summary: "low5" }),
      ],
    };
    const drafts = renderAuditIssues(cat, 11, { fromPhase: 9, toPhase: 11 });
    expect(drafts.length).toBe(5); // cap
    // 順序: critical → high → medium (low は落ちる)
    expect(drafts.slice(0, 3).every(d => d.severity === "critical")).toBe(true);
    expect(drafts[3].severity).toBe("high");
    expect(drafts[4].severity).toBe("high");
    // critical は defer ラベル無し、high は defer:phase-12
    expect(drafts[0].labels).not.toContain("defer:phase-12");
    expect(drafts[3].labels).toContain("defer:phase-12");
    // Title に phase 番号
    expect(drafts[0].title).toContain("audit(phase11,critical)");
    expect(drafts[3].title).toContain("audit(phase11,high)");
    // Body に scope
    expect(drafts[0].body).toContain("Phase 9〜11");
  });

  test("low のみ → 0 Issue", () => {
    const cat = {
      critical: [],
      high: [],
      medium: [],
      low: [mkFinding({ source: "opus", severity: "low" as const })],
    };
    const drafts = renderAuditIssues(cat, 11, { fromPhase: 9, toPhase: 11 });
    expect(drafts.length).toBe(0);
  });

  test("空 categorized → 0 Issue", () => {
    const drafts = renderAuditIssues({ critical: [], high: [], medium: [], low: [] }, 11, {
      fromPhase: 9,
      toPhase: 11,
    });
    expect(drafts.length).toBe(0);
  });
});

// --- runAudit: end-to-end (with in-memory deps) ------------------------------

describe("runAudit", () => {
  test("T_bonus_empty_findings: 0 findings → 0 Issues (dry-run は log も書かない)", async () => {
    const { deps, writtenFiles, appendedLog, createdIssues } = inMemoryDeps();
    const result = await runAudit({ phase: 11, dryRun: true, depth: 3 }, deps);
    expect(result.drafts.length).toBe(0);
    expect(createdIssues.length).toBe(0);
    // dry-run は appendLog を呼ばない
    expect(appendedLog.length).toBe(0);
    // input.md は書かれる (scope 情報の永続化)
    expect(writtenFiles.size).toBe(1);
    const inputPath = [...writtenFiles.keys()][0];
    expect(inputPath).toBe("features/.loop/phase-audit-11/input.md");
    expect(writtenFiles.get(inputPath)).toContain("Phase 11 監査入力");
  });

  test("実行モード: findings あり → Issue 起票 + log append", async () => {
    const finding = mkFinding({ source: "codex-architect", severity: "critical", location: "x.rs:1", summary: "boom" });
    const { deps, appendedLog, createdIssues } = inMemoryDeps({
      dispatch: {
        opus: async () => [],
        codex: {
          architect: async () => [finding],
          contrarian: async () => [],
          migration: async () => [],
        },
        glm: {
          architect: async () => [],
          contrarian: async () => [],
          migration: async () => [],
        },
      },
    });
    const result = await runAudit({ phase: 14, dryRun: false, depth: 3 }, deps);
    expect(result.drafts.length).toBe(1);
    expect(createdIssues.length).toBe(1);
    expect(createdIssues[0].labels).toContain("type: foundation");
    expect(createdIssues[0].labels).toContain("batch:kernel");
    // log に entry 1 件
    expect(appendedLog.length).toBe(1);
    expect(appendedLog[0].phase).toBe(14);
    expect(appendedLog[0].scope).toBe("Phase 12-14");
    const findings = appendedLog[0].findings as Record<string, number>;
    expect(findings.critical).toBe(1);
    expect((appendedLog[0].created_issues as number[]).length).toBe(1);
  });
});

// --- CLI end-to-end (subprocess) --------------------------------------------

describe("CLI", () => {
  test("T02_phase_audit_wrong_phase: --phase 10 → exit 2", () => {
    const r = Bun.spawnSync(["bun", SCRIPT, "--phase", "10", "--dry-run"], {
      stdout: "pipe",
      stderr: "pipe",
    });
    expect(r.exitCode).toBe(2);
    const err = new TextDecoder().decode(r.stderr);
    expect(err).toContain("FABLE5_AUDIT_PHASES");
  });

  test("T01_phase_audit_dry_run: --phase 11 --dry-run → 3 系統 dispatch 計画 + input.md preview", () => {
    const r = Bun.spawnSync(["bun", SCRIPT, "--phase", "11", "--dry-run"], {
      stdout: "pipe",
      stderr: "pipe",
    });
    // 実 git log を叩くので repo 内で走らせる想定。exit 0 を期待
    expect(r.exitCode).toBe(0);
    const out = new TextDecoder().decode(r.stdout);
    expect(out).toContain("phase-audit result (phase=11");
    expect(out).toContain("Phase 9-11");
    expect(out).toContain("planned dispatch (3 系統)");
    expect(out).toContain("Opus 4.7");
    expect(out).toContain("Codex 3 persona");
    expect(out).toContain("GLM 3 persona");
    expect(out).toContain("input.md preview");
    expect(out).toContain("Phase 11 監査入力");
    expect(out).toContain("planned issue drafts");
  });

  test("--phase 未指定 → exit 2 with Usage", () => {
    const r = Bun.spawnSync(["bun", SCRIPT, "--dry-run"], { stdout: "pipe", stderr: "pipe" });
    expect(r.exitCode).toBe(2);
    const err = new TextDecoder().decode(r.stderr);
    expect(err).toContain("--phase");
    expect(err).toContain("Usage");
  });
});
