#!/usr/bin/env bun
// dispatch-codex-auto.ts — issue 種別を自動判定して dispatch-codex.ts を呼ぶラッパー
// ループ上限超過時は exit 3 で終了(エスカレーションシグナル)。

import { readFileSync, writeFileSync, existsSync, mkdirSync, copyFileSync, unlinkSync } from "fs";
import { getReviewConfig } from "./get-review-config.ts";
import { incState } from "./state.ts";
import { dispatchCodex } from "./dispatch-codex.ts";
import { runChecked, LoopCommandError } from "../../3ailoop/scripts/loop-spawn-checked.ts";

interface Opts {
  issueNum: string;
  mode: "design" | "final";
  inputFile?: string;
  baseBranch?: string;
  planFile?: string;
  stateFile: string;
  resultFile: string;
  rejectionFile?: string;
  adrContextFile?: string;
  judgmentSummaryFile?: string;
  planSnapshotDir?: string;
  testSummaryFile?: string;
}

export async function dispatchCodexAuto(opts: Opts): Promise<void> {
  const { issueNum, mode, stateFile, resultFile } = opts;

  // detect_base for final mode
  // #227: git symbolic-ref が失敗するケース (origin/HEAD 未設定 = clone --no-tags 等) を silent で
  // 通すと baseBranch が "" のまま dispatch-codex に渡って diff が壊れる。runChecked で throw。
  if (mode === "final" && !opts.baseBranch) {
    try {
      const r = await runChecked(["git", "symbolic-ref", "refs/remotes/origin/HEAD"]);
      const ref = r.stdout.trim();
      opts.baseBranch = ref.replace("refs/remotes/origin/", "");
      if (!opts.baseBranch) throw new Error("--base unset and origin/HEAD undetectable");
    } catch (e) {
      if (e instanceof LoopCommandError) {
        throw new Error(
          `--base unset and 'git symbolic-ref refs/remotes/origin/HEAD' failed (exit ${e.exitCode}): ${e.stderr.trim()}`,
        );
      }
      throw e;
    }
  }

  const config = await getReviewConfig(issueNum, mode, {
    diffBase: opts.baseBranch,
    planFile: opts.planFile,
  });
  const { deliverable, reviewInstructionPath, maxLoops, scopeHint } = config;

  const counter = mode === "final" ? "final_loops" : "design_loops";
  const n = incState(stateFile, counter);

  if (n > maxLoops) {
    process.stderr.write(`=== dispatch-codex-auto: ESCALATION ===\n`);
    process.stderr.write(`  issue=${issueNum} mode=${mode} deliverable=${deliverable}\n`);
    process.stderr.write(`  ループ上限 ${maxLoops} 回に達しました(今回 ${n} 回目)。\n`);
    process.stderr.write(`  停止してユーザーにエスカレーションしてください。\n`);
    process.exit(3);
  }

  process.stderr.write(
    `=== dispatch-codex-auto: issue=${issueNum} mode=${mode} deliverable=${deliverable} loop=${n}/${maxLoops} ===\n`
  );

  let extraInputFile: string | undefined;
  let finalExtraFile: string | undefined;

  if (mode === "design" && opts.planFile) {
    if (!opts.inputFile) throw new Error("--input required for mode=design");

    const planText = readFileSync(opts.planFile, "utf-8");
    if (!planText.includes("## Non-Goals")) {
      process.stderr.write(
        "ERROR: plan に '## Non-Goals' セクションが必須です。実装しない項目を明記するか '- 該当なし' と書いてください。\n"
      );
      process.exit(1);
    }

    // snapshot
    if (opts.planSnapshotDir) {
      mkdirSync(opts.planSnapshotDir, { recursive: true });
      copyFileSync(opts.planFile, `${opts.planSnapshotDir}/plan.md.round-${n}`);
    }

    let extra = "";

    // ① Issue context
    // #227: gh exit code を検証して取得失敗時は stderr を context に残す (silent fail で空 context にしない)。
    try {
      const r = await runChecked(
        ["gh", "issue", "view", issueNum, "--json", "title,body,number"],
        { allowFailure: true },
      );
      if (r.exitCode === 0) {
        const json = JSON.parse(r.stdout);
        const issueText = `Issue #${json.number ?? ""} ${json.title}\n\n${json.body}`;
        extra += `===== ISSUE CONTEXT =====\n${issueText}\n===== END ISSUE CONTEXT =====\n\n`;
      } else {
        const errSnippet = r.stderr.trim().slice(0, 200);
        extra += `===== ISSUE CONTEXT =====\n(Issue 取得失敗 exit=${r.exitCode}: ${errSnippet})\n===== END ISSUE CONTEXT =====\n\n`;
        process.stderr.write(`WARN: gh issue view ${issueNum} exit=${r.exitCode}: ${errSnippet}\n`);
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      extra += `===== ISSUE CONTEXT =====\n(Issue 取得失敗: ${msg.slice(0, 200)})\n===== END ISSUE CONTEXT =====\n\n`;
    }

    // ② ADR excerpt
    if (opts.adrContextFile && existsSync(opts.adrContextFile)) {
      const adrText = readFileSync(opts.adrContextFile, "utf-8");
      extra += `===== ADR EXCERPT =====\n以下は本 Issue が前提とする設計決定（ADR 抜粋）。この方式自体への異議は挙げないこと。\n${adrText}\n===== END ADR EXCERPT =====\n\n`;
    }

    // ③ Non-Goals section
    const ngSection = planText.match(/^## Non-Goals\r?\n([\s\S]*?)(?=\r?\n## |\s*$)/m)?.[1] ?? "";
    const ngLines = ngSection.split("\n").filter((l) => l.trim()).join("\n");
    extra += `===== SCOPE DEFENSE =====\n以下は本 Issue のスコープ外。指摘・拡張提案・改善要求の対象としないこと。\n${ngLines}\n===== END SCOPE DEFENSE =====\n\n`;

    // ④ rejection.md
    if (opts.rejectionFile && existsSync(opts.rejectionFile)) {
      const rejText = readFileSync(opts.rejectionFile, "utf-8");
      extra += `===== PRIOR REJECTIONS =====\n以下は過去 round で棄却済み。蒸し返さないこと。\n${rejText}\n===== END PRIOR REJECTIONS =====\n\n`;
    }

    // ⑤ PRIOR JUDGMENTS (round 2+)
    if (n >= 2 && opts.judgmentSummaryFile && existsSync(opts.judgmentSummaryFile)) {
      const jText = readFileSync(opts.judgmentSummaryFile, "utf-8");
      extra += `===== PRIOR JUDGMENTS =====\n前 round で Claude が採用・棄却を判定済みの一覧。採用済み指摘は「足りない」と再指摘しない。棄却済み事項は再度持ち出さない。\n${jText}\n===== END PRIOR JUDGMENTS =====\n\n`;
    }

    // ⑥ PLAN DIFF (round 2+)
    if (n >= 2 && opts.planSnapshotDir) {
      const prevSnapshot = `${opts.planSnapshotDir}/plan.md.round-${n - 1}`;
      if (existsSync(prevSnapshot)) {
        const diffProc = Bun.spawn(["diff", "-u", prevSnapshot, opts.planFile], {
          stdout: "pipe",
        });
        const diffText = await new Response(diffProc.stdout).text();
        await diffProc.exited;
        if (diffText.trim()) {
          extra += `===== PLAN DIFF =====\n前 round からの plan 変更点（unified diff）。指摘対応として行われた変更箇所への「やり方が違う」指摘は必ず理由を添えること。\n${diffText}\n===== END PLAN DIFF =====\n\n`;
        }
      }
    }

    extraInputFile = `/tmp/3ai-extra-${Date.now()}-${Math.random().toString(36).slice(2)}.txt`;
    writeFileSync(extraInputFile, extra);
  }

  if (mode === "final" && opts.testSummaryFile && existsSync(opts.testSummaryFile)) {
    const sumText = readFileSync(opts.testSummaryFile, "utf-8");
    finalExtraFile = `/tmp/3ai-final-extra-${Date.now()}-${Math.random().toString(36).slice(2)}.txt`;
    writeFileSync(
      finalExtraFile,
      `===== TEST SUMMARY =====\nテスト実行結果サマリ（構造化 JSON）。coverage_hints をテスト網羅性評価の入力として使うこと。\n${sumText}\n===== END TEST SUMMARY =====\n\n`
    );
  }

  let codexExit = 0;
  try {
    if (mode === "design") {
      codexExit = await dispatchCodex({
        mode: "design",
        instructionFile: reviewInstructionPath,
        resultFile,
        inputFile: opts.inputFile,
        extraInputFile,
        scopeHint,
      });
    } else {
      codexExit = await dispatchCodex({
        mode: "review",
        instructionFile: reviewInstructionPath,
        resultFile,
        baseBranch: opts.baseBranch,
        extraInputFile: finalExtraFile,
        scopeHint,
      });
    }
  } finally {
    if (extraInputFile) try { unlinkSync(extraInputFile); } catch {}
    if (finalExtraFile) try { unlinkSync(finalExtraFile); } catch {}
  }
  if (codexExit !== 0) {
    throw new Error(`dispatch-codex-auto: Codex CLI failed (exit ${codexExit})`);
  }
}

if (import.meta.main) {
  let issueNum = "";
  let mode: "design" | "final" | undefined;
  let inputFile: string | undefined;
  let baseBranch: string | undefined;
  let planFile: string | undefined;
  let stateFile = "";
  let resultFile = "";
  let rejectionFile: string | undefined;
  let adrContextFile: string | undefined;
  let judgmentSummaryFile: string | undefined;
  let planSnapshotDir: string | undefined;
  let testSummaryFile: string | undefined;

  const args = process.argv.slice(2);
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--issue") issueNum = args[++i];
    else if (args[i] === "--mode") mode = args[++i] as "design" | "final";
    else if (args[i] === "--input") inputFile = args[++i];
    else if (args[i] === "--base") baseBranch = args[++i];
    else if (args[i] === "--plan") planFile = args[++i];
    else if (args[i] === "--state") stateFile = args[++i];
    else if (args[i] === "--result") resultFile = args[++i];
    else if (args[i] === "--rejection") rejectionFile = args[++i];
    else if (args[i] === "--adr-context") adrContextFile = args[++i];
    else if (args[i] === "--judgment-summary") judgmentSummaryFile = args[++i];
    else if (args[i] === "--plan-snapshot-dir") planSnapshotDir = args[++i];
    else if (args[i] === "--test-summary") testSummaryFile = args[++i];
    else { console.error(`Unknown arg: ${args[i]}`); process.exit(1); }
  }

  if (!issueNum || !mode || !stateFile || !resultFile) {
    console.error("ERROR: --issue, --mode, --state, and --result are required");
    process.exit(1);
  }

  await dispatchCodexAuto({
    issueNum,
    mode,
    inputFile,
    baseBranch,
    planFile,
    stateFile,
    resultFile,
    rejectionFile,
    adrContextFile,
    judgmentSummaryFile,
    planSnapshotDir,
    testSummaryFile,
  });
}
